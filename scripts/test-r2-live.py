"""Harness regression tests. Real local TLS/sockets; synthetic PG is NOT live PG acceptance."""
from pathlib import Path
import base64
import hashlib
import importlib.util
import json
import socket
import ssl
import struct
import tempfile
import threading
import unittest
from urllib.error import HTTPError
from urllib.parse import urlencode, urlsplit, parse_qs
from urllib.request import Request, build_opener, HTTPSHandler, HTTPRedirectHandler, ProxyHandler
import jwt

ROOT = Path(__file__).resolve().parents[1]

def load(name):
    spec=importlib.util.spec_from_file_location(name.replace('-','_'),ROOT/'scripts'/(name+'.py'))
    value=importlib.util.module_from_spec(spec); spec.loader.exec_module(value); return value

wire=load('r2-wire')

class Redirect(HTTPRedirectHandler):
    def redirect_request(self,*args): return None

class ProviderTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory()
        self.provider=load('r2-provider').Provider(self.temp.name,'http://127.0.0.1:1/api/v1/oidc/callback')
        self.opener=build_opener(ProxyHandler({}),HTTPSHandler(context=ssl.create_default_context(cafile=str(self.provider.ca_path))),Redirect())
    def tearDown(self): self.provider.close(); self.temp.cleanup()
    def request(self,path,data=None,headers=None):
        request=Request(self.provider.issuer+path,data=data,headers=headers or {})
        try: response=self.opener.open(request,timeout=3)
        except HTTPError as error: response=error
        with response: return response.status,response.headers,json.loads(response.read())
    def authorization(self):
        verifier='v'*64
        challenge=base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).rstrip(b'=').decode()
        status,headers,_=self.request('/authorize?'+urlencode(dict(client_id='r2-client',redirect_uri=self.provider.redirect,state='state',nonce='nonce',code_challenge=challenge,code_challenge_method='S256',response_type='code',scope='openid')))
        self.assertEqual(status,303)
        return parse_qs(urlsplit(headers['Location']).query)['code'][0],verifier
    def token(self,code,verifier):
        headers={'Authorization':'Basic '+base64.b64encode(b'r2-client:r2-disposable-client-secret').decode(),'Content-Type':'application/x-www-form-urlencoded'}
        return self.request('/token',urlencode(dict(code=code,code_verifier=verifier,redirect_uri=self.provider.redirect,grant_type='authorization_code')).encode(),headers)
    def test_actual_tls_metadata_and_no_private_jwks(self):
        status,_,metadata=self.request('/.well-known/openid-configuration')
        self.assertEqual(status,200); self.assertEqual(metadata['issuer'],self.provider.issuer)
        status,_,jwks=self.request('/jwks')
        self.assertEqual(status,200); self.assertNotIn('d',jwks['keys'][0])
    def test_exact_s256_and_one_shot_code(self):
        code,verifier=self.authorization(); status,_,token=self.token(code,verifier)
        self.assertEqual(status,200)
        decoded=jwt.decode(token['id_token'],self.provider.keys[0].public_key(),algorithms=['RS256'],issuer=self.provider.issuer,audience='r2-client')
        self.assertEqual(decoded['nonce'],'nonce'); self.assertEqual(self.token(code,verifier)[0],400)
    def test_bad_verifier_rejected_not_reusable(self):
        code,verifier=self.authorization(); self.assertEqual(self.token(code,'wrong')[0],400)
        self.assertEqual(self.token(code,verifier)[0],400)
    def test_rotation_and_signed_negative_modes(self):
        for mode in ('valid','wrong_issuer','wrong_audience','bad_signature','expired'):
            with self.subTest(mode=mode):
                self.provider.active=1; self.provider.mode=mode
                code,verifier=self.authorization(); status,_,token=self.token(code,verifier); self.assertEqual(status,200)
                if mode=='valid': jwt.decode(token['id_token'],self.provider.keys[1].public_key(),algorithms=['RS256'],issuer=self.provider.issuer,audience='r2-client')
                else:
                    with self.assertRaises(jwt.InvalidTokenError): jwt.decode(token['id_token'],self.provider.keys[1].public_key(),algorithms=['RS256'],issuer=self.provider.issuer,audience='r2-client')
    def test_outage(self):
        self.provider.mode='outage'; self.assertEqual(self.request('/.well-known/openid-configuration')[0],503)

