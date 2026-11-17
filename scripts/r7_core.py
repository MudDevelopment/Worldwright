"""R7 private-beta decision contracts. No signer, publisher, deployer or waiver bypass."""
from pathlib import Path, PurePosixPath
import base64
import hashlib
import importlib.util
import json
import math
import os
import re
import time
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
from r3_io import collect_phase

ROOT = Path(__file__).resolve().parents[1]
ROLES = {'project', 'engineering', 'security', 'service', 'legal'}
GATE_ROLES = {'R1':'engineering','R2':'engineering','R3':'service','R4':'service','R5':'engineering','R6':'security'}
GATES = {
 'R1': {'native-ubuntu','native-windows','installed-no-checkout','browser-display-accessibility','process-privacy-cleanup'},
 'R2': {'complete-migrations','live-gameplay-replay-concurrency','unknown-commit-restart','approved-oidc-playable','cleanup-verified'},
 'R3': {'actual-image-ingress','separate-db-roles','network-operator-denial','rotation-termination','rollout-content-rollback'},
 'R4': {'fixed-beta-250','separate-soak-abuse','protected-measurements','retention-growth-adopted'},
 'R5': {'maximum-multibyte-draft','preview-reviewed-active-identity','publication-auth-recovery','relocated-sdk-both-games','independent-builder-accessibility'},
 'R6': {'issuer-key-custody','current-authority-offboarding','os-credential-isolation','real-restore-takeover-security','measured-objectives','retention-entitlement-safety','external-effects-scope','supply-chain-reporting-owners'},
}
POLICY_ROLES = {'license-distribution':'legal','support':'service','security-reporting':'security',
                'privacy-retention':'security','recovery':'service','runtime-matrix':'engineering'}
PRIMARY = {'artifact','native-ubuntu','native-windows','beta','soak','deployment','sbom'}
DOMAIN = b'WORLDWRIGHT-R7-SIGNATURE-V1\n'
JSON_LIMIT = 16*1024*1024
FILE_LIMIT = 512*1024*1024
TOTAL_LIMIT = 4*1024*1024*1024


def require(value, code):
    if not value: raise ValueError(code)


def integer(value, low=0, high=2**63-1):
    return type(value) is int and low <= value <= high


def text(value, limit=512):
    return isinstance(value,str) and 0 < len(value.encode('utf-8')) <= limit and value.strip()==value and value.isprintable()


def digest(value):
    return isinstance(value,str) and re.fullmatch('[0-9a-f]{64}',value) is not None


def identifier(value):
    return isinstance(value,str) and re.fullmatch('[a-z][a-z0-9._-]{0,63}',value) is not None


def canonical(value):
    return json.dumps(value,sort_keys=True,separators=(',',':'),ensure_ascii=True,allow_nan=False).encode()


def sha(data): return hashlib.sha256(data).hexdigest()


def pairs(items):
    result={}
    for key,value in items:
        require(key not in result,'duplicate-json-key')
        result[key]=value
    return result


def parse(data):
    def floating(raw):
        value=float(raw); require(math.isfinite(value),'nonfinite-number'); return value
    return json.loads(data,object_pairs_hook=pairs,parse_float=floating,
                      parse_constant=lambda _: require(False,'nonfinite-number'))


def raw(path, limit=JSON_LIMIT):
    path=Path(path)
    require(path.is_file() and not path.is_symlink() and path.stat().st_size<=limit,'bounded-regular-file-required')
    with path.open('rb') as source: data=source.read(limit+1)
    require(len(data)<=limit,'file-grew-beyond-bound')
    return data


def load(path):
    result=parse(raw(path)); require(isinstance(result,dict),'json-object-required'); return result


def safe_path(root, name):
    require(isinstance(name,str) and name and '\\' not in name and ':' not in name
            and all(ord(c)>=32 and ord(c)!=127 for c in name),'unsafe-path')
    part=PurePosixPath(name)
    require(not part.is_absolute() and part.as_posix()==name and '..' not in part.parts and '.' not in part.parts,'unsafe-path')
    root=Path(root).resolve(strict=True); path=root.joinpath(*part.parts)
    require(path.resolve().is_relative_to(root),'path-escape')
    for item in (path,*path.parents):
        if item==root: break
        require(not item.is_symlink(),'symlink-refused')
    return path


