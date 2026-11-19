"""Provisioner validation tests; mocked network, no WSL/DB operations.
Download fixtures use only new disposable directories in the repository-local temp.
"""
from pathlib import Path
import base64
import hashlib
import importlib.util
import io
import tempfile
import unittest
from unittest.mock import Mock, patch

spec=importlib.util.spec_from_file_location('provision_wsl',Path(__file__).with_name('provision-acceptance-wsl.py'))
p=importlib.util.module_from_spec(spec);spec.loader.exec_module(p)


class ProvisionTests(unittest.TestCase):
    def test_exact_signed_filename(self):
        value=('a'*64+' *'+p.IMAGE+'\n').encode()
        self.assertEqual(p.checksum_entry(value),'a'*64)
        value=('B'*64+'  '+p.IMAGE+'\n').encode()
        self.assertEqual(p.checksum_entry(value),'b'*64)

    def test_missing_duplicate_and_path_substitution_fail(self):
        good=('a'*64+' *'+p.IMAGE+'\n').encode()
        for value in (b'',good+good,('a'*64+' *../'+p.IMAGE+'\n').encode(),b'not-a-hash *'+p.IMAGE.encode()):
            with self.assertRaises(ValueError):p.checksum_entry(value)

    def test_public_key_dearmor_only_decodes_not_authenticates(self):
        value=(b'-----BEGIN PGP PUBLIC KEY BLOCK-----\nVersion: fixture\n\n'+base64.b64encode(b'fixture')+
               b'\n=AAAA\n-----END PGP PUBLIC KEY BLOCK-----\n')
        self.assertEqual(p.dearmor(value),b'fixture')

    def test_non_key_and_invalid_base64_fail(self):
        for value in (b'not a key',b'-----BEGIN PGP PRIVATE KEY BLOCK-----\nx\n-----END PGP PRIVATE KEY BLOCK-----',
                      b'-----BEGIN PGP PUBLIC KEY BLOCK-----\n!\n-----END PGP PUBLIC KEY BLOCK-----'):
            with self.assertRaises(ValueError):p.dearmor(value)

    def status(self,signer=None,primary=None):
        signer=signer or p.FINGERPRINT
        text='[GNUPG:] VALIDSIG '+signer+' 2024-04-25 1714036200 0 4 0 1 10 00'
        if primary:text+=' '+primary
        return (text+'\n').encode()

    def test_validsig_requires_pinned_primary(self):
        self.assertTrue(p.signed_by_ubuntu(self.status()))
        self.assertTrue(p.signed_by_ubuntu(self.status('A'*40,p.FINGERPRINT)))
        self.assertFalse(p.signed_by_ubuntu(self.status('A'*40)))
        self.assertFalse(p.signed_by_ubuntu(self.status(p.FINGERPRINT,'A'*40)))
        self.assertFalse(p.signed_by_ubuntu(b'Good signature from an arbitrary key'))

    def test_bad_revoked_expired_statuses_override_valid_signature(self):
        for status in ('BADSIG','ERRSIG','REVKEYSIG','EXPKEYSIG','EXPSIG','KEYREVOKED','KEYEXPIRED','SIGEXPIRED'):
            self.assertFalse(p.signed_by_ubuntu(self.status()+('[GNUPG:] '+status+' fixture\n').encode()))

    def test_unapproved_redirect_fails(self):
        handler=p.SafeRedirect()
        for url in ('http://cloud-images.ubuntu.com/file','https://example.invalid/file',
                    'https://name:secret@cloud-images.ubuntu.com/file'):
            with self.assertRaises(ValueError):handler.redirect_request(None,None,302,'',{},url)

    def test_response_extent_accepts_exact_ranges_and_full_lengths(self):
        self.assertEqual(p.response_extent(206,{'Content-Range':'bytes 3-5/6','Content-Length':'3'},3,10),(6,6))
        self.assertEqual(p.response_extent(200,{'Content-Length':'6'},0,10),(6,6))
        self.assertEqual(p.response_extent(200,{},0,10),(None,None))

    def test_response_extent_refuses_ignored_wrong_or_encoded_ranges(self):
        cases=[(200,{'Content-Length':'6'},3),
               (206,{'Content-Range':'bytes 2-5/6'},3),
               (206,{'Content-Range':'bytes 3-5/*'},3),
               (206,{'Content-Range':'bytes 3-5/6','Content-Length':'6'},3),
               (206,{'Content-Range':'bytes 3-50/51'},3),
               (206,{'Content-Range':'bytes 3-5/6','Content-Encoding':'gzip'},3),
               (200,{'Content-Length':'0'},0)]
        for status,headers,offset in cases:
            with self.subTest(headers=headers),self.assertRaises(ValueError):p.response_extent(status,headers,offset,10)

    def download_fixture(self,body=b'abc'):
        temp=tempfile.TemporaryDirectory(prefix='wsl-download-unit-',dir=p.local.local_path(p.local.LOCAL/'temp'))
        self.addCleanup(temp.cleanup)
        target=Path(temp.name)/p.IMAGE;seed=target.with_name(target.name+'.partial-'+'a'*32)
        seed.write_bytes(body)
        return target,seed

    def mock_response(self,body):
        response=io.BytesIO(body);response.status=206
        response.headers={'Content-Range':'bytes 3-5/6','Content-Length':'3'}
        return response

    def test_resume_preserves_seed_and_validates_completed_image(self):
        target,seed=self.download_fixture();response=self.mock_response(b'def')
        opener=Mock();opener.open.return_value=response
        with patch.object(p.urllib.request,'build_opener',return_value=opener):
            p.download(p.BASE+p.IMAGE,target,100,hashlib.sha256(b'abcdef').hexdigest())
        self.assertEqual(target.read_bytes(),b'abcdef');self.assertEqual(seed.read_bytes(),b'abc')
        request=opener.open.call_args.args[0]
        self.assertEqual(request.get_header('Range'),'bytes=3-')
        self.assertEqual(opener.open.call_args.kwargs['timeout'],120)

    def test_complete_partial_is_recovered_without_network(self):
        target,seed=self.download_fixture(b'abcdef')
        with patch.object(p.urllib.request,'build_opener') as network:
            p.download(p.BASE+p.IMAGE,target,100,hashlib.sha256(b'abcdef').hexdigest())
        network.assert_not_called();self.assertEqual(target.read_bytes(),b'abcdef')
        self.assertEqual(seed.read_bytes(),b'abcdef')

    def test_corrupt_continuation_never_becomes_import_image(self):
        target,seed=self.download_fixture();opener=Mock();opener.open.return_value=self.mock_response(b'xyz')
        with patch.object(p.urllib.request,'build_opener',return_value=opener),self.assertRaisesRegex(ValueError,'signed checksum'):
            p.download(p.BASE+p.IMAGE,target,100,hashlib.sha256(b'abcdef').hexdigest())
        self.assertFalse(target.exists());self.assertEqual(seed.read_bytes(),b'abc')
        self.assertEqual(sorted(x.stat().st_size for x in target.parent.glob(target.name+'.partial-*')),[3,6])

    def test_timeout_preserves_original_and_new_partial(self):
        target,seed=self.download_fixture();response=self.mock_response(b'')
        response.read=Mock(side_effect=[b'd',TimeoutError('synthetic timeout')])
        opener=Mock();opener.open.return_value=response
        with patch.object(p.urllib.request,'build_opener',return_value=opener),self.assertRaises(TimeoutError):
            p.download(p.BASE+p.IMAGE,target,100,hashlib.sha256(b'abcdef').hexdigest())
        self.assertFalse(target.exists());self.assertEqual(seed.read_bytes(),b'abc')
        self.assertEqual(sorted(x.stat().st_size for x in target.parent.glob(target.name+'.partial-*')),[3,4])

    def test_embedded_scripts_compile_without_execution(self):
        compile(p.BOOTSTRAP,'new-distro-bootstrap','exec')
        compile(p.POSIX_CHECK,'new-distro-posix-check','exec')
        self.assertNotIn(p.DISTRO,('Ubuntu-24.04','Ubuntu-26.04'))


if __name__=='__main__':unittest.main(verbosity=2)
