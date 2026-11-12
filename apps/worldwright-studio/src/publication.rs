use serde::Deserialize;
use std::{
    fmt,
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use worldwright_core::ids::{PublicationId, WorldId};
use worldwright_operations::{
    PublicationView, ReleaseSubmissionMetadata, encode_release_submission,
};
use worldwright_release::{
    DeploymentTarget, PublicationKind, PublicationStatus, ReleaseId, Sha256Digest,
};

const MAX_RESPONSE_BYTES: usize = 64 * 1024;
const HTTP_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishRequest {
    pub artifact_digest: String,
    pub kind: PublicationKind,
    pub target: DeploymentTarget,
    pub expected_current: Option<Sha256Digest>,
    pub reason: String,
    pub change_summary: String,
    pub production_confirmation: Option<String>,
    pub allow_stale: bool,
}

#[derive(Clone)]
pub struct PublicationClient {
    address: SocketAddr,
    authority: String,
    token: String,
    token_file: Option<PathBuf>,
}

impl fmt::Debug for PublicationClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PublicationClient")
            .field("address", &self.address)
            .field("token", &"[redacted]")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationClientError {
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    Unavailable,
    InvalidRequest,
    InvalidResponse,
}

impl fmt::Display for PublicationClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unauthorized => "publication authentication failed",
            Self::Forbidden => "publication permission denied",
            Self::NotFound => "publication request was not found",
            Self::Conflict => "publication state conflicts with the request",
            Self::Unavailable => "publication service is unavailable",
            Self::InvalidRequest => "publication request is invalid",
            Self::InvalidResponse => "publication service returned an invalid response",
        })
    }
}

impl std::error::Error for PublicationClientError {}

impl PublicationClient {
    pub fn new(endpoint: String, token: String) -> Result<Self, PublicationClientError> {
        if token.is_empty() || token.len() > 4096 || !token.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(PublicationClientError::InvalidRequest);
        }
        let raw = endpoint
            .strip_prefix("http://")
            .ok_or(PublicationClientError::InvalidRequest)?;
        if raw.contains(['/', '?', '#', '@', '\r', '\n']) {
            return Err(PublicationClientError::InvalidRequest);
        }
        let (host, port) = raw
            .split_once(':')
            .ok_or(PublicationClientError::InvalidRequest)?;
        if host != "127.0.0.1" || port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit()) {
            return Err(PublicationClientError::InvalidRequest);
        }
        let port = port
            .parse::<u16>()
            .map_err(|_| PublicationClientError::InvalidRequest)?;
        if port == 0 {
            return Err(PublicationClientError::InvalidRequest);
        }
        Ok(Self {
            address: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
            authority: raw.to_owned(),
            token,
            token_file: None,
        })
    }

    /// The browser never receives a token or a filesystem path. Read a fresh
    /// token for each explicit operation; never automatically replay a POST.
    pub fn from_token_file(endpoint: String, path: PathBuf) -> Result<Self, PublicationClientError> {
        if !path.is_absolute() {
            return Err(PublicationClientError::InvalidRequest);
        }
        let mut client = Self::new(endpoint, read_token_file(&path)?)?;
        client.token.clear();
        client.token_file = Some(path);
        Ok(client)
    }

    pub fn renewable(&self) -> bool {
        self.token_file.is_some()
    }

    pub fn submit(
        &self,
        metadata: &ReleaseSubmissionMetadata,
        artifact: &[u8],
    ) -> Result<PublicationView, PublicationClientError> {
        let body = encode_release_submission(metadata, artifact)
            .map_err(|_| PublicationClientError::InvalidRequest)?;
        let response = self.request("POST", "/api/v1/operator/releases", Some(&body))?;
        serde_json::from_slice(&response).map_err(|_| PublicationClientError::InvalidResponse)
    }

    pub fn status(&self, request: &str) -> Result<PublicationView, PublicationClientError> {
        let request_id: PublicationId = request
            .parse()
            .map_err(|_| PublicationClientError::InvalidRequest)?;
        if request_id.to_string() != request {
            return Err(PublicationClientError::InvalidRequest);
        }
        let response =
            self.request("GET", &format!("/api/v1/operator/releases/{request}"), None)?;
        serde_json::from_slice(&response).map_err(|_| PublicationClientError::InvalidResponse)
    }

    fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&[u8]>,
    ) -> Result<Vec<u8>, PublicationClientError> {
        let token = match &self.token_file {
            Some(path) => read_token_file(path)?,
            None => self.token.clone(),
        };
        let deadline = Instant::now() + HTTP_TIMEOUT;
        let mut stream = TcpStream::connect_timeout(&self.address, remaining(deadline)?)
            .map_err(|_| PublicationClientError::Unavailable)?;
        let bytes = body.unwrap_or_default();
        let mut request = format!(
            "{method} {path} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nConnection: close\r\nContent-Length: {}\r\n",
            self.authority,
            token,
            bytes.len()
        );
        if body.is_some() {
            request.push_str("Content-Type: application/vnd.worldwright.release-submission-v1\r\n");
        }
        request.push_str("\r\n");
        write_before_deadline(&mut stream, request.as_bytes(), deadline)?;
        write_before_deadline(&mut stream, bytes, deadline)?;
        let response = read_before_deadline(&mut stream, deadline)?;
        parse_http_response(&response)
    }
}

