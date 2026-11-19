"""Provision ONLY a separate repository-backed Ubuntu24.04 WSL2 distro.
Downloads signed official rootfs; verifies before import; preserves existing distros.
No project migration, database cluster, Rust build, test acceptance or signing.
Run with the repository-local Windows Python and --confirm-create.
"""
from pathlib import Path
import argparse
import base64
import gzip
import hashlib
import importlib.util
import json
import os
import re
import shutil
import sys
import time
import urllib.request
import urllib.parse
import uuid
import worldwright_local as local
import r3_io

_spec=importlib.util.spec_from_file_location('wsl_probe',Path(__file__).with_name('wsl-acceptance-preflight.py'))
probe=importlib.util.module_from_spec(_spec);_spec.loader.exec_module(probe)
DISTRO='Worldwright-Acceptance-24.04'
VERIFIER='Ubuntu-24.04'
BASE='https://cloud-images.ubuntu.com/wsl/releases/24.04/current/'
IMAGE='ubuntu-noble-wsl-amd64-wsl.rootfs.tar.gz'
FINGERPRINT='D2EB44626FDDC30B513D5BB71A5D6C4C7DB87C81'
KEY_URL='https://keyserver.ubuntu.com/pks/lookup?op=get&search=0x'+FINGERPRINT
# Historical manifests may also carry the Canonical cloud-builder signature.
SECONDARY_KEY='4A3CE3CD565D7EB5C810E2B97FF3F408476CF100'
ALLOWED_HOSTS={'cloud-images.ubuntu.com','keyserver.ubuntu.com'}

BOOTSTRAP = r'''
import configparser,json,os,pwd,subprocess,sys
from pathlib import Path
if os.geteuid()!=0:raise ValueError('new-distro setup requires root')
release=Path('/etc/os-release').read_text()
if '\nID=ubuntu\n' not in '\n'+release or 'VERSION_ID="24.04"' not in release:raise ValueError('wrong OS image')
nonce=sys.argv[1]
if len(nonce)!=32 or any(c not in '0123456789abcdef' for c in nonce):raise ValueError('invalid ownership nonce')
root=Path('/var/lib/worldwright-provision');root.mkdir(mode=0o700,exist_ok=True)
marker=root/'ownership'
if marker.exists():
    if marker.read_text()!=nonce:raise ValueError('guest ownership differs')
else:
    with marker.open('x') as stream:stream.write(nonce)
    marker.chmod(0o600)
def replace_owned(path,data,mode=0o644):
    if path.is_symlink():raise ValueError('linked guest config refused')
    if path.exists() and path.read_text()==data:return
    backup=root/(path.name+'.original')
    if path.exists() and not backup.exists():
        with backup.open('xb') as stream:stream.write(path.read_bytes())
    path.parent.mkdir(parents=True,exist_ok=True)
    temp=path.with_name(path.name+'.worldwright-new')
    with temp.open('x') as stream:stream.write(data)
    temp.chmod(mode);os.replace(temp,path)
# All writes here are in the new distro's explicitly repository-backed disk.
config=Path('/etc/wsl.conf');cp=configparser.ConfigParser()
if config.exists():cp.read(config)
for section,values in {'user':{'default':'worldwright'},'boot':{'systemd':'false'},
                       'interop':{'appendWindowsPath':'false'}}.items():
    if not cp.has_section(section):cp.add_section(section)
    for key,value in values.items():cp.set(section,key,value)
import io
text=io.StringIO();cp.write(text);replace_owned(config,text.getvalue())
# Do not allow package installation to create/start an unsolicited PG cluster.
replace_owned(Path('/usr/sbin/policy-rc.d'),'#!/bin/sh\nexit 101\n',0o755)
replace_owned(Path('/etc/postgresql-common/createcluster.conf'),'create_main_cluster = false\n')
try:user=pwd.getpwnam('worldwright')
except KeyError:
    subprocess.run(['/usr/sbin/useradd','--create-home','--user-group','--shell','/bin/bash','worldwright'],check=True)
    user=pwd.getpwnam('worldwright')
if user.pw_uid==0 or user.pw_dir!='/home/worldwright':raise ValueError('unexpected test-user identity')
workspace=Path(user.pw_dir)/'Worldwright'
for path in (workspace,workspace/'.worldwright-local',workspace/'.worldwright-local'/'evidence'):
    if path.is_symlink():raise ValueError('linked workspace refused')
    path.mkdir(mode=0o700,exist_ok=True);os.chown(path,user.pw_uid,user.pw_gid);path.chmod(0o700)
print(json.dumps({'uid':user.pw_uid,'gid':user.pw_gid,'workspace':str(workspace),'ownership':nonce,'database_started':False}))
'''

