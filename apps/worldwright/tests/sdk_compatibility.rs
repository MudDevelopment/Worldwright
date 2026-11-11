//! P2 drift guards and retained source consumer; no historical release/ABI claim.
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_worldwright"))
}

#[test]
fn compatibility_matches_real_sdk_content_and_protocol_exports() {
    let matrix = worldwright_sdk::COMPATIBILITY;
    assert_eq!(
        matrix.protocol_supported,
        worldwright_protocol::SUPPORTED_PROTOCOL_VERSIONS
    );
    assert_eq!(
        matrix.content_schema_supported,
        worldwright_content::SUPPORTED_CONTENT_SCHEMA_VERSIONS
    );
    assert_eq!(
        worldwright_sdk::SDK_PACKAGE_VERSION,
        env!("CARGO_PKG_VERSION")
    );
    let output = cli().args(["compatibility", "--json"]).output().unwrap();
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schema_version"], 1);
    assert_eq!(
        value["sdk_package_version"],
        worldwright_sdk::SDK_PACKAGE_VERSION
    );
    assert_eq!(value["sdk_current"], matrix.sdk_current);
    assert_eq!(
        value["sdk_supported"],
        serde_json::json!(matrix.sdk_supported)
    );
    assert_eq!(
        value["protocol_supported"],
        serde_json::json!(matrix.protocol_supported)
    );
    assert_eq!(
        value["content_schema_supported"],
        serde_json::json!(matrix.content_schema_supported)
    );
    assert_eq!(value["minimum_rust"], matrix.minimum_rust);
    assert_eq!(value["policy"], worldwright_sdk::COMPATIBILITY_POLICY);
}

struct TempRoot(PathBuf);
impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn retained_sdk_api_v1_fixture_builds_from_relocated_private_bundle() {
    let fixture = include_str!("fixtures/sdk-api-v1/consumer.rs").replace("\r\n", "\n");
    let inventory: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/sdk-api-v1/inventory.json")).unwrap();
    assert_eq!(inventory["schema_version"], 1);
    assert_eq!(inventory["sdk_api_version"], 1);
    assert_eq!(inventory["captured_from_released_package"], false);
    assert_eq!(
        inventory["source_sha256_lf"],
        format!("{:x}", Sha256::digest(fixture.as_bytes()))
    );
    assert!(worldwright_sdk::COMPATIBILITY.sdk_supported.contains(&1));
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = TempRoot(
        std::env::temp_dir().join(format!("worldwright-sdk-v1-{}-{nonce}", std::process::id())),
    );
    fs::create_dir(&root.0).unwrap();
    let bundle = root.0.join("original-sdk");
    let status = cli()
        .env_remove("WORLDWRIGHT_SDK_BUNDLE")
        .args(["sdk", "bundle", "--output"])
        .arg(&bundle)
        .status()
        .unwrap();
    assert!(status.success());
    let consumer = root.0.join("independent-consumer");
    fs::create_dir(&consumer).unwrap();
    let sdk = consumer.join("sdk");
    fs::rename(&bundle, &sdk).unwrap();
    assert!(!bundle.exists());
    assert!(
        cli()
            .args(["sdk", "verify-bundle"])
            .arg(&sdk)
            .status()
            .unwrap()
            .success()
    );
    let sdk_lock = fs::read(sdk.join("Cargo.lock")).unwrap();
    fs::create_dir(consumer.join("src")).unwrap();
    fs::create_dir(consumer.join("content")).unwrap();
    fs::write(consumer.join("src/lib.rs"), fixture).unwrap();
    fs::copy(
        sdk.join("examples/two-room/world.json"),
        consumer.join("content/world.json"),
    )
    .unwrap();
    fs::write(consumer.join("Cargo.toml"), format!(
        "[workspace]\n[package]\nname = \"sdk-api-v1-consumer\"\nversion = \"0.0.0\"\nedition = \"2024\"\nrust-version = \"{}\"\npublish = false\n[dependencies]\nworldwright-sdk = {{ path = \"sdk/crates/worldwright-sdk\" }}\n",
        worldwright_sdk::COMPATIBILITY.minimum_rust)).unwrap();
    let host_output = Command::new("rustc").arg("-vV").output().unwrap();
    assert!(host_output.status.success());
    let host_text = String::from_utf8(host_output.stdout).unwrap();
    let host = host_text
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .unwrap();
    let target = root.0.join("native-target");
    for locked in [false, true] {
        let before = locked.then(|| fs::read(consumer.join("Cargo.lock")).unwrap());
        let mut cargo = Command::new(env!("CARGO"));
        cargo
            .current_dir(&consumer)
            .args(["test", "--offline", "--target", host])
            .arg("--target-dir")
            .arg(&target);
        if locked {
            cargo.arg("--locked");
        }
        let output = cargo.output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout)
                .contains("retained_api_v1_builds_and_executes_real_reference_rules")
        );
        if let Some(before) = before {
            assert_eq!(fs::read(consumer.join("Cargo.lock")).unwrap(), before);
        }
    }
    let metadata = Command::new(env!("CARGO"))
        .current_dir(&consumer)
        .args([
            "metadata",
            "--format-version",
            "1",
            "--offline",
            "--locked",
            "--filter-platform",
            host,
        ])
        .output()
        .unwrap();
    assert!(
        metadata.status.success(),
        "{}",
        String::from_utf8_lossy(&metadata.stderr)
    );
    let metadata: serde_json::Value = serde_json::from_slice(&metadata.stdout).unwrap();
    let packages = metadata["packages"].as_array().unwrap();
    let consumer_root = consumer.canonicalize().unwrap();
    let mut local_names = Vec::new();
    for package in packages {
        if package["source"].is_null() {
            let manifest = PathBuf::from(package["manifest_path"].as_str().unwrap())
                .canonicalize()
                .unwrap();
            assert!(
                manifest.starts_with(&consumer_root),
                "dependency escaped relocated consumer: {}",
                manifest.display()
            );
            local_names.push(package["name"].as_str().unwrap());
        }
    }
    local_names.sort_unstable();
    assert_eq!(
        local_names,
        [
            "sdk-api-v1-consumer",
            "worldwright-content",
            "worldwright-core",
            "worldwright-sdk"
        ]
    );
    assert!(
        !packages
            .iter()
            .any(|package| package["name"] == "worldwright-protocol")
    );
    assert_eq!(fs::read(sdk.join("Cargo.lock")).unwrap(), sdk_lock);
    assert!(
        cli()
            .args(["sdk", "verify-bundle"])
            .arg(&sdk)
            .status()
            .unwrap()
            .success()
    );
}
