"""Owned ext4 Linux tooling/build/PG stage, driven by linux-next-stage.py.
Not independent acceptance. No default PostgreSQL service or deployment DB is used.
"""
from pathlib import Path
import argparse
import hashlib
import importlib.util
import json
import os
import re
import secrets
import shutil
import socket
import sys
import tarfile
import time
import uuid
import worldwright_local as local
import native_paths
import r3_io
import enterprise_ca
from fresh_baseline import baseline_bytes,inventory
from r6_support import encode,new_file,source_identity,test_listing,test_result

NODE='24.22.3'
NODE_HASH='4f2b175a6d53a5c617a80dba2676238a0688cef28c6da4eba35c7d844e66fcfb'
HOST='x86_64-unknown-linux-gnu'
RUST='1.85.1'
PYTHON=local.LOCAL/'python/Worldwright-Linux-Security/bin/python'
NODE_ROOT=local.LOCAL/'tools'/('node-v'+NODE+'-linux-x64')
CHAIN=local.LOCAL/'rustup-home/toolchains'/('1.85.1-'+HOST)


NODE_TLS_CHECK = r'''
const tls = require('node:tls');
const hosts = ['static.rust-lang.org','nodejs.org','pypi.org','files.pythonhosted.org',
               'index.crates.io','static.crates.io','cdn.playwright.dev'];
(async () => {
  const results = [];
  for (const host of hosts) {
    results.push(await new Promise(resolve => {
      let finished = false;
      const socket = tls.connect({host, port:443, servername:host, rejectUnauthorized:true});
      const finish = result => { if (!finished) { finished=true; socket.destroy(); resolve(result); } };
      socket.setTimeout(10000, () => finish({host, verified:false, error:'timeout'}));
      socket.once('error', error => finish({host, verified:false, error:error.code || error.message}));
      socket.once('secureConnect', () => finish({host, verified:socket.authorized,
                      protocol:socket.getProtocol(), fingerprint256:socket.getPeerCertificate().fingerprint256}));
    }));
  }
  console.log(JSON.stringify({hosts:results, all_verified:results.every(item => item.verified)}));
  process.exitCode = results.every(item => item.verified) ? 0 : 1;
})().catch(error => { console.error(error); process.exitCode=1; });
'''


def need(value,message):
    if not value:raise ValueError(message)


def load(name):
    spec=importlib.util.spec_from_file_location(name.replace('-','_'),local.ROOT/'scripts'/(name+'.py'))
    value=importlib.util.module_from_spec(spec);spec.loader.exec_module(value);return value


def hash_file(path):return native_paths.hash_file(path)


def extract_verified(archive,parent,top):
    parent=local.local_path(parent);need(not parent.exists(),'new extraction directory required');parent.mkdir()
    with tarfile.open(archive,'r:xz') as bundle:
        members=bundle.getmembers();need(0<len(members)<=100000,'tool archive entry bound')
        names=set();total=0
        for member in members:
            parts=Path(member.name).parts
            need(parts and parts[0]==top and not Path(member.name).is_absolute() and '..' not in parts and '\\' not in member.name,'tool archive path')
            need(member.name not in names,'duplicate tool archive member');names.add(member.name)
            total+=member.size;need(total<=12*1024**3,'tool archive size bound')
        bundle.extractall(parent,filter='data')
    return parent/top


class Run:
    def __init__(self,action):
        self.output=local.new_output(local.LOCAL/'evidence'/('linux-'+action+'-'+uuid.uuid4().hex));self.output.mkdir()
        self.value={'schema_version':1,'action':action,'status':'failed','phases':[],'acceptance_passed':False,'started':int(time.time())}
        self.secret=r3_io.secret_values()
        print('LINUX STAGE EVIDENCE: '+str(self.output),flush=True)
    def phase(self,name,argv,timeout=120,cwd=None,required=True):
        print('Linux phase: '+name,flush=True)
        result=r3_io.record_phase(self.value,self.output,name,[str(x) for x in argv],timeout,cwd=cwd or local.ROOT,sensitive=self.secret)
        ok=result['returncode']==0 and result['interruption'] is None
        if required:need(ok,'phase failed: '+name+'; see retained log')
        return result,ok
    def finish(self):
        self.value['finished']=int(time.time());new_file(self.output/'stage.json',encode(self.value))
        print(self.value['status']+': '+str(self.output/'stage.json'),flush=True)


