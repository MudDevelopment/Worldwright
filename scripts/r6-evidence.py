"""Read-only R6 collection integrity/completeness checks. NEVER closes R6."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import re
from r6_support import encode, read, require, safe_path, sha, source_identity, test_listing, test_result

BROWSER_CASES = {'real-http-ready', 'browser-import-edit-save-reload-validate',
                 'real-preview-reference', 'real-preview-expedition',
                 'real-host-origin-content-type-authority-denial', 'browser-no-script-errors-or-external-requests'}
NATIVE_CASES = {'r5-native', 'studio-full', 'operations-authority', 'release-integrity',
                'console-authority', 'terminal', 'relocated-consumers', 'retained-sdk'}


def unique(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate JSON key')
        result[key] = value
    return result


def load(path):
    return json.loads(read(path), object_pairs_hook=unique)


def records(records, key):
    require(isinstance(records, list) and all(isinstance(v, dict) and isinstance(v.get(key), str) for v in records), 'invalid record inventory')
    names = [v[key] for v in records]
    require(len(names) == len(set(names)), 'duplicate record')
    return set(names)


def verify(path, check_local=False):
    path = Path(path).resolve(strict=True)
    report = load(path)
    require(type(report.get('schema_version')) is int and report['schema_version'] == 1 and report.get('track') == 'R6', 'unsupported report')
    require(report.get('full_r6_closed') is False and report.get('independent_adoption') is False, 'collector cannot claim closure/adoption')
    files = report.get('files')
    names = records(files, 'path')
    require(0 < len(files) <= 1000 and path.name not in names, 'invalid evidence inventory')
    for item in files:
        require(re.fullmatch('[0-9a-f]{64}', item.get('sha256', '')), 'invalid hash')
        require(sha(safe_path(path.parent, item['path'])) == item['sha256'], 'evidence changed: ' + item['path'])
    actual = {p.relative_to(path.parent).as_posix() for p in path.parent.rglob('*') if p.is_file() and p != path}
    require(actual == names, 'unlisted or missing evidence file')
    require('events.jsonl' in names, 'partial-progress journal missing')
    require(report.get('status') != 'failed', 'failed collection is retained evidence, not successful completion')
    cases = records(report.get('cases'), 'case')
    require(all(v.get('status') == 'passed' for v in report['cases']), 'case did not pass')
    kind = report.get('kind')
    if kind == 'real-studio-browser':
        windows = report.get('status') == 'browser_workflow_passed_windows_shutdown_open'
        expected = BROWSER_CASES if windows else BROWSER_CASES | {'unix-graceful-active-preview-shutdown', 'unix-same-workspace-reopen'}
        require(cases == expected, 'browser case inventory incomplete')
        if windows:
            require(report.get('windows_shutdown', {}).get('status') == 'manual_pending', 'Windows shutdown must remain open')
        else:
            require(report.get('status') == 'browser_workflow_passed_manual_security_open', 'invalid browser status')
        require(report.get('browser_version'), 'actual browser version missing')
        require(isinstance(report.get('binaries'), dict) and set(report['binaries']) == {'studio', 'preview'}, 'binary inventory incomplete')
        for binary in report['binaries'].values():
            require(isinstance(binary, dict) and isinstance(binary.get('path'), str)
                    and re.fullmatch('[0-9a-f]{64}', binary.get('sha256', '')), 'invalid binary identity')
        for game in ('reference', 'expedition'):
            case = next(v for v in report['cases'] if v['case'] == 'real-preview-' + game)
            digest = case.get('artifact_digest', '')
            require(re.fullmatch('[0-9a-f]{64}', digest) and re.fullmatch('[0-9a-f]{64}', case.get('payload_digest', '')), 'preview identity incomplete')
            require(sha(safe_path(path.parent, 'workspace/artifacts/' + digest + '.wwr')) == digest, 'preview artifact bytes differ')
        require(not check_local, 'use native report for complete local source/config/binary binding')
    elif kind == 'native-browser-security-prerequisites':
        require(report.get('status') == 'collected_native_browser_not_closed' and cases == NATIVE_CASES, 'native collection incomplete')
        expected = {'rustc-version', 'cargo-version', 'native-host', 'node-version', 'format', 'locked-metadata',
                    'build-studio', 'real-browser', 'clippy', 'studio-capabilities', 'studio-workflow-fixtures'}
        expected |= {name + suffix for name in NATIVE_CASES for suffix in ('-list', '-run')}
        require(records(report.get('phases'), 'name') == expected, 'native phase inventory differs')
        for phase in report['phases']:
            require(type(phase.get('exit_code')) is int and phase['exit_code'] == 0 and phase.get('interruption') is None, 'failed/interrupted phase')
            require(sha(path.parent / (phase['name'] + '.log')) == phase.get('log_sha256'), 'phase log mismatch')
        for case in report['cases']:
            label = case['case']
            discovered = test_listing(read(path.parent / (label + '-list.log')))
            passed = test_result(read(path.parent / (label + '-run.log')), len(discovered))
            require(discovered == case.get('discovered') and type(case.get('passed')) is int and passed == case['passed'], 'native test counts differ')
        before = load(path.parent / 'source-before.json')
        after = load(path.parent / 'source-after.json')
        require(before == after and before['source_sha256'] == report.get('source_sha256'), 'source changed during run')
        require(hashlib.sha256(encode(before['inventory'])).hexdigest() == before['source_sha256'], 'source inventory hash differs')
        child = verify(path.parent / 'browser/r6-report.json')
        require(report.get('browser_status') == child['status'], 'browser outcome mismatch')
        for short, full in (('studio', 'worldwright-studio'), ('preview', 'worldwright')):
            require(child['binaries'][short]['sha256'] == report['binaries'][full]['sha256'], 'browser/native binaries differ')
        if check_local:
            require(source_identity() == before, 'current checkout no longer matches collection')
            for binary in report['binaries'].values():
                require(sha(Path(binary['path'])) == binary['sha256'], 'current binary differs')
            for config in report['cargo_configurations']:
                require(sha(Path(config['path'])) == config['sha256'], 'current Cargo configuration differs')
    else:
        raise ValueError('unknown collection kind')
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--check-local', action='store_true')
    args = parser.parse_args()
    try:
        report = verify(args.report, args.check_local)
        print('R6 collection integrity verified:', report['kind'])
        print('R6 NOT CLOSED: unsigned collected evidence is not independent security/recovery/retention adoption.')
        return 0
    except (OSError, ValueError, KeyError, TypeError) as error:
        print('R6 NOT CLOSED; collection verification failed:', type(error).__name__, str(error))
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
