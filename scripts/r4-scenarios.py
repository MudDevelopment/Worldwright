"""Separate real protocol scenarios/soak; never substitute for fixed Rust beta run."""
from pathlib import Path
import argparse,json,re,ssl,time,uuid
from urllib.request import Request,build_opener,HTTPSHandler,ProxyHandler
from websockets.sync.client import connect
from r3_io import NoRedirects
from r4_common import CASES,read,write_new,require,plan_check,signed,origin
class Session:
    def __init__(self,base,credential,context=None):
        origin(base); self.base=base; self.credential=credential; self.context=context or ssl.create_default_context(); self.socket=None
        self.http=build_opener(ProxyHandler({}),HTTPSHandler(context=self.context),NoRedirects())
        self.login(); self.attach()
    def login(self):
        body=json.dumps(self.credential).encode()
        request=Request(self.base+'/api/v1/session',data=body,headers={'Origin':self.base,'Content-Type':'application/json'})
        with self.http.open(request,timeout=5) as response:
            require(response.status==204,'scenario login failed')
            cookies=[value.split(';',1)[0] for value in response.headers.get_all('Set-Cookie',[]) if value.startswith('ww_session=')]
            require(len(cookies)==1 and re.fullmatch(r'ww_session=[0-9a-f]{64}',cookies[0]),'unambiguous session cookie required'); self.cookie=cookies[0]
    def attach(self):
        self.socket=connect(self.base.replace('https:','wss:',1)+'/api/v1/play',origin=self.base,additional_headers={'Cookie':self.cookie},ssl=self.context,proxy=None,max_size=16384,max_queue=1,open_timeout=5,close_timeout=1)
        self.epoch=None; self.sequence=0
        self.send({'type':'hello','supported_versions':[1]}); require(self.receive()['type']=='welcome','welcome missing')
        request='r4-attach-'+uuid.uuid4().hex; self.send({'type':'attach','request_id':request}); messages=self.until(request)
        require(any(message['type']=='attached' for message in messages),'attached missing')
    def send(self,value): self.socket.send(json.dumps(value,separators=(',',':')))
    def receive(self,timeout=10):
        raw=self.socket.recv(timeout=timeout); require(isinstance(raw,str) and len(raw.encode())<=16384,'bounded text protocol frame required')
        value=json.loads(raw); require(set(value)=={'protocol_version','stream_epoch','sequence','message'} and type(value['protocol_version']) is int and value['protocol_version']==1,'protocol envelope')
        require(isinstance(value['stream_epoch'],str) and re.fullmatch(r'[0-9a-f]{32}',value['stream_epoch']),'stream epoch')
        require(type(value['sequence']) is int and value['sequence']==self.sequence+1,'stream sequence gap/duplicate')
        if self.epoch is None: self.epoch=value['stream_epoch']
        require(value['stream_epoch']==self.epoch,'stale stream epoch'); self.sequence=value['sequence']
        message=value['message']; require(isinstance(message,dict) and message.get('type') in {'welcome','accepted','attached','output','exits','command_result','error','reward_result','transfer_result','inventory','pong'},'unsupported server message')
        return message
    def until(self,request):
        messages=[]; end=time.monotonic()+10; accepted=False
        while time.monotonic()<end and len(messages)<64:
            message=self.receive(max(.01,end-time.monotonic())); messages.append(message)
            require(message['type']!='error','server rejected scenario command')
            if message.get('request_id')==request and message['type']=='accepted': accepted=True
            if message.get('request_id')==request and message['type'] in {'command_result','reward_result','transfer_result','inventory'}:
                require(accepted,'completion before acceptance'); return messages
        raise ValueError('matching completion deadline/bound')
    def command(self,value,request=None):
        request=request or 'r4-'+uuid.uuid4().hex
        self.send({'type':'command','request_id':request,'command':value}); return self.until(request)
    def ready(self):
        with self.http.open(self.base+'/health/ready',timeout=5) as response: require(response.status==200,'readiness failure')
    def close(self):
        if self.socket: self.socket.close()
