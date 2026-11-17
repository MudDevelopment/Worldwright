"""Build bounded CycloneDX/notices inventory from captured Cargo metadata and lock.
Imported advisory evidence and license declarations are NOT owner approval.
No network, signing, publishing, dependency changes or application execution.
"""
from pathlib import Path
import argparse,hashlib,json,os,re,tomllib

def require(value,message):
    if not value:raise ValueError(message)
def read(path,limit=16*1024*1024):
    path=Path(path);require(path.is_file() and not path.is_symlink() and path.stat().st_size<=limit,'bounded regular file required')
    with path.open('rb') as stream:data=stream.read(limit+1)
    require(len(data)<=limit,'file grew beyond bounds');return data
def hash_bytes(data):return hashlib.sha256(data).hexdigest()
def digest(value):return isinstance(value,str) and re.fullmatch('[0-9a-f]{64}',value) is not None
def unique(pairs):
    result={}
    for key,value in pairs:
        require(key not in result,'duplicate JSON key');result[key]=value
    return result
def load(path):return json.loads(read(path),object_pairs_hook=unique)
def new(path,value):
    data=value if isinstance(value,bytes) else json.dumps(value,sort_keys=True,indent=2).encode()+b'\n'
    with os.fdopen(os.open(path,os.O_CREAT|os.O_EXCL|os.O_WRONLY,0o600),'wb') as stream:stream.write(data);stream.flush();os.fsync(stream.fileno())

def assemble(metadata,lock,identity):
    require(isinstance(metadata,dict) and isinstance(metadata.get('resolve'),dict),'full resolved Cargo metadata required')
    packages=metadata.get('packages');nodes=metadata['resolve'].get('nodes')
    require(isinstance(packages,list) and 1<=len(packages)<=10000 and isinstance(nodes,list),'full resolved Cargo metadata required')
    ids=[p['id'] for p in packages];require(len(ids)==len(set(ids)),'duplicate package ID')
    locked={}
    for p in lock.get('package',[]):
        key=(p['name'],p['version'],p.get('source'))
        require(key not in locked,'duplicate lockfile package');locked[key]=p
    components=[];notices=[];issues=[]
    for p in packages:
        require(isinstance(p['name'],str) and isinstance(p['version'],str),'package fields')
        source=p.get('source');row=locked.get((p['name'],p['version'],source))
        require(row is not None,'metadata package absent from lockfile: '+p['name'])
        properties=[{'name':'worldwright:source','value':source or 'workspace/path'}]
        if source and source.startswith('registry+'):
            require(digest(row.get('checksum')),'registry checksum missing')
            properties.append({'name':'worldwright:registry-archive-sha256','value':row['checksum']})
        declaration=p.get('license')
        kind='application' if any('bin' in t.get('kind',[]) for t in p.get('targets',[])) else 'library'
        component={'type':kind,'bom-ref':p['id'],'name':p['name'],'version':p['version'],'properties':properties}
        if declaration:component['licenses']=[{'expression':declaration}]
        else:issues.append({'package':p['id'],'issue':'license-expression-missing'})
        notice={'package':p['id'],'license_expression':declaration,'license_file_sha256':None,'review_status':'not-adopted'}
        if p.get('license_file'):
            try:
                parent=Path(p['manifest_path']).resolve().parent
                candidate=Path(p['license_file']);candidate=candidate if candidate.is_absolute() else parent/candidate
                require(not candidate.is_symlink() and candidate.resolve().is_relative_to(parent),'license file escapes package')
                data=read(candidate,1024*1024);notice['license_file_sha256']=hash_bytes(data)
            except (OSError,ValueError):issues.append({'package':p['id'],'issue':'declared-license-file-unavailable-or-unsafe'})
        notices.append(notice);components.append(component)
    graph=[];seen=set()
    for node in nodes:
        require(node['id'] in ids and node['id'] not in seen,'resolve node identity');seen.add(node['id'])
        depends=sorted(set(d['pkg'] for d in node.get('deps',[])))
        require(all(p in ids for p in depends),'unresolved dependency reference')
        graph.append({'ref':node['id'],'dependsOn':depends})
    require(seen==set(ids),'incomplete resolved dependency graph')
    bom={'bomFormat':'CycloneDX','specVersion':'1.6','version':1,
        'metadata':{'properties':[{'name':k,'value':v} for k,v in identity.items()]},'components':components,'dependencies':graph}
    return bom,{'schema_version':1,'license_approval':False,'notices':notices,'issues':issues}

