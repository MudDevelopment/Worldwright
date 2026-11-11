"""R3 bounded helper/contract tests. Synthetic data is never deployment acceptance."""
from pathlib import Path
import ast
import base64
import copy
import hashlib
import importlib.util
import json
import sys
import tempfile
import time
import unittest
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
import yaml

ROOT=Path(__file__).resolve().parents[1]
def load(name,path):
    spec=importlib.util.spec_from_file_location(name,path); value=importlib.util.module_from_spec(spec); spec.loader.exec_module(value); return value
sys.path.insert(0,str(ROOT/'deploy/helm/worldwright/tests'))
chart=load('r3_verify',ROOT/'deploy/helm/worldwright/tests/verify.py')
image=load('r3_image',ROOT/'scripts/r3-image.py')
close=load('r3_close',ROOT/'scripts/r3-close.py')
staging=load('r3_stage',ROOT/'scripts/r3-staging.py')
values_migration=load('r3_values',ROOT/'deploy/helm/worldwright/tests/migrate-values.py')

class R3Tests(unittest.TestCase):
    def test_actual_manifest_bytes_not_config_identity(self):
        raw=json.dumps({'schemaVersion':2,'mediaType':'application/vnd.oci.image.manifest.v1+json'}).encode()
        digest='sha256:'+hashlib.sha256(raw).hexdigest()
        self.assertEqual(image.registry_manifest(raw,digest),[digest])
        for invalid in ('sha256:'+'a'*64,None,'sha256:'+'A'*64):
            with self.assertRaises(ValueError): image.registry_manifest(raw,invalid)
    def test_no_mutable_or_credential_image_reference(self):
        for value in ('rust:1.85.1-bookworm','registry/repo:latest','user:secret@registry/repo@sha256:'+'a'*64): self.assertIsNone(image.REF.fullmatch(value))
        self.assertIsNotNone(image.REF.fullmatch('registry/repo@sha256:'+'a'*64))
    def test_values_migration_requires_explicit_safe_identity(self):
        old=yaml.safe_load((ROOT/'deploy/helm/worldwright/tests/acceptance-values.yaml').read_text())
        old.pop('configVersion'); old['postgres'].pop('migrationSecret'); old.pop('networkPolicy')
        args=[old,'migration-new','sha256:'+'a'*64,'10.23.4.5/32','ingress-nginx','role=controller','kube-system','k8s-app=kube-dns',[]]
        result=values_migration.migrate(*args); self.assertEqual(result['configVersion'],2); self.assertTrue(result['networkPolicy']['enabled']); self.assertNotIn('configVersion',old)
        args[1]='world-postgres'
        with self.assertRaises(AssertionError): values_migration.migrate(*args)
    def test_schema_and_restricted_semantic_matrix(self):
        self.assertGreaterEqual(chart.schema_matrix(ROOT/'deploy/helm/worldwright'),35)
    def test_manual_case_skip_duplicate_and_absence_refused(self):
        good=[{'case':name,'status':'passed'} for name in close.MANUAL]
        close.records(good,close.MANUAL)
        for values in ([],good[:-1],good+[good[0]],[dict(item,status='skipped') for item in good]):
            with self.assertRaises(ValueError): close.records(values,close.MANUAL)
    def test_missing_phase_and_changed_phase_log_refused(self):
        with tempfile.TemporaryDirectory() as temporary:
            parent=Path(temporary)
            with self.assertRaises(ValueError): close.check_phases({'phases':[]},parent,{'install'})
            (parent/'install.log').write_text('synthetic fixture log')
            phase={'name':'install','exit_code':0,'log_sha256':close.sha(parent/'install.log')}
            close.check_phases({'phases':[phase]},parent,{'install'})
            (parent/'install.log').write_text('changed fixture log')
            with self.assertRaises(ValueError): close.check_phases({'phases':[phase]},parent,{'install'})
    def test_staging_emitted_phase_inventory_matches_closure(self):
        tree=ast.parse((ROOT/'scripts/r3-staging.py').read_text())
        emitted={call.args[0].value for call in ast.walk(tree) if isinstance(call,ast.Call) and isinstance(call.func,ast.Name) and call.func.id in {'run','upgrade','observe_pod'} and call.args and isinstance(call.args[0],ast.Constant) and isinstance(call.args[0].value,str)}
        self.assertEqual(emitted,close.PHASES)
        self.assertEqual(len(emitted),13)
        self.assertTrue({'rotated-pod-image','rollback-pod-image'}<=emitted)
    def test_full_current_phase_inventory_and_missing_replacement_evidence(self):
        with tempfile.TemporaryDirectory() as temporary:
            parent=Path(temporary); phases=[]
            for name in sorted(close.PHASES):
                path=parent/(name+'.log'); path.write_text('SYNTHETIC checker fixture, not staging evidence: '+name+'\n')
                phases.append({'name':name,'exit_code':1 if name=='failed-rollout' else 0,'expected_exit_code':1 if name=='failed-rollout' else 0,'log_sha256':close.sha(path)})
            close.check_phases({'phases':phases},parent,close.PHASES)
            for name in ('rotated-pod-image','rollback-pod-image'):
                missing=[phase for phase in phases if phase['name']!=name]
                with self.assertRaises(ValueError): close.check_phases({'phases':missing},parent,close.PHASES)
                (parent/(name+'.log')).write_text('tampered replacement Pod evidence')
                with self.assertRaises(ValueError): close.check_phases({'phases':phases},parent,close.PHASES)
                (parent/(name+'.log')).write_text('SYNTHETIC checker fixture, not staging evidence: '+name+'\n')
    def test_signal_kill_boolean_and_wrong_expected_exit_not_rollout_failure(self):
        with tempfile.TemporaryDirectory() as temporary:
            parent=Path(temporary); path=parent/'failed-rollout.log'; path.write_text('SYNTHETIC negative checker fixture')
            phase={'name':'failed-rollout','exit_code':1,'expected_exit_code':1,'log_sha256':close.sha(path)}
            close.check_phases({'phases':[phase]},parent,{'failed-rollout'})
            for code in (0,-9,True,'1'):
                invalid=dict(phase,exit_code=code)
                with self.assertRaises(ValueError): close.check_phases({'phases':[invalid]},parent,{'failed-rollout'})
            with self.assertRaises(ValueError): close.check_phases({'phases':[dict(phase,expected_exit_code=0)]},parent,{'failed-rollout'})
    def test_invalid_or_duplicated_phase_records_refused(self):
        with tempfile.TemporaryDirectory() as temporary:
            parent=Path(temporary); path=parent/'install.log'; path.write_text('SYNTHETIC fixture')
            phase={'name':'install','exit_code':0,'log_sha256':close.sha(path)}
            for phases,expected in (([None],{'install'}),([dict(phase,name=[])],{'install'}),([phase,phase],{'install','deployment'})):
                with self.assertRaises(ValueError): close.check_phases({'phases':phases},parent,expected)
    def test_independent_signature_expiry_and_payload(self):
        with tempfile.TemporaryDirectory() as temporary:
            key=Ed25519PrivateKey.generate(); path=Path(temporary)/'unit-only-key.pem'
            path.write_bytes(key.public_key().public_bytes(serialization.Encoding.PEM,serialization.PublicFormat.SubjectPublicKeyInfo))
            value={'kind':'r3-service-adoption','status':'adopted','reviewer':'SYNTHETIC UNIT KEY NOT SERVICE APPROVAL','expires_at':int(time.time())+120}
            value['signature']=base64.b64encode(key.sign(json.dumps(value,sort_keys=True,separators=(',',':')).encode())).decode(); close.signature(value,path)
            value['status']='observed_not_adopted'
            with self.assertRaises(ValueError): close.signature(value,path)
            value['status']='adopted'; value['expires_at']=int(time.time())-1
            with self.assertRaises(ValueError): close.signature(value,path)
    def test_required_signal_and_role_source_inventory(self):
        signal=(ROOT/'apps/worldwright-server/src/lifecycle.rs').read_text(); cli=(ROOT/'apps/worldwright-server/src/cli.rs').read_text()
        self.assertIn('r3_shutdown_signals_register_before_waiting',signal); self.assertIn('SignalKind::terminate()',signal); self.assertIn('SignalKind::hangup()',signal); self.assertIn('lifecycle::shutdown_signal()',cli)
        roles=(ROOT/'scripts/r3-roles.py').read_text(); self.assertIn('pg_has_role',roles); self.assertIn('rolcanlogin',roles); self.assertIn('exact current successful migration ledger required',roles)
        self.assertEqual(close.AUTO,staging.AUTO_CASES); self.assertEqual(close.MANUAL,staging.MANUAL_CASES)
if __name__=='__main__': unittest.main(verbosity=2)
