"""Real restricted staging install/restart/rotation/failed-rollout HTTPS checks.
Requires existing approved context/namespace, pre-seeded quiescent test world,
external credential references, passed current R0/R2, and real built image.
CNI negative controls, termination exit evidence, old-secret revocation and content
rollback/schema review remain required independently witnessed closure cases.
"""
from pathlib import Path
import argparse
import hashlib
import importlib.util
import json
import os
import re
import shutil
import ssl
import subprocess
import sys
import time
import uuid
from urllib.parse import urlsplit
from urllib.request import Request,build_opener,HTTPSHandler,ProxyHandler
from urllib.error import HTTPError
import psycopg
import yaml
from jsonschema import ValidationError
from websockets.sync.client import connect
from websockets.exceptions import InvalidStatus, WebSocketException

ROOT=Path(__file__).resolve().parents[1]
R2_OPTIONS={'collection','ubuntu-r0','windows-r0','process-report','controlled-report','provider-report','approval-public-key'}
AUTO_CASES={'startup-tls','single-writer-reconstruction','db-role-audit','websocket-idle-origin-operator-denial','secret-rotation-restart','failed-rollout-helm-rollback'}
MANUAL_CASES={'networkpolicy-live-positive-negative','controlled-termination','content-rollback-schema-review','secret-rotation-revocation'}

def require(value,message):
    if not value: raise ValueError(message)
def load(name,path):
    spec=importlib.util.spec_from_file_location(name,path); result=importlib.util.module_from_spec(spec); spec.loader.exec_module(result); return result
def read(path):
    path=Path(path); require(path.is_file() and not path.is_symlink() and path.stat().st_size<=4*1024*1024,'bounded regular input required'); return json.loads(path.read_text())
def phase_outcome(code,expected):
    # bool is an int subclass, but is not observed process-exit evidence.
    return type(code) is int and type(expected) is int and expected in (0,1) and (code==0 if expected==0 else code>0)

def plan_check(plan):
    required={'configVersion','context','namespace','release','values_file','image_report','r2_arguments','rotation_secret','failed_digest','database_url_env','credentials_env','world_id','artifact_digest','runtime_role','migration_role','recovery_role','schema'}
    require(set(plan)==required|({'ca_file'} if 'ca_file' in plan else set()),'complete known plan fields required')
    require(type(plan['configVersion']) is int and plan['configVersion']==1,'plan configVersion must be integer 1')
    require(re.fullmatch(r'worldwright-r3-[a-z0-9-]{1,40}',plan['namespace']),'explicit isolated worldwright-r3 namespace required')
    require(re.fullmatch(r'[a-z][a-z0-9-]{0,30}',plan['release']),'bounded release required')
    require(isinstance(plan['context'],str) and plan['context'] and not plan['context'].startswith('-'),'explicit context required')
    require(re.fullmatch(r'sha256:[0-9a-f]{64}',plan['failed_digest']),'explicit reviewed failing image digest required')
    require(re.fullmatch(r'[0-9a-f]{64}',plan['artifact_digest']),'artifact digest required')
    require(uuid.UUID(plan['world_id']).int>0,'nonzero world UUID required')
    require(set(plan['r2_arguments'])==R2_OPTIONS,'all current R2 closure arguments required')