def configure():
    need(sys.platform=='linux' and os.geteuid()!=0,'non-root Linux execution required')
    need(local.ROOT==Path('/home/worldwright/Worldwright'),'owned guest checkout required')
    need(local.ROOT.stat().st_dev==Path('/').stat().st_dev,'checkout not on guest root disk')
    local.configure_tools_environment()
    os.environ['PATH']=str(CHAIN/'bin')+':'+str(NODE_ROOT/'bin')+':/usr/lib/postgresql/16/bin:/usr/bin:/bin'
    for relative in ('downloads','tools','python','pg-sockets'):
        local.local_path(local.LOCAL/relative).mkdir(exist_ok=True)


def tools(run):
    net=load('provision-acceptance-wsl')
    # Tool acquisition is separate from OS-image trust: TLS-published Rust checksum
    # and Node's reviewed release-page digest, not a claim of detached-signature validation.
    net.ALLOWED_HOSTS.update({'static.rust-lang.org','nodejs.org'})
    downloads=local.LOCAL/'downloads'
    if not (CHAIN/'.stage-install-complete.json').exists():
        need(not CHAIN.exists(),'partial Rust installation requires inspection; no automatic overwrite')
        name='rust-'+RUST+'-'+HOST+'.tar.xz';base='https://static.rust-lang.org/dist/'
        sums=downloads/(name+'.sha256');net.download(base+name+'.sha256',sums,65536)
        text=sums.read_text().strip();match=re.fullmatch(r'([0-9a-f]{64})\s+\*?(?:\./)?'+re.escape(name),text)
        need(match is not None,'unexpected official Rust checksum record')
        archive=downloads/name;net.download(base+name,archive,4*1024**3,expected_sha256=match[1])
        unpack=local.LOCAL/'tools'/('rust-unpack-'+uuid.uuid4().hex)
        package=extract_verified(archive,unpack,'rust-'+RUST+'-'+HOST)
        required=['rustc','cargo','rust-std-'+HOST,'rustfmt-preview','clippy-preview']
        actual=(package/'components').read_text().splitlines()
        need(set(required)<=set(actual),'Rust package does not contain all required components')
        CHAIN.parent.mkdir(parents=True,exist_ok=True)
        run.phase('install-pinned-rust',['/bin/sh',package/'install.sh','--prefix='+str(CHAIN),'--components='+','.join(required),'--disable-ldconfig'],900,cwd=package)
        identities={name:hash_file(CHAIN/'bin'/name) for name in native_paths.TOOLS}
        new_file(CHAIN/'.stage-install-complete.json',encode({'archive_sha256':match[1],'binaries':identities}))
    marker=json.loads((CHAIN/'.stage-install-complete.json').read_text())
    need(all(hash_file(CHAIN/'bin'/name)==value for name,value in marker['binaries'].items()),'installed Rust binary changed')
    if not NODE_ROOT.exists():
        name='node-v'+NODE+'-linux-x64.tar.xz';archive=downloads/name
        net.download('https://nodejs.org/dist/v'+NODE+'/'+name,archive,512*1024**2,expected_sha256=NODE_HASH)
        unpack=local.LOCAL/'tools'/('node-unpack-'+uuid.uuid4().hex)
        package=extract_verified(archive,unpack,'node-v'+NODE+'-linux-x64');package.rename(NODE_ROOT)
        new_file(NODE_ROOT/'.stage-install-complete.json',encode({'archive_sha256':NODE_HASH,'node_sha256':hash_file(NODE_ROOT/'bin/node')}))
    node_marker=json.loads((NODE_ROOT/'.stage-install-complete.json').read_text())
    need(node_marker['archive_sha256']==NODE_HASH and node_marker['node_sha256']==hash_file(NODE_ROOT/'bin/node'),'installed Node differs')
    result,_=run.phase('node-version',[NODE_ROOT/'bin/node','--version']);need(result['stdout'].strip()==('v'+NODE).encode(),'Node version mismatch')
    result,_=run.phase('node-verified-https',[NODE_ROOT/'bin/node','-e',NODE_TLS_CHECK],180)
    run.value['node_https']=json.loads(result['stdout'].decode('utf-8'))
    need(run.value['node_https'].get('all_verified'),'Node HTTPS trust remains blocked')
    result,_=run.phase('rust-version',[CHAIN/'bin/rustc','--version']);need(result['stdout'].startswith(b'rustc 1.85.1 '),'Rust version mismatch')
    if not PYTHON.exists():
        need(not PYTHON.parent.parent.exists(),'partial Python environment needs inspection')
        run.phase('create-linux-python',['/usr/bin/python3','-I','-B','-m','venv','--copies',PYTHON.parent.parent],180)
    run.phase('install-reviewed-python',[PYTHON,'-B','-m','pip','install','--only-binary=:all:','--disable-pip-version-check','--requirement',local.ROOT/'scripts/python-test-requirements.txt'],1800)
    run.phase('python-pip-check',[PYTHON,'-B','-m','pip','check'])
    run.phase('python-pins',[PYTHON,'-B','-c',"import sys;sys.path.insert(0,'scripts');import python_dependencies as p;print(p.require_installed('security'))"])
    run.phase('python-inventory',[PYTHON,'-B','-m','pip','freeze','--all'])
    run.phase('chromium-download',[PYTHON,'-B','-m','playwright','install','chromium'],1800)
    config=native_paths.configure_native();run.value['native_paths']=config
    # Fetch once against the real committed lock, then all builds/tests remain offline.
    os.environ.pop('CARGO_NET_OFFLINE',None)
    lock_before=hash_file(local.ROOT/'Cargo.lock')
    run.phase('locked-linux-fetch',[CHAIN/'bin/cargo','fetch','--locked','--target',HOST],1800)
    need(hash_file(local.ROOT/'Cargo.lock')==lock_before,'Cargo.lock changed during fetch')
    os.environ['CARGO_NET_OFFLINE']='true'
    run.value['status']='linux-tools-ready-build-and-db-tests-pending'


