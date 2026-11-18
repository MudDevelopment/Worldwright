"""Prepare cached native tooling and run repository-local preflight.
prepare copies existing caches; check runs offline probes; format explicitly
backs up Git-visible Rust sources and applies/checks the pinned formatter.
lint runs a focused format/Clippy diagnostic using the native gate's exact scope.
No command downloads tools or grants release approval.
"""
from pathlib import Path
import argparse
import hashlib
import json
import os
import shutil
import stat
import sys
import time
import uuid
import native_paths as native
import worldwright_local as local
from r6_support import Collector, encode, new_file, require, sha, read, source_identity


def copy_cached_tree(source, destination):
    source = local.reject_links(source).resolve(strict=True)
    destination = local.local_path(destination)
    require(source.is_dir(), 'source cache/toolchain directory required: ' + str(source))
    require(not source.is_relative_to(destination) and not destination.is_relative_to(source), 'source/destination copy overlap')
    if destination.exists():
        require(destination.is_dir(), 'existing cache destination is not a directory')
        return {'source': str(source), 'destination': str(destination),
                'status': 'existing-preserved-not-recopied'}
    destination.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    staging = local.local_path(destination.parent / ('.importing-' + destination.name + '-' + uuid.uuid4().hex))
    staging.mkdir(mode=0o700)
    digest = hashlib.sha256(); count = 0; total = 0
    print('Copying cached tree: ' + str(source) + ' -> ' + str(destination), flush=True)
    # No merge/overwrite. Failed staging is preserved for diagnosis.
    for directory, directories, names in os.walk(source, followlinks=False):
        directories.sort()
        relative_dir = Path(directory).relative_to(source)
        for name in sorted(directories):
            entry = local.reject_links(Path(directory) / name)
            require(entry.is_dir(), 'non-directory in source walk')
            (staging / relative_dir / name).mkdir(mode=0o700)
        for name in sorted(names):
            entry = local.reject_links(Path(directory) / name)
            before = entry.stat()
            require(stat.S_ISREG(before.st_mode), 'nonregular source file refused')
            count += 1; total += before.st_size
            require(count <= 250000 and total <= 32 * 1024**3, 'copy inventory exceeds 250000 files/32 GiB')
            dest = staging / relative_dir / name
            # Verify copied bytes; preserve source metadata and original files.
            source_hash = sha(entry)
            shutil.copy2(entry, dest)
            after = entry.stat()
            require((before.st_size, before.st_mtime_ns) == (after.st_size, after.st_mtime_ns)
                    and sha(dest) == source_hash, 'source changed or copied bytes differ')
            item = {'path': entry.relative_to(source).as_posix(), 'bytes': before.st_size, 'sha256': source_hash}
            digest.update(encode(item))
            if count % 1000 == 0: print('Copied and checked ' + str(count) + ' files', flush=True)
    require(not destination.exists(), 'destination appeared during import; staging preserved')
    staging.rename(destination)
    return {'source': str(source), 'destination': str(destination), 'status': 'copied-and-byte-checked',
            'files': count, 'bytes': total, 'copied_inventory_sha256': digest.hexdigest(),
            'provenance': 'operator existing cache, not independently authenticated vendor distribution'}


def prepare(collector, source_cargo, source_rustup):
    destination = native.toolchain_path()
    if not destination.exists():
        source_chain = source_rustup / 'toolchains' / (native.PIN + '-' + native.host())
        require(source_chain.is_dir(), 'cached pinned toolchain missing: ' + str(source_chain) + '; no automatic download is allowed')
        collector.report['toolchain_import'] = copy_cached_tree(source_chain, destination)
    else:
        collector.report['toolchain_import'] = {'destination': str(destination), 'status': 'existing-preserved-not-recopied'}
    collector.report['cache_imports'] = []
    for name in ('registry', 'git'):
        destination = local.LOCAL / 'cargo-home' / name
        source = source_cargo / name
        if destination.exists():
            result = {'destination': str(destination), 'status': 'existing-preserved-not-recopied'}
        elif source.is_dir():
            result = copy_cached_tree(source, destination)
        else:
            result = {'source': str(source), 'status': 'source-absent-offline-check-will-decide'}
        collector.report['cache_imports'].append(result)
    collector.report['not_copied'] = ['credentials', 'Cargo config files', 'rustup settings/overrides', 'old target directories']
    collector.report['native_paths'] = native.configure_native()
    collector.report['status'] = 'native-storage-prepared-preflight-required'