def file_sha(path, limit=FILE_LIMIT):
    path=Path(path); require(path.is_file() and not path.is_symlink() and path.stat().st_size<=limit,'bounded-file-required')
    total=0; result=hashlib.sha256()
    with path.open('rb') as source:
        for block in iter(lambda: source.read(65536),b''):
            total+=len(block); require(total<=limit,'file-grew-beyond-bound'); result.update(block)
    return result.hexdigest()


def write_new(path,data):
    path=Path(path)
    with os.fdopen(os.open(path,os.O_CREAT|os.O_EXCL|os.O_WRONLY,0o600),'wb') as output:
        output.write(data); output.flush(); os.fsync(output.fileno())


def source_identity(root):
    root=Path(root).resolve(strict=True)
    def git(*args):
        value=collect_phase(['git',*args],30,cwd=root)
        require(value['returncode']==0 and value['interruption'] is None,'source-git-failed')
        return value['stdout']
    names=sorted(set(git('ls-files','-z','--cached','--others','--exclude-standard').split(b'\0'))-{b''})
    require(0<len(names)<=10000,'source-inventory-bound')
    text_suffixes={'.rs','.toml','.lock','.md','.txt','.json','.yaml','.yml','.sh','.ps1','.py','.cjs','.js','.css','.html'}
    inventory=[]
    for item in names:
        name=os.fsdecode(item); path=safe_path(root,name)
        hashed=sha(raw(path).replace(b'\r\n',b'\n')) if path.suffix.lower() in text_suffixes or path.name in {'.gitignore','.gitattributes'} else file_sha(path)
        inventory.append({'path':name,'sha256_lf_text':hashed})
    return {'commit':git('rev-parse','HEAD').decode().strip(), 'dirty':bool(git('status','--porcelain')),
            'source_sha256':sha(json.dumps(inventory,sort_keys=True).encode()),
            'lock_sha256':sha(raw(root/'Cargo.lock').replace(b'\r\n',b'\n')), 'inventory':inventory,
            'patch_sha256':sha(git('diff','--binary','HEAD'))}


def trust_keys(store, now):
    require(set(store)=={'schema_version','keys'} and type(store['schema_version']) is int and store['schema_version']==1,'trust-schema')
    require(isinstance(store['keys'],list) and 5<=len(store['keys'])<=100,'trust-key-inventory')
    result={}
    for key in store['keys']:
        require(isinstance(key,dict) and set(key)=={'key_id','role','subject','public_key_hex','not_before','not_after','revoked'},'trust-key-fields')
        require(identifier(key['key_id']) and key['key_id'] not in result and key['role'] in ROLES and text(key['subject'],256),'trust-key-identity')
        require(digest(key['public_key_hex']) and integer(key['not_before']) and integer(key['not_after'],key['not_before']+1)
                and type(key['revoked']) is bool,'trust-key-types')
        result[key['key_id']]=key
    return result


def signed(envelope, keys, purpose, now):
    require(isinstance(envelope,dict) and set(envelope)=={'payload','signature'},'signature-envelope')
    p=envelope['payload']; require(isinstance(p,dict),'signature-payload')
    require(p.get('purpose')==purpose and type(p.get('schema_version')) is int and p['schema_version']==1,'signature-domain')
    key=keys.get(p.get('key_id')); require(key is not None,'unknown-signing-key')
    require(not key['revoked'] and key['not_before']<=now<key['not_after'],'revoked-or-expired-key')
    require(p.get('role')==key['role'] and p.get('subject')==key['subject'],'signer-role-or-subject')
    maximum=7*86400 if purpose=='private-beta-decision' else 30*86400
    require(integer(p.get('issued_at'),key['not_before'],now) and integer(p.get('expires_at'),now+1,key['not_after'])
            and p['expires_at']-p['issued_at']<=maximum,'signature-time-window')
    require(isinstance(envelope['signature'],str) and len(envelope['signature'])<=128,'signature-bound')
    try:
        signature=base64.b64decode(envelope['signature'],validate=True)
        require(len(signature)==64,'signature-length')
        Ed25519PublicKey.from_public_bytes(bytes.fromhex(key['public_key_hex'])).verify(signature,DOMAIN+canonical(p))
    except Exception as error:
        raise ValueError('signature-verification-failed') from error
    return p,key