def verify_tools(run):
    """Offline cache validation only: no installers, dependency fetch or apt."""
    marker=json.loads((CHAIN/'.stage-install-complete.json').read_text())
    need(set(marker.get('binaries',{}))==set(native_paths.TOOLS),'cached Rust tool inventory differs')
    need(all(hash_file(CHAIN/'bin'/name)==digest for name,digest in marker['binaries'].items()),'cached Rust binary changed')
    node=json.loads((NODE_ROOT/'.stage-install-complete.json').read_text())
    need(node['archive_sha256']==NODE_HASH and node['node_sha256']==hash_file(NODE_ROOT/'bin/node'),'cached Node identity differs')
    result,_=run.phase('cached-rust-version',[CHAIN/'bin/rustc','--version'])
    need(result['stdout'].startswith(b'rustc 1.85.1 '),'cached Rust version mismatch')
    result,_=run.phase('cached-node-version',[NODE_ROOT/'bin/node','--version'])
    need(result['stdout'].strip()==('v'+NODE).encode(),'cached Node version mismatch')
    run.phase('cached-python-check',[PYTHON,'-B','-m','pip','check'])
    run.phase('cached-python-pins',[PYTHON,'-B','-c',"import sys;sys.path.insert(0,'scripts');import python_dependencies as p;print(p.require_installed('security'))"])
    browser_probe=(
        "from playwright.sync_api import sync_playwright\n"
        "with sync_playwright() as p:\n"
        "    browser=p.chromium.launch(headless=True)\n"
        "    try:\n"
        "        page=browser.new_page()\n"
        "        page.set_content('<p>cache-verification</p>')\n"
        "        assert page.locator('p').inner_text()=='cache-verification'\n"
        "        print(browser.version)\n"
        "    finally:\n"
        "        browser.close()\n")
    run.phase('cached-chromium-and-system-libraries',[PYTHON,'-B','-c',browser_probe],120)
    run.value['native_paths']=native_paths.configure_native()
    run.phase('cached-locked-metadata',[CHAIN/'bin/cargo','metadata','--locked','--offline','--no-deps','--format-version','1'])
    run.value['installers_or_downloads_run']=False
    run.value['status']='existing-linux-tools-verified-checks-pending'


