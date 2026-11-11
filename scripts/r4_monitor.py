"""Read-only PG/loopback telemetry/Linux server-process samples; never operator API."""
from pathlib import Path
import hashlib, json, math, os, re, threading, time
from urllib.request import build_opener,ProxyHandler
import psycopg
from psycopg import sql
from r4_common import require,integer,origin,sha
from r3_io import NoRedirects
TABLES=('worldwright_game_counters','worldwright_scheduled_counter_adjustments','worldwright_gameplay_receipts','worldwright_operation_receipts','worldwright_gameplay_audit','worldwright_gameplay_timer_audit','worldwright_sessions','worldwright_reward_claims','worldwright_item_instances')
def metrics_parse(raw):
    require(len(raw)<=256*1024,'metrics body bound')
    values={}
    for line in raw.decode().splitlines():
        if not line or line.startswith('#'): continue
        key,value=line.rsplit(None,1)
        if key.startswith('worldwright_'):
            number=float(value); require(math.isfinite(number) and number>=0 and key not in values,'finite unique telemetry')
            values[key]=number
    for key in ('worldwright_ready','worldwright_connected_clients','worldwright_pg_pool_size','worldwright_pg_pool_idle'): require(key in values,'required real server metric: '+key)
    require(values['worldwright_ready']==1,'server not ready')
    return values
class Monitor:
    def __init__(self,plan,path):
        self.plan=plan; self.path=Path(path); self.stop=threading.Event(); self.failed=None; self.count=0; self.first=None; self.last=None; self.max_rss=0; self.started=time.monotonic()
        self.http=build_opener(ProxyHandler({}),NoRedirects())
        self.thread=threading.Thread(target=self._run,name='r4-private-monitor',daemon=True)
    def process(self):
        root=Path('/proc')/str(self.plan['process_pid']); stat=(root/'stat').read_text(); fields=stat[stat.rfind(')')+2:].split()
        executable=root/'exe'; require(executable.resolve().name.startswith('worldwright-server'),'observed process is not server')
        identity={'pid':self.plan['process_pid'],'start_ticks':int(fields[19]),'executable_sha256':self.identity['executable_sha256'] if hasattr(self,'identity') else sha(executable)}
        # Cache executable hash after first sample; PID start ticks still checked every time.
        if hasattr(self,'identity'): require(identity==self.identity,'observed process replaced')
        self.identity=identity
        rss=int(fields[21])*os.sysconf('SC_PAGE_SIZE'); cpu=(int(fields[11])+int(fields[12]))/os.sysconf('SC_CLK_TCK')
        return identity,rss,cpu
    def sample(self,db):
        plan=self.plan
        db.execute('SELECT pg_stat_clear_snapshot()')
        version,role,database=db.execute('SELECT current_setting(\'server_version\'),current_user,current_database()').fetchone()
        world=db.execute(sql.SQL("SELECT writer_epoch,active_writer_id::text,authority_incarnation::text,encode(active_release_digest,'hex'),game_state_version,release_maintenance FROM {}.worldwright_worlds WHERE world_id=%s").format(sql.Identifier(plan['schema'])),('00000000-0000-0000-0000-000000000001',)).fetchone()
        require(world and world[3]==plan['artifact_digest'] and not world[5] and world[0]>0 and world[1],'actual active artifact/writer readiness')
        peers=db.execute('SELECT COUNT(*) FROM pg_stat_activity WHERE datname=current_database() AND usename=%s AND client_addr=%s::inet',(plan['runtime_role'],plan['pod_ip'])).fetchone()[0]
        require(peers>0,'no actual runtime DB connection from expected Pod')
        stats=db.execute('SELECT xact_commit,xact_rollback,tup_inserted,tup_updated,tup_deleted,stats_reset::text FROM pg_stat_database WHERE datname=current_database()').fetchone()
        rows=db.execute('SELECT relname,n_tup_ins,n_tup_upd,n_tup_del,pg_total_relation_size(relid) FROM pg_stat_user_tables WHERE schemaname=%s AND relname=ANY(%s)',(plan['schema'],list(TABLES))).fetchall()
        require({row[0] for row in rows}==set(TABLES),'complete monitored app table inventory')
        identity,rss,cpu=self.process()
        with self.http.open(plan['telemetry_url'],timeout=3) as response:
            require(response.status==200,'private telemetry unavailable'); telemetry=metrics_parse(response.read(256*1024+1))
        result={'monotonic_seconds':round(time.monotonic()-self.started,6),'unix_seconds':time.time(),'pg_version':version,'monitor_role':role,'database_sha256':hashlib.sha256(database.encode()).hexdigest(),'writer_epoch':world[0],'writer_id':world[1],'incarnation':world[2],'artifact_digest':world[3],'state_version':world[4],'runtime_peers':peers,'pg_stats':list(stats),'tables':{row[0]:list(row[1:]) for row in rows},'process':identity,'rss_bytes':rss,'cpu_seconds':cpu,'telemetry':telemetry}
        if self.first:
            for key in ('database_sha256','writer_epoch','writer_id','incarnation','artifact_digest','process'): require(result[key]==self.first[key],'monitor identity/authority drift: '+key)
            require(result['pg_stats'][-1]==self.first['pg_stats'][-1],'PostgreSQL stats reset invalidates window')
            require(result['state_version']>=self.last['state_version'],'state version rewound')
        self.max_rss=max(self.max_rss,rss); self.last=result
        if self.first is None: self.first=result
        return result
    def _run(self):
        try:
            value=os.environ.get(self.plan['database_url_env']); require(value,'explicit private monitoring DB credentials required')
            with psycopg.connect(value,autocommit=True,connect_timeout=5,sslmode='verify-full',options='-c default_transaction_read_only=on -c statement_timeout=3000') as db:
                require(db.pgconn.ssl_in_use,'verified private PG TLS required')
                with os.fdopen(os.open(self.path,os.O_CREAT|os.O_EXCL|os.O_WRONLY,0o600),'w') as output:
                    while not self.stop.is_set():
                        if self.count>=4000: raise ValueError('monitor sample bound')
                        result=self.sample(db); output.write(json.dumps(result,separators=(',',':'))+'\n'); output.flush(); self.count+=1
                        self.stop.wait(1)
        except Exception as error: self.failed=type(error).__name__
    def start(self):
        self.thread.start()
        end=time.monotonic()+10
        while self.first is None and self.failed is None and time.monotonic()<end: time.sleep(.02)
        require(self.first is not None and not self.failed,'real monitor did not start')
    def finish(self):
        self.stop.set(); self.thread.join(timeout=10)
        require(not self.thread.is_alive() and self.failed is None and self.count>=2,'complete live monitoring required')
        return {'sample_count':self.count,'duration_seconds':self.last['monotonic_seconds'],'first':self.first,'last':self.last,'max_rss_bytes':self.max_rss,'history_growth_bytes':sum(self.last['tables'][key][-1]-self.first['tables'][key][-1] for key in TABLES),'rss_growth_bytes':self.max_rss-self.first['rss_bytes'],'pg_stats_scope':'whole dedicated database; observer queries add transactions; not per-command SQL attribution'}
