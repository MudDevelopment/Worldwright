"""Read-only history capture and isolated repair proposal. NEVER applies a repair."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import re
import time
import worldwright_local as local
from fresh_baseline import baseline_bytes
from urllib.parse import urlsplit,parse_qs
ROOT=Path(__file__).resolve().parents[1]
TARGET='20261106000100_operational_control.sql'
EXPECTED='6ddb32cf0e6f68bb1c0de353b58400896a90c664d0afc6c4401995fb2a8cf221d75f099c9ee16f4f794a394691e0998a'

def require(value,message):
    if not value: raise ValueError(message)

def write(path,value):
    data=value if isinstance(value,bytes) else json.dumps(value,sort_keys=True,indent=2).encode()+b'\n'
    with os.fdopen(os.open(path,os.O_CREAT|os.O_EXCL|os.O_WRONLY,0o600),'wb') as stream:
        stream.write(data);stream.flush();os.fsync(stream.fileno())

def output(path):
    local.configure_tools_environment();path=local.new_output(path)
    path.mkdir(mode=0o700);return path

def proposal(root=ROOT):
    path=Path(root)/'migrations'/TARGET
    require(path.is_file() and not path.is_symlink() and path.stat().st_size<=1024*1024,'bounded original migration required')
    data,fixed=baseline_bytes(root)
    require(path.read_bytes() in (data,fixed),'current migration differs from both reviewed baselines; stop for history review')
    return data,fixed

def disposition(history):
    if history is None:return 'history-inventory-required'
    require(type(history.get('schema_version')) is int and history['schema_version']==1 and history.get('kind')=='read-only-migration-history','history schema')
    require(history.get('read_only') is True and type(history.get('user_relation_count')) is int and history['user_relation_count']>=0,'history read-only/count fields')
    rows=history.get('migrations');require(isinstance(rows,list) and len(rows)<=1000,'history bounds')
    seen=set()
    for row in rows:
        require(type(row.get('version')) is int and row['version']>0 and row['version'] not in seen and type(row.get('success')) is bool,'history row')
        require(isinstance(row.get('checksum_sha384'),str) and re.fullmatch('[0-9a-f]{96}',row['checksum_sha384']),'history checksum')
        seen.add(row['version'])
    if any(row['version']==20261106000100 and row['success'] for row in rows):return 'successful-history-baseline-reconciliation-required'
    if rows:return 'partial-history-review-required'
    if history['user_relation_count']:return 'nonempty-database-review-required'
    return 'pristine-database-owner-review-required'

def capture(env_name,allow_remote):
    import psycopg
    from psycopg import sql
    require(re.fullmatch('[A-Za-z_][A-Za-z0-9_]{0,127}',env_name),'environment variable name required')
    url=os.environ.get(env_name,'');parts=urlsplit(url)
    require(parts.scheme in ('postgres','postgresql') and parts.hostname and parts.username and len(parts.path)>1,'explicit database URI required')
    options=parse_qs(parts.query)
    require(not set(options)&{'host','hostaddr','port','dbname','user','password','service','servicefile'},'URI routing overrides are refused')
    if parts.hostname not in ('localhost','127.0.0.1','::1'):
        require(allow_remote and parse_qs(parts.query).get('sslmode')==['verify-full'],'remote read requires --allow-remote-read and verify-full TLS')
    with psycopg.connect(url,connect_timeout=5,application_name='worldwright-history-readonly') as conn:
        conn.execute('SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY')
        conn.execute("SET LOCAL statement_timeout='3s'")
        database,schema,version=conn.execute('SELECT current_database(),current_schema(),version()').fetchone()
        require(schema is not None,'current schema missing')
        table=conn.execute('SELECT to_regclass(%s)',(f'"{schema.replace(chr(34),chr(34)*2)}"."_sqlx_migrations"',)).fetchone()[0]
        rows=[]
        if table is not None:
            result=conn.execute(sql.SQL('SELECT version,success,encode(checksum,\'hex\') FROM {} ORDER BY version LIMIT 1001').format(sql.Identifier(schema,'_sqlx_migrations'))).fetchall()
            require(len(result)<=1000,'history inventory exceeds bound')
            rows=[{'version':r[0],'success':r[1],'checksum_sha384':r[2]} for r in result]
        count=conn.execute("SELECT COUNT(*) FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname NOT IN ('pg_catalog','information_schema') AND n.nspname NOT LIKE 'pg_toast%' AND c.relkind IN ('r','p','v','m','S','f')").fetchone()[0]
        result={'schema_version':1,'kind':'read-only-migration-history','observed_at':int(time.time()),'read_only':True,
                'database':database,'schema':schema,'server_version':version,'user_relation_count':count,'migrations':rows,'repair_authorized':False}
        conn.rollback();return result

def main():
    p=argparse.ArgumentParser(description=__doc__);sub=p.add_subparsers(dest='action',required=True)
    a=sub.add_parser('capture-history');a.add_argument('--database-url-env',required=True);a.add_argument('--allow-remote-read',action='store_true');a.add_argument('--output',type=Path,required=True)
    a=sub.add_parser('plan');a.add_argument('--history',type=Path);a.add_argument('--output',type=Path,required=True)
    args=p.parse_args();out=output(args.output)
    try:
        if args.action=='capture-history':write(out/'history.json',capture(args.database_url_env,args.allow_remote_read))
        else:
            history=None
            if args.history:
                require(args.history.is_file() and not args.history.is_symlink() and args.history.stat().st_size<4*1024*1024,'bounded history file required')
                history=json.loads(args.history.read_text())
            old,fixed=proposal();status=disposition(history)
            entries=[{'name':x.name,'sha384':hashlib.sha384(x.read_bytes()).hexdigest()} for x in sorted((ROOT/'migrations').glob('*.sql'))]
            write(out/('PROPOSED-'+TARGET),fixed)
            write(out/'plan.json',{'schema_version':1,'kind':'migration-repair-proposal','status':status,'repair_authorized':False,
                'historical_files_modified':False,'target':TARGET,'original_sha384':EXPECTED,'proposed_sha384':hashlib.sha384(fixed).hexdigest(),
                'current_migrations':entries,'history_sha256':None if args.history is None else hashlib.sha256(args.history.read_bytes()).hexdigest(),
                'required_actions':['Inventory ALL affected database histories','Obtain named database/security/source-owner approval','Choose pristine corrected baseline or separately designed history reconciliation','Rebuild and run native complete migrations and recovery tests','Recollect all affected same-release evidence; do not rewrite historical logs/checksums']})
        print('Read-only capture/proposal retained:',out,'; no repair applied or approved');return 0
    except Exception as error:
        write(out/'failure.json',{'kind':'migration-repair-proposal','status':'failed','repair_authorized':False,'failure_type':type(error).__name__})
        print('Migration review stopped; preserve',out);return 1

if __name__=='__main__':raise SystemExit(main())
