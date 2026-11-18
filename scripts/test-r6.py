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
import r6_subjects as subjects

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

    def test_native_v2_rejects_v1_without_inventing_server_evidence(self):
        collector = support.Collector(self.root / 'native', 'native-browser-security-prerequisites')
        self.assertEqual(collector.report['schema_version'], 2)
        self.assertIn('server-r6', evidence.NATIVE_CASES)
        with contextlib.redirect_stdout(io.StringIO()): collector.finish()
        path = collector.output / 'r6-report.json'
        self.edit_report(path, lambda value: value.update(schema_version=1))
        with self.assertRaisesRegex(ValueError, 'native v1 requires recollection'):
            evidence.verify(path)

    def test_server_build_tests_and_clippy_are_wired(self):
        import ast
        tree = ast.parse(Path(__file__).with_name('r6-native.py').read_text())
        calls = [v for v in ast.walk(tree) if isinstance(v, ast.Call)]
        for phase in ('build-studio', 'clippy'):
            call = next(v for v in calls if isinstance(v.func, ast.Attribute) and v.func.attr == 'phase'
                        and v.args and isinstance(v.args[0], ast.Constant) and v.args[0].value == phase)
            constants = [v.value for v in ast.walk(call.args[1]) if isinstance(v, ast.Constant)]
            self.assertIn('worldwright-server', constants)
        self.assertTrue(any(isinstance(v.func, ast.Name) and v.func.id == 'suite' and v.args
                            and isinstance(v.args[0], ast.Constant) and v.args[0].value == 'server-r6' for v in calls))

    def retained_fixture(self):
        source = self.root / 'cache' / 'app.exe'
        source.parent.mkdir()
        source.write_bytes(b'original executable fixture')
        output = self.root / 'retained'; output.mkdir()
        records = subjects.retain_binaries({'app': source}, output)
        return source, output, records

    def test_retained_copy_survives_cache_rebuild(self):
        source, output, records = self.retained_fixture()
        copy = Path(records['app']['path'])
        self.assertFalse(source.samefile(copy))
        self.assertEqual(copy.stat().st_nlink, 1)
        source.write_bytes(b'rebuilt cache fixture')
        subjects.verify_retained(output, records, check_local=True)
        self.assertFalse(subjects.cache_observations(records)['app']['matches_retained'])
        self.assertEqual(copy.read_bytes(), b'original executable fixture')

    def test_retained_copy_tamper_is_rejected(self):
        _, output, records = self.retained_fixture()
        Path(records['app']['path']).write_bytes(b'changed subject')
        with self.assertRaisesRegex(ValueError, 'retained binary changed'):
            subjects.verify_retained(output, records)

    def test_retention_never_overwrites_existing_subject_directory(self):
        source, output, records = self.retained_fixture()
        before = Path(records['app']['path']).read_bytes()
        with self.assertRaises(ValueError): subjects.retain_binaries({'app': source}, output)
        self.assertEqual(Path(records['app']['path']).read_bytes(), before)

    def test_source_change_during_retention_fails(self):
        source = self.root / 'app.exe'; source.write_bytes(b'first')
        output = self.root / 'retained'; output.mkdir()
        original_new_file = subjects.new_file
        def concurrent_change(path, data):
            original_new_file(path, data)
            source.write_bytes(b'second')
        with patch.object(subjects, 'new_file', side_effect=concurrent_change):
            with self.assertRaisesRegex(ValueError, 'changed during retention'):
                subjects.retain_binaries({'app': source}, output)

    def test_retained_path_escape_is_rejected(self):
        _, output, records = self.retained_fixture()
        records['app']['retained_relative_path'] = '../app.exe'
        with self.assertRaises(ValueError): subjects.verify_retained(output, records)

    def test_hardlinked_retained_subject_is_rejected(self):
        _, output, records = self.retained_fixture()
        os.link(records['app']['path'], self.root / 'alias.exe')
        with self.assertRaisesRegex(ValueError, 'linked or nonregular'):
            subjects.verify_retained(output, records)

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
