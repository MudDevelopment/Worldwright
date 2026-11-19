"""Unit checks for non-authorizing migration review and supply-chain inventory."""
from pathlib import Path
import copy,hashlib,importlib.util,tempfile,unittest
ROOT=Path(__file__).resolve().parents[1]
def load(name):
    spec=importlib.util.spec_from_file_location(name.replace('-','_'),ROOT/'scripts'/(name+'.py'));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module
repair=load('migration-repair');supply=load('supply-chain')
class ReviewTests(unittest.TestCase):
    def test_proposal_does_not_edit_source(self):
        path=ROOT/'migrations'/repair.TARGET;before=path.read_bytes();old,new=repair.proposal()
        self.assertIn(before,(old,new));self.assertNotEqual(new,old);self.assertEqual(new.count(b'"authorization"'),3);self.assertEqual(path.read_bytes(),before)
        self.assertEqual(hashlib.sha384(old).hexdigest(),repair.EXPECTED)
    def test_changed_original_is_not_automatically_repaired(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'migrations';p.mkdir();(p/repair.TARGET).write_bytes((ROOT/'migrations'/repair.TARGET).read_bytes()+b'\n')
            history=Path(d)/'tests/fixtures/migration-history';history.mkdir(parents=True)
            (history/repair.TARGET).write_bytes((ROOT/'tests/fixtures/migration-history'/repair.TARGET).read_bytes())
            with self.assertRaises(ValueError):repair.proposal(Path(d))
    def test_every_history_disposition_requires_review(self):
        self.assertEqual(repair.disposition(None),'history-inventory-required')
        value={'schema_version':1,'kind':'read-only-migration-history','read_only':True,'user_relation_count':0,'migrations':[]}
        self.assertEqual(repair.disposition(value),'pristine-database-owner-review-required')
        value['user_relation_count']=1;self.assertEqual(repair.disposition(value),'nonempty-database-review-required')
        value['migrations']=[{'version':20261106000100,'success':False,'checksum_sha384':'a'*96}]
        self.assertEqual(repair.disposition(value),'partial-history-review-required')
        value['migrations'][0]['success']=True;self.assertEqual(repair.disposition(value),'successful-history-baseline-reconciliation-required')
    def test_malformed_history_fails(self):
        value={'schema_version':True,'kind':'read-only-migration-history','read_only':True,'user_relation_count':0,'migrations':[]}
        with self.assertRaises(ValueError):repair.disposition(value)
        value['schema_version']=1;value['read_only']=False
        with self.assertRaises(ValueError):repair.disposition(value)
    def data(self):
        p={'id':'registry+fixture#pkg@1.0.0','name':'pkg','version':'1.0.0','source':'registry+fixture','license':'MIT','manifest_path':'/unread/manifest/Cargo.toml','license_file':None}
        metadata={'packages':[p],'resolve':{'nodes':[{'id':p['id'],'deps':[]}]}}
        lock={'package':[{'name':'pkg','version':'1.0.0','source':'registry+fixture','checksum':'a'*64}]}
        return metadata,lock
    def test_sbom_binds_graph_archive_and_license_declarations(self):
        m,l=self.data();bom,notices=supply.assemble(m,l,{'commit':'a'*40})
        self.assertEqual(bom['bomFormat'],'CycloneDX');self.assertEqual(len(bom['dependencies']),1)
        self.assertFalse(notices['license_approval']);self.assertEqual(notices['notices'][0]['review_status'],'not-adopted')
        self.assertEqual(bom['components'][0]['licenses'][0]['expression'],'MIT')
    def test_lock_and_graph_mismatch_fail(self):
        m,l=self.data();bad=copy.deepcopy(l);bad['package'][0]['version']='2.0'
        with self.assertRaises(ValueError):supply.assemble(m,bad,{})
        m['resolve']['nodes'][0]['deps']=[{'pkg':'absent'}]
        with self.assertRaises(ValueError):supply.assemble(m,l,{})
    def test_registry_checksum_and_partial_metadata_fail(self):
        m,l=self.data();l['package'][0]['checksum']=None
        with self.assertRaises(ValueError):supply.assemble(m,l,{})
        m['resolve']=None
        with self.assertRaises(ValueError):supply.assemble(m,l,{})
    def test_license_escape_is_reported_not_read(self):
        m,l=self.data()
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);package=root/'pkg';package.mkdir();(root/'private').write_text('not a license')
            m['packages'][0].update(manifest_path=str(package/'Cargo.toml'),license_file='../private')
            _,notices=supply.assemble(m,l,{})
            self.assertIsNone(notices['notices'][0]['license_file_sha256']);self.assertTrue(notices['issues'])
    def test_advisories_cannot_self_approve_or_change_lock(self):
        value={'vulnerabilities':{'found':False,'count':0,'list':[]},'warnings':{}}
        result=supply.advisory(value,'a'*64,'a'*64);self.assertFalse(result['security_approval']);self.assertEqual(result['status'],'imported-review-required')
        with self.assertRaises(ValueError):supply.advisory(value,'a'*64,'b'*64)
        value['vulnerabilities']['count']=True
        with self.assertRaises(ValueError):supply.advisory(value,'a'*64,'a'*64)
    def test_legacy_unsigned_decisions_never_authorize_distribution(self):
        import json,sys
        from unittest.mock import patch
        legacy=load('release-evidence')
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);(root/'Cargo.lock').write_text('version = 4\n');(root/'decisions.json').write_text('{}')
            def command(args):
                if args[:2]==['git','rev-parse']:return b'a'*40
                if args[0]=='git':return b''
                return json.dumps({'packages':[]}).encode()
            for collect in (False,True):
                destination=root/('collect' if collect else 'blocked')
                args=['release-evidence.py','--output',str(destination),'--artifact-digest','a'*64,'--container-digest','sha256:'+'b'*64,'--decisions',str(root/'decisions.json')]
                if collect:args.append('--collect-only')
                with patch.object(legacy,'ROOT',root),patch.object(legacy,'command',command),patch.object(legacy,'check_decisions',return_value=[]),patch.object(sys,'argv',args):
                    self.assertEqual(legacy.main(),0 if collect else 1)
                result=json.loads((destination/'release-evidence.json').read_text())
                self.assertFalse(result['supported_distribution_gate_passed']);self.assertEqual(result['schema_version'],2)
    def test_duplicate_json_is_refused(self):
        with tempfile.TemporaryDirectory() as d:
            path=Path(d)/'x.json';path.write_text('{"a":1,"a":2}')
            with self.assertRaises(ValueError):supply.load(path)
if __name__=='__main__':unittest.main(verbosity=2)
