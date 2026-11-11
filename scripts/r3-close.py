"""R3 same-source closure. Requires actual collectors and independent service adoption."""
from pathlib import Path
import argparse
import base64
import hashlib
import importlib.util
import json
import re
import subprocess
import sys
import time
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey

ROOT=Path(__file__).resolve().parents[1]
AUTO={'startup-tls','single-writer-reconstruction','db-role-audit','websocket-idle-origin-operator-denial','secret-rotation-restart','failed-rollout-helm-rollback'}
MANUAL={'networkpolicy-live-positive-negative','controlled-termination','content-rollback-schema-review','secret-rotation-revocation'}
PHASES={'r0-r2-prerequisites','chart-matrix','namespace-approval','db-role-audit','install','deployment','actual-pod-image','rotate-runtime-secret','rotated-pod-image','helm-status','failed-rollout','helm-rollback','rollback-pod-image'}

def require(value,message):
    if not value: raise ValueError(message)
def sha(path): return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def read(path):
    path=Path(path); require(path.is_file() and not path.is_symlink() and path.stat().st_size<=4*1024*1024,'bounded regular report required'); return json.loads(path.read_text())
def records(value,expected):
    require(isinstance(value,list) and len(value)==len(expected),'case inventory incomplete/duplicate')
    require(all(isinstance(item,dict) and isinstance(item.get('case'),str) and item.get('status')=='passed' for item in value),'required cases must actually pass')
    require({item['case'] for item in value}==expected,'required case missing/unknown')
def signature(value,key_path):
    require(value.get('kind')=='r3-service-adoption' and value.get('status')=='adopted','actual service adoption required')
    reviewer=value.get('reviewer')
    require(isinstance(reviewer,str) and reviewer.strip() and len(reviewer)<=256 and all(char.isprintable() for char in reviewer),'nonblank bounded reviewer text without controls required')
    require(type(value.get('expires_at')) is int and int(time.time())<value['expires_at']<=int(time.time())+30*86400,'bounded unexpired adoption required')
    key=serialization.load_pem_public_key(Path(key_path).read_bytes()); require(isinstance(key,Ed25519PublicKey),'independently trusted Ed25519 public key required')
    payload={k:v for k,v in value.items() if k!='signature'}
    key.verify(base64.b64decode(value['signature'],validate=True),json.dumps(payload,sort_keys=True,separators=(',',':')).encode())
def check_phases(report,parent,expected):
    phases=report.get('phases'); require(isinstance(phases,list) and len(phases)==len(expected),'required phases missing/duplicate')
    require(all(isinstance(item,dict) and isinstance(item.get('name'),str) for item in phases),'invalid phase record')
    require({item['name'] for item in phases}==expected,'phase inventory differs')
    for item in phases:
        name=item['name']; require(re.fullmatch(r'[a-z0-9-]+',name),'safe phase name required')
        path=parent/(name+'.log'); require(path.is_file() and not path.is_symlink() and path.stat().st_size<=128*1024*1024 and sha(path)==item.get('log_sha256'),'phase log missing/modified')
        require(item.get('interruption') is None,'interrupted/truncated phase cannot be acceptance evidence')
        code=item.get('exit_code')
        require(type(code) is int,'integer process exit code required')
        declared=item.get('expected_exit_code',1 if name=='failed-rollout' else 0)
        require(type(declared) is int and declared==(1 if name=='failed-rollout' else 0),'declared phase expectation differs')
        require(code>0 if name=='failed-rollout' else code==0,'phase outcome differs')

