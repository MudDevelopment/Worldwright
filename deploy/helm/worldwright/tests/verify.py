"""Required Helm acceptance: schema matrix, native exits, actual YAML resources.

Python 3.10+; install the test-only requirements.txt into a venv. --schema-only
is an explicitly limited local check, never used by the required render gates.
"""
from __future__ import annotations
import argparse
import copy
import json
import shutil
import subprocess
import sys
from pathlib import Path
import yaml
from jsonschema import Draft7Validator

OIDC = {
    "oidc.enabled": True,
    "oidc.issuer": "https://issuer.example.invalid",
    "oidc.clientId": "acceptance-client",
    "oidc.redirectUri": "https://world.example.invalid/api/v1/oidc/callback",
    "oidc.existingSecret": "world-oidc",
    "oidc.key": "WORLDWRIGHT_OIDC_CLIENT_SECRET",
}

def merge(base: dict, patch: dict) -> dict:
    result = copy.deepcopy(base)
    for key, value in patch.items():
        if isinstance(value, dict) and isinstance(result.get(key), dict):
            result[key] = merge(result[key], value)
        else:
            result[key] = copy.deepcopy(value)
    return result

def apply(values: dict, overrides: dict) -> dict:
    result = copy.deepcopy(values)
    for dotted, value in overrides.items():
        node = result
        parts = dotted.split(".")
        for part in parts[:-1]:
            node = node.setdefault(part, {})
        node[parts[-1]] = value
    return result

def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)

def schema_matrix(chart: Path) -> int:
    schema = json.loads((chart / "values.schema.json").read_text())
    Draft7Validator.check_schema(schema)
    validator = Draft7Validator(schema)
    defaults = yaml.safe_load((chart / "values.yaml").read_text())
    acceptance = yaml.safe_load((chart / "tests/acceptance-values.yaml").read_text())
    values = merge(defaults, acceptance)
    count = 0
    def check(value: dict, expected: bool, label: str) -> None:
        nonlocal count
        errors = list(validator.iter_errors(value))
        require(bool(errors) != expected, f"schema {label}: expected valid={expected}; {[e.message for e in errors]}")
        count += 1
    check(defaults, False, "unconfigured deployment still fails closed")
    for game in ("reference", "expedition"):
        check(apply(values, {"game": game}), True, f"{game}/OIDC disabled")
        check(apply(values, {**OIDC, "game": game}), True, f"{game}/OIDC enabled")
    absent = copy.deepcopy(values)
    absent.pop("oidc")
    check(absent, True, "OIDC omitted does not imply enabled")
    missing_flag = copy.deepcopy(values)
    missing_flag["oidc"].pop("enabled")
    check(missing_flag, True, "OIDC flag omitted")
    check(apply(values, {"oidc.enabled": True}), False, "partial enabled OIDC")
    for key in ("issuer", "clientId", "redirectUri", "existingSecret", "key"):
        enabled = apply(values, OIDC)
        enabled["oidc"][key] = ""
        check(enabled, False, f"empty enabled {key}")
        enabled = apply(values, OIDC)
        enabled["oidc"].pop(key)
        check(enabled, False, f"missing enabled {key}")
    for overrides, label in [
        ({**OIDC, "oidc.issuer": "http://issuer.example.invalid"}, "HTTP issuer"),
        ({**OIDC, "oidc.redirectUri": "https://world.example.invalid/api/v1/oidc/callback?x=1"}, "callback query"),
        ({"oidc.enabled": "true"}, "non-boolean enabled"),
        ({"oidc.clientSecret": "client-secret-value"}, "plaintext secret field"),
        ({"image.digest": ""}, "mutable image without approval"),
        ({"ingress.enabled": False}, "disabled ingress"),
        ({"postgres.existingSecret": ""}, "missing PostgreSQL reference"),
        ({"game": "unknown"}, "unknown game"),
    ]:
        check(apply(values, overrides), False, label)
    check(apply(values, {"image.digest": "", "image.allowMutableTag": True, "image.tag": "acceptance"}), True, "explicit mutable image opt-in")
    print(f"PASS: {count} JSON Schema cases (not Helm rendering)")
    return count

