"""Run actual Studio/preview binaries with Chromium in a NEW isolated workspace.
No external URL, publication credentials, fake backend or acceptance closure.
"""
from pathlib import Path
import argparse
import json
import os
import re
import signal
import subprocess
import threading
import time
import urllib.error
import urllib.request
from r6_support import Collector, ROOT, new_file, read, require, sha
from r3_io import NoRedirects
import worldwright_local as local

MAX_LOG = 1024 * 1024


def api(url, path, method='GET', value=None, headers=None, expected=200):
    payload = None if value is None else json.dumps(value).encode()
    fields = {'Origin': url, 'Content-Type': 'application/json'}
    fields.update(headers or {})
    request = urllib.request.Request(url + path, data=payload, method=method, headers=fields)
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirects())
    try:
        response = opener.open(request, timeout=5)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        body = response.read(33 * 1024 * 1024 + 1)
        require(len(body) <= 33 * 1024 * 1024, 'HTTP response exceeds bounds')
        require(response.status == expected, f'HTTP {path}: expected {expected}, got {response.status}')
        return json.loads(body) if body else None


class Studio:
    def __init__(self, collector, binary, preview, workspace, name):
        import psutil
        self.psutil = psutil
        self.collector = collector
        self.workspace = workspace
        self.data = bytearray()
        self.overflow = False
        self.reader_error = False
        self.lock = threading.Lock()
        self.name = name
        env = {key: value for key, value in os.environ.items() if key.upper() in {
            'PATH', 'SYSTEMROOT', 'WINDIR', 'TEMP', 'TMP', 'TMPDIR', 'HOME', 'USERPROFILE', 'LOCALAPPDATA',
            'APPDATA', 'XDG_CACHE_HOME', 'XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME', 'XDG_RUNTIME_DIR',
            'PSMODULEANALYSISCACHEPATH'}}
        options = {'creationflags': subprocess.CREATE_NEW_PROCESS_GROUP} if os.name == 'nt' else {}
        self.process = subprocess.Popen([str(binary), 'serve', '--workspace', str(workspace),
            '--preview-command', str(preview), '--bind', '127.0.0.1:0'], cwd=collector.output,
            env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, **options)
        self.identity = psutil.Process(self.process.pid)
        self.saved = False
        self.thread = threading.Thread(target=self.drain, daemon=True)
        self.thread.start()

    def drain(self):
        try:
            while True:
                piece = self.process.stdout.read1(4096)
                if not piece:
                    break
                with self.lock:
                    room = MAX_LOG - len(self.data)
                    self.data.extend(piece[:room])
                    self.overflow |= len(piece) > room
        except OSError:
            self.reader_error = True
        finally:
            self.process.stdout.close()

    def ready(self):
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            require(self.process.poll() is None and not self.overflow and not self.reader_error, 'Studio exited or output capture failed')
            with self.lock:
                text = bytes(self.data).decode(errors='replace')
            match = re.search(r'^Worldwright Studio: (http://127\.0\.0\.1:([0-9]+)/)$', text, re.M)
            if match:
                url = match[1].rstrip('/')
                try:
                    api(url, '/api/v1/workspace')
                    return url
                except (OSError, urllib.error.URLError):
                    pass
            time.sleep(0.05)
        raise RuntimeError('Studio HTTP readiness timed out')

    def stop(self, graceful):
        ps = self.psutil
        children = []
        if self.process.poll() is None:
            children = self.identity.children(recursive=True)
            if graceful:
                require(os.name != 'nt', 'Windows Ctrl+C requires the separate manual console check')
                self.process.send_signal(signal.SIGINT)
            else:
                self.process.terminate()
            try:
                self.process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                for child in children:
                    try: child.kill()
                    except ps.NoSuchProcess: pass
                self.process.kill()
                self.process.wait(timeout=5)
                raise RuntimeError('Studio cleanup deadline exceeded; forced exit is not graceful')
        _, alive = ps.wait_procs(children, timeout=3)
        if alive:
            for child in alive:
                try: child.kill()
                except ps.NoSuchProcess: pass
            ps.wait_procs(alive, timeout=3)
            raise RuntimeError('preview descendants survived shutdown; forced cleanup used')
        self.thread.join(timeout=3)
        require(not self.thread.is_alive() and not self.overflow and not self.reader_error, 'Studio output not completely captured')
        if not self.saved:
            new_file(self.collector.output / (self.name + '.log'), bytes(self.data))
            self.saved = True
        if graceful:
            require(self.process.returncode == 0, 'Studio graceful exit was nonzero')
            require(not (self.workspace / '.worldwright-studio-session').exists(), 'session sentinel remained after graceful exit')
        return self.process.returncode