def role_report(value,collection):
    # Local permission-fixture success is not a deployed database-role proof.
    require(type(value.get('schema_version')) is int and value['schema_version']==1 and value.get('track')=='R3' and value.get('kind')=='database-role-audit' and value.get('status')=='passed' and value.get('full_r3_closed') is False,'actual database-role-audit report required')
    require(value.get('identity_checks')=='passed' and value.get('recovery_authority_adopted') is False and type(value.get('runtime_setup_applied')) is bool,'complete role identity audit required')
    expected_count=len(list((ROOT/'migrations').glob('*.sql')))
    require(expected_count>0 and type(value.get('migration_count')) is int and value['migration_count']==expected_count,'current migration inventory count required')
    require(type(value.get('table_count')) is int and value['table_count']>0,'actual audited table count required')
    names=[value.get(key) for key in ('runtime_role','migration_role','recovery_role')]
    require(all(isinstance(name,str) and re.fullmatch(r'[a-z_][a-z0-9_]{0,62}',name) for name in names) and len(set(names))==3,'distinct audited role identities required')
    for key in ('runtime_role','migration_role','recovery_role','schema'):
        require(value.get(key)==collection.get(key) and isinstance(value.get(key),str),'audited role/schema differs from collection: '+key)

def close(args):
    collection=read(args.collection); image=read(args.image); adoption=read(args.adoption); r2=read(args.r2_arguments)
    require(collection.get('kind')=='staging-collection' and collection.get('status')=='collected_not_closed','actual complete staging collection required')
    records(collection.get('cases'),AUTO); check_phases(collection,args.collection.parent,PHASES)
    require(image.get('kind')=='image-build' and image.get('status')=='passed','actual image build/run required')
    check_phases(image,args.image.parent,{'revision','engine','buildx','build','manifest','pull','inspect','non-root-linkage'})
    spec=importlib.util.spec_from_file_location('r3_close_source',ROOT/'scripts/r0-verify.py'); module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
    source=module.source_identity(); require(source==collection['source']==image['source'],'image/staging/current source tuple differs')
    require(collection['image_reference']==image['image_reference'],'staging image differs')
    require(sha(args.collection.parent/'roles.json')==collection['roles_sha256'],'actual role audit missing/modified')
    role_report(read(args.collection.parent/'roles.json'),collection)
    signature(adoption,args.approval_public_key)
    for field,expected in {'source_sha256':source['source_sha256'],'collection_sha256':sha(args.collection),'image_report_sha256':sha(args.image),'image_reference':image['image_reference'],'artifact_digest':collection['artifact_digest'],'namespace_uid':collection['namespace_uid'],'deployment_uid':collection['deployment_uid']}.items():
        require(adoption.get(field)==expected,'adoption tuple differs: '+field)
    records(adoption.get('cases'),MANUAL)
    for item in adoption['cases']:
        proofs=item.get('evidence'); require(isinstance(proofs,list) and proofs,'actual manual observations/logs required')
        for proof in proofs:
            relative=Path(proof['path']); require(not relative.is_absolute() and '..' not in relative.parts,'contained evidence path required')
            path=args.adoption.parent/relative
            require(path.resolve().is_relative_to(args.adoption.parent.resolve()) and not path.is_symlink() and path.is_file() and path.stat().st_size<=128*1024*1024,'bounded regular evidence required')
            require(sha(path)==proof['sha256'],'adopted evidence modified')
    expected={'collection','ubuntu-r0','windows-r0','process-report','controlled-report','provider-report','approval-public-key'}
    require(set(r2)==expected,'every current R2 prerequisite required')
    command=[sys.executable,'scripts/r2-verify.py','close']
    for option in sorted(expected): command+=['--'+option,str(r2[option])]
    spec=importlib.util.spec_from_file_location('r3_close_io',ROOT/'scripts/r3_io.py'); io=importlib.util.module_from_spec(spec); spec.loader.exec_module(io)
    result=io.collect_phase(command,180,cwd=ROOT)
    require(result['interruption'] is None and result['returncode']==0,'R0/R2 current prerequisite closure failed/interrupted')
    print('R3 evidence and independently trusted service adoption verified. No R4 load/soak or production readiness is implied.')

def main():
    parser=argparse.ArgumentParser()
    for option in ('collection','image','adoption','r2-arguments','approval-public-key'): parser.add_argument('--'+option,type=Path,required=True)
    args=parser.parse_args()
    try: close(args); return 0
    except Exception as error:
        print('R3 NOT CLOSED:',type(error).__name__,str(error)); return 1
if __name__=='__main__': raise SystemExit(main())