class ProxyTests(unittest.TestCase):
    def test_commit_packet_scope_and_bounds(self):
        payload=b'COMMIT\0'; packet=b'Q'+struct.pack('!I',4+len(payload))+payload
        self.assertTrue(wire.is_commit(packet)); self.assertFalse(wire.is_commit(packet.replace(b'COMMIT',b'ROLLBK')))
        self.assertFalse(wire.is_commit(packet.replace(b'Q',b'P',1)))
    def test_all_fault_boundaries_on_synthetic_pg_sockets(self):
        for mode in ('before','after','lost-ack'):
            with self.subTest(mode=mode):
                listener=socket.socket(); listener.bind(('127.0.0.1',0)); listener.listen(1); listener.settimeout(3)
                received=threading.Event(); errors=[]
                def backend():
                    try:
                        conn,_=listener.accept(); conn.settimeout(3)
                        with conn:
                            n=struct.unpack('!I',wire.exact(conn,4))[0]; wire.exact(conn,n-4)
                            conn.sendall(b'Z'+struct.pack('!I',5)+b'I')
                            packet=wire.frame(conn)
                            if wire.is_commit(packet):
                                received.set(); conn.sendall(b'C'+struct.pack('!I',11)+b'COMMIT\0'+b'Z'+struct.pack('!I',5)+b'I')
                    except (EOFError,OSError): pass
                    except Exception as error: errors.append(str(error))
                thread=threading.Thread(target=backend); thread.start()
                proxy=wire.CommitProxy(listener.getsockname())
                client=socket.create_connection(proxy.address,timeout=3)
                try:
                    body=struct.pack('!I',196608)+b'user\0fixture\0\0'
                    client.sendall(struct.pack('!I',4+len(body))+body); self.assertEqual(wire.frame(client)[:1],b'Z')
                    proxy.arm(mode); client.sendall(b'Q'+struct.pack('!I',11)+b'COMMIT\0')
                    self.assertTrue(proxy.hit.wait(3)); self.assertEqual(received.is_set(),mode!='before')
                    proxy.release.set()
                    with self.assertRaises((EOFError,OSError)): wire.frame(client)
                finally:
                    client.close(); proxy.close(); listener.close(); thread.join(4)
                self.assertFalse(errors); self.assertFalse(proxy.errors); self.assertFalse(thread.is_alive())

class ApprovalTests(unittest.TestCase):
    def test_signature_payload_expiry_and_wrong_key(self):
        from datetime import datetime,timedelta,timezone
        from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
        from cryptography.hazmat.primitives.serialization import Encoding,PublicFormat
        verifier=load('r2-approval')
        with tempfile.TemporaryDirectory() as directory:
            private=Ed25519PrivateKey.generate(); public=Path(directory)/'unit-only-public.pem'
            public.write_bytes(private.public_key().public_bytes(Encoding.PEM,PublicFormat.SubjectPublicKeyInfo))
            value={'schema_version':1,'log_sha256':'a'*64,'adoption':{'decision':'approved','policy_reference':'SYNTHETIC-UNIT-NOT-ADOPTION','deployment_id':'unit-only','expires_at':(datetime.now(timezone.utc)+timedelta(hours=1)).isoformat()}}
            def sign():
                unsigned={k:v for k,v in value.items() if k!='approval_signature'}
                value['approval_signature']=base64.b64encode(private.sign(json.dumps(unsigned,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode())).decode()
            sign(); self.assertEqual(verifier.verify(value,public)['decision'],'approved')
            value['log_sha256']='b'*64
            with self.assertRaises(ValueError): verifier.verify(value,public)
            sign(); public.write_bytes(Ed25519PrivateKey.generate().public_key().public_bytes(Encoding.PEM,PublicFormat.SubjectPublicKeyInfo))
            with self.assertRaises(ValueError): verifier.verify(value,public)
            value['adoption']['expires_at']=(datetime.now(timezone.utc)-timedelta(hours=1)).isoformat(); sign()
            with self.assertRaises(ValueError): verifier.verify(value,public)

if __name__=='__main__': unittest.main(verbosity=2)