def helm_command(helm: str, chart: Path, command: str, overrides: dict | None = None) -> list[str]:
    args = [helm, command]
    if command == "template":
        args.append("worldwright")
    args.extend([str(chart), "--values", str(chart / "tests/acceptance-values.yaml")])
    if command == "lint":
        args.append("--strict")
    for key, value in (overrides or {}).items():
        if isinstance(value, bool):
            args.extend(["--set", f"{key}={str(value).lower()}"])
        else:
            args.extend(["--set-string", f"{key}={value}"])
    return args

def invoke(args: list[str], failure: str | None = None) -> str:
    result = subprocess.run(args, capture_output=True, text=True, encoding="utf-8", timeout=60, check=False)
    if failure is not None:
        require(result.returncode != 0, f"negative Helm case incorrectly succeeded: {failure}")
        text = result.stdout + result.stderr
        require(failure.lower() in text.lower(), f"negative case failed for an unrelated reason, expected {failure}: {text[:1500]}")
        return ""
    require(result.returncode == 0, f"Helm command failed ({result.returncode}): {result.stderr[:2000]}")
    require(len(result.stdout.encode()) <= 1024 * 1024, "rendered output exceeds acceptance bound")
    return result.stdout

def rendered_resources(text: str, game: str, oidc: bool) -> None:
    docs = [doc for doc in yaml.safe_load_all(text) if isinstance(doc, dict) and doc.get("kind")]
    keys = [(doc["kind"], doc.get("metadata", {}).get("name")) for doc in docs]
    require(len(keys) == len(set(keys)), "duplicate rendered resource identities")
    def one(kind: str) -> dict:
        found = [doc for doc in docs if doc["kind"] == kind]
        require(len(found) == 1, f"expected one {kind}")
        return found[0]
    require(not any(doc["kind"] == "Secret" for doc in docs), "chart must reference external secrets, not render credentials")
    deployment = one("Deployment")
    require(deployment["spec"]["replicas"] == 1, "single writer replica")
    require(deployment["spec"]["strategy"]["type"] == "Recreate", "Recreate strategy")
    pod = deployment["spec"]["template"]["spec"]
    require(pod["automountServiceAccountToken"] is False, "service account token disabled")
    require(pod["securityContext"]["runAsNonRoot"] is True, "non-root pod")
    require(pod["securityContext"]["seccompProfile"]["type"] == "RuntimeDefault", "seccomp")
    require(pod["terminationGracePeriodSeconds"] == 60, "termination bound")
    require(len(pod["containers"]) == 1 and len(pod["initContainers"]) == 1, "one server and migration init")
    server = pod["containers"][0]
    migration = pod["initContainers"][0]
    for container in (server, migration):
        require(container["image"].endswith("@sha256:" + "a" * 64), "digest-pinned image")
        secure = container["securityContext"]
        require(secure["allowPrivilegeEscalation"] is False, "no privilege escalation")
        require(secure["readOnlyRootFilesystem"] is True, "read-only root filesystem")
        require(secure["capabilities"]["drop"] == ["ALL"], "drop all capabilities")
        refs = {item["name"]: item.get("valueFrom", {}).get("secretKeyRef") for item in container["env"]}
        require(refs["WORLDWRIGHT_DATABASE_URL"] == {"name": "world-postgres", "key": "WORLDWRIGHT_DATABASE_URL"}, "Postgres secretKeyRef")
        require(all("value" not in item for item in container["env"]), "no plaintext environment credentials")
    args = server["args"]
    for required in ("serve", "--acquire-writer", "--behind-tls-ingress"):
        require(required in args, f"required public flag {required}")
    require(args[args.index("--game") + 1] == game, "game flag matches artifact profile")
    require(args[args.index("--origin") + 1] == "https://world.example.invalid", "exact origin")
    require("--allow-insecure-http" not in args and not any(arg.startswith("--operator-") for arg in args), "forbidden public flags")
    refs = {item["name"]: item.get("valueFrom", {}).get("secretKeyRef") for item in server["env"]}
    if oidc:
        require(args[args.index("--oidc-issuer") + 1] == OIDC["oidc.issuer"], "OIDC issuer")
        require(args[args.index("--oidc-redirect-uri") + 1] == OIDC["oidc.redirectUri"], "OIDC callback")
        require(refs["WORLDWRIGHT_OIDC_CLIENT_SECRET"] == {"name": "world-oidc", "key": "WORLDWRIGHT_OIDC_CLIENT_SECRET"}, "OIDC secret reference")
    else:
        require(not any(arg.startswith("--oidc-") for arg in args), "disabled OIDC flags absent")
        require("WORLDWRIGHT_OIDC_CLIENT_SECRET" not in refs, "disabled OIDC secret absent")
    require(server["resources"]["requests"] and server["resources"]["limits"], "resources")
    require(server["startupProbe"]["httpGet"]["path"] == "/health/live", "startup probe")
    require(server["readinessProbe"]["httpGet"]["path"] == "/health/ready", "readiness probe")
    ingress = one("Ingress")
    require(ingress["spec"]["tls"] == [{"hosts": ["world.example.invalid"], "secretName": "world-tls"}], "actual Ingress spec.tls.secretName")
    require(ingress["spec"]["rules"][0]["host"] == "world.example.invalid", "ingress host")
    require(one("Service")["spec"]["type"] == "ClusterIP", "private service")
    require(one("ConfigMap")["data"]["GAME_PROFILE"] == game, "config map profile")
    one("ServiceAccount")
    require("super-secret" not in text and "client-secret-value" not in text, "secret sentinel leakage")

