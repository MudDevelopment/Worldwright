use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use rpassword::prompt_password;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    env,
    fs::{self, OpenOptions},
    io::{Read, Write},
    net::{IpAddr, TcpStream},
    path::{Path, PathBuf},
    process::ExitCode,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use worldwright_core::ids::{AccountId, CorrelationId, GrantId, OperatorSessionId, WorldId};
use worldwright_operations::{
    Capability, Environment, OperatorClaims, OperatorKind, issue_token, verify_totp,
};

const MAX_FILE: usize = 65_536;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CredentialFile {
    schema_version: u32,
    subject: String,
    kind: OperatorKind,
    password_hash: String,
    totp_secret_hex: Option<String>,
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("worldwright-console: {e}");
            ExitCode::from(2)
        }
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    if args.len() > 64 {
        return Err("too many arguments".into());
    }
    match args.first().map(String::as_str) {
        Some("--help") | Some("-h") => {
            print_help();
            Ok(())
        }
        Some("--version") | Some("-V") => {
            println!("worldwright-console {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("credential") => credential_command(&args[1..]),
        Some("token") => token_command(&args[1..]),
        Some("status") => api_command(&args[1..], "status"),
        Some("metrics") => api_command(&args[1..], "metrics"),
        Some("moderate") | Some("drain") | Some("session") | Some("operator") | Some("grant") => {
            api_command(&args, args[0].as_str())
        }
        _ => Err("use --help".into()),
    }
}

fn credential_command(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("create") => {
            let mut p = Params::new(&args[1..]);
            let output = PathBuf::from(p.req("--output")?);
            let subject = p.req("--subject")?;
            let kind = parse_kind(&p.req("--kind")?)?;
            p.finish()?;
            credential_create(&output, &subject, kind)
        }
        Some("status") => {
            let mut p = Params::new(&args[1..]);
            let path = PathBuf::from(p.req("--credential")?);
            p.finish()?;
            let c = read_credential(&path)?;
            println!(
                "subject: {}\nkind: {:?}\nTOTP: {}",
                c.subject,
                c.kind,
                if c.totp_secret_hex.is_some() {
                    "configured"
                } else {
                    "not configured"
                }
            );
            Ok(())
        }
        _ => Err("credential create|status is required".into()),
    }
}
fn token_command(args: &[String]) -> Result<(), String> {
    if args.first().map(String::as_str) != Some("issue") {
        return Err("token issue is required".into());
    }
    let mut p = Params::new(&args[1..]);
    let credential = PathBuf::from(p.req("--credential")?);
    let key = p.req("--signing-key-env")?;
    let issuer = p.req("--issuer")?;
    let audience = p.req("--audience")?;
    let reason = p.opt("--reason")?;
    p.finish()?;
    token_issue(&credential, &key, &issuer, &audience, reason.as_deref())
}

struct Params<'a> {
    args: &'a [String],
    used: Vec<bool>,
}
impl<'a> Params<'a> {
    fn new(args: &'a [String]) -> Self {
        Self {
            args,
            used: vec![false; args.len()],
        }
    }
    fn req(&mut self, n: &str) -> Result<String, String> {
        self.opt(n)?.ok_or(format!("{n} is required"))
    }
    fn opt(&mut self, n: &str) -> Result<Option<String>, String> {
        let mut got = None;
        for i in 0..self.args.len() {
            if !self.used[i] && self.args[i] == n {
                if got.is_some() {
                    return Err(format!("duplicate {n}"));
                }
                self.used[i] = true;
                let j = i + 1;
                if j >= self.args.len() || self.used[j] || self.args[j].starts_with("--") {
                    return Err(format!("{n} requires a value"));
                }
                self.used[j] = true;
                got = Some(self.args[j].clone())
            }
        }
        Ok(got)
    }
    fn take(&mut self) -> Option<String> {
        for i in 0..self.args.len() {
            if !self.used[i] {
                self.used[i] = true;
                return Some(self.args[i].clone());
            }
        }
        None
    }
    fn finish(&self) -> Result<(), String> {
        if self.used.iter().all(|v| *v) {
            Ok(())
        } else {
            Err("unknown option".into())
        }
    }
}

