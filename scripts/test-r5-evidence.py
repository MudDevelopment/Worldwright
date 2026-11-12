"""Synthetic verifier regressions only; never application acceptance."""
from pathlib import Path
from contextlib import redirect_stdout
import importlib.util
import io
import json
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('r5_evidence', Path(__file__).with_name('verify-r5-evidence.py'))
verifier = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verifier)


class EvidenceTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='r5-evidence-fixture-')
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        (self.root / 'source.rs').write_bytes(b'fn main() {}\n')
        (self.root / 'log.txt').write_bytes(b'fixture only\n')
        self.report = {'schema_version': 1, 'track': 'R5', 'native_rust_executed': False,
                       'full_r5_closed': False, 'subjects': [{'path': 'source.rs', 'sha256_lf_text': verifier.digest(b'fn main() {}\n')}],
                       'log': {'path': 'log.txt', 'sha256': verifier.digest(b'fixture only\n')}}

    def check(self):
        (self.root / 'report.json').write_text(json.dumps(self.report), encoding='utf-8')
        with redirect_stdout(io.StringIO()):
            return verifier.verify(self.root, 'report.json')

    def test_matches_and_does_not_mutate_files(self):
        self.assertTrue(self.check())
        before = {p.name: p.read_bytes() for p in self.root.iterdir()}
        self.assertTrue(self.check())
        self.assertEqual(before, {p.name: p.read_bytes() for p in self.root.iterdir()})

    def test_source_crlf_normalizes_but_changed_content_fails(self):
        (self.root / 'source.rs').write_bytes(b'fn main() {}\r\n')
        self.assertTrue(self.check())
        (self.root / 'source.rs').write_bytes(b'fn main() {panic!()}\n')
        self.assertFalse(self.check())

    def test_log_is_byte_exact_even_when_only_newline_changes(self):
        (self.root / 'log.txt').write_bytes(b'fixture only\r\n')
        self.assertFalse(self.check())
        (self.root / 'log.txt').write_bytes(b'fixture only\n\n')
        self.assertFalse(self.check())

    def test_missing_file_and_escape_fail(self):
        for name in ('missing.rs', '../outside', '/absolute', 'C:/outside', 'a\\b'):
            self.report['subjects'][0]['path'] = name
            with self.subTest(name=name), self.assertRaises((OSError, ValueError)):
                self.check()

    def test_duplicate_inventory_and_invalid_digest_fail(self):
        self.report['subjects'] *= 2
        with self.assertRaises(ValueError): self.check()
        self.report['subjects'] = self.report['subjects'][:1]
        self.report['subjects'][0]['sha256_lf_text'] = 'not-a-hash'
        with self.assertRaises(ValueError): self.check()

    def test_wrong_profile_and_closure_claim_fail(self):
        for field, value in (('schema_version', True), ('native_rust_executed', True), ('full_r5_closed', True)):
            old = self.report[field]
            self.report[field] = value
            with self.subTest(field=field), self.assertRaises(ValueError): self.check()
            self.report[field] = old

    def test_duplicate_json_keys_fail(self):
        (self.root / 'report.json').write_text('{"schema_version":1,"schema_version":1}', encoding='utf-8')
        with self.assertRaises(ValueError): verifier.verify(self.root, 'report.json')


if __name__ == '__main__':
    unittest.main(verbosity=2)