POSIX_CHECK = r'''
import json,os,pwd,stat,sys
from pathlib import Path
if os.geteuid()==0 or pwd.getpwuid(os.geteuid()).pw_name!='worldwright':raise ValueError('non-root test user required')
root=Path('/home/worldwright/Worldwright');out=root/'.worldwright-local'/'evidence'/('storage-'+sys.argv[1])
os.umask(0o077);out.mkdir(mode=0o700)
f=out/'mode-check'
with f.open('xb') as stream:stream.write(b'nonsecret POSIX check\n')
a=stat.S_IMODE(f.stat().st_mode);f.chmod(0o400);b=stat.S_IMODE(f.stat().st_mode)
denied=False
try:
    fd=os.open(f,os.O_WRONLY);os.close(fd)
except PermissionError:denied=True
f.chmod(0o600);link=out/'symlink';link.symlink_to('mode-check');hard=out/'hardlink';os.link(f,hard)
mounts=Path('/proc/self/mountinfo').read_text().splitlines()
root_mount=[line for line in mounts if line.split(' - ',1)[0].split()[4]=='/']
root_ext4=len(root_mount)==1 and root_mount[0].split(' - ',1)[1].split()[0]=='ext4'
# Workspace must reside on the same ext4 device; reject an overmounted /home.
passed=(a==0o600 and b==0o400 and stat.S_IMODE(out.stat().st_mode)==0o700 and denied
        and link.is_symlink() and f.samefile(hard) and f.stat().st_uid==os.getuid()
        and root_ext4 and root.stat().st_dev==Path('/').stat().st_dev)
value={'uid':os.getuid(),'directory_mode':oct(stat.S_IMODE(out.stat().st_mode)),
       'file_mode':oct(a),'readonly_mode':oct(b),'write_denied':denied,'ext4_root':root_ext4,
       'workspace':str(root),'evidence':str(out),'posix_passed':passed,'acceptance_passed':False}
with (out/'storage.json').open('x') as stream:json.dump(value,stream,sort_keys=True,indent=2)
print(json.dumps(value));sys.exit(0 if passed else 1)
'''


def require(value,message):
    if not value:raise ValueError(message)


def file_hash(path):
    digest=hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda:stream.read(1024*1024),b''):digest.update(block)
    return digest.hexdigest()


def checksum_entry(data,name=IMAGE):
    found=[]
    for line in data.decode('ascii').splitlines():
        match=re.fullmatch(r'([0-9a-fA-F]{64}) [ *](.+)',line)
        if match and match[2]==name:found.append(match[1].lower())
    require(len(found)==1,'signed manifest must contain exactly one exact image filename')
    return found[0]


def dearmor(data):
    text=data.decode('ascii').strip()
    begin='-----BEGIN PGP PUBLIC KEY BLOCK-----';end='-----END PGP PUBLIC KEY BLOCK-----'
    require(text.startswith(begin) and text.endswith(end),'expected armored public key')
    lines=text[len(begin):-len(end)].strip('\r\n').splitlines();payload=[];started=False
    for line in lines:
        line=line.strip()
        if not line or (not started and ':' in line):continue
        if line.startswith('='):continue
        started=True;payload.append(line)
    value=base64.b64decode(''.join(payload),validate=True)
    require(0<len(value)<=1024*1024,'public key size out of bounds')
    return value


def signed_by_ubuntu(data):
    lines=data.decode('utf-8','replace').splitlines()
    bad={'BADSIG','ERRSIG','REVKEYSIG','EXPKEYSIG','EXPSIG','KEYREVOKED','KEYEXPIRED','SIGEXPIRED'}
    valid=False
    for line in lines:
        if not line.startswith('[GNUPG:] '):continue
        fields=line.split()[1:]
        if fields and fields[0] in bad:return False
        if len(fields)>=10 and fields[0]=='VALIDSIG':
            # A signing subkey is acceptable only when GnuPG supplies the pinned primary fingerprint.
            primary=fields[10] if len(fields)>=11 else fields[1]
            if primary==FINGERPRINT:valid=True
    return valid