def inventory(candidate, base):
    records=candidate['files']; require(isinstance(records,list) and 1<=len(records)<=3000,'file-inventory-bound')
    ids={}; names=set(); total=0
    for record in records:
        require(isinstance(record,dict) and set(record)=={'id','path','sha256','kind'},'file-record-fields')
        require(isinstance(record['path'],str) and record['path'].startswith('data/'), 'file-path-must-use-data-directory')
        require(identifier(record['id']) and record['id'] not in ids and record['path'].casefold() not in names and digest(record['sha256']),'file-record-identity')
        require(record['kind'] in {'artifact','binary','report','log','policy','document','migration'},'file-kind')
        path=safe_path(base,record['path']); require(file_sha(path)==record['sha256'],'file-hash-mismatch:'+record['id'])
        total+=path.stat().st_size; require(total<=TOTAL_LIMIT,'bundle-total-bound')
        ids[record['id']]=(record,path); names.add(record['path'].casefold())
    return ids


def artifact(data, release):
    prefix=b'WORLDWRIGHT-RELEASE-V1\n'
    require(len(data)<=len(prefix)+9+16384+32*1024*1024 and data.startswith(prefix),'artifact-header-or-bound')
    offset=len(prefix); header=data[offset:offset+9]
    require(re.fullmatch(b'[0-9a-f]{8}\n',header) is not None,'artifact-length-header')
    size=int(header[:8],16); require(1<=size<=16384,'artifact-manifest-bound')
    offset+=9; original=data[offset:offset+size]; m=parse(original); payload=data[offset+size:]
    require(isinstance(m,dict) and list(m)==['manifest_version','release_id','content_schema_version','game_compatibility','payload','provenance'],'artifact-manifest-fields')
    require(json.dumps(m,separators=(',',':'),ensure_ascii=False,allow_nan=False).encode()==original,'artifact-canonical-manifest')
    require(type(m['manifest_version']) is int and m['manifest_version']==1 and type(m['content_schema_version']) is int and m['content_schema_version']==1,'artifact-schema')
    require(m['release_id']==release['release_id'] and m['game_compatibility']=={'game_id':release['game_id'],'contract_version':1},'artifact-game-release-mismatch')
    require(type(m['game_compatibility'].get('contract_version')) is int,'artifact-game-version-type')
    p=m['payload']; require(set(p)=={'media_type','byte_length','sha256'} and p['media_type']=='application/vnd.worldwright.world+json','artifact-payload-fields')
    require(integer(p['byte_length'],1,32*1024*1024) and p['byte_length']==len(payload) and sha(payload)==p['sha256']==release['payload_sha256'],'artifact-payload-mismatch')
    require(sha(data)==release['artifact_sha256'],'artifact-digest-mismatch')
    require(set(m['provenance'])=={'source_revision','builder_identity'} and all(text(v,128) for v in m['provenance'].values()),'artifact-provenance')
    require(m['provenance']['source_revision']==release['content_source_revision'],'artifact-content-source-revision')
    if release['content_source_revision'].startswith('studio-r'):
        matched=re.fullmatch(r'studio-r([1-9][0-9]*)-([0-9a-f]{64})',release['content_source_revision'])
        require(matched is not None and 1<=int(matched[1])<=10000 and matched[2]==release['payload_sha256'],'artifact-studio-source')
    # Deliberately not a replacement for Rust content validation/graph checks.
    return m


