"""Repository-local storage for R6/R7 collection and native preflight.
This redirects cooperative tools; it is not an OS filesystem sandbox.
Native tool selection and override/config guards live in native_paths.py.
"""
from pathlib import Path
import os
import stat
import sys
import tempfile

ROOT = Path(__file__).absolute().parents[1]
LOCAL = ROOT / '.worldwright-local'


def reject_links(path):
    path = Path(path).absolute()
    for item in (path, *path.parents):
        try:
            info = item.lstat()
        except FileNotFoundError:
            continue
        if stat.S_ISLNK(info.st_mode) or getattr(info, 'st_file_attributes', 0) & 0x400:
            raise ValueError('linked/reparse path refused: ' + str(item))
    return path


def local_path(path):
    path = reject_links(path)
    root = reject_links(LOCAL).resolve()
    resolved = path.resolve()
    if resolved == root or not resolved.is_relative_to(root):
        raise ValueError('generated path must be strictly inside ' + str(LOCAL))
    return resolved


def new_output(path, forbidden=()):
    path = local_path(path)
    if path.exists() or not path.parent.is_dir():
        raise ValueError('new output with existing local parent required')
    for item in forbidden:
        other = Path(item).resolve()
        if path.is_relative_to(other) or other.is_relative_to(path):
            raise ValueError('output and input directories must be disjoint')
    return path


def configure_tools_environment():
    """Set process/child settings before tempfile, Git or Playwright operations."""
    reject_links(ROOT)
    if os.name == 'nt' and ROOT.resolve() != Path('E:/Hold/Worldwright').resolve():
        raise ValueError('only E:/Hold/Worldwright is authorized on this host')
    if not LOCAL.is_dir():
        raise ValueError('repository-local layout must exist before collection')
    paths = {
        'TEMP': 'temp', 'TMP': 'temp', 'TMPDIR': 'temp',
        'HOME': 'tool-home', 'USERPROFILE': 'tool-home',
        'LOCALAPPDATA': 'tool-home/AppData/Local', 'APPDATA': 'tool-home/AppData/Roaming',
        'XDG_CACHE_HOME': 'tool-home/xdg-cache', 'XDG_CONFIG_HOME': 'tool-home/xdg-config',
        'XDG_DATA_HOME': 'tool-home/xdg-data', 'XDG_STATE_HOME': 'tool-home/xdg-state',
        'XDG_RUNTIME_DIR': 'tool-home/xdg-runtime',
        'PIP_CACHE_DIR': 'pip-cache', 'PLAYWRIGHT_BROWSERS_PATH': 'browsers',
        'npm_config_cache': 'npm-cache', 'CARGO_HOME': 'cargo-home',
        'RUSTUP_HOME': 'rustup-home', 'CARGO_TARGET_DIR': 'target',
    }
    os.umask(0o077)
    for relative in sorted(set(paths.values()) | {'evidence'}):
        path = local_path(LOCAL / relative)
        path.mkdir(mode=0o700, parents=True, exist_ok=True)
        if not path.is_dir():
            raise ValueError('local runtime directory required: ' + str(path))
    for name, relative in paths.items():
        os.environ[name] = str(local_path(LOCAL / relative))
    for name in ('PYTHONHOME', 'PYTHONPATH', 'PYTHONPYCACHEPREFIX', 'PIP_TARGET',
                 'PIP_PREFIX', 'PIP_USER', 'PIP_LOG', 'PIP_BUILD_TRACKER',
                 'GIT_DIR', 'GIT_WORK_TREE', 'GIT_INDEX_FILE', 'GIT_OBJECT_DIRECTORY',
                 'GIT_ALTERNATE_OBJECT_DIRECTORIES', 'GIT_CONFIG', 'GIT_CONFIG_COUNT',
                 'GIT_CONFIG_PARAMETERS', 'GIT_EXTERNAL_DIFF', 'GIT_TEMPLATE_DIR', 'NODE_OPTIONS', 'GIT_TRACE',
                 'GIT_TRACE_SETUP', 'GIT_TRACE_PERFORMANCE', 'GIT_TRACE_PACKET',
                 'GIT_TRACE_CURL', 'GIT_TRACE2', 'GIT_TRACE2_EVENT', 'GIT_TRACE2_PERF'):
        os.environ.pop(name, None)
    for name in tuple(os.environ):
        if name.upper().startswith('GIT_TRACE'):
            os.environ.pop(name, None)
    os.environ.update(PYTHONDONTWRITEBYTECODE='1', PYTHONNOUSERSITE='1',
                      PIP_CONFIG_FILE=os.devnull, CARGO_TERM_COLOR='never',
                      GIT_OPTIONAL_LOCKS='0', GIT_TERMINAL_PROMPT='0', GIT_CONFIG_NOSYSTEM='1',
                      GIT_CONFIG_GLOBAL=str(LOCAL / 'tool-home' / 'gitconfig'))
    os.environ['PSModuleAnalysisCachePath'] = str(local_path(LOCAL / 'tool-home' / 'powershell-module-analysis-cache'))
    os.environ['DOTNET_CLI_HOME'] = str(local_path(LOCAL / 'tool-home'))
    os.environ['DOTNET_CLI_TELEMETRY_OPTOUT'] = '1'
    sys.dont_write_bytecode = True
    tempfile.tempdir = str(LOCAL / 'temp')
    return {name: os.environ[name] for name in sorted(paths)}
