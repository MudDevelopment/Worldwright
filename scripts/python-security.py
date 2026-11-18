"""Prepare a separate patched Python environment; survey full test coverage.
All generated paths are repository-local. No DB/cluster mutation is automatic.
Exit 0: requested coverage passed; 1: failure; 2: blocked/skipped coverage remains.
"""
from pathlib import Path
import argparse
import ast
import importlib.metadata
import json
import os
import re
import ssl
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
import native_paths
import python_dependencies as deps
import worldwright_local as local
import r3_io
from r6_support import Collector, encode, new_file, require, source_identity, read

BLOCKED_SCRIPTS = {
    'scripts/test-desktop.py': 'requires non-root Linux launcher/process-group/mode acceptance; not a native Windows test',
    'scripts/test-r2-expiry-sql.py': 'requires approved disposable PostgreSQL and SQL path/inventory review; still expects eight migrations in a nine-migration tree',
    'scripts/test-r3-role-fixture.py': 'requires an explicitly owned PostgreSQL cluster and permission to create/drop fixture databases and roles',
    'scripts/test-r6-sql-rehearsal.py': 'requires supported pgserver host/SQL-review dependencies and output-path review; not native migration acceptance',
}
PUBLIC_DISTRIBUTIONS = {
    'pip', 'setuptools', 'wheel', 'cffi', 'pycparser', 'typing-extensions', 'tzdata',
    'greenlet', 'pyee', 'attrs', 'referencing', 'rpds-py', 'jsonschema-specifications', 'packaging',
}

SCRIPT_MARKERS = {
    'scripts/test-p2-negatives.py': b'PASS: 6 negative source-contract checks',
    'scripts/test-studio-browser.py': b'PASS: 5 Chromium fixture groups',
}


def pypi_metadata(name, version):
    require(re.fullmatch(r'[a-z0-9-]+', name) is not None, 'invalid distribution name')
    url = 'https://pypi.org/pypi/' + name + '/' + urllib.parse.quote(version, safe='') + '/json'
    request = urllib.request.Request(url, headers={'User-Agent': 'Worldwright-security-review', 'Accept': 'application/json'})
    with urllib.request.build_opener(r3_io.NoRedirects()).open(request, timeout=20) as response:
        data = response.read(4 * 1024 * 1024 + 1)
    require(len(data) <= 4 * 1024 * 1024, 'package metadata exceeds bound')
    value = json.loads(data)
    require(value.get('info', {}).get('version') == version, 'package metadata version mismatch')
    require(isinstance(value.get('vulnerabilities'), list), 'advisory field unavailable; cannot infer clean')
    return value


def phase(collector, name, argv, timeout=600, expected=0):
    argv = list(map(str, argv))
    print('Security/test phase: ' + name, flush=True)
    collector.event('phase-started', name=name)
    result = r3_io.record_phase(collector.report, collector.output, name, argv, timeout,
                               cwd=local.ROOT, sensitive=collector.sensitive, expected=expected)
    collector.report['phases'][-1]['command'] = argv
    collector.event('phase-finished', **collector.report['phases'][-1])
    ok = result['interruption'] is None and type(result['returncode']) is int and result['returncode'] == expected
    return ok, result


def must(collector, name, argv, timeout=600):
    ok, result = phase(collector, name, argv, timeout)
    require(ok, 'required phase failed: ' + name)
    return result['stdout']


def rust_summaries(stdout):
    # Failed targets have FAILED, not ok, in libtest's terminal summary.
    matches = re.findall(rb'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;', stdout)
    return [[int(n) for n in row] for row in matches]


def candidate_python(candidate):
    return candidate / ('Scripts/python.exe' if os.name == 'nt' else 'bin/python')