fn credential_create(path: &Path, subject: &str, kind: OperatorKind) -> Result<(), String> {
    validate_subject(subject)?;
    if path.exists() {
        return Err("credential output already exists".into());
    }
    let pw = prompt_password("Password: ").map_err(|_| "password prompt failed")?;
    let confirm = prompt_password("Confirm password: ").map_err(|_| "password prompt failed")?;
    if pw != confirm || pw.len() < 12 || pw.len() > 1024 || pw.chars().any(char::is_control) {
        return Err("password invalid".into());
    }
    let salt =
        SaltString::encode_b64(&random_bytes::<16>()?).map_err(|_| "salt generation failed")?;
    let hash = Argon2::default()
        .hash_password(pw.as_bytes(), &salt)
        .map_err(|_| "password hashing failed")?
        .to_string();
    let secret = if kind == OperatorKind::Service {
        None
    } else {
        Some(encode_hex(&random_bytes::<32>()?))
    };
    let c = CredentialFile {
        schema_version: 1,
        subject: subject.into(),
        kind,
        password_hash: hash,
        totp_secret_hex: secret.clone(),
    };
    let bytes = serde_json::to_vec_pretty(&c).map_err(|_| "serialization failed")?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|_| "credential output cannot be created")?;
    if file
        .write_all(&bytes)
        .and_then(|_| file.sync_all())
        .is_err()
    {
        drop(file);
        let _ = fs::remove_file(path);
        return Err("credential write failed".into());
    }
    if let Some(s) = secret {
        println!("TOTP secret (save once): {s}");
        println!(
            "otpauth://totp/Worldwright:{}?secret={}&issuer=Worldwright&algorithm=SHA256&digits=8&period=30",
            subject,
            base32(&decode_hex(&s)?)
        );
    }
    #[cfg(windows)]
    eprintln!("warning: Windows ACLs were not changed; protect credential file manually");
    Ok(())
}
fn token_issue(
    path: &Path,
    key_env: &str,
    issuer: &str,
    audience: &str,
    reason: Option<&str>,
) -> Result<(), String> {
    let c = read_credential(path)?;
    let pw = prompt_password("Password: ").map_err(|_| "password prompt failed")?;
    let ph = PasswordHash::new(&c.password_hash).map_err(|_| "credential hash invalid")?;
    Argon2::default()
        .verify_password(pw.as_bytes(), &ph)
        .map_err(|_| "password rejected")?;
    if c.kind == OperatorKind::BreakGlass && reason.is_none() {
        return Err("break-glass reason required".into());
    }
    if c.kind != OperatorKind::BreakGlass && reason.is_some() {
        return Err("reason only valid for break-glass".into());
    }
    let t = now()?;
    let assurance = match c.kind {
        OperatorKind::Service => worldwright_operations::Assurance::Service,
        _ => {
            let code = prompt_password("TOTP: ").map_err(|_| "TOTP prompt failed")?;
            let sec = decode_hex(c.totp_secret_hex.as_ref().ok_or("TOTP secret missing")?)?;
            verify_totp(&sec, &code, t).map_err(|_| "TOTP rejected")?;
            if c.kind == OperatorKind::BreakGlass {
                worldwright_operations::Assurance::BreakGlass
            } else {
                worldwright_operations::Assurance::Mfa
            }
        }
    };
    let sid = OperatorSessionId::from_u128(u128::from_be_bytes(random_bytes::<16>()?))
        .map_err(|_| "session generation failed")?;
    let claims = OperatorClaims {
        issuer: issuer.into(),
        audience: audience.into(),
        subject: c.subject,
        session_id: sid.to_string(),
        kind: c.kind,
        assurance,
        issued_at: t,
        expires_at: t + if c.kind == OperatorKind::Service {
            300
        } else {
            600
        },
        auth_time: t,
        break_glass_reason: reason.map(str::to_owned),
    };
    let key = decode_hex(&env::var(key_env).map_err(|_| "signing key unavailable")?)?;
    println!(
        "{}",
        issue_token(&claims, &key).map_err(|_| "token issuance failed")?
    );
    Ok(())
}
fn read_credential(p: &Path) -> Result<CredentialFile, String> {
    let m = fs::metadata(p).map_err(|_| "credential cannot be read")?;
    if !m.is_file() || m.len() == 0 || m.len() > MAX_FILE as u64 {
        return Err("credential bounds invalid".into());
    }
    let c: CredentialFile =
        serde_json::from_slice(&fs::read(p).map_err(|_| "credential cannot be read")?)
            .map_err(|_| "credential invalid")?;
    if c.schema_version != 1 {
        Err("credential schema invalid".into())
    } else {
        validate_subject(&c.subject)?;
        if c.password_hash.len() > 512 || PasswordHash::new(&c.password_hash).is_err() {
            return Err("credential hash invalid".into());
        }
        if c.kind == OperatorKind::Service && c.totp_secret_hex.is_some() {
            return Err("service TOTP invalid".into());
        }
        if c.kind != OperatorKind::Service
            && c.totp_secret_hex.as_deref().is_none_or(|s| {
                s.len() != 64
                    || !s
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            })
        {
            return Err("TOTP secret invalid".into());
        }
        Ok(c)
    }
}
fn validate_subject(s: &str) -> Result<(), String> {
    let b = s.as_bytes();
    if b.is_empty()
        || b.len() > 128
        || !b[0].is_ascii_lowercase()
        || b[1..]
            .iter()
            .any(|c| !c.is_ascii_lowercase() && !c.is_ascii_digit() && !b"._-".contains(c))
    {
        Err("subject invalid".into())
    } else {
        Ok(())
    }
}
fn parse_kind(s: &str) -> Result<OperatorKind, String> {
    serde_json::from_value(serde_json::Value::String(s.into()))
        .map_err(|_| "operator kind invalid".into())
}
fn now() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_secs())
        .map_err(|_| "clock unavailable".into())
}
fn random_bytes<const N: usize>() -> Result<[u8; N], String> {
    let mut b = [0; N];
    getrandom::fill(&mut b).map_err(|_| "random failed".to_owned())?;
    Ok(b)
}
fn encode_hex(b: &[u8]) -> String {
    let mut output = String::with_capacity(b.len() * 2);
    for value in b {
        use std::fmt::Write as _;
        write!(output, "{value:02x}").expect("String formatting cannot fail");
    }
    output
}
fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0
        || !s
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err("hex invalid".into());
    }
    s.as_bytes()
        .chunks(2)
        .map(|p| {
            u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16)
                .map_err(|_| "hex invalid".into())
        })
        .collect()
}
fn base32(b: &[u8]) -> String {
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut o = String::new();
    let (mut a, mut n) = (0u32, 0);
    for x in b {
        a = (a << 8) | u32::from(*x);
        n += 8;
        while n >= 5 {
            n -= 5;
            o.push(A[((a >> n) & 31) as usize] as char)
        }
    }
    if n > 0 {
        o.push(A[((a << (5 - n)) & 31) as usize] as char)
    }
    o
}
fn api_command(args: &[String], op: &str) -> Result<(), String> {
    let mut p = Params::new(if op == "status" || op == "metrics" {
        args
    } else {
        &args[1..]
    });
    let endpoint = p.req("--endpoint")?;
    let token_env = p.req("--token-env")?;
    let body = match op {
        "status" | "metrics" => {
            p.finish()?;
            None
        }
        "moderate" => {
            let account: AccountId = p.req("--account")?.parse().map_err(|_| "invalid account")?;
            let disabled = match p.req("--disabled")?.as_str() {
                "true" => true,
                "false" => false,
                _ => return Err("disabled must be true or false".into()),
            };
            let reason = p.req("--reason")?;
            let body = json!({"account_id":account.to_string(),"disabled":disabled,"reason":reason,"correlation_id":random_corr()?});
            p.finish()?;
            Some(("/api/v1/operator/moderation", body))
        }
        "drain" => {
            let reason = p.req("--reason")?;
            let body = json!({"reason":reason,"correlation_id":random_corr()?});
            p.finish()?;
            Some(("/api/v1/operator/drain", body))
        }
        "session" => {
            if p.take() != Some("revoke".into()) {
                return Err("session revoke required".into());
            }
            let subject = p.req("--subject")?;
            let session: OperatorSessionId =
                p.req("--session")?.parse().map_err(|_| "invalid session")?;
            let reason = p.req("--reason")?;
            let body = json!({"subject":subject,"session_id":session.to_string(),"reason":reason,"correlation_id":random_corr()?});
            p.finish()?;
            Some(("/api/v1/operator/sessions/revoke", body))
        }
        "operator" => {
            let action = p.take().ok_or("operator create|enable|disable required")?;
            if action == "create" {
                let subject = p.req("--subject")?;
                let display_name = p.req("--display-name")?;
                let kind: OperatorKind =
                    serde_json::from_value(serde_json::Value::String(p.req("--kind")?))
                        .map_err(|_| "invalid operator kind")?;
                let reason = p.req("--reason")?;
                let body = json!({"subject":subject,"display_name":display_name,"kind":kind,"reason":reason,"correlation_id":random_corr()?});
                p.finish()?;
                Some(("/api/v1/operator/operators", body))
            } else {
                let enabled = match action.as_str() {
                    "enable" => true,
                    "disable" => false,
                    _ => return Err("operator enable|disable required".into()),
                };
                let subject = p.req("--subject")?;
                let reason = p.req("--reason")?;
                let body = json!({"subject":subject,"enabled":enabled,"reason":reason,"correlation_id":random_corr()?});
                p.finish()?;
                Some(("/api/v1/operator/operators/enabled", body))
            }
        }
        "grant" => {
            let action = p.take().ok_or("grant create|revoke required")?;
            let grant: GrantId = p.req("--grant")?.parse().map_err(|_| "invalid grant")?;
            let reason = p.req("--reason")?;
            if action == "revoke" {
                let body = json!({"grant_id":grant.to_string(),"reason":reason,"correlation_id":random_corr()?});
                p.finish()?;
                Some(("/api/v1/operator/grants/revoke", body))
            } else if action == "create" {
                let subject = p.req("--subject")?;
                let capability: Capability =
                    serde_json::from_value(serde_json::Value::String(p.req("--capability")?))
                        .map_err(|_| "invalid capability")?;
                let environment: Environment =
                    serde_json::from_value(serde_json::Value::String(p.req("--environment")?))
                        .map_err(|_| "invalid environment")?;
                let world: WorldId = p.req("--world")?.parse().map_err(|_| "invalid world")?;
                let expires = p
                    .opt("--expires-seconds")?
                    .map(|v| v.parse::<u64>().map_err(|_| "invalid expiry"))
                    .transpose()?;
                if expires.is_some_and(|v| !(1..=31_536_000).contains(&v)) {
                    return Err("expiry out of bounds".into());
                }
                let body = json!({"grant_id":grant.to_string(),"subject":subject,"capability":capability,"environment":environment,"world_id":world.to_string(),"expires_seconds":expires,"reason":reason,"correlation_id":random_corr()?});
                p.finish()?;
                Some(("/api/v1/operator/grants", body))
            } else {
                return Err("grant create|revoke required".into());
            }
        }
        _ => return Err("unknown API command".into()),
    };
    let token = env::var(token_env).map_err(|_| "token environment unavailable".to_owned())?;
    let (path, method, bytes) = match body {
        None => (
            if op == "status" {
                "/api/v1/operator/status"
            } else {
                "/api/v1/operator/metrics"
            },
            "GET",
            None,
        ),
        Some((path, value)) => (
            path,
            "POST",
            Some(serde_json::to_vec(&value).map_err(|_| "request serialization failed")?),
        ),
    };
    http_request(&endpoint, &token, method, path, bytes.as_deref())
}

