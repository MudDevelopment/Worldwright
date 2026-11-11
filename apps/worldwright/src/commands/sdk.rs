use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    process::ExitCode,
};
const MAX_FILES: usize = 512;
const MAX_FILE: u64 = 1024 * 1024;
const MAX_TOTAL: u64 = 8 * 1024 * 1024;
const CRATES: &[&str] = &[
    "worldwright-core",
    "worldwright-content",
    "worldwright-sdk",
    "worldwright-expedition",
    "worldwright-testkit",
];
#[derive(Default)]
struct BundleBounds {
    files: usize,
    total: u64,
}
fn forbidden_metadata(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BundleFile {
    path: String,
    bytes: u64,
    sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BundleManifest {
    schema_version: u32,
    sdk_package_version: String,
    sdk_api_version: u32,
    min_rust: String,
    compatibility: String,
    creation_format_version: u32,
    files: Vec<BundleFile>,
}
pub fn bundle(p: &Path) -> ExitCode {
    match bundle_result(p) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("worldwright: {e}");
            ExitCode::from(1)
        }
    }
}
pub fn verify_bundle(p: &Path) -> ExitCode {
    match verify_result(p) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("worldwright: {e}");
            ExitCode::from(1)
        }
    }
}
pub(crate) fn bundle_result(out: &Path) -> Result<(), String> {
    validate_parent(out)?;
    if out.exists() {
        return Err("bundle output exists".into());
    }
    fs::create_dir(out).map_err(|_| "bundle output create failed")?;
    let result = (|| {
        if let Some(source) = std::env::var_os("WORLDWRIGHT_SDK_BUNDLE") {
            let source = std::path::PathBuf::from(source);
            verify_result(&source)?;
            copy_tree(&source, out, &mut BundleBounds::default())?;
            return verify_result(out);
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .ok_or("repository root unavailable")?;
        let mut bounds = BundleBounds::default();
        for name in CRATES {
            copy_tree(
                &root.join("crates").join(name),
                &out.join("crates").join(name),
                &mut bounds,
            )?
        }
        copy_tree(
            &root.join("examples").join("two-room"),
            &out.join("examples").join("two-room"),
            &mut bounds,
        )?;
        copy_tree(
            &root.join("examples").join("expedition"),
            &out.join("examples").join("expedition"),
            &mut bounds,
        )?;
        write_new(
            &out.join("Cargo.lock"),
            include_bytes!("../../sdk-bundle-Cargo.lock"),
        )?;
        write_new(
            &out.join("SOURCE-Cargo.lock"),
            &fs::read(root.join("Cargo.lock")).map_err(|_| "Cargo.lock read failed")?,
        )?;
        write_new(&out.join("Cargo.toml"), generated_cargo().as_bytes())?;
        write_new(&out.join("PRIVATE-BUNDLE.md"), private_readme().as_bytes())?;
        write_new(&out.join("bundle-manifest.json"), &manifest_bytes(out)?)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(out);
    }
    result
}
fn generated_cargo() -> String {
    r#"[workspace]
members = [@MEMBERS@]
resolver = "3"
[workspace.package]
version = "@PACKAGE_VERSION@"
edition = "2024"
rust-version = "@MIN_RUST@"
publish = false
[workspace.dependencies]
worldwright-core = { path = "crates/worldwright-core", version = "=@PACKAGE_VERSION@" }
worldwright-content = { path = "crates/worldwright-content", version = "=@PACKAGE_VERSION@" }
worldwright-sdk = { path = "crates/worldwright-sdk", version = "=@PACKAGE_VERSION@" }
worldwright-expedition = { path = "crates/worldwright-expedition", version = "=@PACKAGE_VERSION@" }
worldwright-testkit = { path = "crates/worldwright-testkit", version = "=@PACKAGE_VERSION@" }
serde = { version = "=1.0.228", features = ["derive"] }
serde_json = "=1.0.145"
serde_path_to_error = "=0.1.20"
getrandom = "=0.3.4"
[workspace.lints.rust]
unsafe_code = "forbid"
[workspace.lints.clippy]
all = "warn"
"#
    .replace("@PACKAGE_VERSION@", worldwright_sdk::SDK_PACKAGE_VERSION)
    .replace("@MIN_RUST@", worldwright_sdk::COMPATIBILITY.minimum_rust)
    .replace(
        "@MEMBERS@",
        &CRATES
            .iter()
            .map(|name| format!("\"crates/{name}\""))
            .collect::<Vec<_>>()
            .join(","),
    )
}
fn private_readme() -> String {
    "# Private SDK bundle\n\nInternal source delivery; no registry publication or license grant is inferred.\n\nCargo.lock is the reduced buildable SDK workspace lock. SOURCE-Cargo.lock records the full source workspace lock for provenance. Both are included in the manifest.\n\nSDK package @PACKAGE_VERSION@; Rust @MIN_RUST@; policy: @POLICY@. Only actually implemented API versions are advertised; no previous minor is invented.\n\nIncluded crates: @CRATES@.\n\nBuild: `cargo check --workspace --all-targets --locked --offline`\nTest: `cargo test --workspace --locked --offline`\n"
    .replace("@PACKAGE_VERSION@", worldwright_sdk::SDK_PACKAGE_VERSION)
    .replace("@MIN_RUST@", worldwright_sdk::COMPATIBILITY.minimum_rust)
    .replace("@POLICY@", worldwright_sdk::COMPATIBILITY_POLICY)
    .replace("@CRATES@", &CRATES.join(", "))
}
fn validate_parent(out: &Path) -> Result<(), String> {
    let parent = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut p = Some(parent);
    while let Some(path) = p {
        let m = path
            .symlink_metadata()
            .map_err(|_| "bundle parent invalid")?;
        if !m.is_dir() || forbidden_metadata(&m) {
            return Err("bundle parent invalid".into());
        }
        p = path.parent().filter(|v| !v.as_os_str().is_empty())
    }
    Ok(())
}
fn copy_tree(src: &Path, dst: &Path, bounds: &mut BundleBounds) -> Result<(), String> {
    let m = src
        .symlink_metadata()
        .map_err(|_| "bundle source invalid")?;
    if !m.is_dir() || forbidden_metadata(&m) {
        return Err("bundle source invalid".into());
    }
    fs::create_dir_all(dst).map_err(|_| "bundle directory create failed")?;
    let mut es = fs::read_dir(src)
        .map_err(|_| "bundle source listing failed")?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "bundle source listing failed")?;
    es.sort_by_key(|e| e.file_name());
    for e in es {
        let n = e.file_name();
        let s = n.to_str().ok_or("bundle filename invalid")?;
        if s == ".git" || s == "target" || s.ends_with(".rs.bk") {
            return Err("forbidden bundle source entry".into());
        }
        let p = e.path();
        let m = p
            .symlink_metadata()
            .map_err(|_| "bundle source metadata failed")?;
        if forbidden_metadata(&m) {
            return Err("bundle source symlink".into());
        }
        let d = dst.join(s);
        if m.is_dir() {
            copy_tree(&p, &d, bounds)?
        } else if m.is_file() {
            let b = fs::read(&p).map_err(|_| "bundle source read failed")?;
            if b.len() as u64 > MAX_FILE {
                return Err("bundle file limit".into());
            }
            bounds.files += 1;
            bounds.total = bounds
                .total
                .checked_add(b.len() as u64)
                .ok_or("bundle total overflow")?;
            if bounds.files > MAX_FILES || bounds.total > MAX_TOTAL {
                return Err("bundle bounds exceeded".into());
            }
            write_new(&d, &b)?
        } else {
            return Err("bundle source entry invalid".into());
        }
    }
    Ok(())
}
fn write_new(p: &Path, b: &[u8]) -> Result<(), String> {
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(p)
        .map_err(|_| "bundle file create failed")?;
    f.write_all(b)
        .and_then(|_| f.sync_all())
        .map_err(|_| "bundle file write failed".into())
}
fn digest(b: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(b);
    format!("{:x}", h.finalize())
}
fn manifest_bytes(root: &Path) -> Result<Vec<u8>, String> {
    let mut files = Vec::new();
    collect(root, root, &mut files)?;
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let m = BundleManifest {
        schema_version: 1,
        sdk_package_version: worldwright_sdk::SDK_PACKAGE_VERSION.into(),
        sdk_api_version: worldwright_sdk::COMPATIBILITY.sdk_current,
        min_rust: worldwright_sdk::COMPATIBILITY.minimum_rust.into(),
        compatibility: worldwright_sdk::COMPATIBILITY_POLICY.into(),
        creation_format_version: 1,
        files,
    };
    serde_json::to_vec(&m).map_err(|_| "manifest serialize failed".into())
}
fn collect(root: &Path, base: &Path, files: &mut Vec<BundleFile>) -> Result<(), String> {
    let mut es = fs::read_dir(root)
        .map_err(|_| "bundle listing failed")?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "bundle listing failed")?;
    es.sort_by_key(|e| e.file_name());
    for e in es {
        let p = e.path();
        if p.file_name().and_then(|v| v.to_str()) == Some("bundle-manifest.json") {
            continue;
        }
        let m = p.symlink_metadata().map_err(|_| "bundle metadata failed")?;
        if forbidden_metadata(&m) {
            return Err("bundle symlink".into());
        }
        if m.is_dir() {
            collect(&p, base, files)?
        } else if m.is_file() {
            let rel = p
                .strip_prefix(base)
                .map_err(|_| "bundle path")?
                .to_string_lossy()
                .replace('\\', "/");
            validate_path(&rel)?;
            let b = fs::read(&p).map_err(|_| "bundle read failed")?;
            if b.len() as u64 > MAX_FILE {
                return Err("bundle file limit".into());
            }
            files.push(BundleFile {
                path: rel,
                bytes: b.len() as u64,
                sha256: digest(&b),
            });
            if files.len() > MAX_FILES {
                return Err("bundle count limit".into());
            }
        } else {
            return Err("bundle entry invalid".into());
        }
    }
    let total = files
        .iter()
        .try_fold(0u64, |total, file| total.checked_add(file.bytes))
        .ok_or("bundle total overflow")?;
    if total > MAX_TOTAL {
        return Err("bundle total limit".into());
    }
    Ok(())
}
fn validate_path(p: &str) -> Result<(), String> {
    if p.is_empty()
        || p.contains('\\')
        || p.contains(':')
        || p.starts_with('/')
        || p.contains(char::is_control)
        || p == "bundle-manifest.json"
    {
        return Err("manifest path invalid".into());
    }
    if p.split('/').any(|s| s.is_empty() || s == "." || s == "..") {
        return Err("manifest path invalid".into());
    }
    Ok(())
}
pub(crate) fn verify_result(root: &Path) -> Result<(), String> {
    let root_metadata = root
        .symlink_metadata()
        .map_err(|_| "bundle directory invalid")?;
    if !root_metadata.is_dir() || forbidden_metadata(&root_metadata) {
        return Err("bundle directory invalid".into());
    }
    let mm = root
        .join("bundle-manifest.json")
        .symlink_metadata()
        .map_err(|_| "manifest missing")?;
    if !mm.is_file() || forbidden_metadata(&mm) || mm.len() > MAX_FILE {
        return Err("manifest invalid".into());
    }
    let m: BundleManifest = serde_json::from_slice(
        &fs::read(root.join("bundle-manifest.json")).map_err(|_| "manifest missing")?,
    )
    .map_err(|_| "manifest invalid")?;
    if m.schema_version != 1
        || m.sdk_package_version != worldwright_sdk::SDK_PACKAGE_VERSION
        || m.sdk_api_version != worldwright_sdk::COMPATIBILITY.sdk_current
        || m.min_rust != worldwright_sdk::COMPATIBILITY.minimum_rust
        || m.compatibility != worldwright_sdk::COMPATIBILITY_POLICY
        || m.creation_format_version != 1
        || m.files.len() > MAX_FILES
    {
        return Err("manifest metadata invalid".into());
    }
    let mut set = BTreeSet::new();
    let mut prev = None;
    let mut total = 0u64;
    for f in &m.files {
        validate_path(&f.path)?;
        if prev.as_ref().is_some_and(|p: &String| p >= &f.path) || !set.insert(f.path.clone()) {
            return Err("manifest paths not ordered".into());
        }
        prev = Some(f.path.clone());
        if f.bytes > MAX_FILE
            || f.sha256.len() != 64
            || !f
                .sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err("manifest file invalid".into());
        }
        let p = root.join(&f.path);
        let meta = p.symlink_metadata().map_err(|_| "manifest file missing")?;
        if !meta.is_file() || forbidden_metadata(&meta) {
            return Err("manifest file invalid".into());
        }
        let b = fs::read(p).map_err(|_| "manifest file missing")?;
        if b.len() as u64 != f.bytes || digest(&b) != f.sha256 {
            return Err("manifest file mismatch".into());
        }
        total = total.checked_add(f.bytes).ok_or("bundle total overflow")?
    }
    if total > MAX_TOTAL {
        return Err("bundle total limit".into());
    }
    let mut actual = Vec::new();
    collect_paths(root, root, &mut actual)?;
    actual.sort();
    if actual.len() != set.len() || actual.iter().any(|p| !set.contains(p)) {
        return Err("bundle file set mismatch".into());
    }
    Ok(())
}
fn collect_paths(root: &Path, base: &Path, out: &mut Vec<String>) -> Result<(), String> {
    for e in fs::read_dir(root).map_err(|_| "bundle listing failed")? {
        let e = e.map_err(|_| "bundle listing failed")?;
        let p = e.path();
        let m = p.symlink_metadata().map_err(|_| "bundle metadata failed")?;
        if forbidden_metadata(&m) {
            return Err("bundle symlink".into());
        }
        if m.is_dir() {
            collect_paths(&p, base, out)?
        } else if m.is_file()
            && p.file_name().and_then(|v| v.to_str()) != Some("bundle-manifest.json")
        {
            let rel = p
                .strip_prefix(base)
                .map_err(|_| "bundle path")?
                .to_string_lossy()
                .replace('\\', "/");
            validate_path(&rel)?;
            out.push(rel);
            if out.len() > MAX_FILES {
                return Err("bundle count limit".into());
            }
        } else if !m.is_file() {
            return Err("bundle entry invalid".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    fn temp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "worldwright-sdk-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
    #[test]
    fn generated_metadata_uses_sdk_authority() {
        let cargo = generated_cargo();
        assert!(!cargo.contains('@'));
        assert!(cargo.contains(&format!(
            "version = \"{}\"",
            worldwright_sdk::SDK_PACKAGE_VERSION
        )));
        assert!(cargo.contains(&format!(
            "rust-version = \"{}\"",
            worldwright_sdk::COMPATIBILITY.minimum_rust
        )));
        for name in CRATES {
            assert!(cargo.contains(&format!("\"crates/{name}\"")));
            assert!(cargo.contains(&format!(
                "{name} = {{ path = \"crates/{name}\", version = \"={}\" }}",
                worldwright_sdk::SDK_PACKAGE_VERSION
            )));
        }
        let readme = private_readme();
        assert!(!readme.contains('@'));
        assert!(readme.contains(worldwright_sdk::SDK_PACKAGE_VERSION));
        assert!(readme.contains(worldwright_sdk::COMPATIBILITY.minimum_rust));
        assert!(readme.contains(worldwright_sdk::COMPATIBILITY_POLICY));
    }
    #[test]
    fn generated_bundle_is_deterministic_and_verifiable() {
        let a = temp("a");
        let b = temp("b");
        bundle_result(&a).unwrap();
        bundle_result(&b).unwrap();
        verify_result(&a).unwrap();
        verify_result(&b).unwrap();
        for name in CRATES {
            let manifest =
                fs::read_to_string(a.join("crates").join(name).join("Cargo.toml")).unwrap();
            assert!(manifest.contains("workspace = \"../..\""), "{name}");
        }
        fn files(root: &Path) -> Vec<(String, Vec<u8>)> {
            let mut out = Vec::new();
            fn walk(p: &Path, b: &Path, o: &mut Vec<(String, Vec<u8>)>) {
                for e in fs::read_dir(p).unwrap() {
                    let e = e.unwrap();
                    if e.path().is_dir() {
                        walk(&e.path(), b, o)
                    } else {
                        o.push((
                            e.path().strip_prefix(b).unwrap().to_string_lossy().into(),
                            fs::read(e.path()).unwrap(),
                        ))
                    }
                }
            }
            walk(root, root, &mut out);
            out.sort_by(|x, y| x.0.cmp(&y.0));
            out
        }
        assert_eq!(files(&a), files(&b));
        let _ = fs::remove_dir_all(a);
        let _ = fs::remove_dir_all(b);
    }
    #[test]
    fn verification_rejects_mutations() {
        let root = temp("mutate");
        bundle_result(&root).unwrap();
        let target = root.join("PRIVATE-BUNDLE.md");
        let mut b = fs::read(&target).unwrap();
        b[0] ^= 1;
        fs::write(&target, b).unwrap();
        assert!(verify_result(&root).is_err());
        let _ = fs::remove_dir_all(root);
    }
}
