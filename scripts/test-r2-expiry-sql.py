"""Supplementary real PostgreSQL fixture/COMMIT tests; NOT Rust/SQLx owner acceptance.
Requires a loopback disposable worldwright_test DB. Applies immutable source SQL
in unique current-run schemas, extracts actual fixture SQL, and always checks cleanup.
"""
from pathlib import Path
import ast
import hashlib
import importlib.util
import json
import os
import re
import threading
import unittest
import uuid
from urllib.parse import urlsplit, urlunsplit, urlencode, unquote
import psycopg
from psycopg import sql

ROOT = Path(__file__).resolve().parents[1]

def load(name):
    spec = importlib.util.spec_from_file_location(name.replace('-', '_'), ROOT/'scripts'/(name+'.py'))
    value = importlib.util.module_from_spec(spec); spec.loader.exec_module(value); return value

def source_python_sql(prefix, marker):
    nodes = ast.walk(ast.parse((ROOT/'scripts/r2-live.py').read_text()))
    values = [node.value for node in nodes if isinstance(node, ast.Constant) and isinstance(node.value, str) and node.value.startswith(prefix) and marker in node.value]
    if len(values) != 1:
        raise ValueError('actual Python expiry SQL is absent or ambiguous')
    return values[0]

def source_rust_sql(marker):
    text = (ROOT/'crates/worldwright-persistence/src/lib.rs').read_text()
    values = [value for value in re.findall(r'sqlx::query\("([^"\n]+)"\)', text) if value.startswith('UPDATE worldwright_sessions SET ') and marker in value]
    if len(values) != 1:
        raise ValueError('actual Rust expiry SQL is absent or ambiguous')
    # These exact simple fixture statements contain one $1 parameter, no quoted text containing $1.
    if values[0].count('$1') != 1:
        raise ValueError('unexpected fixture parameter contract')
    return values[0].replace('$1', '%s')

class ExpirySqlTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        url = os.environ.get('WORLDWRIGHT_TEST_DATABASE_URL', '')
        parsed = urlsplit(url)
        if parsed.scheme not in ('postgres', 'postgresql') or parsed.hostname not in ('localhost','127.0.0.1','::1') or 'worldwright_test' not in unquote(parsed.path) or parsed.query:
            raise ValueError('explicit loopback worldwright_test URL required; no caller search_path')
        cls.url = url
        cls.db = psycopg.connect(url, autocommit=True, connect_timeout=5)
        with cls.db.cursor() as cursor:
            cursor.execute('SELECT current_database(), current_user, version()')
            cls.identity = cursor.fetchone()
            if 'worldwright_test' not in cls.identity[0]: raise ValueError('actual database-name guard failed')
        cls.schema = 'ww_r2_'+uuid.uuid4().hex
        cls.created = False
        try:
            with cls.db.cursor() as cursor:
                cursor.execute(sql.SQL('CREATE SCHEMA {}').format(sql.Identifier(cls.schema)))
                cls.created = True
                print('WORLDWRIGHT_R2_SCHEMA_CREATED='+cls.schema, flush=True)
                cursor.execute(sql.SQL('SET search_path TO {}').format(sql.Identifier(cls.schema)))
                cursor.execute("SET statement_timeout='5s'")
                cursor.execute("SET lock_timeout='2s'")
            cls.migrations = {path.name:hashlib.sha384(path.read_bytes()).hexdigest() for path in sorted((ROOT/'migrations').glob('*.sql'))}
            if len(cls.migrations) != 8: raise ValueError('expected reviewed eight-file inventory; review changed inventory')
            # Authentication fixtures need these exact source migrations only.
            # The separate FullMigrationTests below MUST run the complete inventory.
            for name in ('20261020000100_durable_gameplay.sql', '20261118000300_oidc_identity.sql'):
                with cls.db.transaction(), cls.db.cursor() as cursor:
                    cursor.execute((ROOT/'migrations'/name).read_text())
            with cls.db.cursor() as cursor:
                cursor.execute("INSERT INTO worldwright_worlds(world_id,content_namespace,content_schema_version) VALUES(%s,'reference',1)", (uuid.UUID(int=1),))
                for identifier, username, name in [(1,'alice','Alice'), (2,'bob','Bob')]:
                    cursor.execute('INSERT INTO worldwright_accounts(account_id,username,password_hash) VALUES(%s,%s,%s)', (uuid.UUID(int=identifier),username,'fixture-schema-only-not-auth'))
                    cursor.execute("INSERT INTO worldwright_characters(character_id,account_id,world_id,character_name,room_definition) VALUES(%s,%s,%s,%s,'reference:rooms.atrium')", (uuid.UUID(int=identifier),uuid.UUID(int=identifier),uuid.UUID(int=1),name))
            cls.addClassCleanup(cls.clean)
        except Exception:
            cls.clean(); raise
    @classmethod
    def clean(cls):
        try:
            if cls.created:
                with cls.db.cursor() as cursor:
                    cursor.execute(sql.SQL('DROP SCHEMA {} CASCADE').format(sql.Identifier(cls.schema)))
                    cursor.execute('SELECT EXISTS(SELECT 1 FROM pg_namespace WHERE nspname=%s)',(cls.schema,))
                    if cursor.fetchone()[0]: raise ValueError('expiry SQL schema leaked')
                print('WORLDWRIGHT_R2_SCHEMA_DROPPED='+cls.schema, flush=True)
                cls.created = False
        finally:
            cls.db.close()
    def setUp(self):
        self.token = hashlib.sha256(uuid.uuid4().bytes).digest()
        with self.db.cursor() as cursor:
            cursor.execute('DELETE FROM worldwright_sessions')
            cursor.execute('DELETE FROM worldwright_player_invites')
    def session(self, account=1, revoked=False):
        token = hashlib.sha256(uuid.uuid4().bytes).digest()
        with self.db.cursor() as cursor:
            cursor.execute("INSERT INTO worldwright_sessions(token_hash,account_id,character_id,world_id,expires_at,revoked_at) VALUES(%s,%s,%s,%s,statement_timestamp()+INTERVAL '12 hours',CASE WHEN %s THEN clock_timestamp() ELSE NULL END)",(token,uuid.UUID(int=account),uuid.UUID(int=account),uuid.UUID(int=1),revoked))
        return token
    def expired(self, token):
        with self.db.cursor() as cursor:
            cursor.execute('SELECT created_at<expires_at AND expires_at<statement_timestamp() FROM worldwright_sessions WHERE token_hash=%s',(token,))
            return cursor.fetchone()[0]
    def test_migrations_source_immutable_and_tables_present(self):
        current={path.name:hashlib.sha384(path.read_bytes()).hexdigest() for path in sorted((ROOT/'migrations').glob('*.sql'))}
        self.assertEqual(current,self.migrations)
        with self.db.cursor() as cursor:
            for table in ('worldwright_sessions','worldwright_player_invites','worldwright_oidc_identities','worldwright_oidc_audit'):
                cursor.execute('SELECT to_regclass(%s) IS NOT NULL',(table,)); self.assertTrue(cursor.fetchone()[0])
    def test_old_session_fixture_rejected_by_existing_check(self):
        token=self.session()
        with self.assertRaises(psycopg.errors.CheckViolation) as caught, self.db.cursor() as cursor:
            cursor.execute("UPDATE worldwright_sessions SET expires_at=statement_timestamp()-INTERVAL '1 second' WHERE token_hash=%s",(token,))
        self.assertEqual(caught.exception.sqlstate,'23514'); self.assertFalse(self.expired(token))
    def test_actual_rust_session_fixture_sql(self):
        token=self.session()
        with self.db.cursor() as cursor: cursor.execute(source_rust_sql("INTERVAL '2 seconds'"),(token,))
        self.assertTrue(self.expired(token))
    def test_actual_rust_retention_fixture_sql(self):
        token=self.session()
        with self.db.cursor() as cursor: cursor.execute(source_rust_sql("INTERVAL '2 days'"),(token,))
        self.assertTrue(self.expired(token))
    def test_actual_python_session_sql_scoped_and_preserves_revocation(self):
        alice=self.session(); bob=self.session(2); revoked=self.session(revoked=True)
        with self.db.cursor() as cursor: cursor.execute(source_python_sql('UPDATE worldwright_sessions SET ',"INTERVAL '2 seconds'"))
        self.assertTrue(self.expired(alice)); self.assertFalse(self.expired(bob)); self.assertFalse(self.expired(revoked))
    def test_old_invite_fixture_rejected_by_existing_check(self):
        with self.assertRaises(psycopg.errors.CheckViolation) as caught, self.db.cursor() as cursor:
            cursor.execute("INSERT INTO worldwright_player_invites(invite_id,world_id,account_id,issuer,token_hash,expires_at) VALUES(%s,%s,%s,%s,%s,statement_timestamp()-INTERVAL '1 second')",(uuid.uuid4(),uuid.UUID(int=1),uuid.UUID(int=2),'https://fixture.example.invalid',self.token))
        self.assertEqual(caught.exception.sqlstate,'23514')
    def test_actual_python_invite_fixture_sql(self):
        identifier=uuid.uuid4()
        with self.db.cursor() as cursor:
            cursor.execute(source_python_sql('INSERT INTO worldwright_player_invites(',"character_name='Bob'"),(identifier,'https://fixture.example.invalid',self.token))
            self.assertEqual(cursor.rowcount,1)
            cursor.execute('SELECT created_at<expires_at AND expires_at<statement_timestamp() AND consumed_at IS NULL FROM worldwright_player_invites WHERE invite_id=%s',(identifier,))
            self.assertTrue(cursor.fetchone()[0])
            cursor.execute('SELECT COUNT(*) FROM worldwright_oidc_identities'); self.assertEqual(cursor.fetchone()[0],0)
            cursor.execute('SELECT COUNT(*) FROM worldwright_oidc_audit'); self.assertEqual(cursor.fetchone()[0],0)
    def test_actual_postgres_commit_fault_boundaries(self):
        wire=load('r2-wire')
        parsed=urlsplit(self.url)
        with self.db.cursor() as cursor: cursor.execute('CREATE TABLE r2_wire_values(id INTEGER PRIMARY KEY)')
        proxy=wire.CommitProxy((parsed.hostname,parsed.port or 5432))
        try:
            credentials=parsed.netloc.rsplit('@',1)[0]+'@' if '@' in parsed.netloc else ''
            url=urlunsplit((parsed.scheme,credentials+'127.0.0.1:'+str(proxy.address[1]),parsed.path,urlencode({'sslmode':'disable','options':'-csearch_path='+self.schema}),''))
            for index,mode in enumerate(('before','after','lost-ack')):
                with self.subTest(mode=mode):
                    conn=psycopg.connect(url,connect_timeout=5)
                    conn.execute('INSERT INTO r2_wire_values(id) VALUES(%s)',(index,))
                    errors=[]
                    def commit():
                        try: conn.commit()
                        except psycopg.Error as error: errors.append(type(error).__name__)
                    proxy.arm(mode); thread=threading.Thread(target=commit); thread.start()
                    try:
                        self.assertTrue(proxy.hit.wait(5),'actual PostgreSQL COMMIT boundary absent')
                        with self.db.cursor() as cursor:
                            cursor.execute('SELECT COUNT(*) FROM r2_wire_values WHERE id=%s',(index,))
                            self.assertEqual(cursor.fetchone()[0],0 if mode=='before' else 1)
                    finally:
                        proxy.release.set(); thread.join(10); conn.close()
                    self.assertFalse(thread.is_alive()); self.assertTrue(errors,'uncertain/lost acknowledgement incorrectly returned success')
            self.assertFalse(proxy.errors)
        finally: proxy.close()

