"""Windows entry: fresh baseline, verified ext4 snapshot, Linux tools/build/PG checks.
Reuses the owned acceptance distro; never reprovisions it or self-signs acceptance.
"""
from pathlib import Path
import argparse
import hashlib
import importlib.util
import io
import json
import os
import re
import stat
import sys
import tarfile
import uuid
import fresh_baseline
import worldwright_local as local
import r3_io
from r6_support import source_identity,encode,new_file,safe_path

DISTRO='Worldwright-Acceptance-24.04'
GUEST='/home/worldwright/Worldwright'
TEXT_SUFFIXES={'.rs','.toml','.lock','.md','.txt','.json','.yaml','.yml','.sh','.ps1','.py','.cjs','.js','.css','.html'}

SNAPSHOT = r'''
import hashlib,json,os,shutil,stat,subprocess,sys,tarfile,uuid
from pathlib import Path
root=Path('/home/worldwright/Worldwright');transport=Path(sys.argv[1]);manifest_sha=sys.argv[2];archive_sha=sys.argv[3]
if os.geteuid()==0 or root.stat().st_dev!=Path('/').stat().st_dev:raise ValueError('owned non-root ext4 workspace required')
local=root/'.worldwright-local'
if (local/'linux-stage.lock').exists():raise ValueError('Linux stage may still be active; inspect before updating source')
def hashfile(path):
    result=hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda:stream.read(1024*1024),b''):result.update(block)
    return result.hexdigest()
def relative(name):
    p=Path(name)
    if not name or p.is_absolute() or any(v in ('..','.git','.worldwright-local') for v in p.parts) or '\\' in name or ':' in name:raise ValueError('invalid source path')
    return p
def no_links(path):
    for p in (path,*path.parents):
        if p.is_symlink():raise ValueError('linked source path')
    return path
manifest=transport/'snapshot.json';archive=transport/'source.tar'
if hashfile(manifest)!=manifest_sha or hashfile(archive)!=archive_sha:raise ValueError('source transport changed')
value=json.loads(manifest.read_text());entries=value['files']
if not 0<len(entries)<=10000:raise ValueError('snapshot inventory bound')
expected={entry['path']:entry for entry in entries}
if len(expected)!=len(entries):raise ValueError('duplicate snapshot source')
for name in expected:relative(name)
marker=local/'source-snapshot.json';previous=json.loads(marker.read_text()) if marker.exists() else None
if (root/'.git').exists() and previous is None:raise ValueError('unowned existing guest repository refused')
if previous:
    for entry in previous['files']:
        path=no_links(root/relative(entry['path']))
        if not path.is_file() or hashfile(path)!=entry['sha256']:raise ValueError('guest source edited since snapshot; preserve it before resync')
# Every acquisition/cache/temp path used by Git is inside the repository-backed guest root.
home=local/'tool-home';home.mkdir(exist_ok=True)
os.environ.update(HOME=str(home),TMPDIR=str(local/'temp'),GIT_CONFIG_NOSYSTEM='1',GIT_CONFIG_GLOBAL=str(home/'gitconfig'),GIT_TERMINAL_PROMPT='0')
(local/'temp').mkdir(exist_ok=True)
def git(*args):subprocess.run(['/usr/bin/git',*args],cwd=root,check=True,stdin=subprocess.DEVNULL,stdout=sys.stderr)
if previous:
    result=subprocess.run(['/usr/bin/git','ls-files','--cached','--others','--exclude-standard','-z'],cwd=root,check=True,stdout=subprocess.PIPE)
    actual={os.fsdecode(p) for p in result.stdout.split(b'\0') if p}
    if actual!={entry['path'] for entry in previous['files']}:raise ValueError('unrecorded guest source; refuse overwrite')
else:
    if any(p.name!='.worldwright-local' for p in root.iterdir()):raise ValueError('guest workspace not pristine')
    git('init','--initial-branch=acceptance')
    git('config','core.autocrlf','false')
    git('config','core.fileMode','true')
bundle=transport/'source.bundle'
if hashfile(bundle)!=value['bundle_sha256']:raise ValueError('Git bundle changed')
git('fetch','--no-tags',str(bundle),'HEAD')
resolved=subprocess.run(['/usr/bin/git','rev-parse','FETCH_HEAD'],cwd=root,check=True,stdout=subprocess.PIPE).stdout.decode().strip()
if resolved!=value['commit']:raise ValueError('bundle HEAD differs')
staging=local/('source-staging-'+uuid.uuid4().hex);staging.mkdir(mode=0o700)
with tarfile.open(archive,'r:') as tar:
    members=tar.getmembers()
    if len(members)!=len(expected) or {m.name for m in members}!=set(expected):raise ValueError('tar inventory differs')
    for member in members:
        if not member.isfile() or member.size!=expected[member.name]['bytes']:raise ValueError('source must be exact bounded regular file')
        destination=staging/relative(member.name);destination.parent.mkdir(parents=True,exist_ok=True)
        source=tar.extractfile(member)
        with destination.open('xb') as target:shutil.copyfileobj(source,target)
        if hashfile(destination)!=expected[member.name]['sha256']:raise ValueError('source bytes differ')
        destination.chmod(expected[member.name]['mode'])
backup=local/('source-before-'+uuid.uuid4().hex)
if previous:
    backup.mkdir(mode=0o700)
    for entry in previous['files']:
        path=no_links(root/relative(entry['path']));saved=backup/entry['path'];saved.parent.mkdir(parents=True,exist_ok=True)
        path.rename(saved)
for name in expected:
    dest=no_links(root/relative(name));dest.parent.mkdir(parents=True,exist_ok=True)
    if dest.exists():raise ValueError('unowned source collision')
    (staging/name).rename(dest)
git('update-ref','refs/heads/acceptance',value['commit']);git('symbolic-ref','HEAD','refs/heads/acceptance');git('read-tree',value['commit'])
for name,entry in expected.items():
    if hashfile(root/name)!=entry['sha256']:raise ValueError('installed snapshot differs')
new=marker.with_name(marker.name+'.'+uuid.uuid4().hex)
with new.open('x') as stream:json.dump(value,stream,indent=2,sort_keys=True)
os.replace(new,marker)
print(json.dumps({'workspace':str(root),'commit':value['commit'],'file_count':len(expected),'source_sha256':value['source_sha256'],'copied_source_verified':True}))
'''


