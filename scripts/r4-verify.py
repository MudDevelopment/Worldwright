"""R4 actual native fixed-load/scenario collection and independent closure."""
from pathlib import Path
import argparse,json,os,shutil,sys,time
from r4_common import ROOT,CASES,read,write_new,sha,require,plan_check,signed,beta_report,load,contained,compare
from r3_io import record_phase,secret_values
from r4_monitor import Monitor
PHASES={'r3-prerequisites','rustc','fmt','build-load','load-version','integrity-list','integrity-pg','deployment-before','smoke','beta','scenarios','deployment-after'}
def r3_command(path):
    args=read(path); expected={'collection','image','adoption','r2-arguments','approval-public-key'}
    require(set(args)==expected and all(isinstance(value,str) and Path(value).is_absolute() for value in args.values()),'all real R3 closure inputs required')
    return [sys.executable,str(ROOT/'scripts/r3-close.py')]+[item for key in sorted(args) for item in ('--'+key,args[key])]
def deployment(plan):
    return ['kubectl','--context',plan['context'],'-n',plan['namespace'],'get','deployment',plan['deployment'],'-o','json']
def check_deployment(value,plan):
    require(value['metadata']['name']==plan['deployment'] and value['metadata']['namespace']==plan['namespace'],'observed deployment identity')
    require(type(value['spec']['replicas']) is int and value['spec']['replicas']==1 and value['spec']['strategy']['type']=='Recreate' and type(value['status'].get('readyReplicas')) is int and value['status']['readyReplicas']==1,'actual single ready writer deployment')
    require(value['spec']['template']['spec']['containers'][0]['image'].count('@sha256:')==1,'actual immutable deployment image')
    return {'uid':value['metadata']['uid'],'generation':value['metadata']['generation'],'image_reference':value['spec']['template']['spec']['containers'][0]['image']}
def collect(args):
    output=args.output.resolve(); require(not output.exists() and not output.is_relative_to(ROOT),'new external private output directory required'); output.mkdir(mode=0o700)
    report={'schema_version':1,'kind':'r4-collection','status':'failed','full_r4_closed':False,'phases':[]}; monitor=None
    def phase(name,command,timeout=1800,retain=True):
        result=record_phase(report,output,name,command,timeout,cwd=ROOT,sensitive=secret_values(),retain=retain)
        require(result['interruption'] is None and result['returncode']==0,'required phase failed/interrupted: '+name); return result['stdout']
    try:
        require(args.confirm_load,'explicit isolated load/fixture mutations confirmation required')
        plan=read(args.plan); plan_check(plan); signed(plan,args.approval_public_key,'r4-performance-plan')
        for tool in ('cargo','rustc','kubectl','git'): require(shutil.which(tool),'missing required actual R4 tool: '+tool)
        source=load('r4_source',ROOT/'scripts/r0-verify.py').source_identity(); require(source['source_sha256']==plan['source_sha256'],'current source differs from approved plan')
        report.update(source=source,plan_sha256=sha(args.plan),plan=plan,started_unix_seconds=int(time.time()))
        phase('r3-prerequisites',r3_command(plan['r3_arguments']))
        version=phase('rustc',['rustc','-vV']).decode(); require('release: 1.85.1' in version and 'host: x86_64-unknown-linux-gnu' in version,'actual pinned Ubuntu/Linux native compiler')
        host='x86_64-unknown-linux-gnu'; target=output/'native-target'
        phase('fmt',['cargo','fmt','--all','--','--check'])
        common=['--locked','--offline','--target',host,'--target-dir',str(target)]
        phase('build-load',['cargo','build','--release','-p','worldwright-load',*common])
        binary=target/host/'release/worldwright-load'; report['load_binary_sha256']=sha(binary)
        phase('load-version',[str(binary),'--version'])
        require(os.environ.get('WORLDWRIGHT_TEST_DATABASE_URL'),'separate owned disposable integrity database required')
        listed=phase('integrity-list',['cargo','test','-p','worldwright-persistence',*common,'--','--ignored','--list']).decode()
        for test in ('gameplay_browser_request_id_lifecycle','timer_commit_fences_reconstructs_and_rolls_back_atomically','oidc_link_concurrency_collision_revocation_and_restart','session_retention_preserves_receipts_and_active_sessions'): require('database_tests::'+test+': test' in listed,'required native integrity test absent')
        phase('integrity-pg',['cargo','test','-p','worldwright-persistence',*common,'--','--ignored','--test-threads=1'])
        before=check_deployment(json.loads(phase('deployment-before',deployment(plan),retain=False)),plan); report['deployment']=before
        prior=read(read(plan['r3_arguments'])['collection'])
        require(before['uid']==prior['deployment_uid'] and before['image_reference']==prior['image_reference'] and plan['artifact_digest']==prior['artifact_digest'],'load deployment/image/artifact must match closed R3 tuple')
        monitor=Monitor(plan,output/'monitor.jsonl'); monitor.start()
        base=[str(binary),'run','--base-url',plan['base_url'],'--origin',plan['base_url'],'--credentials',plan['credentials'],'--output',str(output/'beta.json'),'--server-revision',source['commit'],'--artifact-digest',plan['artifact_digest'],'--environment-label',plan['environment_label']]
        smoke=list(base); smoke[1]='__smoke'; smoke[smoke.index('--credentials')+1]=plan['scenario_credentials']; smoke[smoke.index('--output')+1]=str(output/'smoke.json'); smoke+=['--clients','2','--commands','2','--period-ms','100']
        phase('smoke',smoke,180); beta_report(read(output/'smoke.json'),smoke=True)
        phase('beta',base,220); beta=beta_report(read(output/'beta.json'))
        for key in ('artifact_digest','environment_label'): require(beta[key]==plan[key],'client report target drift')
        require(beta['server_revision']==source['commit'],'client revision drift')
        phase('scenarios',[sys.executable,str(ROOT/'scripts/r4-scenarios.py'),'--plan',str(args.plan),'--approval-public-key',str(args.approval_public_key),'--output',str(output/'scenarios.json')],min(3600,plan['soak_seconds']+400))
        summary=monitor.finish(); monitor=None; report['monitor']=summary
        require(summary['rss_growth_bytes']<=plan['max_rss_growth_bytes'] and summary['history_growth_bytes']<=plan['max_history_growth_bytes'],'adopted resource/history growth bounds exceeded')
        after=check_deployment(json.loads(phase('deployment-after',deployment(plan),retain=False)),plan); require(before==after,'deployment replaced during measured run')
        require(source==load('r4_source_after',ROOT/'scripts/r0-verify.py').source_identity(),'source/lock changed during collection')
        report['evidence']={name:sha(output/name) for name in ('beta.json','smoke.json','scenarios.json','monitor.jsonl')}; report['status']='collected_not_closed'; report['ended_unix_seconds']=int(time.time())
    except Exception as error: report['failure']={'type':type(error).__name__,'reason':str(error) if isinstance(error,ValueError) else 'private collector failure; inspect phase logs'}
    finally:
        if monitor:
            monitor.stop.set(); monitor.thread.join(timeout=10)
        write_new(output/'r4-report.json',report)
    print('R4 collection:',report['status']); return 0 if report['status']=='collected_not_closed' else 1

