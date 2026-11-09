"""R0 native/integration evidence; Python 3.11+, no successful skips.
Run in the real source tree with approved tools/cache and disposable DB env.
"""
from pathlib import Path
import argparse
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
import time
import tomllib

ROOT = Path(__file__).resolve().parents[1]
OVERRIDES = ('CARGO_BUILD_TARGET', 'CARGO_TARGET_DIR', 'RUSTUP_TOOLCHAIN', 'RUSTC', 'RUSTDOC', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTDOCFLAGS', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'RUSTC_BOOTSTRAP')


def digest(path: Path) -> str:
    value = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(65536), b''):
            value.update(chunk)
    return value.hexdigest()


def canonical_sha(path: Path) -> str:
    text_suffixes = {'.rs', '.toml', '.lock', '.md', '.txt', '.json', '.yaml', '.yml', '.sh', '.ps1', '.py', '.cjs', '.js', '.css', '.html'}
    if path.suffix.lower() in text_suffixes or path.name in {'.gitignore', '.gitattributes'}:
        if path.stat().st_size > 16 * 1024 * 1024: raise ValueError('text inventory file bound exceeded')
        return hashlib.sha256(path.read_bytes().replace(b'\r\n', b'\n')).hexdigest()
    return digest(path)


def source_identity() -> dict:
    def git(*args: str) -> bytes:
        return subprocess.check_output(['git', *args], cwd=ROOT)
    paths = sorted(set(git('ls-files', '-z', '--cached', '--others', '--exclude-standard').split(b'\0')) - {b''})
    if len(paths) > 10000:
        raise ValueError('source inventory bound exceeded')
    inventory = []
    for raw in paths:
        relative = os.fsdecode(raw)
        path = ROOT / relative
        if path.is_symlink() or not path.is_file():
            raise ValueError(f'nonregular/missing inventoried source: {relative}')
        inventory.append({'path': relative, 'sha256_lf_text': canonical_sha(path)})
    encoded = json.dumps(inventory, sort_keys=True).encode()
    return {'commit': git('rev-parse', 'HEAD').decode().strip(),
            'dirty': bool(git('status', '--porcelain')),
            'source_sha256': hashlib.sha256(encoded).hexdigest(),
            'lock_sha256': canonical_sha(ROOT / 'Cargo.lock'), 'inventory': inventory}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--allow-dirty', action='store_true')
    args = parser.parse_args()
    output = args.output.resolve()
    if output.exists() or output.is_symlink() or output.is_relative_to(ROOT):
        parser.error('new evidence directory outside the checkout required')
    os.umask(0o077)
    output.mkdir(mode=0o700)
    report = {'schema_version': 1, 'track': 'R0', 'status': 'failed', 'host_os': platform.system(), 'host_os_version': platform.version(), 'python_version': platform.python_version(), 'phases': [], 'overrides': {name: bool(os.environ.get(name)) for name in OVERRIDES}}
    target = output / 'native-target'
    target.mkdir()
    report['target_directory'] = str(target)
    def run(name: str, command: list[str], env: dict | None = None) -> bytes:
        phase = {'name': name, 'command': command, 'status': 'failed'}
        report['phases'].append(phase)
        start = time.monotonic()
        result = subprocess.run(command, cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=1800)
        data = result.stdout
        secret = os.environ.get('WORLDWRIGHT_TEST_DATABASE_URL', '')
        if secret:
            data = data.replace(secret.encode(), b'[DATABASE_URL_REDACTED]')
        (output / f'{name}.log').write_bytes(data)
        phase.update(exit_code=result.returncode, elapsed_seconds=time.monotonic()-start, log_sha256=digest(output / f'{name}.log'))
        if result.returncode:
            raise RuntimeError(f'{name} failed: exit {result.returncode}; see private log')
        phase['status'] = 'passed'
        return result.stdout
    try:
        for tool in ('git', 'cargo', 'rustc', 'node', 'helm'):
            if not shutil.which(tool):
                raise RuntimeError(f'missing required R0 tool: {tool}')
        report['executables'] = {tool: shutil.which(tool) for tool in ('git', 'cargo', 'rustc', 'node', 'helm')}
        for name in ('RUSTC', 'RUSTDOC', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'RUSTC_BOOTSTRAP'):
            if os.environ.get(name): raise RuntimeError(f'R0 forbids compiler substitution/bootstrap override: {name}')
        node_version = run('node-version', ['node', '--version']).decode().strip()
        if int(node_version.lstrip('v').split('.')[0]) < 20:
            raise RuntimeError('Node 20+ required')
        run('helm-version', ['helm', 'version', '--short'])
        if not os.environ.get('WORLDWRIGHT_TEST_DATABASE_URL'):
            raise RuntimeError('disposable WORLDWRIGHT_TEST_DATABASE_URL required; no DB skip')
        pin = tomllib.loads((ROOT / 'rust-toolchain.toml').read_text())['toolchain']['channel']
        for tool in ('cargo', 'rustc'):
            version = run(f'{tool}-version', [tool, '--version']).decode().split()[1]
            if version != pin:
                raise RuntimeError(f'{tool} must match pin {pin}')
        report['toolchain'] = pin
        details = run('rustc-details', ['rustc', '-vV']).decode()
        host = next(line.removeprefix('host: ') for line in details.splitlines() if line.startswith('host: '))
        report['host_target'] = host
        if (os.name == 'nt' and '-windows-' not in host) or (sys.platform.startswith('linux') and '-linux-' not in host):
            raise RuntimeError('compiler host does not match the native OS')
        if os.name != 'nt' and not sys.platform.startswith('linux'):
            raise RuntimeError('R0 supported hosts are Ubuntu/Linux and Windows')
        if sys.platform.startswith('linux'):
            distribution = platform.freedesktop_os_release()
            report['distribution'] = {'id':distribution.get('ID'), 'version':distribution.get('VERSION_ID')}
            if report['distribution'] != {'id':'ubuntu', 'version':'24.04'}:
                raise RuntimeError('R0 baseline requires actual Ubuntu 24.04 LTS, not a renamed Linux report')
        before = source_identity()
        if before['dirty'] and not args.allow_dirty:
            raise RuntimeError('dirty source requires explicit --allow-dirty and recorded fingerprint')
        report['source'] = before
        configs = []
        for parent in (ROOT, *ROOT.parents, Path(os.environ.get('CARGO_HOME', str(Path.home()/'.cargo')))):
            for name in ('.cargo/config', '.cargo/config.toml') if parent != Path(os.environ.get('CARGO_HOME', str(Path.home()/'.cargo'))) else ('config', 'config.toml'):
                path = parent / name
                if path.is_file(): configs.append({'path': str(path), 'sha256': digest(path)})
        report['cargo_configurations'] = configs
        run('sysroot', ['rustc', '--print', 'sysroot'])
        metadata = run('locked-graph', ['cargo', 'metadata', '--format-version', '1', '--all-features', '--locked', '--offline', '--filter-platform', host])
        json.loads(metadata)
        report['dependency_graph_sha256'] = hashlib.sha256(metadata).hexdigest()
        run('fmt', ['cargo', 'fmt', '--all', '--', '--check'])
        common = ['--workspace', '--locked', '--offline', '--target', host, '--target-dir', str(target)]
        run('check', ['cargo', 'check', '--all-targets', *common])
        run('clippy', ['cargo', 'clippy', '--all-targets', '--all-features', *common, '--', '-D', 'warnings'])
        run('test', ['cargo', 'test', *common])
        run('build', ['cargo', 'build', *common])
        suffix = '.exe' if os.name == 'nt' else ''
        report['binaries'] = {}
        for name in ('worldwright', 'worldwright-studio'):
            binary = target / host / 'debug' / (name + suffix)
            run(name + '-version', [str(binary), '--version'])
            report['binaries'][name] = {'path': str(binary), 'sha256': digest(binary)}
        env = os.environ.copy(); env['RUSTDOCFLAGS'] = env.get('RUSTDOCFLAGS', '') + ' -D warnings'
        run('rustdoc', ['cargo', 'doc', '--no-deps', *common], env)
        run('browser-ids', ['node', 'scripts/test-expedition-browser.cjs'])
        run('studio-bounds', ['node', 'scripts/test-studio-capabilities.cjs'])
        run('p2-contracts', [sys.executable, 'scripts/test-p2-contracts.py'])
        run('p2-negatives', [sys.executable, 'scripts/test-p2-negatives.py'])
        listed = run('receipt-list', ['cargo', 'test', '-p', 'worldwright-persistence', '--locked', '--offline', '--target', host, '--target-dir', str(target), 'gameplay_browser_request_id_lifecycle', '--', '--ignored', '--list'])
        if b'database_tests::gameplay_browser_request_id_lifecycle: test' not in listed:
            raise RuntimeError('required receipt test absent; zero tests cannot pass')
        run('receipt-pg', ['cargo', 'test', '-p', 'worldwright-persistence', '--locked', '--offline', '--target', host, '--target-dir', str(target), 'gameplay_browser_request_id_lifecycle', '--', '--ignored', '--test-threads=1'])
        chart = ['pwsh', '-NoProfile', '-File', 'deploy/helm/worldwright/tests/render.ps1'] if os.name == 'nt' else ['bash', 'deploy/helm/worldwright/tests/render.sh']
        run('required-helm', chart)
        if os.name != 'nt':
            bins = target / host / 'debug'
            run('real-first-run', [sys.executable, 'scripts/test-desktop.py', '--real-studio', str(bins/'worldwright-studio'), '--real-preview', str(bins/'worldwright'), '-q'])
        after = source_identity()
        if after != before:
            raise RuntimeError('source/lock changed during R0; new tuple requires rerun')
        report['status'] = 'passed'
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError, StopIteration) as error:
        report['failure'] = str(error)
    finally:
        (output / 'r0-report.json').write_text(json.dumps(report, indent=2)+'\n')
    print(f"R0 {report['status']}: {output / 'r0-report.json'}")
    return 0 if report['status'] == 'passed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
