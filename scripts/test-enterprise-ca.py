"""Six in-memory CA policy regressions; no network, trust-store writes or real keys."""
from datetime import datetime,timedelta,timezone
from pathlib import Path
import importlib.util
import unittest
from cryptography import x509
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.x509.oid import NameOID
import enterprise_ca as policy


class EnterpriseCaTests(unittest.TestCase):
    def setUp(self):
        self.now=datetime.now(timezone.utc)
        self.rootkey=ec.generate_private_key(ec.SECP256R1())
        self.childkey=ec.generate_private_key(ec.SECP256R1())
        self.rootname=x509.Name([x509.NameAttribute(NameOID.COMMON_NAME,'synthetic-root')])
        self.childname=x509.Name([x509.NameAttribute(NameOID.COMMON_NAME,'synthetic-subordinate')])
        self.root=self.cert(self.rootname,self.rootname,self.rootkey,self.rootkey)
        self.child=self.cert(self.childname,self.rootname,self.childkey,self.rootkey)

    def cert(self,subject,issuer,key,signer,ca=True,expired=False):
        return (x509.CertificateBuilder().subject_name(subject).issuer_name(issuer).public_key(key.public_key())
                .serial_number(x509.random_serial_number()).not_valid_before(self.now-timedelta(days=2))
                .not_valid_after(self.now+timedelta(days=-1 if expired else 10))
                .add_extension(x509.BasicConstraints(ca=ca,path_length=3 if ca else None),critical=True)
                .add_extension(x509.KeyUsage(digital_signature=True,content_commitment=False,key_encipherment=False,
                   data_encipherment=False,key_agreement=False,key_cert_sign=ca,crl_sign=ca,
                   encipher_only=False,decipher_only=False),critical=True).sign(signer,hashes.SHA256()))

    def resolve(self,*certs,trusted=None,anchor=None):
        pool={policy.fingerprint(cert):cert for cert in certs}
        return policy.resolve_chain(anchor or policy.fingerprint(self.child),pool,set(pool) if trusted is None else trusted,self.now)

    def test_subordinate_anchor_resolves_to_self_signed_root(self):
        chain=self.resolve(self.child,self.root)
        self.assertEqual([policy.fingerprint(c) for c in chain],[policy.fingerprint(self.child),policy.fingerprint(self.root)])
        self.assertNotEqual(self.child.subject,self.child.issuer)

    def test_missing_or_untrusted_root_does_not_enable_partial_chain(self):
        with self.assertRaises(ValueError):self.resolve(self.child)
        with self.assertRaises(ValueError):self.resolve(self.child,self.root,trusted={policy.fingerprint(self.child)})
        with self.assertRaises(ValueError):self.resolve(self.child,self.root,trusted={policy.fingerprint(self.root)})

    def test_matching_issuer_name_without_valid_signature_is_rejected(self):
        key=ec.generate_private_key(ec.SECP256R1());wrong=self.cert(self.rootname,self.rootname,key,key)
        with self.assertRaises(ValueError):self.resolve(self.child,wrong)

    def test_expired_and_non_ca_parents_are_rejected(self):
        for bad in (self.cert(self.rootname,self.rootname,self.rootkey,self.rootkey,expired=True),
                    self.cert(self.rootname,self.rootname,self.rootkey,self.rootkey,ca=False)):
            with self.subTest(sha256=policy.fingerprint(bad)):
                with self.assertRaises(ValueError):self.resolve(self.child,bad)

    def test_multiple_trusted_complete_paths_require_review(self):
        second=self.cert(self.rootname,self.rootname,self.rootkey,self.rootkey)
        with self.assertRaisesRegex(ValueError,'found 2'):self.resolve(self.child,self.root,second)

    def test_repair_scripts_compile_without_execution(self):
        path=Path(__file__).with_name('repair-wsl-trust.py')
        compile(path.read_bytes(),str(path),'exec')
        spec=importlib.util.spec_from_file_location('trust_repair_test',path)
        module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
        compile(module.INSTALL,'scoped-ca-install','exec')
        compile(module.LINUX_VERIFY,'full-chain-linux-verify','exec')
        compile(Path(__file__).with_name('linux-stage-worker.py').read_bytes(),'linux-stage-worker.py','exec')


if __name__=='__main__':unittest.main(verbosity=2)