def scenarios(plan,report,session_factory=Session):
    credentials=read(plan['scenario_credentials'],1024*1024)
    require(credentials.get('schema_version')==1 and isinstance(credentials.get('credentials'),list) and len(credentials['credentials'])==2,'exactly two independent fresh scenario accounts')
    require(credentials['credentials'][0]['username']!=credentials['credentials'][1]['username'],'distinct scenario accounts')
    a=b=None
    def record(case,count=1,**extra): report['cases'].append({'case':case,'status':'passed','observations':count,**extra})
    try:
        a=session_factory(plan['base_url'],credentials['credentials'][0]); b=session_factory(plan['base_url'],credentials['credentials'][1])
        speech='R4 synthetic fanout '+uuid.uuid4().hex
        messages=a.command({'type':'say','text':speech}); peer=b.receive()
        require(any(m['type']=='output' and m.get('kind')=='speech' and m.get('text')==speech for m in messages) and peer.get('type')=='output' and peer.get('request_id') is None and peer.get('text')==speech,'same-room speech fanout missing'); record('speech-fanout',2)
        request='r4-reward-'+uuid.uuid4().hex
        first=a.command({'type':'claim_reward','reward':'welcome'},request)[-1]
        require(first.get('outcome')=='reward_granted' and first.get('item_id'),'fresh seeded entitlement required')
        again=a.command({'type':'claim_reward','reward':'welcome'},request)[-1]
        duplicate=a.command({'type':'claim_reward','reward':'welcome'})[-1]
        require(again.get('replayed') is True and again.get('item_id')==first['item_id'] and again.get('operation_id')==first['operation_id'] and duplicate.get('outcome')=='already_claimed','reward replay/business uniqueness'); record('reward-replay',3)
        transfer={'type':'transfer_item','item_id':first['item_id'],'recipient':credentials['credentials'][1]['username']}; request='r4-transfer-'+uuid.uuid4().hex
        moved=a.command(transfer,request)[-1]; repeated=a.command(transfer,request)[-1]
        owned=b.command({'type':'inventory'})[-1]; old=a.command({'type':'inventory'})[-1]
        require(moved.get('outcome')=='item_transferred' and repeated.get('replayed') is True and repeated.get('operation_id')==moved.get('operation_id'),'transfer replay')
        require(sum(item['item_id']==first['item_id'] for item in owned['items'])==1 and not any(item['item_id']==first['item_id'] for item in old['items']),'single authoritative item owner'); record('transfer-replay',4)
        old_epoch=a.epoch; a.close(); a.attach(); a.command({'type':'look'}); require(a.epoch!=old_epoch,'fresh reconnect stream'); record('reconnect')
        for nonce in range(3):
            a.send({'type':'ping','nonce':nonce}); message=a.receive(); require(message.get('type')=='pong' and message.get('nonce')==nonce,'idle application heartbeat'); time.sleep(.1)
        record('idle-heartbeat',3)
        # max_queue=1 deliberately stops the peer library from unlimited receive buffering.
        # A bounded flood must show peer disconnect while the sender remains healthy.
        end=time.monotonic()+90; sent=0; dropped=False
        while sent<2048 and time.monotonic()<end:
            a.command({'type':'say','text':'R4 bounded slow-reader fixture '+'x'*4000}); sent+=1
            if sent%32==0:
                from r4_monitor import metrics_parse
                client=build_opener(ProxyHandler({}),NoRedirects())
                with client.open(plan['telemetry_url'],timeout=3) as response: metrics=metrics_parse(response.read(262145))
                if metrics['worldwright_connected_clients']==1: dropped=True; break
        require(dropped,'slow reader did not exercise bounded disconnect; no false saturation pass')
        a.command({'type':'look'}); record('slow-reader',sent,transport_backpressure_observed=True); b.close(); b.attach()
        start=time.monotonic(); count=0; latencies=[]
        while time.monotonic()-start<plan['soak_seconds']:
            due=start+count*2
            if due>time.monotonic(): time.sleep(due-time.monotonic())
            for client in (a,b):
                before=time.monotonic(); client.command({'type':'look'}); latencies.append(int((time.monotonic()-before)*1000000)); client.ready()
            count+=1
            require(len(latencies)<=4096,'bounded soak latency history')
        record('soak',len(latencies),duration_seconds=time.monotonic()-start,max_latency_us=max(latencies),profile='two-client-look-soak-v1')
    finally:
        if a: a.close()
        if b: b.close()
def main():
    parser=argparse.ArgumentParser(); parser.add_argument('--plan',type=Path,required=True); parser.add_argument('--approval-public-key',type=Path,required=True); parser.add_argument('--output',type=Path,required=True); args=parser.parse_args()
    report={'schema_version':1,'kind':'r4-supplemental-scenarios','status':'failed','full_r4_closed':False,'cases':[]}
    try:
        plan=read(args.plan); plan_check(plan); signed(plan,args.approval_public_key,'r4-performance-plan')
        report.update(source_sha256=plan['source_sha256'],artifact_digest=plan['artifact_digest'],environment_label=plan['environment_label']); scenarios(plan,report)
        require({case['case'] for case in report['cases']}==CASES,'complete independent scenario inventory'); report['status']='passed'
    except Exception as error: report['failure']=type(error).__name__
    write_new(args.output,report); print('R4 supplemental scenarios:',report['status']); return 0 if report['status']=='passed' else 1
if __name__=='__main__': raise SystemExit(main())