def checks(run):
    need(Path(sys.prefix).resolve()==PYTHON.parent.parent.resolve(),'checks must use patched Linux Python')
    config=native_paths.configure_native();run.value['native_paths']=config
    import python_dependencies
    python_dependencies.require_installed('security')
    start=source_identity();run.value['source_before']=start
    old,fixed=baseline_bytes(local.ROOT);need((local.ROOT/'migrations'/__import__('fresh_baseline').TARGET).read_bytes()==fixed,'fresh baseline not applied')
    cargo=CHAIN/'bin/cargo';common=['--workspace','--all-targets','--all-features','--locked','--offline']
    run.phase('format',[cargo,'fmt','--all','--','--check'])
    run.phase('cargo-check',[cargo,'check',*common],3600)
    # Same scope as the established native collector, not an invented whole-tree lint claim.
    run.phase('clippy',[cargo,'clippy','-p','worldwright','-p','worldwright-studio','-p','worldwright-server',
                       '-p','worldwright-operations','-p','worldwright-persistence','--all-targets','--all-features','--locked','--offline','--','-D','warnings'],3600)
    run.phase('build-apps',[cargo,'build','--workspace','--all-features','--locked','--offline'],3600)
    failures=[]
    # Keep the live list in the report even if a later cleanup check throws.
    run.value['failures']=failures
    for name,args in [('workspace-tests',['test',*common,'--no-fail-fast','--','--test-threads=1']),
                      ('doctests',['test','--workspace','--all-features','--doc','--locked','--offline','--no-fail-fast','--','--test-threads=1'])]:
        result,ok=run.phase(name,[cargo,*args],3600,required=False)
        summaries=re.findall(rb'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;',result['stdout'])
        run.value[name]={'summaries':[[int(v) for v in x] for x in summaries],'process_passed':ok}
        if not ok or not summaries or any(int(x[1]) for x in summaries):failures.append(name)
    selected=['test-r0-r1.py','test-r3-io.py','test-r3-local.py','test-python-security.py','test-release-followthrough.py','test-linux-stage.py']
    for index,name in enumerate(selected):
        result,ok=run.phase('linux-helper-'+str(index),[PYTHON,'-B',local.ROOT/'scripts'/name],600,required=False)
        if not ok or not re.search(rb'Ran \d+ tests? in ',result['stdout']) or b'skipped=' in result['stdout']:failures.append(name)
    binaries=local.LOCAL/'target'/HOST/'debug'
    _,ok=run.phase('linux-real-desktop',[PYTHON,'-B',local.ROOT/'scripts/test-desktop.py','--real-studio',binaries/'worldwright-studio','--real-preview',binaries/'worldwright'],600,required=False)
    if not ok:failures.append('linux-real-desktop')
    pg=Path('/usr/lib/postgresql/16/bin');data=run.output/'pgdata';pwfile=run.output/'pg-password'
    socketdir=local.LOCAL/'pg-sockets'/uuid.uuid4().hex[:8];socketdir.mkdir(mode=0o700)
    password=secrets.token_urlsafe(32);new_file(pwfile,(password+'\n').encode())
    with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
    admin='worldwright_test_admin';database='worldwright_test_stage'
    url='postgresql://'+admin+':'+password+'@127.0.0.1:'+str(port)+'/'+database
    os.environ['WORLDWRIGHT_TEST_DATABASE_URL']=url;run.secret=tuple(run.secret)+(password,url)
    begun=False
    try:
        run.phase('pg-version',[pg/'postgres','--version'])
        run.phase('pg-init',[pg/'initdb','-D',data,'--username='+admin,'--pwfile='+str(pwfile),
                             '--auth-host=scram-sha-256','--auth-local=scram-sha-256','--encoding=UTF8','--no-locale'],120)
        begun=True
        run.phase('pg-start',[pg/'pg_ctl','-D',data,'-l',run.output/'postgres.log','-o',
                    '-h 127.0.0.1 -p '+str(port)+' -k '+str(socketdir),'start','-w','-t','30'],60)
        import psycopg
        from psycopg import sql
        with psycopg.connect(host='127.0.0.1',port=port,user=admin,password=password,dbname='postgres',autocommit=True) as conn:
            need(conn.execute("SELECT COUNT(*) FROM pg_database WHERE datname NOT IN ('postgres','template0','template1')").fetchone()[0]==0,'new cluster is not pristine')
            conn.execute(sql.SQL('CREATE DATABASE {}').format(sql.Identifier(database)))
        run.value['database']={'host':'127.0.0.1','port':port,'name':database,'data':str(data),'new_cluster':True}
        with psycopg.connect(url,autocommit=True) as conn:
            need(conn.execute('SELECT current_database()').fetchone()[0]==database,'DB identity mismatch')
            schema='stage_migrations_'+uuid.uuid4().hex[:12]
            conn.execute(sql.SQL('CREATE SCHEMA {}').format(sql.Identifier(schema)))
            try:
                conn.execute(sql.SQL('SET search_path TO {}').format(sql.Identifier(schema)))
                for name in inventory(local.ROOT):
                    with conn.transaction():conn.execute((local.ROOT/'migrations'/name).read_text())
                run.value['nine_current_migrations_applied']=True
            finally:conn.execute(sql.SQL('DROP SCHEMA {} CASCADE').format(sql.Identifier(schema)))
        result,_=run.phase('discover-native-db-tests',[cargo,'test','-p','worldwright-persistence','--lib','--all-features','--locked','--offline','--','--ignored','--list'],600)
        names=test_listing(result['stdout']);need(len(names)==34,'review changed native PostgreSQL test inventory')
        run.value['native_db_inventory']=names
        result,ok=run.phase('native-db-tests',[cargo,'test','-p','worldwright-persistence','--lib','--all-features','--locked','--offline','--','--ignored','--test-threads=1'],3600,required=False)
        db_summaries=re.findall(rb'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;',result['stdout'])
        run.value['native_db_result']={
            'process_passed':ok,'summaries':[[int(v) for v in row] for row in db_summaries],
            'failed_cases':[name.decode('utf-8') for name in re.findall(rb'^test (\S+) \.\.\. FAILED\r?$',result['stdout'],re.M)]}
        created_schemas={name.decode('ascii') for name in re.findall(rb'^WORLDWRIGHT_TEST_SCHEMA_CREATED=(ww_test_[0-9]+_[0-9]+)\r?$',result['stdout'],re.M)}
        if ok:
            try:need(test_result(result['stdout'],34)==34,'native DB count differs')
            except ValueError:ok=False
        if not ok:failures.append('native-db-tests')
        for label,filename,count in [('expiry-sql','test-r2-expiry-sql.py',9),('role-fixtures','test-r3-role-fixture.py',13)]:
            if label=='role-fixtures':os.environ['WORLDWRIGHT_R3_ROLE_FIXTURES']='1'
            result,ok=run.phase(label,[PYTHON,'-B',local.ROOT/'scripts'/filename],600,required=False)
            counts=re.findall(rb'Ran (\d+) tests? in ',result['stdout'])
            if not ok or counts!=[str(count).encode()] or b'skipped=' in result['stdout']:failures.append(label)
        with psycopg.connect(url,autocommit=True) as conn:
            leftovers=conn.execute("SELECT nspname FROM pg_namespace WHERE nspname NOT IN ('public','pg_catalog','information_schema') AND nspname NOT LIKE 'pg_toast%' AND nspname NOT LIKE 'pg_temp_%'").fetchall()
            # With bound parameters Psycopg parses every percent in the SQL.
            # Bind the LIKE pattern too, so its wildcard stays data, not syntax.
            databases=conn.execute("SELECT datname FROM pg_database WHERE datname LIKE %s AND datname<>%s",('worldwright_test%',database)).fetchall()
            roles=conn.execute("SELECT rolname FROM pg_roles WHERE rolname LIKE 'r3_fixture_%'").fetchall()
            run.value['db_cleanup_observation']={
                'schemas':[row[0] for row in leftovers],
                'databases':[row[0] for row in databases],
                'roles':[row[0] for row in roles],
                'schema_markers_from_native_failures':sorted(created_schemas),
                'leftover_schemas_all_attributed':all(row[0] in created_schemas for row in leftovers),
                'automatic_schema_deletion':False}
        run.value['db_fixture_cleanup_verified']=not leftovers and not databases and not roles
        if not run.value['db_fixture_cleanup_verified']:
            failures.append('db-fixture-cleanup')
            print('DB cleanup failed; exact leftovers retained in stage.json (no automatic deletion).',flush=True)
    finally:
        if begun and (data/'postmaster.pid').exists():
            _,stopped=run.phase('pg-stop',[pg/'pg_ctl','-D',data,'stop','-m','fast','-w','-t','30'],60,required=False)
            run.value['owned_cluster_stopped']=stopped and not (data/'postmaster.pid').exists()
            if not run.value['owned_cluster_stopped']:failures.append('owned-cluster-stop')
        elif begun:run.value['owned_cluster_stopped']=True
        os.environ.pop('WORLDWRIGHT_TEST_DATABASE_URL',None);os.environ.pop('WORLDWRIGHT_R3_ROLE_FIXTURES',None)
    end=source_identity();run.value['source_after']=end;need(start==end,'source changed during checks')
    run.value['failures']=failures
    run.value['remaining']=['orchestrated ignored server/COMMIT fixture','SQL-literal R6 rehearsal','full native browser collection',
                            'installed desktop/manual matrix','live deployment/load/recovery gates','independent signed approvals']
    run.value['status']='linux-build-and-persistence-checks-passed-acceptance-incomplete' if not failures else 'failed'


def main():
    parser=argparse.ArgumentParser();parser.add_argument('action',choices=['tools','verify-tools','checks']);args=parser.parse_args()
    configure();lock=local.LOCAL/'linux-stage.lock';run=Run('tools' if args.action=='verify-tools' else args.action);acquired=False
    run.value['requested_action']=args.action
    try:
        with lock.open('x') as stream:stream.write(str(os.getpid()))
        acquired=True
        run.value['enterprise_ca']=enterprise_ca.configure_linux_trust()
        need(run.value['enterprise_ca'] is not None,'approved enterprise trust repair receipt required; run repair-wsl-trust.py first')
        if args.action=='tools':tools(run)
        elif args.action=='verify-tools':verify_tools(run)
        else:checks(run)
    except (Exception,KeyboardInterrupt) as error:
        run.value['status']='failed';run.value['error']=r3_io.log_bytes(str(error).encode(),run.secret,4000).decode('utf-8','replace')
        print('Linux stage stopped: '+run.value['error'],flush=True)
    finally:
        if acquired:lock.unlink()
        run.finish()
    return 1 if run.value['status']=='failed' else 0


if __name__=='__main__':raise SystemExit(main())