def preflight(collector):
    import re
    tests = collector.phase('native-path-regressions', [sys.executable, '-B', str(local.ROOT / 'scripts/test-native-paths.py')]).decode('utf-8').replace('\r\n', '\n')
    require(re.findall(r'^Ran (\d+) tests? in ', tests, re.M) == ['16'] and '\nOK\n' in tests and '(skipped=' not in tests, 'native path regression inventory differs')
    paths = native.configure_native()
    collector.report['native_paths'] = paths
    for name in ('cargo', 'rustc'):
        output = collector.phase(name + '-version', [paths['binaries'][name]['path'], '--version']).decode()
        require(output.split()[1] == native.PIN, 'wrong local ' + name + ' version')
    details = collector.phase('rustc-host', [paths['binaries']['rustc']['path'], '-vV']).decode()
    require('host: ' + paths['host'] in details.splitlines(), 'native compiler host mismatch')
    sysroot = collector.phase('sysroot', [paths['binaries']['rustc']['path'], '--print', 'sysroot']).decode().strip()
    require(native.same_local_entry(sysroot, native.toolchain_path()), 'rustc sysroot escapes local toolchain')
    for name in ('git', 'node'):
        require(shutil.which(name), 'missing prerequisite: ' + name)
    node = collector.phase('node-version', ['node', '--version']).decode().strip()
    require(int(node.lstrip('v').split('.')[0]) >= 20, 'Node 20+ required')
    for name in ('rustfmt', 'cargo-clippy'):
        collector.phase(name + '-version', [paths['binaries'][name]['path'], '--version'])
    cargo = paths['binaries']['cargo']['path']
    raw = collector.phase('offline-metadata', [cargo, 'metadata', '--format-version', '1', '--locked', '--offline', '--filter-platform', paths['host']], 180)
    metadata = json.loads(raw)
    require(native.same_local_entry(metadata['target_directory'], paths['target']), 'Cargo target-directory override')
    for package in metadata['packages']:
        manifest = local.reject_links(package['manifest_path']).resolve()
        require(manifest.is_relative_to(local.ROOT.resolve()), 'package manifest outside repository: ' + str(manifest))
    probe = collector.output / 'probe'; probe.mkdir()
    (probe / 'src').mkdir()
    new_file(probe / 'Cargo.toml', b'[workspace]\n[package]\nname = "worldwright-native-path-probe"\nversion = "0.0.0"\nedition = "2024"\npublish = false\n')
    new_file(probe / 'src/main.rs', b'fn main() {\n    println!("WW_TEMP={}", std::env::temp_dir().display());\n    println!("WW_CARGO={}", env!("CARGO"));\n    println!("WW_EXE={}", std::env::current_exe().unwrap().display());\n}\n')
    collector.phase('probe-lock', [cargo, 'generate-lockfile', '--offline', '--manifest-path', str(probe / 'Cargo.toml')])
    result = collector.phase('native-temp-probe', [cargo, 'run', '--quiet', '--locked', '--offline',
        '--manifest-path', str(probe / 'Cargo.toml'), '--target', paths['host'], '--target-dir', str(probe / 'target')], 300).decode()
    observed = dict(line.split('=', 1) for line in result.splitlines() if line.startswith(('WW_TEMP=', 'WW_CARGO=', 'WW_EXE=')))
    collector.report['observed_native_paths'] = observed
    require(set(observed) == {'WW_TEMP', 'WW_CARGO', 'WW_EXE'}, 'incomplete native path probe')
    require(native.same_local_entry(observed['WW_TEMP'], local.LOCAL / 'temp'), 'native temp path escapes')
    require(native.same_local_entry(observed['WW_CARGO'], cargo), 'nested compile embeds wrong Cargo file: observed=' + observed['WW_CARGO'] + '; expected=' + cargo)
    native.existing_local_path(observed['WW_EXE'])
    collector.report['embedded_cargo_identity'] = {'method': 'local-containment-plus-filesystem-samefile',
                                                'observed': observed['WW_CARGO'], 'expected': cargo, 'matched': True}
    if os.name == 'nt':
        shell = Path(os.environ['SystemRoot']) / 'System32/WindowsPowerShell/v1.0/powershell.exe'
        text = collector.phase('powershell-temp-probe', [str(shell), '-NoLogo', '-NoProfile', '-NonInteractive', '-Command', '[IO.Path]::GetTempPath()']).decode().strip()
        require(native.same_local_entry(text, local.LOCAL / 'temp'), 'PowerShell temp path escapes')
    browser_code = 'from playwright.sync_api import sync_playwright\nwith sync_playwright() as p:\n    b=p.chromium.launch(headless=True)\n    print("CHROMIUM="+b.version)\n    b.close()\n'
    collector.phase('chromium-launch-probe', [sys.executable, '-B', '-c', browser_code], 90)
    require(native.configuration_inventory() == paths['cargo_configurations'], 'Cargo config changed during preflight')
    require(all(sha(Path(item['path'])) == item['sha256'] for item in paths['binaries'].values()), 'native tools changed during preflight')
    collector.report['status'] = 'native-storage-preflight-passed-not-acceptance'


