"""R0/R1 helper/failure-policy tests. Synthetic packages are NOT native evidence."""
from pathlib import Path
import hashlib
import importlib.util
import json
import os
import signal
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, ROOT/'scripts'/file)
    value = importlib.util.module_from_spec(spec); spec.loader.exec_module(value)
    return value


package = load('private_package', 'package-desktop.py')
r0 = load('native_evidence', 'r0-verify.py')


class Contracts(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='worldwright-r0-r1-unit-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name); self.root.chmod(0o700)
        self.file = self.root/'fixture'; self.file.write_bytes(b'synthetic integrity data'); self.file.chmod(0o644)
        self.manifest = {'schema_version':1, 'distribution':'private-acceptance-only', 'files':[
            {'path':'fixture', 'bytes':self.file.stat().st_size, 'mode':0o644, 'sha256':hashlib.sha256(self.file.read_bytes()).hexdigest()}]}
        self.save()
    def save(self):
        (self.root/'package-manifest.json').write_text(json.dumps(self.manifest))
    def test_synthetic_integrity_inventory_only(self):
        self.assertEqual(package.verify(self.root), self.manifest)
    def test_malformed_package_records_are_rejected(self):
        base = json.loads(json.dumps(self.manifest))
        values = [[], {**base, 'schema_version':True}, {**base, 'files':[None]}]
        for field, invalid in [('bytes',True), ('mode',0o4755), ('sha256',None), ('path','')]:
            record = {**base['files'][0], field:invalid}
            values.append({**base, 'files':[record]})
        for value in values:
            with self.subTest(value=value):
                (self.root/'package-manifest.json').write_text(json.dumps(value))
                with self.assertRaises(ValueError): package.verify(self.root)
    def test_synthetic_source_inventory_hash_is_recomputed(self):
        close = load('source_validator', 'r0-r1-close.py')
        lock = hashlib.sha256(b'unit-only lock bytes').hexdigest()
        inventory = [{'path':'Cargo.lock','sha256_lf_text':lock}]
        source = {'commit':'0'*40, 'inventory':inventory, 'lock_sha256':lock,
                  'source_sha256':hashlib.sha256(json.dumps(inventory,sort_keys=True).encode()).hexdigest()}
        close.check_source(source)  # metadata helper only, NOT a native R0 report
        source['inventory'][0]['sha256_lf_text'] = 'f'*64
        with self.assertRaises(ValueError): close.check_source(source)
    def test_missing_source_and_null_digests_cannot_close(self):
        close = load('null_validator', 'r0-r1-close.py')
        with self.assertRaises(ValueError): close.check_source(None)
        with self.assertRaises(ValueError): close.require_digest(None,'missing')
    def test_rejected_package_build_preserves_existing_output(self):
        report = self.root/'failed-r0.json'
        report.write_text(json.dumps({'schema_version':1,'track':'R0','status':'failed'}))
        archive = self.root/'existing.tar.gz'; archive.write_bytes(b'keep original file')
        with self.assertRaises(ValueError): package.build(report, archive)
        self.assertEqual(archive.read_bytes(), b'keep original file')
    def test_tampered_content_rejected(self):
        self.file.write_bytes(b'changed')
        with self.assertRaises(ValueError): package.verify(self.root)
    def test_extra_file_rejected(self):
        (self.root/'extra').touch()
        with self.assertRaises(ValueError): package.verify(self.root)
    @unittest.skipIf(os.name == 'nt', 'Unix symlink/mode semantics require Linux gate')
    def test_symlink_rejected(self):
        self.file.unlink(); self.file.symlink_to(self.root/'package-manifest.json')
        with self.assertRaises(ValueError): package.verify(self.root)
    @unittest.skipIf(os.name == 'nt', 'Unix mode semantics require Linux gate')
    def test_wrong_mode_rejected(self):
        self.file.chmod(0o600)
        with self.assertRaises(ValueError): package.verify(self.root)
    def test_traversal_inventory_rejected(self):
        self.manifest['files'][0]['path'] = '../outside'; self.save()
        with self.assertRaises(ValueError): package.verify(self.root)
    def test_cross_host_text_identity_normalizes_only_line_endings(self):
        a = self.root/'a.txt'; b = self.root/'b.txt'
        a.write_bytes(b'one\ntwo\n'); b.write_bytes(b'one\r\ntwo\r\n')
        self.assertEqual(r0.canonical_sha(a), r0.canonical_sha(b))
        b.write_bytes(b'changed\n')
        self.assertNotEqual(r0.canonical_sha(a), r0.canonical_sha(b))
    def test_missing_native_tools_fails_and_records_no_pass(self):
        env = os.environ.copy(); env['PATH'] = ''
        output = self.root/'failed-evidence'
        result = subprocess.run([sys.executable, str(ROOT/'scripts/r0-verify.py'), '--output', str(output)], env=env, capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 1)
        report = json.loads((output/'r0-report.json').read_text())
        self.assertEqual(report['status'], 'failed')
        self.assertIn('missing required R0 tool', report['failure'])
        self.assertEqual(report['phases'], [])
    def test_incomplete_closure_evidence_is_rejected(self):
        invalid = self.root/'invalid.json'; invalid.write_text(json.dumps({'schema_version':1, 'track':'R0', 'status':'failed'}))
        close = load('closure_validator', 'r0-r1-close.py')
        with self.assertRaises(ValueError): close.check_r0(invalid, 'Linux')
    @unittest.skipUnless(sys.platform.startswith('linux'), 'Linux session/pidfd test')
    def test_real_kernel_pidfd_cleanup_of_cooperative_python_child(self):
        cleanup = load('kernel_cleanup', 'cleanup-session.py')
        child = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(30)'], start_new_session=True)
        try:
            self.assertTrue(cleanup.cleanup(child.pid, 3))
            self.assertEqual(child.wait(timeout=3), -signal.SIGTERM)
            with self.assertRaises(ValueError): cleanup.cleanup(os.getsid(0), 0)
        finally:
            if child.poll() is None: child.kill(); child.wait()


if __name__ == '__main__':
    unittest.main(verbosity=2)
