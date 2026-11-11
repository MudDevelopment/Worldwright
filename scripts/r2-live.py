"""Execute real disposable R2 subprocess/wire faults and HTTPS OIDC browser flows.
No report can authorize adoption. Run only against a loopback worldwright_test DB.
"""
from pathlib import Path
import argparse
import hashlib
import importlib.util
import json
import os
import socket
import subprocess
import sys
import time
import uuid
from urllib.error import HTTPError
from urllib.parse import parse_qsl, urlencode, urlsplit, urlunsplit
from urllib.request import Request, build_opener, HTTPRedirectHandler, ProxyHandler
import psycopg
from psycopg import sql
from websockets.sync.client import connect

ROOT = Path(__file__).resolve().parents[1]


def load(name):
    spec = importlib.util.spec_from_file_location(name.replace('-', '_'), ROOT/'scripts'/(name+'.py'))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


class GateError(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise GateError(message)


def wait(predicate, message, seconds=15):
    end = time.monotonic()+seconds
    while time.monotonic() < end:
        if predicate():
            return
        time.sleep(.05)
    raise ValueError(message)


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, *args):
        return None


def http(origin, path, method='GET', body=None, cookie=None):
    headers = {'Origin': origin}
    if cookie:
        headers['Cookie'] = cookie
    if isinstance(body, dict):
        body = json.dumps(body).encode()
        headers['Content-Type'] = 'application/json'
    request = Request(origin+path, data=body, method=method, headers=headers)
    try:
        response = build_opener(ProxyHandler({}), NoRedirect()).open(request, timeout=5)
    except HTTPError as error:
        response = error
    with response:
        return response.status, response.headers, response.read(65536)


class Fixture:
    def __init__(self, args, output, schema, game, proxy=None, provider=None):
        self.args, self.output, self.schema, self.game = args, output, schema, game
        self.proxy, self.provider = proxy, provider
        self.process = None
        self.log = None
        self.port = self.free_port()
        self.origin = 'http://127.0.0.1:'+str(self.port)

    @staticmethod
    def free_port():
        with socket.socket() as sock:
            sock.bind(('127.0.0.1', 0))
            return sock.getsockname()[1]

    def start(self):
        parsed = urlsplit(os.environ['WORLDWRIGHT_TEST_DATABASE_URL'])
        query = dict(parse_qsl(parsed.query))
        query.update(options='-csearch_path='+self.schema, sslmode='disable')
        netloc = parsed.netloc
        if self.proxy:
            credentials = netloc.rsplit('@', 1)[0]+'@' if '@' in netloc else ''
            netloc = credentials+'127.0.0.1:'+str(self.proxy.address[1])
        url = urlunsplit((parsed.scheme, netloc, parsed.path, urlencode(query), ''))
        env = os.environ.copy()
        for key in ('HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY', 'http_proxy', 'https_proxy', 'all_proxy', 'WORLDWRIGHT_R2_CA', 'WORLDWRIGHT_R2_ISSUER'):
            env.pop(key, None)
        env.update(WORLDWRIGHT_TEST_DATABASE_URL=url, WORLDWRIGHT_R2_FIXTURE='isolated-test-only',
            WORLDWRIGHT_R2_GAME=self.game, WORLDWRIGHT_R2_ORIGIN=self.origin, WORLDWRIGHT_R2_BIND='127.0.0.1:'+str(self.port))
        if self.provider:
            env.update(WORLDWRIGHT_R2_ISSUER=self.provider.issuer, WORLDWRIGHT_R2_CA=str(self.provider.ca_path))
        self.log = (self.output/(self.schema+'-'+str(time.time_ns())+'.log')).open('xb')
        self.process = subprocess.Popen([str(self.args.fixture_executable.resolve()), '--exact', 'r2_live_tests::r2_fixture_process', '--ignored', '--nocapture'], cwd=ROOT, env=env, stdin=subprocess.DEVNULL, stdout=self.log, stderr=subprocess.STDOUT)
        def ready():
            require(self.process.poll() is None, 'fixture exited before readiness; inspect private log')
            try:
                return http(self.origin, '/health/ready')[0] == 200
            except OSError:
                return False
        wait(ready, 'fixture readiness timed out', 30)

    def kill(self):
        if self.process and self.process.poll() is None:
            self.process.kill()
            self.process.wait(timeout=10)
        if self.log:
            self.log.close()
        self.process = None
        self.log = None

    def login(self, username):
        status, headers, _ = http(self.origin, '/api/v1/session', 'POST', {'username':username, 'password':'r2-disposable-only-password'})
        require(status == 204, 'disposable password login failed')
        cookie = headers.get('Set-Cookie', '').split(';')[0]
        require(cookie, 'session cookie absent')
        return cookie

    def attach(self, cookie):
        ws = connect(self.origin.replace('http:', 'ws:')+'/api/v1/play', origin=self.origin,
            additional_headers={'Cookie':cookie}, proxy=None, open_timeout=5, close_timeout=1, max_size=16384)
        ws.send(json.dumps({'type':'hello', 'supported_versions':[1]}))
        require(json.loads(ws.recv(timeout=5))['message']['type'] == 'welcome', 'welcome missing')
        request = 'r2-attach-'+uuid.uuid4().hex
        ws.send(json.dumps({'type':'attach', 'request_id':request}))
        messages = receive(ws, request)
        require(any(m['type']=='attached' for m in messages), 'playable attachment missing')
        return ws


