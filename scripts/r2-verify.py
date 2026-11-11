"""R2 collect/close gates. Collection is NEVER release acceptance. Python 3.11+."""
from pathlib import Path
import argparse
import hashlib
import importlib.util
import json
import os
import platform
import re
import shutil
import signal
import subprocess
import sys
from urllib.parse import urlsplit, unquote

ROOT=Path(__file__).resolve().parents[1]
DB_REQUIRED={
 'r2_migration_fresh_upgrade_corrupt_future_and_failed_ledgers',
 'r2_expiry_fixtures_preserve_lifetime_and_reject_expired_identity',
 'durable_sessions_restart_lookup_revocation_and_expiry',
 'r2_both_game_contracts_state_replay_conflict_and_authority',
 'r2_concurrent_state_candidates_have_one_commit',
 'r2_equal_deadlines_and_lock_latency_do_not_slide_recovery',
 'r2_gameplay_audit_failure_rolls_back_every_candidate_write',
 'r2_lost_result_restart_preserves_business_entitlement_uniqueness',
 'gameplay_browser_request_id_lifecycle', 'timer_commit_fences_reconstructs_and_rolls_back_atomically',
 'oidc_link_concurrency_collision_revocation_and_restart', 'session_retention_preserves_receipts_and_active_sessions',
}
UNIT_REQUIRED={'r2_signed_claims_reject_bad_issuer_audience_nonce_signature_and_expiry',
 'r2_rotated_key_requires_refreshed_key_material','r2_start_binds_unique_nonce_state_and_exact_s256_verifier',
 'r2_expired_pending_state_and_provider_outage_are_one_shot','r2_gameplay_rng_checkpoint_rewinds_draws_without_constant_fallback'}
PROCESS_REQUIRED={'kill-before-commit','kill-after-commit','lost-commit-ack','current-owner-no-speculative-success',
 'no-historical-speech-rebroadcast','receipt-audit-entitlement-uniqueness','idle-restart-clock-rng'}
COLLECTION_REQUIRED={'host','cargo-version','rustc-version','fmt','check','clippy','db-discovery','all-ignored-pg','owner-identity-discovery','all-owner-network-unit','fixture-build','browser-resume','r2-helper','r2-harness-helper','expiry-fixture-sql','process-driver','controlled-driver'}
PROVIDER_REQUIRED={'issuer-audience-signature','nonce-state-pkce','key-rotation','invite-expiry-consumption-denied-link',
 'revocation-outage-restart','browser-playable-attachment-no-password-fallback'}

def module(name,file):
 spec=importlib.util.spec_from_file_location(name,ROOT/'scripts'/file)
 result=importlib.util.module_from_spec(spec); spec.loader.exec_module(result); return result

def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()

def digest(value):
 if not isinstance(value,str) or not re.fullmatch('[a-f0-9]{64}',value): raise ValueError('missing/invalid evidence digest')

def read(path):
 if path.is_symlink() or not path.is_file() or path.stat().st_size>16*1024*1024: raise ValueError('invalid evidence file')
 value=json.loads(path.read_text())
 if not isinstance(value,dict) or type(value.get('schema_version')) is not int or value['schema_version']!=1: raise ValueError('unsupported evidence')
 return value

def external(path,required,source,kind):
 value=read(path)
 if value.get('kind')!=kind or value.get('status')!='passed' or not isinstance(value.get('reviewer'),str) or not value['reviewer'].strip(): raise ValueError('real reviewed '+kind+' evidence missing')
 for field in ('source_sha256','lock_sha256'):
  digest(value.get(field))
  if value[field]!=source[field]: raise ValueError('external source/lock mismatch')
 digest(value.get('artifact_sha256')); digest(value.get('log_sha256'))
 log=path.with_suffix('.log')
 if log.is_symlink() or not log.is_file() or log.stat().st_size>128*1024*1024 or sha(log)!=value['log_sha256']: raise ValueError('external log missing/modified')
 if kind != 'approved-provider':
  if value.get('schema_cleanup') != 'verified': raise ValueError('live schema cleanup not verified')
  child_logs=value.get('child_logs')
  if not isinstance(child_logs,list) or not child_logs: raise ValueError('live child log inventory absent')
  child_names=set()
  for child in child_logs:
   if not isinstance(child,dict): raise ValueError('invalid child log record')
   name=child.get('name','')
   if not re.fullmatch(r'ww_r2_[a-f0-9]{32}-[0-9]+\.log',name) or name in child_names: raise ValueError('invalid/duplicate child log')
   child_names.add(name); digest(child.get('sha256'))
   child_path=path.parent/name
   if child_path.is_symlink() or not child_path.is_file() or sha(child_path)!=child['sha256']: raise ValueError('child log missing/modified')
  digest(value.get('fixture_sha256'))
 records=value.get('cases')
 if not isinstance(records,list) or any(not isinstance(case,dict) or case.get('status')!='passed' for case in records): raise ValueError('missing/failed real case')
 if any(not isinstance(case.get('case'),str) or not case['case'] for case in records): raise ValueError('invalid case name')
 names={case['case'] for case in records}
 if len(names)!=len(records) or not required<=names: raise ValueError('incomplete/duplicate '+kind+' matrix')
 if kind=='approved-provider':
  issuer=urlsplit(value.get('issuer',''))
  if issuer.scheme!='https' or not issuer.hostname or issuer.username or issuer.password or issuer.hostname.endswith('.invalid'): raise ValueError('actual approved HTTPS issuer required; not a sample domain')
 return value

