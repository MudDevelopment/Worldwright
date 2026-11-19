"""Pure helper regressions: no WSL launch, database connection or filesystem writes."""
from pathlib import Path
import copy
import importlib.util
import unittest

spec=importlib.util.spec_from_file_location('wsl_preflight',Path(__file__).with_name('wsl-acceptance-preflight.py'))
wsl=importlib.util.module_from_spec(spec)
spec.loader.exec_module(wsl)


class PreflightTests(unittest.TestCase):
    def observation(self):
        return {'os_release':{'ID':'ubuntu','VERSION_ID':'24.04'},'uid':1000,
                'machine':'x86_64','posix_probe':{'passed':True},
                'project_mount':{'filesystem':'ext4','drvfs_detected':False}}

    def test_wsl_utf16_with_and_without_bom(self):
        text='Ubuntu-24.04\r\n'
        self.assertEqual(wsl.decode_wsl(text.encode('utf-16')),'Ubuntu-24.04')
        self.assertEqual(wsl.decode_wsl(text.encode('utf-16-le')),'Ubuntu-24.04')

    def test_wsl_utf8(self):
        self.assertEqual(wsl.decode_wsl(b'Ubuntu\n'),'Ubuntu')
        self.assertEqual(wsl.decode_wsl('Ubuntu\n'.encode('utf-8-sig')),'Ubuntu')

    def test_distro_selection_is_exact_and_unambiguous(self):
        self.assertEqual(wsl.choose_distro(['docker-desktop','Ubuntu-24.04']),'Ubuntu-24.04')
        self.assertEqual(wsl.choose_distro(['WorkLinux'],'WorkLinux'),'WorkLinux')
        for names,requested in ((['Ubuntu','Ubuntu-24.04'],None),(['docker-desktop'],None),(['Ubuntu'],'missing')):
            with self.assertRaises(ValueError):wsl.choose_distro(names,requested)

    def test_observation_does_not_require_changes_to_classify(self):
        value=self.observation();before=copy.deepcopy(value)
        self.assertEqual(wsl.classify(value,{'version':2}),[])
        self.assertEqual(value,before)
        self.assertNotIn('acceptance_passed',value)

    def test_wrong_os_root_unknown_mount_and_wsl1_are_blocked(self):
        value=self.observation();value.update(uid=0,project_mount=None)
        value['os_release']['VERSION_ID']='22.04'
        blockers=wsl.classify(value,{'version':1})
        for blocker in ('Ubuntu-24.04-required','non-root-Linux-user-required',
                        'project-mount-not-identified','registered-distro-is-not-verified-WSL2'):
            self.assertIn(blocker,blockers)

    def test_drvfs_and_failed_permissions_are_not_accepted(self):
        value=self.observation();value['project_mount']['drvfs_detected']=True
        value['posix_probe']['passed']=False
        blockers=wsl.classify(value,{'version':2})
        self.assertIn('POSIX-permission-link-probe-failed',blockers)
        self.assertIn('DrvFS-requires-review-before-native-filesystem-acceptance',blockers)

    def test_embedded_linux_probe_compiles_without_executing(self):
        compile(wsl.LINUX_PROBE,'wsl-read-only-inventory','exec')


if __name__=='__main__':unittest.main(verbosity=2)
