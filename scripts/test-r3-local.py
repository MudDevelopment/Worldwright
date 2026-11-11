"""Additional local R3 input/proof regressions. No deployment/adoption evidence."""
from pathlib import Path
import ast
import base64
import hashlib
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import time
import unittest
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

ROOT=Path(__file__).resolve().parents[1]
def load(name):
    spec=importlib.util.spec_from_file_location(name.replace('-','_'),ROOT/'scripts'/(name+'.py'))
    module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module); return module
stage=load('r3-staging'); close=load('r3-close'); image=load('r3-image')

def plan():
    return {'configVersion':1,'context':'synthetic-local-contract','namespace':'worldwright-r3-fixture','release':'wwfixture','values_file':'NOT_DEPLOYMENT_VALUES','image_report':'NOT_IMAGE_EVIDENCE','r2_arguments':{key:'NOT_PREREQUISITE_EVIDENCE' for key in stage.R2_OPTIONS},'rotation_secret':'fixture-rotation','failed_digest':'sha256:'+'a'*64,'database_url_env':'NOT_DB_CREDENTIALS','credentials_env':'NOT_PLAYER_CREDENTIALS','world_id':'00000000-0000-0000-0000-000000000001','artifact_digest':'b'*64,'runtime_role':'fixture_rt','migration_role':'fixture_mig','recovery_role':'fixture_rec','schema':'fixture_schema'}