def prepare(candidate):
    base = Path(getattr(sys, '_base_executable', sys.executable)).resolve(strict=True)
    collector = Collector(local.LOCAL / 'evidence' / ('python-security-prepare-' + uuid.uuid4().hex), 'python-security-prepare')
    print('PREPARATION EVIDENCE: ' + str(collector.output), flush=True)
    try:
        collector.report['candidate'] = str(candidate)
        collector.report['requirements'] = deps.policy_inventory('security')
        expected = deps.expected('security')
        collector.report['expected_dependencies'] = expected
        python = candidate_python(candidate)
        if candidate.exists():
            require(python.is_file(), 'candidate is partial; choose a new --candidate directory, do not overwrite the old one')
            must(collector, 'existing-candidate-pins', [python, '-B', '-c',
                 "import sys;sys.path.insert(0," + repr(str(local.ROOT / 'scripts')) + ");import python_dependencies as d;d.require_installed('security')"])
        else:
            require(candidate.parent.is_dir(), 'candidate parent must already exist')
            # Read installed base Python; all child temp/cache/profile writes are redirected.
            must(collector, 'create-candidate', [base, '-I', '-B', '-m', 'venv', '--without-pip', candidate])
            must(collector, 'bootstrap-pip', [python, '-I', '-B', '-m', 'ensurepip', '--upgrade'])
            collector.sensitive = tuple(sorted(set(collector.sensitive) | set(r3_io.secret_values(('PIP_INDEX_URL', 'PIP_EXTRA_INDEX_URL'))), key=len, reverse=True))
            must(collector, 'install-patched-profile', [python, '-B', '-m', 'pip', 'install',
                 '--require-virtualenv', '--no-input', '--only-binary=:all:', '--cache-dir', local.LOCAL / 'pip-cache',
                 '-r', local.ROOT / 'scripts/python-test-requirements.txt'], 900)
        must(collector, 'pip-check', [python, '-B', '-m', 'pip', 'check'])
        must(collector, 'candidate-pins', [python, '-B', '-c',
             "import sys;sys.path.insert(0," + repr(str(local.ROOT / 'scripts')) + ");import python_dependencies as d;print(d.require_installed('security'))"])
        must(collector, 'browser-cache-check', [python, '-B', '-c',
             "from pathlib import Path;from playwright.sync_api import sync_playwright;p=sync_playwright().start();x=Path(p.chromium.executable_path);print(x);assert x.is_file(),'Approved Chromium cache missing';p.stop()"])
        require(collector.report['requirements'] == deps.policy_inventory('security'), 'requirements changed during installation')
        collector.report['status'] = 'candidate-prepared-tests-pending'
    except (Exception, KeyboardInterrupt) as error:
        collector.fail(error)
    finally:
        collector.finish()
    return collector.report['status'] == 'candidate-prepared-tests-pending'


