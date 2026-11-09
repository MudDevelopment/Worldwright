"""Private Ubuntu desktop package; Python 3.11+. Hashes are integrity, not approval.
Build from passing same-source R0 evidence. Verify installed files before launch.
"""
from pathlib import Path
import argparse
import gzip
import hashlib
import importlib.util
import io
import json
import os
import re
import stat
import subprocess
import sys
import tarfile
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
MAX_FILE = 256 * 1024 * 1024
MAX_TOTAL = 768 * 1024 * 1024


def sha(path: Path) -> str:
    value = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(65536), b''): value.update(chunk)
    return value.hexdigest()


def verify(root: Path) -> dict:
    root = root.absolute()
    if root.is_symlink() or not root.is_dir(): raise ValueError('real package directory required')
    if sys.platform.startswith('linux') and (root.stat().st_uid != os.geteuid() or stat.S_IMODE(root.stat().st_mode) & 0o077):
        raise ValueError('package directory must be owned and private (0700)')
    manifest_path = root / 'package-manifest.json'
    if manifest_path.is_symlink() or manifest_path.stat().st_size > 65536: raise ValueError('invalid manifest')
    manifest = json.loads(manifest_path.read_text())
    if not isinstance(manifest, dict) or type(manifest.get('schema_version')) is not int or manifest['schema_version'] != 1 or manifest.get('distribution') != 'private-acceptance-only': raise ValueError('unsupported package')
    files = manifest.get('files')
    if not isinstance(files, list) or not 1 <= len(files) <= 32: raise ValueError('file inventory bound')
    expected = {'package-manifest.json'}
    total = 0
    for record in files:
        if not isinstance(record, dict) or not isinstance(record.get('path'), str) or not record['path']: raise ValueError('invalid file record')
        if type(record.get('bytes')) is not int or not 0 <= record['bytes'] <= MAX_FILE: raise ValueError('invalid byte count')
        if type(record.get('mode')) is not int or not 0 <= record['mode'] <= 0o777: raise ValueError('invalid file mode')
        if not isinstance(record.get('sha256'), str) or not re.fullmatch(r'[a-f0-9]{64}', record['sha256']): raise ValueError('invalid file digest')
        relative = Path(record['path'])
        if relative.is_absolute() or '..' in relative.parts or str(relative) in expected: raise ValueError('unsafe/duplicate path')
        path = root / relative
        for parent in (path, *path.parents):
            if parent == root.parent: break
            if parent.is_symlink(): raise ValueError('package symlink rejected')
        if not path.is_file() or path.stat().st_size > MAX_FILE: raise ValueError('invalid package file')
        total += path.stat().st_size
        if total > MAX_TOTAL or path.stat().st_size != record['bytes'] or sha(path) != record['sha256']: raise ValueError('package integrity mismatch')
        if stat.S_IMODE(path.stat().st_mode) != record['mode']: raise ValueError('package mode mismatch')
        expected.add(str(relative))
    actual = set()
    for path in root.rglob('*'):
        if path.is_symlink(): raise ValueError('unexpected symlink')
        if path.is_file(): actual.add(path.relative_to(root).as_posix())
    if actual != expected: raise ValueError('unexpected/missing package files')
    return manifest


