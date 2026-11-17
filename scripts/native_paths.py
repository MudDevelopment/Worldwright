"""Pinned native paths and configuration guards; no tool downloads or builds."""
from pathlib import Path
import hashlib
import os
import platform
import shutil
import tomllib
import worldwright_local as local

PIN = '1.85.1'
TOOLS = ('cargo', 'rustc', 'rustdoc', 'rustfmt', 'cargo-fmt', 'cargo-clippy', 'clippy-driver')


def host():
    machine = platform.machine().lower()
    if os.name == 'nt' and machine in ('amd64', 'x86_64'):
        return 'x86_64-pc-windows-msvc'
    if platform.system() == 'Linux' and machine in ('x86_64', 'aarch64'):
        return machine + '-unknown-linux-gnu'
    raise ValueError('native storage setup supports Windows x64 and Linux x64/arm64 only')


def toolchain_path():
    return local.local_path(local.LOCAL / 'rustup-home' / 'toolchains' / (PIN + '-' + host()))


def drive_path_spelling(raw):
    """Convert only an unambiguous extended DOS drive spelling, not device/UNC paths.
    Filesystem identity must still be checked before accepting the conversion.
    """
    raw = os.fspath(raw)
    if raw.startswith('\\\\.\\'):
        raise ValueError('device namespace refused')
    if not raw.startswith('\\\\?\\'):
        return raw
    tail = raw[4:]
    if len(tail) < 3 or tail[0] not in 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz' or tail[1:3] != ':\\':
        raise ValueError('only extended absolute drive paths are supported')
    if '/' in tail or ':' in tail[2:] or any(part in ('.', '..') or part.endswith((' ', '.')) for part in tail[3:].split('\\') if part):
        raise ValueError('ambiguous extended drive path refused')
    return tail


def existing_local_path(path):
    """Resolve an existing local entry without confusing a Windows namespace prefix
    with a different file. Never accept outside-root aliases, links or ADS paths.
    """
    raw = os.fspath(path)
    spelling = drive_path_spelling(raw) if os.name == 'nt' else raw
    candidate = Path(spelling)
    if not candidate.is_absolute():
        raise ValueError('absolute native path required')
    if os.name == 'nt' and any(':' in part for part in candidate.parts[1:]):
        raise ValueError('alternate data stream path refused')
    canonical = local.local_path(candidate).resolve(strict=True)
    original = local.reject_links(Path(raw))
    if not original.samefile(canonical):
        raise ValueError('native namespace spelling identifies a different entry')
    return canonical


def same_local_entry(observed, expected):
    left = existing_local_path(observed)
    right = existing_local_path(expected)
    return left.samefile(right)


def hash_file(path):
    digest = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for piece in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(piece)
    return digest.hexdigest()


def validate_configuration(value):
    aliases = {
        'check-all': 'check --workspace --all-targets --locked --offline',
        'test-all': 'test --workspace --locked --offline',
        'fmt-check': 'fmt --all -- --check',
        'lint': 'clippy --workspace --all-targets --all-features --locked --offline -- -D warnings',
    }
    if set(value) - {'alias'} or not isinstance(value.get('alias', {}), dict):
        raise ValueError('only reviewed alias-only Cargo configuration is enabled')
    if any(aliases.get(name) != command for name, command in value.get('alias', {}).items()):
        raise ValueError('unreviewed Cargo alias')


def configuration_inventory():
    """Fail on unreviewed config routing/wrappers before invoking Cargo.
    The checked-in file currently contains aliases only. Private registries,
    target overrides or custom linker config need explicit source review.
    """
    files = []
    candidates = [p / '.cargo' / name for p in (local.ROOT, *local.ROOT.parents)
                  for name in ('config', 'config.toml')]
    candidates += [local.LOCAL / 'cargo-home' / name for name in ('config', 'config.toml')]
    for path in candidates:
        local.reject_links(path)
        if not path.exists():
            continue
        if not path.resolve().is_relative_to(local.ROOT.resolve()):
            raise ValueError('external ancestor Cargo config requires review: ' + str(path))
        if not path.is_file() or path.stat().st_size > 1024 * 1024:
            raise ValueError('bounded regular Cargo configuration required')
        data = path.read_bytes()
        value = tomllib.loads(data.decode('utf-8'))
        try:
            validate_configuration(value)
        except ValueError as error:
            raise ValueError(str(error) + ': ' + str(path)) from error
        files.append({'path': str(path), 'sha256': hashlib.sha256(data).hexdigest()})
    return files


def reject_overrides(environment):
    for key in ('RUSTC', 'RUSTDOC', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER',
                'RUSTC_BOOTSTRAP', 'RUSTFLAGS', 'RUSTDOCFLAGS', 'CARGO_ENCODED_RUSTFLAGS',
                'CARGO_ENCODED_RUSTDOCFLAGS', 'WORLDWRIGHT_R5_SESSION_TEST_ROOT'):
        if environment.get(key):
            raise ValueError('unreviewed native override: ' + key)
    allowed = {'CARGO_HOME', 'CARGO_TARGET_DIR', 'CARGO_BUILD_TARGET', 'CARGO_TERM_COLOR', 'CARGO_NET_OFFLINE'}
    for key, value in environment.items():
        if value and key.startswith('CARGO_') and key not in allowed:
            raise ValueError('unreviewed Cargo environment setting: ' + key)


def configure_native(target=None):
    reject_overrides(os.environ)
    paths = local.configure_tools_environment()
    requested = local.local_path(target or local.LOCAL / 'target')
    # Protect old workspaces, evidence, caches and interpreter from build output.
    if requested != (local.LOCAL / 'target').resolve():
        raise ValueError('native target must be .worldwright-local/target')
    chain = toolchain_path()
    extension = '.exe' if os.name == 'nt' else ''
    binaries = {}
    for name in TOOLS:
        path = local.local_path(chain / 'bin' / (name + extension))
        if not path.is_file():
            raise ValueError('missing local pinned tool: ' + str(path) + '; run native-storage.py prepare')
        binaries[name] = {'path': str(path), 'sha256': hash_file(path)}
    manifest = tomllib.loads((local.ROOT / 'rust-toolchain.toml').read_text(encoding='utf-8'))
    if manifest.get('toolchain', {}).get('channel') != PIN:
        raise ValueError('repository toolchain pin changed; review required')
    configurations = configuration_inventory()
    # Put real installed binaries first, not global rustup proxy shims. Nested
    # SDK tests use env!("CARGO") and rustc from PATH, so both must resolve locally.
    original = os.environ.get('PATH', '')
    os.environ['PATH'] = str(chain / 'bin') + os.pathsep + original
    os.environ.update(CARGO_TARGET_DIR=str(requested), CARGO_BUILD_TARGET=host(),
                      CARGO_NET_OFFLINE='true', RUSTUP_TOOLCHAIN=PIN + '-' + host(),
                      RUSTUP_AUTO_INSTALL='0')
    for name, item in binaries.items():
        resolved = shutil.which(name)
        if not resolved or not same_local_entry(resolved, item['path']):
            raise ValueError('tool resolves outside pinned local bin: ' + name)
    paths['CARGO_TARGET_DIR'] = str(requested)
    return {'schema_version': 1, 'policy': 'repository-local-native-v1',
            'host': host(), 'toolchain': PIN, 'sysroot': str(chain),
            'target': str(requested), 'paths': paths, 'binaries': binaries,
            'cargo_configurations': configurations,
            'scope': 'cooperative tool path configuration, not OS filesystem isolation or release approval'}