def native_report(path, report, expected_os, release, ids):
    require(report.get('track')=='R0' and report.get('status')=='passed' and report.get('host_os')==expected_os and report.get('toolchain')=='1.85.1','native-report-not-passed')
    if expected_os=='Linux': require(report.get('distribution')=={'id':'ubuntu','version':'24.04'},'native-linux-baseline')
    require(('-linux-' if expected_os=='Linux' else '-windows-') in report.get('host_target',''),'native-host-target')
    for key in ('commit','source_sha256','lock_sha256'): require(report['source'][key]==release[key],'native-release-tuple')
    entries=report['source'].get('inventory')
    require(isinstance(entries,list) and 1<=len(entries)<=10000 and sha(json.dumps(entries,sort_keys=True).encode())==release['source_sha256'],'native-source-inventory-hash')
    require(len({v['path'] for v in entries})==len(entries) and all(digest(v.get('sha256_lf_text')) for v in entries),'native-source-inventory-records')
    lock=[v for v in entries if v['path']=='Cargo.lock']; require(len(lock)==1 and lock[0]['sha256_lf_text']==release['lock_sha256'],'native-lock-inventory')
    require(type(report['source'].get('dirty')) is bool and report['source']['dirty']==(release['worktree_state']=='documented_patch'),'native-worktree-state')
    required={'node-version','helm-version','cargo-version','rustc-version','rustc-details','sysroot','locked-graph','fmt','check','clippy','test','build','rustdoc','worldwright-version','worldwright-studio-version','worldwright-server-version','receipt-list','receipt-pg','required-helm','browser-ids','studio-bounds','p2-contracts','p2-negatives'}
    if expected_os=='Linux': required.add('real-first-run')
    phases=report.get('phases'); require(isinstance(phases,list) and 1<=len(phases)<=64,'native-phases')
    seen=set(); inventory_paths={v[1] for v in ids.values()}
    for phase in phases:
        require(isinstance(phase,dict) and identifier(phase.get('name')) and phase['name'] not in seen,'native-phase-identity')
        seen.add(phase['name']); require(type(phase.get('exit_code')) is int and phase['exit_code']==0 and phase.get('status')=='passed' and phase.get('interruption') is None,'native-phase-failed')
        log=safe_path(path.parent,phase['name']+'.log')
        require(log in inventory_paths and file_sha(log)==phase.get('log_sha256'),'native-log-not-in-candidate-or-changed')
    require(required<=seen,'native-required-phase-missing')
    for name in ('worldwright','worldwright-studio','worldwright-server'):
        binary_digest=report.get('binaries',{}).get(name,{}).get('sha256')
        require(digest(binary_digest) and any(v[0]['kind']=='binary' and v[0]['sha256']==binary_digest for v in ids.values()),'native-binary-not-in-candidate')