def receive(ws, request):
    messages = []
    end = time.monotonic()+10
    while time.monotonic() < end:
        message = json.loads(ws.recv(timeout=max(.01, end-time.monotonic())))['message']
        messages.append(message)
        if message.get('request_id') == request and message['type'] in ('command_result','reward_result','transfer_result','inventory','error'):
            return messages
    raise ValueError('matching completion not received')


def send(ws, request, command):
    ws.send(json.dumps({'type':'command', 'request_id':request, 'command':command}))


def command(ws, request, value):
    send(ws, request, value)
    messages = receive(ws, request)
    require(not any(m['type']=='error' for m in messages), 'command rejected')
    return messages


def scalar(db, schema, query, parameters=()):
    with db.cursor() as cursor:
        cursor.execute(sql.SQL('SET search_path TO {}').format(sql.Identifier(schema)))
        cursor.execute(query, parameters)
        return cursor.fetchone()[0]


def process_cases(args, output, db, schema, record):
    wire = load('r2-wire')
    parsed = urlsplit(os.environ['WORLDWRIGHT_TEST_DATABASE_URL'])
    proxy = wire.CommitProxy((parsed.hostname, parsed.port or 5432))
    fixture = Fixture(args, output, schema, 'reference', proxy=proxy)
    sockets = []
    try:
        fixture.start()
        for name, mode in [('kill-before-commit','before'), ('kill-after-commit','after'), ('lost-commit-ack','lost-ack')]:
            cookie = fixture.login('alice')
            ws = fixture.attach(cookie)
            sockets.append(ws)
            request = 'r2-'+name
            before = scalar(db, schema, 'SELECT COUNT(*) FROM worldwright_gameplay_receipts WHERE request_id=%s', (request,))
            require(before == 0, 'fault request already present')
            proxy.arm(mode)
            send(ws, request, {'type':'say', 'text':'R2 historical speech '+name})
            require(proxy.hit.wait(10), 'real COMMIT boundary not intercepted')
            # An Accepted message is admission only, never gameplay success.
            try:
                while True:
                    message = json.loads(ws.recv(timeout=.2))['message']
                    require(message['type'] in ('accepted','error'), 'speculative output or completion escaped commit gate')
            except TimeoutError:
                pass
            except Exception as error:
                if not isinstance(error, __import__('websockets').exceptions.ConnectionClosed):
                    raise
            if mode == 'lost-ack':
                wait(lambda: http(fixture.origin, '/health/ready')[0] != 200, 'owner remained ready after uncertain COMMIT')
            fixture.kill()
            proxy.release.set()
            expected = 0 if mode == 'before' else 1
            wait(lambda: scalar(db, schema, 'SELECT COUNT(*) FROM worldwright_gameplay_receipts WHERE request_id=%s',(request,)) == expected, 'durable fault outcome mismatch')
            require(scalar(db, schema, 'SELECT COUNT(*) FROM worldwright_gameplay_audit WHERE request_id=%s',(request,)) == expected, 'audit cardinality mismatch')
            fixture.start()
            ws = fixture.attach(cookie)
            sockets.append(ws)
            bob = fixture.attach(fixture.login('bob'))
            sockets.append(bob)
            command(ws, request, {'type':'say','text':'R2 historical speech '+name})
            if expected:
                end = time.monotonic()+.5
                try:
                    while time.monotonic() < end:
                        message = json.loads(bob.recv(timeout=max(.01,end-time.monotonic())))['message']
                        require(message.get('kind') != 'speech', 'historical speech rebroadcast')
                except TimeoutError:
                    pass
                record('no-historical-speech-rebroadcast', name)
            require(scalar(db, schema, 'SELECT COUNT(*) FROM worldwright_gameplay_receipts WHERE request_id=%s',(request,)) == 1, 'receipt replay duplicated')
            require(scalar(db, schema, 'SELECT COUNT(*) FROM worldwright_gameplay_audit WHERE request_id=%s',(request,)) == 1, 'audit replay duplicated')
            record(name, 'actual subprocess kill/wire boundary/reconstructed owner')
            record('current-owner-no-speculative-success', name)
            for socket_ in sockets:
                socket_.close()
            sockets.clear()
        ws = fixture.attach(fixture.login('alice'))
        sockets.append(ws)
        first = command(ws,'r2-reward-original',{'type':'claim_reward','reward':'welcome'})[-1]
        require(first['outcome']=='reward_granted', 'first entitlement not granted')
        fixture.kill()
        fixture.start()
        ws = fixture.attach(fixture.login('alice'))
        sockets.append(ws)
        replay = command(ws,'r2-reward-original',{'type':'claim_reward','reward':'welcome'})[-1]
        require(replay['replayed'] and replay['operation_id']==first['operation_id'], 'entitlement receipt identity changed')
        duplicate = command(ws,'r2-reward-new-id',{'type':'claim_reward','reward':'welcome'})[-1]
        require(duplicate['outcome']=='already_claimed', 'fresh request bypassed business uniqueness')
        require(scalar(db,schema,'SELECT COUNT(*) FROM worldwright_reward_claims') == 1, 'duplicate business claim')
        require(scalar(db,schema,"SELECT balance FROM worldwright_characters WHERE character_name='Alice'") == 100, 'balance duplicated')
        record('receipt-audit-entitlement-uniqueness','real owner restart and independent business key')
    finally:
        fixture.kill()
        for ws in sockets:
            ws.close()
        proxy.close()
        require(not proxy.errors, 'fault proxy failed: '+str(proxy.errors))