EXPORT = r'''
import hashlib,json,re,sys
from pathlib import Path
base=Path('/home/worldwright/Worldwright/.worldwright-local/evidence');target=Path(sys.argv[1])/'guest-evidence'
if not target.is_dir():raise ValueError('existing Windows evidence export directory required')
records=[]
for name in json.loads(sys.argv[2]):
    if not re.fullmatch(r'linux-(tools|checks)-[0-9a-f]{32}',name):raise ValueError('invalid guest evidence name')
    directory=base/name;destination=target/name;destination.mkdir()
    if directory.is_symlink():raise ValueError('linked guest evidence refused')
    for source in sorted(directory.iterdir()):
        if source.name=='postgres.log' or (source.suffix!='.log' and source.name!='stage.json'):continue
        if source.is_symlink() or not source.is_file() or source.stat().st_size>64*1024*1024:raise ValueError('bounded regular evidence required')
        data=source.read_bytes()
        with (destination/source.name).open('xb') as stream:stream.write(data)
        records.append({'path':name+'/'+source.name,'sha256':hashlib.sha256(data).hexdigest()})
print(json.dumps({'exported':records,'guest_worker_lock_present':(base.parent/'linux-stage.lock').exists(),
                  'database_files_password_files_and_raw_pg_log_exported':False}))
'''


def need(value,message):
    if not value:raise ValueError(message)


def load(name):
    spec=importlib.util.spec_from_file_location(name.replace('-','_'),local.ROOT/'scripts'/(name+'.py'))
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module


