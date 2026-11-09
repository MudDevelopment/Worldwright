"""Real installed-package Chromium/HTTP/process acceptance, not manual R1 closure.
Python 3.11+, approved Playwright and browser. No Cargo or source-checkout runtime.
"""
from pathlib import Path
import argparse
import hashlib
import importlib.util
import json
import os
import platform
import re
import signal
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]


def load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
    return module


def request(url: str, path: str, value=None):
    headers = {'Origin': url.rstrip('/')}
    if value is not None: headers['Content-Type'] = 'application/json'
    req = urllib.request.Request(url+path, data=None if value is None else json.dumps(value).encode(), headers=headers)
    with urllib.request.urlopen(req, timeout=5) as response:
        body = response.read()
        return None if not body else json.loads(body)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package-directory', type=Path, required=True)
    parser.add_argument('--browser-executable', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    package = args.package_directory.resolve()
    output = args.output.resolve()
    if output.exists() or output.is_relative_to(package) or output.is_relative_to(ROOT): parser.error('new evidence directory outside package and checkout required')
    if not sys.platform.startswith('linux') or os.geteuid() == 0: parser.error('ordinary Ubuntu/Linux user required')
    os.umask(0o077); output.mkdir(mode=0o700)
    report = {'schema_version': 1, 'track': 'R1', 'status': 'failed', 'cases': [], 'manual_status': 'not_checked'}
    process = None
    log = None
    active_session = None
    try:
        distribution = platform.freedesktop_os_release()
        report['distribution'] = {'id':distribution.get('ID'), 'version':distribution.get('VERSION_ID')}
        if report['distribution'] != {'id':'ubuntu', 'version':'24.04'}: raise RuntimeError('actual Ubuntu 24.04 R1 acceptance host required')
        manifest = load('package_desktop', ROOT/'scripts/package-desktop.py').verify(package)
        report['package_manifest_sha256'] = hashlib.sha256((package/'package-manifest.json').read_bytes()).hexdigest()
        report['source_sha256'] = manifest['source_sha256']; report['lock_sha256'] = manifest['lock_sha256']
        report['r0_report_sha256'] = manifest['r0_report_sha256']
        report['package_binaries'] = {record['path'].removeprefix('bin/'): record['sha256'] for record in manifest['files'] if record['path'] in ('bin/worldwright', 'bin/worldwright-studio')}
        from playwright.sync_api import sync_playwright
        if not args.browser_executable.is_file(): raise RuntimeError('approved browser missing')
        cleanup = load('session_cleanup', ROOT/'scripts/cleanup-session.py')
        with tempfile.TemporaryDirectory(prefix='worldwright R1 installed acceptance ') as folder:
            temp = Path(folder); home = temp/'fresh home'; home.mkdir()
            tools = temp/'tool guards'; tools.mkdir()
            trace = temp/'forbidden-tool-trace'
            for name in ('cargo', 'rustc'):
                guard = tools/name; guard.write_text('#!/bin/sh\nprintf invoked > "$R1_FORBIDDEN_TOOL_TRACE"\nexit 99\n'); guard.chmod(0o700)
            env = os.environ.copy(); env.update(HOME=str(home), TMPDIR=str(temp), PATH=str(tools)+os.pathsep+env['PATH'], R1_FORBIDDEN_TOOL_TRACE=str(trace))
            env.pop('XDG_DATA_HOME', None); env.pop('DISPLAY', None); env.pop('WAYLAND_DISPLAY', None)
            workspace = home/'.local/share/worldwright/studio'
            def launch():
                nonlocal process, log, active_session
                log = (output/'launcher.log').open('ab')
                old_size = (output/'launcher.log').stat().st_size
                process = subprocess.Popen(['bash', str(package/'bin/worldwright-desktop'), '--no-browser'], cwd=temp, env=env, stdout=log, stderr=subprocess.STDOUT)
                deadline = time.monotonic()+35
                while time.monotonic() < deadline:
                    if process.poll() is not None: raise RuntimeError('installed launcher failed; see private log')
                    text = (output/'launcher.log').read_bytes()[old_size:].decode(errors='replace')
                    match = re.search(r'^Studio: (http://127\.0\.0\.1:\d+/)$', text, re.M)
                    if match:
                        request(match[1], 'api/v1/workspace')
                        children = Path(f'/proc/{process.pid}/task/{process.pid}/children').read_text().split()
                        if len(children) != 1: raise RuntimeError('cannot identify exact Studio session')
                        pid = int(children[0])
                        if os.getsid(pid) != pid: raise RuntimeError('Studio session isolation failed')
                        active_session = pid
                        return match[1], pid
                    time.sleep(0.05)
                raise RuntimeError('installed readiness timed out')
            def stop(sig, session):
                nonlocal process, log, active_session
                members = cleanup.session_members(session)
                process.send_signal(sig); code = process.wait(timeout=16)
                if code != {signal.SIGINT:130, signal.SIGTERM:143, signal.SIGHUP:129}[sig]: raise RuntimeError('launcher did not complete graceful signal cleanup')
                if cleanup.session_members(session): raise RuntimeError('session process remains after graceful stop')
                log.close(); log=None; process=None; active_session=None
                report['cases'].append({'case': signal.Signals(sig).name, 'status': 'passed', 'tracked_members': len(members)})
            url, session = launch()
            with sync_playwright() as playwright:
                browser = playwright.chromium.launch(executable_path=str(args.browser_executable.resolve()), headless=True)
                report['browser_version'] = browser.version
                page = browser.new_page()
                page.goto(url); page.locator('#namespace').wait_for()
                page.wait_for_function("() => document.querySelector('#revision-status').textContent.includes('Revision 1')")
                draft = {'schema_version':1, 'namespace':'acceptance', 'entry_room':'acceptance:rooms.home', 'rooms':[
                    {'id':'acceptance:rooms.home','title':'Acceptance home','description':'Literal <b>content</b>','exits':{'east':'acceptance:rooms.field'}},
                    {'id':'acceptance:rooms.field','title':'Field','description':'A test field','exits':{'west':'acceptance:rooms.home'}}]}
                imported = temp/'import.json'; imported.write_text(json.dumps(draft))
                page.locator('#import-file').set_input_files(imported)
                page.wait_for_function("() => document.querySelector('#namespace').value === 'acceptance'")
                page.locator('#room-title').fill('Acceptance ✓')
                page.locator('#world-form').evaluate('form => form.requestSubmit()')
                page.wait_for_function("() => document.querySelector('#revision-status').textContent.includes('Revision 2')")
                page.locator('#validate-draft').click()
                page.wait_for_function("() => !document.querySelector('#package-artifact').disabled")
                pinned = request(url, 'api/v1/workspace')
                for game in ('reference', 'expedition'):
                    page.locator('#release-id').fill('r1-'+game); page.locator('#builder-identity').fill('r1-browser')
                    page.locator('#package-game').select_option(game)
                    page.locator('#package-form').evaluate('form => form.requestSubmit()')
                    page.wait_for_function("game => document.querySelector('#artifact-select').selectedOptions[0]?.textContent.includes('r1-' + game)", arg=game)
                    digest = page.locator('#artifact-select').input_value()
                    artifact = workspace/'artifacts'/(digest+'.wwr')
                    before = hashlib.sha256(artifact.read_bytes()).hexdigest()
                    if before != digest: raise RuntimeError('artifact digest mismatch')
                    inspect = subprocess.run([str(package/'bin/worldwright'), 'content', 'inspect', str(artifact), '--json'], capture_output=True, text=True, check=True, timeout=10)
                    if 'worldwright-'+game not in json.dumps(json.loads(inspect.stdout)): raise RuntimeError('wrong game contract')
                    page.locator('#preview-start').click()
                    page.wait_for_function("() => document.querySelector('#preview-output').textContent.includes('Acceptance ✓')", timeout=15000)
                    if game == 'expedition':
                        page.wait_for_function("() => document.querySelector('#preview-output').textContent.includes('Supplies:')")
                        page.locator('#preview-input').fill('forage'); page.locator('#preview-input-form').evaluate('form => form.requestSubmit()')
                        page.wait_for_function("() => document.querySelector('#preview-output').textContent.includes('Foraging recovers')")
                    page.locator('#preview-input').fill('quit'); page.locator('#preview-input-form').evaluate('form => form.requestSubmit()')
                    page.wait_for_function("() => document.querySelector('#preview-pinned').textContent.includes('succeeded')", timeout=15000)
                    status = request(url, 'api/v1/preview')
                    if status['artifact_digest'] != digest or status['draft_revision'] != pinned['revision']: raise RuntimeError('preview identity mismatch')
                    if hashlib.sha256(artifact.read_bytes()).hexdigest() != before: raise RuntimeError('preview mutated artifact')
                    report['cases'].append({'case':'browser-'+game, 'status':'passed', 'artifact_digest':digest, 'snapshot_sha256':pinned['snapshot_sha256']})
                    page.screenshot(path=str(output/(game+'.png')))
                request(url, 'api/v1/preview/start', {'artifact_digest':digest, 'max_seconds':1})
                deadline = time.monotonic()+15
                while time.monotonic() < deadline:
                    timed = request(url, 'api/v1/preview')
                    if timed['state'] != 'running': break
                    time.sleep(0.1)
                if timed['state'] != 'failed' or timed['exit_code'] != 1 or 'exceeded the maximum lifetime' not in timed['output'].replace('[stderr] ', ''): raise RuntimeError('preview timeout did not match the supervisor contract')
                report['cases'].append({'case':'preview-timeout', 'status':'passed', 'exit_code':timed['exit_code']})
                browser.close()
            stop(signal.SIGTERM, session)
            for sig in (signal.SIGINT, signal.SIGHUP):
                url, session = launch()
                if request(url, 'api/v1/workspace')['snapshot_sha256'] != pinned['snapshot_sha256']: raise RuntimeError('reopen changed draft')
                digest = report['cases'][1]['artifact_digest']
                request(url, 'api/v1/preview/start', {'artifact_digest':digest, 'max_seconds':30})
                stop(sig, session)
            url, session = launch()
            request(url, 'api/v1/preview/start', {'artifact_digest':digest, 'max_seconds':30})
            os.kill(session, signal.SIGKILL)
            crash_code = process.wait(timeout=16)
            if crash_code == 0 or cleanup.session_members(session): raise RuntimeError('Studio crash leaked a session or reported success')
            log.close(); log=None; process=None; active_session=None
            report['cases'].append({'case':'Studio-crash-session-cleanup', 'status':'passed', 'launcher_exit':crash_code})
            env['XDG_DATA_HOME'] = str(temp/'fresh XDG with spaces')
            url, session = launch()
            if request(url, 'api/v1/workspace')['revision'] != 1: raise RuntimeError('fresh XDG not isolated')
            stop(signal.SIGTERM, session)
            denied = temp/'read-only parent'; denied.mkdir(); denied.chmod(0)
            try:
                rejected = subprocess.run(['bash', str(package/'bin/worldwright-desktop'), '--no-browser', str(denied/'studio')], cwd=temp, env=env, capture_output=True, timeout=15)
                if rejected.returncode == 0: raise RuntimeError('read-only workspace was accepted')
            finally: denied.chmod(0o700)
            destination = temp/'symlink destination'; destination.mkdir()
            link = temp/'linked XDG'; link.symlink_to(destination, target_is_directory=True)
            env['XDG_DATA_HOME'] = str(link)
            rejected = subprocess.run(['bash', str(package/'bin/worldwright-desktop'), '--no-browser'], cwd=temp, env=env, capture_output=True, timeout=15)
            if rejected.returncode == 0 or list(destination.iterdir()): raise RuntimeError('symlink was followed or accepted')
            report['cases'].append({'case':'fresh-XDG-readonly-symlink', 'status':'passed'})
            if trace.exists(): raise RuntimeError('installed application invoked build tools')
            report['cases'].append({'case':'no-checkout-no-Cargo-runtime', 'status':'passed'})
            report['status'] = 'automated_passed_manual_pending'
    except Exception as error:
        report['failure'] = f'{type(error).__name__}: {error}'
    finally:
        if process is not None and process.poll() is None:
            process.terminate()
            try: process.wait(timeout=16)
            except subprocess.TimeoutExpired: process.kill(); process.wait()
        if active_session is not None:
            try:
                clean = load('failure_cleanup', ROOT/'scripts/cleanup-session.py').cleanup(active_session, 1)
                if not clean: report['cleanup_failure'] = 'failed run required forced session cleanup'
            except (OSError, ValueError, RuntimeError) as error:
                report['cleanup_failure'] = str(error)
        if log is not None: log.close()
        (output/'r1-report.json').write_text(json.dumps(report, indent=2)+'\n')
    print(f"R1 {report['status']}: {output/'r1-report.json'}")
    return 0 if report['status'] == 'automated_passed_manual_pending' else 1


if __name__ == '__main__':
    raise SystemExit(main())
