"""R7 candidate/init, reviewer handoff, go/no-go, private packet and archive verification."""
from pathlib import Path
import argparse
import html
import json
import os
import shutil
import sys
import time
import r7_core as c
import worldwright_local as local


def new_output(path, forbidden=()):
    path=local.new_output(path, forbidden)
    path.mkdir(mode=0o700,parents=False)
    return path


def json_new(path,value): c.write_new(path,json.dumps(value,sort_keys=True,indent=2,allow_nan=False).encode()+b'\n')


def template(output):
    output=new_output(output)
    release={name:None for name in ('release_id','commit','source_sha256','lock_sha256','patch_sha256','worktree_state','artifact_sha256','payload_sha256','content_source_revision','game_id','contract_version','image_reference','namespace_uid','deployment_uid','soak_seconds')}
    release['migrations']=[]
    candidate={'schema_version':1,'kind':'r7-private-beta-candidate','release':release,'files':[],
               'primary':{key:None for key in sorted(c.PRIMARY)},'migration_files':{},
               'policies':{key:None for key in sorted(c.POLICY_ROLES)},
               'gate_certificates':{key:'gates/'+key.lower()+'.json' for key in c.GATES},
               'exceptions':[],'open_blockers':['R1-R6 current same-release acceptance is not supplied','R6 feature work and native validation remain unfinished','Known full-migration syntax blocker remains unresolved'],
               'notes':['DRAFT ONLY: no private beta release is authorized.']}
    json_new(output/'candidate.json',candidate)
    json_new(output/'trust-store.template.json',{'schema_version':1,'keys':[
        {'key_id':role+'-key','role':role,'subject':None,'public_key_hex':None,'not_before':None,'not_after':None,'revoked':False} for role in sorted(c.ROLES)]})
    policies=output/'policy-templates'; policies.mkdir(mode=0o700)
    for name in c.POLICY_ROLES:
        json_new(policies/(name+'.json'),{'schema_version':1,'policy':name,'status':'pending','scope':'private-beta','owner':None,'reference':None,'details':{}})
    print('R7 DRAFT / NO-GO:',output/'candidate.json')


def review_templates(candidate_path, output):
    candidate=c.load(candidate_path); output=new_output(output,(Path(candidate_path).parent,))
    binding={'tuple_sha256':c.sha(c.canonical(candidate['release'])),'files_sha256':c.sha(c.canonical(candidate['files']))}
    for gate,role in c.GATE_ROLES.items():
        p={'schema_version':1,'purpose':'prerequisite-closure','key_id':None,'role':role,'subject':None,
           'issued_at':None,'expires_at':None,'gate':gate,'status':'pending',**binding,
           'cases':[{'case':name,'status':'pending','evidence':[]} for name in sorted(c.GATES[gate])]}
        json_new(output/(gate.lower()+'.unsigned.json'),p)
    certs={}
    for gate,name in candidate['gate_certificates'].items():
        path=c.safe_path(Path(candidate_path).parent,name)
        if path.is_file(): certs[gate]=c.load(path)
    for role in sorted(c.ROLES):
        json_new(output/(role+'.unsigned.json'),{'schema_version':1,'purpose':'private-beta-decision','key_id':None,
            'role':role,'subject':None,'issued_at':None,'expires_at':None,'decision':'pending',
            'candidate_sha256':c.sha(c.canonical(candidate)),
            'gate_certificates_sha256':c.sha(c.canonical(certs)) if set(certs)==set(c.GATES) else None,'trust_sha256':None})
    print('Unsigned review requests only; no approval or signature created:',output)


def signing_bytes(payload_path, output):
    p=c.load(payload_path)
    c.require(p.get('purpose') in ('prerequisite-closure','private-beta-decision'),'unsupported-signing-purpose')
    output=new_output(output,(Path(payload_path).parent,))
    json_new(output/'payload.json',p)
    c.write_new(output/'signing-input.bin',c.DOMAIN+c.canonical(p))
    print('Canonical bytes only. Sign using the independently managed reviewer key; this command grants nothing.')


def migration_inventory(root):
    import hashlib
    return [{'name':p.name,'sha384':hashlib.sha384(c.raw(p)).hexdigest()} for p in sorted((Path(root)/'migrations').glob('*.sql'))]


def trusted(path, expected, base):
    c.require(path is not None and c.digest(expected),'independent-trust-store-and-pin-required')
    path=Path(path).resolve(strict=True)
    c.require(not path.is_relative_to(Path(base).resolve()),'trust-store-must-be-outside-candidate')
    data=c.raw(path); c.require(c.sha(data)==expected,'trust-store-pin-mismatch')
    return c.parse(data)


