"""Exact installed-version checks derived from reviewed requirement files.
No installation, broad version acceptance or historical-evidence rewriting.
"""
from pathlib import Path
import hashlib
import importlib.metadata
import re

ROOT = Path(__file__).resolve().parents[1]
PROFILES = {
    'collector': ('scripts/r6-requirements.txt', 'scripts/r7-requirements.txt'),
    'security': ('scripts/r1-requirements.txt', 'scripts/r2-requirements.txt',
                 'scripts/r5-requirements.txt', 'scripts/r6-requirements.txt',
                 'scripts/r7-requirements.txt', 'deploy/helm/worldwright/tests/requirements.txt'),
}
# Advisory-set minimums, not a promise against future advisories.
SECURITY_FLOORS = {'pyjwt': (2, 15, 0), 'cryptography': (50, 0, 0)}


def normalized(name):
    return re.sub(r'[-_.]+', '-', name).lower()


def merge_pins(texts):
    pins = {}
    for text in texts:
        for line in text.splitlines():
            line = line.strip()
            if not line or line.startswith('#'):
                continue
            match = re.fullmatch(r'([A-Za-z0-9_.-]+)(\[binary\])?==([0-9]+(?:\.[0-9]+)+)', line)
            if not match:
                raise ValueError('Only exact stable pins are supported in policy source files')
            name, extra, version = match.groups(); name = normalized(name)
            names = [name]
            if extra:
                if name != 'psycopg':
                    raise ValueError('Unreviewed requirement extra')
                names.append('psycopg-binary')
            for package in names:
                if package in pins and pins[package] != version:
                    raise ValueError('Conflicting exact dependency pins: ' + package)
                if package in SECURITY_FLOORS and tuple(map(int, version.split('.'))) < SECURITY_FLOORS[package]:
                    raise ValueError('Dependency below reviewed security floor: ' + package)
                pins[package] = version
    if not pins:
        raise ValueError('Empty dependency policy')
    return dict(sorted(pins.items()))


def expected(profile='collector'):
    return merge_pins([(ROOT / relative).read_text(encoding='utf-8') for relative in PROFILES[profile]])


def require_installed(profile='collector'):
    pins = expected(profile)
    actual = {name: importlib.metadata.version(name) for name in pins}
    if actual != pins:
        differences = [name + ': expected ' + pins[name] + ', installed ' + actual[name]
                       for name in pins if actual[name] != pins[name]]
        raise ValueError('Python dependency mismatch: ' + '; '.join(differences))
    return actual


def policy_inventory(profile='security'):
    return [{'path': p, 'sha256': hashlib.sha256((ROOT / p).read_bytes()).hexdigest()}
            for p in PROFILES[profile]]