fn remaining(deadline: Instant) -> Result<Duration, PublicationClientError> {
    deadline.checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .ok_or(PublicationClientError::Unavailable)
}

fn write_before_deadline(stream: &mut TcpStream, mut bytes: &[u8], deadline: Instant) -> Result<(), PublicationClientError> {
    while !bytes.is_empty() {
        stream.set_write_timeout(Some(remaining(deadline)?))
            .map_err(|_| PublicationClientError::Unavailable)?;
        match stream.write(bytes) {
            Ok(0) => return Err(PublicationClientError::Unavailable),
            Ok(count) => bytes = &bytes[count..],
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(PublicationClientError::Unavailable),
        }
    }
    remaining(deadline)?;
    Ok(())
}

fn read_before_deadline(stream: &mut TcpStream, deadline: Instant) -> Result<Vec<u8>, PublicationClientError> {
    let mut response = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        stream.set_read_timeout(Some(remaining(deadline)?))
            .map_err(|_| PublicationClientError::Unavailable)?;
        let limit = chunk.len().min(MAX_RESPONSE_BYTES + 1 - response.len());
        match stream.read(&mut chunk[..limit]) {
            Ok(0) => { remaining(deadline)?; return Ok(response); }
            Ok(count) => response.extend_from_slice(&chunk[..count]),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(PublicationClientError::Unavailable),
        }
        if response.len() > MAX_RESPONSE_BYTES {
            return Err(PublicationClientError::InvalidResponse);
        }
    }
}

fn read_token_file(path: &Path) -> Result<String, PublicationClientError> {
    use std::fs;
    let denied = || PublicationClientError::Unauthorized;
    for ancestor in path.ancestors() {
        let metadata = fs::symlink_metadata(ancestor).map_err(|_| denied())?;
        if metadata.file_type().is_symlink() {
            return Err(denied());
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err(denied());
            }
        }
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| denied())?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > 4098 {
        return Err(denied());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.mode() & 0o077 != 0 || metadata.nlink() != 1 {
            return Err(denied());
        }
        #[cfg(target_os = "linux")]
        if metadata.uid() != fs::metadata("/proc/self").map_err(|_| denied())?.uid() {
            return Err(denied());
        }
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_FLAG_OPEN_REPARSE_POINT; do not follow a swapped final component.
        // FILE_SHARE_READ permits concurrent reads but denies writes/deletes
        // until this short bounded credential read has completed.
        options.custom_flags(0x0020_0000).share_mode(1);
    }
    let file = options.open(path).map_err(|_| denied())?;
    let opened = file.metadata().map_err(|_| denied())?;
    if !opened.is_file() || opened.len() != metadata.len() {
        return Err(denied());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if opened.dev() != metadata.dev() || opened.ino() != metadata.ino()
            || opened.mode() & 0o077 != 0 || opened.nlink() != 1 || opened.uid() != metadata.uid()
        {
            return Err(denied());
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if opened.file_attributes() & 0x400 != 0 { return Err(denied()); }
    }
    let mut bytes = Vec::new();
    file.take(4099).read_to_end(&mut bytes).map_err(|_| denied())?;
    if bytes.len() as u64 != opened.len() {
        return Err(denied());
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| denied())?;
    let text = text.strip_suffix("\r\n").or_else(|| text.strip_suffix('\n')).unwrap_or(text);
    if text.is_empty() || text.len() > 4096 || !text.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err(denied());
    }
    Ok(text.to_owned())
}