def format_source(collector):
    tools = native.configure_native()
    collector.report['native_paths'] = tools
    before = source_identity()
    new_file(collector.output / 'source-before.json', encode(before))
    rust = [item['path'] for item in before['inventory'] if Path(item['path']).suffix == '.rs']
    require(0 < len(rust) <= 800, 'Rust backup inventory must contain 1..800 files')
    # Snapshot exact current bytes, including uncommitted changes, before applying.
    total = 0
    for name in rust:
        path = local.reject_links(local.ROOT / name)
        require(path.resolve().is_relative_to(local.ROOT.resolve()), 'source escapes repository')
        data = read(path); total += len(data)
        require(total <= 64 * 1024 * 1024, 'Rust source backup exceeds 64 MiB')
        backup = collector.output / 'rust-source-before' / name
        backup.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        new_file(backup, data)
    protected = {item['path']: sha(local.ROOT / item['path']) for item in before['inventory'] if item['path'] not in rust}
    collector.report['rust_backup_files'] = len(rust)
    collector.report['rust_backup_bytes'] = total
    require(source_identity()['source_sha256'] == before['source_sha256'], 'source changed while preparing formatter backup; no formatting started')
    cargo = tools['binaries']['cargo']['path']
    try:
        collector.phase('format-apply', [cargo, 'fmt', '--all'], 300)
        collector.phase('format-check', [cargo, 'fmt', '--all', '--', '--check'], 300)
    finally:
        after = source_identity()
        new_file(collector.output / 'source-after.json', encode(after))
        old = {item['path']: item['sha256'] for item in before['inventory']}
        new = {item['path']: item['sha256'] for item in after['inventory']}
        collector.report['changed_sources'] = sorted(name for name in old.keys() | new.keys() if old.get(name) != new.get(name))
        collector.report['source_before_sha256'] = before['source_sha256']
        collector.report['source_after_sha256'] = after['source_sha256']
        require(old.keys() == new.keys(), 'source inventory changed during formatting; inspect backup/report')
        require(all(sha(local.ROOT / name) == value for name, value in protected.items()), 'non-Rust source changed during formatting; inspect report, no automatic rollback')
    require(all(sha(Path(item['path'])) == item['sha256'] for item in tools['binaries'].values()), 'tool changed during formatting')
    collector.report['status'] = 'source-format-passed-native-not-run'


