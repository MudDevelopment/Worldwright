"""Explicit, fingerprint-bound enterprise CA repair in the owned acceptance distro.
Reads Windows stores only. No certificate downloads, TLS bypass, distro reset or
release approval. Resume the existing next stage only after verified Linux TLS.
"""
from pathlib import Path
from datetime import datetime,timezone
import argparse
import hashlib
import importlib.util
import json
import os
import re
import socket
import ssl
import subprocess
import sys
import uuid
from cryptography import x509
from cryptography.hazmat.primitives import serialization
import enterprise_ca as ca
import worldwright_local as local
import r3_io

SERVER_AUTH='1.3.6.1.5.5.7.3.1'
ALL_HOSTS=('static.rust-lang.org','nodejs.org','pypi.org','files.pythonhosted.org',
           'index.crates.io','static.crates.io','cdn.playwright.dev')

INSTALL = r'''
import hashlib,json,os,re,ssl,stat,subprocess,sys,uuid
from pathlib import Path
if os.geteuid()!=0:raise ValueError('scoped certificate installation requires root in the owned distro')
transport=Path(sys.argv[1]);expected=sys.argv[2];nonce=sys.argv[3]
anchor='93d1dbce2c583b226bbd80c6d4480ce99bf8c9de6f9bca25f2929ecc38f05741'
root=Path('/home/worldwright/Worldwright');owner=Path('/var/lib/worldwright-provision/ownership')
if owner.read_text()!=nonce or root.stat().st_dev!=Path('/').stat().st_dev:raise ValueError('guest ownership/backing mismatch')
if (root/'.worldwright-local/linux-stage.lock').exists():raise ValueError('Linux stage may still run; no trust change during execution')
raw=(transport/'install-request.json').read_bytes()
if len(raw)>65536 or hashlib.sha256(raw).hexdigest()!=expected:raise ValueError('certificate request changed')
request=json.loads(raw)
if request.get('anchor_sha256')!=anchor or not 2<=len(request['certificates'])<=8:raise ValueError('expected subordinate CA path to self-signed root')
certificates=[];seen=set()
for item in request['certificates']:
    digest=item['der_sha256']
    if not re.fullmatch('[0-9a-f]{64}',digest) or digest in seen:raise ValueError('certificate identity/duplication')
    seen.add(digest);data=(transport/(digest+'.pem')).read_bytes()
    if not 0<len(data)<=32768 or data.count(b'-----BEGIN CERTIFICATE-----')!=1:raise ValueError('one bounded certificate required')
    actual=hashlib.sha256(ssl.PEM_cert_to_DER_cert(data.decode('ascii'))).hexdigest()
    if actual!=digest or hashlib.sha256(data).hexdigest()!=item['pem_sha256']:raise ValueError('certificate bytes changed')
    certificates.append((digest,data))
if certificates[0][0]!=anchor or not request['certificates'][-1]['self_signed_trusted_root']:raise ValueError('full approved chain required')
# Root-owned staging in the approved distro; do not run privileged parsers against
# files that could change on the Windows transport after hash validation.
staging=Path('/var/lib/worldwright-provision')/('ca-stage-'+uuid.uuid4().hex);staging.mkdir(mode=0o700)
for digest,data in certificates:
    with (staging/(digest+'.crt')).open('xb') as stream:stream.write(data)
bundle=b''.join(data for _,data in certificates)
with (staging/'chain.pem').open('xb') as stream:stream.write(bundle)
subprocess.run(['/usr/bin/openssl','verify','-x509_strict','-CAfile',str(staging/(certificates[-1][0]+'.crt')),
                '-untrusted',str(staging/'chain.pem'),str(staging/(anchor+'.crt'))],check=True,stdout=sys.stderr)
def checked(path):
    for item in (path,*path.parents):
        if item.is_symlink():raise ValueError('linked trust path refused')
    if path.exists():
        info=path.stat()
        if info.st_uid!=0 or stat.S_IMODE(info.st_mode)&0o022 or info.st_dev!=Path('/').stat().st_dev:raise ValueError('unsafe trust-file ownership/mode/backing')
    return path
config=checked(Path('/etc/worldwright-enterprise'));store=checked(Path('/usr/local/share/ca-certificates/worldwright-enterprise'))
for directory in (config,store):
    directory.mkdir(mode=0o755,exist_ok=True)
    marker=checked(directory/'ownership')
    if marker.exists():
        if marker.read_text()!=nonce:raise ValueError('enterprise trust directory belongs to another setup')
    else:
        if list(directory.iterdir()):raise ValueError('unowned existing trust directory refused')
        with marker.open('x') as stream:stream.write(nonce)
        marker.chmod(0o644)
receipt={'schema_version':1,'anchor_sha256':anchor,'chain_sha256':[digest for digest,_ in certificates],
         'bundle_sha256':hashlib.sha256(bundle).hexdigest(),'scope':'owned acceptance distro only',
         'node_extra_ca_certs':'/etc/worldwright-enterprise/chain.pem','tls_verification_disabled':False}
receipt_bytes=(json.dumps(receipt,indent=2,sort_keys=True)+'\n').encode()
planned={store/(digest+'.crt'):data for digest,data in certificates}
planned[config/'chain.pem']=bundle;planned[config/'receipt.json']=receipt_bytes
if {p.name for p in store.iterdir()}-({'ownership'}|{digest+'.crt' for digest,_ in certificates}):raise ValueError('unexpected certificates in owned trust directory')
for path,data in planned.items():
    checked(path)
    if path.exists() and path.read_bytes()!=data:raise ValueError('different certificate configuration exists; no silent replacement')
before=hashlib.sha256(Path('/etc/ssl/certs/ca-certificates.crt').read_bytes()).hexdigest()
for path,data in planned.items():
    if path.name=='receipt.json' or path.exists():continue
    temporary=path.with_name(path.name+'.'+uuid.uuid4().hex)
    with temporary.open('xb') as stream:stream.write(data);stream.flush();os.fsync(stream.fileno())
    temporary.chmod(0o644);temporary.rename(path)
subprocess.run(['/usr/sbin/update-ca-certificates'],check=True,stdout=sys.stderr)
receipt_path=config/'receipt.json'
if not receipt_path.exists():
    with receipt_path.open('xb') as stream:stream.write(receipt_bytes);stream.flush();os.fsync(stream.fileno())
    receipt_path.chmod(0o644)
after=hashlib.sha256(Path('/etc/ssl/certs/ca-certificates.crt').read_bytes()).hexdigest()
print('TRUST_INSTALL_JSON:'+json.dumps({'receipt':receipt,'system_bundle_before_sha256':before,'system_bundle_after_sha256':after,
                                     'windows_or_other_distros_modified':False}))
'''