def same_registration(registration,disk):
    value=registration.get('base_path','')
    if value.startswith('\\\\?\\'):value=value[4:]
    require(registration.get('version')==2 and value,'expected WSL2 registration')
    actual=local.local_path(Path(value))
    require(actual.is_dir() and disk.is_dir() and os.path.samefile(actual,disk),'distro backing disk not at owned repository location')
    return str(actual)


class SafeRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self,req,fp,code,msg,headers,newurl):
        target=urllib.parse.urlsplit(newurl)
        require(target.scheme=='https' and target.hostname in ALLOWED_HOSTS and not target.username and not target.password,
                'download redirect outside approved HTTPS hosts')
        return super().redirect_request(req,fp,code,msg,headers,newurl)


def response_extent(status,headers,offset,limit):
    require(headers.get('Content-Encoding','identity').lower()=='identity','encoded download refused')
    length=headers.get('Content-Length')
    if offset:
        require(status==206,'server did not honor Range; original partial preserved')
        match=re.fullmatch(r'bytes (\d+)-(\d+)/(\d+)',headers.get('Content-Range',''))
        require(match is not None,'exact Content-Range required')
        start,end,total=map(int,match.groups())
        require(start==offset and offset<=end<total<=limit,'invalid range offset/end/total')
        if length is not None:require(int(length)==end-start+1,'range length mismatch')
        return end+1,total
    require(status==200 and not headers.get('Content-Range'),'unexpected initial response')
    if length is not None:
        require(0<int(length)<=limit,'download Content-Length exceeds bound')
        return int(length),int(length)
    return None,None


