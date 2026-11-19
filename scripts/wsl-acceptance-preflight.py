"""Windows-driven WSL inventory and local permission probe, NOT acceptance.
No installs, distro configuration, DB connections, builds, signing or migration edits.
Use the existing repository-local Windows Python. Each run retains new evidence.
"""
from pathlib import Path
import argparse
import hashlib
import json
import os
import re
import sys
import uuid
import worldwright_local as local
import r3_io

LINUX_PROBE = r'''
import hashlib,json,os,pwd,re,stat,subprocess,sys
from pathlib import Path
root=Path(sys.argv[1]); out=Path(sys.argv[2])
if not root.is_absolute() or not out.is_absolute(): raise ValueError('absolute paths required')
for p in (root,out):
    for entry in (p,*p.parents):
        if entry.is_symlink(): raise ValueError('linked project/evidence path refused')
root=root.resolve(strict=True);out=out.resolve(strict=True)
if not out.is_relative_to(root/'.worldwright-local'/'evidence'): raise ValueError('local evidence required')
marker=out/'binding.json'
binding=json.loads(marker.read_text())
if binding['nonce']!=sys.argv[3]: raise ValueError('Windows/WSL evidence binding mismatch')
source=root/'scripts'/'wsl-acceptance-preflight.py'
if hashlib.sha256(source.read_bytes()).hexdigest()!=binding['script_sha256']: raise ValueError('different source visible to WSL')
os.umask(0o077)
os.chdir(root)
report={'schema_version':1,'kind':'wsl-preflight-observation','root':str(root),'output':str(out),'uid':os.getuid(),'gid':os.getgid(),
        'kernel':os.uname().release,'machine':os.uname().machine,'python':sys.version,'python_executable':sys.executable,
        'db_started':False,'acceptance_passed':False}
report['os_release']={}
for line in Path('/etc/os-release').read_text().splitlines():
    if '=' in line:
        key,value=line.split('=',1)
        if key in ('ID','VERSION_ID','PRETTY_NAME'): report['os_release'][key]=value.strip('"')
def unescape(value):
    return re.sub(r'\\([0-7]{3})',lambda m:chr(int(m.group(1),8)),value)
def mount_for(path):
    candidates=[]
    for line in Path('/proc/self/mountinfo').read_text().splitlines():
        before,after=line.split(' - ',1);fields=before.split();fs=after.split()
        mount=Path(unescape(fields[4]))
        if path==mount or path.is_relative_to(mount):
            candidates.append((len(mount.parts),{'mountpoint':str(mount),'filesystem':fs[0],
                'source':unescape(fs[1]),'mount_options':fields[5],
                'drvfs_detected':'drvfs' in line.lower()}))
    return max(candidates,key=lambda row:row[0])[1] if candidates else None
report['project_mount']=mount_for(root)
report['root_mount']=mount_for(Path('/'))
report['project_free_bytes']=os.statvfs(root).f_bavail*os.statvfs(root).f_frsize
report['tools']={}
paths=[Path('/usr/bin/git'),Path('/usr/bin/node'),Path('/usr/bin/cc'),Path('/usr/bin/pkg-config'),
       Path('/usr/bin/psql'),Path('/usr/bin/pg_config'),Path('/usr/bin/helm')]
paths+=sorted(Path('/usr/lib/postgresql').glob('*/bin/postgres')) if Path('/usr/lib/postgresql').is_dir() else []
paths+=sorted(Path('/usr/lib/postgresql').glob('*/bin/initdb')) if Path('/usr/lib/postgresql').is_dir() else []
paths+=sorted(Path('/usr/lib/postgresql').glob('*/bin/pg_ctl')) if Path('/usr/lib/postgresql').is_dir() else []
user_home=Path(pwd.getpwuid(os.getuid()).pw_dir)
for base in (user_home/'.rustup',root/'.worldwright-local'/'rustup-home'):
    chain=base/'toolchains'/'1.85.1-x86_64-unknown-linux-gnu'/'bin'
    for name in ('rustc','cargo','rustfmt','cargo-clippy'):
        paths.append(chain/name)
for path in paths:
    item={'exists':path.is_file(),'executable':os.access(path,os.X_OK),'resolved':str(path.resolve())}
    # Only known version-only commands; never execute a rustup shim or download a tool.
    if item['exists'] and item['executable'] and path.name not in ('cargo-clippy',):
        try:
            argv=[str(path),'version','--short'] if path.name=='helm' else [str(path),'--version']
            result=subprocess.run(argv,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,timeout=10,check=False)
            item.update(exit_code=result.returncode,version=result.stdout.decode('utf-8','replace')[:2048])
        except (OSError,subprocess.TimeoutExpired) as error:
            item['error']=type(error).__name__
    report['tools'][str(path)]=item
report['posix_probe']={'status':'not-run'}
if os.getuid()!=0:
    try:
        probe=out/'posix-probe';probe.mkdir(mode=0o700)
        file=probe/'mode-check';fd=os.open(file,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
        with os.fdopen(fd,'wb') as stream:stream.write(b'nonsecret filesystem probe\n')
        before=stat.S_IMODE(file.stat().st_mode)
        file.chmod(0o400);readonly=stat.S_IMODE(file.stat().st_mode)
        write_denied=False
        try:
            fd=os.open(file,os.O_WRONLY);os.close(fd)
        except PermissionError:write_denied=True
        file.chmod(0o600)
        link=probe/'symlink-check';link.symlink_to(file.name)
        hard=probe/'hardlink-check';os.link(file,hard)
        report['posix_probe']={'status':'observed','directory_mode':oct(stat.S_IMODE(probe.stat().st_mode)),
            'initial_file_mode':oct(before),'readonly_file_mode':oct(readonly),'write_denied':write_denied,
            'symlink':link.is_symlink(),'hardlink_samefile':file.samefile(hard),
            'owned_by_uid':file.stat().st_uid==os.getuid()}
        report['posix_probe']['passed']=(stat.S_IMODE(probe.stat().st_mode)==0o700 and before==0o600 and readonly==0o400
            and write_denied and link.is_symlink() and file.samefile(hard) and file.stat().st_uid==os.getuid())
    except OSError as error:
        report['posix_probe']={'status':'failed','error':type(error).__name__,'errno':error.errno,'passed':False}
else:
    report['posix_probe']={'status':'blocked-root-user','passed':False}
# Leave only our new probe files for review. Do not chmod/delete existing paths.
print(json.dumps(report,sort_keys=True))
'''