def collect(args):
 output=args.output.resolve()
 if output.exists() or output.is_relative_to(ROOT): raise ValueError('new private external output required')
 os.umask(0o077); output.mkdir(mode=0o700)
 report={'schema_version':1,'track':'R2','status':'failed','phases':[],'full_r2_closed':False}
 connection=None; created=set(); baseline=set()
 def run(name,command):
  process=subprocess.Popen(command,cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,start_new_session=(os.name!='nt'))
  try:
   stdout,_=process.communicate(timeout=1800)
  except subprocess.TimeoutExpired:
   if os.name=='nt': subprocess.run(['taskkill','/PID',str(process.pid),'/T','/F'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=10)
   else: os.killpg(process.pid,signal.SIGKILL)
   stdout,_=process.communicate(timeout=10)
  result=subprocess.CompletedProcess(command,process.returncode,stdout)
  text=result.stdout.decode('utf-8','replace').replace(os.environ.get('WORLDWRIGHT_TEST_DATABASE_URL','\0'),'[TEST-DB-URL]')
  (output/(name+'.log')).write_text(text)
  report['phases'].append({'name':name,'exit_code':result.returncode,'log_sha256':sha(output/(name+'.log'))})
  created.update(re.findall(r'WORLDWRIGHT_TEST_SCHEMA_CREATED=(ww_test_[0-9]+_[0-9]+)',text))
  created.update(re.findall(r'WORLDWRIGHT_R2_SCHEMA_CREATED=(ww_r2_[a-f0-9]{32})',text))
  if result.returncode: raise ValueError('required phase failed: '+name)
  return text
 try:
  if not shutil.which('cargo') or not shutil.which('rustc'): raise ValueError('required native Cargo/rustc missing')
  if sys.platform.startswith('linux'):
   distribution=platform.freedesktop_os_release()
   report['distribution']={'id':distribution.get('ID'),'version':distribution.get('VERSION_ID')}
  else: report['distribution']={'id':sys.platform,'version':platform.version()}
  report['source']=module('r2_source','r0-verify.py').source_identity()
  r0=module('r2_r0_prerequisite','r0-r1-close.py').check_r0(args.r0_report,platform.system())
  if r0['source']!=report['source']: raise ValueError('R0 source tuple differs before collection')
  server_artifact=Path(r0['binaries']['worldwright-server']['path'])
  if not server_artifact.is_file() or sha(server_artifact)!=r0['binaries']['worldwright-server']['sha256']: raise ValueError('R0 production server artifact missing/modified')
  report['server_artifact_sha256']=sha(server_artifact)
  host=run('host',['rustc','-vV']); host=next(line[6:] for line in host.splitlines() if line.startswith('host: '))
  report['host_target']=host
  if run('cargo-version',['cargo','--version']).split()[1]!='1.85.1' or run('rustc-version',['rustc','--version']).split()[1]!='1.85.1': raise ValueError('pin mismatch')
  url=os.environ.get('WORLDWRIGHT_TEST_DATABASE_URL',''); parsed=urlsplit(url)
  if parsed.scheme not in ('postgres','postgresql') or 'worldwright_test' not in unquote(parsed.path): raise ValueError('approved disposable worldwright_test URL required')
  import psycopg
  connection=psycopg.connect(url,connect_timeout=5,autocommit=True)
  with connection.cursor() as cursor:
   cursor.execute("SET statement_timeout='5s'"); cursor.execute("SET lock_timeout='2s'")
   cursor.execute('SELECT current_database(),current_user,version()')
   database,role,version=cursor.fetchone()
   if 'worldwright_test' not in database: raise ValueError('actual DB guard failed')
   report['database']={'name':database,'role':role,'engine':version}
   cursor.execute("SELECT nspname FROM pg_namespace WHERE nspname LIKE 'ww_test_%' OR nspname LIKE 'ww_r2_%'")
   baseline={row[0] for row in cursor.fetchall()}
  migrations=sorted((ROOT/'migrations').glob('*.sql'))
  report['migrations']=[{'name':path.name,'sha384':hashlib.sha384(path.read_bytes()).hexdigest()} for path in migrations]
  run('expiry-fixture-sql',[sys.executable,'scripts/test-r2-expiry-sql.py'])
  target=output/'native-target'; common=['--locked','--offline','--target',host,'--target-dir',str(target)]
  run('fmt',['cargo','fmt','--all','--','--check'])
  run('check',['cargo','check','--workspace','--all-targets',*common])
  run('clippy',['cargo','clippy','--workspace','--all-targets','--all-features',*common,'--','-D','warnings'])
  listing=run('db-discovery',['cargo','test','-p','worldwright-persistence','--lib',*common,'--','--ignored','--list'])
  names=set(re.findall(r'^([a-zA-Z0-9_:]+): test$',listing,re.M))
  if not names or not DB_REQUIRED<={name.split('::')[-1] for name in names}: raise ValueError('required DB tests undiscovered')
  text=run('all-ignored-pg',['cargo','test','-p','worldwright-persistence','--lib',*common,'--','--ignored','--test-threads=1','--nocapture'])
  summary=re.findall(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored;',text)
  if not summary or int(summary[-1])!=len(names): raise ValueError('required DB count/skip mismatch')
  report['database_tests_executed']=len(names)
  listing=run('owner-identity-discovery',['cargo','test','-p','worldwright-server','--lib',*common,'r2_','--','--list'])
  names=set(re.findall(r'^([a-zA-Z0-9_:]+): test$',listing,re.M))
  if not UNIT_REQUIRED<={name.split('::')[-1] for name in names}: raise ValueError('required owner/identity tests missing')
  owner_output=run('all-owner-network-unit',['cargo','test','-p','worldwright-server','--lib',*common,'--','--test-threads=1'])
  passed={name.split('::')[-1] for name in re.findall(r'^test ([a-zA-Z0-9_:]+) \.\.\. ok$',owner_output,re.M)}
  if not UNIT_REQUIRED<=passed: raise ValueError('required owner/identity unit skipped or not executed')
  run('browser-resume',['node','scripts/test-expedition-browser.cjs'])
  run('r2-helper',[sys.executable,'scripts/test-r2.py'])
  run('r2-harness-helper',[sys.executable,'scripts/test-r2-live.py'])
  listing=run('fixture-build',['cargo','test','-p','worldwright-server','--lib',*common,'--no-run','--message-format=json'])
  artifacts=[json.loads(line) for line in listing.splitlines() if line.startswith('{')]
  executables=[Path(item['executable']) for item in artifacts if item.get('reason')=='compiler-artifact' and item.get('executable') and item.get('target',{}).get('name')=='worldwright_server' and item.get('profile',{}).get('test')]
  if len(executables)!=1 or not executables[0].is_file(): raise ValueError('actual fixture executable not discovered')
  fixture=executables[0]; report['fixture_sha256']=sha(fixture)
  for mode,phase in [('process','process-driver'),('controlled','controlled-driver')]:
   command=[sys.executable,'scripts/r2-live.py',mode,'--fixture-executable',str(fixture),'--server-artifact',str(server_artifact),'--output',str(output/mode)]
   if args.browser_executable: command+=['--browser-executable',str(args.browser_executable)]
   run(phase,command)
  external(output/'process/process.json',PROCESS_REQUIRED,report['source'],'real-process-faults')
  external(output/'controlled/controlled-provider.json',PROVIDER_REQUIRED,report['source'],'controlled-provider')
  if module('r2_final_source','r0-verify.py').source_identity()!=report['source']: raise ValueError('source/lock changed')
  report['status']='collected_not_closed'
 except (OSError,ValueError,KeyError,TypeError,StopIteration,ImportError,subprocess.TimeoutExpired) as error:
  report['failure']=str(error).replace(os.environ.get('WORLDWRIGHT_TEST_DATABASE_URL','\0'),'[TEST-DB-URL]')
 finally:
  if connection is not None:
   try:
    with connection.cursor() as cursor:
     cursor.execute("SELECT nspname FROM pg_namespace WHERE (nspname LIKE 'ww_test_%' OR nspname LIKE 'ww_r2_%') AND nspowner=(SELECT oid FROM pg_roles WHERE rolname=current_user)")
     leftovers=(set(row[0] for row in cursor.fetchall()) & created)-baseline
     report['orphaned_test_schemas']=sorted(leftovers)
     for schema in leftovers:
      cursor.execute(psycopg.sql.SQL('DROP SCHEMA {} CASCADE').format(psycopg.sql.Identifier(schema)))
     if leftovers: report['status']='failed'; report['failure']='owned test schemas leaked; cleaned, NOT accepted'
   except Exception as error:
    report['status']='failed'; report['cleanup_failure']=str(error)
   connection.close()
  (output/'r2-collection.json').write_text(json.dumps(report,indent=2)+'\n')
 print('R2 collection:',report['status'],output/'r2-collection.json')
 return 0 if report['status']=='collected_not_closed' else 1

def close(args):
 value=read(args.collection)
 if value.get('track')!='R2' or value.get('status')!='collected_not_closed' or value.get('orphaned_test_schemas')!=[] or type(value.get('database_tests_executed')) is not int or value['database_tests_executed']<len(DB_REQUIRED): raise ValueError('live collection not accepted')
 phases=value.get('phases')
 if not isinstance(phases,list) or any(not isinstance(phase,dict) for phase in phases) or {phase.get('name') for phase in phases}!=COLLECTION_REQUIRED or len(phases)!=len(COLLECTION_REQUIRED): raise ValueError('required collection phase absent/duplicate')
 if value.get('distribution')!={'id':'ubuntu','version':'24.04'}: raise ValueError('actual Ubuntu R2 collection required')
 checker=module('r2_r0_checker','r0-r1-close.py')
 ubuntu=checker.check_r0(args.ubuntu_r0,'Linux'); windows=checker.check_r0(args.windows_r0,'Windows')
 current=module('r2_current_source','r0-verify.py').source_identity()
 for candidate in (value['source'],ubuntu['source'],windows['source']):
  for field in ('commit','source_sha256','lock_sha256'):
   if candidate[field]!=current[field]: raise ValueError('R0/R2/current source tuple mismatch')
 for phase in value.get('phases',[]):
  path=args.collection.parent/(phase['name']+'.log')
  if phase['exit_code'] != 0 or path.is_symlink() or not path.is_file() or path.stat().st_size>128*1024*1024 or sha(path)!=phase['log_sha256']: raise ValueError('collection log modified/failed')
 expected_migrations=[{'name':path.name,'sha384':hashlib.sha384(path.read_bytes()).hexdigest()} for path in sorted((ROOT/'migrations').glob('*.sql'))]
 if value.get('migrations')!=expected_migrations: raise ValueError('migration evidence differs from current immutable inventory')
 process=external(args.process_report,PROCESS_REQUIRED,current,'real-process-faults')
 controlled=external(args.controlled_report,PROVIDER_REQUIRED,current,'controlled-provider')
 module('r2_owner_approval','r2-approval.py').verify(read(args.provider_report),args.approval_public_key)
 for report in (process,controlled):
  if report['fixture_sha256']!=value.get('fixture_sha256'): raise ValueError('live fixture tuple differs')
 provider=external(args.provider_report,PROVIDER_REQUIRED,current,'approved-provider')
 expected=ubuntu['binaries']['worldwright-server']['sha256']
 if any(report['artifact_sha256']!=expected for report in (process,controlled,provider)) or value.get('server_artifact_sha256')!=expected: raise ValueError('process/provider/R0 server artifact mismatch')
 print('R2 evidence and independently trusted provider-adoption signature verified. R1 and other release tracks are not closed by this command.')
 return 0

if __name__=='__main__':
 parser=argparse.ArgumentParser(description=__doc__); sub=parser.add_subparsers(dest='command',required=True)
 p=sub.add_parser('collect'); p.add_argument('--output',type=Path,required=True)
 p.add_argument('--r0-report',type=Path,required=True); p.add_argument('--browser-executable',type=Path)
 p=sub.add_parser('close')
 for option in ('collection','ubuntu-r0','windows-r0','process-report','controlled-report','provider-report','approval-public-key'): p.add_argument('--'+option,type=Path,required=True)
 args=parser.parse_args()
 try: raise SystemExit(collect(args) if args.command=='collect' else close(args))
 except (OSError,ValueError,KeyError,TypeError) as error:
  print('R2 NOT CLOSED:',error,file=sys.stderr); raise SystemExit(1)
