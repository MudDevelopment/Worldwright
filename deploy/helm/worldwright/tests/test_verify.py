"""Checker unit tests using synthetic YAML/subprocess results, NOT Helm proof."""
import copy
import subprocess
import unittest
from unittest.mock import patch
import yaml
import verify


def resources(game="reference", oidc=False):
    image = "registry.example.invalid/worldwright@sha256:" + "a" * 64
    security = {"allowPrivilegeEscalation": False, "readOnlyRootFilesystem": True, "capabilities": {"drop": ["ALL"]}}
    database = {"name": "WORLDWRIGHT_DATABASE_URL", "valueFrom": {"secretKeyRef": {"name": "world-postgres", "key": "WORLDWRIGHT_DATABASE_URL"}}}
    migration = {"image": image, "env": [copy.deepcopy(database)], "securityContext": copy.deepcopy(security)}
    args = ["serve", "--acquire-writer", "--behind-tls-ingress", "--game", game, "--origin", "https://world.example.invalid"]
    env = [copy.deepcopy(database)]
    if oidc:
        args += ["--oidc-issuer", verify.OIDC["oidc.issuer"], "--oidc-redirect-uri", verify.OIDC["oidc.redirectUri"]]
        env.append({"name": "WORLDWRIGHT_OIDC_CLIENT_SECRET", "valueFrom": {"secretKeyRef": {"name": "world-oidc", "key": "WORLDWRIGHT_OIDC_CLIENT_SECRET"}}})
    server = {"image": image, "env": env, "args": args, "securityContext": copy.deepcopy(security),
              "resources": {"requests": {"cpu": "500m"}, "limits": {"cpu": "2"}},
              "startupProbe": {"httpGet": {"path": "/health/live"}}, "readinessProbe": {"httpGet": {"path": "/health/ready"}}}
    return [
        {"kind": "Deployment", "metadata": {"name": "test"}, "spec": {"replicas": 1, "strategy": {"type": "Recreate"},
            "template": {"spec": {"automountServiceAccountToken": False, "securityContext": {"runAsNonRoot": True, "seccompProfile": {"type": "RuntimeDefault"}},
            "terminationGracePeriodSeconds": 60, "containers": [server], "initContainers": [migration]}}}},
        {"kind": "Ingress", "metadata": {"name": "test"}, "spec": {"tls": [{"hosts": ["world.example.invalid"], "secretName": "world-tls"}], "rules": [{"host": "world.example.invalid"}]}},
        {"kind": "Service", "metadata": {"name": "test"}, "spec": {"type": "ClusterIP"}},
        {"kind": "ConfigMap", "metadata": {"name": "test"}, "data": {"GAME_PROFILE": game}},
        {"kind": "ServiceAccount", "metadata": {"name": "test"}},
    ]

class VerifyTests(unittest.TestCase):
    def test_synthetic_valid_profile_structures(self):
        for game in ("reference", "expedition"):
            for oidc in (False, True):
                verify.rendered_resources(yaml.safe_dump_all(resources(game, oidc)), game, oidc)
    def test_actual_tls_key_required_not_template_input_key(self):
        docs = resources()
        docs[1]["spec"]["tls"][0]["tlsSecretName"] = docs[1]["spec"]["tls"][0].pop("secretName")
        with self.assertRaises(AssertionError):
            verify.rendered_resources(yaml.safe_dump_all(docs), "reference", False)
    def test_secret_values_operator_flags_and_duplicate_resources_rejected(self):
        for mutation in ("value", "operator", "duplicate", "secret"):
            docs = resources()
            server = docs[0]["spec"]["template"]["spec"]["containers"][0]
            if mutation == "value": server["env"][0]["value"] = "super-secret"
            if mutation == "operator": server["args"].append("--operator-signing-key-env")
            if mutation == "duplicate": docs.append(copy.deepcopy(docs[1]))
            if mutation == "secret": docs.append({"kind": "Secret", "metadata": {"name": "leak"}})
            with self.assertRaises(AssertionError):
                verify.rendered_resources(yaml.safe_dump_all(docs), "reference", False)
    def test_positive_and_negative_native_exits_are_checked(self):
        result = subprocess.CompletedProcess(["helm"], 7, "", "oidc invalid")
        with patch.object(verify.subprocess, "run", return_value=result):
            with self.assertRaises(AssertionError): verify.invoke(["helm"])
            verify.invoke(["helm"], failure="oidc")
            with self.assertRaises(AssertionError): verify.invoke(["helm"], failure="publicOrigin")
        result = subprocess.CompletedProcess(["helm"], 0, "", "")
        with patch.object(verify.subprocess, "run", return_value=result):
            with self.assertRaises(AssertionError): verify.invoke(["helm"], failure="oidc")
    def test_missing_helm_is_failure_not_skip(self):
        with patch.object(verify.shutil, "which", return_value=None):
            with self.assertRaisesRegex(AssertionError, "Helm is required"):
                verify.helm_matrix(verify.Path("."))

if __name__ == "__main__":
    unittest.main()
