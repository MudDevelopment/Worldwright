"""Real local subprocess/redirect checks, not Worldwright deployment acceptance."""
from pathlib import Path
import hashlib
import http.server
import importlib.util
import json
import os
import sys
import tempfile
import threading
import time
import unittest
from unittest.mock import patch
from urllib.request import Request, build_opener, ProxyHandler
ROOT=Path(__file__).resolve().parents[1]
def load(name,path):
    spec=importlib.util.spec_from_file_location(name,path); module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module); return module
IO=load('r3_test_io',ROOT/'scripts/r3_io.py')
CLOSE=load('r3_test_close_io',ROOT/'scripts/r3-close.py')
class PhaseIO(unittest.TestCase):
    def run_child(self,source,timeout=2,limit=4096):
        return IO.collect_phase([sys.executable,'-c',source],timeout,limit=limit)
    def test_success_exact_stdout_stderr_and_exit(self):
        result=self.run_child("import os; os.write(1,b'one'); os.write(2,b'two')")
        self.assertEqual((result['returncode'],result['stdout'],result['interruption']),(0,b'onetwo',None))
    def test_positive_failure_is_observed_not_interrupted(self):
        result=self.run_child("import sys; print('failed'); sys.exit(7)")
        self.assertEqual(result['returncode'],7); self.assertIsNone(result['interruption'])
    def test_timeout_retains_partial_output_and_reaps_reader(self):
        result=self.run_child("import os,time; os.write(1,b'partial'); time.sleep(30)",timeout=.5)
        self.assertEqual(result['stdout'],b'partial'); self.assertEqual(result['interruption'],'timeout')
        self.assertFalse(any(t.name=='r3-phase-output' for t in threading.enumerate()))
        self.assertLess(result['elapsed_seconds'],5)
    def test_output_limit_is_bounded_and_cannot_pass(self):
        result=self.run_child("import os; os.write(1,b'x'*1000000)")
        self.assertEqual(len(result['stdout']),4096); self.assertEqual(result['interruption'],'output-limit')
    def test_exact_limit_can_pass(self):
        result=self.run_child("import os; os.write(1,b'x'*4096)")
        self.assertEqual(len(result['stdout']),4096); self.assertIsNone(result['interruption'])
    def test_stdin_is_noninteractive(self):
        result=self.run_child("import sys; print(len(sys.stdin.read()))")
        self.assertEqual(result['stdout'].strip(),b'0')
    def test_deadline_also_bounds_closed_stdout_live_process(self):
        result=self.run_child("import os,time; os.close(1); os.close(2); time.sleep(30)",timeout=.5)
        self.assertEqual(result['interruption'],'timeout'); self.assertLess(result['elapsed_seconds'],5)
    @unittest.skipUnless(os.name=='posix','actual process-group case is POSIX-specific; Windows remains separate')
    def test_exited_parent_with_live_pipe_descendant(self):
        source="import subprocess,sys; p=subprocess.Popen([sys.executable,'-c','import time; time.sleep(30)']); print(p.pid,flush=True)"
        result=self.run_child(source,timeout=.5)
        self.assertEqual(result['returncode'],0); self.assertEqual(result['interruption'],'timeout')
        pid=int(result['stdout']); state=Path('/proc')/str(pid)/'stat'
        self.assertTrue(not state.exists() or state.read_text().split()[2]=='Z')
    def test_invalid_budgets_and_no_shell_string(self):
        for bad in (True,0,-1,3601,'1'):
            with self.subTest(bad=bad),self.assertRaises(ValueError): IO.collect_phase([sys.executable,'-V'],bad)
        for bad in (True,0,-1,IO.MAX_CAPTURE+1):
            with self.subTest(bad=bad),self.assertRaises(ValueError): IO.collect_phase([sys.executable,'-V'],1,limit=bad)
        with self.assertRaises(ValueError): IO.collect_phase('echo unexpected-shell',1)
    def test_credential_masking_full_and_partial_at_cap(self):
        secret='synthetic-fixture-password'
        self.assertNotIn(secret.encode(),IO.log_bytes(('before '+secret+' after').encode(),(secret,)))
        self.assertEqual(IO.log_bytes(b'before synthetic-fixture-pass',(secret,)),b'before [REDACTED-PARTIAL]')
        with patch.dict(os.environ,{'PRIVATE_JSON':json.dumps({'username':'test','password':secret}),'PRIVATE_URL':'postgresql://u:encoded%40password@localhost/worldwright_test'},clear=True):
            found=IO.secret_values(('PRIVATE_JSON','PRIVATE_URL'))
            self.assertIn(secret,found); self.assertIn('encoded@password',found)
    def test_log_representation_stays_bounded_after_redaction(self):
        self.assertLessEqual(len(IO.log_bytes(b'a'*10000,('a',),limit=4096)),4096)
    def test_interruption_refused_even_for_positive_expected_failure(self):
        with tempfile.TemporaryDirectory() as name:
            parent=Path(name); (parent/'failed-rollout.log').write_bytes(b'partial')
            phase={'name':'failed-rollout','exit_code':1,'expected_exit_code':1,'log_sha256':hashlib.sha256(b'partial').hexdigest()}
            CLOSE.check_phases({'phases':[phase]},parent,{'failed-rollout'})
            for reason in ('timeout','output-limit','output-cleanup-failure',False):
                phase['interruption']=reason
                with self.subTest(reason=reason),self.assertRaises(ValueError): CLOSE.check_phases({'phases':[phase]},parent,{'failed-rollout'})
    def test_actual_redirect_target_is_never_contacted(self):
        touched=[]
        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                touched.append(self.path)
                if self.path=='/redirect':
                    self.send_response(302); self.send_header('Location','/target'); self.end_headers()
                else: self.send_response(200); self.end_headers(); self.wfile.write(b'ready')
            def log_message(self,*args): pass
        server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Handler)
        thread=threading.Thread(target=server.serve_forever,daemon=True); thread.start()
        try:
            client=build_opener(ProxyHandler({}),IO.NoRedirects()); base='http://127.0.0.1:'+str(server.server_port)
            with client.open(base+'/target',timeout=2) as response: self.assertEqual(response.read(),b'ready')
            touched.clear()
            with self.assertRaises(ValueError): client.open(Request(base+'/redirect',headers={'Cookie':'synthetic-fixture-only'}),timeout=2)
            self.assertEqual(touched,['/redirect'])
        finally: server.shutdown(); server.server_close(); thread.join(timeout=2)
    def test_actual_phase_log_retention_masking_and_exclusive_write(self):
        with tempfile.TemporaryDirectory() as name:
            parent=Path(name); report={'phases':[]}; secret='synthetic-phase-secret'
            result=IO.record_phase(report,parent,'command',[sys.executable,'-c',"print('synthetic-phase-secret')"],2,sensitive=(secret,))
            self.assertIn(secret.encode(),result['stdout']); self.assertNotIn(secret.encode(),(parent/'command.log').read_bytes())
            self.assertEqual(report['phases'][0]['log_sha256'],hashlib.sha256((parent/'command.log').read_bytes()).hexdigest())
            if os.name=='posix': self.assertEqual((parent/'command.log').stat().st_mode&0o777,0o600)
            with self.assertRaises(ValueError): IO.record_phase(report,parent,'command',[sys.executable,'-V'],2)
    def test_actual_interrupted_phase_persisted_before_classification(self):
        with tempfile.TemporaryDirectory() as name:
            parent=Path(name); report={'phases':[]}
            result=IO.record_phase(report,parent,'failed-rollout',[sys.executable,'-c',"import os,time; os.write(1,b'partial'); time.sleep(30)"],.5,expected=1)
            self.assertEqual((parent/'failed-rollout.log').read_bytes(),b'partial')
            self.assertEqual(report['phases'][0]['interruption'],'timeout')
            with self.assertRaises(ValueError): CLOSE.check_phases(report,parent,{'failed-rollout'})
    def test_private_api_body_omission_preserves_return_bytes(self):
        with tempfile.TemporaryDirectory() as name:
            parent=Path(name); report={'phases':[]}
            result=IO.record_phase(report,parent,'namespace-approval',[sys.executable,'-c',"print('synthetic-private-body')"],2,retain=False)
            self.assertIn(b'synthetic-private-body',result['stdout'])
            self.assertNotIn(b'synthetic-private-body',(parent/'namespace-approval.log').read_bytes())
    def test_real_collector_wiring_and_private_logs(self):
        image=(ROOT/'scripts/r3-image.py').read_text(); staging=(ROOT/'scripts/r3-staging.py').read_text()
        for source in (image,staging):
            self.assertIn('io.record_phase(',source); self.assertIn("result['interruption'] is None",source)
        self.assertIn('os.O_EXCL',(ROOT/'scripts/r3_io.py').read_text())
        self.assertIn('io.NoRedirects()',staging)
if __name__=='__main__': unittest.main(verbosity=2)