def run(candidate, include_native):
    require(Path(sys.prefix).resolve() == candidate.resolve(), 'run must use the candidate interpreter; use the all operation')
    require(__debug__, 'Python optimization is forbidden; assertions must execute')
    # Tests must not accidentally consume operator/deployment credentials.
    for name in tuple(os.environ):
        if name.startswith('WORLDWRIGHT_') or name in ('DATABASE_URL', 'PGHOST', 'PGDATABASE', 'PGSERVICE', 'PGPASSWORD'):
            os.environ.pop(name, None)
    collector = Collector(local.LOCAL / 'evidence' / ('python-security-tests-' + uuid.uuid4().hex), 'python-security-test-survey')
    print('TEST EVIDENCE: ' + str(collector.output), flush=True)
    collector.report.update(full_suite_passed=False, release_authorized=False,
                            tests=[], blocked=[], advisory_audit={'status': 'pending'})
    failed = False
    incomplete = False
    before = None
    try:
        collector.report['dependencies'] = deps.require_installed('security')
        collector.report['requirements'] = deps.policy_inventory('security')
        before = source_identity(); new_file(collector.output / 'source-before.json', encode(before))
        must(collector, 'pip-check', [sys.executable, '-B', '-m', 'pip', 'check'])
        must(collector, 'pip-freeze', [sys.executable, '-B', '-m', 'pip', 'freeze', '--all'])
        from cryptography.hazmat.backends.openssl.backend import backend
        collector.report['openssl'] = {'cryptography_backend': backend.openssl_version_text(), 'python_stdlib_ssl': ssl.OPENSSL_VERSION}
        python_files = [item['path'] for item in before['inventory'] if item['path'].endswith('.py')]
        require(python_files, 'no Python source discovered')
        for relative in python_files:
            source = read(local.ROOT / relative)
            ast.parse(source, filename=relative)
            require(not re.search(rb'46\.0\.6|2\.10\.1', source), 'stale vulnerable pin in Python source: ' + relative)
        collector.report['python_sources_parsed'] = python_files
        test_files = sorted(p for p in python_files if Path(p).name.startswith(('test-', 'test_')))
        require(test_files, 'no Python tests discovered')
        unit_dir = collector.output / 'unit-results'; unit_dir.mkdir()
        for index, relative in enumerate(test_files):
            row = {'path': relative, 'status': 'pending'}
            collector.report['tests'].append(row)
            if relative in BLOCKED_SCRIPTS:
                row.update(status='blocked', reason=BLOCKED_SCRIPTS[relative]); incomplete = True
                continue
            tree = ast.parse(read(local.ROOT / relative), filename=relative)
            unit = any(isinstance(n, ast.ClassDef) and any(isinstance(b, ast.Attribute) and b.attr == 'TestCase' for b in n.bases) for n in ast.walk(tree))
            label = 'python-test-' + str(index).zfill(2) + '-' + re.sub(r'[^a-z0-9-]', '-', Path(relative).stem.lower())
            if unit:
                report_path = unit_dir / (str(index).zfill(2) + '.json')
                ok, result = phase(collector, label, [sys.executable, '-B', local.ROOT / 'scripts/run-python-unit.py',
                                                    '--file', local.ROOT / relative, '--report', report_path])
                if report_path.is_file():
                    detail = json.loads(read(report_path)); row['result'] = detail
                    row['status'] = detail['status']
                    if result['interruption'] is not None or (result['returncode'] not in (0, 2)):
                        row['status'] = 'failed'; failed = True
                    elif row['status'] == 'executed-with-unverified-cases':
                        incomplete = True
                    elif not ok or row['status'] != 'passed':
                        failed = True
                else:
                    row['status'] = 'failed'; failed = True
            elif relative in SCRIPT_MARKERS:
                ok, result = phase(collector, label, [sys.executable, '-B', local.ROOT / relative])
                row['status'] = 'passed' if ok and SCRIPT_MARKERS[relative] in result['stdout'] else 'failed'
                failed |= row['status'] == 'failed'
            else:
                row.update(status='blocked', reason='new/non-unittest entry point requires explicit invocation review')
                incomplete = True
            collector.event('test-result', **row)
        draft = collector.output / 'draft'
        ok, _ = phase(collector, 'draft-init', [sys.executable, '-B', local.ROOT / 'scripts/r7.py', 'init', '--output', draft])
        failed |= not ok
        if ok:
            ok, _ = phase(collector, 'draft-no-go', [sys.executable, '-B', local.ROOT / 'scripts/r7.py', 'check',
                          '--candidate', draft / 'candidate.json', '--output', collector.output / 'draft-check'], expected=1)
            decision_path = collector.output / 'draft-check/decision.json'
            decision = json.loads(read(decision_path)) if decision_path.is_file() else {}
            failed |= not ok or decision.get('decision') != 'NO-GO' or decision.get('blockers') != ['open-prerequisite-blockers']
        # Resolve/advisory survey is a fresh index snapshot, not proof of no unknown vulnerabilities.
        audit_dir = collector.output / 'package-advisories'; audit_dir.mkdir()
        packages = sorted({(deps.normalized(d.metadata['Name']), d.version) for d in importlib.metadata.distributions()})
        require(len(packages) <= 128, 'unexpected candidate package inventory')
        audit = []
        for name, version in packages:
            entry = {'name': name, 'version': version, 'status': 'unverified'}
            try:
                require(name in PUBLIC_DISTRIBUTIONS or name in deps.expected('security'),
                        'unreviewed extra distribution; do not disclose its name to a public index')
                metadata = pypi_metadata(name, version)
                new_file(audit_dir / (name + '.json'), encode(metadata))
                active = [v for v in metadata['vulnerabilities'] if not v.get('withdrawn')]
                entry.update(status='known-advisories-found' if active else 'no-advisories-reported',
                             advisories=[{'id': v.get('id'), 'aliases': v.get('aliases'), 'link': v.get('link')} for v in active])
                failed |= bool(active)
            except Exception as error:
                entry['failure_type'] = type(error).__name__; incomplete = True
            audit.append(entry)
        collector.report['advisory_audit'] = {'status': 'complete' if all(x['status'] != 'unverified' for x in audit) else 'incomplete',
                                             'source': 'PyPI version JSON advisory metadata', 'packages': audit}
        if include_native:
            configuration = native_paths.configure_native()
            collector.report['native_paths'] = configuration
            ok, _ = phase(collector, 'native-collection', [sys.executable, '-B', local.ROOT / 'scripts/r6-native.py',
                          '--output', collector.output / 'native', '--target-dir', configuration['target'], '--allow-dirty'], 3600)
            collector.report['native_collection_passed'] = ok
            failed |= not ok
            if ok:
                ok, _ = phase(collector, 'native-integrity', [sys.executable, '-B', local.ROOT / 'scripts/r6-evidence.py',
                              '--report', collector.output / 'native/r6-report.json', '--check-local'])
                collector.report['native_integrity_passed'] = ok; failed |= not ok
            cargo = configuration['binaries']['cargo']['path']
            common = ['--workspace', '--all-features', '--locked', '--offline', '--target', configuration['host'], '--target-dir', configuration['target']]
            for label, kind in (('workspace-all-target-tests', '--all-targets'), ('workspace-doctests', '--doc')):
                ok, result = phase(collector, label, [cargo, 'test', *common, kind, '--no-fail-fast', '--', '--test-threads=1'], 3600)
                summaries = rust_summaries(result['stdout'])
                ignored = sum(s[2] for s in summaries)
                collector.report[label] = {'process_passed': ok, 'summaries': summaries, 'ignored': ignored}
                failed |= not ok or not summaries or any(s[1] for s in summaries)
                incomplete |= ignored > 0
            if collector.report.get('native_integrity_passed'):
                ok, _ = phase(collector, 'native-integrity-final', [sys.executable, '-B', local.ROOT / 'scripts/r6-evidence.py',
                              '--report', collector.output / 'native/r6-report.json', '--check-local'])
                collector.report['native_integrity_after_workspace_tests'] = ok
                failed |= not ok
        else:
            incomplete = True
            collector.report['blocked'].append('native/workspace tests not requested')
        collector.report['blocked'] += [r for r in collector.report['tests'] if r['status'] == 'blocked']
        collector.report['acceptance_not_supplied'] = ['live provider/recovery/load, installed desktop/manual shutdown and independent release approval are not supplied by this local run']
        after = source_identity(); new_file(collector.output / 'source-after.json', encode(after))
        stable = before['source_sha256'] == after['source_sha256']
        collector.report['source_unchanged'] = stable
        failed |= not stable or collector.report['requirements'] != deps.policy_inventory('security')
        collector.report['status'] = 'failed' if failed else 'available-checks-passed-full-suite-incomplete' if incomplete else 'all-requested-tests-passed'
        collector.report['executed_checks_passed'] = not failed
        collector.report['full_suite_passed'] = not failed and not incomplete
    except (Exception, KeyboardInterrupt) as error:
        collector.fail(error)
    finally:
        collector.finish()
    for row in collector.report['tests']:
        if row['status'] != 'passed':
            print('Test coverage:', row['status'], row['path'], flush=True)
    print('Full suite coverage:', 'COMPLETE' if collector.report['full_suite_passed'] else 'INCOMPLETE; see skips/blocked/failures', flush=True)
    return 0 if collector.report['full_suite_passed'] else 1 if collector.report['status'] == 'failed' else 2


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=('prepare', 'run', 'all'))
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--without-native', action='store_true', help='explicitly incomplete Python-only diagnostic')
    args = parser.parse_args()
    local.configure_tools_environment()
    candidate = local.local_path(args.candidate or local.LOCAL / 'python/Worldwright-Python-Security')
    require(candidate.parent == (local.LOCAL / 'python').resolve(), 'candidate must be directly inside .worldwright-local/python')
    require(candidate != (local.LOCAL / 'python/Worldwright-R6-Python').resolve(), 'preserve the original accepted Python environment')
    if args.operation in ('prepare', 'all'):
        if not prepare(candidate):
            return 1
        if args.operation == 'prepare':
            return 0
        command = [str(candidate_python(candidate)), '-B', str(Path(__file__).resolve()), 'run', '--candidate', str(candidate)]
        if args.without_native: command.append('--without-native')
        return subprocess.run(command, cwd=local.ROOT, check=False).returncode
    return run(candidate, not args.without_native)


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (OSError, ValueError) as error:
        print('Python security setup failed:', error, file=sys.stderr)
        raise SystemExit(1)