def download(url,path,limit,expected_sha256=None):
    path=local.local_path(path)
    if expected_sha256 is not None:
        require(re.fullmatch('[0-9a-f]{64}',expected_sha256) is not None,'expected SHA256 required')
    if path.exists():
        require(path.is_file() and 0<path.stat().st_size<=limit,'cached download invalid')
        if expected_sha256:require(file_hash(path)==expected_sha256,'cached image hash differs from signed checksum')
        print('Using retained download: '+path.name,flush=True);return
    require(urllib.parse.urlsplit(url).hostname in ALLOWED_HOSTS and url.startswith('https://'),'unapproved download URL')
    # Only the signed-checksum-bound image may resume. Small key/manifest files do not.
    candidates=[]
    if expected_sha256:
        for item in path.parent.glob(path.name+'.partial-*'):
            if not re.fullmatch(re.escape(path.name)+r'\.partial-[0-9a-f]{32}',item.name):continue
            item=local.local_path(item)
            require(item.is_file() and item.stat().st_size<=limit,'invalid partial file')
            if item.stat().st_size:candidates.append(item)
        require(len(candidates)<=32,'too many partials; inspect retained downloads')
        candidates.sort(key=lambda item:(item.stat().st_size,item.name),reverse=True)
        for item in candidates:
            if file_hash(item)==expected_sha256:
                # A timeout after the last body bytes must not force another transfer.
                print('Retained partial already matches signed image checksum',flush=True)
                recovered=path.with_name(path.name+'.partial-'+uuid.uuid4().hex)
                with item.open('rb') as source,recovered.open('xb') as target:
                    shutil.copyfileobj(source,target,1024*1024);target.flush();os.fsync(target.fileno())
                require(file_hash(recovered)==expected_sha256,'copied image changed during recovery')
                recovered.rename(path)
                return
    seed=candidates[0] if candidates else None
    offset=seed.stat().st_size if seed else 0
    partial=path.with_name(path.name+'.partial-'+uuid.uuid4().hex)
    headers={'User-Agent':'Worldwright-acceptance-provision/2','Accept-Encoding':'identity'}
    if offset:headers['Range']='bytes='+str(offset)+'-'
    deadline=time.monotonic()+1800;total=offset;next_progress=(offset//(64*1024*1024)+1)*64*1024*1024
    opener=urllib.request.build_opener(SafeRedirect())
    print(('Resuming at byte '+str(offset)+' from '+seed.name if seed else 'Downloading '+url),flush=True)
    with opener.open(urllib.request.Request(url,headers=headers),timeout=120) as response:
        end,total_size=response_extent(response.status,response.headers,offset,limit)
        # Validate Range BEFORE creating a continuation, and never append to the old evidence.
        with partial.open('xb') as stream:
            if seed:
                with seed.open('rb') as source:shutil.copyfileobj(source,stream,1024*1024)
                require(stream.tell()==offset,'partial changed while preparing resume')
            while end is None or total<end:
                require(time.monotonic()<deadline,'download time limit reached; partial preserved')
                count=min(1024*1024,end-total) if end is not None else 1024*1024
                block=response.read(count)
                if not block:break
                total+=len(block);require(total<=limit and (end is None or total<=end),'download exceeds bound')
                stream.write(block)
                if total>=next_progress:
                    print(str(total//(1024*1024))+' MiB retained/downloaded',flush=True);next_progress+=64*1024*1024
            if total_size is not None:require(total==total_size,'short/incomplete download; continuation preserved')
            require(total>0,'empty download');stream.flush();os.fsync(stream.fileno())
    if expected_sha256:require(file_hash(partial)==expected_sha256,'completed image differs from signed checksum; no import')
    partial.rename(path)


def unpack_gzip(source,target):
    require(not target.exists(),'new uncompressed import file required')
    part=target.with_name(target.name+'.partial-'+uuid.uuid4().hex);total=0
    with gzip.open(source,'rb') as incoming,part.open('xb') as outgoing:
        while True:
            block=incoming.read(1024*1024)
            if not block:break
            total+=len(block);require(total<=16*1024**3,'rootfs uncompressed bound exceeded')
            outgoing.write(block)
        outgoing.flush();os.fsync(outgoing.fileno())
    require(total>0,'empty rootfs');part.rename(target)


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--confirm-create',action='store_true')
    args=parser.parse_args()
    if os.name!='nt' or not args.confirm_create:parser.error('Windows and --confirm-create required')
    local.local_path(Path(sys.prefix));local.configure_tools_environment()
    base=local.local_path(local.LOCAL/'wsl');base.mkdir(exist_ok=True)
    downloads=local.local_path(base/'downloads');downloads.mkdir(exist_ok=True)
    disk=local.local_path(base/DISTRO);state_path=local.local_path(base/'provision-state.json')
    output=local.new_output(local.LOCAL/'evidence'/('wsl-provision-'+uuid.uuid4().hex));output.mkdir()
    print('WSL PROVISION EVIDENCE: '+str(output),flush=True)
    report={'schema_version':1,'kind':'isolated-acceptance-distro-provision','status':'failed','phases':[],
            'distro':DISTRO,'disk':str(disk),'acceptance_passed':False,'migrations_modified':False,'database_cluster_created':False}
    wsl=str(Path(os.environ['SystemRoot'])/'System32'/'wsl.exe');state=None
    lock=base/'provision.lock';acquired=False
    def save_state():
        temp=local.local_path(base/('state-'+uuid.uuid4().hex+'.json'));probe.new_json(temp,state);os.replace(temp,state_path)
    def phase(name,command,timeout=120,mutating=False):
        print('Provision phase: '+name,flush=True)
        if mutating:
            state['pending_phase']=name;save_state()
        result=r3_io.record_phase(report,output,name,command,timeout,cwd=local.ROOT,sensitive=r3_io.secret_values())
        require(result['returncode']==0 and result['interruption'] is None,'phase failed: '+name+'; preserve evidence and inspect before resuming')
        if mutating:
            state['pending_phase']=None;save_state()
        return result['stdout']
    def distro_names(label):
        return [x.strip() for x in probe.decode_wsl(phase(label,[wsl,'--list','--quiet'])).splitlines() if x.strip()]
    def guest(command,user='root'):
        home='/root' if user=='root' else '/home/worldwright'
        return [wsl,'--distribution',DISTRO,'--user',user,'--cd','/','--exec','/usr/bin/env','-i',
                'PATH=/usr/sbin:/usr/bin:/sbin:/bin','HOME='+home,'TMPDIR='+home,'TMP='+home,'TEMP='+home,'LANG=C.UTF-8',
                'PYTHONDONTWRITEBYTECODE=1','PYTHONNOUSERSITE=1','DEBIAN_FRONTEND=noninteractive',*command]
    try:
        with lock.open('x') as stream:stream.write(str(os.getpid()))
        acquired=True
        tests=phase('unit-tests',[sys.executable,'-B',str(local.ROOT/'scripts/test-wsl-provision.py')]).decode('utf-8').replace('\r\n','\n')
        require(re.findall(r'^Ran (\d+) tests? in ',tests,re.M)==['14'] and '\nOK\n' in tests,'provision helper tests incomplete')
        names=distro_names('distros-before');before=probe.registrations();report['registrations_before']=before
        if state_path.exists():
            require(state_path.stat().st_size<=65536,'state size bound')
            state=json.loads(state_path.read_text())
            require(state.get('schema_version')==1 and state.get('distro')==DISTRO and state.get('disk')==str(disk)
                    and re.fullmatch('[0-9a-f]{32}',state.get('nonce','')),'owned provision state mismatch')
            require(not state.get('pending_phase'),'prior mutating phase incomplete; inspect it before resume (no automatic retry/reset)')
        else:
            require(DISTRO.casefold() not in {n.casefold() for n in names} and not disk.exists(),'existing distro/location refused; no overwrite or adoption')
            state={'schema_version':1,'distro':DISTRO,'disk':str(disk),'nonce':uuid.uuid4().hex,'pending_phase':None,'os_ready':False}
            save_state()
        if DISTRO in names:
            same_registration(before.get(DISTRO,{}),disk)
        else:
            require(not state.get('imported'),'owned distro disappeared; stop instead of reimporting')
            require(VERIFIER in names,'existing Ubuntu-24.04 verifier required; no installs in it')
            require(shutil.disk_usage(base).free>=40*1024**3,'at least 40 GiB free required for initial acquisition/import')
            # Do not assume an account name: run version/path checks as the distro's existing default noninteractive user.
            prefix=[wsl,'--distribution',VERIFIER,'--cd','/','--exec']
            mapped=probe.decode_wsl(phase('map-downloads',prefix+['/usr/bin/wslpath','-u',str(downloads)]))
            require(mapped.startswith('/') and '\n' not in mapped and '\r' not in mapped,'invalid mapped download path')
            env=['/usr/bin/env','-i','PATH=/usr/bin:/bin','HOME='+mapped,'TMPDIR='+mapped,'GNUPGHOME='+mapped+'/gpg-home','LANG=C.UTF-8']
            phase('verifier-version',prefix+env+['/usr/bin/gpgv','--version'])
            download(BASE+'SHA256SUMS',downloads/'SHA256SUMS',1024*1024)
            download(BASE+'SHA256SUMS.gpg',downloads/'SHA256SUMS.gpg',1024*1024)
            download(KEY_URL,downloads/'ubuntu-cloud-key.asc',1024*1024)
            download('https://keyserver.ubuntu.com/pks/lookup?op=get&search=0x'+SECONDARY_KEY,
                     downloads/'ubuntu-cloud-builder-key.asc',1024*1024)
            key=(dearmor((downloads/'ubuntu-cloud-key.asc').read_bytes())+
                 dearmor((downloads/'ubuntu-cloud-builder-key.asc').read_bytes()))
            keyring=local.local_path(downloads/'ubuntu-cloud-key.gpg')
            if keyring.exists():require(keyring.read_bytes()==key,'cached keyring differs')
            else:
                with keyring.open('xb') as stream:stream.write(key)
            homedir=local.local_path(downloads/'gpg-home');homedir.mkdir(exist_ok=True)
            valid=phase('verify-ubuntu-signature',prefix+env+['/usr/bin/gpgv','--homedir',mapped+'/gpg-home',
                '--keyring',mapped+'/ubuntu-cloud-key.gpg','--status-fd','1',mapped+'/SHA256SUMS.gpg',mapped+'/SHA256SUMS'])
            require(signed_by_ubuntu(valid),'valid signature from pinned Canonical primary key required')
            digest=checksum_entry((downloads/'SHA256SUMS').read_bytes())
            archive=local.local_path(downloads/IMAGE);download(BASE+IMAGE,archive,4*1024**3,expected_sha256=digest)
            require(file_hash(archive)==digest,'rootfs hash mismatch against signed manifest; no import')
            state['rootfs_sha256']=digest;state['signer']=FINGERPRINT;save_state()
            tar=local.local_path(downloads/'ubuntu-24.04-amd64.rootfs.tar')
            if tar.exists():
                require(state.get('tar_sha256') and file_hash(tar)==state['tar_sha256'],'retained tar has no matching hash; manual inspection required')
            else:
                print('Decompressing verified rootfs inside repository',flush=True);unpack_gzip(archive,tar)
                state['tar_sha256']=file_hash(tar);save_state()
            require(DISTRO.casefold() not in {n.casefold() for n in distro_names('distros-before-import')},'distro name appeared before import')
            require(not disk.exists(),'new disk directory required');disk.mkdir()
            phase('import-new-distro',[wsl,'--import',DISTRO,str(disk),str(tar),'--version','2'],1800,mutating=True)
            same_registration(probe.registrations().get(DISTRO,{}),disk)
            state['imported']=True;save_state()
        report['backing_directory_verified']=same_registration(probe.registrations().get(DISTRO,{}),disk)
        if not state.get('os_ready'):
            result=phase('configure-new-distro',guest(['/usr/bin/python3','-I','-B','-c',BOOTSTRAP,state['nonce']]),180,mutating=True)
            report['guest_setup']=json.loads(result.decode('utf-8'))
            state['guest_setup']=report['guest_setup'];save_state()
            apt=['/usr/bin/apt-get','-o','Dpkg::Options::=--force-confold','-o','Acquire::Retries=0']
            phase('apt-update',guest(apt+['update','--error-on=any']),900,mutating=True)
            phase('apt-upgrade',guest(apt+['--yes','upgrade']),3600,mutating=True)
            phase('apt-build-pg-prerequisites',guest(apt+['--yes','--no-install-recommends','install',
                'build-essential','pkg-config','libssl-dev','ca-certificates','curl','xz-utils','git',
                'python3','python3-venv','postgresql-16','postgresql-client-16']),3600,mutating=True)
            # Store exact acquired package versions; do not represent them as a pre-pinned lock.
            phase('package-inventory',guest(['/usr/bin/dpkg-query','-W','-f=${Package}\t${Version}\n']),120)
            clusters=phase('verify-no-pg-clusters',guest(['/usr/bin/pg_lsclusters','--no-header']),60)
            require(not clusters.strip(),'unexpected PostgreSQL cluster exists; stop for inspection')
            state['os_ready']=True;save_state()
        else:
            phase('verify-owned-guest',guest(['/usr/bin/python3','-I','-B','-c',
                "from pathlib import Path; import sys; assert Path('/var/lib/worldwright-provision/ownership').read_text()==sys.argv[1]",state['nonce']]))
        result=phase('verify-ext4-private-workspace',guest(['/usr/bin/python3','-I','-B','-c',POSIX_CHECK,uuid.uuid4().hex],user='worldwright'),120)
        report['storage']=json.loads(result.decode('utf-8'))
        after=probe.registrations();report['registrations_after']=after
        require(all(after.get(name)==value for name,value in before.items() if name!=DISTRO),'existing distro registration changed during provisioning')
        state['storage_verified']=report['storage']['posix_passed'];save_state()
        report['rootfs_provenance']={key:state.get(key) for key in ('rootfs_sha256','tar_sha256','signer')}
        report['acquisition_files']=[{'name':p.name,'sha256':file_hash(p),'bytes':p.stat().st_size}
            for p in sorted(downloads.iterdir()) if p.is_file() and '.partial-' not in p.name]
        report['status']='isolated-distro-and-os-prerequisites-ready-project-tests-pending'
        report['pending']=['version-pinned Linux Rust/Node and Python/browser tooling','source snapshot and fresh migration repair',
                           'full Linux/PostgreSQL test execution','independent signed acceptance']
    except (Exception,KeyboardInterrupt) as error:
        report['failure_type']=type(error).__name__
        report['failure']=r3_io.log_bytes(str(error).encode(),r3_io.secret_values(),4096).decode('utf-8','replace')
        report['pending_phase']=state.get('pending_phase') if state else None
        print('Provisioning stopped: '+report['failure'],flush=True)
    finally:
        if acquired:lock.unlink()
        probe.new_json(output/'provision.json',report)
    print(report['status']+': '+str(output/'provision.json'),flush=True)
    return 0 if report['status']=='isolated-distro-and-os-prerequisites-ready-project-tests-pending' else 1


if __name__=='__main__':raise SystemExit(main())
