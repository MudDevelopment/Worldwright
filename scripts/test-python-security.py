"""Patched dependency policy and synthetic JWT/Ed25519 compatibility regressions.
No real users, externally supplied tokens, live endpoints, or release approvals.
"""
import importlib.util
import json
from pathlib import Path
import time
import unittest
from unittest.mock import patch
import jwt
from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric import ed25519, rsa
import python_dependencies as deps


class PythonSecurityTests(unittest.TestCase):
    def test_rust_summary_keeps_failed_and_ignored_counts(self):
        path = Path(__file__).with_name('python-security.py')
        spec = importlib.util.spec_from_file_location('security_summary_test', path)
        runner = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(runner)
        log = (b'test result: ok. 4 passed; 0 failed; 2 ignored; 0 measured;\n'
               b'test result: FAILED. 6 passed; 1 failed; 0 ignored; 0 measured;\n')
        self.assertEqual(runner.rust_summaries(log), [[4, 0, 2], [6, 1, 0]])
        self.assertEqual(runner.rust_summaries(b'compilation failed'), [])

    def test_actual_installed_security_profile_matches_exact_pins(self):
        self.assertEqual(deps.require_installed('security'), deps.expected('security'))

    def test_conflicting_or_unsupported_pins_fail(self):
        for texts in (("cryptography==50.0.2", "cryptography==50.0.1"),
                      ("PyJWT>=2.15.0",), ("-r external.txt",),
                      ("PyJWT==2.15.1rc1",), ("PyJWT==2.14.0",)):
            with self.subTest(texts=texts), self.assertRaises(ValueError):
                deps.merge_pins(texts)

    def test_binary_extra_is_checked_as_a_separate_distribution(self):
        self.assertEqual(deps.merge_pins(['psycopg[binary]==3.3.9']),
                         {'psycopg': '3.3.9', 'psycopg-binary': '3.3.9'})
        with self.assertRaises(ValueError):
            deps.merge_pins(['other[binary]==1.0.0'])

    def test_installed_version_mismatch_does_not_weaken_guard(self):
        with patch.object(deps.importlib.metadata, 'version', return_value='0.0.0'):
            with self.assertRaisesRegex(ValueError, 'mismatch'):
                deps.require_installed('collector')

    def test_rsa_rs256_token_and_public_jwk_shape_remain_compatible(self):
        key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
        claims = {'iss': 'https://fixture.invalid', 'aud': 'fixture', 'sub': 'synthetic',
                  'iat': int(time.time()), 'exp': int(time.time()) + 60, 'nonce': 'fixture-nonce'}
        token = jwt.encode(claims, key, algorithm='RS256')
        self.assertIsInstance(token, str)
        public = json.loads(jwt.algorithms.RSAAlgorithm.to_jwk(key.public_key()))
        self.assertNotIn('d', public)
        decoded = jwt.decode(token, jwt.PyJWK.from_dict(public).key, algorithms=['RS256'],
                             issuer=claims['iss'], audience=claims['aud'])
        self.assertEqual(decoded, claims)

    def test_pyjwk_cannot_override_explicit_algorithm_policy(self):
        key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
        public = json.loads(jwt.algorithms.RSAAlgorithm.to_jwk(key.public_key()))
        public['alg'] = 'RS256'
        token = jwt.encode({'sub': 'synthetic'}, key, algorithm='RS256')
        with self.assertRaises(jwt.InvalidAlgorithmError):
            jwt.decode(token, jwt.PyJWK.from_dict(public), algorithms=['RS512'])

    def test_asymmetric_public_pem_is_not_an_hmac_secret(self):
        key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
        public = key.public_key().public_bytes(serialization.Encoding.PEM,
                                               serialization.PublicFormat.SubjectPublicKeyInfo)
        with self.assertRaises(jwt.InvalidKeyError):
            jwt.encode({'sub': 'synthetic'}, public, algorithm='HS256')

    def test_ed25519_raw_signature_and_tamper_rejection_remain_compatible(self):
        key = ed25519.Ed25519PrivateKey.generate()
        message = b'WORLDWRIGHT-SYNTHETIC-SECURITY-TEST\x00payload'
        signature = key.sign(message)
        public = ed25519.Ed25519PublicKey.from_public_bytes(key.public_key().public_bytes(
            serialization.Encoding.Raw, serialization.PublicFormat.Raw))
        public.verify(signature, message)
        with self.assertRaises(InvalidSignature):
            public.verify(signature, message + b'changed')
        with self.assertRaises(InvalidSignature):
            ed25519.Ed25519PrivateKey.generate().public_key().verify(signature, message)


if __name__ == '__main__':
    unittest.main(verbosity=2)
