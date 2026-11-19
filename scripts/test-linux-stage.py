"""Local-only stage guards; no network, tool install, WSL operation or DB access."""
from pathlib import Path
import ast
import hashlib
import re
import importlib.util
import shutil
import tempfile
import unittest
import fresh_baseline as baseline
import worldwright_local as local


class StageTests(unittest.TestCase):
    def fixture(self):
        temp=tempfile.TemporaryDirectory(prefix='fresh-baseline-unit-',dir=local.local_path(local.LOCAL/'temp'))
        self.addCleanup(temp.cleanup);root=Path(temp.name)
        (root/'migrations').mkdir();history=root/'tests/fixtures/migration-history';history.mkdir(parents=True)
        old,fixed=baseline.baseline_bytes(local.ROOT)
        (history/baseline.TARGET).write_bytes(old)
        for name in baseline.MIGRATIONS:shutil.copyfile(local.ROOT/'migrations'/name,root/'migrations'/name)
        (root/'migrations'/baseline.TARGET).write_bytes(old)
        out=root/'evidence';out.mkdir()
        return root,out,old,fixed

    def test_preserved_history_and_exact_three_site_correction(self):
        old,fixed=baseline.baseline_bytes(local.ROOT)
        self.assertEqual(hashlib.sha384(old).hexdigest(),baseline.ORIGINAL_SHA384)
        self.assertEqual(fixed.count(b'"authorization"'),3)
        self.assertEqual(fixed.replace(b'"authorization"',b'authorization'),old)
        self.assertGreater(old.count(b'authorization'),3)
        for name in (b'authorization_version',b'worldwright_operator_audit_authorization_valid',
                     b'worldwright_publication_authorization_version_valid',b'worldwright_publication_authorization_shape'):
            with self.subTest(identifier=name):
                self.assertGreater(old.count(name),0)
                self.assertEqual(fixed.count(name),old.count(name))
        self.assertNotIn(b'"authorization"_version',fixed)
        # Independent expected patch: only the three reviewed SQL expression sites.
        expected=old
        for original,replacement in (
            (b'    authorization JSONB NOT NULL,',b'    "authorization" JSONB NOT NULL,'),
            (b'jsonb_typeof(authorization)',b'jsonb_typeof("authorization")'),
            (b'octet_length(authorization::TEXT)',b'octet_length("authorization"::TEXT)')):
            self.assertEqual(expected.count(original),1)
            expected=expected.replace(original,replacement)
        self.assertEqual(fixed,expected)

    def test_authorization_required_without_source_changes(self):
        root,out,old,_=self.fixture()
        with self.assertRaises(ValueError):baseline.apply_fresh(root,out)
        self.assertEqual((root/'migrations'/baseline.TARGET).read_bytes(),old)
        self.assertEqual(list(out.iterdir()),[])

    def test_apply_is_idempotent_and_other_migrations_unchanged(self):
        root,out,old,fixed=self.fixture();before=baseline.inventory(root)
        result=baseline.apply_fresh(root,out,authorized=True)
        self.assertTrue(result['changed']);self.assertEqual((out/('before-'+baseline.TARGET)).read_bytes(),old)
        self.assertEqual((root/'migrations'/baseline.TARGET).read_bytes(),fixed)
        after=baseline.inventory(root)
        self.assertEqual({k:v for k,v in before.items() if k!=baseline.TARGET},{k:v for k,v in after.items() if k!=baseline.TARGET})
        self.assertFalse(baseline.apply_fresh(root,out,authorized=True)['changed'])

    def test_changed_current_or_original_refused(self):
        root,out,old,_=self.fixture();target=root/'migrations'/baseline.TARGET
        target.write_bytes(old+b'\n')
        with self.assertRaises(ValueError):baseline.apply_fresh(root,out,authorized=True)
        target.write_bytes(old);(root/'tests/fixtures/migration-history'/baseline.TARGET).write_bytes(old+b'\n')
        with self.assertRaises(ValueError):baseline.apply_fresh(root,out,authorized=True)

    def test_extra_migration_is_not_silently_accepted(self):
        root,out,_,_=self.fixture();(root/'migrations/99999999999999_extra.sql').write_bytes(b'SELECT 1;\n')
        with self.assertRaises(ValueError):baseline.apply_fresh(root,out,authorized=True)

    def test_stage_scripts_compile_and_sql_log_projection_is_exact(self):
        for name in ('linux-next-stage.py','linux-stage-worker.py'):
            compile((local.ROOT/'scripts'/name).read_bytes(),name,'exec')
        spec=importlib.util.spec_from_file_location('linux_stage_test',local.ROOT/'scripts/linux-next-stage.py')
        module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
        compile(module.SNAPSHOT,'snapshot-bootstrap','exec')
        compile(module.EXPORT,'evidence-export','exec')
        self.assertEqual(module.projected('migration.sql',b'a\r\n'),b'a\r\n')
        self.assertEqual(module.projected('retained.log',b'a\r\n'),b'a\r\n')
        self.assertEqual(module.projected('start.sh',b'a\r\n'),b'a\n')
        # Psycopg parses literal percent characters whenever parameters are
        # supplied. Catch accidental LIKE 'prefix%' mixed with %s before WSL.
        tree=ast.parse((local.ROOT/'scripts/linux-stage-worker.py').read_bytes())
        parameterized=[]
        for node in ast.walk(tree):
            if (isinstance(node,ast.Call) and isinstance(node.func,ast.Attribute)
                    and node.func.attr=='execute' and len(node.args)>=2
                    and isinstance(node.args[0],ast.Constant)
                    and isinstance(node.args[0].value,str)):
                query=node.args[0].value;parameterized.append(query)
                self.assertNotIn('%',re.sub(r'%%|%[sbt]','',query),query)
        self.assertIn('SELECT datname FROM pg_database WHERE datname LIKE %s AND datname<>%s',parameterized)


if __name__=='__main__':unittest.main(verbosity=2)