class LocalGuards(unittest.TestCase):
    def test_plan_exact_integer_version(self):
        stage.plan_check(plan())
        for version in (True,False,'1',1.0,0,2,None):
            with self.subTest(version=version), self.assertRaises(ValueError):
                candidate=plan(); candidate['configVersion']=version; stage.plan_check(candidate)
    def test_plan_missing_unknown_and_unsafe_scope(self):
        for change in ({'namespace':'production'},{'release':'-option'},{'context':'-option'},{'world_id':'00000000-0000-0000-0000-000000000000'}):
            with self.subTest(change=change),self.assertRaises(ValueError): stage.plan_check(dict(plan(),**change))
        candidate=plan(); candidate['unexpected']=True
        with self.assertRaises(ValueError): stage.plan_check(candidate)
        candidate=plan(); candidate.pop('runtime_role')
        with self.assertRaises(ValueError): stage.plan_check(candidate)
        candidate=plan(); candidate['r2_arguments']={}
        with self.assertRaises(ValueError): stage.plan_check(candidate)
    def test_collector_and_closure_exit_semantics_agree(self):
        with tempfile.TemporaryDirectory() as directory:
            parent=Path(directory)
            for name,expected in (('install',0),('failed-rollout',1)):
                path=parent/(name+'.log'); path.write_text('SYNTHETIC process-outcome fixture, not actual staging\n')
                for code in (0,1,2,-9,True,False,'1',None):
                    accepted=type(code) is int and (code==0 if expected==0 else code>0)
                    self.assertEqual(stage.phase_outcome(code,expected),accepted)
                    phase={'name':name,'exit_code':code,'expected_exit_code':expected,'log_sha256':close.sha(path)}
                    if accepted: close.check_phases({'phases':[phase]},parent,{name})
                    else:
                        with self.assertRaises(ValueError): close.check_phases({'phases':[phase]},parent,{name})
            for expected in (True,'1',-1,2,None): self.assertFalse(stage.phase_outcome(1,expected))
        tree=ast.parse((ROOT/'scripts/r3-staging.py').read_text())
        self.assertTrue(any(isinstance(call,ast.Call) and isinstance(call.func,ast.Name) and call.func.id=='phase_outcome' for call in ast.walk(tree)))
    def test_reviewers_require_nonblank_bounded_text(self):
        with tempfile.TemporaryDirectory() as directory:
            key=Ed25519PrivateKey.generate(); path=Path(directory)/'SYNTHETIC-NOT-APPROVAL.pem'
            path.write_bytes(key.public_key().public_bytes(serialization.Encoding.PEM,serialization.PublicFormat.SubjectPublicKeyInfo))
            for reviewer in ('','   ',123,True,[],{},'x'*257,'name\nforged-line','name\u0085forged-line'):
                value={'kind':'r3-service-adoption','status':'adopted','reviewer':reviewer,'expires_at':int(time.time())+120}
                value['signature']=base64.b64encode(key.sign(json.dumps(value,sort_keys=True,separators=(',',':')).encode())).decode()
                with self.subTest(reviewer=reviewer),self.assertRaises(ValueError): close.signature(value,path)
    def test_report_bounds_directory_and_bad_json(self):
        with tempfile.TemporaryDirectory() as directory:
            parent=Path(directory); valid=parent/'valid.json'; valid.write_text('{"synthetic":true}')
            bad=parent/'bad.json'; bad.write_text('{'); big=parent/'big.json'; big.write_bytes(b' '*((4*1024*1024)+1))
            for reader in (close.read,stage.read):
                self.assertEqual(reader(valid),{'synthetic':True})
                for target in (parent,parent/'missing.json',bad,big):
                    with self.subTest(target=target),self.assertRaises(ValueError): reader(target)
    @unittest.skipUnless(os.name=='posix','POSIX symlink fixture; Windows ACL/link gates remain independent')
    def test_report_leaf_symlink_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            parent=Path(directory); valid=parent/'valid.json'; valid.write_text('{}'); link=parent/'link.json'; link.symlink_to(valid)
            for reader in (close.read,stage.read):
                with self.assertRaises(ValueError): reader(link)
    def test_oci_index_scope_and_exact_bytes(self):
        obj={'schemaVersion':2,'mediaType':'application/vnd.oci.image.index.v1+json','manifests':[{'digest':'sha256:'+'a'*64,'platform':{'os':'linux','architecture':'amd64'}},{'digest':'sha256:'+'b'*64,'platform':{'os':'linux','architecture':'arm64'}},{'digest':'sha256:'+'c'*64,'platform':{'os':'unknown','architecture':'unknown'}}]}
        raw=json.dumps(obj).encode(); digest='sha256:'+hashlib.sha256(raw).hexdigest()
        self.assertEqual(image.registry_manifest(raw,digest),sorted([digest,'sha256:'+'a'*64]))
        self.assertEqual(image.registry_manifest(raw+b'\n',digest),sorted([digest,'sha256:'+'a'*64]))
        with self.assertRaises(ValueError): image.registry_manifest(raw+b'\n\n',digest)
    def test_fixture_result_cannot_replace_actual_role_proof(self):
        collection={'runtime_role':'fixture_rt','migration_role':'fixture_mig','recovery_role':'fixture_rec','schema':'fixture_schema'}
        # Positive metadata shape only, NOT an actual audited DB or service approval.
        value=dict(collection,schema_version=1,track='R3',kind='database-role-audit',status='passed',full_r3_closed=False,identity_checks='passed',recovery_authority_adopted=False,runtime_setup_applied=False,migration_count=len(list((ROOT/'migrations').glob('*.sql'))),table_count=24)
        close.role_report(value,collection)
        for invalid in ({'kind':'local-R3-SYNTHETIC-role-fixture-tests','status':'passed'},dict(value,schema_version=True),dict(value,migration_count=True),dict(value,migration_count=1),dict(value,table_count=0),dict(value,recovery_authority_adopted=True),dict(value,runtime_role='wrong_runtime'),dict(value,schema='wrong_schema')):
            with self.assertRaises(ValueError): close.role_report(invalid,collection)
    def test_cli_help_has_no_side_effects(self):
        for name in ('r3-image','r3-staging','r3-roles','r3-close'):
            result=subprocess.run([sys.executable,str(ROOT/'scripts'/(name+'.py')),'--help'],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=20)
            self.assertEqual(result.returncode,0,result.stdout.decode(errors='replace'))
            self.assertIn(b'--help',result.stdout)

if __name__=='__main__': unittest.main(verbosity=2)