def build(report_path: Path, output: Path) -> None:
    if not sys.platform.startswith('linux'): raise ValueError('Ubuntu/Linux package build required')
    check_spec = importlib.util.spec_from_file_location('r0_closure', ROOT/'scripts/r0-r1-close.py')
    checker = importlib.util.module_from_spec(check_spec); check_spec.loader.exec_module(checker)
    report = checker.check_r0(report_path, 'Linux')
    required = {'fmt', 'check', 'clippy', 'test', 'build', 'rustdoc', 'receipt-pg', 'required-helm', 'real-first-run'}
    passed = {phase['name'] for phase in report.get('phases', []) if phase.get('status') == 'passed' and phase.get('exit_code') == 0}
    if report.get('status') != 'passed' or not required <= passed or '-linux-' not in report.get('host_target', ''):
        raise ValueError('passing Linux R0 report with all required gates needed')
    spec = importlib.util.spec_from_file_location('r0_verify', ROOT/'scripts/r0-verify.py')
    module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
    if module.source_identity() != report.get('source'): raise ValueError('source/lock differs from R0')
    version = tomllib.loads((ROOT/'Cargo.toml').read_text())['workspace']['package']['version']
    output = output.resolve()
    if output.exists() or output.is_relative_to(ROOT): raise ValueError('new package path outside checkout required')
    os.umask(0o077)
    with tempfile.TemporaryDirectory(prefix='worldwright-package-') as folder:
        package = Path(folder) / f'worldwright-private-{version}-{report["host_target"]}'
        package.mkdir(mode=0o700)
        records = []
        def add(relative: str, source: Path | bytes, mode: int) -> None:
            target = package / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            if isinstance(source, Path):
                if source.is_symlink() or not source.is_file() or source.stat().st_size > MAX_FILE: raise ValueError('invalid input')
                import shutil
                shutil.copyfile(source, target)
            else: target.write_bytes(source)
            target.chmod(mode)
            records.append({'path': relative, 'bytes': target.stat().st_size, 'sha256': sha(target), 'mode': mode})
        dependencies = {}
        for name in ('worldwright', 'worldwright-studio'):
            binary = Path(report['binaries'][name]['path'])
            if sha(binary) != report['binaries'][name]['sha256']: raise ValueError('binary differs from R0')
            with binary.open('rb') as stream:
                if stream.read(4) != b'\x7fELF': raise ValueError('native ELF required')
            result = subprocess.run(['ldd', str(binary)], capture_output=True, text=True, timeout=10)
            if result.returncode or 'not found' in result.stdout: raise ValueError('unresolved runtime libraries')
            dependencies[name] = result.stdout
            add('bin/'+name, binary, 0o755)
        for name in ('desktop.sh', 'cleanup-session.py', 'package-desktop.py'):
            add('scripts/'+name, ROOT/'scripts'/name, 0o644)
        add('bin/worldwright-desktop', b'#!/usr/bin/env bash\nset -euo pipefail\nROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"\nexec bash "$ROOT/scripts/desktop.sh" --installed "$@"\n', 0o755)
        add('README.txt', b'Private acceptance package; no license grant or supported-release approval.\nRun bin/worldwright-desktop as your ordinary Ubuntu desktop user.\nRequires Python 3.11+, Bash, GNU coreutils, util-linux setsid, glibc/libgcc libraries recorded in the manifest, and a browser. xdg-utils is optional for opening a browser.\nRust/Cargo and the original checkout are not required for Studio editing/preview. Game source compilation still requires a development toolchain.\nKeep this directory private (0700); do not provide operator secrets.\nEmbedded Studio assets are compiled into worldwright-studio.\n', 0o644)
        manifest = {'schema_version': 1, 'distribution': 'private-acceptance-only', 'package_version': version, 'host_target': report['host_target'], 'source_sha256': report['source']['source_sha256'], 'lock_sha256': report['source']['lock_sha256'], 'r0_report_sha256': sha(report_path), 'build_profile': 'debug', 'runtime_libraries': dependencies, 'files': sorted(records, key=lambda row: row['path'])}
        (package/'package-manifest.json').write_text(json.dumps(manifest, indent=2)+'\n')
        (package/'package-manifest.json').chmod(0o644)
        verify(package)
        raw = output.open('xb')
        try:
            with raw, gzip.GzipFile(filename='', fileobj=raw, mode='wb', mtime=0) as zipped, tarfile.open(fileobj=zipped, mode='w') as archive:
                for path in [package, *sorted(package.rglob('*'))]:
                    info = archive.gettarinfo(str(path), arcname=str(path.relative_to(package.parent)))
                    info.uid = info.gid = info.mtime = 0; info.uname = info.gname = ''
                    if path.is_file():
                        with path.open('rb') as stream: archive.addfile(info, stream)
                    else: archive.addfile(info)
        except Exception:
            output.unlink(missing_ok=True)
            raise
    print(f'Private package: {output}; sha256={sha(output)}; R1 acceptance still required')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='task', required=True)
    make = sub.add_parser('build'); make.add_argument('--r0-report', type=Path, required=True); make.add_argument('--output', type=Path, required=True)
    check = sub.add_parser('verify'); check.add_argument('--directory', type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.task == 'build': build(args.r0_report, args.output)
        else: verify(args.directory); print('Package integrity verified; not authorship or acceptance')
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr); raise SystemExit(1)
