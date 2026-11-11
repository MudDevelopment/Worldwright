"""R2 helper/fixture tests. No native DB/process/provider acceptance is implied."""
from pathlib import Path
import hashlib
import importlib.util
import json
import tempfile
import unittest
import jwt

ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('r2_verify',ROOT/'scripts/r2-verify.py')
r2=importlib.util.module_from_spec(spec); spec.loader.exec_module(r2)

class FixtureTests(unittest.TestCase):
 def test_signed_rsa_vectors_and_rotation(self):
  value=json.loads((ROOT/'apps/worldwright-server/src/fixtures/r2-oidc-verifier.json').read_text())
  initial=jwt.PyJWK.from_dict(value['jwks_initial']['keys'][0]).key
  rotated=jwt.PyJWK.from_dict(value['jwks_rotated']['keys'][0]).key
  for name in ('valid','rotated'):
   decoded=jwt.decode(value['tokens'][name], rotated if name=='rotated' else initial, algorithms=['RS256'],issuer=value['issuer'],audience=value['audience'])
   self.assertEqual(decoded['nonce'],value['nonce'])
  for name in ('wrong_issuer','wrong_audience','invalid_signature','expired'):
   with self.subTest(name=name),self.assertRaises(jwt.InvalidTokenError):
    jwt.decode(value['tokens'][name],initial,algorithms=['RS256'],issuer=value['issuer'],audience=value['audience'])
  decoded=jwt.decode(value['tokens']['wrong_nonce'],initial,algorithms=['RS256'],issuer=value['issuer'],audience=value['audience'])
  self.assertNotEqual(decoded['nonce'],value['nonce'])
  with self.assertRaises(jwt.InvalidTokenError): jwt.decode(value['tokens']['rotated'],initial,algorithms=['RS256'],issuer=value['issuer'],audience=value['audience'])
  self.assertNotIn('PRIVATE KEY',json.dumps(value))
 def test_source_case_inventory_not_native_execution(self):
  storage=(ROOT/'crates/worldwright-persistence/src/database_tests/r2.rs').read_text()
  owner=(ROOT/'apps/worldwright-server/src/r2_oidc_tests.rs').read_text()+(ROOT/'apps/worldwright-server/src/world.rs').read_text()
  for name in r2.DB_REQUIRED:
   if name.startswith('r2_'): self.assertIn('fn '+name,storage)
  for name in r2.UNIT_REQUIRED: self.assertIn('fn '+name,owner)

 def test_expiry_python_fixtures_preserve_immutable_lifetime(self):
  import ast
  source=(ROOT/'scripts/r2-live.py').read_text()
  statements=[node.value for node in ast.walk(ast.parse(source)) if isinstance(node,ast.Constant) and isinstance(node.value,str) and node.value.startswith(('UPDATE worldwright_sessions SET ','INSERT INTO worldwright_player_invites(')) and "INTERVAL '1 second'" in node.value]
  self.assertEqual(len(statements),2)
  for statement in statements:
   self.assertIn('created_at',statement)
   self.assertIn("statement_timestamp()-INTERVAL '2 seconds'",statement)
   self.assertIn("statement_timestamp()-INTERVAL '1 second'",statement)
  session=next(statement for statement in statements if statement.startswith('UPDATE'))
  self.assertIn("username='alice'",session); self.assertIn('revoked_at IS NULL',session)
 def test_expiry_rust_fixtures_and_checks_are_retained(self):
  source=(ROOT/'crates/worldwright-persistence/src/lib.rs').read_text()
  self.assertIn("created_at = statement_timestamp() - INTERVAL '2 seconds', expires_at = statement_timestamp() - INTERVAL '1 second'",source)
  self.assertIn('assert!(expired_fixture)',source)
  for name in ('20261020000100_durable_gameplay.sql','20261118000300_oidc_identity.sql'):
   self.assertIn('expires_at > created_at',(ROOT/'migrations'/name).read_text())
  self.assertIn('r2_expiry_fixtures_preserve_lifetime_and_reject_expired_identity',r2.DB_REQUIRED)
  self.assertIn('durable_sessions_restart_lookup_revocation_and_expiry',r2.DB_REQUIRED)