fn parse_http_response(response: &[u8]) -> Result<Vec<u8>, PublicationClientError> {
    let marker = b"\r\n\r\n";
    let split = response
        .windows(marker.len())
        .position(|window| window == marker)
        .ok_or(PublicationClientError::InvalidResponse)?;
    let head = std::str::from_utf8(&response[..split])
        .map_err(|_| PublicationClientError::InvalidResponse)?;
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .ok_or(PublicationClientError::InvalidResponse)?;
    let mut status_parts = status.split(' ');
    if status_parts.next() != Some("HTTP/1.1") {
        return Err(PublicationClientError::InvalidResponse);
    }
    let code = status_parts
        .next()
        .ok_or(PublicationClientError::InvalidResponse)?
        .parse::<u16>()
        .map_err(|_| PublicationClientError::InvalidResponse)?;
    if status_parts.next().is_none() {
        return Err(PublicationClientError::InvalidResponse);
    }

    let mut content_length = None;
    let mut content_type = None;
    for line in lines {
        let (name, value) = line
            .split_once(':')
            .ok_or(PublicationClientError::InvalidResponse)?;
        let name = name.trim();
        let value = value.trim();
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(PublicationClientError::InvalidResponse);
        }
        if name.eq_ignore_ascii_case("content-length") {
            if content_length.is_some() {
                return Err(PublicationClientError::InvalidResponse);
            }
            content_length = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| PublicationClientError::InvalidResponse)?,
            );
        }
        if name.eq_ignore_ascii_case("content-type") {
            if content_type.is_some() {
                return Err(PublicationClientError::InvalidResponse);
            }
            content_type = Some(value);
        }
    }
    let body = &response[split + marker.len()..];
    if content_length != Some(body.len()) {
        return Err(PublicationClientError::InvalidResponse);
    }
    match code {
        200..=299 => {
            let media_type = content_type
                .and_then(|value| value.split(';').next())
                .map(str::trim);
            if media_type != Some("application/json") {
                return Err(PublicationClientError::InvalidResponse);
            }
            Ok(body.to_vec())
        }
        401 => Err(PublicationClientError::Unauthorized),
        403 => Err(PublicationClientError::Forbidden),
        404 => Err(PublicationClientError::NotFound),
        409 => Err(PublicationClientError::Conflict),
        500..=599 => Err(PublicationClientError::Unavailable),
        _ => Err(PublicationClientError::InvalidResponse),
    }
}

pub(crate) fn validate_publication_view(view: &PublicationView) -> Result<(), &'static str> {
    let request: PublicationId = view.request_id.parse().map_err(|_| "invalid request ID")?;
    if request.to_string() != view.request_id {
        return Err("noncanonical request ID");
    }
    let world: WorldId = view.world_id.parse().map_err(|_| "invalid world ID")?;
    if world.to_string() != view.world_id {
        return Err("noncanonical world ID");
    }
    let release: ReleaseId = view.release_id.parse().map_err(|_| "invalid release ID")?;
    if release.as_str() != view.release_id {
        return Err("noncanonical release ID");
    }
    let digest =
        Sha256Digest::parse(&view.artifact_digest).map_err(|_| "invalid artifact digest")?;
    if digest.to_string() != view.artifact_digest {
        return Err("noncanonical artifact digest");
    }
    validate_actor(&view.requested_by)?;
    if let Some(value) = &view.reviewed_by {
        validate_actor(value)?;
    }
    if let Some(value) = &view.activation_executor {
        validate_actor(value)?;
    }
    if let Some(value) = &view.review_reason {
        validate_text(value, 512)?;
    }
    if let Some(context) = &view.review_context {
        if context.schema_version != 1
            || context.draft_revision == 0
            || context.validation_diagnostic_count != 0
            || context.preview_completed_at == 0
            || Sha256Digest::parse(&context.snapshot_sha256).is_err()
            || Sha256Digest::parse(&context.preview_artifact_sha256).is_err()
        {
            return Err("invalid review context");
        }
        validate_text(&context.change_summary, 1024)?;
        if context.preview_artifact_sha256 != view.artifact_digest {
            return Err("review artifact mismatch");
        }
    }
    Ok(())
}

