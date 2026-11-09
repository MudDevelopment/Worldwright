"""Reject P2 source-contract mutations in bounded disposable copies only.
Python 3.11+ standard library. Never copy target/.git, secrets or the entire repo.
"""
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
FILES = (
    "Cargo.toml", "Cargo.lock", "apps/worldwright/sdk-bundle-Cargo.lock",
    "scripts/test-p2-contracts.py", "crates/worldwright-sdk/src/lib.rs",
    "crates/worldwright-sdk/Cargo.toml", "apps/worldwright/Cargo.toml",
    "apps/worldwright/src/commands/sdk.rs", "apps/worldwright/src/commands/maturity.rs",
    "apps/worldwright/tests/sdk_compatibility.rs",
    "crates/worldwright-content/src/loader.rs", "crates/worldwright-content/src/validation.rs",
    "crates/worldwright-content/src/validated.rs", "crates/worldwright-testkit/src/benchmark.rs",
    "apps/worldwright-load/src/report.rs", "apps/worldwright/tests/fixtures/sdk-api-v1/consumer.rs",
    "apps/worldwright/tests/fixtures/sdk-api-v1/inventory.json",
)
CASES = (
    ("fixture-integrity", "apps/worldwright/tests/fixtures/sdk-api-v1/consumer.rs", "Retained API-v1 consumer", "Changed API-v1 consumer"),
    ("registry-template-drift", "apps/worldwright/src/commands/sdk.rs", 'serde_json = "=1.0.145"', 'serde_json = "=1.0.144"'),
    ("handwritten-CLI-inventory", "apps/worldwright/src/commands/maturity.rs", '"sdk_supported": matrix.sdk_supported', '"sdk_supported": [99]'),
    ("beta-threshold-drift", "apps/worldwright-load/src/report.rs", "750_000", "751_000"),
    ("unrecognized-source-shape", "apps/worldwright/src/commands/sdk.rs", "fn generated_cargo()", "fn changed_cargo()"),
    ("missing-relocation-escape-guard", "apps/worldwright/tests/sdk_compatibility.rs", "manifest.starts_with(&consumer_root)", "manifest.is_absolute()"),
)


def run(root: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run([sys.executable, str(root / "scripts/test-p2-contracts.py")],
                          capture_output=True, text=True, timeout=30)


def main() -> None:
    with tempfile.TemporaryDirectory(prefix="worldwright-p2-negative-") as folder:
        parent = Path(folder)
        baseline = parent / "baseline"
        for relative in FILES:
            source = ROOT / relative
            if source.is_symlink() or not source.is_file() or source.stat().st_size > 2 * 1024 * 1024:
                raise RuntimeError(f"invalid/oversized source fixture: {relative}")
            target = baseline / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
        result = run(baseline)
        if result.returncode != 0:
            raise RuntimeError(f"baseline must pass before negative proof: {result.stderr}")
        for name, relative, old, new in CASES:
            copy = parent / name
            shutil.copytree(baseline, copy)
            target = copy / relative
            text = target.read_text(encoding="utf-8")
            if old not in text:
                raise RuntimeError(f"mutation source shape changed: {name}")
            target.write_text(text.replace(old, new), encoding="utf-8")
            result = run(copy)
            if result.returncode == 0 or not ("FAIL" in result.stderr or "ERROR" in result.stderr):
                raise RuntimeError(f"mutation not rejected: {name}: {result.stderr}")
            print(f"PASS: mutation rejected: {name}")
        print("PASS: 6 negative source-contract checks; only temporary copies were mutated")


if __name__ == "__main__":
    main()