def notes(decision,candidate):
    release=decision['release']
    lines=['# Worldwright private beta: '+html.escape(release['release_id']),
           '', '**Authorized scope: private beta only. No public registry publication or production rollout.**',
           '', 'Decision expires at Unix timestamp '+str(decision['expires_at'])+'.',
           '', '## Release identity', '', 'Commit: '+release['commit'], 'Artifact: '+release['artifact_sha256'],
           'Game: '+release['game_id'], 'Image: '+release['image_reference'], '', '## Changes and scope','']
    lines += ['- '+html.escape(value) for value in candidate['notes']]
    lines += ['', '## Bounded exceptions','']
    lines += ['- '+html.escape(e['id']+': '+e['reason'])+'; expires '+str(e['expires_at']) for e in candidate['exceptions']] or ['- None.']
    lines += ['', '## Reviewers','']+[ '- '+role+': '+html.escape(subject) for role,subject in sorted(decision['reviewers'].items())]
    lines += ['', 'Native/runtime, recovery, support, reporting, licensing and privacy scope are in the signed candidate and adopted policy files.',
              'Archive verification is not a fresh live deployment check. Re-evaluate before relying on a changed or expired decision.','']
    return '\n'.join(lines).encode()


def copy_checked(source,destination,expected,executable=False):
    destination.parent.mkdir(parents=True,exist_ok=True,mode=0o700)
    mode=0o700 if executable else 0o600
    with Path(source).open('rb') as input_file, os.fdopen(os.open(destination,os.O_CREAT|os.O_EXCL|os.O_WRONLY,mode),'wb') as output:
        total=0
        while True:
            block=input_file.read(65536)
            if not block: break
            total+=len(block); c.require(total<=c.FILE_LIMIT,'copy-bound')
            output.write(block)
        output.flush(); os.fsync(output.fileno())
    c.require(c.file_sha(destination)==expected,'input-changed-during-copy')


def packet(output,candidate,decision,ids,certificates,reviews):
    destination=output/'packet'; destination.mkdir(mode=0o700)
    json_new(destination/'candidate.json',candidate)
    for record,path in ids.values(): copy_checked(path,c.safe_path(destination,record['path']),record['sha256'],record['kind']=='binary')
    (destination/'gates').mkdir(mode=0o700)
    for gate,envelope in certificates.items(): json_new(destination/'gates'/(gate.lower()+'.json'),envelope)
    (destination/'approvals').mkdir(mode=0o700)
    for role,envelope in reviews.items(): json_new(destination/'approvals'/(role+'.json'),envelope)
    json_new(destination/'decision.json',decision)
    c.write_new(destination/'release-notes.md',notes(decision,candidate))
    files=[{'path':p.relative_to(destination).as_posix(),'sha256':c.file_sha(p)} for p in sorted(destination.rglob('*')) if p.is_file()]
    json_new(destination/'packet-manifest.json',{'schema_version':1,'kind':'r7-private-beta-packet','candidate_sha256':decision['candidate_sha256'],'files':files})
    return destination


def evaluate(args):
    output=new_output(args.output,(args.candidate.parent,))
    c.write_new(output/'events.jsonl',b'{"event":"started","track":"R7"}\n')
    report={'schema_version':1,'track':'R7','decision':'NO-GO','supported_private_beta_authorized':False,
            'public_publication_authorized':False,'production_rollout_authorized':False,'evaluated_at':int(time.time()),'blockers':[]}
    try:
        candidate=c.load(args.candidate)
        c.require(candidate.get('open_blockers')==[],'open-prerequisite-blockers')
        trust=trusted(args.trust_store,args.trust_store_sha256,args.candidate.parent)
        source=c.source_identity(c.ROOT); migrations=migration_inventory(c.ROOT)
        decision,ids,certificates,reviews=c.authorize(candidate,args.candidate.parent,source,migrations,trust,args.approval,args.trust_store_sha256,report['evaluated_at'])
        c.require(c.load(args.candidate)==candidate,'candidate-changed-during-check')
        c.require(c.source_identity(c.ROOT)==source,'source-changed-during-check')
        report.update(decision)
        if args.operation=='release':
            packet_path=packet(output,candidate,report,ids,certificates,reviews)
            verify_packet(packet_path,args.trust_store,args.trust_store_sha256,quiet=True)
            c.require(c.source_identity(c.ROOT)==source,'source-changed-during-packaging')
        c.require(c.sha(c.raw(args.trust_store))==args.trust_store_sha256,'trust-store-changed-during-check')
        c.require(int(time.time())<report['expires_at'],'decision-expired-during-check')
        if args.operation=='check': c.write_new(output/'release-notes.md',notes(report,candidate))
    except (Exception, KeyboardInterrupt) as error:
        report.update(decision='NO-GO',supported_private_beta_authorized=False)
        report['blockers']=[str(error) if isinstance(error,ValueError) else 'invalid-or-unavailable-input:'+type(error).__name__]
        if (output/'packet').exists():
            c.write_new(output/'packet'/'INCOMPLETE-NOT-AUTHORIZED.txt',b'Packaging failed. This directory is not a release.\n')
    json_new(output/'decision.json',report)
    with (output/'events.jsonl').open('ab') as log:
        log.write(c.canonical({'event':'finished','decision':report['decision']})+b'\n'); log.flush(); os.fsync(log.fileno())
    print('R7 '+report['decision']+': '+str(output/'decision.json'))
    if report['blockers']: print('Blockers: '+', '.join(report['blockers']))
    return 0 if report['decision']=='GO' else 1