def idle_case(args, output, db, schema, record):
    fixture = Fixture(args, output, schema, 'expedition')
    ws = None
    try:
        fixture.start()
        cookie = fixture.login('alice')
        ws = fixture.attach(cookie)
        command(ws,'r2-rest-before-downtime',{'type':'game_action','action':'expedition:actions.rest','argument':None})
        require(scalar(db,schema,'SELECT COUNT(*) FROM worldwright_scheduled_counter_adjustments') == 2, 'rest did not persist equal deadlines')
        fixture.kill()
        time.sleep(1.2)
        fixture.start()
        wait(lambda: scalar(db,schema,'SELECT COUNT(*) FROM worldwright_scheduled_counter_adjustments') == 0, 'idle restart did not apply overdue schedules')
        require(scalar(db,schema,"SELECT counter_value FROM worldwright_game_counters WHERE counter_key='expedition:state.supplies' ORDER BY character_id LIMIT 1") == 4, 'recovery counter mismatch')
        require(scalar(db,schema,'SELECT COUNT(*) FROM worldwright_gameplay_timer_audit') == 1, 'timer audit cardinality mismatch')
        ws = fixture.attach(cookie)
        request='r2-rng-after-restart'
        command(ws,request,{'type':'game_action','action':'expedition:actions.forage','argument':None})
        supplies=scalar(db,schema,"SELECT counter_value FROM worldwright_game_counters WHERE counter_key='expedition:state.supplies' ORDER BY character_id LIMIT 1")
        fixture.kill(); fixture.start(); ws=fixture.attach(cookie)
        command(ws,request,{'type':'game_action','action':'expedition:actions.forage','argument':None})
        require(scalar(db,schema,"SELECT counter_value FROM worldwright_game_counters WHERE counter_key='expedition:state.supplies' ORDER BY character_id LIMIT 1")==supplies,'replayed random action changed committed supplies')
        version=scalar(db,schema,'SELECT game_state_version FROM worldwright_worlds')
        send(ws,'r2-rejected-strike',{'type':'game_action','action':'expedition:actions.strike','argument':None})
        rejected=receive(ws,'r2-rejected-strike')
        require(rejected[-1]['type']=='error','invalid strike was accepted')
        require(scalar(db,schema,'SELECT game_state_version FROM worldwright_worlds')==version,'rejected action mutated durable state')
        record('idle-restart-clock-rng','real overdue idle timer once; committed random outcomes retained; RNG stream intentionally reseeded on process restart')
    finally:
        fixture.kill()
        if ws:
            ws.close()


