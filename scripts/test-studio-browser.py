"""Real Chromium/DOM + loopback fixture HTTP; NOT Rust/PG/operator acceptance."""
from pathlib import Path
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from threading import Thread, Event
import copy,json,sys
from playwright.sync_api import sync_playwright
ROOT=Path(__file__).resolve().parents[1]
assets=ROOT/'apps/worldwright-studio/assets'
started=Event();release=Event()
auth={'schema_version':1,'configured':False,'renewal':'disabled','automatic_retry':False}
reconcile_calls=[]
rooms=[{'id':f'test:rooms.r{i}','title':f'Room {i}','description':'Room','exits':{},'allow_unreachable':False} for i in range(1024)]
rooms[0]['exits']={f'exit-{i}':rooms[i]['id'] for i in range(1,65)}
snapshot={'revision':1,'snapshot_sha256':'a'*64,'draft':{'schema_version':1,'namespace':'test','entry_room':rooms[0]['id'],'rooms':rooms}}
caps={'schema_version':1,'max_rooms':1024,'max_exits_per_room':64,'max_text_bytes':4096,'max_definition_bytes':256,'max_exit_label_bytes':64,'max_input_bytes':32*1024*1024}
class Handler(BaseHTTPRequestHandler):
    def log_message(self,*args): pass
    def send(self,data,kind='application/json',status=200):
        body=data if isinstance(data,bytes) else json.dumps(data).encode()
        self.send_response(status);self.send_header('Content-Type',kind);self.send_header('Content-Length',str(len(body)));self.send_header('Cache-Control','no-store')
        self.send_header('Content-Security-Policy',"default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'")
        self.end_headers();self.wfile.write(body)
    def do_GET(self):
        if self.path in ('/','/app.js','/app.css'):
            name={'/':'index.html','/app.js':'app.js','/app.css':'app.css'}[self.path]
            return self.send((assets/name).read_bytes(),{'index.html':'text/html; charset=utf-8','app.js':'text/javascript','app.css':'text/css'}[name])
        values={'/api/v1/capabilities':caps,'/api/v1/workspace':snapshot,'/api/v1/artifacts':[],
            '/api/v1/preview':{'state':'idle','output':'','artifact_digest':None},'/api/v1/publications':[],
            '/api/v1/publication-auth':auth,'/api/v1/publication-recovery':[]}
        if self.path not in values: return self.send({'message':'fixture route missing'},status=404)
        self.send(copy.deepcopy(values[self.path]))
    def do_POST(self):
        expected='/api/v1/publications/'+'1'*32+'/reconcile'
        if self.path!=expected: return self.send({'message':'unexpected mutation'},status=404)
        count=int(self.headers['Content-Length']);assert count<=4096
        assert json.loads(self.rfile.read(count))=={}
        reconcile_calls.append(self.path)
        self.send({'request_id':'1'*32,'artifact_digest':'c'*64,'status':'approved','target':'staging'})
    def do_PUT(self):
        if self.path!='/api/v1/draft': return self.send({},status=404)
        count=int(self.headers['Content-Length']);assert count<=caps['max_input_bytes']
        draft=json.loads(self.rfile.read(count));base=int(self.headers['X-Worldwright-Draft-Revision'])
        if base!=snapshot['revision']: return self.send({'current_revision':snapshot['revision']},status=409)
        if base==1:
            started.set()
            if not release.wait(15): return self.send({'message':'fixture release timeout'},status=500)
        snapshot.update(revision=base+1,snapshot_sha256=('b' if base==1 else 'c')*64,draft=draft)
        self.send(copy.deepcopy(snapshot))
def main():
    server=ThreadingHTTPServer(('127.0.0.1',0),Handler);thread=Thread(target=server.serve_forever,daemon=True);thread.start()
    errors=[]
    try:
        with sync_playwright() as playwright:
            browser=playwright.chromium.launch(headless=True)
            page=browser.new_page(viewport={'width':1440,'height':1000})
            page.on('pageerror',lambda error:errors.append(str(error)))
            page.goto(f'http://127.0.0.1:{server.server_port}/');page.wait_for_function("() => document.querySelector('#revision-status').textContent.includes('Revision 1')",timeout=90000)
            assert page.locator('#room-list > li').count()==1024
            assert page.locator('#entry-room > option').count()==1024
            assert page.locator('#exit-list .exit-row').count()==64
            print('PASS: real Chromium renders 1024 rooms and 64 exit controls')
            page.locator('#room-title').fill('🙂'*1024)
            assert page.locator('#room-title').evaluate('input => input.checkValidity()')
            page.locator('#room-title').fill('🙂'*1025)
            assert not page.locator('#room-title').evaluate('input => input.checkValidity()')
            print('PASS: real DOM enforces 4096 UTF-8 bytes, not character count')
            page.locator('#room-title').fill('Saved first value')
            page.locator('#package-game').select_option('expedition')
            page.locator('#world-form').evaluate('form => form.requestSubmit()')
            assert started.wait(5)
            page.locator('#room-title').fill('New edit during save')
            release.set()
            page.wait_for_function("() => document.querySelector('#revision-status').textContent.includes('Revision 2')")
            assert page.locator('#room-title').input_value()=='New edit during save'
            assert 'Unsaved changes' in page.locator('#dirty-status').inner_text()
            assert page.locator('#package-game').input_value()=='expedition'
            page.locator('#world-form').evaluate('form => form.requestSubmit()')
            page.wait_for_function("() => document.querySelector('#revision-status').textContent.includes('Revision 3')")
            assert snapshot['draft']['rooms'][0]['title']=='New edit during save'
            assert 'Draft saved' in page.locator('#dirty-status').inner_text()
            print('PASS: real browser save race preserves edits, selected game and next revision')
            page.locator('summary').filter(has_text='Publication connection and local recovery').click()
            page.wait_for_function("() => document.querySelector('#publication-auth-note').textContent.startsWith('Publication is disabled')")
            assert page.locator('#publication-auth-note').text_content().startswith('Publication is disabled')
            assert page.locator('#submit-publication').is_disabled()
            assert page.locator('#publication-recovery-list').get_attribute('aria-label')
            page.set_viewport_size({'width':390,'height':844})
            assert page.locator('label[for="room-title"]').count()==1
            assert page.locator('#live-status').get_attribute('aria-live')=='polite'
            assert not errors,errors
            print('PASS: publication disabled without credentials; labeled recovery/status UI; no page errors')
            assert page.locator('#reconcile-publication').is_disabled()
            auth.update(configured=True,renewal='token_file')
            page.locator('#refresh-publication-support').click()
            page.wait_for_function("() => !document.querySelector('#reconcile-publication').disabled")
            page.get_by_label('Existing request ID confirmed by operator').fill('1'*32)
            page.locator('#reconcile-publication').focus()
            page.keyboard.press('Enter')
            page.wait_for_function("() => document.querySelector('#publication-note').textContent.includes('Existing request reconciled')")
            assert page.locator('#publication-list > li').count()==1
            assert 'Approved — awaiting activation' in page.locator('#publication-list').inner_text()
            assert 'No submission, review or activation' in page.locator('#publication-note').inner_text()
            assert len(reconcile_calls)==1
            assert not errors,errors
            print('PASS: keyboard-triggered known-ID reconciliation is labeled status-only, single request')
            print('Chromium:',browser.version)
            browser.close()
    finally:
        release.set();server.shutdown();server.server_close();thread.join(timeout=3)
    print('PASS: 5 Chromium fixture groups; actual Rust/PG/TLS/manual accessibility remain OPEN')
if __name__=='__main__': main()