def closure(args):
    report=read(args.report); plan=report['plan']; plan_check(plan); signed(plan,args.approval_public_key,'r4-performance-plan')
    require(report.get('kind')=='r4-collection' and report.get('schema_version')==1 and report.get('status')=='collected_not_closed' and report.get('full_r4_closed') is False,'actual complete R4 collection required')
    source=load('r4_closure_source',ROOT/'scripts/r0-verify.py').source_identity(); require(source==report['source'] and source['source_sha256']==plan['source_sha256'],'current source/lock tuple differs')
    prior=read(read(plan['r3_arguments'])['collection']); require(report['deployment']['uid']==prior['deployment_uid'] and report['deployment']['image_reference']==prior['image_reference'] and plan['artifact_digest']==prior['artifact_digest'],'same closed R3 deployment/image/artifact required')
    phases=report['phases']; require(len(phases)==len(PHASES) and {p['name'] for p in phases}==PHASES,'complete phase inventory')
    for item in phases:
        require(type(item['exit_code']) is int and item['exit_code']==0 and item.get('interruption') is None,'required actual phase outcome')
        path=contained(args.report.parent,item['name']+'.log'); require(sha(path)==item['log_sha256'],'phase log modified')
    require(set(report['evidence'])=={'beta.json','smoke.json','scenarios.json','monitor.jsonl'},'complete evidence inventory')
    for name,digest in report['evidence'].items(): require(sha(contained(args.report.parent,name,128*1024*1024))==digest,'evidence modified')
    beta=beta_report(read(args.report.parent/'beta.json')); beta_report(read(args.report.parent/'smoke.json'),smoke=True)
    for key in ('artifact_digest','environment_label'): require(beta[key]==plan[key],'load target differs')
    require(beta['server_revision']==source['commit'],'load source differs')
    scenarios=read(args.report.parent/'scenarios.json'); require(scenarios.get('kind')=='r4-supplemental-scenarios' and scenarios.get('status')=='passed','actual separate scenarios required')
    for key in ('source_sha256','artifact_digest','environment_label'): require(scenarios[key]==plan[key],'scenario tuple differs')
    cases=scenarios['cases']; require(len(cases)==len(CASES) and {case['case'] for case in cases}==CASES and all(case.get('status')=='passed' and type(case.get('observations')) is int and case['observations']>0 for case in cases),'complete non-skipped scenario inventory')
    soak=next(case for case in cases if case['case']=='soak'); require(isinstance(soak.get('duration_seconds'),(int,float)) and not isinstance(soak['duration_seconds'],bool) and soak['duration_seconds']>=plan['soak_seconds'],'full actual separate soak required')
    summary=report['monitor']; require(summary['rss_growth_bytes']<=plan['max_rss_growth_bytes'] and summary['history_growth_bytes']<=plan['max_history_growth_bytes'],'adopted growth bounds')
    # Recompute sample summary instead of trusting manually edited JSON aggregates.
    samples=[json.loads(line) for line in contained(args.report.parent,'monitor.jsonl',128*1024*1024).read_text().splitlines()]; require(2<=len(samples)<=4000,'bounded complete monitor samples')
    require(summary['first']==samples[0] and summary['last']==samples[-1] and type(summary['sample_count']) is int and summary['sample_count']==len(samples) and summary['max_rss_bytes']==max(sample['rss_bytes'] for sample in samples),'monitor aggregates must derive from actual samples')
    actual_rss=max(sample['rss_bytes'] for sample in samples)-samples[0]['rss_bytes']
    actual_history=sum(samples[-1]['tables'][key][-1]-samples[0]['tables'][key][-1] for key in samples[0]['tables'])
    require(summary['rss_growth_bytes']==actual_rss and summary['history_growth_bytes']==actual_history and actual_rss<=plan['max_rss_growth_bytes'] and actual_history<=plan['max_history_growth_bytes'],'actual recomputed resource/history growth bounds')
    require(samples[0]['unix_seconds']<=beta['started_unix_seconds']+1 and samples[-1]['unix_seconds']>=beta['ended_unix_seconds']-1,'monitor must cover actual client run')
    for left,right in zip(samples,samples[1:]):
        require(0<right['monotonic_seconds']-left['monotonic_seconds']<=6,'monitor gaps/reset')
        for key in ('process','artifact_digest','writer_epoch','writer_id','incarnation','database_sha256'): require(left[key]==right[key],'monitor tuple changed')
        require(right['telemetry']['worldwright_ready']==1 and right['runtime_peers']>0 and right['state_version']>=left['state_version'] and right['pg_stats'][-1]==left['pg_stats'][-1],'coherent actual DB/ready evidence')
    require(samples[-1]['monotonic_seconds']-samples[0]['monotonic_seconds']>=plan['soak_seconds'],'monitor must cover long-duration soak')
    adoption=read(args.adoption); signed(adoption,args.approval_public_key,'r4-performance-adoption')
    for key,value in {'report_sha256':sha(args.report),'source_sha256':source['source_sha256'],'artifact_digest':plan['artifact_digest'],'image_reference':report['deployment']['image_reference'],'deployment_uid':report['deployment']['uid'],'retention_policy_sha256':plan['retention_policy_sha256']}.items(): require(adoption.get(key)==value,'adoption tuple differs: '+key)
    require(adoption.get('capacity_scope')=='beta-250-v1-only' and adoption.get('retention_adopted') is True,'explicit bounded capacity/retention adoption required')
    from r3_io import collect_phase
    result=collect_phase(r3_command(plan['r3_arguments']),1800,cwd=ROOT); require(result['returncode']==0 and result['interruption'] is None,'current R3/R2 prerequisites must still close')
    print('R4 exact beta and separately scoped soak/adoption verified; no production or 500+ capacity implied')
def main():
    parser=argparse.ArgumentParser(); sub=parser.add_subparsers(dest='operation',required=True)
    collect_parser=sub.add_parser('collect'); collect_parser.add_argument('--plan',type=Path,required=True); collect_parser.add_argument('--output',type=Path,required=True); collect_parser.add_argument('--confirm-load',action='store_true'); collect_parser.add_argument('--approval-public-key',type=Path,required=True)
    close_parser=sub.add_parser('close'); close_parser.add_argument('--report',type=Path,required=True); close_parser.add_argument('--adoption',type=Path,required=True); close_parser.add_argument('--approval-public-key',type=Path,required=True)
    compare_parser=sub.add_parser('compare'); compare_parser.add_argument('--baseline',type=Path,required=True); compare_parser.add_argument('--candidate',type=Path,required=True); compare_parser.add_argument('--bottleneck',required=True); compare_parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    try:
        if args.operation=='collect': return collect(args)
        if args.operation=='close': closure(args)
        else: write_new(args.output,compare(read(args.baseline),read(args.candidate),args.bottleneck))
        return 0
    except Exception as error: print('R4 NOT CLOSED:',type(error).__name__,str(error)); return 1
if __name__=='__main__': raise SystemExit(main())