fn random_corr() -> Result<String, String> {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).map_err(|_| "random generation failed")?;
    let id =
        CorrelationId::from_u128(u128::from_be_bytes(b)).map_err(|_| "random generation failed")?;
    Ok(id.to_string())
}
fn http_request(
    endpoint: &str,
    token: &str,
    method: &str,
    path: &str,
    body: Option<&[u8]>,
) -> Result<(), String> {
    if token.is_empty() || token.len() > 4096 || !token.bytes().all(|b| b.is_ascii_graphic()) {
        return Err("token invalid".into());
    }
    let (host, port) = endpoint_host(endpoint)?;
    let mut stream = TcpStream::connect_timeout(
        &(
            host.parse::<IpAddr>().map_err(|_| "endpoint invalid")?,
            port,
        )
            .into(),
        Duration::from_secs(5),
    )
    .map_err(|_| "connection failed")?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|_| "connection failed")?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|_| "connection failed")?;
    let mut req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}:{port}\r\nAuthorization: Bearer {token}\r\nConnection: close\r\n"
    );
    if let Some(b) = body {
        req.push_str(&format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            b.len()
        ))
    }
    req.push_str("\r\n");
    if req.len() + body.map_or(0, |b| b.len()) > 16 * 1024 {
        return Err("request too large".into());
    }
    stream
        .write_all(req.as_bytes())
        .map_err(|_| "request failed")?;
    if let Some(b) = body {
        stream.write_all(b).map_err(|_| "request failed")?
    }
    let mut data = Vec::new();
    stream
        .take(65_537)
        .read_to_end(&mut data)
        .map_err(|_| "response failed")?;
    if data.len() > 65_536 {
        return Err("response too large".into());
    }
    let (code, body) = parse_response(&data)?;
    if !(200..300).contains(&code) {
        return Err(format!("request failed with HTTP status {code}"));
    }
    if body.is_empty() {
        println!("ok")
    } else {
        let body = std::str::from_utf8(body).map_err(|_| "response body is not UTF-8")?;
        print!("{body}")
    }
    Ok(())
}
fn endpoint_host(endpoint: &str) -> Result<(String, u16), String> {
    let x = endpoint
        .strip_prefix("http://")
        .ok_or("endpoint must be loopback HTTP")?;
    if x.contains(['@', '?', '#', '/', '\r', '\n']) {
        return Err("endpoint invalid".into());
    }
    let (h, p) = x.rsplit_once(':').ok_or("endpoint requires port")?;
    if !h
        .parse::<IpAddr>()
        .map_err(|_| "endpoint invalid")?
        .is_loopback()
    {
        return Err("endpoint must be loopback".into());
    }
    let port: u16 = p.parse().map_err(|_| "endpoint invalid")?;
    if port == 0 || h.contains('[') || h.contains(']') {
        return Err("endpoint invalid".into());
    }
    Ok((h.into(), port))
}

