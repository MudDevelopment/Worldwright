"""Pinned non-DB native build/browser/security-test collection. R6 stays open.
Run locally on native Windows or Ubuntu 24.04 with approved cached dependencies.
"""
from pathlib import Path
import argparse
import json
import os
import platform
import shutil
import sys
import tomllib
from r6_support import Collector, ROOT, encode, new_file, read, require, sha, source_identity, test_listing, test_result
import native_paths as native
import worldwright_local as local
import r6_subjects as subjects


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--target-dir', type=Path, required=True)
    parser.add_argument('--allow-dirty', action='store_true')
    parser.add_argument('--browser-executable', type=Path)
    args = parser.parse_args()
    local.local_path(Path(sys.prefix))
    local.configure_tools_environment()
    collector = Collector(args.output, 'native-browser-security-prerequisites')
    target = args.target_dir.resolve()
    try:
        target = local.local_path(args.target_dir)
        require(not target.is_relative_to(collector.output.resolve())
                and not collector.output.resolve().is_relative_to(target), 'target/evidence must be disjoint')
        collector.report['storage_policy'] = 'repository-local-native-v1'
        collector.report['native_paths'] = native.configure_native(target)
        collector.phase('native-path-preflight', [sys.executable, '-B', str(ROOT / 'scripts/native-storage.py'),
                        'check', '--output', str(collector.output / 'path-preflight')], 900)
        preflight = json.loads(read(collector.output / 'path-preflight/r6-report.json'))
        require(preflight.get('status') == 'native-storage-preflight-passed-not-acceptance', 'native path preflight did not pass')
        require(os.name == 'nt' or sys.platform.startswith('linux'), 'supported native OS required')
        if os.name != 'nt':
            require(os.geteuid() != 0, 'ordinary non-root Ubuntu user required')
            distro = platform.freedesktop_os_release()
            require(distro.get('ID') == 'ubuntu' and distro.get('VERSION_ID') == '24.04', 'Ubuntu 24.04 baseline required')
        collector.report['host_os'] = platform.system()
        collector.report['target_directory'] = str(target)
        collector.report['target_policy'] = 'cached-development-not-fresh-R0'
        for tool in ('cargo', 'rustc', 'git', 'node'):
            require(shutil.which(tool), 'missing native tool: ' + tool)
        for key in ('RUSTC', 'RUSTDOC', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'RUSTC_BOOTSTRAP', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS'):
            require(not os.environ.get(key), 'unreviewed compiler override: ' + key)
        require(not os.environ.get('WORLDWRIGHT_R5_SESSION_TEST_ROOT'), 'test child override must not be inherited')
        for tool in ('rustc', 'cargo'):
            text = collector.phase(tool + '-version', [tool, '--version']).decode().split()
            require(len(text) > 1 and text[1] == '1.85.1', 'pinned Rust/Cargo 1.85.1 required')
        pin = tomllib.loads(read(ROOT / 'rust-toolchain.toml').decode())['toolchain']['channel']
        require(pin == '1.85.1', 'repository toolchain pin changed; review required')
        details = collector.phase('native-host', ['rustc', '-vV']).decode()
        hosts = [line[6:] for line in details.splitlines() if line.startswith('host: ')]
        require(len(hosts) == 1, 'native host missing')
        host = hosts[0]
        require(('-windows-' in host) if os.name == 'nt' else ('-linux-' in host), 'native host mismatch')
        collector.report['host_target'] = host
        node = collector.phase('node-version', ['node', '--version']).decode().strip()
        require(int(node.lstrip('v').split('.')[0]) >= 20, 'Node 20+ required')
        before = source_identity()
        require(args.allow_dirty or not before['dirty'], 'dirty source requires explicit --allow-dirty')
        new_file(collector.output / 'source-before.json', encode(before))
        collector.report['source_sha256'] = before['source_sha256']
        collector.report['commit'] = before['commit']
        configs = []
        for parent in (ROOT, *ROOT.parents):
            for name in ('.cargo/config', '.cargo/config.toml'):
                path = parent / name
                if path.is_file(): configs.append({'path': str(path), 'sha256': sha(path)})
        cargo_home = Path(os.environ.get('CARGO_HOME', str(Path.home() / '.cargo')))
        for name in ('config', 'config.toml'):
            if (cargo_home / name).is_file(): configs.append({'path': str(cargo_home / name), 'sha256': sha(cargo_home / name)})
        collector.report['cargo_configurations'] = configs
        common = ['--locked', '--offline', '--target', host, '--target-dir', str(target)]
        collector.phase('format', ['cargo', 'fmt', '--all', '--', '--check'], 180)
        collector.phase('locked-metadata', ['cargo', 'metadata', '--format-version', '1', '--locked', '--offline', '--filter-platform', host], 180)
        collector.phase('build-studio', ['cargo', 'build', '-p', 'worldwright', '-p', 'worldwright-studio', '-p', 'worldwright-server', '--bin', 'worldwright', '--bin', 'worldwright-studio', '--bin', 'worldwright-server', *common], 1800)
        extension = '.exe' if os.name == 'nt' else ''
        build_outputs = {name: target / host / 'debug' / (name + extension) for name in ('worldwright', 'worldwright-studio', 'worldwright-server')}
        collector.report['binary_subject_policy'] = subjects.POLICY
        collector.report['binaries'] = subjects.retain_binaries(build_outputs, collector.output)
        binaries = {name: Path(record['path']) for name, record in collector.report['binaries'].items()}
        collector.event('binary-subjects-retained', policy=subjects.POLICY, binaries=collector.report['binaries'])
        browser = [sys.executable, str(ROOT / 'scripts/r6-browser.py'), '--studio', str(binaries['worldwright-studio']), '--preview', str(binaries['worldwright']), '--output', str(collector.output / 'browser')]
        if args.browser_executable: browser += ['--browser-executable', str(args.browser_executable.resolve())]
        collector.phase('real-browser', browser, 1200)
        browser_report = json.loads(read(collector.output / 'browser/r6-report.json'))
        require(browser_report.get('status') in ('browser_workflow_passed_windows_shutdown_open', 'browser_workflow_passed_manual_security_open'), 'browser collection incomplete')
        for short, full in (('studio', 'worldwright-studio'), ('preview', 'worldwright')):
            require(browser_report['binaries'][short]['sha256'] == collector.report['binaries'][full]['sha256'], 'browser/native binary identity mismatch')
        def suite(label, arguments, minimum=1, expected=None):
            listed = collector.phase(label + '-list', ['cargo', 'test', *arguments, *common, '--', '--list'], 1800)
            names = test_listing(listed)
            require(len(names) >= minimum and (expected is None or len(names) == expected), 'required test inventory differs: ' + label)
            output = collector.phase(label + '-run', ['cargo', 'test', *arguments, *common, '--', '--test-threads=1', '--nocapture'], 1800)
            count = test_result(output, len(names))
            collector.case(label, discovered=names, passed=count)
        suite('r5-native', ['-p', 'worldwright-studio', '--lib', 'r5_'], expected=19 if os.name == 'nt' else 21)
        suite('studio-full', ['-p', 'worldwright-studio'])
        suite('server-r6', ['-p', 'worldwright-server', '--lib', 'r6_'], minimum=2)
        suite('operations-authority', ['-p', 'worldwright-operations', '--lib'])
        suite('release-integrity', ['-p', 'worldwright-release', '--lib'])
        suite('console-authority', ['-p', 'worldwright-console'])
        suite('terminal', ['-p', 'worldwright-expedition', '--lib', 'terminal::tests'])
        suite('relocated-consumers', ['-p', 'worldwright', '--test', 'maturity'])
        suite('retained-sdk', ['-p', 'worldwright', '--test', 'sdk_compatibility'])
        collector.phase('clippy', ['cargo', 'clippy', '-p', 'worldwright-studio', '-p', 'worldwright-server', '-p', 'worldwright-operations', '-p', 'worldwright-console', '-p', 'worldwright-release', '--all-targets', '--all-features', *common, '--', '-D', 'warnings'], 1800)
        collector.phase('studio-capabilities', ['node', 'scripts/test-studio-capabilities.cjs'])
        collector.phase('studio-workflow-fixtures', ['node', 'scripts/test-studio-workflow.cjs'])
        after = source_identity()
        new_file(collector.output / 'source-after.json', encode(after))
        require(before == after, 'source/lock/worktree changed during collection')
        require(all(sha(path) == item['sha256'] for path, item in ((Path(c['path']), c) for c in configs)), 'Cargo configuration changed during collection')
        require(native.configuration_inventory() == collector.report['native_paths']['cargo_configurations'], 'Cargo config inventory changed during collection')
        require(all(sha(Path(item['path'])) == item['sha256'] for item in collector.report['native_paths']['binaries'].values()), 'native tool binary changed during collection')
        collector.report['cache_binary_observations'] = subjects.cache_observations(collector.report['binaries'])
        subjects.verify_retained(collector.output, collector.report['binaries'], check_local=True)
        changed = [name for name, value in collector.report['cache_binary_observations'].items() if not value['matches_retained']]
        if changed:
            print('Cargo cache outputs changed; retained browser subjects verified unchanged: ' + ', '.join(changed), flush=True)
        collector.report['browser_status'] = browser_report['status']
        collector.report['status'] = 'collected_native_browser_not_closed'
    except (Exception, KeyboardInterrupt) as error:
        collector.fail(error)
    finally:
        collector.finish()
    return 0 if collector.report['status'] == 'collected_native_browser_not_closed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
