"""Isolated PostgreSQL rehearsal of current baseline and actual literal SQL.
Never connects to deployment databases; never changes repository migrations.
Not Rust/SQLx, live restore, Windows, policy adoption, or R7 release acceptance.
"""
from pathlib import Path
import argparse,hashlib,importlib.util,json,tempfile,uuid
import pgserver,psycopg,pglast
from psycopg import sql
from psycopg.types.json import Jsonb
from r6_sql_source import constants,one,queries
ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('repair',ROOT/'scripts/migration-repair.py');repair=importlib.util.module_from_spec(spec);spec.loader.exec_module(repair)

def main():
    repair.require(__debug__, 'run rehearsal without Python -O; assertions must execute')
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path,required=True);args=parser.parse_args()
    out=repair.output(args.output);report={'schema_version':1,'kind':'current-baseline-sql-rehearsal-with-historical-negative','status':'failed','cases':[],
        'native_rust_executed':False,'migration_repair_applied_to_repository':False,'release_authorized':False}
    files=sorted((ROOT/'migrations').glob('*.sql'));before={p.name:hashlib.sha384(p.read_bytes()).hexdigest() for p in files}
    def passed(name,**details):
        report['cases'].append({'case':name,'status':'passed',**details});print('PASS',name,flush=True)
    server=None;tmp=None
    try:
        assert len(files)==9,'review migration inventory change'
        old,fixed=repair.proposal();assert (ROOT/'migrations'/repair.TARGET).read_bytes()==fixed,'fresh baseline must be applied first'
        pglast.parse_sql(fixed.decode())
        q=constants(ROOT/'crates/worldwright-persistence/src/operational_queries.rs')
        assert len(q)==20,len(q)
        for name,value in q.items():pglast.parse_sql(value)
        for file in ('retention.rs','recovery_security.rs'):
            for value in queries(ROOT/'crates/worldwright-persistence/src'/file):pglast.parse_sql(value)
        passed('current-literal-sql-parses',operational_constants=len(q))
        tmp=tempfile.TemporaryDirectory(prefix='ww-r6-sql-');server=pgserver.get_server(Path(tmp.name)/'data',cleanup_mode='delete')
        with psycopg.connect(server.get_uri(),autocommit=True) as conn:
            report['postgres_version']=conn.execute('SELECT version()').fetchone()[0]
            conn.execute("SET statement_timeout='5s'")
            conn.execute('CREATE SCHEMA original_check');conn.execute('SET search_path=original_check,public')
            failure=None
            for p in files:
                try:
                    with conn.transaction():conn.execute(old.decode() if p.name==repair.TARGET else p.read_text())
                except psycopg.errors.SyntaxError as error:
                    failure={'migration':p.name,'sqlstate':error.sqlstate};break
            assert failure=={'migration':repair.TARGET,'sqlstate':'42601'},failure
            passed('original-baseline-blocker-reproduced',**failure)
            conn.execute('CREATE SCHEMA proposal_check');conn.execute('SET search_path=proposal_check,public')
            for p in files:
                with conn.transaction():conn.execute(fixed.decode() if p.name==repair.TARGET else p.read_text())
            passed('nine-current-migrations-apply-with-historical-negative-retained',migration_count=len(files))
            for name,value in q.items():
                if value.lstrip().startswith('LOCK '):
                    with conn.transaction():conn.execute(value)
                else:conn.execute(sql.SQL('PREPARE {} AS ').format(sql.Identifier(name.lower()))+sql.SQL(value))
            def execute(name,*values):
                return conn.execute(sql.SQL('EXECUTE {} ({})').format(sql.Identifier(name.lower()),sql.SQL(',').join(map(sql.Literal,values))))
            world=uuid.UUID(int=1);account=uuid.UUID(int=2);character=uuid.UUID(int=3)
            conn.execute("INSERT INTO worldwright_worlds(world_id,content_namespace,content_schema_version,writer_epoch,active_writer_id) VALUES(%s,'test',1,7,%s)",(world,uuid.UUID(int=9)))
            execute('INSERT_OPERATOR','admin','Admin','human')
            execute('INSERT_GRANT',uuid.UUID(int=4),'admin','manage_operators','staging',world,'admin','rehearsal',None)
            now=conn.execute('SELECT EXTRACT(EPOCH FROM clock_timestamp())').fetchone()[0]
            auth=execute('AUTHORIZE_OPERATOR','admin',float(now),uuid.UUID(int=5),'manage_operators','staging',world).fetchone()
            assert auth==('human',True,True,True,True),auth
            execute('INSERT_OPERATOR_AUDIT',uuid.UUID(int=6),uuid.UUID(int=7),'admin',uuid.UUID(int=5),'mfa','grant_created','staging',world,'grant','rehearsal','rehearsal','confirmed',Jsonb({'capability':'manage_operators'}),None,None)
            execute('REVOKE_OPERATOR_SESSION',uuid.UUID(int=5),'admin','admin','rehearsal')
            auth=execute('AUTHORIZE_OPERATOR','admin',float(now),uuid.UUID(int=5),'manage_operators','staging',world).fetchone();assert auth[3] is False
            passed('authorization-audit-and-session-revocation-SQL-execute')
            recovery=ROOT/'crates/worldwright-persistence/src/recovery_security.rs'
            with conn.transaction(force_rollback=True):
                try:
                    with conn.transaction():conn.execute('UPDATE worldwright_worlds SET active_writer_id=NULL,writer_epoch=8 WHERE world_id=%s',(world,))
                except psycopg.errors.CheckViolation:pass
                else:raise AssertionError('old invalid writer shape unexpectedly accepted')
            statement=one(recovery,'UPDATE worldwright_worlds SET authority_incarnation=gen_random_uuid()')
            conn.execute('PREPARE quarantine AS '+statement)
            execute('quarantine',world)
            assert conn.execute('SELECT writer_epoch,active_writer_id FROM worldwright_worlds').fetchone()==(0,None)
            passed('restore-quarantine-obeys-writer-constraint')
            conn.execute("INSERT INTO worldwright_accounts(account_id,username,password_hash) VALUES(%s,'alice','test-only')",(account,))
            conn.execute("INSERT INTO worldwright_characters(character_id,account_id,world_id,character_name,room_definition) VALUES(%s,%s,%s,'Alice','test:rooms.home')",(character,account,world))
            conn.execute("INSERT INTO worldwright_sessions(token_hash,account_id,character_id,world_id,created_at,expires_at) VALUES(%s,%s,%s,%s,clock_timestamp()-INTERVAL '2 days',clock_timestamp()-INTERVAL '1 day')",(b'a'*32,account,character,world))
            conn.execute("INSERT INTO worldwright_sessions(token_hash,account_id,character_id,world_id,expires_at) VALUES(%s,%s,%s,%s,clock_timestamp()+INTERVAL '1 hour')",(b'b'*32,account,character,world))
            conn.execute("INSERT INTO worldwright_player_invites(invite_id,world_id,account_id,issuer,token_hash,created_at,expires_at) VALUES(%s,%s,%s,'https://example.invalid',%s,clock_timestamp()-INTERVAL '2 days',clock_timestamp()-INTERVAL '1 day')",(uuid.UUID(int=10),world,account,b'c'*32))
            conn.execute("INSERT INTO worldwright_operation_receipts(world_id,account_id,character_id,request_id,operation_id,intent_kind,intent_fingerprint,outcome,result) VALUES(%s,%s,%s,'req',%s,'claim_reward',%s,'inventory_full','{}')",(world,account,character,uuid.UUID(int=11),b'd'*32))
            conn.execute("INSERT INTO worldwright_outbox(operation_id,world_id,topic,payload) VALUES(%s,%s,'reward_claimed','{}')",(uuid.UUID(int=11),world))
            retention=ROOT/'crates/worldwright-persistence/src/retention.rs'
            conn.execute('PREPARE prune_sessions AS '+one(retention,'WITH candidates AS (SELECT token_hash'))
            conn.execute('PREPARE prune_invites AS '+one(retention,'WITH candidates AS (SELECT i.invite_id'))
            conn.execute('PREPARE retention_record AS '+one(retention,'INSERT INTO worldwright_retention_runs'))
            with conn.transaction(force_rollback=True):
                assert execute('prune_sessions',world,1).rowcount==1
            assert conn.execute('SELECT COUNT(*) FROM worldwright_sessions').fetchone()[0]==2
            with conn.transaction():
                deleted=execute('prune_sessions',world,1).rowcount;invites=execute('prune_invites',world,1).rowcount
                assert (deleted,invites)==(1,1)
                execute('retention_record',uuid.UUID(int=12),world,'staging','admin',uuid.UUID(int=5),uuid.UUID(int=7),'expired-authentication-only-v1','rehearsal',2,deleted,invites,Jsonb({'capability':'manage_operators'}))
            assert conn.execute('SELECT COUNT(*) FROM worldwright_sessions').fetchone()[0]==1
            assert conn.execute('SELECT COUNT(*) FROM worldwright_operation_receipts').fetchone()[0]==1
            assert conn.execute('SELECT COUNT(*) FROM worldwright_outbox').fetchone()[0]==1
            passed('bounded-authentication-pruning-rollback-and-protected-records')
            try:
                with conn.transaction():
                    execute('retention_record',uuid.UUID(int=12),world,'staging','admin',uuid.UUID(int=5),uuid.UUID(int=7),'expired-authentication-only-v1','rehearsal',2,1,1,Jsonb({}))
            except psycopg.errors.UniqueViolation:pass
            else:raise AssertionError('duplicate retention operation accepted')
            passed('retention-operation-uniqueness')
        assert before=={p.name:hashlib.sha384(p.read_bytes()).hexdigest() for p in files}
        passed('repository-migration-bytes-unchanged')
        report['status']='rehearsal-passed-current-baseline'
    except Exception as error:
        report['failure_type']=type(error).__name__;report['failure']=str(error)[:1500]
    finally:
        if server is not None:server.cleanup()
        if tmp is not None:tmp.cleanup()
        report['cleanup']='completed';report['migration_hashes']=before
        repair.write(out/'rehearsal.json',report)
    print(report['status'],out/'rehearsal.json')
    return 0 if report['status']=='rehearsal-passed-current-baseline' else 1
if __name__=='__main__':raise SystemExit(main())
