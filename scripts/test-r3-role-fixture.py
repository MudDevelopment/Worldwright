"""Real PostgreSQL grant-logic regressions on a SYNTHETIC isolated fixture.
NOT current app migrations, native SQLx, deployed Secret/role audit or R3 adoption.
Requires existing loopback worldwright_test DB; creates/removes only test-owned
DB/roles. ROOT is patched to the executed one-file fixture migration, never to
pretend the immutable eight-file application migration gate has passed.
"""
from pathlib import Path
import hashlib
import importlib.util
import json
import os
import tempfile
import unittest
import uuid
from unittest.mock import patch
from urllib.parse import urlsplit,urlunsplit
import psycopg
from psycopg import sql

ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('r3_fixture_roles',ROOT/'scripts/r3-roles.py')
roles=importlib.util.module_from_spec(spec); spec.loader.exec_module(roles)
READONLY={'_sqlx_migrations','worldwright_content_artifacts','worldwright_publication_requests','worldwright_operator_grants','worldwright_operators','worldwright_recovery_jobs','worldwright_player_invites'}

class RoleFixture(unittest.TestCase):
    def setUp(self):
        if os.environ.get('WORLDWRIGHT_R3_ROLE_FIXTURES')!='1':
            raise RuntimeError('explicit WORLDWRIGHT_R3_ROLE_FIXTURES=1 and owned disposable test cluster required')
        url=os.environ.get('WORLDWRIGHT_TEST_DATABASE_URL',''); parsed=urlsplit(url)
        if parsed.scheme not in ('postgres','postgresql') or parsed.hostname not in ('127.0.0.1','localhost','::1') or 'worldwright_test' not in parsed.path:
            raise RuntimeError('explicit existing loopback worldwright_test fixture URL required; no successful skip')
        self.admin=psycopg.connect(url,autocommit=True,connect_timeout=5)
        self.addCleanup(self.admin.close)
        if 'worldwright_test' not in self.admin.execute('SELECT current_database()').fetchone()[0]: raise RuntimeError('actual test DB guard failed')
        suffix=uuid.uuid4().hex[:16]; self.database='worldwright_test_r3_fixture_'+suffix; self.schema='ww_r3_fixture_'+suffix
        self.runtime='r3_fixture_rt_'+suffix; self.migration='r3_fixture_mig_'+suffix; self.recovery='r3_fixture_rec_'+suffix; self.group='r3_fixture_group_'+suffix
        self.created_roles=[]; self.created_database=False; self.db=None
        self.addCleanup(self.cleanup_owned)
        for name in (self.runtime,self.migration,self.recovery,self.group):
            self.admin.execute(sql.SQL('CREATE ROLE {} LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS').format(sql.Identifier(name)))
            self.created_roles.append(name)
        self.admin.execute(sql.SQL('CREATE DATABASE {} OWNER {}').format(sql.Identifier(self.database),sql.Identifier(self.migration))); self.created_database=True
        fixture_url=urlunsplit(parsed._replace(path='/'+self.database)); self.db=psycopg.connect(fixture_url,autocommit=True,connect_timeout=5)
        self.db.execute(sql.SQL('CREATE SCHEMA {} AUTHORIZATION {}').format(sql.Identifier(self.schema),sql.Identifier(self.migration)))
        self.db.execute(sql.SQL('SET ROLE {}').format(sql.Identifier(self.migration)))
        self.db.execute(sql.SQL('SET search_path TO {}').format(sql.Identifier(self.schema)))
        self.temporary=tempfile.TemporaryDirectory(prefix='r3-SYNTHETIC-grant-fixture-'); self.addCleanup(self.temporary.cleanup)
        self.fixture_root=Path(self.temporary.name); (self.fixture_root/'migrations').mkdir()
        names=roles.WRITE_TABLES|roles.INSERT_TABLES|set(roles.COLUMNS)|READONLY
        statements=[]
        for name in sorted(names):
            if name=='_sqlx_migrations':
                statements.append('CREATE TABLE _sqlx_migrations(version BIGINT PRIMARY KEY,checksum BYTEA NOT NULL,success BOOLEAN NOT NULL);')
            else:
                # Permission/SQL quoting model ONLY: these are not application columns/types.
                columns=['id BIGSERIAL PRIMARY KEY','payload TEXT']+[column+' TEXT' for column in roles.COLUMNS.get(name,[]) if column not in ('id','payload')]
                if name=='worldwright_accounts': columns+=['password_hash TEXT','disabled BOOLEAN']
                statements.append('CREATE TABLE '+name+'('+','.join(columns)+');')
        ddl='-- SYNTHETIC grant logic fixture, NOT app migrations or accepted deployment\n'+'\n'.join(statements)+'\n'
        migration=self.fixture_root/'migrations/1_fixture.sql'; migration.write_text(ddl)
        self.db.execute(ddl)
        self.db.execute('INSERT INTO _sqlx_migrations(version,checksum,success) VALUES(1,%s,true)',(hashlib.sha384(migration.read_bytes()).digest(),))
        self.db.execute('RESET ROLE')
        print('R3_SYNTHETIC_FIXTURE_CREATED='+self.database,flush=True)
    def cleanup_owned(self):
        if self.db is not None: self.db.close()
        if self.created_database:
            self.admin.execute(sql.SQL('DROP DATABASE {} WITH (FORCE)').format(sql.Identifier(self.database)))
        for name in self.created_roles:
            self.admin.execute(sql.SQL('DROP ROLE {}').format(sql.Identifier(name)))
        if self.created_database:
            assert not self.admin.execute('SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname=%s)',(self.database,)).fetchone()[0]
            assert not self.admin.execute('SELECT EXISTS(SELECT 1 FROM pg_roles WHERE rolname=ANY(%s))',(self.created_roles,)).fetchone()[0]
            print('R3_SYNTHETIC_FIXTURE_REMOVED='+self.database,flush=True)
    def inspect(self,apply=False):
        with patch.object(roles,'ROOT',self.fixture_root):
            return roles.inspect(self.db,self.schema,self.runtime,self.migration,self.recovery,apply)
    def execute(self,template,*identifiers):
        return self.db.execute(sql.SQL(template).format(*(sql.Identifier(value) for value in identifiers)))
    def setup_grants(self):
        result=self.inspect(True); self.assertEqual(result['migration_count'],1); self.assertFalse(result['recovery_authority_adopted']); return result
    def test_fixture_setup_and_readonly_audit(self):
        with self.assertRaises(ValueError): self.inspect()
        result=self.setup_grants(); self.assertEqual(result['identity_checks'],'passed')
        self.assertEqual(self.inspect()['identity_checks'],'passed')
    def test_fixture_missing_schema_usage_refused(self):
        self.setup_grants(); self.execute('REVOKE USAGE ON SCHEMA {} FROM {}',self.schema,self.runtime)
        with self.assertRaises(ValueError): self.inspect()
    def test_fixture_missing_artifact_select_refused(self):
        self.setup_grants(); self.execute('REVOKE SELECT ON TABLE {}.worldwright_content_artifacts FROM {}',self.schema,self.runtime)
        with self.assertRaises(ValueError): self.inspect()
    def test_fixture_missing_audit_insert_refused(self):
        self.setup_grants(); self.execute('REVOKE INSERT ON TABLE {}.worldwright_gameplay_audit FROM {}',self.schema,self.runtime)
        with self.assertRaises(ValueError): self.inspect()
    def test_fixture_missing_world_column_update_refused(self):
        self.setup_grants(); self.execute('REVOKE UPDATE(writer_epoch) ON TABLE {}.worldwright_worlds FROM {}',self.schema,self.runtime)
        with self.assertRaises(ValueError): self.inspect()
    def test_fixture_missing_invite_update_refused(self):
        self.setup_grants(); self.execute('REVOKE UPDATE ON TABLE {}.worldwright_player_invites FROM {}',self.schema,self.runtime)
        with self.assertRaises(ValueError): self.inspect()
    def test_fixture_missing_sequence_usage_refused(self):
        self.setup_grants(); self.execute('REVOKE USAGE ON SEQUENCE {}.worldwright_gameplay_audit_id_seq FROM {}',self.schema,self.runtime)
        with self.assertRaises(ValueError): self.inspect()
    def test_fixture_migration_or_recovery_membership_refused(self):
        self.setup_grants()
        for parent in (self.migration,self.recovery):
            self.execute('GRANT {} TO {}',parent,self.runtime)
            try:
                with self.assertRaises(ValueError): self.inspect()
            finally: self.execute('REVOKE {} FROM {}',parent,self.runtime)
    def test_fixture_inherited_password_update_refused(self):
        self.setup_grants(); self.execute('GRANT UPDATE(password_hash) ON TABLE {}.worldwright_accounts TO {}',self.schema,self.group); self.execute('GRANT {} TO {}',self.group,self.runtime)
        with self.assertRaises(ValueError): self.inspect()
    def test_fixture_role_flags_and_schema_creation_refused(self):
        self.setup_grants(); self.execute('ALTER ROLE {} SUPERUSER',self.runtime)
        with self.assertRaises(ValueError): self.inspect()
        self.execute('ALTER ROLE {} NOSUPERUSER',self.runtime); self.execute('GRANT CREATE ON SCHEMA {} TO {}',self.schema,self.runtime)
        with self.assertRaises(ValueError): self.inspect()
    def test_fixture_migration_owner_and_checksum_refused(self):
        self.setup_grants(); self.execute('ALTER TABLE {}.worldwright_outbox OWNER TO {}',self.schema,self.recovery)
        with self.assertRaises(ValueError): self.inspect()
        self.execute('ALTER TABLE {}.worldwright_outbox OWNER TO {}',self.schema,self.migration)
        self.db.execute("UPDATE _sqlx_migrations SET checksum=decode('00','hex')")
        with self.assertRaises(ValueError): self.inspect()
    def test_fixture_grant_setup_transaction_rolls_back(self):
        self.execute('GRANT CREATE ON DATABASE {} TO PUBLIC',self.database)
        changed=dict(roles.COLUMNS); changed['worldwright_accounts']=['no_such_fixture_column']
        with patch.object(roles,'COLUMNS',changed),self.assertRaises(psycopg.errors.UndefinedColumn): self.inspect(True)
        self.assertFalse(self.db.execute("SELECT has_schema_privilege(%s,%s,'USAGE')",(self.runtime,self.schema)).fetchone()[0])
        self.assertFalse(self.db.execute("SELECT has_table_privilege(%s,%s,'INSERT')",(self.runtime,self.schema+'.worldwright_sessions')).fetchone()[0])
        self.assertTrue(self.db.execute("SELECT has_database_privilege(%s,current_database(),'CREATE')",(self.runtime,)).fetchone()[0])
    def test_fixture_real_runtime_insert_and_denied_writes(self):
        self.setup_grants(); self.execute('SET ROLE {}',self.runtime)
        try:
            row=self.db.execute("INSERT INTO worldwright_gameplay_audit(payload) VALUES('SYNTHETIC fixture') RETURNING id").fetchone(); self.assertGreater(row[0],0)
            for statement in ("UPDATE worldwright_accounts SET password_hash='not-credentials'","UPDATE worldwright_accounts SET disabled=true","INSERT INTO worldwright_operator_grants(payload) VALUES('SYNTHETIC')"):
                with self.assertRaises(psycopg.errors.InsufficientPrivilege): self.db.execute(statement)
        finally: self.db.execute('RESET ROLE')

if __name__=='__main__':
    result=unittest.main(verbosity=2,exit=False).result
    destination=os.environ.get('WORLDWRIGHT_R3_FIXTURE_REPORT')
    if destination:
        path=Path(destination)
        if path.exists() or path.is_symlink(): raise RuntimeError('new fixture observation report required')
        value={'schema_version':1,'kind':'local-R3-SYNTHETIC-role-fixture-tests','status':'passed' if result.wasSuccessful() and not result.skipped else 'failed','scope':'actual PostgreSQL privileges/SQL on bounded synthetic schema and one executed fixture migration; NOT current eight-file app migration, deployed role/Secret/SQLx audit or service adoption','full_r3_closed':False,'tests_run':result.testsRun,'passed':result.testsRun-len(result.errors)-len(result.failures)-len(result.skipped),'errors':[str(test) for test,_ in result.errors],'failures':[str(test) for test,_ in result.failures],'skipped':len(result.skipped)}
        with os.fdopen(os.open(path,os.O_CREAT|os.O_EXCL|os.O_WRONLY,0o600),'w') as file: file.write(json.dumps(value,indent=2)+'\n')
    raise SystemExit(0 if result.wasSuccessful() and not result.skipped else 1)
