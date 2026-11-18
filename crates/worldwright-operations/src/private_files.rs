//! Fail-closed private credential/backup file handling without unsafe code.
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read},
    path::{Path, PathBuf},
};

fn denied() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "private file requires an owned protected directory, no links/reparse points, and owner-only access (Windows SYSTEM/Administrators are trusted)",
    )
}
fn absolute(path: &Path) -> io::Result<PathBuf> {
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    if path
        .components()
        .any(|p| matches!(p, std::path::Component::ParentDir))
    {
        return Err(denied());
    }
    Ok(path)
}
fn no_links(path: &Path) -> io::Result<()> {
    for item in path.ancestors() {
        let m = fs::symlink_metadata(item)?;
        if m.file_type().is_symlink() {
            return Err(denied());
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if m.file_attributes() & 0x400 != 0 {
                return Err(denied());
            }
        }
    }
    Ok(())
}
fn private_metadata(path: &Path, m: &fs::Metadata) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        #[cfg(target_os = "linux")]
        let uid = fs::metadata("/proc/self")?.uid();
        #[cfg(not(target_os = "linux"))]
        let uid = {
            return Err(denied());
        };
        if m.uid() != uid || m.mode() & 0o077 != 0 || (m.is_file() && m.nlink() != 1) {
            return Err(denied());
        }
        let _ = path;
    }
    #[cfg(windows)]
    {
        let _ = m;
        windows_acl(path)?;
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (path, m);
        return Err(denied());
    }
    Ok(())
}

/// Validate existing parent BEFORE secrets are written. Never silently chmod.
pub fn validate_private_parent(path: &Path) -> io::Result<()> {
    let path = absolute(path)?;
    let parent = path.parent().ok_or_else(denied)?;
    no_links(parent)?;
    let m = fs::metadata(parent)?;
    if !m.is_dir() {
        return Err(denied());
    }
    private_metadata(parent, &m)
}

pub fn create_private(path: &Path) -> io::Result<File> {
    validate_private_parent(path)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path)?;
    // Empty file only at this point. A failed ACL check never writes a secret.
    no_links(&absolute(path)?)?;
    private_metadata(&absolute(path)?, &file.metadata()?)?;
    Ok(file)
}

pub fn open_private(path: &Path) -> io::Result<File> {
    let path = absolute(path)?;
    validate_private_parent(&path)?;
    no_links(&path)?;
    let original = fs::symlink_metadata(&path)?;
    private_metadata(&path, &original)?;
    if !original.is_file() {
        return Err(denied());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000).share_mode(1);
    }
    let file = options.open(&path)?;
    let opened = file.metadata()?;
    private_metadata(&path, &opened)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if original.ino() != opened.ino() || original.dev() != opened.dev() {
            return Err(denied());
        }
    }
    if !opened.is_file() {
        return Err(denied());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if opened.file_attributes() & 0x400 != 0 {
            return Err(denied());
        }
    }
    Ok(file)
}

pub fn read_private(path: &Path, max: usize) -> io::Result<Vec<u8>> {
    if max == 0 || max > 64 * 1024 * 1024 {
        return Err(denied());
    }
    let file = open_private(path)?;
    let expected = file.metadata()?.len();
    if expected > max as u64 {
        return Err(denied());
    }
    let mut bytes = Vec::new();
    file.take(max as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > max || bytes.len() as u64 != expected {
        return Err(denied());
    }
    Ok(bytes)
}

#[cfg(windows)]
fn windows_acl(path: &Path) -> io::Result<()> {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let root = PathBuf::from(std::env::var_os("SystemRoot").ok_or_else(denied)?);
    if !root.is_absolute() {
        return Err(denied());
    }
    let executable = root.join("System32/WindowsPowerShell/v1.0/powershell.exe");
    // Fixed code; the path is an environment value, never interpolated as code.
    const CHECK: &str = r#"$ErrorActionPreference='Stop';try {
$me=[System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value;
$trusted=@($me,'S-1-5-18','S-1-5-32-544');
$acl=Get-Acl -LiteralPath $env:WORLDWRIGHT_PRIVATE_FILE;
$owner=$acl.GetOwner([System.Security.Principal.SecurityIdentifier]).Value;
if ($trusted -notcontains $owner) {exit 2};
$rules=$acl.GetAccessRules($true,$true,[System.Security.Principal.SecurityIdentifier]);
if ($rules.Count -eq 0) {exit 2};
foreach($rule in $rules) {if ($rule.AccessControlType -eq 'Allow' -and $trusted -notcontains $rule.IdentityReference.Value) {exit 2}};
exit 0
} catch {exit 2}"#;
    let mut command = Command::new(executable);
    command
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            CHECK,
        ])
        .env_clear()
        .env("SystemRoot", &root)
        .env("WINDIR", &root)
        .env("WORLDWRIGHT_PRIVATE_FILE", path);
    // Keep only non-secret storage settings after env_clear. Otherwise the
    // PowerShell ACL probe can fall back to the real user's profile/temp cache.
    for key in [
        "TEMP",
        "TMP",
        "TMPDIR",
        "HOME",
        "USERPROFILE",
        "LOCALAPPDATA",
        "APPDATA",
        "PSModuleAnalysisCachePath",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ok(())
                } else {
                    Err(denied())
                };
            }
            Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(denied());
            }
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::{io::Write, os::unix::fs::PermissionsExt};
    #[test]
    fn r6_private_files_reject_permissions_links_and_oversize() {
        let root = std::env::temp_dir().join(format!(
            "r6-private-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let path = root.join("credential");
        let mut f = create_private(&path).unwrap();
        f.write_all(b"private").unwrap();
        drop(f);
        assert_eq!(read_private(&path, 7).unwrap(), b"private");
        assert!(read_private(&path, 6).is_err());
        assert!(create_private(&path).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(read_private(&path, 7).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let link = root.join("hard");
        fs::hard_link(&path, &link).unwrap();
        assert!(read_private(&path, 7).is_err());
        fs::remove_file(link).unwrap();
        std::os::unix::fs::symlink(&path, root.join("sym")).unwrap();
        assert!(read_private(&root.join("sym"), 7).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