LINUX_VERIFY = r'''
import hashlib,json,socket,ssl,sys
hosts=json.loads(sys.argv[1]);context=ssl.create_default_context()
if not context.check_hostname or context.verify_mode!=ssl.CERT_REQUIRED:raise ValueError('normal TLS verification required')
# Require the full issuer chain: no partial-chain workaround is introduced.
context.verify_flags &= ~ssl.VERIFY_X509_PARTIAL_CHAIN
results=[]
for host in hosts:
    item={'host':host,'verified':False}
    try:
        with socket.create_connection((host,443),timeout=10) as raw:
            with context.wrap_socket(raw,server_hostname=host) as tls:
                item.update(verified=True,tls_version=tls.version(),leaf_sha256=hashlib.sha256(tls.getpeercert(binary_form=True)).hexdigest())
    except Exception as error:item.update(error_type=type(error).__name__,error=str(error)[:1000])
    results.append(item)
print(json.dumps({'hosts':results,'verify_flags':int(context.verify_flags),'partial_chain_enabled':False,'all_verified':all(item['verified'] for item in results)}))
sys.exit(0 if all(item['verified'] for item in results) else 1)
'''


def load(name):
    spec=importlib.util.spec_from_file_location(name.replace('-','_'),local.ROOT/'scripts'/(name+'.py'))
    value=importlib.util.module_from_spec(spec);spec.loader.exec_module(value);return value


def write_json(path,value):
    with path.open('x',encoding='utf-8',newline='\n') as stream:json.dump(value,stream,indent=2,sort_keys=True);stream.write('\n')


