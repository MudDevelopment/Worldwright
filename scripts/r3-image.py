"""Actual digest-pinned image build/push/linkage collector, never staging approval."""
from pathlib import Path
import argparse
import hashlib
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[1]
REF=re.compile(r'^[a-z0-9][a-z0-9._:/-]*@sha256:[0-9a-f]{64}$')

def require(value,message):
    if not value: raise ValueError(message)

def source_identity():
    spec=importlib.util.spec_from_file_location('r3_source',ROOT/'scripts/r0-verify.py')
    module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
    return module.source_identity()

def registry_manifest(raw,digest):
    require(isinstance(digest,str) and re.fullmatch(r'sha256:[0-9a-f]{64}',digest),'manifest digest required, not config ID')
    require(any('sha256:'+hashlib.sha256(candidate).hexdigest()==digest for candidate in (raw,raw.removesuffix(b'\n'))),'registry manifest bytes differ from build metadata digest')
    value=json.loads(raw)
    require(value.get('schemaVersion')==2,'OCI/Docker v2 manifest required')
    accepted={digest}
    for entry in value.get('manifests',[]):
        platform=entry.get('platform',{})
        if platform.get('os')=='linux' and platform.get('architecture')=='amd64':
            require(re.fullmatch(r'sha256:[0-9a-f]{64}',entry.get('digest','')),'invalid platform digest')
            accepted.add(entry['digest'])
    return sorted(accepted)

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--builder-image'); parser.add_argument('--runtime-image'); parser.add_argument('--tag')
    parser.add_argument('--confirm-registry-push',action='store_true')
    args=parser.parse_args(); output=args.output.resolve()
    require(not output.exists() and not output.is_relative_to(ROOT),'new external output directory required')
    output.mkdir(mode=0o700); report={'schema_version':1,'track':'R3','kind':'image-build','status':'failed','full_r3_closed':False,'phases':[]}
    spec=importlib.util.spec_from_file_location('r3_image_io',ROOT/'scripts/r3_io.py')
    io=importlib.util.module_from_spec(spec); spec.loader.exec_module(io)
    sensitive=io.secret_values()
    def run(name,command,timeout=900):
        print('R3 image phase:',name,flush=True)
        result=io.record_phase(report,output,name,command,timeout,cwd=ROOT,sensitive=sensitive)
        require(result['interruption'] is None and result['returncode']==0,'image phase failed/interrupted: '+name)
        return result['stdout']
    try:
        require(shutil.which('docker'),'Docker engine/buildx required; no successful skip')
        require(args.confirm_registry_push,'explicit --confirm-registry-push required')
        require(all(REF.fullmatch(ref or '') for ref in (args.builder_image,args.runtime_image)),'approved digest-pinned --builder-image and --runtime-image required')
        require(re.fullmatch(r'[a-z0-9][a-z0-9._:/-]*:[a-zA-Z0-9_.-]+',args.tag or ''),'explicit registry repository:tag required')
        report['source']=source_identity()
        revision=run('revision',['git','rev-parse','HEAD'],30).decode().strip()
        run('engine',['docker','version'],30); run('buildx',['docker','buildx','version'],30)
        metadata=output/'build-metadata.json'
        run('build',['docker','buildx','build','--platform','linux/amd64','--provenance=mode=max','--push','--metadata-file',str(metadata),'--build-arg','RUST_IMAGE='+args.builder_image,'--build-arg','RUNTIME_IMAGE='+args.runtime_image,'--build-arg','SOURCE_REVISION='+revision,'--build-arg','SOURCE_SHA256='+report['source']['source_sha256'],'--tag',args.tag,'.'])
        build=json.loads(metadata.read_text()); digest=build.get('containerimage.digest')
        repository=args.tag.rsplit(':',1)[0]; reference=repository+'@'+str(digest)
        raw=run('manifest',['docker','buildx','imagetools','inspect',reference,'--raw'],60)
        report['accepted_image_ids']=registry_manifest(raw,digest)
        run('pull',['docker','pull',reference],120)
        image=json.loads(run('inspect',['docker','image','inspect',reference],30))[0]
        require(image['Config']['User']=='65532:65532','runtime UID/GID differs')
        require(image['Config']['Entrypoint']==['/usr/local/bin/worldwright-server'],'runtime entrypoint differs')
        require(image['Config']['Labels']['io.worldwright.source-sha256']==report['source']['source_sha256'],'image source label differs')
        output_version=run('non-root-linkage',['docker','run','--rm','--network','none','--read-only','--cap-drop','ALL','--security-opt','no-new-privileges',reference,'--version'],30)
        require(output_version.strip()==b'worldwright-server 0.1.0-dev.0','binary/version/linkage differs')
        require(source_identity()==report['source'],'source changed during build')
        report.update(status='passed',image_reference=reference,manifest_digest=digest,image_config_id=image['Id'],builder_image=args.builder_image,runtime_image=args.runtime_image,revision=revision,platform='linux/amd64',scope='actual build/push/manifest/linkage; not running-service or cluster acceptance')
        code=0
    except (OSError,ValueError,KeyError,TypeError,subprocess.SubprocessError) as error:
        report['failure']=str(error); code=1
    (output/'image.json').write_text(json.dumps(report,indent=2)+'\n'); print('R3 image:',report['status']); return code
if __name__=='__main__': raise SystemExit(main())
