use crate::cli::GenerateOptions;
use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    process::ExitCode,
};
use worldwright_core::ids::{AccountId, CharacterId};

const CLIENTS: usize = 250;
const MAX_FILE_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Credential {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CredentialsFile {
    pub schema_version: u32,
    pub credentials: Vec<Credential>,
}

#[derive(Serialize)]
struct SeedFile {
    schema_version: u32,
    accounts: Vec<SeedAccount>,
}

#[derive(Serialize)]
struct SeedAccount {
    username: String,
    password_hash: String,
    account_id: String,
    character_id: String,
    character_name: String,
}

pub(crate) fn generate(options: GenerateOptions) -> ExitCode {
    match generate_inner(&options) {
        Ok(()) => {
            println!(
                "generated {CLIENTS} accounts in {} and {}",
                options.seed_output.display(),
                options.credentials_output.display()
            );
            println!("warning: credentials output contains plaintext test passwords");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("worldwright-load: {message}");
            ExitCode::from(1)
        }
    }
}

fn generate_inner(options: &GenerateOptions) -> Result<(), String> {
    if options.seed_output == options.credentials_output
        || options.seed_output.exists()
        || options.credentials_output.exists()
    {
        return Err("outputs must be distinct create-new paths".into());
    }
    validate_parent(&options.seed_output)?;
    validate_parent(&options.credentials_output)?;
    if options.password_env.is_empty()
        || options.password_env.len() > 128
        || !options
            .password_env
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
    {
        return Err("password environment name is invalid".into());
    }
    let password =
        std::env::var(&options.password_env).map_err(|_| "password environment is unavailable")?;
    validate_password(&password)?;
    let mut salt_bytes = [0u8; 16];
    getrandom::fill(&mut salt_bytes).map_err(|_| "salt generation failed")?;
    let salt = SaltString::encode_b64(&salt_bytes).map_err(|_| "salt encoding failed")?;
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| "password hashing failed")?
        .to_string();
    let mut accounts = Vec::with_capacity(CLIENTS);
    let mut credentials = Vec::with_capacity(CLIENTS);
    for index in 1..=CLIENTS {
        let username = format!("load{index:03}");
        let account = AccountId::from_u128(0x9000 + index as u128)
            .map_err(|_| "account identity generation failed")?;
        let character = CharacterId::from_u128(0xa000 + index as u128)
            .map_err(|_| "character identity generation failed")?;
        accounts.push(SeedAccount {
            username: username.clone(),
            password_hash: hash.clone(),
            account_id: account.to_string(),
            character_id: character.to_string(),
            character_name: format!("Load {index:03}"),
        });
        credentials.push(Credential {
            username,
            password: password.clone(),
        });
    }
    let seed = serde_json::to_vec(&SeedFile {
        schema_version: 1,
        accounts,
    })
    .map_err(|_| "seed serialization failed")?;
    let credentials = serde_json::to_vec(&CredentialsFile {
        schema_version: 1,
        credentials,
    })
    .map_err(|_| "credential serialization failed")?;
    if seed.len() as u64 > MAX_FILE_BYTES || credentials.len() as u64 > MAX_FILE_BYTES {
        return Err("generated fixture exceeds its size limit".into());
    }
    write_new(&options.seed_output, &seed, false)?;
    if let Err(error) = write_new(&options.credentials_output, &credentials, true) {
        let _ = fs::remove_file(&options.seed_output);
        return Err(error);
    }
    Ok(())
}

fn validate_parent(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let metadata = parent
        .symlink_metadata()
        .map_err(|_| "output parent is unavailable")?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("output parent is invalid".into());
    }
    Ok(())
}

fn write_new(path: &Path, bytes: &[u8], sensitive: bool) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    if sensitive {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    #[cfg(not(unix))]
    let _ = sensitive;
    let mut file = options.open(path).map_err(|_| "output create failed")?;
    let result = file.write_all(bytes).and_then(|_| file.sync_all());
    drop(file);
    if result.is_err() {
        let _ = fs::remove_file(path);
        return Err("output write failed".into());
    }
    Ok(())
}

pub(crate) fn load_credentials(path: &Path, expected: usize) -> Result<Vec<Credential>, String> {
    let metadata = path
        .symlink_metadata()
        .map_err(|_| "credentials are unavailable")?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() == 0
        || metadata.len() > MAX_FILE_BYTES
    {
        return Err("credentials file is invalid".into());
    }
    let bytes = fs::read(path).map_err(|_| "credentials read failed")?;
    let file: CredentialsFile =
        serde_json::from_slice(&bytes).map_err(|_| "credentials JSON is invalid")?;
    if file.schema_version != 1 || file.credentials.len() != expected {
        return Err("credentials do not match the selected profile".into());
    }
    let mut usernames = HashSet::new();
    for credential in &file.credentials {
        if credential.username.is_empty()
            || credential.username.len() > 64
            || !credential
                .username
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
            || !usernames.insert(credential.username.clone())
        {
            return Err("credential username is invalid or duplicated".into());
        }
        validate_password(&credential.password)?;
    }
    Ok(file.credentials)
}

fn validate_password(password: &str) -> Result<(), String> {
    if !(8..=256).contains(&password.len()) || password.chars().any(char::is_control) {
        return Err("password is invalid".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use argon2::{PasswordHash, PasswordVerifier};

    #[test]
    fn generated_identity_ranges_are_nonzero() {
        assert_ne!(AccountId::from_u128(0x9001).unwrap().get(), 0);
        assert_ne!(CharacterId::from_u128(0xa001).unwrap().get(), 0);
    }

    #[test]
    fn password_bounds_are_strict() {
        assert!(validate_password("12345678").is_ok());
        assert!(validate_password("short").is_err());
        let hash = Argon2::default()
            .hash_password(b"12345678", &SaltString::encode_b64(&[7; 16]).unwrap())
            .unwrap()
            .to_string();
        assert!(
            Argon2::default()
                .verify_password(b"12345678", &PasswordHash::new(&hash).unwrap())
                .is_ok()
        );
    }
}