def lint_source(collector):
    tools = native.configure_native()
    collector.report['native_paths'] = tools
    collector.report['scope'] = 'format-and-native-clippy-diagnostic-only'
    before = source_identity()
    new_file(collector.output / 'source-before.json', encode(before))
    cargo = tools['binaries']['cargo']['path']
    common = ['--locked', '--offline', '--target', tools['host'], '--target-dir', tools['target']]
    try:
        collector.phase('format-check', [cargo, 'fmt', '--all', '--', '--check'], 180)
        collector.phase('clippy', [cargo, 'clippy', '-p', 'worldwright-studio', '-p', 'worldwright-server',
            '-p', 'worldwright-operations', '-p', 'worldwright-console', '-p', 'worldwright-release',
            '--all-targets', '--all-features', *common, '--', '-D', 'warnings'], 1800)
    finally:
        after = source_identity()
        new_file(collector.output / 'source-after.json', encode(after))
        collector.report['source_sha256'] = before['source_sha256']
        collector.report['source_unchanged'] = before == after
        require(before == after, 'source changed during lint diagnostic')
    require(native.configuration_inventory() == tools['cargo_configurations'], 'Cargo config changed during lint')
    require(all(sha(Path(item['path'])) == item['sha256'] for item in tools['binaries'].values()), 'native tool changed during lint')
    collector.report['status'] = 'native-lint-passed-not-acceptance'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=('prepare', 'check', 'format', 'lint'))
    parser.add_argument('--source-cargo-home', type=Path)
    parser.add_argument('--source-rustup-home', type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--confirm-format-source', action='store_true')
    args = parser.parse_args()
    if args.confirm_format_source != (args.operation == 'format'):
        parser.error('format requires --confirm-format-source; that flag is invalid for other operations')
    # Capture source locations BEFORE redirecting process/child homes.
    home = Path.home()
    source_cargo = (args.source_cargo_home or Path(os.environ.get('CARGO_HOME', str(home / '.cargo')))).absolute()
    source_rustup = (args.source_rustup_home or Path(os.environ.get('RUSTUP_HOME', str(home / '.rustup')))).absolute()
    local.local_path(Path(sys.prefix))
    local.configure_tools_environment()
    output = args.output or local.LOCAL / 'evidence' / ('native-storage-' + args.operation + '-' + uuid.uuid4().hex)
    collector = Collector(output, 'native-storage-' + args.operation)
    print('Native storage evidence: ' + str(collector.output), flush=True)
    lock = local.LOCAL / 'native-storage.lock'
    acquired = False
    try:
        new_file(lock, encode({'pid': os.getpid(), 'operation': args.operation, 'report': str(collector.output)}))
        acquired = True
        if args.operation == 'prepare': prepare(collector, source_cargo, source_rustup)
        elif args.operation == 'format': format_source(collector)
        elif args.operation == 'lint': lint_source(collector)
        else: preflight(collector)
    except (Exception, KeyboardInterrupt) as error:
        collector.fail(error)
        print(collector.report['failure'], flush=True)
    finally:
        if acquired: lock.unlink()
        collector.finish()
    return 0 if collector.report['status'] in ('native-storage-prepared-preflight-required', 'native-storage-preflight-passed-not-acceptance', 'source-format-passed-native-not-run', 'native-lint-passed-not-acceptance') else 1


if __name__ == '__main__':
    try: raise SystemExit(main())
    except (OSError, ValueError) as error:
        print('Native storage setup failed:', str(error), file=sys.stderr)
        raise SystemExit(1)
