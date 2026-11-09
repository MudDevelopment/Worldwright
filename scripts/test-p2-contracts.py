"""P2 source/TOML/fixture drift checks, not Rust compilation or load evidence.
Python 3.11+; standard library only. Source parsing fails on unfamiliar shapes.
"""
from pathlib import Path
import hashlib
import json
import re
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def capture(pattern: str, source: str) -> str:
    match = re.search(pattern, source, re.S)
    if match is None:
        raise AssertionError(f"source shape changed; review drift guard: {pattern}")
    return match.group(1)


class P2Contracts(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.workspace = tomllib.loads(read("Cargo.toml"))
        cls.sdk = read("crates/worldwright-sdk/src/lib.rs")
        cls.bundle = read("apps/worldwright/src/commands/sdk.rs")
        cls.crates = re.findall(r'"([^"]+)"', capture(r'const CRATES:.*?= &\[(.*?)\];', cls.bundle))
        cls.minimum = capture(r'MINIMUM_RUST_VERSION: &str = "([^"]+)";', cls.sdk)
        cls.version = cls.workspace["workspace"]["package"]["version"]
        template = capture(r'fn generated_cargo\(\).*?r#"(.*?)"#', cls.bundle)
        template = template.replace("@PACKAGE_VERSION@", cls.version).replace("@MIN_RUST@", cls.minimum)
        template = template.replace("@MEMBERS@", ",".join(json.dumps(f"crates/{name}") for name in cls.crates))
        cls.generated = tomllib.loads(template)

    def test_generated_package_members_and_private_policy(self) -> None:
        workspace = self.generated["workspace"]
        self.assertEqual(workspace["members"], [f"crates/{name}" for name in self.crates])
        self.assertEqual(workspace["package"]["version"], self.version)
        self.assertEqual(workspace["package"]["rust-version"], self.minimum)
        self.assertFalse(workspace["package"]["publish"])
        for name in self.crates:
            self.assertEqual(workspace["dependencies"][name], {"path": f"crates/{name}", "version": f"={self.version}"})

    def test_registry_template_matches_root_and_locked_registry_provenance(self) -> None:
        for name, spec in self.generated["workspace"]["dependencies"].items():
            if name not in self.crates:
                self.assertEqual(spec, self.workspace["workspace"]["dependencies"][name], name)
        reduced = tomllib.loads(read("apps/worldwright/sdk-bundle-Cargo.lock"))["package"]
        source = tomllib.loads(read("Cargo.lock"))["package"]
        source_by_key = {(package["name"], package["version"]): package for package in source}
        for package in reduced:
            if "source" in package:
                actual = source_by_key[(package["name"], package["version"])]
                self.assertEqual(package["source"], actual["source"])
                self.assertEqual(package["checksum"], actual["checksum"])
            else:
                self.assertIn(package["name"], self.crates)
                self.assertEqual(package["version"], self.version)

    def test_metadata_wires_to_authoritative_values(self) -> None:
        maturity = read("apps/worldwright/src/commands/maturity.rs")
        for field in ["sdk_supported", "protocol_supported", "content_schema_supported"]:
            self.assertRegex(maturity, rf'"{field}"\s*:\s*matrix\.{field}')
        self.assertIn('SDK_PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION")', self.sdk)
        self.assertIn('sdk_package_version: worldwright_sdk::SDK_PACKAGE_VERSION.into()', self.bundle)
        self.assertIn('m.sdk_package_version != worldwright_sdk::SDK_PACKAGE_VERSION', self.bundle)
        self.assertIn('.replace("@PACKAGE_VERSION@", worldwright_sdk::SDK_PACKAGE_VERSION)', self.bundle)
        self.assertIn('.replace("@MIN_RUST@", worldwright_sdk::COMPATIBILITY.minimum_rust)', self.bundle)

    def test_content_authority_is_used_by_dispatch_validation_and_inventory(self) -> None:
        for file in ["loader.rs", "validation.rs", "validated.rs"]:
            self.assertIn("crate::CONTENT_SCHEMA_VERSION", read(f"crates/worldwright-content/src/{file}"))
        self.assertIn("content_schema_supported: worldwright_content::SUPPORTED_CONTENT_SCHEMA_VERSIONS", self.sdk)

    def test_shared_rank_wiring_and_integer_recipe_reference(self) -> None:
        for file in ["crates/worldwright-testkit/src/benchmark.rs", "apps/worldwright-load/src/report.rs"]:
            self.assertIn("worldwright_core::measurement::nearest_rank_index", read(file))
        # Independent arithmetic model only; native Rust tests execute the utility.
        for count in [1, 2, 3, 20, 100, 7500, (1 << 64) - 1]:
            for percent in range(1, 101):
                split = count // 100 * percent + (count % 100 * percent + 99) // 100 - 1
                exact = (count * percent + 99) // 100 - 1
                self.assertEqual(split, exact)
                self.assertTrue(0 <= split < count)

    def test_retained_fixture_inventory_and_integrity(self) -> None:
        base = "apps/worldwright/tests/fixtures/sdk-api-v1/"
        inventory = json.loads(read(base + "inventory.json"))
        self.assertEqual(inventory["schema_version"], 1)
        self.assertEqual(inventory["sdk_api_version"], 1)
        self.assertFalse(inventory["captured_from_released_package"])
        self.assertEqual(inventory["captured_package_version"], "0.1.0-dev.0")
        self.assertEqual(hashlib.sha256(read(base + "consumer.rs").encode()).hexdigest(), inventory["source_sha256_lf"])
        self.assertIn("retained_api_v1_builds_and_executes_real_reference_rules", read(base + "consumer.rs"))

    def test_transport_independence_and_real_export_drift_guard(self) -> None:
        manifest = tomllib.loads(read("crates/worldwright-sdk/Cargo.toml"))
        self.assertNotIn("worldwright-protocol", manifest["dependencies"])
        cli = tomllib.loads(read("apps/worldwright/Cargo.toml"))
        self.assertIn("worldwright-protocol", cli["dev-dependencies"])
        consumer = read("apps/worldwright/tests/sdk_compatibility.rs")
        self.assertIn("worldwright_protocol::SUPPORTED_PROTOCOL_VERSIONS", consumer)
        self.assertIn("worldwright_content::SUPPORTED_CONTENT_SCHEMA_VERSIONS", consumer)
        self.assertIn('fs::rename(&bundle, &sdk)', consumer)
        self.assertIn('"--filter-platform"', consumer)
        self.assertIn('manifest.starts_with(&consumer_root)', consumer)
        self.assertRegex(consumer, r'if locked\s*\{\s*cargo\.arg\("--locked"\);\s*\}')

    def test_fixed_beta_thresholds_and_runtime_recipe_version_remain_explicit(self) -> None:
        report = read("apps/worldwright-load/src/report.rs")
        for literal in ["750_000", "1_500_000", "10_000_000", "90_000", "7_500", "250"]:
            self.assertIn(literal, report)
        benchmark = read("crates/worldwright-testkit/src/benchmark.rs")
        self.assertIn('RUNTIME_WORKLOAD_RECIPE: &str = "runtime-look-nearest-rank-v2"', benchmark)
        self.assertIn('RUNTIME_WORKLOAD_RECIPE_VERSION: u32 = 2', benchmark)
        self.assertIn('"schema_version\\\":2', read("apps/worldwright/src/commands/maturity.rs"))


if __name__ == "__main__":
    unittest.main(verbosity=2)
