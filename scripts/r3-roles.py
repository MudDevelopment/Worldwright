"""Live R3 DB identity/grant audit; optional explicit isolated runtime-grant setup.
Never creates users/passwords, runs migrations, or grants recovery/operator authority.
"""
from pathlib import Path
import argparse
import hashlib
import json
import os
import re
import psycopg
from psycopg import sql

ROOT=Path(__file__).resolve().parents[1]
WRITE_TABLES={'worldwright_sessions','worldwright_game_counters','worldwright_scheduled_counter_adjustments','worldwright_gameplay_receipts','worldwright_operation_receipts','worldwright_reward_claims','worldwright_item_instances','worldwright_oidc_identities'}
INSERT_TABLES={'worldwright_gameplay_audit','worldwright_gameplay_timer_audit','worldwright_audit_log','worldwright_outbox','worldwright_oidc_audit'}
COLUMNS={'worldwright_worlds':['writer_epoch','active_writer_id','updated_at','game_state_version','runtime_revision'],'worldwright_accounts':['updated_at'],'worldwright_characters':['room_definition','balance','version','updated_at']}

def require(value,message):
    if not value: raise ValueError(message)

def inspect(db,schema,runtime,migration,recovery,apply=False):
    require(len({runtime,migration,recovery})==3,'migration/runtime/recovery role names must differ')
    require(all(re.fullmatch(r'[a-z_][a-z0-9_]{0,62}',x) for x in (schema,runtime,migration,recovery)),'bounded SQL identifiers required')
    roles=db.execute('SELECT rolname,rolsuper,rolcreatedb,rolcreaterole,rolreplication,rolbypassrls,rolcanlogin FROM pg_roles WHERE rolname=ANY(%s)',([runtime,migration,recovery],)).fetchall()
    require(len(roles)==3 and all(not any(row[1:6]) and row[6] for row in roles),'all three reviewed roles must exist without superuser/cluster creation/replication/bypass authority')
    require(not db.execute("SELECT pg_has_role(%s,%s,'MEMBER') OR pg_has_role(%s,%s,'MEMBER')",(runtime,migration,runtime,recovery)).fetchone()[0],'runtime must not inherit migration/recovery')
    tables=db.execute("SELECT c.relname,pg_get_userbyid(c.relowner) FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=%s AND c.relkind='r' AND (c.relname LIKE 'worldwright_%%' OR c.relname='_sqlx_migrations')",(schema,)).fetchall()
    names={row[0] for row in tables}
    require(WRITE_TABLES|INSERT_TABLES|set(COLUMNS)|{'worldwright_player_invites','_sqlx_migrations'}<=names,'current migrated table inventory required')
    require(all(owner==migration for _,owner in tables),'migration login must own reviewed app tables; no owner exception inferred')
    rows=db.execute(sql.SQL('SELECT version,checksum,success FROM {}._sqlx_migrations ORDER BY version').format(sql.Identifier(schema))).fetchall()
    expected=[(int(p.name.split('_',1)[0]),hashlib.sha384(p.read_bytes()).digest(),True) for p in sorted((ROOT/'migrations').glob('*.sql'))]
    require([(version,bytes(checksum),success) for version,checksum,success in rows]==expected,'exact current successful migration ledger required')
    if apply:
        require('worldwright_test' in db.execute('SELECT current_database()').fetchone()[0],'grant setup restricted to disposable worldwright_test database')
        with db.transaction():
            db.execute(sql.SQL('REVOKE CREATE ON SCHEMA {} FROM PUBLIC, {}').format(sql.Identifier(schema),sql.Identifier(runtime)))
            db.execute(sql.SQL('GRANT USAGE ON SCHEMA {} TO {}').format(sql.Identifier(schema),sql.Identifier(runtime)))
            database=db.execute('SELECT current_database()').fetchone()[0]
            db.execute(sql.SQL('REVOKE CREATE ON DATABASE {} FROM PUBLIC, {}').format(sql.Identifier(database),sql.Identifier(runtime)))
            for table in names:
                qualified=sql.Identifier(schema,table)
                db.execute(sql.SQL('REVOKE ALL ON TABLE {} FROM PUBLIC, {}').format(qualified,sql.Identifier(runtime)))
                db.execute(sql.SQL('GRANT SELECT ON TABLE {} TO {}').format(qualified,sql.Identifier(runtime)))
                if table in WRITE_TABLES:
                    db.execute(sql.SQL('GRANT INSERT,UPDATE,DELETE ON TABLE {} TO {}').format(qualified,sql.Identifier(runtime)))
                elif table in INSERT_TABLES:
                    db.execute(sql.SQL('GRANT INSERT ON TABLE {} TO {}').format(qualified,sql.Identifier(runtime)))
                elif table=='worldwright_player_invites':
                    db.execute(sql.SQL('GRANT UPDATE ON TABLE {} TO {}').format(qualified,sql.Identifier(runtime)))
                elif table in COLUMNS:
                    db.execute(sql.SQL('GRANT UPDATE ({}) ON TABLE {} TO {}').format(sql.SQL(',').join(map(sql.Identifier,COLUMNS[table])),qualified,sql.Identifier(runtime)))
                if table in WRITE_TABLES|INSERT_TABLES:
                    serials=db.execute('SELECT pg_get_serial_sequence(%s,a.attname) FROM pg_attribute a WHERE a.attrelid=%s::regclass AND a.attnum>0 AND NOT a.attisdropped',(schema+'.'+table,schema+'.'+table)).fetchall()
                    for (sequence,) in serials:
                        if sequence:
                            ns,name=sequence.split('.',1)
                            db.execute(sql.SQL('GRANT USAGE ON SEQUENCE {} TO {}').format(sql.Identifier(ns.strip('"'),name.strip('"')),sql.Identifier(runtime)))
    require(db.execute("SELECT has_schema_privilege(%s,%s,'USAGE')",(runtime,schema)).fetchone()[0],'runtime schema USAGE absent')
    require(not db.execute('SELECT has_schema_privilege(%s,%s,\'CREATE\')',(runtime,schema)).fetchone()[0],'runtime cannot CREATE schema objects')
    require(not db.execute("SELECT has_database_privilege(%s,current_database(),'CREATE')",(runtime,)).fetchone()[0],'runtime cannot create database schemas')
    for table in ('_sqlx_migrations','worldwright_content_artifacts','worldwright_publication_requests','worldwright_operator_grants','worldwright_operators','worldwright_recovery_jobs'):
        require(not db.execute('SELECT has_table_privilege(%s,%s,\'INSERT,UPDATE,DELETE,TRUNCATE,REFERENCES,TRIGGER\')',(runtime,schema+'.'+table)).fetchone()[0],'runtime must not mutate migration/release/operator/recovery authority')
    for column in ('password_hash','disabled'):
        require(not db.execute('SELECT has_column_privilege(%s,%s,%s,\'UPDATE\')',(runtime,schema+'.worldwright_accounts',column)).fetchone()[0],'runtime cannot change credentials/offboarding')
    for table in WRITE_TABLES:
        require(db.execute('SELECT has_table_privilege(%s,%s,\'SELECT\') AND has_table_privilege(%s,%s,\'INSERT\') AND has_table_privilege(%s,%s,\'UPDATE\') AND has_table_privilege(%s,%s,\'DELETE\')',(runtime,schema+'.'+table,runtime,schema+'.'+table,runtime,schema+'.'+table,runtime,schema+'.'+table)).fetchone()[0],'runtime transaction grants absent')
    # Audit every prerequisite in the setup recipe, not only full-DML tables.
    for table in names:
        require(db.execute("SELECT has_table_privilege(%s,%s,'SELECT')",(runtime,schema+'.'+table)).fetchone()[0],'runtime reviewed-table SELECT absent')
    for table in INSERT_TABLES:
        require(db.execute("SELECT has_table_privilege(%s,%s,'INSERT')",(runtime,schema+'.'+table)).fetchone()[0],'runtime audit/outbox INSERT absent')
    require(db.execute("SELECT has_table_privilege(%s,%s,'UPDATE')",(runtime,schema+'.worldwright_player_invites')).fetchone()[0],'runtime invite-consumption UPDATE absent')
    for table,columns in COLUMNS.items():
        for column in columns:
            require(db.execute("SELECT has_column_privilege(%s,%s,%s,'UPDATE')",(runtime,schema+'.'+table,column)).fetchone()[0],'runtime required column UPDATE absent')
    for table in WRITE_TABLES|INSERT_TABLES:
        serials=db.execute('SELECT pg_get_serial_sequence(%s,a.attname) FROM pg_attribute a WHERE a.attrelid=%s::regclass AND a.attnum>0 AND NOT a.attisdropped',(schema+'.'+table,schema+'.'+table)).fetchall()
        for (sequence,) in serials:
            if sequence:
                require(db.execute("SELECT has_sequence_privilege(%s,%s,'USAGE')",(runtime,sequence)).fetchone()[0],'runtime owned-sequence USAGE absent')
    return {'runtime_role':runtime,'migration_role':migration,'recovery_role':recovery,'schema':schema,'table_count':len(tables),'migration_count':len(rows),'identity_checks':'passed','runtime_setup_applied':apply,'recovery_authority_adopted':False}

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--database-url-env',required=True)
    for name in ('runtime-role','migration-role','recovery-role'): parser.add_argument('--'+name,required=True)
    parser.add_argument('--schema',default='public'); parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--apply-runtime-grants',action='store_true')
    args=parser.parse_args(); report={'schema_version':1,'track':'R3','kind':'database-role-audit','status':'failed','full_r3_closed':False}
    try:
        require(not args.output.exists() and not args.output.is_symlink(),'new nonsymlink output required')
        url=os.environ.get(args.database_url_env)
        require(bool(url),'DB audit URL environment value required')
        with psycopg.connect(url,autocommit=True,connect_timeout=5) as db:
            db.execute("SET statement_timeout='5s'"); db.execute("SET lock_timeout='2s'")
            report.update(inspect(db,args.schema,args.runtime_role,args.migration_role,args.recovery_role,args.apply_runtime_grants))
        report['status']='passed'; code=0
    except (OSError,ValueError,psycopg.Error) as error:
        report['failure']=type(error).__name__+': role audit/setup failed; no credentials retained'; code=1
    if args.output.exists(): return 1
    with os.fdopen(os.open(args.output,os.O_CREAT|os.O_EXCL|os.O_WRONLY,0o600),'w') as file:
        file.write(json.dumps(report,indent=2)+'\n')
    print('R3 database role audit:',report['status']); return code
if __name__=='__main__': raise SystemExit(main())