class FullMigrationTests(unittest.TestCase):
    def test_complete_current_migrations_apply_unchanged(self):
        url=os.environ.get('WORLDWRIGHT_TEST_DATABASE_URL','')
        parsed=urlsplit(url)
        if parsed.scheme not in ('postgres','postgresql') or parsed.hostname not in ('localhost','127.0.0.1','::1') or 'worldwright_test' not in unquote(parsed.path) or parsed.query:
            raise ValueError('loopback disposable database required')
        with psycopg.connect(url,autocommit=True,connect_timeout=5) as db:
            schema='ww_r2_'+uuid.uuid4().hex
            created=False
            try:
                with db.cursor() as cursor:
                    cursor.execute('SELECT current_database()')
                    self.assertIn('worldwright_test',cursor.fetchone()[0])
                    cursor.execute(sql.SQL('CREATE SCHEMA {}').format(sql.Identifier(schema)))
                    created=True
                    print('WORLDWRIGHT_R2_SCHEMA_CREATED='+schema,flush=True)
                    cursor.execute(sql.SQL('SET search_path TO {}').format(sql.Identifier(schema)))
                    cursor.execute("SET statement_timeout='5s'")
                migrations=sorted((ROOT/'migrations').glob('*.sql'))
                self.assertEqual(len(migrations),8)
                for migration in migrations:
                    print('WORLDWRIGHT_R2_MIGRATION='+migration.name,flush=True)
                    with db.transaction(),db.cursor() as cursor:
                        cursor.execute(migration.read_text())
            finally:
                if created:
                    with db.cursor() as cursor:
                        cursor.execute(sql.SQL('DROP SCHEMA {} CASCADE').format(sql.Identifier(schema)))
                        cursor.execute('SELECT EXISTS(SELECT 1 FROM pg_namespace WHERE nspname=%s)',(schema,))
                        self.assertFalse(cursor.fetchone()[0])
                    print('WORLDWRIGHT_R2_SCHEMA_DROPPED='+schema,flush=True)

if __name__=='__main__':
    result=unittest.main(verbosity=2,exit=False).result
    report_path=os.environ.get('WORLDWRIGHT_R2_SQL_REPORT')
    if report_path:
        path=Path(report_path).resolve()
        if path.exists() or path.is_relative_to(ROOT) or not path.parent.is_dir():
            raise ValueError('new supplementary report outside checkout required')
        value={'schema_version':1,'kind':'supplementary-postgresql-fixture-sql','status':'passed' if result.wasSuccessful() and not result.skipped else 'failed','full_r2_closed':False,
            'scope':'actual fixture SQL/real PostgreSQL wire tests; NOT compiled Rust/SQLx/current-owner or approved-provider acceptance',
            'database':getattr(ExpirySqlTests,'identity',None),'tests_run':result.testsRun,'passed':result.testsRun-len(result.errors)-len(result.failures)-len(result.skipped),
            'errors':[{'test':str(test),'diagnostic':diagnostic} for test,diagnostic in result.errors],
            'failures':[{'test':str(test),'diagnostic':diagnostic} for test,diagnostic in result.failures],
            'skipped':len(result.skipped),'migrations':[{'name':p.name,'sha384':hashlib.sha384(p.read_bytes()).hexdigest()} for p in sorted((ROOT/'migrations').glob('*.sql'))]}
        path.write_text(json.dumps(value,indent=2)+'\n')
    raise SystemExit(0 if result.wasSuccessful() and not result.skipped else 1)
