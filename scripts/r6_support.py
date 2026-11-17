"""R6 collection primitives. Reports record execution; they never grant approval."""
from pathlib import Path
import hashlib
import json
import os
import re
import time
import r3_io

ROOT = Path(__file__).resolve().parents[1]
MAX_FILE = 64 * 1024 * 1024


def require(value, message):
    if not value:
        raise ValueError(message)


def sha(path):
    path = Path(path)
    require(path.is_file() and not path.is_symlink(), 'regular non-symlink file required')
    result = hashlib.sha256()
    with path.open('rb') as stream:
        for piece in iter(lambda: stream.read(65536), b''):
            result.update(piece)
    return result.hexdigest()


def read(path):
    path = Path(path)
    require(path.is_file() and not path.is_symlink() and path.stat().st_size <= MAX_FILE, 'bounded regular file required')
    with path.open('rb') as stream:
        data = stream.read(MAX_FILE + 1)
    require(len(data) <= MAX_FILE, 'file grew beyond bound')
    return data


def new_file(path, data):
    with os.fdopen(os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600), 'wb') as stream:
        stream.write(data)
        stream.flush()
        os.fsync(stream.fileno())


def encode(value):
    return (json.dumps(value, indent=2, sort_keys=True) + '\n').encode()


def safe_path(base, relative):
    require(isinstance(relative, str) and relative and '\\' not in relative and ':' not in relative, 'invalid evidence path')
    path = Path(relative)
    require(not path.is_absolute() and '..' not in path.parts, 'contained evidence path required')
    candidate = base / path
    require(candidate.resolve().is_relative_to(base.resolve()), 'evidence escapes directory')
    for parent in (candidate, *candidate.parents):
        require(not parent.is_symlink(), 'symlink evidence path refused')
        if parent == base:
            break
    return candidate


def command(argv, timeout=120):
    result = r3_io.collect_phase(argv, timeout, cwd=ROOT)
    require(result['interruption'] is None and type(result['returncode']) is int and result['returncode'] == 0, 'required command failed: ' + argv[0])
    return result['stdout']


def source_identity():
    raw = command(['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard'])
    paths = sorted(set(raw.split(b'\0')) - {b''})
    require(0 < len(paths) <= 10000, 'source inventory bound exceeded')
    items = []
    suffixes = {'.rs', '.toml', '.lock', '.md', '.txt', '.json', '.yaml', '.yml', '.sh', '.ps1', '.py', '.cjs', '.js', '.css', '.html'}
    for item in paths:
        relative = os.fsdecode(item)
        path = safe_path(ROOT, relative)
        data = read(path)
        if path.suffix.lower() in suffixes or path.name in {'.gitignore', '.gitattributes'}:
            data = data.replace(b'\r\n', b'\n')
        items.append({'path': relative, 'sha256': hashlib.sha256(data).hexdigest()})
    return {'commit': command(['git', 'rev-parse', 'HEAD']).decode().strip(),
            'dirty': bool(command(['git', 'status', '--porcelain'])),
            'inventory': items, 'source_sha256': hashlib.sha256(encode(items)).hexdigest(),
            'recipe': 'r6-source-inventory-v1-not-r0-interchangeable'}


def test_listing(data):
    names = re.findall(r'^(.+): test\r?$', data.decode('utf-8', errors='strict'), re.M)
    require(names and len(names) == len(set(names)), 'empty/duplicate native test discovery')
    return names


def test_result(data, minimum):
    summaries = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', data.decode('utf-8', errors='strict'))
    require(summaries and sum(int(s[0]) for s in summaries) >= minimum, 'missing/zero/incomplete executed test results')
    require(all(int(s[1]) == 0 and int(s[2]) == 0 for s in summaries), 'failed/ignored required test cannot pass')
    return sum(int(s[0]) for s in summaries)


class Collector:
    def __init__(self, output, kind):
        self.output = Path(output).absolute()
        require(not self.output.exists() and not self.output.is_symlink(), 'new output directory required')
        require(not self.output.resolve().is_relative_to(ROOT.resolve()), 'evidence must be outside checkout')
        require(self.output.parent.is_dir(), 'existing trusted output parent required')
        for parent in self.output.parents:
            require(not parent.is_symlink(), 'symlink output ancestor refused')
        os.umask(0o077)
        self.output.mkdir(mode=0o700)
        self.report = {'schema_version': 2 if kind == 'native-browser-security-prerequisites' else 1, 'track': 'R6', 'kind': kind, 'status': 'failed',
                       'started_at': int(time.time()), 'full_r6_closed': False,
                       'independent_adoption': False, 'phases': [], 'cases': []}
        self.events = self.output / 'events.jsonl'
        new_file(self.events, b'')
        self.sensitive = r3_io.secret_values()
        self.event('started', kind=kind)

    def event(self, event_name, **details):
        with self.events.open('ab') as stream:
            stream.write(r3_io.log_bytes((json.dumps({'event': event_name, **details}) + '\n').encode(), self.sensitive))
            stream.flush()
            os.fsync(stream.fileno())

    def phase(self, name, argv, timeout=120):
        self.event('phase-started', name=name)
        print('R6 phase: ' + name, flush=True)
        result = r3_io.record_phase(self.report, self.output, name, argv, timeout, cwd=ROOT, sensitive=self.sensitive)
        self.report['phases'][-1]['command'] = argv
        self.event('phase-finished', **self.report['phases'][-1])
        require(result['interruption'] is None and type(result['returncode']) is int and result['returncode'] == 0, 'phase failed/interrupted: ' + name)
        return result['stdout']

    def case(self, name, **details):
        require(not any(c['case'] == name for c in self.report['cases']), 'duplicate case')
        value = {'case': name, 'status': 'passed', **details}
        self.report['cases'].append(value)
        self.event('case', **value)

    def fail(self, error):
        message = r3_io.log_bytes((type(error).__name__ + ': ' + str(error)).encode(), self.sensitive).decode(errors='replace')
        self.report['status'] = 'failed'
        self.report['failure'] = message[:1000]
        self.event('failed', failure=message[:1000])

    def finish(self):
        files = []
        try:
            for path in sorted(self.output.rglob('*')):
                require(not path.is_symlink(), 'symlink in evidence output')
                if path.is_file():
                    require(len(files) < 1000 and path.stat().st_size <= MAX_FILE, 'evidence inventory bounds')
                    files.append({'path': path.relative_to(self.output).as_posix(), 'sha256': sha(path)})
        except (OSError, ValueError) as error:
            self.report['status'] = 'failed'
            self.report['inventory_failure'] = type(error).__name__ + ': ' + str(error)
            # Preserve the failed report; incomplete inventories never validate.
        self.report['files'] = files
        self.report['finished_at'] = int(time.time())
        new_file(self.output / 'r6-report.json', encode(self.report))
        print('R6 ' + self.report['status'] + ': ' + str(self.output / 'r6-report.json'), flush=True)
