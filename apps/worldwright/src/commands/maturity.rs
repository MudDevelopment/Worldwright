use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    process::ExitCode,
};
use worldwright_content::{ContentLimits, load_draft_bytes, serialize_draft};
use worldwright_testkit::{Workload, run_runtime_workload};

pub fn compatibility(json: bool) -> ExitCode {
    let matrix = worldwright_sdk::COMPATIBILITY;
    if json {
        println!(
            "{{\"sdk_current\":{},\"sdk_supported\":[1],\"protocol_supported\":[1],\"content_schema_supported\":[1],\"minimum_rust\":\"{}\",\"policy\":\"{}\"}}",
            matrix.sdk_current,
            matrix.minimum_rust,
            worldwright_sdk::COMPATIBILITY_POLICY
        );
    } else {
        println!("SDK API: {} (supported: 1)", matrix.sdk_current);
        println!("Protocol: 1");
        println!("Content schema: 1");
        println!("Minimum Rust: {}", matrix.minimum_rust);
        println!("Policy: {}", worldwright_sdk::COMPATIBILITY_POLICY);
    }
    ExitCode::SUCCESS
}

pub fn benchmark(clients: usize, commands_per_client: usize, json: bool) -> ExitCode {
    let report = match run_runtime_workload(Workload {
        clients,
        commands_per_client,
    }) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("worldwright: {error}");
            return ExitCode::from(1);
        }
    };
    if json {
        println!(
            "{{\"clients\":{},\"commands\":{},\"elapsed_ns\":{},\"p50_ns\":{},\"p95_ns\":{},\"p99_ns\":{},\"max_ns\":{},\"delivered_messages\":{}}}",
            report.clients,
            report.commands,
            report.elapsed.as_nanos(),
            report.p50.as_nanos(),
            report.p95.as_nanos(),
            report.p99.as_nanos(),
            report.max.as_nanos(),
            report.delivered_messages
        );
    } else {
        println!("Clients: {}", report.clients);
        println!("Commands: {}", report.commands);
        println!("Elapsed: {:?}", report.elapsed);
        println!(
            "Latency p50/p95/p99/max: {:?} / {:?} / {:?} / {:?}",
            report.p50, report.p95, report.p99, report.max
        );
        println!("Delivered messages: {}", report.delivered_messages);
        println!("Development workload only; this is not a beta capacity promise.");
    }
    ExitCode::SUCCESS
}