pub(crate) fn valid_status_transition(
    previous: PublicationStatus,
    next: PublicationStatus,
) -> bool {
    previous == next
        || matches!(
            (previous, next),
            (PublicationStatus::Requested, PublicationStatus::Approved)
                | (PublicationStatus::Requested, PublicationStatus::Rejected)
                | (PublicationStatus::Requested, PublicationStatus::Activating)
                | (PublicationStatus::Requested, PublicationStatus::Activated)
                | (PublicationStatus::Requested, PublicationStatus::Failed)
                | (PublicationStatus::Approved, PublicationStatus::Activating)
                | (PublicationStatus::Approved, PublicationStatus::Activated)
                | (PublicationStatus::Approved, PublicationStatus::Failed)
                | (PublicationStatus::Activating, PublicationStatus::Activated)
                | (PublicationStatus::Activating, PublicationStatus::Failed)
        )
}

pub(crate) fn validate_text(value: &str, max: usize) -> Result<(), &'static str> {
    if value.is_empty()
        || value.len() > max
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        Err("invalid text")
    } else {
        Ok(())
    }
}

fn validate_actor(value: &str) -> Result<(), &'static str> {
    if value.is_empty()
        || value.len() > 128
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        Err("invalid actor")
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_and_token_are_strict_and_redacted() {
        let client =
            PublicationClient::new("http://127.0.0.1:3000".into(), "secret-token".into()).unwrap();
        assert!(!format!("{client:?}").contains("secret-token"));
        for endpoint in [
            "https://127.0.0.1:3000",
            "http://localhost:3000",
            "http://127.0.0.1:0",
            "http://127.0.0.1:3000/",
            "http://127.0.0.1:3000?x",
        ] {
            assert!(PublicationClient::new(endpoint.into(), "token".into()).is_err());
        }
        assert!(
            PublicationClient::new("http://127.0.0.1:3000".into(), "bad token".into()).is_err()
        );
    }

    fn token_fixture() -> (PathBuf, PathBuf) {
        use std::fs;
        let root = std::env::temp_dir().join(format!("worldwright-r5-token-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let path = root.join("token");
        let mut options = fs::OpenOptions::new(); options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options.open(&path).unwrap().write_all(b"first-token\n").unwrap();
        (root, path)
    }

    #[test]
    fn r5_token_file_renewal_is_server_side_and_never_retries_unauthorized() {
        use std::{fs, net::TcpListener, thread};
        let (root, path) = token_fixture();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = PublicationClient::from_token_file(format!("http://{}", listener.local_addr().unwrap()), path.clone()).unwrap();
        assert!(client.renewable());
        assert!(!format!("{client:?}").contains("first-token"));
        let server = thread::spawn(move || {
            for (token, status) in [("first-token", "401 Unauthorized"), ("second-token", "200 OK")] {
                let (mut stream, _) = listener.accept().unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                let mut bytes = Vec::new(); let mut chunk = [0; 512];
                while !bytes.windows(4).any(|value| value == b"\r\n\r\n") {
                    let count = stream.read(&mut chunk).unwrap(); assert!(count > 0);
                    bytes.extend_from_slice(&chunk[..count]); assert!(bytes.len() < 8192);
                }
                assert!(String::from_utf8(bytes).unwrap().contains(&format!("Authorization: Bearer {token}\r\n")));
                write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}").unwrap();
            }
        });
        assert_eq!(client.request("GET", "/fixture", None), Err(PublicationClientError::Unauthorized));
        fs::write(&path, b"second-token\r\n").unwrap();
        assert_eq!(client.request("GET", "/fixture", None).unwrap(), b"{}");
        server.join().unwrap();
        fs::remove_file(&path).unwrap();
        assert_eq!(client.request("GET", "/fixture", None), Err(PublicationClientError::Unauthorized));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn r5_token_file_rejects_invalid_bytes_and_unsafe_files() {
        use std::fs;
        let (root, path) = token_fixture();
        for value in [Vec::new(), b"has space".to_vec(), b"two\nlines".to_vec(), vec![255], vec![b'x'; 4099]] {
            fs::write(&path, value).unwrap(); assert!(read_token_file(&path).is_err());
        }
        fs::write(&path, b"valid-token").unwrap();
        assert_eq!(read_token_file(&path).unwrap(), "valid-token");
        #[cfg(unix)]
        {
            use std::os::unix::fs::{symlink, PermissionsExt};
            let linked = root.join("linked"); symlink(&path, &linked).unwrap();
            assert!(read_token_file(&linked).is_err()); fs::remove_file(linked).unwrap();
            let linked = root.join("hard-link"); fs::hard_link(&path, &linked).unwrap();
            assert!(read_token_file(&path).is_err()); fs::remove_file(linked).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
            assert!(read_token_file(&path).is_err());
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn r5_total_deadline_rejects_trickled_response_and_expired_writes() {
        use std::{net::TcpListener, thread};
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (mut peer, _) = listener.accept().unwrap();
        let sender = thread::spawn(move || {
            peer.set_write_timeout(Some(Duration::from_millis(100))).unwrap();
            for _ in 0..50 {
                if peer.write_all(b"x").is_err() { break; }
                thread::sleep(Duration::from_millis(20));
            }
        });
        let start = Instant::now();
        assert_eq!(read_before_deadline(&mut client, start + Duration::from_millis(150)), Err(PublicationClientError::Unavailable));
        assert!(start.elapsed() < Duration::from_secs(2));
        assert_eq!(write_before_deadline(&mut client, b"never send", Instant::now()), Err(PublicationClientError::Unavailable));
        drop(client);
        sender.join().unwrap();
    }

    #[test]
    fn r5_network_response_cap_is_enforced_before_parsing() {
        use std::{net::TcpListener, thread};
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (mut peer, _) = listener.accept().unwrap();
        let sender = thread::spawn(move || {
            peer.set_write_timeout(Some(Duration::from_secs(1))).unwrap();
            let _ = peer.write_all(&vec![b'x'; MAX_RESPONSE_BYTES + 1]);
        });
        assert_eq!(read_before_deadline(&mut client, Instant::now() + Duration::from_secs(2)), Err(PublicationClientError::InvalidResponse));
        drop(client);
        sender.join().unwrap();
    }

    #[test]
    fn http_response_parser_is_bounded_and_strict() {
        let valid =
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}";
        assert_eq!(parse_http_response(valid).unwrap(), b"{}");
        for invalid in [
            &b"HTTP/1.0 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}"[..],
            &b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{}"[..],
            &b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 3\r\n\r\n{}"[..],
            &b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Length: 2\r\nContent-Type: application/json\r\n\r\n{}"[..],
            &b"HTTP/1.1 302 Found\r\nContent-Length: 0\r\n\r\n"[..],
        ] {
            assert_eq!(
                parse_http_response(invalid),
                Err(PublicationClientError::InvalidResponse)
            );
        }
        assert_eq!(
            parse_http_response(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n"),
            Err(PublicationClientError::Forbidden)
        );
        assert_eq!(
            parse_http_response(b"HTTP/1.1 409 Conflict\r\nContent-Length: 0\r\n\r\n"),
            Err(PublicationClientError::Conflict)
        );
    }
}
