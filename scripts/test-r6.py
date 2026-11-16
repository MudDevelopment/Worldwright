"""R6 collector/validator fixture tests, not application or operational acceptance."""
from pathlib import Path
import contextlib
import importlib.util
import io
import json
import os
import sys
import tempfile
import unittest
from unittest.mock import patch
import r6_support as support

spec = importlib.util.spec_from_file_location('r6_evidence', Path(__file__).with_name('r6-evidence.py'))
evidence = importlib.util.module_from_spec(spec)
spec.loader.exec_module(evidence)


class R6Tests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='r6-helper-test-')
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()

    def bundle(self):
        collector = support.Collector(self.root / 'collection', 'real-studio-browser')
        artifacts = collector.output / 'workspace/artifacts'
        artifacts.mkdir(parents=True)
        for name in sorted(evidence.BROWSER_CASES):
            details = {}
            if name.startswith('real-preview-'):
                data = name.encode()
                digest = support.hashlib.sha256(data).hexdigest()
                support.new_file(artifacts / (digest + '.wwr'), data)
                details = {'artifact_digest': digest, 'payload_digest': 'a' * 64}
            collector.case(name, **details)
        collector.report['binaries'] = {name: {'path': '/synthetic-only/' + name, 'sha256': 'b' * 64} for name in ('studio', 'preview')}
        collector.report.update(status='browser_workflow_passed_windows_shutdown_open',
            browser_version='synthetic-only', windows_shutdown={'status': 'manual_pending'})
        with contextlib.redirect_stdout(io.StringIO()): collector.finish()
        return collector.output / 'r6-report.json'

    def edit_report(self, path, change):
        value = json.loads(path.read_text())
        change(value)
        path.write_bytes(support.encode(value))

    def test_valid_fixture_still_has_no_closure_or_adoption(self):
        path = self.bundle()
        result = evidence.verify(path)
        self.assertIs(result['full_r6_closed'], False)
        self.assertIs(result['independent_adoption'], False)

    def test_tampered_file_is_rejected(self):
        path = self.bundle()
        (path.parent / 'events.jsonl').write_bytes(b'modified\n')
        with self.assertRaises(ValueError): evidence.verify(path)

    def test_unlisted_file_is_rejected(self):
        path = self.bundle()
        (path.parent / 'injected.txt').write_text('not inventoried')
        with self.assertRaises(ValueError): evidence.verify(path)

    def test_missing_browser_case_is_rejected(self):
        path = self.bundle()
        self.edit_report(path, lambda value: value['cases'].pop())
        with self.assertRaises(ValueError): evidence.verify(path)

    def test_closure_claim_is_rejected(self):
        path = self.bundle()
        self.edit_report(path, lambda value: value.update(full_r6_closed=True))
        with self.assertRaises(ValueError): evidence.verify(path)

    def test_boolean_schema_is_not_integer_version(self):
        path = self.bundle()
        self.edit_report(path, lambda value: value.update(schema_version=True))
        with self.assertRaises(ValueError): evidence.verify(path)

    def test_duplicate_case_is_rejected(self):
        path = self.bundle()
        self.edit_report(path, lambda value: value['cases'].append(value['cases'][0]))
        with self.assertRaises(ValueError): evidence.verify(path)

    def test_empty_and_duplicate_test_discovery_fail(self):
        for text in (b'0 tests\n', b'a: test\na: test\n'):
            with self.assertRaises(ValueError): support.test_listing(text)
        self.assertEqual(support.test_listing(b'module::case: test\r\n'), ['module::case'])

    def test_ignored_and_zero_or_incomplete_execution_fail(self):
        for text in (b'test result: ok. 0 passed; 0 failed; 0 ignored;', b'test result: ok. 2 passed; 0 failed; 1 ignored;', b'finished compilation'):
            with self.assertRaises(ValueError): support.test_result(text, 2)
        self.assertEqual(support.test_result(b'test result: ok. 2 passed; 0 failed; 0 ignored;', 2), 2)

    def test_path_escape_refused(self):
        for name in ('../escape', '/absolute', 'C:/absolute', 'a\\b'):
            with self.assertRaises(ValueError): support.safe_path(self.root, name)

    def test_new_output_is_exclusive(self):
        support.Collector(self.root / 'collection', 'test')
        with self.assertRaises(ValueError): support.Collector(self.root / 'collection', 'test')

    def test_nonzero_phase_is_retained_before_failure(self):
        collector = support.Collector(self.root / 'failed', 'test')
        with contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaises(ValueError): collector.phase('intentional-fail', [sys.executable, '-c', 'raise SystemExit(7)'])
            collector.finish()
        value = json.loads((collector.output / 'r6-report.json').read_text())
        self.assertEqual(value['phases'][0]['exit_code'], 7)
        self.assertEqual(value['status'], 'failed')
        self.assertTrue((collector.output / 'intentional-fail.log').exists())

    def test_timeout_is_failure_not_success(self):
        collector = support.Collector(self.root / 'timeout', 'test')
        with contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaises(ValueError): collector.phase('timeout', [sys.executable, '-c', 'import time;time.sleep(5)'], 0.1)
        self.assertEqual(collector.report['phases'][0]['interruption'], 'timeout')

    def test_known_secret_is_redacted(self):
        with patch.dict(os.environ, {'R6_TEST_TOKEN': 'test-only-redaction-canary'}):
            collector = support.Collector(self.root / 'redaction', 'test')
            with contextlib.redirect_stdout(io.StringIO()):
                collector.phase('redact', [sys.executable, '-c', 'import os;print(os.environ["R6_TEST_TOKEN"])'])
        log = (collector.output / 'redact.log').read_text()
        self.assertNotIn('test-only-redaction-canary', log)
        self.assertIn('[REDACTED]', log)


if __name__ == '__main__':
    unittest.main(verbosity=2)