pub fn migrate(input: &Path, output: &Path, backup: Option<&Path>, dry_run: bool) -> ExitCode {
    let bytes = match fs::read(input) {
        Ok(bytes) => bytes,
        Err(_) => {
            eprintln!("worldwright: migration input could not be read");
            return ExitCode::from(1);
        }
    };
    let draft = match load_draft_bytes("migration input", &bytes, ContentLimits::default()) {
        Ok(draft) => draft,
        Err(report) => {
            eprint!("{report}");
            return ExitCode::from(1);
        }
    };
    let encoded = match serialize_draft("migration output", &draft, ContentLimits::default()) {
        Ok(encoded) => encoded,
        Err(report) => {
            eprint!("{report}");
            return ExitCode::from(1);
        }
    };
    println!("Input schema: {}", draft.schema_version);
    println!("Output schema: 1");
    println!("Canonical bytes: {}", encoded.len());
    if let Some(path) = backup {
        println!("Backup: {}", path.display());
    }
    if dry_run {
        println!("Dry run: no file written");
        return ExitCode::SUCCESS;
    }
    if let Some(path) = backup {
        let mut backup_file = match OpenOptions::new().write(true).create_new(true).open(path) {
            Ok(file) => file,
            Err(_) => {
                eprintln!("worldwright: backup exists or cannot be created; no output was written");
                return ExitCode::from(3);
            }
        };
        if backup_file
            .write_all(&bytes)
            .and_then(|()| backup_file.sync_all())
            .is_err()
        {
            let _ = fs::remove_file(path);
            eprintln!("worldwright: backup write failed; no output was written");
            return ExitCode::from(1);
        }
    }
    let mut file = match OpenOptions::new().write(true).create_new(true).open(output) {
        Ok(file) => file,
        Err(_) => {
            eprintln!("worldwright: output exists or cannot be created; no file was overwritten");
            return ExitCode::from(3);
        }
    };
    if file
        .write_all(&encoded)
        .and_then(|()| file.sync_all())
        .is_err()
    {
        let _ = fs::remove_file(output);
        eprintln!("worldwright: migration output write failed");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

pub fn project_new(path: &Path, name: &str, template: &str, dry_run: bool) -> ExitCode {
    if !valid_package_name(name) {
        eprintln!(
            "worldwright: package name must use lowercase ASCII letters, digits, and hyphens"
        );
        return ExitCode::from(2);
    }
    if path.file_name().and_then(|value| value.to_str()) != Some(name) {
        eprintln!("worldwright: destination directory, package, and binary names must match");
        return ExitCode::from(2);
    }
    if path.exists() {
        eprintln!("worldwright: destination already exists; nothing was changed");
        return ExitCode::from(3);
    }
    println!("Project: {}", path.display());
    println!("Package/binary: {name}");
    println!("Template: {template}");
    if dry_run {
        println!("Dry run: no files created");
        return ExitCode::SUCCESS;
    }
    let source = path.join("src");
    let tests = path.join("tests");
    let content = path.join("content");
    if fs::create_dir_all(&source)
        .and_then(|()| fs::create_dir(&tests))
        .and_then(|()| fs::create_dir(&content))
        .is_err()
    {
        eprintln!("worldwright: project directory creation failed");
        return ExitCode::from(1);
    }
    if let Err(error) = super::sdk::bundle_result(&path.join("sdk")) {
        let _ = fs::remove_dir_all(path);
        eprintln!("worldwright: project SDK bundle failed: {error}");
        return ExitCode::from(1);
    }
    let sdk = "sdk/crates/worldwright-sdk";
    let expedition = "sdk/crates/worldwright-expedition";
    let expedition_dependency = format!("worldwright-expedition = {{ path = {expedition:?} }}\n");
    let cargo = format!(
        "[workspace]\n\n[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\nrust-version = \"1.85\"\npublish = false\n\n[dependencies]\nworldwright-sdk = {{ path = {sdk:?} }}\n{expedition_dependency}"
    );
    let main = if template == "expedition" {
        "fn main() -> std::process::ExitCode {\n    worldwright_expedition::terminal::run(include_bytes!(\"../content/world.json\"))\n}\n"
    } else {
        "fn main() -> std::process::ExitCode {\n    worldwright_expedition::terminal::run_reference(include_bytes!(\"../content/world.json\"))\n}\n"
    };
    let test = r#"#[test]
fn generated_content_is_valid() {
    let bytes = include_bytes!("../content/world.json");
    assert!(worldwright_sdk::content::load_bytes(
        "content/world.json",
        bytes,
        Default::default(),
    ).is_ok());
}
"#;
    let world = if template == "expedition" {
        include_bytes!("../../../../examples/expedition/world.json").as_slice()
    } else {
        include_bytes!("../../../../examples/two-room/world.json").as_slice()
    };
    let result = fs::write(path.join("Cargo.toml"), cargo)
        .and_then(|()| fs::write(source.join("main.rs"), main))
        .and_then(|()| fs::write(tests.join("content.rs"), test))
        .and_then(|()| fs::write(content.join("world.json"), world))
        .and_then(|()| fs::write(path.join(".gitignore"), "/target\n"))
        .and_then(|()| {
            fs::write(
                path.join("README.md"),
                format!(
                    "# {name}\n\nGenerated by Worldwright without a license grant. This project carries a bounded verified private SDK under sdk/; dependency paths are relative and the whole directory can be moved away from the original checkout. Keep sdk/bundle-manifest.json and SOURCE-Cargo.lock as source provenance (not an authorship signature).\n\nFirst bootstrap: `cargo test --offline` creates Cargo.lock; retain/review it.\nThen run `cargo test --locked --offline` and `cargo run --locked --offline`.\n\nExpedition templates are playable: look/status, go <exit>, say <text>, forage, encounter, strike, rest, rank and quit. Recovery is applied on the first command after its one-second deadline. Local progress is ephemeral; multiplayer PostgreSQL persistence belongs to worldwright-server.\n"
                ),
            )
        });
    if result.is_err() {
        eprintln!("worldwright: project write failed; remove the incomplete destination");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn valid_package_name(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z'))
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}
