"""Fail-closed R0/R1 evidence validation; not authentication of human approvals."""
from pathlib import Path
import argparse
import hashlib
import json
import re
import sys

REQUIRED = {'node-version', 'helm-version', 'cargo-version', 'rustc-version', 'rustc-details', 'sysroot', 'locked-graph', 'fmt', 'check', 'clippy', 'test', 'build', 'rustdoc', 'worldwright-version', 'worldwright-studio-version', 'receipt-list', 'receipt-pg', 'required-helm', 'browser-ids', 'studio-bounds', 'p2-contracts', 'p2-negatives'}
MANUAL = {'chromium-x11', 'chromium-wayland', 'firefox-x11', 'firefox-wayland', 'manual-no-browser', 'windows-credentials-and-discovery'}


def read(path: Path) -> dict:
    if path.is_symlink() or path.stat().st_size > 16 * 1024 * 1024: raise ValueError('invalid evidence file')
    value = json.loads(path.read_text())
    if not isinstance(value, dict) or type(value.get('schema_version')) is not int or value['schema_version'] != 1: raise ValueError('unsupported evidence schema')
    return value


def require_digest(value, label: str) -> None:
    if not isinstance(value, str) or not re.fullmatch(r'[a-f0-9]{64}', value):
        raise ValueError('missing/invalid digest: '+label)


def check_source(source) -> None:
    if not isinstance(source, dict) or not isinstance(source.get('commit'), str) or not re.fullmatch(r'(?:[a-f0-9]{40}|[a-f0-9]{64})', source['commit']):
        raise ValueError('missing/invalid source commit')
    for field in ('source_sha256', 'lock_sha256'): require_digest(source.get(field), field)
    inventory = source.get('inventory')
    if not isinstance(inventory, list) or not 1 <= len(inventory) <= 10000: raise ValueError('source inventory missing/bounded')
    names = set()
    for item in inventory:
        if not isinstance(item, dict) or not isinstance(item.get('path'), str): raise ValueError('invalid inventory record')
        name = item['path']
        if not name or Path(name).is_absolute() or '..' in Path(name).parts or name in names: raise ValueError('unsafe/duplicate source inventory')
        require_digest(item.get('sha256_lf_text'), name)
        names.add(name)
    if hashlib.sha256(json.dumps(inventory, sort_keys=True).encode()).hexdigest() != source['source_sha256']:
        raise ValueError('source inventory hash differs')
    locks = [item for item in inventory if item['path'] == 'Cargo.lock']
    if len(locks) != 1 or locks[0]['sha256_lf_text'] != source['lock_sha256']: raise ValueError('lock inventory mismatch')


