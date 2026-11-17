"""Eight repository-local path/helper regressions; no native acceptance."""
from pathlib import Path
import json
import stat
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import worldwright_local as local


class LocalStorageTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='worldwright-path-test-', dir=local.LOCAL / 'temp')
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)

    def test_new_local_output_allowed(self):
        path = self.root / 'output'
        self.assertEqual(local.new_output(path), path.resolve())
        self.assertFalse(path.exists())

    def test_source_and_external_paths_rejected_without_writes(self):
        for path in (local.ROOT / 'unapproved-output', local.ROOT.parent / 'unapproved-output', local.LOCAL):
            with self.subTest(path=path), self.assertRaises(ValueError):
                local.local_path(path)

    def test_sibling_prefix_is_not_containment(self):
        path = local.ROOT / '.worldwright-local-other' / 'output'
        with self.assertRaises(ValueError): local.local_path(path)

    def test_existing_output_refused(self):
        path = self.root / 'output'; path.mkdir()
        with self.assertRaises(ValueError): local.new_output(path)

    def test_input_output_overlap_refused(self):
        candidate = self.root / 'candidate'; candidate.mkdir()
        with self.assertRaises(ValueError): local.new_output(candidate / 'output', (candidate,))
        with self.assertRaises(ValueError): local.new_output(self.root / 'future', (self.root / 'future' / 'candidate',))

    def test_parent_traversal_refused(self):
        with self.assertRaises(ValueError): local.local_path(local.LOCAL / '..' / 'unapproved-output')

    def test_windows_reparse_metadata_refused(self):
        info = SimpleNamespace(st_mode=stat.S_IFDIR | 0o700, st_file_attributes=0x400)
        with patch.object(Path, 'lstat', return_value=info), self.assertRaises(ValueError):
            local.reject_links(self.root / 'junction')

    def test_child_tempfile_uses_repository(self):
        code = 'import json,tempfile; f=tempfile.TemporaryFile(); print(json.dumps({"temp":tempfile.gettempdir()})); f.close()'
        result = subprocess.run([sys.executable, '-B', '-c', code], cwd=local.ROOT,
                                capture_output=True, check=True, timeout=15)
        value = json.loads(result.stdout)
        self.assertEqual(Path(value['temp']).resolve(), (local.LOCAL / 'temp').resolve())


if __name__ == '__main__':
    local.configure_tools_environment()
    unittest.main(verbosity=2)
