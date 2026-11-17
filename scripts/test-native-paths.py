"""Sixteen native path/import regressions. No real Rust build or release approval."""
from pathlib import Path
import importlib.util
import os
import tempfile
import tomllib
import unittest
from unittest.mock import patch
import native_paths as native
import worldwright_local as local

spec = importlib.util.spec_from_file_location('native_storage_cli', Path(__file__).with_name('native-storage.py'))
cli = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cli)


class NativePathTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='native-path-tests-', dir=local.LOCAL / 'temp')
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)

    def test_existing_entry_accepts_native_namespace_spelling(self):
        path = self.root / 'cargo.exe'; path.write_bytes(b'fixture')
        observed = '\\\\?\\' + str(path) if os.name == 'nt' else str(path)
        self.assertTrue(native.same_local_entry(observed, path))
        self.assertEqual(native.existing_local_path(observed), path.resolve())

    def test_identical_bytes_in_distinct_files_are_not_same_entry(self):
        first = self.root / 'first.exe'; second = self.root / 'second.exe'
        first.write_bytes(b'identical'); second.write_bytes(b'identical')
        self.assertFalse(native.same_local_entry(first, second))

    def test_outside_runtime_root_is_refused_even_with_prefix(self):
        outside = local.ROOT / 'Cargo.toml'
        observed = '\\\\?\\' + str(outside) if os.name == 'nt' else str(outside)
        with self.assertRaises(ValueError): native.existing_local_path(observed)

    def test_ambiguous_namespace_forms_are_refused(self):
        self.assertEqual(native.drive_path_spelling('\\\\?\\E:\\safe\\cargo.exe'), 'E:\\safe\\cargo.exe')
        for raw in ('\\\\?\\UNC\\server\\share\\cargo.exe', '\\\\?\\GLOBALROOT\\Device\\file',
                    '\\\\.\\pipe\\name', '\\\\?\\E:\\safe\\..\\cargo.exe',
                    '\\\\?\\E:\\safe\\cargo.exe:stream', '\\\\?\\E:\\safe\\cargo.exe '):
            with self.subTest(raw=raw), self.assertRaises(ValueError): native.drive_path_spelling(raw)

    def test_host_is_explicit(self):
        self.assertIn(native.host(), ('x86_64-pc-windows-msvc', 'x86_64-unknown-linux-gnu', 'aarch64-unknown-linux-gnu'))

    def test_unsupported_host_refused(self):
        with patch.object(native.platform, 'machine', return_value='unsupported'), self.assertRaises(ValueError):
            native.host()

    def test_checked_in_aliases_are_accepted(self):
        value = tomllib.loads((local.ROOT / '.cargo/config.toml').read_text(encoding='utf-8'))
        native.validate_configuration(value)

    def test_target_or_wrapper_config_refused(self):
        for value in ({'build': {'target-dir': '../escape'}}, {'env': {'TEMP': '/outside'}},
                      {'target': {'x86_64-pc-windows-msvc': {'runner': 'external'}}}):
            with self.subTest(value=value), self.assertRaises(ValueError): native.validate_configuration(value)

    def test_command_shadowing_alias_refused(self):
        with self.assertRaises(ValueError): native.validate_configuration({'alias': {'fmt': '!external-command'}})

    def test_compiler_and_cargo_overrides_refused(self):
        for key in ('RUSTC', 'RUSTC_WRAPPER', 'RUSTDOCFLAGS', 'CARGO_BUILD_RUSTC', 'CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUNNER'):
            with self.subTest(key=key), self.assertRaises(ValueError): native.reject_overrides({key: 'outside'})

    def test_reviewed_process_controls_accepted(self):
        native.reject_overrides({'CARGO_HOME': 'will-be-replaced', 'CARGO_TARGET_DIR': 'will-be-replaced',
                                 'CARGO_BUILD_TARGET': 'will-be-replaced', 'CARGO_NET_OFFLINE': 'true'})

    def test_pinned_toolchain_path_is_local(self):
        path = native.toolchain_path()
        self.assertTrue(path.is_relative_to(local.LOCAL.resolve()))
        self.assertEqual(path.name, native.PIN + '-' + native.host())

    def test_cache_copy_preserves_source_and_bytes(self):
        source = self.root / 'source'; source.mkdir()
        (source / 'cache').write_bytes(b'cached-content\r\n')
        result = cli.copy_cached_tree(source, self.root / 'destination')
        self.assertEqual(result['status'], 'copied-and-byte-checked')
        self.assertEqual(result['files'], 1)
        self.assertEqual((source / 'cache').read_bytes(), b'cached-content\r\n')
        self.assertEqual((self.root / 'destination/cache').read_bytes(), b'cached-content\r\n')

    def test_cache_copy_refuses_overlap(self):
        source = self.root / 'source'; source.mkdir()
        with self.assertRaises(ValueError): cli.copy_cached_tree(source, source / 'destination')

    def test_existing_destination_not_overwritten(self):
        source = self.root / 'source'; source.mkdir(); (source / 'cache').write_bytes(b'new')
        destination = self.root / 'destination'; destination.mkdir(); (destination / 'cache').write_bytes(b'old')
        result = cli.copy_cached_tree(source, destination)
        self.assertEqual(result['status'], 'existing-preserved-not-recopied')
        self.assertEqual((destination / 'cache').read_bytes(), b'old')

    def test_corrupt_copy_retains_staging_without_promotion(self):
        source = self.root / 'source'; source.mkdir(); (source / 'cache').write_bytes(b'expected')
        destination = self.root / 'destination'
        def corrupt(src, dest): Path(dest).write_bytes(b'wrong')
        with patch.object(cli.shutil, 'copy2', side_effect=corrupt), self.assertRaises(ValueError):
            cli.copy_cached_tree(source, destination)
        self.assertFalse(destination.exists())
        self.assertEqual((source / 'cache').read_bytes(), b'expected')
        self.assertTrue(list(self.root.glob('.importing-*')))


if __name__ == '__main__':
    local.configure_tools_environment()
    unittest.main(verbosity=2)
