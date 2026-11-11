"""Explicit Helm values 1 -> 2 migration. Never infers DB/CNI/security authority."""
from pathlib import Path
import argparse
import json
import os
import yaml
import verify

CHART=Path(__file__).resolve().parents[1]

def migrate(old, migration_secret, digest, postgres_cidr, ingress_namespace, ingress_label, dns_namespace, dns_label, https_cidrs):
    if not isinstance(old,dict) or old.get('configVersion',1)!=1:
        raise ValueError('expected old unversioned/configVersion 1 values')
    def selector(namespace,label):
        if not namespace or '=' not in label:
            raise ValueError('explicit namespace and key=value peer label required')
        key,value=label.split('=',1)
        if not key or not value: raise ValueError('nonempty peer key/value required')
        return {'matchLabels':{'kubernetes.io/metadata.name':namespace}}, {'matchLabels':{key:value}}
    ingress_ns,ingress_pod=selector(ingress_namespace,ingress_label)
    dns_ns,dns_pod=selector(dns_namespace,dns_label)
    defaults=yaml.safe_load((CHART/'values.yaml').read_text())
    result=verify.merge(defaults,old)
    result['configVersion']=2
    result['image'].update(digest=digest,tag='',allowMutableTag=False)
    result['postgres'].update(migrationSecret=migration_secret,migrationKey='WORLDWRIGHT_DATABASE_URL')
    result['networkPolicy'].update(enabled=True,namespaceSelector=ingress_ns,podSelector=ingress_pod,postgresCidr=postgres_cidr,dnsNamespaceSelector=dns_ns,dnsPodSelector=dns_pod,httpsEgressCidrs=https_cidrs)
    verify.validate_values(result,CHART)
    return result

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--input',type=Path,required=True); parser.add_argument('--output',type=Path,required=True)
    for option in ('migration-secret','digest','postgres-cidr','ingress-namespace','ingress-label','dns-namespace','dns-label'):
        parser.add_argument('--'+option,required=True)
    parser.add_argument('--https-egress-cidr',action='append',default=[])
    args=parser.parse_args()
    try:
        if args.input.is_symlink() or not args.input.is_file() or args.input.stat().st_size>65536:
            raise ValueError('bounded regular old values file required')
        result=migrate(yaml.safe_load(args.input.read_text()),args.migration_secret,args.digest,args.postgres_cidr,args.ingress_namespace,args.ingress_label,args.dns_namespace,args.dns_label,args.https_egress_cidr)
        with os.fdopen(os.open(args.output,os.O_CREAT|os.O_EXCL|os.O_WRONLY,0o600),'w') as file:
            yaml.safe_dump(result,file,sort_keys=False)
        print('Migrated values format 2. Secret names/CIDRs are inputs, not reviewed grants or CNI enforcement.')
        return 0
    except (OSError,ValueError,AssertionError,verify.ValidationError,yaml.YAMLError) as error:
        print('FAIL:',type(error).__name__,str(error)); return 1
if __name__=='__main__': raise SystemExit(main())
