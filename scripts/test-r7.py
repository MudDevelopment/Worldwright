"""Synthetic R7 policy/crypto/CLI tests. NEVER real R1-R6 acceptance or project approval."""
from pathlib import Path
import copy
import importlib.util
import json
import os
import subprocess
import tempfile
import time
import unittest
from types import SimpleNamespace
from unittest.mock import patch
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
import r7_core as c

spec=importlib.util.spec_from_file_location('r7_cli',Path(__file__).with_name('r7.py'))
cli=importlib.util.module_from_spec(spec); spec.loader.exec_module(cli)


class Fixture:
    def __init__(self,root):
        self.root=root; self.base=root/'candidate'; self.base.mkdir(); self.repo=root/'checkout'; self.repo.mkdir()
        (self.repo/'migrations').mkdir()
        (self.repo/'Cargo.lock').write_text('version = 4\n')
        (self.repo/'migrations/20260101000100_test.sql').write_text('-- synthetic migration only\n')
        def git(*args): subprocess.run(['git',*args],cwd=self.repo,check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        (self.repo/'.test-hooks').mkdir()
        git('init'); git('add','.'); git('-c','core.hooksPath=.test-hooks','-c','commit.gpgsign=false','-c','user.name=Synthetic Test','-c','user.email=test@example.invalid','commit','-m','synthetic fixture')
        self.source=c.source_identity(self.repo); self.migrations=cli.migration_inventory(self.repo)
        self.now=int(time.time()); self.private={role:Ed25519PrivateKey.generate() for role in c.ROLES}
        self.trust={'schema_version':1,'keys':[{'key_id':role+'-key','role':role,'subject':'synthetic-'+role,
            'public_key_hex':self.private[role].public_key().public_bytes(serialization.Encoding.Raw,serialization.PublicFormat.Raw).hex(),
            'not_before':self.now-100,'not_after':self.now+86400,'revoked':False} for role in sorted(c.ROLES)]}
        self.trust_path=root/'independent-test-trust.json'; self.store(self.trust_path,self.trust)
        self.files=[]
        self.release={'release_id':'synthetic-beta','commit':self.source['commit'],
            'source_sha256':self.source['source_sha256'],'lock_sha256':self.source['lock_sha256'],'patch_sha256':self.source['patch_sha256'],
            'worktree_state':'clean','artifact_sha256':None,'payload_sha256':None,'content_source_revision':self.source['commit'],'game_id':'worldwright-reference','contract_version':1,
            'image_reference':'registry.example.invalid/test@sha256:'+'a'*64,'namespace_uid':'synthetic-namespace','deployment_uid':'synthetic-deployment',
            'migrations':self.migrations,'soak_seconds':1800}
        payload=b'{"schema_version":1,"namespace":"synthetic"}'
        m={'manifest_version':1,'release_id':self.release['release_id'],'content_schema_version':1,
            'game_compatibility':{'game_id':'worldwright-reference','contract_version':1},
            'payload':{'media_type':'application/vnd.worldwright.world+json','byte_length':len(payload),'sha256':c.sha(payload)},
            'provenance':{'source_revision':self.source['commit'],'builder_identity':'synthetic-builder'}}
        encoded=json.dumps(m,separators=(',',':')).encode(); artifact=b'WORLDWRIGHT-RELEASE-V1\n'+f'{len(encoded):08x}\n'.encode()+encoded+payload
        self.release.update(artifact_sha256=c.sha(artifact),payload_sha256=c.sha(payload))
        self.add('artifact','data/world.wwr','artifact',artifact)
        self.primary={'artifact':'artifact'}
        migration_map={}
        for i,migration in enumerate(self.migrations):
            key='migration-'+str(i); migration_map[migration['name']]=key
            self.add(key,'data/migrations/'+migration['name'],'migration',(self.repo/'migrations'/migration['name']).read_bytes())
        phases={'node-version','helm-version','cargo-version','rustc-version','rustc-details','sysroot','locked-graph','fmt','check','clippy','test','build','rustdoc','worldwright-version','worldwright-studio-version','worldwright-server-version','receipt-list','receipt-pg','required-helm','browser-ids','studio-bounds','p2-contracts','p2-negatives'}
        for label,host in (('ubuntu','Linux'),('windows','Windows')):
            report={'schema_version':1,'track':'R0','status':'passed','host_os':host,'host_target':'x86_64-unknown-linux-gnu' if host=='Linux' else 'x86_64-pc-windows-msvc',
                    'toolchain':'1.85.1','source':{k:v for k,v in self.source.items() if k!='patch_sha256'},'phases':[],'binaries':{}}
            if host=='Linux': report['distribution']={'id':'ubuntu','version':'24.04'}
            for phase in sorted(phases | ({'real-first-run'} if host=='Linux' else set())):
                data=('SYNTHETIC TEST LOG ONLY: '+phase+'\n').encode()
                self.add(label+'-'+phase,'data/'+label+'/'+phase+'.log','log',data)
                report['phases'].append({'name':phase,'status':'passed','exit_code':0,'log_sha256':c.sha(data)})
            for binary in ('worldwright','worldwright-studio','worldwright-server'):
                data=('SYNTHETIC NOT AN EXECUTABLE '+label+binary).encode()
                self.add(label+'-'+binary+'-binary','data/'+label+'/'+binary+'.bin','binary',data)
                report['binaries'][binary]={'path':'synthetic-only','sha256':c.sha(data)}
            key='native-'+label; self.add(key,'data/'+label+'/r0-report.json','report',report); self.primary[key]=key
        beta={'schema_version':1,'profile_version':1,'profile':'beta-250-v1','target_clients':250,'connected_clients':250,'expected_commands':7500,'completed_commands':7500,
            'failures':{k:0 for k in ('login','attach','command','protocol','disconnect','readiness','internal')},'latency':{'count':7500,'min_us':1,'p50_us':2,'p95_us':3,'p99_us':4,'max_us':5},
            'elapsed_ms':60000,'readiness_checks':2,'passed':True,'failure_reasons':[],'storage_expectation':'operator-supplied-postgresql',
            'server_revision':self.release['commit'],'artifact_digest':self.release['artifact_sha256'],'throughput_milli_commands_per_second':125000,
            'run_nonce':'a'*16,'started_unix_seconds':self.now-60,'ended_unix_seconds':self.now}
        self.add('beta','data/beta.json','report',beta); self.primary['beta']='beta'
        soak={'kind':'r4-supplemental-scenarios','status':'passed','source_sha256':self.release['source_sha256'],'artifact_digest':self.release['artifact_sha256'],
            'cases':[{'case':name,'status':'passed','observations':1,**({'duration_seconds':1800} if name=='soak' else {})} for name in ('speech-fanout','reward-replay','transfer-replay','slow-reader','reconnect','idle-heartbeat','soak')]}
        self.add('soak','data/soak.json','report',soak); self.primary['soak']='soak'
        deployment={'kind':'staging-collection','status':'collected_not_closed','source':self.source,'artifact_digest':self.release['artifact_sha256'],
                    **{k:self.release[k] for k in ('image_reference','namespace_uid','deployment_uid')}}
        self.add('deployment','data/deployment.json','report',deployment); self.primary['deployment']='deployment'
        sbom={'bomFormat':'CycloneDX','specVersion':'1.6','version':1,'components':[{'bom-ref':'synthetic','name':'synthetic','version':'0.0.0'}],
            'metadata':{'properties':[{'name':k,'value':v} for k,v in {'commit':self.release['commit'],'source_patch_sha256':self.release['patch_sha256'],
            'cargo_lock_sha256':self.release['lock_sha256'],'artifact_sha256':self.release['artifact_sha256'],'container_digest':'sha256:'+'a'*64}.items()]}}
        self.add('sbom','data/sbom.json','report',sbom); self.primary['sbom']='sbom'
        policy_refs={}
        for name,role in c.POLICY_ROLES.items():
            details={'synthetic':'NOT ACTUAL ADOPTION'}
            if name=='privacy-retention': details['preserves_business_uniqueness']=True
            if name=='recovery': details.update(rpo_seconds=86400,rto_seconds=14400,audit_days=90)
            if name=='security-reporting': details.update(private_channel='synthetic private route',response_coverage='synthetic coverage')
            if name=='support': details.update(coverage='synthetic coverage',service_owner='synthetic-service')
            if name=='runtime-matrix': details['matrix']=[{'case':n,'version':'synthetic-1.0','runtime_dependencies':'synthetic inventory','evidence':'native-ubuntu'} for n in ('chromium-x11','chromium-wayland','firefox-x11','firefox-wayland','windows-native')]
            p={'schema_version':1,'policy':name,'status':'adopted','scope':'private-beta','owner':'synthetic-'+role,'reference':'SYNTHETIC TEST ONLY','details':details}
            key='policy-'+name; self.add(key,'data/policies/'+name+'.json','policy',p); policy_refs[name]=key
        self.candidate={'schema_version':1,'kind':'r7-private-beta-candidate','release':self.release,'files':self.files,'primary':self.primary,
            'migration_files':migration_map,'policies':policy_refs,'gate_certificates':{g:'gates/'+g.lower()+'.json' for g in c.GATES},
            'exceptions':[],'open_blockers':[],'notes':['SYNTHETIC TEST ONLY. NOT A REAL RELEASE.']}
        self.approval_paths=[]; self.seal()

    @staticmethod
    def store(path,value):
        path.parent.mkdir(parents=True,exist_ok=True)
        path.write_bytes(json.dumps(value,sort_keys=True).encode()+b'\n')

    def add(self,key,name,kind,data):
        path=self.base/name; path.parent.mkdir(parents=True,exist_ok=True)
        path.write_bytes(data if isinstance(data,bytes) else json.dumps(data).encode())
        self.files.append({'id':key,'path':name,'kind':kind,'sha256':c.file_sha(path)})

    def file(self,key): return self.base/next(v['path'] for v in self.files if v['id']==key)

    def mutate(self,key,callback):
        value=c.load(self.file(key)); callback(value); self.store(self.file(key),value)

    def signature(self,payload,role):
        return {'payload':payload,'signature':c.base64.b64encode(self.private[role].sign(c.DOMAIN+c.canonical(payload))).decode()}

    def seal(self,gate_edit=None,decision_edit=None):
        self.store(self.trust_path,self.trust); self.pin=c.file_sha(self.trust_path)
        for f in self.files: f['sha256']=c.file_sha(self.base/f['path'])
        certs={}
        for gate,role in c.GATE_ROLES.items():
            key=next(k for k in self.trust['keys'] if k['role']==role)
            p={'schema_version':1,'purpose':'prerequisite-closure','key_id':key['key_id'],'role':role,'subject':key['subject'],
                'issued_at':self.now-1,'expires_at':self.now+3600,'gate':gate,'status':'closed',
                'tuple_sha256':c.sha(c.canonical(self.release)),'files_sha256':c.sha(c.canonical(self.files)),
                'cases':[{'case':name,'status':'passed','evidence':['native-ubuntu','native-windows','beta','soak']} for name in sorted(c.GATES[gate])]}
            if gate_edit: gate_edit(gate,p)
            certs[gate]=self.signature(p,role); self.store(self.base/self.candidate['gate_certificates'][gate],certs[gate])
        self.store(self.base/'candidate.json',self.candidate)
        self.approval_paths=[]
        for role in sorted(c.ROLES):
            key=next(k for k in self.trust['keys'] if k['role']==role)
            p={'schema_version':1,'purpose':'private-beta-decision','key_id':key['key_id'],'role':role,'subject':key['subject'],
                'issued_at':self.now-1,'expires_at':self.now+1800,'decision':'approve',
                'candidate_sha256':c.sha(c.canonical(self.candidate)),'gate_certificates_sha256':c.sha(c.canonical(certs)),'trust_sha256':self.pin}
            if decision_edit: decision_edit(role,p)
            path=self.root/'decisions'/(role+'.json'); self.store(path,self.signature(p,role)); self.approval_paths.append(path)

    def authorize(self):
        return c.authorize(self.candidate,self.base,self.source,self.migrations,self.trust,self.approval_paths,self.pin,self.now)


class R7Tests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory(prefix='r7-synthetic-'); self.addCleanup(self.tmp.cleanup)
        self.f=Fixture(Path(self.tmp.name))

    def rejects(self,expected=None):
        with self.assertRaises(ValueError) as caught: self.f.authorize()
        if expected: self.assertIn(expected,str(caught.exception))

    def test_complete_signed_synthetic_candidate(self):
        decision,*_=self.f.authorize(); self.assertEqual(decision['decision'],'GO'); self.assertFalse(decision['production_rollout_authorized']); self.assertFalse(decision['public_publication_authorized'])

    def test_no_unsigned_or_tampered_approval(self):
        path=self.f.approval_paths[0]; value=c.load(path); value['signature']=''; self.f.store(path,value); self.rejects('signature')

    def test_denial_cannot_be_overridden(self):
        self.f.seal(decision_edit=lambda role,p:p.update(decision='deny') if role=='security' else None); self.rejects('decision-denied')

    def test_expired_review(self):
        self.f.seal(decision_edit=lambda role,p:p.update(expires_at=self.f.now) if role=='project' else None); self.rejects('time-window')

    def test_revoked_key(self):
        next(v for v in self.f.trust['keys'] if v['role']=='security')['revoked']=True; self.f.seal(); self.rejects('revoked')

    def test_same_public_key_is_not_independent_review(self):
        self.f.private['legal']=self.f.private['project']
        key=next(v for v in self.f.trust['keys'] if v['role']=='legal'); key['public_key_hex']=self.f.private['project'].public_key().public_bytes(serialization.Encoding.Raw,serialization.PublicFormat.Raw).hex()
        self.f.seal(); self.rejects('independence')

    def test_missing_role(self):
        self.f.approval_paths.pop(); self.rejects('five-independent')

    def test_gate_case_cannot_be_waived(self):
        self.f.seal(gate_edit=lambda gate,p:p['cases'].pop() if gate=='R6' else None); self.rejects('gate-case-inventory')

    def test_pending_gate_is_not_closure(self):
        self.f.seal(gate_edit=lambda gate,p:p.update(status='pending') if gate=='R5' else None); self.rejects('gate-not-closed')

    def test_open_blocker_wins(self):
        self.f.candidate['open_blockers']=['R6 unfinished']; self.f.seal(); self.rejects('open-prerequisite')

    def test_source_and_migration_drift(self):
        self.f.source['lock_sha256']='f'*64; self.rejects('source-tuple')
        self.f.source['lock_sha256']=self.f.release['lock_sha256']; self.f.migrations=[]; self.rejects('migration-inventory')

    def test_studio_content_revision_is_not_the_runtime_git_commit(self):
        original=self.f.file('artifact').read_bytes(); start=len(b'WORLDWRIGHT-RELEASE-V1\n')
        size=int(original[start:start+8],16); offset=start+9
        manifest=c.parse(original[offset:offset+size]); payload=original[offset+size:]
        release=copy.deepcopy(self.f.release)
        revision='studio-r12-'+release['payload_sha256']
        release['content_source_revision']=revision; manifest['provenance']['source_revision']=revision
        raw=json.dumps(manifest,separators=(',',':')).encode()
        data=b'WORLDWRIGHT-RELEASE-V1\n'+f'{len(raw):08x}\n'.encode()+raw+payload
        release['artifact_sha256']=c.sha(data)
        self.assertEqual(c.artifact(data,release)['provenance']['source_revision'],revision)
        self.assertNotEqual(revision,release['commit'])
        release['payload_sha256']='f'*64
        with self.assertRaises(ValueError): c.artifact(data,release)

    def test_artifact_tamper(self):
        with self.f.file('artifact').open('ab') as stream: stream.write(b'x')
        self.rejects('file-hash')

    def test_rehashed_wrong_artifact(self):
        with self.f.file('artifact').open('ab') as stream: stream.write(b'x')
        self.f.seal(); self.rejects('artifact-payload')

    def test_native_boolean_exit_is_not_zero(self):
        self.f.mutate('native-windows',lambda v:v['phases'][0].update(exit_code=False)); self.f.seal(); self.rejects('native-phase-failed')

    def test_native_missing_required_phase(self):
        self.f.mutate('native-ubuntu',lambda v:v['phases'].pop()); self.f.seal(); self.rejects('required-phase')

    def test_helper_report_is_not_native_acceptance(self):
        self.f.mutate('native-windows',lambda v:v.update(track='R6',status='collected_native_browser_not_closed')); self.f.seal(); self.rejects('native-report')

    def test_fixed_beta_is_not_tunable(self):
        for field,value in [('profile','smoke-v1'),('connected_clients',249),('elapsed_ms',90001),('passed',False)]:
            with self.subTest(field=field):
                old=c.load(self.f.file('beta')); self.f.mutate('beta',lambda v:v.update({field:value})); self.f.seal(); self.rejects()
                self.f.store(self.f.file('beta'),old); self.f.seal()

    def test_soak_duration_is_real_and_not_boolean(self):
        self.f.mutate('soak',lambda v:next(x for x in v['cases'] if x['case']=='soak').update(duration_seconds=True)); self.f.seal(); self.rejects('soak-duration')

    def test_deployment_and_sbom_match_release(self):
        self.f.mutate('deployment',lambda v:v.update(deployment_uid='different')); self.f.seal(); self.rejects('deployment-tuple')
        self.f.mutate('deployment',lambda v:v.update(deployment_uid=self.f.release['deployment_uid']))
        self.f.mutate('sbom',lambda v:v['metadata']['properties'][0].update(value='different')); self.f.seal(); self.rejects('sbom-release')

    def test_policy_owner_and_runtime_matrix(self):
        self.f.mutate('policy-support',lambda v:v.update(owner='unknown')); self.f.seal(); self.rejects('policy-owner')
        self.f.mutate('policy-support',lambda v:v.update(owner='synthetic-service'))
        self.f.mutate('policy-runtime-matrix',lambda v:v['details']['matrix'].pop()); self.f.seal(); self.rejects('runtime-matrix')

    def test_exceptions_do_not_waive_gates(self):
        e={'id':'limitation','category':'waive-r6','owner':'synthetic-service','reason':'synthetic','expires_at':self.f.now+600,'evidence':'soak'}
        self.f.candidate['exceptions']=[e]; self.f.seal(); self.rejects('waiving')
        e.update(category='nonblocking-operational-limitation',expires_at=self.f.now); self.f.seal(); self.rejects('expired')
        e['expires_at']=self.f.now+600; self.f.seal(); self.assertEqual(self.f.authorize()[0]['expires_at'],e['expires_at'])

    def test_paths_and_duplicate_json(self):
        for name in ('../outside','/absolute','data/../outside','data\\x','data/x:ads','data//x'):
            with self.subTest(name=name), self.assertRaises(ValueError): c.safe_path(self.f.base,name)
        with self.assertRaises(ValueError): c.parse(b'{"x":1,"x":2}')
        with self.assertRaises(ValueError): c.parse(b'{"x":NaN}')

    def test_case_colliding_files_fail(self):
        record=copy.deepcopy(self.f.files[0]); record['id']='case-duplicate'; record['path']='data/WORLD.wwr'; self.f.files.append(record)
        self.f.file('case-duplicate').write_bytes(self.f.file('artifact').read_bytes()); self.f.seal(); self.rejects('file-record')

    def test_schema_boolean_and_unknown_field_fail(self):
        self.f.candidate['schema_version']=True; self.rejects('candidate-schema')
        self.f.candidate['schema_version']=1; self.f.candidate['waive']=True; self.rejects('candidate-schema')

    def test_real_cli_uses_current_git_and_preserves_no_go(self):
        args=SimpleNamespace(candidate=self.f.base/'candidate.json',output=self.f.root/'run',trust_store=self.f.trust_path,
                             trust_store_sha256=self.f.pin,approval=self.f.approval_paths,operation='check')
        with patch.object(c,'ROOT',self.f.repo): self.assertEqual(cli.evaluate(args),0)
        (self.f.repo/'Cargo.lock').write_text('version = 3\n'); args.output=self.f.root/'changed'
        with patch.object(c,'ROOT',self.f.repo): self.assertEqual(cli.evaluate(args),1)
        self.assertEqual(c.load(args.output/'decision.json')['decision'],'NO-GO')

    def test_packet_roundtrip_and_tamper(self):
        decision,ids,certificates,reviews=self.f.authorize(); decision.update(schema_version=1,track='R7')
        output=self.f.root/'release'; output.mkdir()
        path=cli.packet(output,self.f.candidate,decision,ids,certificates,reviews)
        self.assertEqual(cli.verify_packet(path,self.f.trust_path,self.f.pin),0)
        (path/'release-notes.md').write_text('tampered')
        with self.assertRaises(ValueError): cli.verify_packet(path,self.f.trust_path,self.f.pin)

    def test_bad_trust_pin_and_unlisted_packet_file(self):
        with self.assertRaises(ValueError): cli.trusted(self.f.trust_path,'f'*64,self.f.base)
        decision,ids,certificates,reviews=self.f.authorize(); output=self.f.root/'release'; output.mkdir()
        path=cli.packet(output,self.f.candidate,decision,ids,certificates,reviews)
        (path/'unlisted').write_bytes(b'unlisted')
        with self.assertRaises(ValueError): cli.verify_packet(path,self.f.trust_path,self.f.pin)

    def test_fingerprint_cli_records_current_identity_without_approval(self):
        output=self.f.root/'fingerprint'
        with patch.object(c,'ROOT',self.f.repo), patch.object(cli.sys,'argv',['r7.py','fingerprint','--output',str(output)]):
            self.assertEqual(cli.main(),0)
        self.assertEqual(c.load(output/'source.json'),self.f.source)
        self.assertFalse((output/'decision.json').exists())

    def test_unsigned_review_handoff_and_domain_bytes(self):
        output=self.f.root/'requests'; cli.review_templates(self.f.base/'candidate.json',output)
        value=c.load(output/'project.unsigned.json'); self.assertEqual(value['decision'],'pending'); self.assertIsNone(value['subject'])
        signing=self.f.root/'signing'; cli.signing_bytes(output/'project.unsigned.json',signing)
        self.assertEqual((signing/'signing-input.bin').read_bytes(),c.DOMAIN+c.canonical(value))
        self.assertFalse((signing/'signature').exists())

    def test_nested_unlisted_packet_manifest_is_rejected(self):
        decision,ids,certificates,reviews=self.f.authorize(); decision.update(schema_version=1,track='R7')
        output=self.f.root/'release'; output.mkdir(); path=cli.packet(output,self.f.candidate,decision,ids,certificates,reviews)
        (path/'data/packet-manifest.json').write_text('{}')
        with self.assertRaises(ValueError): cli.verify_packet(path,self.f.trust_path,self.f.pin)

    def test_candidate_cannot_supply_its_own_trust_anchor(self):
        path=self.f.base/'trust.json'; self.f.store(path,self.f.trust)
        with self.assertRaises(ValueError): cli.trusted(path,c.file_sha(path),self.f.base)

    def test_release_cli_creates_verifiable_private_packet(self):
        args=SimpleNamespace(candidate=self.f.base/'candidate.json',output=self.f.root/'release-cli',trust_store=self.f.trust_path,
                             trust_store_sha256=self.f.pin,approval=self.f.approval_paths,operation='release')
        with patch.object(c,'ROOT',self.f.repo): self.assertEqual(cli.evaluate(args),0)
        self.assertEqual(cli.verify_packet(args.output/'packet',self.f.trust_path,self.f.pin),0)

    def test_packet_approval_expiry_is_rechecked(self):
        decision,ids,certificates,reviews=self.f.authorize(); decision.update(schema_version=1,track='R7')
        output=self.f.root/'release'; output.mkdir(); path=cli.packet(output,self.f.candidate,decision,ids,certificates,reviews)
        with patch.object(cli.time,'time',return_value=self.f.now+4000), self.assertRaises(ValueError):
            cli.verify_packet(path,self.f.trust_path,self.f.pin)

    def test_draft_init_has_no_approvals_and_never_overwrites(self):
        output=self.f.root/'draft'
        with patch.object(c,'ROOT',self.f.repo): cli.template(output)
        self.assertTrue(c.load(output/'candidate.json')['open_blockers'])
        self.assertTrue(all(v['subject'] is None for v in c.load(output/'trust-store.template.json')['keys']))
        with patch.object(c,'ROOT',self.f.repo), self.assertRaises(ValueError): cli.template(output)

if __name__=='__main__': unittest.main(verbosity=2)
