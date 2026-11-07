use serde::Deserialize;
use std::{
    fmt,
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream},
    time::Duration,
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
        })
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
        let mut stream = TcpStream::connect_timeout(&self.address, HTTP_TIMEOUT)
            .map_err(|_| PublicationClientError::Unavailable)?;
        stream
            .set_read_timeout(Some(HTTP_TIMEOUT))
            .map_err(|_| PublicationClientError::Unavailable)?;
        stream
            .set_write_timeout(Some(HTTP_TIMEOUT))
            .map_err(|_| PublicationClientError::Unavailable)?;
        let bytes = body.unwrap_or_default();
        let mut request = format!(
            "{method} {path} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nConnection: close\r\nContent-Length: {}\r\n",
            self.authority,
            self.token,
            bytes.len()
        );
        if body.is_some() {
            request.push_str("Content-Type: application/vnd.worldwright.release-submission-v1\r\n");
        }
        request.push_str("\r\n");
        stream
            .write_all(request.as_bytes())
            .and_then(|()| stream.write_all(bytes))
            .and_then(|()| stream.flush())
            .map_err(|_| PublicationClientError::Unavailable)?;

        let mut response = Vec::new();
        stream
            .take((MAX_RESPONSE_BYTES + 1) as u64)
            .read_to_end(&mut response)
            .map_err(|_| PublicationClientError::Unavailable)?;
        if response.len() > MAX_RESPONSE_BYTES {
            return Err(PublicationClientError::InvalidResponse);
        }
        parse_http_response(&response)
    }
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