def projected(name,data):
    path=Path(name)
    return data.replace(b'\r\n',b'\n') if path.suffix.lower() in TEXT_SUFFIXES or path.name in ('.gitignore','.gitattributes') else data


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fresh-databases-only',action='store_true')
    parser.add_argument('--reuse-tools',action='store_true',help='verify existing pinned tools/browser offline; no install, fetch or apt')
    args=parser.parse_args()
    if os.name!='nt' or not args.fresh_databases_only:parser.error('Windows and --fresh-databases-only required')
    local.local_path(Path(sys.prefix));local.configure_tools_environment()
    out=local.new_output(local.LOCAL/'evidence'/('linux-next-'+uuid.uuid4().hex));out.mkdir()
    print('NEXT STAGE EVIDENCE: '+str(out),flush=True)
    report={'kind':'linux-next-stage','schema_version':1,'phases':[],'status':'failed','acceptance_passed':False}
    wsl=str(Path(os.environ['SystemRoot'])/'System32/wsl.exe');provision=load('provision-acceptance-wsl')
    lock=local.local_path(local.LOCAL/'wsl/linux-next.lock');acquired=False;guest_evidence=set();mapped=None
    def phase(name,argv,timeout=120):
        print('Next-stage phase: '+name,flush=True)
        result=r3_io.record_phase(report,out,name,[str(x) for x in argv],timeout,cwd=local.ROOT,sensitive=r3_io.secret_values())
        for found in re.findall(rb'^LINUX STAGE EVIDENCE: /home/worldwright/Worldwright/\.worldwright-local/evidence/(linux-(?:tools|checks)-[0-9a-f]{32})\r?$',result['stdout'],re.M):
            guest_evidence.add(found.decode('ascii'))
        need(result['returncode']==0 and result['interruption'] is None,'phase failed: '+name+'; inspect retained logs before retry')
        return result['stdout']
    def guest(command,user='worldwright'):
        return [wsl,'--distribution',DISTRO,'--user',user,'--cd',GUEST,'--exec','/usr/bin/env','-i',
                'PATH=/usr/sbin:/usr/bin:/sbin:/bin','HOME='+GUEST+'/.worldwright-local/tool-home',
                'TMPDIR='+GUEST+'/.worldwright-local/temp','LANG=C.UTF-8','PYTHONDONTWRITEBYTECODE=1',*command]
    try:
        with lock.open('x') as stream:stream.write(str(os.getpid()))
        acquired=True
        raw=phase('stage-unit-tests',[sys.executable,'-B',local.ROOT/'scripts/test-linux-stage.py'])
        need(b'Ran 6 tests in ' in raw and b'OK' in raw,'stage helper inventory incomplete')
        state=json.loads(local.local_path(local.LOCAL/'wsl/provision-state.json').read_text())
        need(state.get('imported') and state.get('os_ready') and state.get('storage_verified') and not state.get('pending_phase'),'isolated distro is not ready')
        provision.same_registration(provision.probe.registrations().get(DISTRO,{}),local.LOCAL/'wsl'/DISTRO)
        phase('guest-ownership',guest(['/usr/bin/python3','-I','-B','-c',
            "from pathlib import Path;import sys;assert Path('/var/lib/worldwright-provision/ownership').read_text()==sys.argv[1]",state['nonce']],user='root'))
        report['baseline']=fresh_baseline.apply_fresh(local.ROOT,out,authorized=True)
        new_file(out/'fresh-baseline.json',encode(report['baseline']))
        before=source_identity();report['source_before']=before
        phase('source-git-bundle',['git','bundle','create',out/'source.bundle','HEAD'],600)
        modes_raw=phase('source-mode-inventory',['git','ls-files','--stage','-z'])
        modes={}
        for item in modes_raw.split(b'\0'):
            if item:
                metadata,name=item.split(b'\t',1);modes[os.fsdecode(name)]=metadata.split()[0]
        records=[];total=0
        with tarfile.open(out/'source.tar','w') as tar:
            for entry in before['inventory']:
                name=entry['path'];need(not any(p in ('.git','.worldwright-local') for p in Path(name).parts),'runtime data in source inventory')
                path=safe_path(local.ROOT,name);raw=path.read_bytes();data=projected(name,raw)
                need(hashlib.sha256(data).hexdigest()==entry['sha256'],'source changed during capture')
                total+=len(data);need(total<=1024**3,'source archive size bound')
                mode=0o755 if modes.get(name)==b'100755' or path.suffix=='.sh' else 0o644
                info=tarfile.TarInfo(name);info.size=len(data);info.mode=mode;info.mtime=0
                tar.addfile(info,io.BytesIO(data))
                records.append({'path':name,'sha256':entry['sha256'],'windows_raw_sha256':hashlib.sha256(raw).hexdigest(),'bytes':len(data),'mode':mode})
        need(source_identity()==before,'Windows source changed during archive creation')
        manifest={'schema_version':1,'commit':before['commit'],'source_sha256':before['source_sha256'],'files':records,
                  'bundle_sha256':provision.file_hash(out/'source.bundle'),'normalization':'r6-source-inventory-v1; SQL/log bytes unchanged'}
        new_file(out/'snapshot.json',encode(manifest))
        mapped=provision.probe.decode_wsl(phase('map-transport',guest(['/usr/bin/wslpath','-u',str(out)])))
        need(mapped.startswith('/') and '\n' not in mapped,'invalid WSL transport mapping')
        phase('install-verified-source',guest(['/usr/bin/python3','-I','-B','-c',SNAPSHOT,mapped,
                            provision.file_hash(out/'snapshot.json'),provision.file_hash(out/'source.tar')]),900)
        worker=GUEST+'/scripts/linux-stage-worker.py';python=GUEST+'/.worldwright-local/python/Worldwright-Linux-Security/bin/python'
        report['reuse_tools']=args.reuse_tools
        if args.reuse_tools:
            phase('verify-existing-linux-tools',guest(['/usr/bin/python3','-B',worker,'verify-tools']),600)
        else:
            phase('linux-tools',guest(['/usr/bin/python3','-B',worker,'tools']),3600)
            # Apt modifies only the already approved repository-backed new distro.
            # Python's browser package defines the real Ubuntu dependencies.
            phase('chromium-system-libraries',guest(['/usr/bin/env','NODE_EXTRA_CA_CERTS=/etc/worldwright-enterprise/chain.pem',python,'-B','-m','playwright','install-deps','chromium'],user='root'),1800)
        phase('linux-build-and-pg-tests',guest([python,'-B',worker,'checks']),3600)
        report['source_after']=source_identity();need(report['source_after']==before,'Windows source changed during Linux execution')
        report['status']='next-stage-passed-independent-acceptance-incomplete'
    except (Exception,KeyboardInterrupt) as error:
        report['failure']=r3_io.log_bytes(str(error).encode(),r3_io.secret_values(),4000).decode('utf-8','replace')
        print('Next stage stopped: '+report['failure'],flush=True)
    finally:
        if guest_evidence and mapped:
            try:
                (out/'guest-evidence').mkdir()
                raw=phase('export-guest-evidence',guest(['/usr/bin/python3','-I','-B','-c',EXPORT,mapped,json.dumps(sorted(guest_evidence))]),180)
                report['guest_evidence_export']=json.loads(raw.decode('utf-8'))
            except Exception as error:
                report['guest_evidence_export_error']=str(error);report['status']='failed'
        if acquired:lock.unlink()
        new_file(out/'next-stage.json',encode(report))
    print(report['status']+': '+str(out/'next-stage.json'),flush=True)
    return 0 if report['status']=='next-stage-passed-independent-acceptance-incomplete' else 1


if __name__=='__main__':raise SystemExit(main())
