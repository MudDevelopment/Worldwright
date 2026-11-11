"""R4 source-bound evidence, fixed recipe validation and bounded private files."""
from pathlib import Path
import base64, hashlib, importlib.util, json, math, os, re, time
from urllib.parse import urlsplit
ROOT=Path(__file__).resolve().parents[1]
HEX=re.compile(r'[0-9a-f]{64}')
CASES={'speech-fanout','reward-replay','transfer-replay','slow-reader','reconnect','idle-heartbeat','soak'}
FAILURES={'login','attach','command','protocol','disconnect','readiness','internal'}
def require(value,message):
    if not value: raise ValueError(message)
def integer(value,low=0,high=2**64-1): return type(value) is int and low<=value<=high
def sha(path):
    value=hashlib.sha256()
    with Path(path).open('rb') as source:
        for block in iter(lambda:source.read(65536),b''): value.update(block)
    return value.hexdigest()
def load(name,path):
    spec=importlib.util.spec_from_file_location(name,path); module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module); return module
def read(path,limit=4*1024*1024):
    path=Path(path); require(path.is_file() and not path.is_symlink() and 0<path.stat().st_size<=limit,'bounded regular JSON required')
    def pairs(values):
        result={}
        for key,value in values:
            require(key not in result,'duplicate JSON key'); result[key]=value
        return result
    def floating(raw):
        value=float(raw); require(math.isfinite(value),'finite JSON numbers required'); return value
    return json.loads(path.read_text(),object_pairs_hook=pairs,parse_float=floating,parse_constant=lambda _: (_ for _ in ()).throw(ValueError('finite JSON required')))
def write_new(path,value):
    data=(json.dumps(value,sort_keys=True,indent=2,allow_nan=False)+'\n').encode(); require(len(data)<=4*1024*1024,'report size bound')
    with os.fdopen(os.open(path,os.O_CREAT|os.O_EXCL|os.O_WRONLY,0o600),'wb') as output: output.write(data); output.flush(); os.fsync(output.fileno())
def contained(parent,name,limit=16*1024*1024):
    relative=Path(name); require(not relative.is_absolute() and '..' not in relative.parts,'contained evidence path')
    path=Path(parent)/relative
    require(path.resolve().is_relative_to(Path(parent).resolve()) and path.is_file() and not path.is_symlink() and path.stat().st_size<=limit,'bounded contained evidence')
    return path
def origin(value):
    parsed=urlsplit(value)
    require(parsed.scheme=='https' and parsed.hostname and not parsed.username and not parsed.password and parsed.path=='' and not parsed.query and not parsed.fragment and not parsed.hostname.endswith('.invalid'),'actual canonical HTTPS origin required')
    return value
def signed(value,key_path,kind):
    from cryptography.hazmat.primitives import serialization
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
    require(value.get('kind')==kind and value.get('status')=='adopted','independent adopted record required')
    reviewer=value.get('reviewer'); require(isinstance(reviewer,str) and reviewer.strip() and len(reviewer)<=256 and reviewer.isprintable(),'bounded reviewer required')
    require(integer(value.get('expires_at'),int(time.time())+1,int(time.time())+30*86400),'bounded unexpired approval required')
    key=serialization.load_pem_public_key(Path(key_path).read_bytes()); require(isinstance(key,Ed25519PublicKey),'trusted Ed25519 key required')
    payload={k:v for k,v in value.items() if k!='signature'}
    key.verify(base64.b64decode(value['signature'],validate=True),json.dumps(payload,sort_keys=True,separators=(',',':'),allow_nan=False).encode())
