#!/usr/bin/env python3
"""Collect private release evidence; never grant approval or sign an artifact.
Python 3.10+, standard library only. Missing/failed tools fail, not skip.
"""
import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ROLES = ("legal", "security", "engineering", "service")
GATES = ("native-ubuntu", "native-windows", "database-identity", "database-recovery",
         "browser-oidc", "preview-tree", "beta250-load", "container", "notices", "advisories", "signing-policy")

def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()

def read_bounded(path: Path) -> bytes:
    if path.is_symlink() or not path.is_file() or path.stat().st_size > 10 * 1024 * 1024:
        raise ValueError("evidence must be a regular file of at most 10 MiB")
    return path.read_bytes()

def command(args: list[str]) -> bytes:
    result = subprocess.run(args, cwd=ROOT, capture_output=True, timeout=120, check=False)
    if result.returncode:
        raise ValueError(f"required tool failed: {args[0]} (exit {result.returncode})")
    return result.stdout

def check_decisions(decisions: dict, release: dict, base: Path) -> list[str]:
    blockers = []
    if decisions.get("schema_version") != 1 or decisions.get("release") != release:
        return ["approval/evidence release tuple does not match"]
    for role in ROLES:
        record = decisions.get("owners", {}).get(role, {})
        if record.get("decision") != "approved" or not record.get("name") or not record.get("approval_reference"):
            blockers.append(f"missing named {role} approval/reference")
    for gate in GATES:
        record = decisions.get("evidence", {}).get(gate, {})
        try:
            if record.get("result") != "passed":
                raise ValueError("no passing result")
            path = Path(record["path"])
            if path.is_absolute() or ".." in path.parts:
                raise ValueError("evidence paths must be relative and contained")
            path = base / path
            if not path.resolve().is_relative_to(base.resolve()):
                raise ValueError("evidence outside approval directory")
            if digest(read_bounded(path)) != record.get("sha256"):
                raise ValueError("evidence hash mismatch")
        except (ValueError, KeyError, OSError):
            blockers.append(f"missing/invalid evidence: {gate}")
    return blockers

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--artifact-digest", required=True)
    parser.add_argument("--container-digest", required=True)
    parser.add_argument("--decisions", type=Path)
    args = parser.parse_args()
    if not re.fullmatch(r"[0-9a-f]{64}", args.artifact_digest) or not re.fullmatch(r"sha256:[0-9a-f]{64}", args.container_digest):
        parser.error("exact lowercase artifact and container digests are required")
    args.output.mkdir(mode=0o700, parents=False, exist_ok=False)
    try:
        lock = read_bounded(ROOT / "Cargo.lock")
        revision = command(["git", "rev-parse", "HEAD"]).decode().strip()
        patch = command(["git", "diff", "--binary", "HEAD"])
        untracked = command(["git", "ls-files", "--others", "--exclude-standard"]).decode().splitlines()
        metadata = json.loads(command(["cargo", "metadata", "--format-version", "1", "--locked", "--offline"]))
        release = {"commit": revision, "source_patch_sha256": digest(patch), "cargo_lock_sha256": digest(lock),
                   "artifact_sha256": args.artifact_digest, "container_digest": args.container_digest}
        components = []
        for package in metadata["packages"]:
            components.append({"type": "library", "bom-ref": package["id"], "name": package["name"],
                               "version": package["version"], "properties": [
                                   {"name": "worldwright:declared-license", "value": package.get("license") or "NOASSERTION"},
                                   {"name": "worldwright:source", "value": package.get("source") or "private-workspace"}]})
        sbom = {"bomFormat": "CycloneDX", "specVersion": "1.6", "version": 1, "components": components,
                "metadata": {"properties": [{"name": key, "value": value} for key, value in release.items()]}}
        (args.output / "sbom.json").write_text(json.dumps(sbom, indent=2) + "\n")
        blockers = ["named decisions/evidence not provided"]
        if args.decisions:
            blockers = check_decisions(json.loads(read_bounded(args.decisions)), release, args.decisions.parent)
        if patch or untracked:
            blockers.append("source tree is not clean; commit reviewed changes before supported distribution")
        report = {"schema_version": 1, "release": release, "sbom_sha256": digest(read_bounded(args.output / "sbom.json")),
                  "blockers": blockers, "supported_distribution_gate_passed": not blockers,
                  "limitations": ["Evidence hashes prove integrity, not authorship or authorized approval.",
                                  "SBOM contains Cargo metadata declarations, not a legal license grant.",
                                  "Advisory, signing, live acceptance and owner evidence must be independently reviewed."]}
        (args.output / "release-evidence.json").write_text(json.dumps(report, indent=2) + "\n")
        print("Release evidence collected; supported-distribution gate:", "PASS" if not blockers else "BLOCKED")
        return int(bool(blockers))
    except (ValueError, OSError, subprocess.SubprocessError, KeyError) as error:
        (args.output / "failure.json").write_text(json.dumps({"error": str(error), "gate_passed": False}) + "\n")
        print("Release evidence failed; no supported-distribution approval.")
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
