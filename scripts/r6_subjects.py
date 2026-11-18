"""Retain exact browser/native subjects separately from Cargo's mutable cache.
No hard-link snapshots, cache clearing, downloads or acceptance waivers.
"""
from pathlib import Path
import hashlib
import os
import stat
from r6_support import MAX_FILE, new_file, read, require, safe_path, sha
import worldwright_local as local

POLICY = 'retained-byte-copy-v1'


def retain_binaries(sources, evidence_root):
    root = local.local_path(evidence_root)
    destination = local.new_output(root / 'binaries')
    destination.mkdir(mode=0o700)
    records = {}
    for name, source in sources.items():
        source = local.local_path(source)
        require(source.is_file(), 'binary source must be a regular local file')
        info = source.stat()
        require(0 < info.st_size <= MAX_FILE, 'binary subject exceeds evidence size bound')
        expected = sha(source)
        data = read(source)
        require(hashlib.sha256(data).hexdigest() == expected, 'binary source changed before retention')
        target = destination / source.name
        new_file(target, data)
        if os.name != 'nt':
            target.chmod(0o600 | (info.st_mode & stat.S_IXUSR))
        # Cargo cache files may themselves be hard-linked. Snapshot files must not be.
        require(target.stat().st_nlink == 1 and not source.samefile(target), 'binary snapshot must be an independent byte copy')
        require(sha(source) == expected and sha(target) == expected, 'binary source/copy changed during retention')
        records[name] = {'path': str(target), 'sha256': expected,
                         'retained_relative_path': target.relative_to(root).as_posix(),
                         'build_output_path': str(source), 'copy_method': 'exclusive-byte-copy'}
    return records


def verify_retained(evidence_root, records, check_local=False):
    root = Path(evidence_root)
    require(isinstance(records, dict) and records, 'retained binary inventory missing')
    seen = set()
    for name, record in records.items():
        require(isinstance(record, dict), 'invalid retained binary record')
        relative = record.get('retained_relative_path')
        require(isinstance(relative, str), 'retained binary path missing')
        pieces = Path(relative).parts
        require(len(pieces) == 2 and pieces[0] == 'binaries', 'retained binary must be inside binaries directory')
        require(relative not in seen, 'duplicate retained binary path')
        seen.add(relative)
        path = local.reject_links(safe_path(root, relative))
        require(path.is_file() and path.stat().st_nlink == 1, 'linked or nonregular retained binary')
        require(record.get('copy_method') == 'exclusive-byte-copy', 'unknown binary retention method')
        require(sha(path) == record.get('sha256'), 'retained binary changed: ' + str(name))
        require(isinstance(record.get('path'), str) and isinstance(record.get('build_output_path'), str), 'retained binary provenance missing')
        if check_local:
            original = local.local_path(record['path'])
            require(original.samefile(path), 'local binary path does not identify retained subject')


def cache_observations(records):
    observations = {}
    for name, record in records.items():
        path = local.local_path(record['build_output_path'])
        value = sha(path) if path.is_file() else None
        observations[name] = {'path': str(path), 'sha256_after_cargo_phases': value,
                              'retained_sha256': record['sha256'], 'matches_retained': value == record['sha256']}
    return observations