def main():
    parser=argparse.ArgumentParser(); parser.add_argument('--plan',type=Path,required=True); parser.add_argument('--output',type=Path,required=True); parser.add_argument('--confirm-staging-mutations',action='store_true')
    args=parser.parse_args(); output=args.output.resolve()
    require(not output.exists() and not output.is_relative_to(ROOT),'new external evidence directory required'); output.mkdir(mode=0o700)
    report={'schema_version':1,'track':'R3','kind':'staging-collection','status':'failed','full_r3_closed':False,'phases':[],'cases':[],'manual_cases_required':sorted(MANUAL_CASES)}
    io=load('r3_staging_io',ROOT/'scripts/r3_io.py'); sensitive=io.secret_values()
    def run(name,command,expected=0,timeout=180,retain=True):
        result=io.record_phase(report,output,name,command,timeout,cwd=ROOT,sensitive=sensitive,retain=retain,expected=expected)
        require(result['interruption'] is None and phase_outcome(result['returncode'],expected),'required staging phase failed/interrupted: '+name)
        return result['stdout']
    def case(name): report['cases'].append({'case':name,'status':'passed'}); print('R3 observed:',name,flush=True)
    try:
        require(args.confirm_staging_mutations,'explicit --confirm-staging-mutations required')
        require(all(shutil.which(tool) for tool in ('helm','kubectl','docker')),'Helm/kubectl/Docker required; missing tools are failure')
        plan=read(args.plan); plan_check(plan)
        sensitive=io.secret_values((plan['database_url_env'],plan['credentials_env']))
        chart=ROOT/'deploy/helm/worldwright'; verify=load('r3_chart',chart/'tests/verify.py')
        values=verify.merge(yaml.safe_load((chart/'values.yaml').read_text()),yaml.safe_load(Path(plan['values_file']).read_text()))
        verify.validate_values(values,chart)
        require('.invalid' not in values['publicOrigin'],'actual approved HTTPS origin required')
        require(plan['rotation_secret'] not in (values['postgres']['existingSecret'],values['postgres']['migrationSecret']),'pre-provisioned rotated runtime secret must be distinct')
        image=read(plan['image_report']); require(image.get('status')=='passed' and image.get('kind')=='image-build','actual passed image report required')
        require(image['image_reference']==values['image']['repository']+'@'+values['image']['digest'],'chart/image tuple differs')
        source=load('r3_source',ROOT/'scripts/r0-verify.py').source_identity(); require(source==image['source'],'current image source differs')
        report.update(source=source,image_reference=image['image_reference'],artifact_digest=plan['artifact_digest'],game=values['game'],context=plan['context'],namespace=plan['namespace'],release=plan['release'],runtime_role=plan['runtime_role'],migration_role=plan['migration_role'],recovery_role=plan['recovery_role'],schema=plan['schema'])
        command=[sys.executable,'scripts/r2-verify.py','close']
        for option in sorted(R2_OPTIONS): command+=['--'+option,str(plan['r2_arguments'][option])]
        run('r0-r2-prerequisites',command)
        run('chart-matrix',[sys.executable,str(chart/'tests/verify.py')])
        kube=['kubectl','--context',plan['context'],'--namespace',plan['namespace']]
        namespace=json.loads(run('namespace-approval',['kubectl','--context',plan['context'],'get','namespace',plan['namespace'],'-o','json'],retain=False))
        require(namespace['metadata'].get('labels',{}).get('worldwright.io/acceptance')=='r3','existing namespace must be explicitly marked for R3 acceptance')
        report['namespace_uid']=namespace['metadata']['uid']
        run('db-role-audit',[sys.executable,'scripts/r3-roles.py','--database-url-env',plan['database_url_env'],'--runtime-role',plan['runtime_role'],'--migration-role',plan['migration_role'],'--recovery-role',plan['recovery_role'],'--schema',plan['schema'],'--output',str(output/'roles.json')]); case('db-role-audit')
        url=os.environ.get(plan['database_url_env']); require(url,'audit DB URL required')
        db=psycopg.connect(url,autocommit=True,connect_timeout=5)
        try:
            require('worldwright_test' in db.execute('SELECT current_database()').fetchone()[0],'isolated test DB required')
            require(re.fullmatch(r'[a-z_][a-z0-9_]{0,62}',plan['schema']),'reviewed simple schema identifier required')
            from psycopg import sql
            db.execute(sql.SQL('SET search_path TO {}').format(sql.Identifier(plan['schema'])))
            db.execute("SET statement_timeout='5s'")
            def snapshot():
                with db.transaction():
                    db.execute('SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY')
                    row=db.execute("SELECT encode(active_release_digest,'hex'),authority_incarnation::text,writer_epoch,game_id,game_state_version,runtime_revision,pending_activation_id,release_maintenance FROM worldwright_worlds WHERE world_id=%s",(uuid.UUID(plan['world_id']),)).fetchone()
                    require(row and row[0]==plan['artifact_digest'] and row[3]=='worldwright.'+values['game'] and row[6] is None and not row[7],'coherent active artifact/game/authority required')
                    count=db.execute('SELECT COUNT(*) FROM worldwright_scheduled_counter_adjustments WHERE world_id=%s',(uuid.UUID(plan['world_id']),)).fetchone()[0]
                    require(count==0,'dedicated quiescent acceptance world must have no due-work schedule')
                    counters=db.execute('SELECT character_id,counter_key,counter_value FROM worldwright_game_counters WHERE world_id=%s ORDER BY character_id,counter_key',(uuid.UUID(plan['world_id']),)).fetchall()
                    rooms=db.execute('SELECT character_id,room_definition FROM worldwright_characters WHERE world_id=%s ORDER BY character_id',(uuid.UUID(plan['world_id']),)).fetchall()
                    return {'active':row[0],'incarnation':row[1],'epoch':row[2],'game':row[3],'state_version':row[4],'revision':row[5],'projection_sha256':hashlib.sha256(json.dumps([counters,rooms],default=str).encode()).hexdigest()}
            before=snapshot(); report['before']=before
            helm=['helm','--kube-context',plan['context'],'--namespace',plan['namespace']]
            def upgrade(name,overrides):
                return run(name,helm+['upgrade','--install',plan['release'],str(chart),'--values',str(plan['values_file']),'--wait','--timeout','120s']+overrides,timeout=150)
            upgrade('install',[])
            origin=values['publicOrigin']; ssl_context=ssl.create_default_context(cafile=plan.get('ca_file')); http=build_opener(ProxyHandler({}),HTTPSHandler(context=ssl_context),io.NoRedirects())
            def request(path,method='GET',body=None,cookie=None):
                headers={'Origin':origin}
                if body is not None: headers['Content-Type']='application/json'
                if cookie: headers['Cookie']=cookie
                try: return http.open(Request(origin+path,data=None if body is None else json.dumps(body).encode(),headers=headers,method=method),timeout=10)
                except HTTPError as error: return error
            def ready():
                with request('/health/ready') as response: require(response.status==200 and response.read(64).strip()==b'ready','actual TLS readiness required')
            ready(); case('startup-tls')
            deployment=json.loads(run('deployment',kube+['get','deployment',plan['release']+'-worldwright','-o','json'],retain=False))
            require(deployment['spec']['replicas']==1 and deployment['spec']['strategy']['type']=='Recreate' and deployment['status'].get('readyReplicas')==1,'one actual ready Recreate writer required')
            report['deployment_uid']=deployment['metadata']['uid']
            selector='app.kubernetes.io/name=worldwright,app.kubernetes.io/instance='+plan['release']
            def observe_pod(phase):
                pods=json.loads(run(phase,kube+['get','pods','--selector',selector,'-o','json'],retain=False))['items']
                pods=[pod for pod in pods if not pod['metadata'].get('deletionTimestamp')]
                require(len(pods)==1 and len(pods[0]['spec']['containers'])==1,'exactly one nonterminating workload pod required')
                pod=pods[0]; status=pod['status']['containerStatuses'][0]
                require(status.get('ready') and status['imageID'].split('@')[-1] in image['accepted_image_ids'],'actual ready pod imageID differs from built OCI manifest/platform identities')
                peers=db.execute("SELECT COUNT(*) FROM pg_stat_activity WHERE datname=current_database() AND usename=%s AND client_addr=%s::inet",(plan['runtime_role'],pod['status']['podIP'])).fetchone()[0]
                require(peers>0,'actual pod must connect as runtime role over reviewed private direct Pod-IP path')
                return {'uid':pod['metadata']['uid'],'image_id':status['imageID'],'runtime_db_peers':peers}
            initial_pod=observe_pod('actual-pod-image'); report['initial_pod']=initial_pod
            require(snapshot()['projection_sha256']==before['projection_sha256'],'startup projection differs'); case('single-writer-reconstruction')
            credentials=json.loads(os.environ[plan['credentials_env']]); require(set(credentials)=={'username','password'},'single test-account credentials required')
            with request('/api/v1/session','POST',credentials) as response:
                require(response.status==204,'real test login failed'); raw=response.headers.get('Set-Cookie',''); require('HttpOnly' in raw and 'Secure' in raw,'private secure session cookie required'); cookie=raw.split(';',1)[0]
            wsurl='wss://'+urlsplit(origin).netloc+'/api/v1/play'
            with connect(wsurl,origin=origin,additional_headers={'Cookie':cookie},ssl=ssl_context,proxy=None,max_size=16384,open_timeout=10) as ws:
                sequence=0; epoch=None
                def message_until(predicate):
                    nonlocal sequence,epoch
                    for _ in range(100):
                        value=json.loads(ws.recv(timeout=10)); require(value['protocol_version']==1 and value['sequence']==sequence+1,'wire version/sequence mismatch'); sequence+=1
                        if epoch is None: epoch=value['stream_epoch']
                        require(value['stream_epoch']==epoch,'stream epoch changed'); message=value['message']; require(message['type']!='error','unexpected protocol error')
                        if predicate(message): return message
                    raise ValueError('bounded expected wire result missing')
                ws.send(json.dumps({'type':'hello','supported_versions':[1]})); message_until(lambda m:m['type']=='welcome')
                request_id=uuid.uuid4().hex; ws.send(json.dumps({'type':'attach','request_id':request_id})); message_until(lambda m:m['type']=='command_result' and m.get('request_id')==request_id)
                for nonce in range(1,5):
                    time.sleep(20); ws.send(json.dumps({'type':'ping','nonce':nonce})); message_until(lambda m:m['type']=='pong' and m['nonce']==nonce)
                request_id=uuid.uuid4().hex; ws.send(json.dumps({'type':'command','request_id':request_id,'command':{'type':'look'}})); message_until(lambda m:m['type']=='command_result' and m.get('request_id')==request_id)
            try:
                with connect(wsurl,origin='https://denied.example.invalid',additional_headers={'Cookie':cookie},ssl=ssl_context,proxy=None,open_timeout=10):
                    raise ValueError('foreign WebSocket origin unexpectedly accepted')
            except InvalidStatus as denied:
                require(denied.response.status_code==403,'foreign-origin denial failed for unrelated reason')
            for path in ('/api/v1/operator/status','/api/v1/operator/metrics','/api/v1/operator/drain','/api/v1/operator/releases'):
                with request(path) as response: require(response.status==404,'public operator route must be absent')
            case('websocket-idle-origin-operator-denial')
            original=snapshot(); upgrade('rotate-runtime-secret',['--set-string','postgres.existingSecret='+plan['rotation_secret']]); ready(); rotated=snapshot()
            rotated_pod=observe_pod('rotated-pod-image'); report['rotated_pod']=rotated_pod
            require(rotated_pod['uid']!=initial_pod['uid'],'rotation did not replace the actual pod')
            require(rotated['incarnation']==original['incarnation'] and rotated['epoch']>original['epoch'] and rotated['projection_sha256']==original['projection_sha256'] and rotated['state_version']>=original['state_version'] and rotated['revision']>=original['revision'],'rotation/restart reconstruction or fence differs'); case('secret-rotation-restart')
            current=json.loads(run('helm-status',helm+['status',plan['release'],'-o','json'],retain=False))['version']
            run('failed-rollout',helm+['upgrade',plan['release'],str(chart),'--values',str(plan['values_file']),'--set-string','postgres.existingSecret='+plan['rotation_secret'],'--set-string','image.digest='+plan['failed_digest'],'--wait','--timeout','30s'],expected=1,timeout=45)
            with request('/health/ready') as response: require(response.status!=200,'failed rollout must not advertise readiness')
            require(snapshot()['projection_sha256']==rotated['projection_sha256'],'failed rollout changed game projection')
            run('helm-rollback',helm+['rollback',plan['release'],str(current),'--wait','--timeout','120s'],timeout=150); ready()
            after=snapshot(); rollback_pod=observe_pod('rollback-pod-image'); report['rollback_pod']=rollback_pod
            require(rollback_pod['uid']!=rotated_pod['uid'],'failed rollout/rollback did not reconstruct a new actual pod')
            require(after['projection_sha256']==rotated['projection_sha256'] and after['incarnation']==rotated['incarnation'] and after['epoch']>rotated['epoch'] and after['state_version']>=rotated['state_version'] and after['revision']>=rotated['revision'],'manifest rollback changed projection or rewound authority/version'); case('failed-rollout-helm-rollback')
            report['after']=after
        finally: db.close()
        require(load('r3_source_end',ROOT/'scripts/r0-verify.py').source_identity()==source,'source changed during staging run')
        report['roles_sha256']=hashlib.sha256((output/'roles.json').read_bytes()).hexdigest()
        require({item['case'] for item in report['cases']}==AUTO_CASES,'required automated case absent')
        report['status']='collected_not_closed'; code=0
    except (OSError,ValueError,KeyError,TypeError,AssertionError,ValidationError,psycopg.Error,WebSocketException,subprocess.SubprocessError) as error:
        report['failure']=type(error).__name__+': '+(str(error) if isinstance(error,ValueError) else 'staging gate failed; inspect retained noncredential phase logs'); code=1
    (output/'staging.json').write_text(json.dumps(report,indent=2)+'\n'); print('R3 staging:',report['status']); return code
if __name__=='__main__': raise SystemExit(main())