def decode_wsl(data):
    if data.startswith((b'\xff\xfe', b'\xfe\xff')):
        return data.decode('utf-16').strip()
    if b'\x00' in data:
        return data.decode('utf-16-le').strip()
    return data.decode('utf-8-sig').strip()


def choose_distro(names, requested=None):
    if requested:
        if requested not in names:
            raise ValueError('requested distro is not in the actual WSL inventory')
        return requested
    candidates=[name for name in names if 'ubuntu' in name.lower()]
    if len(candidates)!=1:
        raise ValueError('choose one installed distro with --distro; see wsl-distributions.log')
    return candidates[0]


def classify(observation, registration):
    blockers=[]
    if registration.get('version')!=2: blockers.append('registered-distro-is-not-verified-WSL2')
    release=observation.get('os_release',{})
    if release.get('ID')!='ubuntu' or release.get('VERSION_ID')!='24.04': blockers.append('Ubuntu-24.04-required')
    if type(observation.get('uid')) is not int or observation['uid']<=0: blockers.append('non-root-Linux-user-required')
    if observation.get('machine')!='x86_64': blockers.append('review-non-x86_64-toolchain-target')
    if not observation.get('posix_probe',{}).get('passed'): blockers.append('POSIX-permission-link-probe-failed')
    if not observation.get('project_mount'): blockers.append('project-mount-not-identified')
    elif observation['project_mount'].get('drvfs_detected'): blockers.append('DrvFS-requires-review-before-native-filesystem-acceptance')
    return blockers


def registrations():
    import winreg
    result={}
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER,r'Software\Microsoft\Windows\CurrentVersion\Lxss') as key:
            for index in range(winreg.QueryInfoKey(key)[0]):
                with winreg.OpenKey(key,winreg.EnumKey(key,index)) as entry:
                    name=winreg.QueryValueEx(entry,'DistributionName')[0]
                    value={}
                    for field,dest in (('BasePath','base_path'),('Version','version'),('DefaultUid','default_uid')):
                        try:value[dest]=winreg.QueryValueEx(entry,field)[0]
                        except FileNotFoundError:pass
                    result[name]=value
    except FileNotFoundError:pass
    return result