fn parse_response(data: &[u8]) -> Result<(u16, &[u8]), String> {
    let marker = b"\r\n\r\n";
    let split = data
        .windows(marker.len())
        .position(|w| w == marker)
        .ok_or("invalid response")?;
    let header =
        std::str::from_utf8(&data[..split]).map_err(|_| "response headers are not UTF-8")?;
    let body = &data[split + marker.len()..];
    let mut lines = header.split("\r\n");
    let status = lines.next().ok_or("invalid response")?;
    let mut parts = status.splitn(3, ' ');
    let version = parts.next().ok_or("invalid response status")?;
    let code_text = parts.next().ok_or("invalid response status")?;
    let reason = parts.next().ok_or("invalid response status")?;
    if !matches!(version, "HTTP/1.0" | "HTTP/1.1")
        || code_text.len() != 3
        || !code_text.bytes().all(|byte| byte.is_ascii_digit())
        || reason.is_empty()
        || reason.trim() != reason
        || !reason
            .bytes()
            .all(|byte| byte == b' ' || byte.is_ascii_graphic())
    {
        return Err("invalid response status".into());
    }
    let code: u16 = code_text.parse().map_err(|_| "invalid response status")?;
    let mut content_length = None;
    for line in lines {
        let (name, value) = line.split_once(':').ok_or("invalid response header")?;
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
            || value.trim().is_empty()
        {
            return Err("invalid response header".into());
        }
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err("invalid response framing".into());
        }
        if name.eq_ignore_ascii_case("content-length") {
            if content_length.is_some() {
                return Err("invalid response framing".into());
            }
            let length: usize = value
                .trim()
                .parse()
                .map_err(|_| "invalid response framing")?;
            if length > 65_536 || length != body.len() {
                return Err("invalid response framing".into());
            }
            content_length = Some(length);
        }
    }
    if (100..200).contains(&code) {
        return Err("invalid response status".into());
    }
    Ok((code, body))
}
fn print_help() {
    println!("Worldwright Console");
    println!("Usage:");
    println!("  worldwright-console credential <create|status> ...");
    println!("  worldwright-console token issue ...");
    println!("  worldwright-console <status|metrics> --endpoint <loopback-http> --token-env <ENV>");
    println!("  worldwright-console moderate|drain|session ...");
    println!("  worldwright-console operator <create|enable|disable> ...");
    println!("  worldwright-console grant <create|revoke> ...");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_framing_is_strict() {
        assert_eq!(
            parse_response(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                .unwrap()
                .0,
            200
        );
        assert_eq!(
            parse_response(b"HTTP/1.1 204 No Content\r\n\r\n")
                .unwrap()
                .0,
            204
        );
        assert!(parse_response(b"HTTP/1.1 100 Continue\r\n\r\n").is_err());
        assert!(parse_response(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nx").is_err());
        assert!(
            parse_response(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nContent-Length: 1\r\n\r\nx")
                .is_err()
        );
        assert!(parse_response(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nx").is_err());
        assert!(parse_response(b"HTTP/1.1 200 OK\r\nBadHeader\r\n\r\n").is_err());
        assert!(parse_response(b"HTTP/1.1 200\r\n\r\n").is_err());
        assert!(parse_response(b"HTTP/1.1 200 OK\r\nX: \xff\r\n\r\n").is_err());
    }

    #[test]
    fn endpoint_and_base32_bounds() {
        assert!(endpoint_host("http://127.0.0.1:3000").is_ok());
        for value in [
            "https://127.0.0.1:3000",
            "http://8.8.8.8:3000",
            "http://127.0.0.1:0",
            "http://user@127.0.0.1:3000",
            "http://127.0.0.1:3000/x",
            "http://[::1]:3000",
            "http://127.0.0.1:3000\r\n",
        ] {
            assert!(endpoint_host(value).is_err(), "{value}");
        }
        assert_eq!(base32(b"foobar"), "MZXW6YTBOI");
    }

    #[test]
    fn credential_json_is_strict_and_bounded() {
        let valid = CredentialFile {
            schema_version: 1,
            subject: "alice".into(),
            kind: OperatorKind::Service,
            password_hash: "$argon2id$v=19$m=19456,t=2,p=1$c2FsdHNhbHQ$QmFkSGFzaA".into(),
            totp_secret_hex: None,
        };
        let mut bytes = serde_json::to_vec(&valid).unwrap();
        bytes.extend_from_slice(b" ");
        assert!(serde_json::from_slice::<CredentialFile>(&bytes).is_ok());
        let unknown = br#"{"schema_version":1,"subject":"alice","kind":"service","password_hash":"x","totp_secret_hex":null,"extra":1}"#;
        assert!(serde_json::from_slice::<CredentialFile>(unknown).is_err());
        let mut path = std::env::temp_dir();
        path.push(format!(
            "worldwright-console-test-{}-credential.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, b"").unwrap();
        assert!(read_credential(&path).is_err());
        let _ = std::fs::remove_file(&path);
    }
}
