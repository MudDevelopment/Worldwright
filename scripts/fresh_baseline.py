"""Byte-preserving, explicitly authorized fresh-database baseline transition.
Never connects to a database or rewrites SQLx history. Used by linux-next-stage.py.
"""
from pathlib import Path
import hashlib
import os
import re
import uuid

TARGET='20261106000100_operational_control.sql'
ORIGINAL_SHA384='6ddb32cf0e6f68bb1c0de353b58400896a90c664d0afc6c4401995fb2a8cf221d75f099c9ee16f4f794a394691e0998a'
MIGRATIONS=(
 '20261020000100_durable_gameplay.sql','20261102000100_content_release.sql',
 TARGET,'20261110000100_studio_workflow.sql','20261118000100_game_contracts.sql',
 '20261118000200_durable_game_state.sql','20261118000300_oidc_identity.sql',
 '20261118000400_gameplay_timer_audit.sql','20261119000100_r6_retention_ledger.sql')


def baseline_bytes(root):
    original=Path(root)/'tests/fixtures/migration-history'/TARGET
    if original.is_symlink() or not original.is_file() or original.stat().st_size>1024*1024:
        raise ValueError('bounded preserved historical migration required')
    old=original.read_bytes()
    actual=hashlib.sha384(old).hexdigest()
    if actual!=ORIGINAL_SHA384:
        raise ValueError('historical migration SHA384 differs: expected '+ORIGINAL_SHA384+'; actual '+actual)
    # This is a transformation of one hash-pinned file, not a general SQL parser.
    # Underscore is a word character: authorization_version and the constraint
    # names must remain byte-for-byte unchanged.
    fixed,count=re.subn(rb'\bauthorization\b',b'"authorization"',old)
    if count!=3:
        raise ValueError('historical migration standalone identifier count differs: '+str(count))
    return old,fixed


def inventory(root):
    files=sorted((Path(root)/'migrations').glob('*.sql'))
    if tuple(p.name for p in files)!=MIGRATIONS:raise ValueError('reviewed nine-file migration inventory changed')
    if any(p.is_symlink() for p in files):raise ValueError('linked migration refused')
    return {p.name:hashlib.sha384(p.read_bytes()).hexdigest() for p in files}


def apply_fresh(root,evidence,authorized=False):
    if authorized is not True:raise ValueError('explicit fresh-database authorization required')
    root=Path(root);path=root/'migrations'/TARGET;before=inventory(root);old,fixed=baseline_bytes(root)
    current=path.read_bytes()
    if current not in (old,fixed):raise ValueError('current migration is neither reviewed baseline; no overwrite')
    changed=current==old
    if changed:
        backup=Path(evidence)/('before-'+TARGET)
        with backup.open('xb') as stream:stream.write(current);stream.flush();os.fsync(stream.fileno())
        temp=path.with_name('.'+path.name+'.fresh-'+uuid.uuid4().hex)
        with temp.open('xb') as stream:stream.write(fixed);stream.flush();os.fsync(stream.fileno())
        os.chmod(temp,path.stat().st_mode & 0o777)
        if path.read_bytes()!=old:raise ValueError('migration changed during preparation; stop')
        os.replace(temp,path)
    after=inventory(root)
    if path.read_bytes()!=fixed or any(before[name]!=after[name] for name in before if name!=TARGET):
        raise ValueError('fresh baseline verification failed')
    return {'scope':'fresh databases only; no SQLx history reconciliation','changed':changed,
            'before':before,'after':after,'historical_sha384':ORIGINAL_SHA384,
            'corrected_sha384':hashlib.sha384(fixed).hexdigest(),'sqlx_history_modified':False}
