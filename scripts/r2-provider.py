"""Controlled loopback HTTPS OIDC fixture. NOT approved-provider evidence.
Ephemeral RSA keys, strict code/redirect/client/S256 binding, no real accounts.
"""
from datetime import datetime, timedelta, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import base64
import hashlib
import json
import secrets
import ssl
import threading
from urllib.parse import parse_qs, urlencode, urlsplit
import jwt
from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import rsa
from cryptography.x509.oid import NameOID


class Provider:
    def __init__(self, directory, redirect_uri):
        self.directory = Path(directory)
        self.redirect = redirect_uri
        self.mode = 'valid'
        self.subject = 'r2-alice'
        self.active = 0
        self.codes = {}
        self.lock = threading.Lock()
        self.observations = []
        self.keys = [rsa.generate_private_key(public_exponent=65537, key_size=2048) for _ in range(2)]
        outer = self

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def reply(self, status, value, location=None):
                body = json.dumps(value).encode()
                self.send_response(status)
                self.send_header('Content-Type', 'application/json')
                self.send_header('Cache-Control', 'no-store')
                if location:
                    self.send_header('Location', location)
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def do_GET(self):
                path = urlsplit(self.path).path
                if outer.mode == 'outage':
                    self.reply(503, {'error': 'fixture_unavailable'})
                elif path == '/.well-known/openid-configuration':
                    self.reply(200, {'issuer': outer.issuer, 'authorization_endpoint': outer.issuer+'/authorize',
                        'token_endpoint': outer.issuer+'/token', 'jwks_uri': outer.issuer+'/jwks',
                        'response_types_supported': ['code'], 'subject_types_supported': ['public'],
                        'id_token_signing_alg_values_supported': ['RS256'],
                        'token_endpoint_auth_methods_supported': ['client_secret_basic'],
                        'code_challenge_methods_supported': ['S256']})
                elif path == '/jwks':
                    self.reply(200, {'keys': [outer.jwk(outer.active)]})
                elif path == '/authorize':
                    q = parse_qs(urlsplit(self.path).query, strict_parsing=True)
                    required = ('client_id', 'redirect_uri', 'state', 'nonce', 'code_challenge', 'code_challenge_method', 'response_type')
                    if any(len(q.get(k, [])) != 1 for k in required) or q['client_id'][0] != 'r2-client' or q['redirect_uri'][0] != outer.redirect or q['response_type'][0] != 'code' or q['code_challenge_method'][0] != 'S256':
                        self.reply(400, {'error': 'invalid_request'})
                        return
                    code = secrets.token_urlsafe(32)
                    with outer.lock:
                        outer.codes[code] = {k: v[0] for k, v in q.items()}
                    state = q['state'][0] if outer.mode != 'wrong_state' else 'wrong-state'
                    outer.observations.append('authorization-state-nonce-s256-bound')
                    self.reply(303, {}, outer.redirect+'?'+urlencode({'code': code, 'state': state}))
                else:
                    self.reply(404, {})

            def do_POST(self):
                length = int(self.headers.get('Content-Length', '0'))
                if urlsplit(self.path).path != '/token' or not 0 < length <= 8192:
                    self.reply(400, {})
                    return
                if outer.mode == 'outage':
                    self.reply(503, {'error': 'fixture_unavailable'})
                    return
                q = parse_qs(self.rfile.read(length).decode(), strict_parsing=True)
                credentials = 'Basic '+base64.b64encode(b'r2-client:r2-disposable-client-secret').decode()
                if self.headers.get('Authorization') != credentials:
                    self.reply(401, {'error': 'invalid_client'})
                    return
                with outer.lock:
                    record = outer.codes.pop(q.get('code', [''])[0], None)
                verifier = q.get('code_verifier', [''])[0]
                challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).rstrip(b'=').decode()
                if not record or q.get('grant_type') != ['authorization_code'] or q.get('redirect_uri') != [outer.redirect] or challenge != record['code_challenge'] or outer.mode == 'bad_pkce':
                    self.reply(400, {'error': 'invalid_grant'})
                    outer.observations.append('token-rejected-binding')
                    return
                now = int(datetime.now(timezone.utc).timestamp())
                claims = {'iss': outer.issuer, 'aud': 'r2-client', 'sub': outer.subject, 'nonce': record['nonce'], 'iat': now, 'exp': now+300}
                if outer.mode == 'wrong_issuer': claims['iss'] = 'https://wrong.example.invalid'
                if outer.mode == 'wrong_audience': claims['aud'] = 'wrong-client'
                if outer.mode == 'wrong_nonce': claims['nonce'] = 'wrong-nonce'
                if outer.mode == 'expired': claims['exp'] = now-60
                key = outer.keys[1-outer.active] if outer.mode == 'bad_signature' else outer.keys[outer.active]
                token = jwt.encode(claims, key, algorithm='RS256', headers={'kid': 'r2-'+str(outer.active)})
                outer.observations.append('token-exact-s256-verified')
                self.reply(200, {'access_token': secrets.token_urlsafe(32), 'token_type': 'Bearer', 'expires_in': 300, 'id_token': token})

        self.server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.issuer = 'https://localhost:'+str(self.server.server_address[1])
        ca_key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
        name = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, 'Worldwright R2 disposable CA')])
        now = datetime.now(timezone.utc)
        ca = x509.CertificateBuilder().subject_name(name).issuer_name(name).public_key(ca_key.public_key()).serial_number(x509.random_serial_number()).not_valid_before(now-timedelta(minutes=1)).not_valid_after(now+timedelta(days=1)).add_extension(x509.BasicConstraints(ca=True, path_length=0), critical=True).sign(ca_key, hashes.SHA256())
        leaf_key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
        leaf = x509.CertificateBuilder().subject_name(x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, 'localhost')])).issuer_name(name).public_key(leaf_key.public_key()).serial_number(x509.random_serial_number()).not_valid_before(now-timedelta(minutes=1)).not_valid_after(now+timedelta(days=1)).add_extension(x509.SubjectAlternativeName([x509.DNSName('localhost')]), critical=False).sign(ca_key, hashes.SHA256())
        self.ca_path = self.directory/'fixture-ca.pem'
        self.ca_path.write_bytes(ca.public_bytes(serialization.Encoding.PEM))
        cert_path, key_path = self.directory/'fixture-cert.pem', self.directory/'fixture-key.pem'
        cert_path.write_bytes(leaf.public_bytes(serialization.Encoding.PEM))
        key_path.write_bytes(leaf_key.private_bytes(serialization.Encoding.PEM, serialization.PrivateFormat.PKCS8, serialization.NoEncryption()))
        key_path.chmod(0o600)
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(cert_path, key_path)
        self.server.socket = context.wrap_socket(self.server.socket, server_side=True)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    def jwk(self, index):
        key = json.loads(jwt.algorithms.RSAAlgorithm.to_jwk(self.keys[index].public_key()))
        key.update(kid='r2-'+str(index), use='sig', alg='RS256')
        return key

    def close(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(3)
        if self.thread.is_alive():
            raise ValueError('provider did not terminate')
        for name in ('fixture-key.pem', 'fixture-cert.pem', 'fixture-ca.pem'):
            (self.directory/name).unlink(missing_ok=True)
        self.keys.clear()
        self.codes.clear()
