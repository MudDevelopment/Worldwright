"""Read-only TLS trust comparison for the owned WSL acceptance distro.
Exports public certificates only from verified Windows handshakes; never installs
trust, disables verification, requests credentials, or touches migration/DB state.
"""
from pathlib import Path
import hashlib
import importlib.util
import json
import os
import socket
import ssl
import sys
import uuid
from cryptography import x509
from cryptography.hazmat.primitives import serialization
import worldwright_local as local
import r3_io

HOSTS=('static.rust-lang.org','nodejs.org','pypi.org','files.pythonhosted.org',
       'index.crates.io','static.crates.io','cdn.playwright.dev')
SERVER_AUTH='1.3.6.1.5.5.7.3.1'

LINUX = r'''
import hashlib,json,socket,ssl,sys,time
hosts=json.loads(sys.argv[1]);expected=('static.rust-lang.org','nodejs.org','pypi.org','files.pythonhosted.org','index.crates.io','static.crates.io','cdn.playwright.dev')
if tuple(hosts)!=expected:raise ValueError('only reviewed download hosts allowed')
context=ssl.create_default_context()
assert context.check_hostname and context.verify_mode==ssl.CERT_REQUIRED
result={'platform':'linux','openssl':ssl.OPENSSL_VERSION,'time_utc_epoch':int(time.time()),
        'verify_flags':int(context.verify_flags),'verify_paths':ssl.get_default_verify_paths()._asdict(),
        'hosts':[],'system_trust_modified':False}
for host in hosts:
    item={'host':host,'port':443,'verified':False}
    try:
        with socket.create_connection((host,443),timeout=10) as raw:
            with context.wrap_socket(raw,server_hostname=host) as tls:
                item.update(verified=True,tls_version=tls.version(),leaf_sha256=hashlib.sha256(tls.getpeercert(binary_form=True)).hexdigest())
    except Exception as error:
        item.update(error_type=type(error).__name__,error=str(error)[:1000])
        if isinstance(error,ssl.SSLCertVerificationError):item.update(verify_code=error.verify_code,verify_message=error.verify_message)
    result['hosts'].append(item)
result['loaded_ca_sha256']=sorted(hashlib.sha256(cert).hexdigest() for cert in context.get_ca_certs(binary_form=True))
print(json.dumps(result,indent=2,sort_keys=True))
'''


def write_new(path,value):
    with path.open('x',encoding='utf-8',newline='\n') as stream:json.dump(value,stream,indent=2,sort_keys=True);stream.write('\n')