def new_json(path,value):
    with path.open('x',encoding='utf-8',newline='\n') as stream:
        json.dump(value,stream,sort_keys=True,indent=2);stream.write('\n')


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--distro')
    args=parser.parse_args()
    if os.name!='nt': parser.error('run from repository-local Windows Python; Linux probe is internal')
    local.local_path(Path(sys.prefix))
    local.configure_tools_environment()
    output=local.new_output(local.LOCAL/'evidence'/('wsl-preflight-'+uuid.uuid4().hex));output.mkdir()
    print('WSL PREFLIGHT EVIDENCE: '+str(output),flush=True)
    report={'schema_version':1,'kind':'wsl-acceptance-preflight','status':'failed','phases':[],
        'acceptance_passed':False,'database_modified':False,'source_migrations_modified':False}
    wsl=Path(os.environ['SystemRoot'])/'System32'/'wsl.exe'
    def phase(name,command,timeout=60):
        result=r3_io.record_phase(report,output,name,command,timeout,cwd=local.ROOT,sensitive=r3_io.secret_values())
        if result['returncode']!=0 or result['interruption'] is not None:
            raise ValueError('preflight phase failed: '+name)
        return result['stdout']
    try:
        tests=phase('preflight-unit-tests',[sys.executable,'-B',str(local.ROOT/'scripts'/'test-wsl-preflight.py')]).decode('utf-8').replace('\r\n','\n')
        if re.findall(r'^Ran (\d+) tests? in ',tests,re.M)!=['7'] or '\nOK\n' not in tests or 'skipped=' in tests:
            raise ValueError('preflight helper inventory incomplete')
        names=[line.strip() for line in decode_wsl(phase('wsl-distributions',[str(wsl),'--list','--quiet'])).splitlines() if line.strip()]
        if not names:raise ValueError('no installed WSL distributions')
        report['distributions']=names
        name=choose_distro(names,args.distro);report['selected_distro']=name
        report['registration']=registrations().get(name,{})
        prefix=[str(wsl),'--distribution',name,'--cd','/','--exec']
        linux_root=decode_wsl(phase('wsl-project-path',prefix+['/usr/bin/wslpath','-u',str(local.ROOT)]))
        linux_out=decode_wsl(phase('wsl-evidence-path',prefix+['/usr/bin/wslpath','-u',str(output)]))
        for path in (linux_root,linux_out):
            if not path.startswith('/') or '\n' in path or '\r' in path:raise ValueError('invalid WSL path mapping')
        nonce=uuid.uuid4().hex
        new_json(output/'binding.json',{'nonce':nonce,'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()})
        home=linux_root+'/.worldwright-local/tool-home';temp=linux_root+'/.worldwright-local/temp'
        environment=['/usr/bin/env','-i','PATH=/usr/bin:/bin','LANG=C.UTF-8','HOME='+home,
            'TMPDIR='+temp,'TMP='+temp,'TEMP='+temp,'PYTHONDONTWRITEBYTECODE=1','PYTHONNOUSERSITE=1']
        observed=phase('wsl-linux-observation',prefix+environment+['/usr/bin/python3','-I','-B','-c',LINUX_PROBE,linux_root,linux_out,nonce],240)
        report['linux']=json.loads(observed.decode('utf-8'))
        report['blockers']=classify(report['linux'],report['registration'])
        report['status']='preflight-observed-blocked' if report['blockers'] else 'preflight-observed-review-required'
        report['storage_boundary']='Project writes only in new local evidence; WSL/system services are not OS-sandboxed by environment redirection.'
        for item in report['blockers']:print('BLOCKER: '+item,flush=True)
    except (Exception,KeyboardInterrupt) as error:
        report['failure_type']=type(error).__name__
        report['failure']=r3_io.log_bytes(str(error).encode(),r3_io.secret_values(),4096).decode('utf-8','replace')
        print('Preflight stopped: '+report['failure'],flush=True)
    finally:
        new_json(output/'preflight.json',report)
    print(report['status']+': '+str(output/'preflight.json'),flush=True)
    return 1 if report['status']=='failed' else 2 if report.get('blockers') else 0


if __name__=='__main__':raise SystemExit(main())
