use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fmt::Write as _,
    fs,
    path::Path,
    str::FromStr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{sync::Semaphore, task};
use worldwright_core::ids::{AccountId, CharacterId};

pub(crate) const MAX_REGISTRY_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_ACCOUNTS: usize = 256;
pub(crate) const MAX_SESSIONS: usize = 1024;
pub(crate) const SESSION_TTL: Duration = Duration::from_secs(12 * 60 * 60);
pub(crate) const COOKIE_NAME: &str = "ww_session";
const DUMMY_PASSWORD: &[u8] = b"worldwright-invalid-login-dummy";
const UNAUTHORIZED_BODY: &str = "unauthorized\n";

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Principal {
    pub(crate) account: AccountId,
    pub(crate) character: CharacterId,
    pub(crate) character_name: String,
}

struct AccountRecord {
    password_hash: String,
    principal: Principal,
}

struct Session {
    principal: Principal,
    expires_at: Instant,
}

#[derive(Clone)]
pub(crate) struct AuthService {
    accounts: Arc<HashMap<String, AccountRecord>>,
    sessions: Arc<Mutex<HashMap<String, Session>>>,
    verification_slots: Arc<Semaphore>,
    dummy_hash: Arc<String>,
    cookie_secure: bool,
    ttl: Duration,
    max_sessions: usize,
}

#[derive(Debug)]
pub(crate) enum AuthError {
    InvalidInput,
    Unauthorized,
    Saturated,
    SessionCapacity,
    Internal,
}

impl AuthService {
    pub(crate) fn load(path: &Path, cookie_secure: bool) -> Result<Self, crate::ServerError> {
        let metadata = fs::metadata(path).map_err(crate::ServerError::io)?;
        if !metadata.is_file() {
            return Err(crate::ServerError::startup(
                "accounts path is not a regular file",
            ));
        }
        if metadata.len() == 0 || metadata.len() > MAX_REGISTRY_BYTES as u64 {
            return Err(crate::ServerError::startup(
                "account registry exceeds its size limit",
            ));
        }
        let bytes = fs::read(path).map_err(crate::ServerError::io)?;
        if bytes.is_empty() || bytes.len() > MAX_REGISTRY_BYTES {
            return Err(crate::ServerError::startup(
                "account registry exceeds its size limit",
            ));
        }
        let registry: Registry = serde_json::from_slice(&bytes)
            .map_err(|_| crate::ServerError::startup("account registry is not valid JSON"))?;
        if registry.schema_version != 1
            || registry.accounts.is_empty()
            || registry.accounts.len() > MAX_ACCOUNTS
        {
            return Err(crate::ServerError::startup(
                "account registry violates schema v1",
            ));
        }

        let mut accounts = HashMap::with_capacity(registry.accounts.len());
        let mut account_ids = std::collections::HashSet::new();
        let mut character_ids = std::collections::HashSet::new();
        for (index, record) in registry.accounts.into_iter().enumerate() {
            let principal = parse_record(index, &record, &mut account_ids, &mut character_ids)?;
            let key = record.username.clone();
            if accounts
                .insert(
                    key,
                    AccountRecord {
                        password_hash: record.password_hash,
                        principal,
                    },
                )
                .is_some()
            {
                return Err(crate::ServerError::startup("duplicate account username"));
            }
        }
        let dummy_hash = make_hash(DUMMY_PASSWORD)
            .map_err(|_| crate::ServerError::startup("unable to initialize password verifier"))?;
        Ok(Self {
            accounts: Arc::new(accounts),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            verification_slots: Arc::new(Semaphore::new(4)),
            dummy_hash: Arc::new(dummy_hash),
            cookie_secure,
            ttl: SESSION_TTL,
            max_sessions: MAX_SESSIONS,
        })
    }

    #[cfg(test)]
    pub(crate) fn from_records_for_test(
        records: Vec<(String, String, Principal)>,
        ttl: Duration,
    ) -> Self {
        Self::from_records_with_capacity_for_test(records, ttl, MAX_SESSIONS)
    }

    #[cfg(test)]
    pub(crate) fn from_records_with_capacity_for_test(
        records: Vec<(String, String, Principal)>,
        ttl: Duration,
        max_sessions: usize,
    ) -> Self {
        let accounts = records
            .into_iter()
            .map(|(username, password_hash, principal)| {
                (
                    username.clone(),
                    AccountRecord {
                        password_hash,
                        principal,
                    },
                )
            })
            .collect();
        Self {
            accounts: Arc::new(accounts),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            verification_slots: Arc::new(Semaphore::new(4)),
            dummy_hash: Arc::new(make_hash(DUMMY_PASSWORD).expect("dummy hash")),
            cookie_secure: false,
            ttl,
            max_sessions,
        }
    }