def main():
    if os.name!='nt':raise SystemExit('use the existing repository-local Windows security Python')
    local.local_path(Path(sys.prefix));local.configure_tools_environment()
    out=local.new_output(local.LOCAL/'evidence'/('wsl-tls-'+uuid.uuid4().hex));out.mkdir()
    print('TLS EVIDENCE: '+str(out),flush=True)
    report={'kind':'windows-wsl-tls-comparison','phases':[],'trust_modified':False,'acceptance_passed':False,'status':'failed'}
    spec=importlib.util.spec_from_file_location('owned_wsl_provision',local.ROOT/'scripts/provision-acceptance-wsl.py')
    provision=importlib.util.module_from_spec(spec);spec.loader.exec_module(provision)
    try:
        state=json.loads(local.local_path(local.LOCAL/'wsl/provision-state.json').read_text())
        if not state.get('os_ready') or state.get('pending_phase'):raise ValueError('owned distro not ready')
        provision.same_registration(provision.probe.registrations().get(provision.DISTRO,{}),local.LOCAL/'wsl'/provision.DISTRO)
        trusted_roots={hashlib.sha256(data).hexdigest() for data,encoding,trust in ssl.enum_certificates('ROOT')
                       if encoding=='x509_asn' and (trust is True or SERVER_AUTH in trust)}
        context=ssl.create_default_context()
        if not context.check_hostname or context.verify_mode!=ssl.CERT_REQUIRED:raise ValueError('normal TLS verification required')
        windows={'openssl':ssl.OPENSSL_VERSION,'verify_flags':int(context.verify_flags),'hosts':[],
                 'tls_override_variables_present':sorted(key for key in ('SSL_CERT_FILE','SSL_CERT_DIR') if os.environ.get(key))}
        candidates={}
        for host in HOSTS:
            print('Windows verified TLS: '+host,flush=True)
            item={'host':host,'verified':False,'chain':[]}
            try:
                with socket.create_connection((host,443),timeout=10) as raw:
                    with context.wrap_socket(raw,server_hostname=host) as tls:
                        # Python3.14 API: called only after a verified hostname-checked handshake.
                        chain=tls.get_verified_chain();item.update(verified=True,tls_version=tls.version())
                if not chain:raise ValueError('no verified chain available; no certificate is exported')
                for index,der in enumerate(chain):
                    cert=x509.load_der_x509_certificate(der);digest=hashlib.sha256(der).hexdigest()
                    try:is_ca=cert.extensions.get_extension_for_class(x509.BasicConstraints).value.ca
                    except x509.ExtensionNotFound:is_ca=False
                    metadata={'sha256':digest,'subject':cert.subject.rfc4514_string(),'issuer':cert.issuer.rfc4514_string(),
                              'not_before':cert.not_valid_before_utc.isoformat(),'not_after':cert.not_valid_after_utc.isoformat(),
                              'ca':is_ca,'present_in_windows_server_auth_ROOT_store':digest in trusted_roots}
                    item['chain'].append(metadata)
                    filename=host+'-chain-'+str(index)+'-'+digest+'.pem'
                    with (out/filename).open('xb') as stream:stream.write(cert.public_bytes(serialization.Encoding.PEM))
                    if index==len(chain)-1 and is_ca and digest in trusted_roots:
                        if digest not in candidates:candidates[digest]={**metadata,'hosts':[],'pem_file':filename}
                        candidates[digest]['hosts'].append(host)
            except Exception as error:
                item['error_type']=type(error).__name__;item['error']=str(error)[:1000]
                if isinstance(error,ssl.SSLCertVerificationError):item.update(verify_code=error.verify_code,verify_message=error.verify_message)
            windows['hosts'].append(item)
        report['windows']=windows
        wsl=Path(os.environ['SystemRoot'])/'System32/wsl.exe';root='/home/worldwright/Worldwright'
        argv=[str(wsl),'--distribution',provision.DISTRO,'--user','worldwright','--cd',root,'--exec','/usr/bin/env','-i',
              'PATH=/usr/bin:/bin','HOME='+root+'/.worldwright-local/tool-home','TMPDIR='+root+'/.worldwright-local/temp',
              'LANG=C.UTF-8','PYTHONDONTWRITEBYTECODE=1','/usr/bin/python3','-I','-B','-c',LINUX,json.dumps(HOSTS)]
        print('Linux verified TLS checks (up to ten seconds per host)',flush=True)
        result=r3_io.record_phase(report,out,'linux-tls',argv,180,cwd=local.ROOT,sensitive=r3_io.secret_values())
        if result['returncode']!=0 or result['interruption'] is not None:raise ValueError('Linux diagnostic process failed; inspect linux-tls.log')
        linux=json.loads(result['stdout'].decode('utf-8'));report['linux']=linux
        linux_roots=set(linux['loaded_ca_sha256'])
        for item in candidates.values():item['present_in_linux_loaded_CA_set']=item['sha256'] in linux_roots
        report['verified_windows_root_candidates']=list(candidates.values())
        report['status']='diagnostic-complete-review-required'
        for item in report['verified_windows_root_candidates']:
            print('Verified Windows root: '+item['subject']+' | SHA256 '+item['sha256']+
                  ' | present in Linux loaded CA set: '+str(item['present_in_linux_loaded_CA_set']),flush=True)
        print('No trust stores or download verification settings were changed.',flush=True)
    except Exception as error:
        report['error']=r3_io.log_bytes(str(error).encode(),r3_io.secret_values(),3000).decode('utf-8','replace')
        print('TLS diagnostic stopped: '+report['error'],flush=True)
    finally:write_new(out/'tls-report.json',report)
    print(report['status']+': '+str(out/'tls-report.json'),flush=True)
    return 0 if report['status']=='diagnostic-complete-review-required' else 1


if __name__=='__main__':raise SystemExit(main())
