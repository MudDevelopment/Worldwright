"""Collect local #2 evidence and prepare #4 handoff; never grants approval.
STORAGE POLICY: generated files must stay within E:/Hold/Worldwright.
--tools-only runs helper tests, source fingerprints and draft handoff.
--native additionally requires a fresh repository-local native preflight before
application compilation/browser tests. Missing tools/cache fail without downloads.
No installation, migration, deployment, key creation, signing or source edits.
"""
from pathlib import Path
import argparse
import hashlib
import importlib.metadata
import json
import os
import subprocess
import sys
import time
import uuid


def write_json(path, value):
    with Path(path).open('x', encoding='utf-8', newline='\n') as stream:
        json.dump(value, stream, indent=2, sort_keys=True)
        stream.write('\n')


def sha(path):
    result = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for block in iter(lambda: stream.read(65536), b''):
            result.update(block)
    return result.hexdigest()


def validate_paths(repo, output, target):
    if not (repo / 'Cargo.toml').is_file():
        raise ValueError('Repository must contain Cargo.toml')
    for path in (output, target):
        if path.resolve() == repo.resolve() or not path.resolve().is_relative_to(repo.resolve()):
            raise ValueError('Evidence and build cache must be strictly inside E:/Hold/Worldwright')
        for ancestor in (path, *path.parents):
            if ancestor.is_symlink() or (hasattr(ancestor, 'is_junction') and ancestor.is_junction()):
                raise ValueError('Linked output/cache paths are refused')
    if output.is_relative_to(target) or target.is_relative_to(output):
        raise ValueError('Evidence and build cache must be disjoint')
    if output.exists() or not output.parent.is_dir():
        raise ValueError('Use a new evidence directory with an existing private parent')