    pub(crate) async fn login(
        &self,
        username: String,
        password: String,
    ) -> Result<String, AuthError> {
        validate_username(&username).map_err(|_| AuthError::InvalidInput)?;
        validate_password(&password).map_err(|_| AuthError::InvalidInput)?;
        let record = self.accounts.get(&username);
        let hash = record
            .map(|record| record.password_hash.clone())
            .unwrap_or_else(|| self.dummy_hash.as_ref().clone());
        let permit = self
            .verification_slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| AuthError::Saturated)?;
        let password_bytes = password.into_bytes();
        let valid = task::spawn_blocking(move || {
            let _permit = permit;
            let parsed = PasswordHash::new(&hash).map_err(|_| ())?;
            Argon2::default()
                .verify_password(&password_bytes, &parsed)
                .map_err(|_| ())
                .map(|_| ())
        })
        .await
        .map_err(|_| AuthError::Internal)?
        .is_ok();
        let Some(record) = record else {
            return Err(AuthError::Unauthorized);
        };
        if !valid {
            return Err(AuthError::Unauthorized);
        }

        let mut sessions = self.sessions.lock().map_err(|_| AuthError::Internal)?;
        let now = Instant::now();
        sessions.retain(|_, session| session.expires_at > now);
        if sessions.len() >= self.max_sessions {
            return Err(AuthError::SessionCapacity);
        }
        let mut token = None;
        for _ in 0..8 {
            let candidate = random_hex(32).map_err(|_| AuthError::Internal)?;
            if !sessions.contains_key(&candidate) {
                token = Some(candidate);
                break;
            }
        }
        let token = token.ok_or(AuthError::Internal)?;
        sessions.insert(
            token.clone(),
            Session {
                principal: record.principal.clone(),
                expires_at: now.checked_add(self.ttl).ok_or(AuthError::Internal)?,
            },
        );
        Ok(token)
    }

    pub(crate) fn lookup(&self, token: &str) -> Option<Principal> {
        let mut sessions = self.sessions.lock().ok()?;
        let now = Instant::now();
        sessions.retain(|_, session| session.expires_at > now);
        sessions.get(token).map(|session| session.principal.clone())
    }

    pub(crate) fn revoke(&self, token: &str) {
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.remove(token);
        }
    }

    pub(crate) fn principals(&self) -> Vec<Principal> {
        self.accounts
            .values()
            .map(|record| record.principal.clone())
            .collect()
    }

    pub(crate) fn cookie_secure(&self) -> bool {
        self.cookie_secure
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Registry {
    schema_version: u32,
    accounts: Vec<RegistryAccount>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryAccount {
    username: String,
    password_hash: String,
    account_id: String,
    character_id: String,
    character_name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LoginRequest {
    pub(crate) username: String,
    pub(crate) password: String,
}

fn parse_record(
    index: usize,
    record: &RegistryAccount,
    account_ids: &mut std::collections::HashSet<AccountId>,
    character_ids: &mut std::collections::HashSet<CharacterId>,
) -> Result<Principal, crate::ServerError> {
    if validate_username(&record.username).is_err()
        || validate_character_name(&record.character_name).is_err()
    {
        return Err(crate::ServerError::startup_record(
            index,
            "invalid account name",
        ));
    }
    let account = AccountId::from_str(&record.account_id)
        .map_err(|_| crate::ServerError::startup_record(index, "invalid account ID"))?;
    let character = CharacterId::from_str(&record.character_id)
        .map_err(|_| crate::ServerError::startup_record(index, "invalid character ID"))?;
    if !account_ids.insert(account) || !character_ids.insert(character) {
        return Err(crate::ServerError::startup_record(
            index,
            "duplicate account or character ID",
        ));
    }
    validate_hash(&record.password_hash)
        .map_err(|_| crate::ServerError::startup_record(index, "invalid password hash"))?;
    Ok(Principal {
        account,
        character,
        character_name: record.character_name.clone(),
    })
}

fn validate_hash(hash: &str) -> Result<(), ()> {
    let parsed = PasswordHash::new(hash).map_err(|_| ())?;
    if parsed.algorithm != argon2::ARGON2ID_IDENT
        || parsed.version != Some(19)
        || parsed.salt.is_none()
        || parsed
            .hash
            .as_ref()
            .is_none_or(|output| output.as_bytes().len() != 32)
    {
        return Err(());
    }
    let actual = argon2::Params::try_from(&parsed).map_err(|_| ())?;
    let expected = argon2::Params::new(19_456, 2, 1, Some(32)).map_err(|_| ())?;
    if actual != expected {
        return Err(());
    }
    Ok(())
}

pub(crate) fn make_hash(password: &[u8]) -> Result<String, ()> {
    let mut salt_bytes = [0u8; 16];
    getrandom::fill(&mut salt_bytes).map_err(|_| ())?;
    let salt = SaltString::encode_b64(&salt_bytes).map_err(|_| ())?;
    Argon2::default()
        .hash_password(password, &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| ())
}

pub(crate) fn validate_username(value: &str) -> Result<(), ()> {
    let bytes = value.as_bytes();
    if bytes.is_empty()
        || bytes.len() > 64
        || !bytes.first().is_some_and(u8::is_ascii_lowercase)
        || bytes[1..].iter().any(|byte| {
            !byte.is_ascii_lowercase() && !byte.is_ascii_digit() && !matches!(byte, b'_' | b'-')
        })
    {
        return Err(());
    }
    Ok(())
}

pub(crate) fn validate_password(value: &str) -> Result<(), ()> {
    if value.is_empty() || value.len() > 1024 || value.chars().any(char::is_control) {
        return Err(());
    }
    Ok(())
}

pub(crate) fn validate_character_name(value: &str) -> Result<(), ()> {
    if value.is_empty()
        || value.len() > 64
        || value.trim().is_empty()
        || value.chars().any(char::is_control)
    {
        return Err(());
    }
    Ok(())
}

pub(crate) fn token_from_cookie(headers: &axum::http::HeaderMap) -> Option<String> {
    let cookie = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    let mut token = None;
    for part in cookie.split(';') {
        let Some((name, value)) = part.trim().split_once('=') else {
            continue;
        };
        if name != COOKIE_NAME {
            continue;
        }
        if token.is_some()
            || value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return None;
        }
        token = Some(value.to_owned());
    }
    token
}

pub(crate) fn session_cookie(token: &str, secure: bool) -> String {
    let secure_attribute = if secure { "; Secure" } else { "" };
    format!(
        "{COOKIE_NAME}={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age=43200{secure_attribute}"
    )
}

pub(crate) fn expired_cookie(secure: bool) -> String {
    let secure_attribute = if secure { "; Secure" } else { "" };
    format!("{COOKIE_NAME}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0{secure_attribute}")
}

pub(crate) fn unauthorized_response() -> axum::response::Response {
    (axum::http::StatusCode::UNAUTHORIZED, UNAUTHORIZED_BODY).into_response()
}

fn random_hex(bytes: usize) -> Result<String, ()> {
    let mut value = vec![0u8; bytes];
    getrandom::fill(&mut value).map_err(|_| ())?;
    let mut output = String::with_capacity(bytes * 2);
    for byte in value {
        let _ = write!(output, "{byte:02x}");
    }
    Ok(output)
}

use axum::response::IntoResponse;

#[cfg(test)]
mod tests {
    use super::*;

    fn principal() -> Principal {
        Principal {
            account: AccountId::from_u128(1).expect("ID"),
            character: CharacterId::from_u128(1).expect("ID"),
            character_name: "Alice".to_owned(),
        }
    }

    #[test]
    fn names_and_passwords_are_bounded() {
        assert!(validate_username("alice_1").is_ok());
        assert!(validate_username("Alice").is_err());
        assert!(validate_username("a".repeat(65).as_str()).is_err());
        assert!(validate_character_name("  ").is_err());
        assert!(validate_character_name("Ada\n").is_err());
        assert!(validate_password("password").is_ok());
        assert!(validate_password("a\n").is_err());
        assert!(validate_password(&"a".repeat(1025)).is_err());
    }

    #[tokio::test]
    async fn authentication_rejects_wrong_password_and_accepts_correct_password() {
        let hash = make_hash(b"correct horse").expect("hash");
        let service = AuthService::from_records_for_test(
            vec![("alice".to_owned(), hash, principal())],
            SESSION_TTL,
        );
        assert!(matches!(
            service.login("alice".to_owned(), "wrong".to_owned()).await,
            Err(AuthError::Unauthorized)
        ));
        let token = service
            .login("alice".to_owned(), "correct horse".to_owned())
            .await
            .expect("login");
        assert!(service.lookup(&token).is_some());
        service.revoke(&token);
        assert!(service.lookup(&token).is_none());
    }

    #[test]
    fn generated_hash_uses_the_required_argon2_profile() {
        let hash = make_hash(b"password").expect("hash");
        assert!(validate_hash(&hash).is_ok());
        let reordered = hash.replace("m=19456,t=2,p=1", "p=1,m=19456,t=2");
        assert!(validate_hash(&reordered).is_ok());
        assert!(validate_hash(&hash.replace("m=19456", "m=19457")).is_err());
    }

    #[test]
    fn registry_rejects_unknown_fields_and_duplicate_ids() {
        let hash = make_hash(b"password").expect("hash");
        let path = std::env::temp_dir().join(format!(
            "worldwright-server-auth-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let record = serde_json::json!({
            "username": "alice",
            "password_hash": hash,
            "account_id": "00000000000000000000000000000001",
            "character_id": "00000000000000000000000000000001",
            "character_name": "Alice"
        });
        let valid = serde_json::json!({"schema_version": 1, "accounts": [record]});
        std::fs::write(&path, serde_json::to_vec(&valid).expect("JSON")).expect("write");
        assert_eq!(
            AuthService::load(&path, false)
                .expect("registry")
                .principals()
                .len(),
            1
        );
        let unknown = serde_json::json!({"schema_version": 1, "accounts": [valid["accounts"][0].clone()], "extra": true});
        std::fs::write(&path, serde_json::to_vec(&unknown).expect("JSON")).expect("write");
        assert!(AuthService::load(&path, false).is_err());
        let duplicate = serde_json::json!({"schema_version": 1, "accounts": [valid["accounts"][0].clone(), valid["accounts"][0].clone()]});
        std::fs::write(&path, serde_json::to_vec(&duplicate).expect("JSON")).expect("write");
        assert!(AuthService::load(&path, false).is_err());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn registry_loader_rejects_bounds_and_invalid_records() {
        let hash = make_hash(b"password").expect("hash");
        let path = std::env::temp_dir().join(format!(
            "worldwright-server-auth-negative-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let record = serde_json::json!({
            "username": "alice",
            "password_hash": hash,
            "account_id": "00000000000000000000000000000001",
            "character_id": "00000000000000000000000000000001",
            "character_name": "Alice"
        });
        let write = |value: &[u8]| {
            std::fs::write(&path, value).expect("fixture");
            assert!(AuthService::load(&path, false).is_err());
        };
        write(b"");
        write(&vec![b' '; MAX_REGISTRY_BYTES + 1]);

        let mut many = Vec::with_capacity(257);
        for index in 0..257u128 {
            many.push(serde_json::json!({
                "username": format!("user{index}"),
                "password_hash": record["password_hash"],
                "account_id": format!("{:032x}", index + 1),
                "character_id": format!("{:032x}", index + 1),
                "character_name": format!("Character{index}"),
            }));
        }
        write(
            &serde_json::to_vec(&serde_json::json!({"schema_version": 1, "accounts": many}))
                .expect("JSON"),
        );

        for (field, value) in [
            ("account_id", "not-an-id"),
            ("character_id", "not-an-id"),
            ("username", "Alice"),
            ("character_name", "   "),
        ] {
            let mut invalid =
                serde_json::json!({"schema_version": 1, "accounts": [record.clone()]});
            invalid["accounts"][0][field] = serde_json::Value::String(value.to_owned());
            write(&serde_json::to_vec(&invalid).expect("JSON"));
        }
        let mut unsupported =
            serde_json::json!({"schema_version": 2, "accounts": [record.clone()]});
        write(&serde_json::to_vec(&unsupported).expect("JSON"));
        unsupported["schema_version"] = serde_json::json!(1);
        unsupported["accounts"][0]["password_hash"] = serde_json::Value::String(
            record["password_hash"]
                .as_str()
                .expect("hash")
                .replace("m=19456", "m=19457"),
        );
        write(&serde_json::to_vec(&unsupported).expect("JSON"));
        write(br#"{"schema_version":1,"schema_version":1,"accounts":[]}"#);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn session_capacity_prunes_expired_entries() {
        let hash = make_hash(b"password").expect("hash");
        let service = AuthService::from_records_with_capacity_for_test(
            vec![("alice".to_owned(), hash, principal())],
            SESSION_TTL,
            1,
        );
        let first = service
            .login("alice".to_owned(), "password".to_owned())
            .await
            .expect("login");
        assert!(matches!(
            service
                .login("alice".to_owned(), "password".to_owned())
                .await,
            Err(AuthError::SessionCapacity)
        ));
        service.revoke(&first);
        assert!(
            service
                .login("alice".to_owned(), "password".to_owned())
                .await
                .is_ok()
        );

        let hash = make_hash(b"password").expect("hash");
        let expired = AuthService::from_records_with_capacity_for_test(
            vec![("alice".to_owned(), hash, principal())],
            Duration::ZERO,
            1,
        );
        assert!(
            expired
                .login("alice".to_owned(), "password".to_owned())
                .await
                .is_ok()
        );
        assert!(
            expired
                .login("alice".to_owned(), "password".to_owned())
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn sessions_expire() {
        let hash = make_hash(b"password").expect("hash");
        let service = AuthService::from_records_for_test(
            vec![("alice".to_owned(), hash, principal())],
            Duration::ZERO,
        );
        let token = service
            .login("alice".to_owned(), "password".to_owned())
            .await
            .expect("login");
        assert!(service.lookup(&token).is_none());
    }
}