def workflow(collector, studio, preview, browser_executable=None):
    from playwright.sync_api import sync_playwright
    workspace = collector.output / 'workspace'
    proc = Studio(collector, studio, preview, workspace, 'studio-first')
    errors = []
    requests_outside = []
    try:
        url = proc.ready()
        collector.case('real-http-ready', url=url)
        with sync_playwright() as engine:
            opts = {'headless': True}
            if browser_executable is not None:
                opts['executable_path'] = str(browser_executable)
            browser = engine.chromium.launch(**opts)
            collector.report['browser_version'] = browser.version
            context = browser.new_context(viewport={'width': 1440, 'height': 1000})
            allowed = [url]
            def route(request):
                if any(request.request.url.startswith(origin + '/') for origin in allowed):
                    request.continue_()
                else:
                    requests_outside.append(request.request.url[:200])
                    request.abort()
            context.route('**/*', route)
            page = context.new_page()
            page.set_default_timeout(15000)
            page.on('pageerror', lambda error: errors.append(str(error)[:500]))
            page.goto(url)
            page.wait_for_function("() => document.querySelector('#revision-status').textContent.includes('Revision 1')")
            draft = {'schema_version': 1, 'namespace': 'r6', 'entry_room': 'r6:rooms.home', 'rooms': [
                {'id': 'r6:rooms.home', 'title': 'Home', 'description': 'Literal <b>text</b>', 'exits': {'east': 'r6:rooms.field'}},
                {'id': 'r6:rooms.field', 'title': 'Field', 'description': 'Test field', 'exits': {'west': 'r6:rooms.home'}}]}
            imported = collector.output / 'import.json'
            new_file(imported, json.dumps(draft).encode())
            page.locator('#import-file').set_input_files(imported)
            page.wait_for_function("() => document.querySelector('#namespace').value === 'r6'")
            page.locator('#room-title').fill('R6 saved \u2713')
            page.locator('#world-form').evaluate('form => form.requestSubmit()')
            page.wait_for_function("() => document.querySelector('#revision-status').textContent.includes('Revision 2')")
            pinned = api(url, '/api/v1/workspace')
            require(pinned['draft']['rooms'][0]['title'] == 'R6 saved \u2713', 'saved editor content differs')
            page.reload()
            page.wait_for_function("() => document.querySelector('#room-title').value === 'R6 saved \u2713'")
            page.locator('#validate-draft').click()
            page.wait_for_function("() => !document.querySelector('#package-artifact').disabled")
            collector.case('browser-import-edit-save-reload-validate', snapshot_sha256=pinned['snapshot_sha256'], revision=pinned['revision'])
            artifacts = []
            for game in ('reference', 'expedition'):
                page.locator('#release-id').fill('r6-' + game)
                page.locator('#builder-identity').fill('r6-browser-collector')
                page.locator('#package-game').select_option(game)
                page.locator('#package-form').evaluate('form => form.requestSubmit()')
                page.wait_for_function("game => document.querySelector('#artifact-select').selectedOptions[0]?.textContent.includes('r6-' + game)", arg=game)
                page.wait_for_function("() => !document.querySelector('#preview-start').disabled")
                digest = page.locator('#artifact-select').input_value()
                require(re.fullmatch('[0-9a-f]{64}', digest), 'invalid artifact digest')
                path = workspace / 'artifacts' / (digest + '.wwr')
                require(sha(path) == digest, 'artifact bytes differ from identifier')
                identity = api(url, '/api/v1/artifacts/' + digest + '/identity')
                require(identity['game'] == game and identity['payload_digest'] == pinned['snapshot_sha256'] and identity['draft_revision'] == pinned['revision'], 'wrong artifact game/revision/payload')
                page.locator('#preview-start').click()
                page.wait_for_function("() => document.querySelector('#preview-output').textContent.includes('R6 saved \u2713')")
                if game == 'expedition':
                    page.wait_for_function("() => document.querySelector('#preview-output').textContent.includes('Supplies:')")
                    text, expected = 'forage', 'Foraging recovers'
                else:
                    text, expected = 'say r6-browser-check', 'r6-browser-check'
                page.locator('#preview-input').fill(text)
                page.locator('#preview-input-form').evaluate('form => form.requestSubmit()')
                page.wait_for_function('text => document.querySelector("#preview-output").textContent.includes(text)', arg=expected)
                page.locator('#preview-input').fill('quit')
                page.locator('#preview-input-form').evaluate('form => form.requestSubmit()')
                page.wait_for_function("() => document.querySelector('#preview-pinned').textContent.includes('succeeded')")
                state = api(url, '/api/v1/preview')
                require(state['state'] == 'succeeded' and state['artifact_digest'] == digest and state['snapshot_sha256'] == pinned['snapshot_sha256'], 'successful preview identity differs')
                require(sha(path) == digest, 'preview mutated artifact')
                artifacts.append(digest)
                page.screenshot(path=str(collector.output / (game + '.png')), animations='disabled')
                collector.case('real-preview-' + game, artifact_digest=digest, payload_digest=identity['payload_digest'])
            auth = api(url, '/api/v1/publication-auth')
            require(auth == {'schema_version': 1, 'configured': False, 'renewal': 'disabled', 'automatic_retry': False}, 'unexpected publication authority')
            require(page.locator('#submit-publication').is_disabled(), 'publication enabled without authority')
            api(url, '/api/v1/capabilities', headers={'Host': 'attacker.invalid'}, expected=421)
            api(url, '/api/v1/preview/stop', method='POST', value={}, headers={'Origin': 'http://attacker.invalid'}, expected=403)
            api(url, '/api/v1/preview/stop', method='POST', value={}, headers={'Content-Type': 'text/plain'}, expected=415)
            api(url, '/api/v1/publications/' + '1' * 32 + '/reconcile', method='POST', value={}, expected=503)
            collector.case('real-host-origin-content-type-authority-denial')
            require(not errors and not requests_outside, 'browser errors or nonlocal requests observed')
            collector.case('browser-no-script-errors-or-external-requests')
            if os.name == 'nt':
                code = proc.stop(graceful=False)
                collector.report['windows_shutdown'] = {'status': 'manual_pending', 'forced_cleanup_exit': code,
                    'reason': 'No synthetic CTRL_BREAK is accepted as the application Ctrl+C contract. Sentinel retained.'}
                collector.report['status'] = 'browser_workflow_passed_windows_shutdown_open'
            else:
                # Exercise graceful shutdown with an active native preview.
                api(url, '/api/v1/preview/start', method='POST', value={'artifact_digest': artifacts[-1], 'max_seconds': 30})
                proc.stop(graceful=True)
                collector.case('unix-graceful-active-preview-shutdown')
                proc = Studio(collector, studio, preview, workspace, 'studio-reopen')
                reopened = proc.ready()
                allowed.append(reopened)
                page.goto(reopened)
                page.wait_for_function("() => document.querySelector('#room-title').value === 'R6 saved \u2713'")
                require(api(reopened, '/api/v1/workspace')['snapshot_sha256'] == pinned['snapshot_sha256'], 'reopen changed draft')
                require({v['artifact_digest'] for v in api(reopened, '/api/v1/artifacts')} == set(artifacts), 'reopen changed artifacts')
                proc.stop(graceful=True)
                collector.case('unix-same-workspace-reopen')
                collector.report['status'] = 'browser_workflow_passed_manual_security_open'
            require(not errors and not requests_outside, 'browser errors or external requests after reopen')
            browser.close()
    finally:
        if proc.process.poll() is None or not proc.saved:
            proc.stop(graceful=False)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('studio', 'preview', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--browser-executable', type=Path)
    args = parser.parse_args()
    local.configure_tools_environment()
    collector = Collector(args.output, 'real-studio-browser')
    try:
        import psutil
        import importlib.metadata
        require(importlib.metadata.version('playwright') == '1.56.0', 'Playwright 1.56.0 required')
        require(importlib.metadata.version('psutil') == '7.2.2', 'psutil 7.2.2 required')
        paths = {name: local.local_path(getattr(args, name)).resolve(strict=True) for name in ('studio', 'preview')}
        if args.browser_executable is not None:
            args.browser_executable = local.local_path(args.browser_executable).resolve(strict=True)
        collector.report['binaries'] = {name: {'path': str(path), 'sha256': sha(path)} for name, path in paths.items()}
        collector.report['scope'] = 'Actual binary browser workflow only; no native build provenance, PG, operator approval or full OS isolation certification'
        workflow(collector, paths['studio'], paths['preview'], args.browser_executable)
        require(all(sha(path) == collector.report['binaries'][name]['sha256'] for name, path in paths.items()), 'binary changed during run')
    except (Exception, KeyboardInterrupt) as error:
        collector.fail(error)
    finally:
        collector.finish()
    return 1 if collector.report['status'] == 'failed' else 0


if __name__ == '__main__':
    raise SystemExit(main())
