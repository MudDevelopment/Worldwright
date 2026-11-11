"""R4 local contract/negative tests. Synthetic records are never release proofs."""
from pathlib import Path
from types import SimpleNamespace
import base64,copy,importlib.util,json,os,sys,tempfile,time,unittest
from unittest.mock import patch
from r4_common import ROOT,PLAN_FIELDS,beta_report,plan_check,signed,read,write_new,contained,compare
from r4_monitor import metrics_parse,Monitor
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
spec=importlib.util.spec_from_file_location('r4_scenario_tests',ROOT/'scripts/r4-scenarios.py'); SCENARIOS=importlib.util.module_from_spec(spec); spec.loader.exec_module(SCENARIOS)
def report(smoke=False):
    clients,commands=(2,4) if smoke else (250,7500)
    return {'schema_version':1,'profile_version':1,'profile':'smoke-v1' if smoke else 'beta-250-v1','storage_expectation':'operator-supplied-postgresql','target_clients':clients,'connected_clients':clients,'expected_commands':commands,'completed_commands':commands,'failures':dict.fromkeys(('login','attach','command','protocol','disconnect','readiness','internal'),0),'readiness_checks':60,'elapsed_ms':60000,'latency':{'count':commands,'min_us':1,'p50_us':10,'p95_us':750000,'p99_us':1500000,'max_us':9999999},'passed':not smoke,'tooling_smoke_passed':smoke,'failure_reasons':[] if not smoke else ['smoke_profile'],'throughput_milli_commands_per_second':commands*1000000//60000,'run_nonce':'a'*16,'started_unix_seconds':100,'ended_unix_seconds':160,'artifact_digest':'a'*64,'environment_label':'synthetic-test','operating_system':'linux','architecture':'x86_64'}
def plan():
    value=dict.fromkeys(PLAN_FIELDS,'test')
    value.update(schema_version=1,kind='r4-performance-plan',status='adopted',reviewer='synthetic-fixture-not-an-owner',expires_at=int(time.time())+60,signature='',source_sha256='a'*64,artifact_digest='b'*64,retention_policy_sha256='c'*64,base_url='https://test.example',environment_label='r4-fixture',context='fixture',namespace='fixture',deployment='fixture',r3_arguments='/tmp/fixture-r3.json',credentials='/tmp/fixture-beta.json',scenario_credentials='/tmp/fixture-scenarios.json',database_url_env='WORLDWRIGHT_TEST_MONITOR_DATABASE_URL',runtime_role='fixture_runtime',schema='public',pod_ip='127.0.0.1',process_pid=42,telemetry_url='http://127.0.0.1:9090/metrics',soak_seconds=1800,max_rss_growth_bytes=1024,max_history_growth_bytes=2048)
    for field,name in (('r3_arguments','fixture-r3.json'),('credentials','fixture-beta.json'),('scenario_credentials','fixture-scenarios.json')): value[field]=str((Path(tempfile.gettempdir())/name).resolve())
    return value
class R4Tests(unittest.TestCase):
    def test_fixed_boundary_report(self): beta_report(report())
    def test_every_fixed_dimension_and_threshold_is_enforced(self):
        mutations=[('target_clients',249),('connected_clients',249),('expected_commands',7499),('completed_commands',7499),('elapsed_ms',90001),('readiness_checks',0),('passed',False),('profile','smoke-v1'),('schema_version',True)]
        for key,value in mutations:
            item=report(); item[key]=value
            with self.subTest(key=key),self.assertRaises(ValueError): beta_report(item)
        for key,value in [('p95_us',750001),('p99_us',1500001),('max_us',10000000),('count',7499),('count',True)]:
            item=report(); item['latency'][key]=value
            with self.subTest(key=key),self.assertRaises(ValueError): beta_report(item)
    def test_all_failure_dimensions_and_unknown_inventory(self):
        for key in report()['failures']:
            item=report(); item['failures'][key]=1
            with self.subTest(key=key),self.assertRaises(ValueError): beta_report(item)
        item=report(); item['failures']['login']=False
        with self.assertRaises(ValueError): beta_report(item)
        item=report(); del item['failures']['internal']
        with self.assertRaises(ValueError): beta_report(item)
    def test_smoke_cannot_be_beta(self):
        beta_report(report(True),smoke=True)
        with self.assertRaises(ValueError): beta_report(report(True))
        item=report(True); item['passed']=True
        with self.assertRaises(ValueError): beta_report(item,smoke=True)
    def test_percentile_order_throughput_and_time(self):
        item=report(); item['latency']['p50_us']=800000
        with self.assertRaises(ValueError): beta_report(item)
        item=report(); item['throughput_milli_commands_per_second']+=1
        with self.assertRaises(ValueError): beta_report(item)
        item=report(); item['ended_unix_seconds']=100
        with self.assertRaises(ValueError): beta_report(item)
    def test_strict_plan_and_bounds(self):
        plan_check(plan())
        for key,value in [('schema_version',True),('soak_seconds',10),('process_pid',False),('max_rss_growth_bytes',-1),('telemetry_url','http://0.0.0.0:9090/metrics'),('base_url','https://example.invalid'),('credentials','relative.json')]:
            item=plan(); item[key]=value
            with self.subTest(key=key),self.assertRaises(ValueError): plan_check(item)
        item=plan(); item['weakened_beta_clients']=2
        with self.assertRaises(ValueError): plan_check(item)
    def test_independent_signature_tamper_wrong_key_and_expiry(self):
        private=Ed25519PrivateKey.generate(); item=plan()
        payload={key:value for key,value in item.items() if key!='signature'}
        item['signature']=base64.b64encode(private.sign(json.dumps(payload,sort_keys=True,separators=(',',':')).encode())).decode()
        with tempfile.TemporaryDirectory() as name:
            path=Path(name)/'synthetic-public-key.pem'; path.write_bytes(private.public_key().public_bytes(serialization.Encoding.PEM,serialization.PublicFormat.SubjectPublicKeyInfo))
            signed(item,path,'r4-performance-plan')
            changed=copy.deepcopy(item); changed['soak_seconds']=1
            with self.assertRaises(Exception): signed(changed,path,'r4-performance-plan')
            changed=copy.deepcopy(item); changed['expires_at']=0
            with self.assertRaises(ValueError): signed(changed,path,'r4-performance-plan')
            path.write_bytes(Ed25519PrivateKey.generate().public_key().public_bytes(serialization.Encoding.PEM,serialization.PublicFormat.SubjectPublicKeyInfo))
            with self.assertRaises(Exception): signed(item,path,'r4-performance-plan')
    def test_bounded_files_create_new_duplicate_and_containment(self):
        with tempfile.TemporaryDirectory() as name:
            root=Path(name); path=root/'test.json'; write_new(path,{'x':1}); self.assertEqual(read(path),{'x':1})
            with self.assertRaises(FileExistsError): write_new(path,{'x':2})
            path.write_text('{"x":1,"x":2}')
            with self.assertRaises(ValueError): read(path)
            with self.assertRaises(ValueError): contained(root,'../outside')
            for invalid in ('NaN','1e9999'):
                path.write_text(invalid)
                with self.assertRaises(ValueError): read(path)
            if os.name=='posix': self.assertEqual(path.stat().st_mode&0o777,0o600)
    def test_measured_comparison_refuses_incomparable_or_unproven(self):
        result=compare(report(),report(),'synthetic-test rationale, not a measured real bottleneck'); self.assertFalse(result['supports_capacity_increase'])
        item=report(); item['environment_label']='other'
        with self.assertRaises(ValueError): compare(report(),item,'fixture')
        with self.assertRaises(ValueError): compare(report(),report(),'')
    def test_telemetry_fixed_dimensions_and_failures(self):
        raw=b'worldwright_ready 1\nworldwright_connected_clients 2\nworldwright_pg_pool_size 8\nworldwright_pg_pool_idle 4\n'
        self.assertEqual(metrics_parse(raw)['worldwright_pg_pool_idle'],4)
        for invalid in (raw.replace(b'ready 1',b'ready 0'),raw.replace(b'pool_idle 4',b'pool_idle NaN'),raw+raw,raw[:40]):
            with self.assertRaises((ValueError,UnicodeError)): metrics_parse(invalid)
    def test_actual_protocol_tracker_fixture_and_gap_rejection(self):
        client=SCENARIOS.Session.__new__(SCENARIOS.Session); client.epoch=None; client.sequence=0
        class Socket:
            def recv(self,timeout): return json.dumps({'protocol_version':1,'stream_epoch':'a'*32,'sequence':1,'message':{'type':'welcome','heartbeat_interval_ms':10000}})
        client.socket=Socket(); self.assertEqual(client.receive()['type'],'welcome')
        with self.assertRaises(ValueError): client.receive()
    def test_actual_failed_collection_is_retained_without_mutation(self):
        spec=importlib.util.spec_from_file_location('r4_failed_collection_test',ROOT/'scripts/r4-verify.py'); module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
        with tempfile.TemporaryDirectory() as name:
            output=Path(name)/'new-evidence'
            args=SimpleNamespace(output=output,confirm_load=False,plan=Path(name)/'missing-plan.json',approval_public_key=Path(name)/'missing-key.pem')
            self.assertEqual(module.collect(args),1); value=read(output/'r4-report.json')
            self.assertEqual(value['status'],'failed'); self.assertEqual(value['phases'],[])
            self.assertIn('confirmation',value['failure']['reason'])
    def test_deployment_replicas_boolean_and_replacement_policy_refused(self):
        spec=importlib.util.spec_from_file_location('r4_deployment_test',ROOT/'scripts/r4-verify.py'); module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
        value={'metadata':{'name':'fixture','namespace':'fixture','uid':'fixture-not-deployment','generation':1},'spec':{'replicas':1,'strategy':{'type':'Recreate'},'template':{'spec':{'containers':[{'image':'fixture/repo@sha256:'+'a'*64}]}}},'status':{'readyReplicas':1}}
        module.check_deployment(value,plan())
        for field in ('replicas','readyReplicas'):
            changed=copy.deepcopy(value); changed['spec' if field=='replicas' else 'status'][field]=True
            with self.assertRaises(ValueError): module.check_deployment(changed,plan())
    def test_real_loopback_monitor_transport_and_fixture_authority_drift(self):
        import http.server,threading
        body=b'worldwright_ready 1\nworldwright_connected_clients 2\nworldwright_pg_pool_size 8\nworldwright_pg_pool_idle 4\n'
        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(self): self.send_response(200); self.end_headers(); self.wfile.write(body)
            def log_message(self,*args): pass
        server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Handler); thread=threading.Thread(target=server.serve_forever,daemon=True); thread.start()
        try:
            candidate=plan(); candidate['telemetry_url']='http://127.0.0.1:'+str(server.server_port)+'/metrics'
            monitor=Monitor(candidate,Path('/tmp/NOT-ACTUAL-MONITOR-EVIDENCE'))
            from r4_monitor import TABLES
            class Cursor:
                def __init__(self,value): self.value=value
                def fetchone(self): return self.value
                def fetchall(self): return self.value
            class Database:
                def execute(self,query,params=None):
                    text=str(query)
                    if 'worldwright_worlds' in text: return Cursor((1,'fixture-writer','fixture-incarnation','b'*64,1,False))
                    if 'current_setting' in text: return Cursor(('16-fixture','fixture-monitor','worldwright_test_fixture'))
                    if 'pg_stat_activity' in text: return Cursor((1,))
                    if 'pg_stat_database' in text: return Cursor((1,0,1,1,0,'fixture-reset'))
                    if 'pg_stat_user_tables' in text: return Cursor([(name,1,1,0,1024) for name in TABLES])
                    return Cursor(None)
            with patch.object(monitor,'process',return_value=({'fixture':'not-server-process'},1024,1.0)):
                first=monitor.sample(Database()); self.assertEqual(first['telemetry']['worldwright_ready'],1)
                monitor.first['writer_id']='changed-fixture'
                with self.assertRaises(ValueError): monitor.sample(Database())
        finally: server.shutdown(); server.server_close(); thread.join(timeout=2)
    def test_cli_help_and_failed_closure_are_not_passes(self):
        import subprocess
        for name in ('r4-verify.py','r4-scenarios.py'):
            result=subprocess.run([sys.executable,str(ROOT/'scripts'/name),'--help'],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=10); self.assertEqual(result.returncode,0)
        with tempfile.TemporaryDirectory() as name:
            path=Path(name)/'failed.json'; write_new(path,{'status':'failed'})
            result=subprocess.run([sys.executable,str(ROOT/'scripts/r4-verify.py'),'close','--report',str(path),'--adoption',str(Path(name)/'absent.json'),'--approval-public-key',str(Path(name)/'absent.pem')],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=10)
            self.assertEqual(result.returncode,1); self.assertIn(b'R4 NOT CLOSED',result.stdout)
    def test_source_wiring_preserves_profile_and_private_monitoring(self):
        cli=(ROOT/'apps/worldwright-load/src/cli.rs').read_text(); runner=(ROOT/'apps/worldwright-load/src/runner.rs').read_text(); pg=(ROOT/'crates/worldwright-persistence/src/game_state.rs').read_text()
        self.assertIn('Self::Beta250 => (250, 30, 2_000)',cli); self.assertIn('ClientCommand::Look',runner); self.assertIn('mpsc::channel(target_clients * commands)',runner); self.assertIn('tooling_smoke_passed',runner)
        self.assertIn('self.measured_replace',pg); self.assertIn('self.measured_commit',pg)
        monitor=(ROOT/'scripts/r4_monitor.py').read_text(); self.assertIn("sslmode='verify-full'",monitor); self.assertNotIn('/api/v1/operator',monitor)
if __name__=='__main__': unittest.main(verbosity=2)