def verify_packet(path,trust_path,pin,quiet=False):
    path=Path(path).resolve(strict=True); index=c.load(path/'packet-manifest.json')
    c.require(set(index)=={'schema_version','kind','candidate_sha256','files'} and type(index['schema_version']) is int and index['schema_version']==1 and index['kind']=='r7-private-beta-packet','packet-schema')
    c.require(isinstance(index['files'],list) and 1<=len(index['files'])<=3020,'packet-inventory-bound')
    names=set()
    for item in index['files']:
        c.require(set(item)=={'path','sha256'} and item['path'] not in names and c.digest(item['sha256']),'packet-record')
        names.add(item['path']); c.require(c.file_sha(c.safe_path(path,item['path']))==item['sha256'],'packet-file-changed')
    actual={p.relative_to(path).as_posix() for p in path.rglob('*') if p.is_file() and p!=path/'packet-manifest.json'}
    c.require(names==actual and not any(p.is_symlink() for p in path.rglob('*')),'packet-unlisted-missing-or-linked-file')
    candidate=c.load(path/'candidate.json'); r=candidate['release']
    source={key:r[key] for key in ('commit','source_sha256','lock_sha256','patch_sha256')}; source['dirty']=r['worktree_state']=='documented_patch'
    trust=trusted(trust_path,pin,path)
    decision,_,_,_=c.authorize(candidate,path,source,r['migrations'],trust,[path/'approvals'/(role+'.json') for role in sorted(c.ROLES)],pin,int(time.time()))
    recorded=c.load(path/'decision.json')
    c.require(type(recorded.get('schema_version')) is int and recorded['schema_version']==1 and recorded.get('track')=='R7'
              and c.integer(recorded.get('approved_at'),1,int(time.time())) and recorded['approved_at']<recorded.get('expires_at',0),'packet-decision-schema-time')
    if os.name=='posix':
        for item in candidate['files']:
            if item['kind']=='binary': c.require(c.safe_path(path,item['path']).stat().st_mode & 0o100,'packet-binary-not-executable')
    c.require(recorded.get('decision')=='GO' and recorded.get('supported_private_beta_authorized') is True and recorded.get('public_publication_authorized') is False and recorded.get('production_rollout_authorized') is False,'packet-decision')
    for key in ('candidate_sha256','gate_certificates_sha256','trust_sha256','release','expires_at','reviewers'):
        c.require(recorded.get(key)==decision[key],'packet-decision-binding')
    c.require(index['candidate_sha256']==decision['candidate_sha256'] and c.raw(path/'release-notes.md')==notes(recorded,candidate),'packet-notes-binding')
    if not quiet: print('R7 packet bytes and current reviewer signatures verified. Archive verification only; current checkout/deployment was NOT revalidated.')
    return 0


def main():
    parser=argparse.ArgumentParser(description=__doc__); sub=parser.add_subparsers(dest='operation',required=True)
    p=sub.add_parser('init'); p.add_argument('--output',type=Path,required=True)
    p=sub.add_parser('fingerprint'); p.add_argument('--output',type=Path,required=True)
    p=sub.add_parser('review-templates'); p.add_argument('--candidate',type=Path,required=True); p.add_argument('--output',type=Path,required=True)
    p=sub.add_parser('signing-bytes'); p.add_argument('--payload',type=Path,required=True); p.add_argument('--output',type=Path,required=True)
    for name in ('check','release'):
        p=sub.add_parser(name); p.add_argument('--candidate',type=Path,required=True); p.add_argument('--output',type=Path,required=True)
        p.add_argument('--trust-store',type=Path); p.add_argument('--trust-store-sha256'); p.add_argument('--approval',type=Path,action='append',default=[])
    p=sub.add_parser('verify-packet'); p.add_argument('--packet',type=Path,required=True); p.add_argument('--trust-store',type=Path,required=True); p.add_argument('--trust-store-sha256',required=True)
    args=parser.parse_args()
    try:
        local.configure_tools_environment()
        if args.operation=='init': template(args.output)
        elif args.operation=='fingerprint':
            output=new_output(args.output)
            json_new(output/'source.json',c.source_identity(c.ROOT))
            json_new(output/'migrations.json',migration_inventory(c.ROOT))
            print('Current source/migration identity only; no build, approval or release:',output)
        elif args.operation=='review-templates': review_templates(args.candidate,args.output)
        elif args.operation=='signing-bytes': signing_bytes(args.payload,args.output)
        elif args.operation=='verify-packet': return verify_packet(args.packet,args.trust_store,args.trust_store_sha256)
        else: return evaluate(args)
        return 0
    except Exception as error:
        print('R7 NO-GO:',str(error) if isinstance(error,ValueError) else type(error).__name__); return 1

if __name__=='__main__': raise SystemExit(main())