def advisory(value,lock_sha,claimed_lock_sha):
    require(digest(claimed_lock_sha) and lock_sha==claimed_lock_sha,'advisory lock binding differs')
    v=value.get('vulnerabilities');require(isinstance(v,dict) and type(v.get('found')) is bool and type(v.get('count')) is int and v['count']>=0 and isinstance(v.get('list'),list),'advisory report shape')
    require(v['count']==len(v['list']) and v['found']==(v['count']>0),'advisory count inconsistency')
    return {'status':'imported-review-required','lock_sha256':lock_sha,'vulnerabilities':v['count'],
        'warnings_present':bool(value.get('warnings')),'independently_authenticated':False,'security_approval':False}

def main():
    p=argparse.ArgumentParser(description=__doc__)
    for flag in ('metadata','lockfile','artifact','output'):p.add_argument('--'+flag,type=Path,required=True)
    for flag in ('commit','source-patch-sha256','container-digest'):p.add_argument('--'+flag,required=True)
    p.add_argument('--advisory-report',type=Path);p.add_argument('--advisory-lock-sha256')
    a=p.parse_args();root=Path(__file__).resolve().parents[1];out=a.output.resolve()
    require(not out.exists() and not out.is_relative_to(root),'new external output required');out.mkdir(mode=0o700)
    try:
        require(re.fullmatch('[0-9a-f]{40}|[0-9a-f]{64}',a.commit) and digest(a.source_patch_sha256) and re.fullmatch('sha256:[0-9a-f]{64}',a.container_digest),'release identity fields')
        lock_bytes=read(a.lockfile);metadata_bytes=read(a.metadata);artifact_bytes=read(a.artifact,34*1024*1024)
        lock_sha=hash_bytes(lock_bytes.replace(b'\r\n',b'\n'))
        identity={'commit':a.commit,'source_patch_sha256':a.source_patch_sha256,'cargo_lock_sha256':lock_sha,
            'artifact_sha256':hash_bytes(artifact_bytes),'container_digest':a.container_digest}
        bom,notices=assemble(json.loads(metadata_bytes,object_pairs_hook=unique),tomllib.loads(lock_bytes.decode()),identity)
        scan={'status':'not-supplied','security_approval':False}
        if a.advisory_report:scan=advisory(load(a.advisory_report),lock_sha,a.advisory_lock_sha256)
        else:require(a.advisory_lock_sha256 is None,'advisory hash without report')
        new(out/'sbom.cdx.json',bom);new(out/'license-inventory.json',notices);new(out/'advisory-review.json',scan)
        new(out/'provenance.json',{'schema_version':1,'kind':'supply-chain-inventory','status':'collected-not-adopted',
            'metadata_sha256':hash_bytes(metadata_bytes),'lockfile_raw_sha256':hash_bytes(lock_bytes),'release':identity,
            'advisory_report_sha256':None if a.advisory_report is None else hash_bytes(read(a.advisory_report)),
            'scope':'Cargo dependencies only; image digest is an input binding, not an OS-package scan',
            'supported_release_authorized':False,'license_approval':False,'security_approval':False})
        print('Supply-chain inventory collected; notices/advisories/owner approval still require review:',out);return 0
    except Exception as error:
        new(out/'failure.json',{'kind':'supply-chain-inventory','status':'failed','failure_type':type(error).__name__,'supported_release_authorized':False})
        print('Supply-chain collection failed; preserve',out);return 1
if __name__=='__main__':raise SystemExit(main())