def file_inventory(root):
    files = []
    for directory, dirs, names in os.walk(root, followlinks=False):
        for name in dirs + names:
            path = Path(directory) / name
            if path.is_symlink() or (hasattr(path, 'is_junction') and path.is_junction()):
                raise ValueError('Linked evidence refused')
        for name in names:
            path = Path(directory) / name
            if len(files) >= 5000 or path.stat().st_size > 512 * 1024 * 1024:
                raise ValueError('Evidence inventory bound exceeded; retain the directory')
            files.append({'path': path.relative_to(root).as_posix(), 'bytes': path.stat().st_size,
                          'sha256': sha(path)})
    return sorted(files, key=lambda item: item['path'])


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--target-dir', type=Path)
    parser.add_argument('--allow-dirty', action='store_true')
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--tools-only', action='store_true', help='Run helper validation only')
    mode.add_argument('--native', action='store_true', help='Run helpers plus preflight-gated offline native collection')
    parser.add_argument('--browser-executable', type=Path)
    args = parser.parse_args(argv)
    if sys.version_info < (3, 11):
        parser.error('Python 3.11+ required')
    repo = args.repo.absolute()
    if repo.resolve() != Path('E:/Hold/Worldwright').resolve():
        parser.error('The only authorized project root is E:/Hold/Worldwright')
    parent = repo / '.worldwright-local' / 'evidence'
    output = (args.output or parent / ('signoff-' + uuid.uuid4().hex)).absolute()
    target = (args.target_dir or repo / '.worldwright-local' / 'target').absolute()
    sys.dont_write_bytecode = True
    import worldwright_local as local
    if local.ROOT.resolve() != repo.resolve():
        parser.error('Collector and repository paths differ')
    local.local_path(Path(sys.prefix))
    local.local_path(output)
    local.local_path(target)
    storage_paths = local.configure_tools_environment()
    validate_paths(repo, output, target)
    os.umask(0o077)
    output.mkdir(mode=0o700)
    report = {'schema_version': 1, 'kind': 'local-signoff-collection-not-approval',
              'status': 'failed', 'started_at': int(time.time()), 'phases': [],
              'item_2_closed': False, 'item_4_closed': False,
              'full_r6_closed': False, 'supported_private_beta_authorized': False,
              'scope': 'tool-regressions-only' if args.tools_only else 'native-and-tool-prerequisites',
              'storage_paths': storage_paths,
              'native_collection_status': 'not-requested' if args.tools_only else 'pending-local-preflight'}
    env = dict(os.environ, PYTHONDONTWRITEBYTECODE='1', CARGO_TERM_COLOR='never')
    print('EVIDENCE DIRECTORY: ' + str(output), flush=True)
    print('Do not edit source, format, or run another build during collection.', flush=True)
    print('Existing collectors print phase names; detailed logs are in their output directories.', flush=True)
    print('No database/deployment changes or signing operations will run.', flush=True)

    def phase(name, arguments):
        record = {'name': name, 'started_at': int(time.time()), 'status': 'running'}
        report['phases'].append(record)
        with (output / 'events.jsonl').open('a', encoding='utf-8') as stream:
            stream.write(json.dumps({'event': 'started', **record}) + '\n')
            stream.flush()
            os.fsync(stream.fileno())
        print('\n=== ' + name + ' ===', flush=True)
        # Inherit the console so the installed collector's progress remains visible.
        # Those collectors implement their own bounded, timed subprocess logging.
        result = subprocess.run([sys.executable, *map(str, arguments)], cwd=repo, env=env, check=False)
        record.update(exit_code=result.returncode, finished_at=int(time.time()),
                      status='passed' if result.returncode == 0 else 'failed')
        with (output / 'events.jsonl').open('a', encoding='utf-8') as stream:
            stream.write(json.dumps({'event': 'finished', **record}) + '\n')
            stream.flush()
            os.fsync(stream.fileno())
        return result.returncode == 0

    try:
        ignored = subprocess.run(['git', 'check-ignore', '-q', '--', '.worldwright-local'],
                                 cwd=repo, env=env, capture_output=True, timeout=30, check=False)
        tracked = subprocess.run(['git', 'ls-files', '-z', '--cached', '--', '.worldwright-local'],
                                 cwd=repo, env=env, capture_output=True, timeout=30, check=False)
        if ignored.returncode != 0 or tracked.returncode != 0 or tracked.stdout:
            report['prerequisite_failure'] = '.worldwright-local must be Git-ignored and contain no tracked files; nothing is automatically untracked.'
            raise ValueError('Runtime storage is not excluded from source identity')
        dependencies = {}
        for name in ('playwright', 'psutil', 'cryptography'):
            try:
                dependencies[name] = importlib.metadata.version(name)
            except importlib.metadata.PackageNotFoundError:
                report['prerequisite_failure'] = 'Missing Python package: ' + name
                raise
        report['python'] = sys.version
        report['dependencies'] = dependencies
        if dependencies != {'playwright': '1.56.0', 'psutil': '7.2.2', 'cryptography': '46.0.6'}:
            raise ValueError('Dependency pins differ; install the repository R6 and R7 requirements with approval')
        from playwright.sync_api import sync_playwright
        if args.browser_executable:
            browser_path = local.local_path(args.browser_executable)
        else:
            with sync_playwright() as playwright:
                browser_path = local.local_path(Path(playwright.chromium.executable_path))
        if not browser_path.is_file():
            report['prerequisite_failure'] = 'Chromium missing: supply --browser-executable with an approved Chromium executable, or provision the approved Playwright browser cache.'
            raise ValueError('Chromium is not installed; no native build started')
        scripts = repo / 'scripts'
        for name in ('r7.py', 'r7_core.py', 'r7-check.py', 'r6-native.py', 'r6-evidence.py'):
            if not (scripts / name).is_file():
                raise ValueError('Missing required installed script: ' + name)
        sys.path.insert(0, str(scripts))
        sys.dont_write_bytecode = True
        import r7_core as core
        if core.ROOT.resolve() != repo.resolve():
            raise ValueError('Imported tooling belongs to a different checkout')
        checklist = {'schema_version': 1, 'kind': 'pending-review-checklist-not-evidence',
                     'item_2': {'local_native_status': 'pending', 'source_review': 'pending',
                                'documentation_review': 'pending', 'live_r6_drills': 'pending',
                                'windows_ctrl_c_and_same_workspace_reopen': 'pending'},
                     'gates': {gate: {'reviewer_role': core.GATE_ROLES[gate],
                                     'cases': [{'case': case, 'status': 'pending', 'evidence': []}
                                               for case in sorted(cases)]}
                               for gate, cases in core.GATES.items()},
                     'policies': {policy: {'owner_role': role, 'status': 'pending'}
                                  for policy, role in core.POLICY_ROLES.items()},
                     'final_independent_roles': sorted(core.ROLES),
                     'notes': ['Do not sign this checklist. Use r7.py review-templates after assembling actual evidence.',
                               'R6 native reports use a different fingerprint recipe; never substitute them for R0 reports.',
                               'Original migration repair and live R2/R3 prerequisites remain separate.',
                               'Source review and documentation completion cannot be proved by running this wrapper.']}
        write_json(output / 'signoff-checklist.json', checklist)
        if not phase('source-before', [scripts / 'r7.py', 'fingerprint', '--output', output / 'source-before']):
            raise ValueError('Source fingerprint failed')
        tools_ok = phase('tool-regressions', [scripts / 'r7-check.py', '--output', output / 'tool-checks'])
        # Keep #4 preparation available even if native compilation later fails.
        draft_ok = phase('draft-handoff', [scripts / 'r7.py', 'init', '--output', output / 'candidate-draft'])
        native_ok = False
        verified = False
        if tools_ok and not args.tools_only:
            arguments = [scripts / 'r6-native.py', '--output', output / 'native', '--target-dir', target]
            if args.allow_dirty:
                arguments.append('--allow-dirty')
            if args.browser_executable:
                arguments += ['--browser-executable', args.browser_executable.absolute()]
            native_ok = phase('native-browser-tests-clippy', arguments)
            report['native_collection_status'] = 'collected-not-closed' if native_ok else 'failed-see-native-report'
            if native_ok:
                verified = phase('native-integrity', [scripts / 'r6-evidence.py', '--report',
                                                      output / 'native/r6-report.json', '--check-local'])
        after_ok = phase('source-after', [scripts / 'r7.py', 'fingerprint', '--output', output / 'source-after'])
        stable = False
        if after_ok:
            stable = all((output / 'source-before' / name).read_bytes() ==
                         (output / 'source-after' / name).read_bytes()
                         for name in ('source.json', 'migrations.json'))
        report.update(tool_regressions_passed=tools_ok, draft_created=draft_ok,
                      native_collection_passed=native_ok, native_integrity_passed=verified,
                      source_unchanged=stable)
        if args.tools_only and tools_ok and draft_ok and stable:
            report['status'] = 'tool-evidence-collected-native-pending'
        elif tools_ok and draft_ok and native_ok and verified and stable:
            report['status'] = 'local-evidence-collected-review-and-live-acceptance-pending'
    except KeyboardInterrupt:
        report['failure'] = 'Interrupted: preserve evidence and verify child processes stopped before restarting.'
    except Exception as error:
        # Do not dump arbitrary environment values or exception payloads into evidence.
        report['failure'] = type(error).__name__ + ': collection could not complete; inspect the retained phase logs.'
        print('Collection could not complete (' + type(error).__name__ + ').', flush=True)
    finally:
        report['finished_at'] = int(time.time())
        try:
            inventory = file_inventory(output)
        except Exception:
            inventory = None
            report['status'] = 'failed'
            report['inventory_failure'] = True
        summary_path = output / 'collection-summary.json'
        write_json(summary_path, report)
        if inventory is not None:
            inventory.append({'path': summary_path.name, 'bytes': summary_path.stat().st_size,
                              'sha256': sha(summary_path)})
            write_json(output / 'files.sha256.json', {'schema_version': 1, 'files': inventory,
                       'note': 'Byte integrity only, not a signature. Manifest excludes itself.'})
        else:
            write_json(output / 'INVENTORY-FAILED.json', {'status': 'failed', 'preserve_all_files': True})
        print('\nRESULT: ' + report['status'], flush=True)
        print('PRESERVE / RETURN THIS DIRECTORY: ' + str(output), flush=True)
        print('Items #2 and #4 are NOT automatically signed off. No approval was created.', flush=True)
    return 0 if report['status'] in ('tool-evidence-collected-native-pending', 'local-evidence-collected-review-and-live-acceptance-pending') else 1


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (OSError, ValueError) as error:
        print('Collection setup failed: ' + str(error), file=sys.stderr)
        raise SystemExit(1)