def controlled_cases(args, output, db, schema, record):
    from playwright.sync_api import sync_playwright
    fixture = Fixture(args, output, schema, 'expedition')
    provider = load('r2-provider').Provider(output, fixture.origin+'/api/v1/oidc/callback')
    fixture.provider = provider
    try:
        fixture.start()
        token = uuid.uuid4().hex+uuid.uuid4().hex
        token_hash = hashlib.sha256(b'worldwright-oidc-token-v1\0'+token.encode()).digest()
        with db.cursor() as cursor:
            cursor.execute(sql.SQL('SET search_path TO {}').format(sql.Identifier(schema)))
            cursor.execute("INSERT INTO worldwright_player_invites(invite_id,world_id,account_id,issuer,token_hash,expires_at) SELECT %s,world_id,account_id,%s,%s,clock_timestamp()+INTERVAL '5 minutes' FROM worldwright_characters WHERE character_name='Alice'", (uuid.uuid4(),provider.issuer,token_hash))
        with sync_playwright() as playwright:
            kwargs = {'headless':True}
            if args.browser_executable:
                kwargs['executable_path'] = str(args.browser_executable)
            browser = playwright.chromium.launch(**kwargs)
            context = browser.new_context(ignore_https_errors=True)
            page = context.new_page()
            password_posts = []
            page.on('request', lambda request: password_posts.append(True) if request.method=='POST' and urlsplit(request.url).path=='/api/v1/session' else None)
            def flow(invite='', success=True):
                page.goto(fixture.origin)
                page.locator('#oidc-invite').fill(invite)
                page.locator('#oidc-form button').click()
                if success:
                    page.wait_for_function("document.querySelector('#status').textContent.startsWith('Attached as ')", timeout=15000)
                    require(not page.locator('#command-submit').is_disabled(), 'OIDC browser not playable')
                    page.locator('#command').fill('look')
                    page.locator('#command-submit').click()
                    page.wait_for_function("document.querySelector('#output').textContent.includes('Supplies:')")
                    require(page.evaluate('document.cookie') == '', 'HttpOnly session exposed to JS')
                    page.locator('#logout').click()
                    page.wait_for_function("document.querySelector('#status').textContent==='Logged out'")
                else:
                    page.wait_for_url('**/api/v1/oidc/callback?**', timeout=15000)
                    require('Attached as' not in page.content(), 'rejected callback attached')
                    require(not context.cookies(), 'rejected callback minted session cookie')
            flow(token)
            require(scalar(db,schema,'SELECT COUNT(*) FROM worldwright_oidc_identities') == 1, 'identity link missing')
            require(scalar(db,schema,'SELECT COUNT(*) FROM worldwright_player_invites WHERE consumed_at IS NOT NULL') == 1, 'invite not consumed once')
            record('browser-playable-attachment-no-password-fallback','real Chromium callback/resume/command/HttpOnly/logout')
            for mode in ('wrong_issuer','wrong_audience','bad_signature','wrong_nonce','expired','wrong_state','bad_pkce'):
                provider.mode = mode
                flow(success=False)
                record('issuer-audience-signature' if mode in ('wrong_issuer','wrong_audience','bad_signature','expired') else 'nonce-state-pkce', mode)
            provider.mode = 'valid'
            provider.active = 1
            flow()
            record('key-rotation','new JWKS loaded by actual callback')
            flow(token, success=False)
            provider.subject = 'uninvited-subject'
            flow(success=False)
            expired = uuid.uuid4().hex+uuid.uuid4().hex
            expired_hash = hashlib.sha256(b'worldwright-oidc-token-v1\0'+expired.encode()).digest()
            with db.cursor() as cursor:
                cursor.execute("INSERT INTO worldwright_player_invites(invite_id,world_id,account_id,issuer,token_hash,created_at,expires_at) SELECT %s,world_id,account_id,%s,%s,statement_timestamp()-INTERVAL '2 seconds',statement_timestamp()-INTERVAL '1 second' FROM worldwright_characters WHERE character_name='Bob'", (uuid.uuid4(),provider.issuer,expired_hash))
            flow(expired, success=False)
            record('invite-expiry-consumption-denied-link','reused/absent/expired invite callback rejection, no account creation')
            provider.subject = 'r2-alice'
            fixture.kill()
            fixture.start()
            flow()
            page.goto(fixture.origin)
            start = page.request.post(fixture.origin+'/api/v1/oidc/start', form={'invite':''}, headers={'Origin':fixture.origin}, max_redirects=0)
            require(start.status == 303, 'outage pending start failed')
            state = dict(parse_qsl(urlsplit(start.headers['location']).query))['state']
            provider.mode = 'outage'
            response = page.request.get(fixture.origin+'/api/v1/oidc/callback?'+urlencode({'code':'outage-code','state':state}), max_redirects=0)
            require(response.status == 503, 'provider outage did not fail closed')
            response = page.request.get(fixture.origin+'/api/v1/oidc/callback?'+urlencode({'code':'outage-code','state':state}), max_redirects=0)
            require(response.status == 401, 'outage state reused')
            provider.mode = 'valid'
            for policy in ('revocation','expiry','disabled'):
                page.goto(fixture.origin)
                page.locator('#oidc-form button').click()
                page.wait_for_function("document.querySelector('#status').textContent.startsWith('Attached as ')")
                require(page.request.get(fixture.origin+'/api/v1/session').status == 204, 'session probe failed')
                if policy == 'revocation':
                    fixture.kill()
                    fixture.start()
                    page.reload()
                    page.wait_for_function("document.querySelector('#status').textContent.startsWith('Attached as ')")
                    require(not page.locator('#command-submit').is_disabled(), 'durable OIDC cookie did not resume after restart')
                with db.cursor() as cursor:
                    if policy == 'revocation': cursor.execute('UPDATE worldwright_sessions SET revoked_at=clock_timestamp()')
                    elif policy == 'expiry': cursor.execute("UPDATE worldwright_sessions SET created_at=statement_timestamp()-INTERVAL '2 seconds', expires_at=statement_timestamp()-INTERVAL '1 second' WHERE account_id=(SELECT account_id FROM worldwright_accounts WHERE username='alice') AND revoked_at IS NULL")
                    else: cursor.execute("UPDATE worldwright_accounts SET disabled=TRUE WHERE username='alice'")
                page.reload()
                page.wait_for_function("document.querySelector('#status').textContent==='Sign in to play'")
                require(page.locator('#command-submit').is_disabled(), 'revoked/expired/disabled browser enabled gameplay')
                context.clear_cookies()
                if policy == 'disabled':
                    with db.cursor() as cursor: cursor.execute("UPDATE worldwright_accounts SET disabled=FALSE WHERE username='alice'")
            record('revocation-outage-restart','actual callback restart/outage one-shot, browser revoked/expired/disabled probe')
            require(not password_posts, 'OIDC silently fell back to password')
            browser.close()
    finally:
        fixture.kill()
        provider.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=('process','controlled'))
    parser.add_argument('--fixture-executable', type=Path, required=True)
    parser.add_argument('--server-artifact', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--browser-executable', type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    require(not output.exists() and not output.is_relative_to(ROOT), 'new external private evidence directory required')
    os.umask(0o077)
    output.mkdir(mode=0o700)
    report = {'schema_version':1,'kind':'real-process-faults' if args.mode=='process' else 'controlled-provider',
        'status':'failed','reviewer':'automation-not-human-adoption','cases':[], 'full_r2_closed':False}
    log_lines, cases = [], {}
    db = None
    schemas = []
    def record(name, detail):
        cases.setdefault(name, []).append(detail)
        log_lines.append(name+': '+detail)
    try:
        require(args.fixture_executable.is_file() and not args.fixture_executable.is_symlink(), 'real compiled fixture executable required')
        require(args.server_artifact.is_file() and not args.server_artifact.is_symlink(), 'same-source production server artifact required')
        source = load('r0-verify').source_identity()
        report.update(source_sha256=source['source_sha256'], lock_sha256=source['lock_sha256'], artifact_sha256=sha(args.server_artifact), fixture_sha256=sha(args.fixture_executable))
        parsed = urlsplit(os.environ.get('WORLDWRIGHT_TEST_DATABASE_URL',''))
        require(parsed.scheme in ('postgres','postgresql') and 'worldwright_test' in parsed.path and parsed.hostname in ('localhost','127.0.0.1','::1') and not parsed.query, 'loopback disposable DB URL required, no caller-supplied search_path/options')
        db = psycopg.connect(os.environ['WORLDWRIGHT_TEST_DATABASE_URL'], autocommit=True, connect_timeout=5)
        require('worldwright_test' in scalar(db,'public','SELECT current_database()'), 'actual database-name guard failed')
        for _ in range(2 if args.mode=='process' else 1):
            schema = 'ww_r2_'+uuid.uuid4().hex
            with db.cursor() as cursor:
                cursor.execute(sql.SQL('CREATE SCHEMA {}').format(sql.Identifier(schema)))
            schemas.append(schema)
            print('WORLDWRIGHT_R2_SCHEMA_CREATED='+schema, flush=True)
        if args.mode == 'process':
            process_cases(args,output,db,schemas[0],record)
            idle_case(args,output,db,schemas[1],record)
            required = load('r2-verify').PROCESS_REQUIRED
        else:
            controlled_cases(args,output,db,schemas[0],record)
            required = load('r2-verify').PROVIDER_REQUIRED
        require(required <= cases.keys(), 'required real case not executed')
        require(load('r0-verify').source_identity() == source, 'source changed during exercise')
        report['status'] = 'passed'
    except Exception as error:
        report['failure'] = type(error).__name__+': '+(str(error) if isinstance(error,GateError) else 'exercise failed; inspect private child logs')
        log_lines.append(report['failure'])
        print('R2 live exercise failed:', report['failure'], file=sys.stderr)
    finally:
        if db:
            try:
                for schema in schemas:
                    with db.cursor() as cursor:
                        cursor.execute(sql.SQL('DROP SCHEMA {} CASCADE').format(sql.Identifier(schema)))
                        cursor.execute('SELECT EXISTS(SELECT 1 FROM pg_namespace WHERE nspname=%s)',(schema,))
                        require(not cursor.fetchone()[0], 'fixture schema cleanup failed')
                        print('WORLDWRIGHT_R2_SCHEMA_DROPPED='+schema, flush=True)
                report['schema_cleanup'] = 'verified'
            except Exception:
                report.update(status='failed', schema_cleanup='failed')
            db.close()
        report['cases'] = [{'case':name,'status':'passed','observations':details} for name,details in sorted(cases.items())]
        prefix = 'process' if args.mode=='process' else 'controlled-provider'
        log = output/(prefix+'.log')
        log.write_text('\n'.join(log_lines)+'\n')
        report['log_sha256'] = sha(log)
        report['child_logs'] = [{'name':path.name,'sha256':sha(path)} for path in sorted(output.glob('ww_r2_*.log'))]
        (output/(prefix+'.json')).write_text(json.dumps(report,indent=2)+'\n')
    print('R2 '+args.mode+': '+report['status'])
    return 0 if report['status']=='passed' else 1


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (ValueError, OSError) as error:
        print('R2 NOT ACCEPTED:',error,file=sys.stderr)
        raise SystemExit(1)