def beta_report(value,smoke=False):
    require(isinstance(value,dict) and integer(value.get('schema_version'),1,1) and integer(value.get('profile_version'),1,1),'load report schema/version')
    require(value.get('profile')==('smoke-v1' if smoke else 'beta-250-v1'),'profile must not be relabeled')
    clients,commands=(2,4) if smoke else (250,7500)
    for key,expected in {'target_clients':clients,'connected_clients':clients,'expected_commands':commands,'completed_commands':commands}.items(): require(integer(value.get(key),expected,expected),'exact load dimensions: '+key)
    failures=value.get('failures'); require(isinstance(failures,dict) and set(failures)==FAILURES and all(integer(v,0,0) for v in failures.values()),'zero complete failure inventory')
    require(integer(value.get('readiness_checks'),1) and integer(value.get('elapsed_ms'),1,90000),'readiness and measured deadline')
    require(value.get('storage_expectation')=='operator-supplied-postgresql','unchanged storage expectation; independent proof still required')
    latency=value.get('latency'); require(isinstance(latency,dict) and set(latency)=={'count','min_us','p50_us','p95_us','p99_us','max_us'},'latency inventory')
    require(integer(latency['count'],commands,commands) and all(integer(v) for v in latency.values()),'latency count/types')
    require(latency['min_us']<=latency['p50_us']<=latency['p95_us']<=latency['p99_us']<=latency['max_us']<10000000,'ordered latency/deadline')
    if not smoke: require(latency['p95_us']<=750000 and latency['p99_us']<=1500000 and value.get('passed') is True and value.get('failure_reasons')==[],'unchanged beta thresholds/pass')
    else: require(value.get('passed') is False and value.get('tooling_smoke_passed') is True,'smoke never proves beta')
    require(integer(value.get('throughput_milli_commands_per_second')) and value['throughput_milli_commands_per_second']==commands*1000000//value['elapsed_ms'],'throughput arithmetic')
    require(isinstance(value.get('run_nonce'),str) and re.fullmatch(r'[0-9a-f]{16}',value['run_nonce']),'bounded run nonce')
    start,end=value.get('started_unix_seconds'),value.get('ended_unix_seconds'); require(integer(start,1) and integer(end,start) and (end-start+2)*1000>=value['elapsed_ms'],'run timestamp coverage')
    return value
PLAN_FIELDS={'schema_version','kind','status','reviewer','expires_at','signature','source_sha256','base_url','artifact_digest','environment_label','r3_arguments','credentials','scenario_credentials','database_url_env','runtime_role','schema','pod_ip','process_pid','telemetry_url','soak_seconds','max_rss_growth_bytes','max_history_growth_bytes','retention_policy_sha256','context','namespace','deployment'}
def plan_check(plan):
    require(isinstance(plan,dict) and set(plan)==PLAN_FIELDS and integer(plan['schema_version'],1,1),'exact R4 plan fields/version')
    origin(plan['base_url'])
    for key in ('source_sha256','artifact_digest','retention_policy_sha256'): require(isinstance(plan[key],str) and HEX.fullmatch(plan[key]),'digest: '+key)
    for key in ('runtime_role','schema'): require(isinstance(plan[key],str) and re.fullmatch(r'[a-z_][a-z0-9_]{0,62}',plan[key]),'database identifier')
    for key in ('context','namespace','deployment','environment_label'): require(isinstance(plan[key],str) and re.fullmatch(r'[a-zA-Z0-9._:-]{1,128}',plan[key]),'bounded deployment identity')
    require(isinstance(plan['database_url_env'],str) and re.fullmatch(r'[A-Z][A-Z0-9_]{0,127}',plan['database_url_env']),'database environment reference')
    import ipaddress
    ipaddress.ip_address(plan['pod_ip'])
    for key in ('r3_arguments','credentials','scenario_credentials'): require(isinstance(plan[key],str) and Path(plan[key]).is_absolute(),'absolute private input path')
    parsed=urlsplit(plan['telemetry_url']); require(parsed.scheme=='http' and parsed.hostname in ('127.0.0.1','::1') and parsed.port and parsed.path=='/metrics' and not parsed.username and not parsed.password and not parsed.query and not parsed.fragment,'protected loopback telemetry or port-forward required')
    require(integer(plan['process_pid'],2,2**31-1) and integer(plan['soak_seconds'],1800,3000),'identified server process and separate 30-50 minute soak')
    for key in ('max_rss_growth_bytes','max_history_growth_bytes'): require(integer(plan[key],0,1024**3),'explicit adopted growth bound')
def compare(before,after,bottleneck):
    beta_report(before); beta_report(after)
    for key in ('artifact_digest','environment_label','operating_system','architecture'): require(before[key]==after[key],'not comparable: '+key)
    require(isinstance(bottleneck,str) and bottleneck.strip() and len(bottleneck)<=1024,'observed bottleneck rationale required')
    return {'kind':'r4-comparison','supports_capacity_increase':False,'p95_delta_us':after['latency']['p95_us']-before['latency']['p95_us'],'p99_delta_us':after['latency']['p99_us']-before['latency']['p99_us'],'bottleneck':bottleneck,'integrity_rerun_required':True}