def helm_matrix(chart: Path) -> None:
    helm = shutil.which("helm")
    require(helm is not None, "Helm is required; missing tool is FAILURE, not SKIP")
    version = invoke([helm, "version", "--short"])
    require(version.startswith("v3."), f"acceptance requires Helm 3; found {version.strip()}")
    for game in ("reference", "expedition"):
        for oidc in (False, True):
            overrides = {"game": game, **(OIDC if oidc else {"oidc.enabled": False})}
            invoke(helm_command(helm, chart, "lint", overrides))
            rendered_resources(invoke(helm_command(helm, chart, "template", overrides)), game, oidc)
            print(f"PASS: actual Helm lint/YAML {game}/OIDC={oidc}")
    negative = [
        ({"image.digest": ""}, "image"),
        ({"image.digest": "sha256:" + "A" * 64}, "image"),
        ({"publicOrigin": "http://world.example.invalid"}, "publicOrigin"),
        ({"publicOrigin": "https://wrong.example.invalid"}, "publicOrigin"),
        ({"ingress.enabled": False}, "ingress"),
        ({"ingress.tlsSecretName": ""}, "tlsSecretName"),
        ({"postgres.existingSecret": ""}, "postgres"),
        ({"game": "unknown"}, "game"),
        ({"oidc.enabled": True}, "oidc"),
        ({**OIDC, "oidc.redirectUri": "https://wrong.example.invalid/api/v1/oidc/callback"}, "redirectUri"),
        ({"oidc.clientSecret": "client-secret-value"}, "oidc"),
    ]
    negative += [({**OIDC, f"oidc.{key}": ""}, "oidc") for key in ("issuer", "clientId", "redirectUri", "existingSecret", "key")]
    for overrides, reason in negative:
        invoke(helm_command(helm, chart, "template", overrides), failure=reason)
    print(f"PASS: {len(negative)} actual Helm negative cases")
    print("PASS: required Helm acceptance (not cluster installation or live OIDC)")

def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--chart", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--schema-only", action="store_true")
    args = parser.parse_args()
    try:
        schema_matrix(args.chart.resolve())
        if not args.schema_only:
            helm_matrix(args.chart.resolve())
        return 0
    except (AssertionError, OSError, ValueError, KeyError, TypeError, IndexError, subprocess.SubprocessError, yaml.YAMLError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