class EvidenceTests(unittest.TestCase):
 def setUp(self):
  self.temp=tempfile.TemporaryDirectory(prefix='r2-shape-only-'); self.root=Path(self.temp.name)
  self.path=self.root/'process.json'; self.log=self.path.with_suffix('.log'); self.log.write_text('SYNTHETIC validator unit data, not real process evidence\n')
  self.source={'source_sha256':'a'*64,'lock_sha256':'b'*64}
  self.value={'schema_version':1,'kind':'real-process-faults','status':'passed','reviewer':'synthetic-unit-only',**self.source,'artifact_sha256':'c'*64,'log_sha256':hashlib.sha256(self.log.read_bytes()).hexdigest(),
   'cases':[{'case':name,'status':'passed'} for name in sorted(r2.PROCESS_REQUIRED)],
   'schema_cleanup':'verified','fixture_sha256':'e'*64,'child_logs':[{'name':'ww_r2_'+('f'*32)+'-1.log','sha256':self.value_log_hash()}]}
  (self.root/('ww_r2_'+('f'*32)+'-1.log')).write_text('unit fixture child log\n')
 def value_log_hash(self): return hashlib.sha256(b'unit fixture child log\n').hexdigest()
 def tearDown(self): self.temp.cleanup()
 def save(self): self.path.write_text(json.dumps(self.value))
 def test_synthetic_complete_shape_only(self):
  self.save(); self.assertEqual(r2.external(self.path,r2.PROCESS_REQUIRED,self.source,'real-process-faults')['reviewer'],'synthetic-unit-only')
 def test_missing_cleanup_or_child_log_fails(self):
  for field in ('schema_cleanup','child_logs'):
   saved=self.value.pop(field); self.save()
   with self.assertRaises(ValueError): r2.external(self.path,r2.PROCESS_REQUIRED,self.source,'real-process-faults')
   self.value[field]=saved
 def test_absent_collection_phases_refuse_before_external_inputs(self):
  from types import SimpleNamespace
  self.path.write_text(json.dumps({'schema_version':1,'track':'R2','status':'collected_not_closed','orphaned_test_schemas':[],'database_tests_executed':len(r2.DB_REQUIRED),'phases':[]}))
  with self.assertRaises(ValueError): r2.close(SimpleNamespace(collection=self.path))
 def test_missing_case_fails(self):
  self.value['cases'].pop(); self.save()
  with self.assertRaises(ValueError): r2.external(self.path,r2.PROCESS_REQUIRED,self.source,'real-process-faults')
 def test_wrong_source_fails(self):
  self.value['source_sha256']='d'*64; self.save()
  with self.assertRaises(ValueError): r2.external(self.path,r2.PROCESS_REQUIRED,self.source,'real-process-faults')
 def test_changed_or_missing_log_fails(self):
  self.save(); self.log.write_text('changed')
  with self.assertRaises(ValueError): r2.external(self.path,r2.PROCESS_REQUIRED,self.source,'real-process-faults')
  self.log.unlink()
  with self.assertRaises(ValueError): r2.external(self.path,r2.PROCESS_REQUIRED,self.source,'real-process-faults')
 def test_null_artifact_fails(self):
  self.value['artifact_sha256']=None; self.save()
  with self.assertRaises(ValueError): r2.external(self.path,r2.PROCESS_REQUIRED,self.source,'real-process-faults')
 def test_sample_provider_domain_is_not_approval(self):
  self.value.update(kind='approved-provider',issuer='https://issuer.example.invalid',cases=[{'case':name,'status':'passed'} for name in sorted(r2.PROVIDER_REQUIRED)]); self.save()
  with self.assertRaises(ValueError): r2.external(self.path,r2.PROVIDER_REQUIRED,self.source,'approved-provider')

if __name__=='__main__': unittest.main(verbosity=2)