def windows_pool():
    pool={};trusted=set()
    for store in ('ROOT','CA'):
        for der,encoding,trust in ssl.enum_certificates(store):
            if encoding!='x509_asn':continue
            try:cert=x509.load_der_x509_certificate(der)
            except ValueError:continue
            digest=ca.fingerprint(cert);pool[digest]=cert
            if store=='ROOT' and (trust is True or SERVER_AUTH in trust):trusted.add(digest)
    return pool,trusted


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--approve-rsta-chain',action='store_true');parser.add_argument('--resume',action='store_true');args=parser.parse_args()
    if os.name!='nt' or not args.approve_rsta_chain:parser.error('Windows and explicit --approve-rsta-chain required')
    local.local_path(Path(sys.prefix));local.configure_tools_environment()
    out=local.new_output(local.LOCAL/'evidence'/('wsl-trust-repair-'+uuid.uuid4().hex));out.mkdir()
    print('TRUST REPAIR EVIDENCE: '+str(out),flush=True)
    report={'kind':'scoped-enterprise-ca-repair','schema_version':1,'phases':[],'status':'failed','acceptance_passed':False,
            'approved_anchor_sha256':ca.RSTA_SHA256,'installation_attempted':False,'windows_trust_modified':False}
    provision=load('provision-acceptance-wsl');wsl=str(Path(os.environ['SystemRoot'])/'System32/wsl.exe')
    lock=local.local_path(local.LOCAL/'wsl/linux-next.lock');acquired=False
    def phase(name,argv,timeout=120):
        print('Trust phase: '+name,flush=True)
        result=r3_io.record_phase(report,out,name,[str(v) for v in argv],timeout,cwd=local.ROOT,sensitive=r3_io.secret_values())
        ca.require(result['returncode']==0 and result['interruption'] is None,'phase failed: '+name+'; preserve evidence before retry')
        return result['stdout']
    def guest(command,user='worldwright'):
        base='/home/worldwright/Worldwright/.worldwright-local'
        return [wsl,'--distribution',provision.DISTRO,'--user',user,'--cd','/','--exec','/usr/bin/env','-i',
                'PATH=/usr/sbin:/usr/bin:/sbin:/bin','HOME='+base+'/tool-home','TMPDIR='+base+'/temp',
                'LANG=C.UTF-8','PYTHONDONTWRITEBYTECODE=1',*command]
    try:
        with lock.open('x') as stream:stream.write(str(os.getpid()))
        acquired=True
        output=phase('trust-helper-tests',[sys.executable,'-B',local.ROOT/'scripts/test-enterprise-ca.py'])
        ca.require(re.findall(rb'Ran (\d+) tests? in ',output)==[b'6'],'trust helper inventory differs')
        state=json.loads(local.local_path(local.LOCAL/'wsl/provision-state.json').read_text())
        ca.require(state.get('os_ready') and state.get('storage_verified') and not state.get('pending_phase'),'owned distro not ready')
        provision.same_registration(provision.probe.registrations().get(provision.DISTRO,{}),local.LOCAL/'wsl'/provision.DISTRO)
        pool,trusted=windows_pool();chain=ca.resolve_chain(ca.RSTA_SHA256,pool,trusted)
        request={'schema_version':1,'anchor_sha256':ca.RSTA_SHA256,'certificates':[]}
        for index,cert in enumerate(chain):
            digest=ca.fingerprint(cert);pem=cert.public_bytes(serialization.Encoding.PEM)
            with (out/(digest+'.pem')).open('xb') as stream:stream.write(pem)
            item={'der_sha256':digest,'pem_sha256':hashlib.sha256(pem).hexdigest(),
                  'subject':cert.subject.rfc4514_string(),'issuer':cert.issuer.rfc4514_string(),
                  'self_signed_trusted_root':index==len(chain)-1 and digest in trusted,
                  'not_after':cert.not_valid_after_utc.isoformat()}
            request['certificates'].append(item)
            print('Verified CA: '+item['subject']+' | SHA256 '+digest,flush=True)
        report['resolved_chain']=request['certificates'];bundle=b''.join(cert.public_bytes(serialization.Encoding.PEM) for cert in chain)
        # Complete-chain verification against only the resolved CA path, not the
        # Windows partial-chain store behavior that hid the missing root in Linux.
        context=ssl.create_default_context(cadata=bundle.decode('ascii'))
        context.verify_flags &= ~ssl.VERIFY_X509_PARTIAL_CHAIN
        windows=[]
        for host in ca.HOSTS:
            print('Full-chain Windows TLS: '+host,flush=True)
            with socket.create_connection((host,443),timeout=10) as raw:
                with context.wrap_socket(raw,server_hostname=host) as tls:
                    windows.append({'host':host,'verified':True,'leaf_sha256':hashlib.sha256(tls.getpeercert(binary_form=True)).hexdigest()})
        report['windows_full_chain_verified']=windows
        write_json(out/'install-request.json',request)
        mapped=provision.probe.decode_wsl(phase('map-trust-request',guest(['/usr/bin/wslpath','-u',str(out)])))
        ca.require(mapped.startswith('/') and '\n' not in mapped,'invalid mapped evidence path')
        report['installation_attempted']=True
        raw=phase('install-scoped-ca',guest(['/usr/bin/python3','-I','-B','-c',INSTALL,mapped,
                    hashlib.sha256((out/'install-request.json').read_bytes()).hexdigest(),state['nonce']],user='root'),180)
        lines=[line[len(b'TRUST_INSTALL_JSON:'):] for line in raw.splitlines() if line.startswith(b'TRUST_INSTALL_JSON:')]
        ca.require(len(lines)==1,'missing unique trust installation result');report['installation']=json.loads(lines[0])
        raw=phase('verify-linux-https',guest(['/usr/bin/python3','-I','-B','-c',LINUX_VERIFY,json.dumps(ALL_HOSTS)]),180)
        report['linux_https']=json.loads(raw.decode('utf-8'));ca.require(report['linux_https']['all_verified'],'Linux TLS remains blocked')
        report['status']='scoped-trust-installed-https-verified'
    except (Exception,KeyboardInterrupt) as error:
        report['error']=r3_io.log_bytes(str(error).encode(),r3_io.secret_values(),4000).decode('utf-8','replace')
        print('Trust repair stopped: '+report['error'],flush=True)
    finally:
        if acquired:lock.unlink()
        write_json(out/'trust-repair.json',report)
    print(report['status']+': '+str(out/'trust-repair.json'),flush=True)
    if report['status']!='scoped-trust-installed-https-verified':return 1
    if args.resume:
        print('Resuming the existing Linux next stage; no reprovision or reset.',flush=True)
        result=subprocess.run([sys.executable,'-B',str(local.ROOT/'scripts/linux-next-stage.py'),'--fresh-databases-only'],cwd=local.ROOT)
        write_json(out/'resume-result.json',{'exit_code':result.returncode,'trust_report':'trust-repair.json','acceptance_passed':False})
        return result.returncode
    return 0


if __name__=='__main__':raise SystemExit(main())