def beta_report(v, release):
    require(type(v.get('schema_version')) is int and v['schema_version']==1 and type(v.get('profile_version')) is int and v['profile_version']==1 and v.get('profile')=='beta-250-v1','fixed-beta-profile')
    for key,num in {'target_clients':250,'connected_clients':250,'expected_commands':7500,'completed_commands':7500}.items(): require(integer(v.get(key),num,num),'fixed-beta-dimensions')
    failures=v.get('failures'); require(isinstance(failures,dict) and set(failures)=={'login','attach','command','protocol','disconnect','readiness','internal'} and all(integer(x,0,0) for x in failures.values()),'beta-failure-inventory')
    p=v.get('latency',{}); require(set(p)=={'count','min_us','p50_us','p95_us','p99_us','max_us'} and all(integer(x) for x in p.values()) and p['count']==7500,'beta-latency-types')
    require(p['min_us']<=p['p50_us']<=p['p95_us']<=p['p99_us']<=p['max_us']<10000000 and p['p95_us']<=750000 and p['p99_us']<=1500000,'beta-thresholds')
    require(integer(v.get('elapsed_ms'),1,90000) and integer(v.get('readiness_checks'),1) and v.get('passed') is True and v.get('failure_reasons')==[],'beta-not-passed')
    require(v.get('storage_expectation')=='operator-supplied-postgresql' and v.get('server_revision')==release['commit'] and v.get('artifact_digest')==release['artifact_sha256'],'beta-target')
    require(integer(v.get('throughput_milli_commands_per_second')) and v['throughput_milli_commands_per_second']==7500*1000000//v['elapsed_ms'],'beta-throughput')
    require(isinstance(v.get('run_nonce'),str) and re.fullmatch('[0-9a-f]{16}',v['run_nonce']),'beta-run-identity')
    require(integer(v.get('started_unix_seconds'),1) and integer(v.get('ended_unix_seconds'),v['started_unix_seconds']) and (v['ended_unix_seconds']-v['started_unix_seconds']+2)*1000>=v['elapsed_ms'],'beta-time-coverage')


def check_candidate(candidate, base, source, migration_inventory, keys, now):
    fields={'schema_version','kind','release','files','primary','migration_files','policies','gate_certificates','exceptions','open_blockers','notes'}
    require(set(candidate)==fields and type(candidate['schema_version']) is int and candidate['schema_version']==1 and candidate['kind']=='r7-private-beta-candidate','candidate-schema')
    require(candidate['open_blockers']==[],'open-prerequisite-blockers')
    release=candidate['release']
    require(isinstance(release,dict) and set(release)=={'release_id','commit','source_sha256','lock_sha256','patch_sha256','worktree_state','artifact_sha256','payload_sha256','content_source_revision','game_id','contract_version','image_reference','namespace_uid','deployment_uid','migrations','soak_seconds'},'release-tuple-fields')
    require(identifier(release['release_id']) and isinstance(release['commit'],str) and re.fullmatch('(?:[0-9a-f]{40}|[0-9a-f]{64})',release['commit']),'release-identity')
    for field in ('source_sha256','lock_sha256','patch_sha256','artifact_sha256','payload_sha256'): require(digest(release[field]),'release-digest')
    require(release['game_id'] in ('worldwright-reference','worldwright-expedition') and type(release['contract_version']) is int and release['contract_version']==1,'release-game')
    require(text(release['image_reference'],512) and re.fullmatch(r'[a-zA-Z0-9./:_-]+@sha256:[0-9a-f]{64}',release['image_reference']),'immutable-image-reference')
    require(text(release['namespace_uid'],128) and text(release['deployment_uid'],128) and integer(release['soak_seconds'],1800,3000),'deployment-or-soak-scope')
    for key in ('commit','source_sha256','lock_sha256','patch_sha256'): require(source[key]==release[key],'current-source-tuple-mismatch')
    require(release['worktree_state']==('documented_patch' if source['dirty'] else 'clean'),'worktree-state')
    require(release['migrations']==migration_inventory and len(migration_inventory)>0,'migration-inventory-mismatch')
    ids=inventory(candidate,base)
    require(isinstance(candidate['primary'],dict) and set(candidate['primary'])==PRIMARY and all(v in ids for v in candidate['primary'].values()),'primary-evidence-incomplete')
    def primary(name): return ids[candidate['primary'][name]][1]
    manifest=artifact(raw(primary('artifact'),34*1024*1024),release)
    for name,os_name in (('native-ubuntu','Linux'),('native-windows','Windows')): native_report(primary(name),load(primary(name)),os_name,release,ids)
    require(set(candidate['migration_files'])=={x['name'] for x in migration_inventory},'migration-file-inventory')
    for item in migration_inventory:
        ref=candidate['migration_files'][item['name']]; require(ref in ids and ids[ref][0]['kind']=='migration','migration-file-reference')
        require(hashlib.sha384(raw(ids[ref][1])).hexdigest()==item['sha384'],'migration-file-hash')
    beta_report(load(primary('beta')),release)
    soak=load(primary('soak')); require(soak.get('kind')=='r4-supplemental-scenarios' and soak.get('status')=='passed' and soak.get('source_sha256')==release['source_sha256'] and soak.get('artifact_digest')==release['artifact_sha256'],'soak-report')
    cases=soak.get('cases'); expected={'speech-fanout','reward-replay','transfer-replay','slow-reader','reconnect','idle-heartbeat','soak'}
    require(isinstance(cases,list) and len(cases)==len(expected) and {c['case'] for c in cases}==expected and all(c.get('status')=='passed' and integer(c.get('observations'),1) for c in cases),'soak-case-inventory')
    duration=next(c for c in cases if c['case']=='soak').get('duration_seconds')
    require(type(duration) in (int,float) and math.isfinite(duration) and duration>=release['soak_seconds'],'soak-duration')
    dep=load(primary('deployment'))
    require(dep.get('kind')=='staging-collection' and dep.get('status')=='collected_not_closed' and dep.get('source',{}).get('source_sha256')==release['source_sha256'],'deployment-collection')
    for field in ('image_reference','namespace_uid','deployment_uid'): require(dep.get(field)==release[field],'deployment-tuple')
    require(dep.get('artifact_digest')==release['artifact_sha256'],'deployed-artifact')
    sbom=load(primary('sbom')); require(sbom.get('bomFormat')=='CycloneDX' and sbom.get('specVersion')=='1.6' and isinstance(sbom.get('components'),list) and sbom['components'],'sbom-required')
    components=sbom['components']; require(len(components)<=10000 and len({v['bom-ref'] for v in components})==len(components) and all(text(v.get('name'),256) and text(v.get('version'),128) for v in components),'sbom-component-inventory')
    properties=sbom.get('metadata',{}).get('properties')
    require(isinstance(properties,list) and len(properties)<=32 and len({v['name'] for v in properties})==len(properties),'sbom-property-inventory')
    properties={v['name']:v['value'] for v in properties}
    for key,value in {'commit':release['commit'],'source_patch_sha256':release['patch_sha256'],'cargo_lock_sha256':release['lock_sha256'],'artifact_sha256':release['artifact_sha256'],'container_digest':release['image_reference'].split('@')[1]}.items():
        require(properties.get(key)==value,'sbom-release-tuple')
    policies={}
    require(isinstance(candidate['policies'],dict) and set(candidate['policies'])==set(POLICY_ROLES),'policy-inventory')
    for name,ref in candidate['policies'].items():
        require(ref in ids and ids[ref][0]['kind']=='policy','policy-reference')
        p=load(ids[ref][1]); require(set(p)=={'schema_version','policy','status','scope','owner','reference','details'} and type(p['schema_version']) is int and p['schema_version']==1,'policy-schema')
        require(p['policy']==name and p['status']=='adopted' and p['scope']=='private-beta' and text(p['owner'],256) and text(p['reference']) and isinstance(p['details'],dict) and p['details'],'policy-not-adopted')
        policies[name]=p
    require(policies['privacy-retention']['details'].get('preserves_business_uniqueness') is True,'privacy-business-uniqueness')
    for field,num in (('rpo_seconds',86400),('rto_seconds',14400),('audit_days',90)):
        require(integer(policies['recovery']['details'].get(field),num,num),'recovery-target-adoption')
    require(text(policies['security-reporting']['details'].get('private_channel')) and text(policies['security-reporting']['details'].get('response_coverage')),'private-reporting-coverage')
    require(text(policies['support']['details'].get('coverage')) and text(policies['support']['details'].get('service_owner'),256),'service-owner-coverage')
    matrix=policies['runtime-matrix']['details'].get('matrix')
    matrix_names={'chromium-x11','chromium-wayland','firefox-x11','firefox-wayland','windows-native'}
    require(isinstance(matrix,list) and len(matrix)==len(matrix_names) and {v['case'] for v in matrix}==matrix_names,'runtime-matrix-incomplete')
    for item in matrix:
        require(text(item.get('version'),128) and item['version'] not in ('latest','unknown','*') and item.get('evidence') in ids and text(item.get('runtime_dependencies'),2048),'runtime-matrix-unobserved')
    require(isinstance(candidate['notes'],list) and 1<=len(candidate['notes'])<=32 and all(text(v,2048) for v in candidate['notes']),'release-notes')
    require(isinstance(candidate['exceptions'],list) and len(candidate['exceptions'])<=10,'exception-bound')
    exceptions=set()
    for exception in candidate['exceptions']:
        require(set(exception)=={'id','category','owner','reason','expires_at','evidence'} and identifier(exception['id']) and exception['id'] not in exceptions,'exception-identity')
        exceptions.add(exception['id']); require(exception['category']=='nonblocking-operational-limitation' and text(exception['owner'],256) and text(exception['reason'],2048) and exception['evidence'] in ids and integer(exception['expires_at'],now+1,now+7*86400),'expired-or-waiving-exception')
    require(isinstance(candidate['gate_certificates'],dict) and set(candidate['gate_certificates'])==set(GATES),'gate-certificate-inventory')
    certificates={}; bound={'tuple_sha256':sha(canonical(release)), 'files_sha256':sha(canonical(candidate['files']))}
    for gate,name in candidate['gate_certificates'].items():
        require(name=='gates/'+gate.lower()+'.json','gate-certificate-path')
        envelope=load(safe_path(base,name)); p,key=signed(envelope,keys,'prerequisite-closure',now)
        require(set(p)=={'schema_version','purpose','key_id','role','subject','issued_at','expires_at','gate','status','tuple_sha256','files_sha256','cases'},'gate-payload-fields')
        require(p['gate']==gate and p['status']=='closed' and key['role']==GATE_ROLES[gate] and all(p.get(k)==v for k,v in bound.items()),'gate-not-closed-or-tuple-differs')
        items=p['cases']; require(isinstance(items,list) and len(items)==len(GATES[gate]) and {v['case'] for v in items}==GATES[gate],'gate-case-inventory')
        for item in items:
            require(set(item)=={'case','status','evidence'} and item['status']=='passed' and isinstance(item['evidence'],list) and 1<=len(item['evidence'])<=16 and len(set(item['evidence']))==len(item['evidence']) and all(ref in ids for ref in item['evidence']),'gate-case-not-observed')
        certificates[gate]=envelope
    return ids,policies,certificates,manifest


def authorize(candidate, base, source, migrations, trust, approval_paths, trust_sha256, now):
    keys=trust_keys(trust,now)
    ids,policies,certificates,manifest=check_candidate(candidate,base,source,migrations,keys,now)
    require(len(approval_paths)==len(ROLES),'five-independent-role-decisions-required')
    binding={'candidate_sha256':sha(canonical(candidate)), 'gate_certificates_sha256':sha(canonical(certificates)), 'trust_sha256':trust_sha256}
    reviews={}; subjects=set(); public_keys=set(); expiry=[]
    for path in approval_paths:
        envelope=load(path); p,key=signed(envelope,keys,'private-beta-decision',now)
        require(set(p)=={'schema_version','purpose','key_id','role','subject','issued_at','expires_at','decision','candidate_sha256','gate_certificates_sha256','trust_sha256'},'decision-payload-fields')
        require(p['role'] not in reviews and p['subject'] not in subjects and key['public_key_hex'] not in public_keys,'reviewer-independence')
        require(p['decision']=='approve' and all(p.get(k)==v for k,v in binding.items()),'decision-denied-or-binding-mismatch')
        require(p['subject']!=manifest['provenance']['builder_identity'],'builder-cannot-approve-own-release')
        reviews[p['role']]=envelope; subjects.add(p['subject']); public_keys.add(key['public_key_hex']); expiry.append(p['expires_at'])
    require(set(reviews)==ROLES,'role-inventory')
    for name,role in POLICY_ROLES.items(): require(policies[name]['owner']==reviews[role]['payload']['subject'],'policy-owner-not-authorized-reviewer')
    require(policies['support']['details']['service_owner']==reviews['service']['payload']['subject'],'service-owner-mismatch')
    for exception in candidate['exceptions']:
        require(exception['owner'] in subjects,'exception-owner-not-approved'); expiry.append(exception['expires_at'])
    expiry += [v['payload']['expires_at'] for v in certificates.values()]
    # Rehash after all parsing/signature work; do not bless mutable input drift.
    inventory(candidate,base)
    for gate,name in candidate['gate_certificates'].items(): require(load(safe_path(base,name))==certificates[gate],'gate-changed-during-check')
    return {'decision':'GO','supported_private_beta_authorized':True,'public_publication_authorized':False,
            'production_rollout_authorized':False,'release':candidate['release'],**binding,
            'approved_at':now,'expires_at':min(expiry),'reviewers':{role:v['payload']['subject'] for role,v in reviews.items()}}, ids, certificates, reviews