def check_r0(path: Path, expected_os: str) -> dict:
    value = read(path)
    if value.get('track') != 'R0' or value.get('status') != 'passed' or value.get('host_os') != expected_os: raise ValueError('R0 host/gate not passed')
    if expected_os == 'Linux' and value.get('distribution') != {'id':'ubuntu', 'version':'24.04'}: raise ValueError('R0 Ubuntu baseline missing/different')
    check_source(value.get('source'))
    host = value.get('host_target', '')
    if not isinstance(host, str) or ('-linux-' if expected_os == 'Linux' else '-windows-') not in host: raise ValueError('native host target mismatch')
    for name in ('worldwright', 'worldwright-studio'):
        record = value.get('binaries', {}).get(name, {})
        require_digest(record.get('sha256'), 'binary '+name)
        if not isinstance(record.get('path'), str) or not record['path']: raise ValueError('missing native binary path')
    phases = value.get('phases', [])
    if not isinstance(phases, list) or len(phases) > 64: raise ValueError('invalid phase inventory')
    names = set()
    for phase in phases:
        if not isinstance(phase, dict): raise ValueError('invalid phase record')
        name = phase.get('name', '')
        if not re.fullmatch(r'[a-z0-9-]+', name) or name in names: raise ValueError('invalid/duplicate phase')
        names.add(name)
        log = path.parent / (name+'.log')
        if log.stat().st_size > 128 * 1024 * 1024: raise ValueError('phase log bound exceeded')
        if phase.get('status') != 'passed' or phase.get('exit_code') != 0 or log.is_symlink() or hashlib.sha256(log.read_bytes()).hexdigest() != phase.get('log_sha256'): raise ValueError('phase/log incomplete or changed')
    required = REQUIRED | ({'real-first-run'} if expected_os == 'Linux' else set())
    if not required <= names: raise ValueError('R0 required phase missing')
    if value.get('toolchain') != '1.85.1': raise ValueError('native pin mismatch')
    return value


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('ubuntu', 'windows', 'desktop', 'manual'):
        parser.add_argument('--'+name, type=Path, required=True)
    args = parser.parse_args()
    try:
        ubuntu = check_r0(args.ubuntu, 'Linux'); windows = check_r0(args.windows, 'Windows')
        for field in ('commit', 'source_sha256', 'lock_sha256'):
            if ubuntu['source'][field] != windows['source'][field]: raise ValueError('R0 source tuple differs across hosts')
        desktop = read(args.desktop)
        if desktop.get('track') != 'R1' or desktop.get('status') != 'automated_passed_manual_pending': raise ValueError('real installed R1 automation not passed')
        if desktop.get('distribution') != {'id':'ubuntu', 'version':'24.04'}: raise ValueError('R1 Ubuntu baseline missing/different')
        for field in ('source_sha256', 'lock_sha256'):
            if desktop.get(field) != ubuntu['source'][field]: raise ValueError('R1 package uses different source/lock')
        if desktop.get('r0_report_sha256') != hashlib.sha256(args.ubuntu.read_bytes()).hexdigest(): raise ValueError('package R0 report identity differs')
        for name in ('worldwright', 'worldwright-studio'):
            if desktop.get('package_binaries', {}).get(name) != ubuntu.get('binaries', {}).get(name, {}).get('sha256'): raise ValueError('package binary differs from native R0')
        require_digest(desktop.get('package_manifest_sha256'), 'package manifest')
        case_list = desktop.get('cases')
        if not isinstance(case_list, list) or not case_list or any(not isinstance(case, dict) or case.get('status') != 'passed' for case in case_list): raise ValueError('R1 failed/invalid case')
        cases = {case['case']: case for case in case_list}
        required_cases = {'browser-reference', 'browser-expedition', 'preview-timeout', 'SIGINT', 'SIGTERM', 'SIGHUP', 'Studio-crash-session-cleanup', 'fresh-XDG-readonly-symlink', 'no-checkout-no-Cargo-runtime'}
        if not required_cases <= cases.keys() or any(case.get('status') != 'passed' for case in cases.values()): raise ValueError('R1 automated case missing/failed')
        manual = read(args.manual)
        if manual.get('package_manifest_sha256') != desktop.get('package_manifest_sha256'): raise ValueError('manual package differs')
        manual_cases = manual.get('cases')
        if not isinstance(manual_cases, list) or any(not isinstance(case, dict) for case in manual_cases): raise ValueError('invalid manual records')
        records = {case['case']: case for case in manual_cases}
        if len(records) != len(manual_cases): raise ValueError('duplicate manual case')
        if not MANUAL <= records.keys(): raise ValueError('manual browser/display/Windows matrix incomplete')
        for name in MANUAL:
            case = records[name]
            if case.get('status') != 'passed' or not case.get('reviewer') or not case.get('observed_at') or not case.get('evidence_sha256') or not case.get('browser_or_platform_version'):
                raise ValueError('unobserved/unreviewed manual gate: '+name)
            require_digest(case['evidence_sha256'], 'manual '+name)
            for field in ('reviewer', 'observed_at', 'browser_or_platform_version'):
                if not isinstance(case[field], str) or not case[field].strip(): raise ValueError('empty/invalid manual field: '+field)
        print('R0/R1 evidence structure, logs and tuple validated. Authorized reviewer must authenticate manual evidence; no release/license approval is implied.')
        return 0
    except (OSError, ValueError, KeyError, TypeError) as error:
        print('R0/R1 NOT CLOSED: '+str(error), file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
