"""Run R7 tool tests and a real draft NO-GO check; never generates project approval."""
from pathlib import Path
import argparse
import ast
import json
import re
import sys
import time
import r7_core as c
from r3_io import collect_phase
import worldwright_local as local


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    local.configure_tools_environment()
    output=local.new_output(args.output)
    output.mkdir(mode=0o700)
    report={'schema_version':1,'track':'R7','kind':'tool-validation-only','status':'failed',
            'synthetic_tests_only':True,'supported_private_beta_authorized':False,'started_at':int(time.time()),'phases':[]}
    subjects=['scripts/r7.py','scripts/r7_core.py','scripts/r7-check.py','scripts/test-r7.py','scripts/r3_io.py',
              'scripts/migration-repair.py','scripts/supply-chain.py','scripts/test-release-followthrough.py','scripts/release-evidence.py',
              'scripts/r6_support.py','scripts/r6-native.py','scripts/r6-evidence.py','scripts/test-r6.py','scripts/r6_subjects.py',
              'scripts/worldwright_local.py','scripts/test-worldwright-local.py','scripts/collect-worldwright-signoff.py',
              'scripts/native_paths.py','scripts/native-storage.py','scripts/test-native-paths.py','scripts/r6-browser.py',
              'scripts/python_dependencies.py','scripts/python-security.py','scripts/run-python-unit.py','scripts/test-python-security.py']
    def inventory(): return [{'path':p,'sha256_lf_text':c.sha(c.raw(c.ROOT/p).replace(b'\r\n',b'\n'))} for p in subjects]
    def phase(name,command,expected=0,timeout=120):
        print('R7 tool phase: '+name+' (limit '+str(timeout)+' seconds)',flush=True)
        result=collect_phase(command,timeout,cwd=c.ROOT)
        c.write_new(output/(name+'.log'),result['stdout'])
        report['phases'].append({'name':name,'exit_code':result['returncode'],'interruption':result['interruption'],
                               'log_sha256':c.sha(result['stdout']),'expected_exit_code':expected,
                               'timeout_seconds':timeout,'elapsed_seconds':result['elapsed_seconds']})
        print('R7 phase finished: '+name+'; elapsed '+str(result['elapsed_seconds'])+' seconds; exit '+str(result['returncode'])+'; interruption '+str(result['interruption']),flush=True)
        c.require(result['interruption'] is None and type(result['returncode']) is int and result['returncode']==expected,'validation-phase-failed:'+name)
        return result['stdout']
    try:
        report['subjects']=inventory()
        for p in subjects: ast.parse((c.ROOT/p).read_text(encoding='utf-8'),filename=p)
        # Each isolated test creates a Git checkout and a signed fixture packet.
        # The observed Windows run completed 29 cases before the old 120s suite cap.
        # This is an execution deadline, not a product performance/acceptance threshold.
        output_text=phase('synthetic-tests',[sys.executable,str(c.ROOT/'scripts/test-r7.py')],timeout=600).decode('utf-8','replace').replace('\r\n','\n')
        counts=re.findall(r'^Ran (\d+) tests? in ',output_text,re.M)
        c.require(counts==['35'] and '\nOK\n' in output_text and '(skipped=' not in output_text,'exact-test-count-or-skips')
        report['tests_passed']=35
        for name,script,count,key in [('native-path-helpers','test-native-paths.py',16,'native_path_tests_passed'),
                                      ('local-storage','test-worldwright-local.py',8,'local_storage_tests_passed'),
                                      ('r6-helper-v2','test-r6.py',22,'r6_helper_tests_passed'),
                                      ('release-followthrough','test-release-followthrough.py',11,'followthrough_tests_passed')]:
            text=phase(name,[sys.executable,str(c.ROOT/'scripts'/script)]).decode('utf-8','replace').replace('\r\n','\n')
            c.require(re.findall(r'^Ran (\d+) tests? in ',text,re.M)==[str(count)] and '\nOK\n' in text and '(skipped=' not in text,'extra-suite-count-or-skips:'+name)
            report[key]=count
        phase('draft-init',[sys.executable,str(c.ROOT/'scripts/r7.py'),'init','--output',str(output/'draft')])
        phase('draft-no-go',[sys.executable,str(c.ROOT/'scripts/r7.py'),'check','--candidate',str(output/'draft/candidate.json'),'--output',str(output/'no-go')],1)
        decision=c.load(output/'no-go/decision.json')
        c.require(decision['decision']=='NO-GO' and decision['supported_private_beta_authorized'] is False and decision['blockers']==['open-prerequisite-blockers'],'draft-must-remain-blocked')
        c.require(report['subjects']==inventory(),'tool-source-changed-during-validation')
        report.update(status='tool-tests-passed-release-not-authorized',draft_no_go_confirmed=True)
    except Exception as error:
        report['failure']=str(error) if isinstance(error,ValueError) else type(error).__name__
    c.write_new(output/'validation.json',json.dumps(report,sort_keys=True,indent=2).encode()+b'\n')
    print('R7 tool validation:',report['status']); print('Evidence:',output)
    if report.get('failure'): print('Failure:',report['failure'])
    print('No real native/live acceptance or supported release has been authorized.')
    return 0 if report['status']=='tool-tests-passed-release-not-authorized' else 1

if __name__=='__main__':
    try: raise SystemExit(main())
    except (OSError,ValueError) as error: print('R7 validation failed:',error); raise SystemExit(1)
