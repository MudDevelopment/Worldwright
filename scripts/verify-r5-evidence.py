"""Read-only R5 selected-text/log integrity check. No build, replay or closure.
Python 3.11+, standard library only. The supplied unsigned report is not a trust anchor.
"""
from pathlib import Path, PurePosixPath
import argparse
import hashlib
import json
import re
import sys

LIMIT = 8 * 1024 * 1024
REPORT = 'docs/verification/r5-local-observations.json'


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('duplicate JSON key')
        result[key] = value
    return result


def contained(base: Path, name: str) -> Path:
    if not isinstance(name, str) or not name or '\\' in name or ':' in name:
        raise ValueError('invalid relative path')
    parts = PurePosixPath(name)
    if parts.is_absolute() or any(p in ('.', '..') for p in name.split('/')):
        raise ValueError('path must be relative and contained')
    path = (base / name).resolve(strict=True)
    if not path.is_relative_to(base.resolve()):
        raise ValueError('path escapes allowed root')
    return path


def read_bytes(path: Path) -> bytes:
    if not path.is_file() or path.stat().st_size > LIMIT:
        raise ValueError('expected bounded regular file')
    with path.open('rb') as stream:
        data = stream.read(LIMIT + 1)
    if len(data) > LIMIT:
        raise ValueError('file exceeds read bound')
    return data


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def lf_text(data: bytes) -> bytes:
    # Matches Python UTF-8 universal-newline text readbacks. Preserve final LF,
    # whitespace and BOM; do not trim or otherwise canonicalize content.
    return data.decode('utf-8').replace('\r\n', '\n').replace('\r', '\n').encode('utf-8')


def expected(value) -> str:
    if not isinstance(value, str) or re.fullmatch('[0-9a-f]{64}', value) is None:
        raise ValueError('invalid SHA-256 field')
    return value


def verify(root: Path, report_name: str = REPORT) -> bool:
    report_path = contained(root, report_name)
    report = json.loads(read_bytes(report_path), object_pairs_hook=unique_object)
    if not isinstance(report, dict) or type(report.get('schema_version')) is not int or report['schema_version'] != 1 or report.get('track') != 'R5':
        raise ValueError('unsupported report profile')
    if report.get('native_rust_executed') is not False or report.get('full_r5_closed') is not False:
        raise ValueError('not the scoped non-native/open R5 observation profile')
    subjects = report.get('subjects')
    if not isinstance(subjects, list) or not 1 <= len(subjects) <= 100:
        raise ValueError('invalid subject inventory')
    seen = set()
    jobs = []
    for subject in subjects:
        if not isinstance(subject, dict):
            raise ValueError('invalid subject record')
        name = subject.get('path')
        path = contained(root, name)
        if path in seen:
            raise ValueError('duplicate subject path')
        seen.add(path)
        jobs.append((name, path, expected(subject.get('sha256_lf_text')), True))
    log = report.get('log')
    if not isinstance(log, dict):
        raise ValueError('missing log reference')
    jobs.append(('evidence log', contained(report_path.parent, log.get('path')), expected(log.get('sha256')), False))
    matches = True
    for name, path, wanted, normalize in jobs:
        data = read_bytes(path)
        actual = digest(lf_text(data) if normalize else data)
        good = actual == wanted
        matches = matches and good
        print(('MATCH ' if good else 'MISMATCH ') + name)
        if not good:
            print('  expected=' + wanted + '\n  actual=' + actual)
            if not normalize and digest(lf_text(data)) == wanted:
                print('  LF-normalized log matches, but BYTE-EXACT integrity still FAILS. Preserve the file; do not rewrite the report.')
    print(f'Subjects={len(subjects)}; log=1; supplied unsigned report only.')
    print('No native execution, signature, full checkout or acceptance closure is established.')
    return matches


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument('--report', default=REPORT, help='root-relative report path')
    args = parser.parse_args()
    try:
        return 0 if verify(args.root.resolve(strict=True), args.report) else 1
    except (OSError, ValueError, TypeError, KeyError) as error:
        print('INVALID INPUT: ' + type(error).__name__ + ': ' + str(error), file=sys.stderr)
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
