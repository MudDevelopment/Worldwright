//! Pure, bounded contracts for authenticated Worldwright operator operations.
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::{error::Error, fmt, str::FromStr};
use worldwright_core::ids::{AccountId, CorrelationId, OperatorSessionId};

pub const TOKEN_PREFIX: &str = "worldwright-operator-v1";
pub const MAX_TOKEN_BYTES: usize = 4096;
pub const TOKEN_TTL_SECONDS: u64 = 600;
pub const SERVICE_TOKEN_TTL_SECONDS: u64 = 300;
pub const MAX_CLOCK_SKEW_SECONDS: u64 = 30;
pub const MAX_AUTH_AGE_SECONDS: u64 = 300;
pub const TOTP_STEP_SECONDS: u64 = 30;
pub const TOTP_DIGITS: u32 = 8;
pub const MIN_SIGNING_KEY_BYTES: usize = 32;
pub const RELEASE_SUBMISSION_VERSION: u32 = 1;
pub const MAX_RELEASE_SUBMISSION_METADATA_BYTES: usize = 4096;
pub const RELEASE_SUBMISSION_PREFIX: &[u8] = b"WORLDWRIGHT-RELEASE-SUBMISSION-V1\n";
pub const MAX_RELEASE_SUBMISSION_BYTES: usize = RELEASE_SUBMISSION_PREFIX.len()
    + 4
    + MAX_RELEASE_SUBMISSION_METADATA_BYTES
    + worldwright_release::MAX_ARTIFACT_BYTES;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StudioReviewContext {
    pub schema_version: u32,
    pub draft_revision: u64,
    pub snapshot_sha256: String,
    pub validation_diagnostic_count: u32,
    pub preview_artifact_sha256: String,
    pub preview_completed_at: u64,
    pub change_summary: String,
}

impl StudioReviewContext {
    pub fn validate_for_artifact(&self, artifact: &[u8]) -> Result<(), OperationsError> {
        if self.schema_version != 1
            || self.draft_revision == 0
            || self.preview_completed_at == 0
            || self.validation_diagnostic_count != 0
            || decode_hex_string(&self.snapshot_sha256).is_err()
        {
            return Err(OperationsError::InvalidRequest);
        }
        validate_reason_text(&self.change_summary, 1024)?;
        let verified = worldwright_release::verify_bytes("studio review artifact", artifact, None)
            .map_err(|_| OperationsError::InvalidRequest)?;
        if verified.manifest().payload_sha256().to_string() != self.snapshot_sha256
            || verified.artifact_digest().to_string() != self.preview_artifact_sha256
        {
            return Err(OperationsError::InvalidRequest);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseSubmissionMetadata {
    pub schema_version: u32,
    pub kind: worldwright_release::PublicationKind,
    pub target: worldwright_release::DeploymentTarget,
    pub expected_current: Option<worldwright_release::Sha256Digest>,
    pub reason: String,
    pub correlation_id: String,
    pub review_context: StudioReviewContext,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationView {
    pub request_id: String,
    pub world_id: String,
    pub release_id: String,
    pub artifact_digest: String,
    pub kind: worldwright_release::PublicationKind,
    pub target: worldwright_release::DeploymentTarget,
    pub expected_current: Option<worldwright_release::Sha256Digest>,
    pub requested_by: String,
    pub status: worldwright_release::PublicationStatus,
    pub reviewed_by: Option<String>,
    pub review_reason: Option<String>,
    pub activation_executor: Option<String>,
    pub review_context: Option<StudioReviewContext>,
}

pub struct DecodedReleaseSubmission<'a> {
    pub metadata: ReleaseSubmissionMetadata,
    pub artifact: &'a [u8],
}

pub fn encode_release_submission(
    metadata: &ReleaseSubmissionMetadata,
    artifact: &[u8],
) -> Result<Vec<u8>, OperationsError> {
    validate_submission(metadata, artifact)?;
    let metadata = serde_json::to_vec(metadata).map_err(|_| OperationsError::Serialization)?;
    if metadata.len() > MAX_RELEASE_SUBMISSION_METADATA_BYTES {
        return Err(OperationsError::InvalidRequest);
    }
    let total = RELEASE_SUBMISSION_PREFIX.len() + 4 + metadata.len() + artifact.len();
    if artifact.is_empty() || total > MAX_RELEASE_SUBMISSION_BYTES {
        return Err(OperationsError::InvalidRequest);
    }
    let mut output = Vec::with_capacity(total);
    output.extend_from_slice(RELEASE_SUBMISSION_PREFIX);
    output.extend_from_slice(&(metadata.len() as u32).to_be_bytes());
    output.extend_from_slice(&metadata);
    output.extend_from_slice(artifact);
    Ok(output)
}

pub fn decode_release_submission(
    bytes: &[u8],
) -> Result<DecodedReleaseSubmission<'_>, OperationsError> {
    if bytes.len() < RELEASE_SUBMISSION_PREFIX.len() + 4
        || bytes.len() > MAX_RELEASE_SUBMISSION_BYTES
        || !bytes.starts_with(RELEASE_SUBMISSION_PREFIX)
    {
        return Err(OperationsError::InvalidRequest);
    }
    let offset = RELEASE_SUBMISSION_PREFIX.len();
    let metadata_len = u32::from_be_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("length checked"),
    ) as usize;
    if metadata_len == 0 || metadata_len > MAX_RELEASE_SUBMISSION_METADATA_BYTES {
        return Err(OperationsError::InvalidRequest);
    }
    let start = offset + 4;
    let end = start
        .checked_add(metadata_len)
        .ok_or(OperationsError::InvalidRequest)?;
    if end >= bytes.len() {
        return Err(OperationsError::InvalidRequest);
    }
    let metadata: ReleaseSubmissionMetadata =
        serde_json::from_slice(&bytes[start..end]).map_err(|_| OperationsError::InvalidRequest)?;
    let artifact = &bytes[end..];
    validate_submission(&metadata, artifact)?;
    Ok(DecodedReleaseSubmission { metadata, artifact })
}

fn validate_submission(
    metadata: &ReleaseSubmissionMetadata,
    artifact: &[u8],
) -> Result<(), OperationsError> {
    if metadata.schema_version != RELEASE_SUBMISSION_VERSION || artifact.is_empty() {
        return Err(OperationsError::InvalidRequest);
    }
    validate_reason_text(&metadata.reason, 512)?;
    metadata.review_context.validate_for_artifact(artifact)?;
    let correlation: CorrelationId = metadata
        .correlation_id
        .parse()
        .map_err(|_| OperationsError::InvalidRequest)?;
    if correlation.get() == 0 {
        return Err(OperationsError::InvalidRequest);
    }
    Ok(())
}

fn validate_reason_text(value: &str, max: usize) -> Result<(), OperationsError> {
    if value.is_empty()
        || value.len() > max
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        Err(OperationsError::InvalidRequest)
    } else {
        Ok(())
    }
}
pub const BACKUP_MANIFEST_VERSION: u32 = 1;
pub const RPO_TARGET_SECONDS: u64 = 86_400;
pub const RTO_TARGET_SECONDS: u64 = 14_400;
pub const AUDIT_RETENTION_DAYS: u32 = 90;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BackupManifest {
    pub schema_version: u32,
    pub backup_id: String,
    pub world_id: String,
    pub created_at: u64,
    pub application_version: String,
    pub active_release: Option<String>,
    pub authority_incarnation: String,
    pub writer_epoch: u64,
    pub backup_sha256: String,
    pub rpo_target_seconds: u64,
    pub rto_target_seconds: u64,
    pub audit_retention_days: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SignedBackupManifest {
    pub manifest: BackupManifest,
    pub signature: String,
}

pub fn sign_backup_manifest(
    manifest: &BackupManifest,
    key: &[u8],
) -> Result<SignedBackupManifest, OperationsError> {
    validate_backup_manifest(manifest)?;
    validate_key(key)?;
    let bytes = serde_json::to_vec(manifest).map_err(|_| OperationsError::Serialization)?;
    let mut input = b"worldwright-backup-manifest-v1\0".to_vec();
    input.extend_from_slice(&bytes);
    Ok(SignedBackupManifest {
        manifest: manifest.clone(),
        signature: URL_SAFE_NO_PAD.encode(sign(&input, key)?),
    })
}

pub fn verify_backup_manifest(
    signed: &SignedBackupManifest,
    key: &[u8],
    backup_digest: [u8; 32],
) -> Result<(), OperationsError> {
    validate_backup_manifest(&signed.manifest)?;
    validate_key(key)?;
    let expected_digest = encode_hex(&backup_digest);
    if signed.manifest.backup_sha256 != expected_digest {
        return Err(OperationsError::InvalidClaims);
    }
    let bytes = serde_json::to_vec(&signed.manifest).map_err(|_| OperationsError::Serialization)?;
    let mut input = b"worldwright-backup-manifest-v1\0".to_vec();
    input.extend_from_slice(&bytes);
    let signature = URL_SAFE_NO_PAD
        .decode(&signed.signature)
        .map_err(|_| OperationsError::InvalidToken)?;
    let expected = sign(&input, key)?;
    if !constant_time_equal(&signature, &expected) {
        return Err(OperationsError::InvalidToken);
    }
    Ok(())
}

fn validate_backup_manifest(m: &BackupManifest) -> Result<(), OperationsError> {
    if m.schema_version != BACKUP_MANIFEST_VERSION
        || m.created_at == 0
        || m.rpo_target_seconds != RPO_TARGET_SECONDS
        || m.rto_target_seconds != RTO_TARGET_SECONDS
        || m.audit_retention_days != AUDIT_RETENTION_DAYS
        || m.writer_epoch == 0
        || m.application_version.is_empty()
        || m.application_version.len() > 64
        || !m.application_version.bytes().all(|b| b.is_ascii_graphic())
        || decode_id_string(&m.backup_id).is_err()
        || decode_id_string(&m.world_id).is_err()
        || decode_id_string(&m.authority_incarnation).is_err()
        || decode_hex_string(&m.backup_sha256).is_err()
        || m.active_release
            .as_ref()
            .is_some_and(|v| decode_hex_string(v).is_err())
    {
        return Err(OperationsError::InvalidClaims);
    }
    Ok(())
}
fn decode_id_string(value: &str) -> Result<(), OperationsError> {
    if value.len() != 32
        || !value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        || value.chars().all(|c| c == '0')
    {
        Err(OperationsError::InvalidClaims)
    } else {
        Ok(())
    }
}
fn decode_hex_string(value: &str) -> Result<(), OperationsError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        Err(OperationsError::InvalidClaims)
    } else {
        Ok(())
    }
}

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorKind {
    Human,
    Service,
    BreakGlass,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Assurance {
    Mfa,
    Service,
    BreakGlass,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    ViewStatus,
    ModerateAccount,
    RequestRelease,
    ReviewRelease,
    ActivateRelease,
    DrainWorld,
    InitiateRecovery,
    ManageOperators,
    BackupWorld,
    RestoreWorld,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Environment {
    Staging,
    Production,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorClaims {
    pub issuer: String,
    pub audience: String,
    pub subject: String,
    pub session_id: String,
    pub kind: OperatorKind,
    pub assurance: Assurance,
    pub issued_at: u64,
    pub expires_at: u64,
    pub auth_time: u64,
    pub break_glass_reason: Option<String>,
}

impl OperatorClaims {
    pub fn session_id(&self) -> Result<OperatorSessionId, OperationsError> {
        let id: OperatorSessionId = decode_id(&self.session_id)?;
        if id.to_string() != self.session_id {
            return Err(OperationsError::InvalidClaims);
        }
        Ok(id)
    }

    pub fn validate_at(
        &self,
        expected_issuer: &str,
        expected_audience: &str,
        now: u64,
    ) -> Result<(), OperationsError> {
        validate_claims(self)?;
        if self.issuer != expected_issuer || self.audience != expected_audience {
            return Err(OperationsError::InvalidClaims);
        }
        if self.issued_at > now.saturating_add(MAX_CLOCK_SKEW_SECONDS)
            || self.expires_at.saturating_add(MAX_CLOCK_SKEW_SECONDS) < now
            || self.auth_time > now
            || now.saturating_sub(self.auth_time) > MAX_AUTH_AGE_SECONDS
        {
            return Err(OperationsError::InvalidClaims);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModerationRequest {
    pub account_id: String,
    pub disabled: bool,
    pub reason: String,
    pub correlation_id: String,
}

impl ModerationRequest {
    pub fn account_id(&self) -> Result<AccountId, OperationsError> {
        decode_id(&self.account_id)
    }
    pub fn correlation_id(&self) -> Result<CorrelationId, OperationsError> {
        decode_id(&self.correlation_id)
    }
    pub fn validate(&self) -> Result<(), OperationsError> {
        self.account_id()?;
        self.correlation_id()?;
        validate_reason(&self.reason)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DrainRequest {
    pub reason: String,
    pub correlation_id: String,
}

impl DrainRequest {
    pub fn correlation_id(&self) -> Result<CorrelationId, OperationsError> {
        decode_id(&self.correlation_id)
    }
    pub fn validate(&self) -> Result<(), OperationsError> {
        self.correlation_id()?;
        validate_reason(&self.reason)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevokeOperatorSessionRequest {
    pub session_id: String,
    pub reason: String,
    pub correlation_id: String,
}

impl RevokeOperatorSessionRequest {
    pub fn session_id(&self) -> Result<OperatorSessionId, OperationsError> {
        let id: OperatorSessionId = decode_id(&self.session_id)?;
        if id.to_string() != self.session_id {
            return Err(OperationsError::InvalidRequest);
        }
        Ok(id)
    }
    pub fn correlation_id(&self) -> Result<CorrelationId, OperationsError> {
        decode_id(&self.correlation_id)
    }
    pub fn validate(&self) -> Result<(), OperationsError> {
        self.session_id()?;
        self.correlation_id()?;
        validate_reason(&self.reason)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorStatus {
    pub lifecycle: String,
    pub ready: bool,
    pub connected_clients: u64,
    pub active_release: Option<String>,
    pub writer_epoch: Option<u64>,
    pub authority_incarnation: Option<String>,
    pub outbox_backlog: Option<u64>,
    pub outbox_oldest_seconds: Option<u64>,
}

impl OperatorStatus {
    pub fn authority_incarnation(
        &self,
    ) -> Result<Option<worldwright_core::ids::AuthorityIncarnationId>, OperationsError> {
        self.authority_incarnation
            .as_deref()
            .map(decode_id)
            .transpose()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationsError {
    InvalidToken,
    InvalidClaims,
    InvalidKey,
    InvalidTotp,
    InvalidRequest,
    Serialization,
}

impl fmt::Display for OperationsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidToken => "operator token is invalid",
            Self::InvalidClaims => "operator claims are invalid",
            Self::InvalidKey => "operator signing key is invalid",
            Self::InvalidTotp => "operator one-time code is invalid",
            Self::InvalidRequest => "operator request is invalid",
            Self::Serialization => "operator data serialization failed",
        })
    }
}

impl Error for OperationsError {}

pub fn issue_token(claims: &OperatorClaims, signing_key: &[u8]) -> Result<String, OperationsError> {
    validate_key(signing_key)?;
    validate_claims(claims)?;
    let payload = serde_json::to_vec(claims).map_err(|_| OperationsError::Serialization)?;
    let encoded = URL_SAFE_NO_PAD.encode(payload);
    let signing_input = format!("{TOKEN_PREFIX}.{encoded}");
    let signature = sign(signing_input.as_bytes(), signing_key)?;
    let token = format!("{signing_input}.{}", URL_SAFE_NO_PAD.encode(signature));
    if token.len() > MAX_TOKEN_BYTES {
        return Err(OperationsError::InvalidToken);
    }
    Ok(token)
}

pub fn verify_token(
    token: &str,
    signing_key: &[u8],
    expected_issuer: &str,
    expected_audience: &str,
    now: u64,
) -> Result<OperatorClaims, OperationsError> {
    validate_key(signing_key)?;
    if token.is_empty() || token.len() > MAX_TOKEN_BYTES {
        return Err(OperationsError::InvalidToken);
    }
    let mut parts = token.split('.');
    let prefix = parts.next().ok_or(OperationsError::InvalidToken)?;
    let payload_encoded = parts.next().ok_or(OperationsError::InvalidToken)?;
    let signature_encoded = parts.next().ok_or(OperationsError::InvalidToken)?;
    if parts.next().is_some() || prefix != TOKEN_PREFIX {
        return Err(OperationsError::InvalidToken);
    }
    let payload = URL_SAFE_NO_PAD
        .decode(payload_encoded)
        .map_err(|_| OperationsError::InvalidToken)?;
    let signature = URL_SAFE_NO_PAD
        .decode(signature_encoded)
        .map_err(|_| OperationsError::InvalidToken)?;
    if signature.len() != 32 {
        return Err(OperationsError::InvalidToken);
    }
    let signing_input = format!("{TOKEN_PREFIX}.{payload_encoded}");
    let mut mac =
        HmacSha256::new_from_slice(signing_key).map_err(|_| OperationsError::InvalidKey)?;
    mac.update(signing_input.as_bytes());
    mac.verify_slice(&signature)
        .map_err(|_| OperationsError::InvalidToken)?;
    let claims: OperatorClaims =
        serde_json::from_slice(&payload).map_err(|_| OperationsError::InvalidClaims)?;
    let canonical = serde_json::to_vec(&claims).map_err(|_| OperationsError::Serialization)?;
    if canonical != payload {
        return Err(OperationsError::InvalidToken);
    }
    claims.validate_at(expected_issuer, expected_audience, now)?;
    Ok(claims)
}

pub fn decode_hex_key(value: &str) -> Result<Vec<u8>, OperationsError> {
    if value.is_empty() || value.len() % 2 != 0 {
        return Err(OperationsError::InvalidKey);
    }
    decode_hex(value).ok_or(OperationsError::InvalidKey)
}

pub fn encode_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use fmt::Write as _;
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

pub fn decode_hex_exact<const N: usize>(value: &str) -> Result<[u8; N], OperationsError> {
    if value.len() != N * 2 {
        return Err(OperationsError::InvalidRequest);
    }
    let bytes = decode_hex(value).ok_or(OperationsError::InvalidRequest)?;
    bytes
        .try_into()
        .map_err(|_| OperationsError::InvalidRequest)
}

pub fn totp_code(secret: &[u8], unix_seconds: u64) -> Result<String, OperationsError> {
    if secret.len() < 20 {
        return Err(OperationsError::InvalidTotp);
    }
    let counter = unix_seconds / TOTP_STEP_SECONDS;
    let digest = hmac_counter(secret, counter)?;
    let offset = usize::from(digest[31] & 0x0f);
    let value = ((u32::from(digest[offset]) << 24)
        | (u32::from(digest[offset + 1]) << 16)
        | (u32::from(digest[offset + 2]) << 8)
        | u32::from(digest[offset + 3]))
        & 0x7fff_ffff;
    Ok(format!("{:08}", value % 10u32.pow(TOTP_DIGITS)))
}

pub fn verify_totp(secret: &[u8], code: &str, unix_seconds: u64) -> Result<(), OperationsError> {
    if secret.len() < 20 || code.len() != 8 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(OperationsError::InvalidTotp);
    }
    let step = TOTP_STEP_SECONDS;
    let current = unix_seconds / step;
    for candidate in [
        current.saturating_sub(1),
        current,
        current.saturating_add(1),
    ] {
        let expected = totp_code(secret, candidate.saturating_mul(step))?;
        if constant_time_equal(expected.as_bytes(), code.as_bytes()) {
            return Ok(());
        }
    }
    Err(OperationsError::InvalidTotp)
}

fn validate_key(key: &[u8]) -> Result<(), OperationsError> {
    if key.len() < MIN_SIGNING_KEY_BYTES {
        Err(OperationsError::InvalidKey)
    } else {
        Ok(())
    }
}

fn validate_claims(claims: &OperatorClaims) -> Result<(), OperationsError> {
    validate_label(&claims.issuer, false)?;
    validate_label(&claims.audience, false)?;
    validate_subject(&claims.subject)?;
    let _ = claims.session_id()?;
    if claims.expires_at <= claims.issued_at
        || claims.expires_at - claims.issued_at
            > match claims.kind {
                OperatorKind::Service => SERVICE_TOKEN_TTL_SECONDS,
                OperatorKind::Human | OperatorKind::BreakGlass => TOKEN_TTL_SECONDS,
            }
        || claims.auth_time > claims.issued_at
    {
        return Err(OperationsError::InvalidClaims);
    }
    let paired = matches!(
        (claims.kind, claims.assurance),
        (OperatorKind::Human, Assurance::Mfa)
            | (OperatorKind::Service, Assurance::Service)
            | (OperatorKind::BreakGlass, Assurance::BreakGlass)
    );
    if !paired {
        return Err(OperationsError::InvalidClaims);
    }
    match (&claims.kind, &claims.break_glass_reason) {
        (OperatorKind::BreakGlass, Some(reason)) => {
            validate_text(reason, 512, true).map_err(|_| OperationsError::InvalidClaims)?
        }
        (OperatorKind::BreakGlass, None) => return Err(OperationsError::InvalidClaims),
        (_, Some(_)) => return Err(OperationsError::InvalidClaims),
        (_, None) => {}
    }
    Ok(())
}

fn validate_label(value: &str, allow_space: bool) -> Result<(), OperationsError> {
    if value.is_empty() || value.len() > 128 {
        return Err(OperationsError::InvalidClaims);
    }
    if value.bytes().any(|byte| {
        byte.is_ascii_control()
            || byte.is_ascii_whitespace()
            || (!allow_space && !byte.is_ascii_graphic())
    }) {
        return Err(OperationsError::InvalidClaims);
    }
    Ok(())
}

fn validate_subject(value: &str) -> Result<(), OperationsError> {
    if value.is_empty()
        || value.len() > 128
        || !value.as_bytes()[0].is_ascii_lowercase()
        || value.as_bytes()[1..].iter().any(|byte| {
            !byte.is_ascii_lowercase() && !byte.is_ascii_digit() && !b"._-".contains(byte)
        })
    {
        Err(OperationsError::InvalidClaims)
    } else {
        Ok(())
    }
}

fn validate_reason(value: &str) -> Result<(), OperationsError> {
    validate_text(value, 512, true).map_err(|_| OperationsError::InvalidRequest)
}

fn validate_text(value: &str, max: usize, trim: bool) -> Result<(), OperationsError> {
    if value.is_empty()
        || value.len() > max
        || (trim && value.trim() != value)
        || value.chars().any(|character| character.is_control())
    {
        Err(OperationsError::InvalidRequest)
    } else {
        Ok(())
    }
}

fn decode_id<T>(value: &str) -> Result<T, OperationsError>
where
    T: FromStr,
{
    value.parse().map_err(|_| OperationsError::InvalidRequest)
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return None;
    }
    Some(
        value
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| (hex(pair[0]) << 4) | hex(pair[1]))
            .collect(),
    )
}

fn hex(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => unreachable!("hex input is validated"),
    }
}

fn sign(input: &[u8], key: &[u8]) -> Result<Vec<u8>, OperationsError> {
    let mut mac = HmacSha256::new_from_slice(key).map_err(|_| OperationsError::InvalidKey)?;
    mac.update(input);
    Ok(mac.finalize().into_bytes().to_vec())
}

fn hmac_counter(secret: &[u8], counter: u64) -> Result<[u8; 32], OperationsError> {
    let mut mac = HmacSha256::new_from_slice(secret).map_err(|_| OperationsError::InvalidTotp)?;
    mac.update(&counter.to_be_bytes());
    mac.finalize()
        .into_bytes()
        .as_slice()
        .try_into()
        .map_err(|_| OperationsError::InvalidTotp)
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0u8;
    for (left, right) in left.iter().zip(right) {
        difference |= left ^ right;
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claims(now: u64) -> OperatorClaims {
        OperatorClaims {
            issuer: "issuer".into(),
            audience: "audience".into(),
            subject: "operator".into(),
            session_id: "00000000000000000000000000000001".into(),
            kind: OperatorKind::Human,
            assurance: Assurance::Mfa,
            issued_at: now,
            expires_at: now + 600,
            auth_time: now,
            break_glass_reason: None,
        }
    }

    #[test]
    fn token_round_trip_and_tamper_rejection() {
        let token = issue_token(&claims(1000), &[7; 32]).unwrap();
        assert_eq!(
            verify_token(&token, &[7; 32], "issuer", "audience", 1000)
                .unwrap()
                .subject,
            "operator"
        );
        let mut tampered = token.into_bytes();
        *tampered.last_mut().unwrap() = b'a';
        assert!(matches!(
            verify_token(
                std::str::from_utf8(&tampered).unwrap(),
                &[7; 32],
                "issuer",
                "audience",
                1000
            ),
            Err(OperationsError::InvalidToken)
        ));
        assert!(matches!(
            verify_token(
                &issue_token(&claims(1000), &[7; 32]).unwrap(),
                &[7; 32],
                "issuer",
                "other",
                1000
            ),
            Err(OperationsError::InvalidClaims)
        ));
    }

    #[test]
    fn claims_and_token_bounds_are_enforced() {
        let mut value = claims(1000);
        assert!(matches!(
            verify_token(
                &issue_token(&value, &[7; 32]).unwrap(),
                &[7; 32],
                "wrong",
                "audience",
                1000
            ),
            Err(OperationsError::InvalidClaims)
        ));
        value.expires_at = 1000 + 601;
        assert!(issue_token(&value, &[7; 32]).is_err());
        value = claims(1000);
        value.kind = OperatorKind::BreakGlass;
        value.assurance = Assurance::BreakGlass;
        assert!(issue_token(&value, &[7; 32]).is_err());
        value.break_glass_reason = Some("incident".into());
        assert!(issue_token(&value, &[7; 32]).is_ok());
        assert!(issue_token(&claims(1000), &[7; 31]).is_err());
        let token = issue_token(&claims(1000), &[7; 32]).unwrap();
        assert!(verify_token(&token, &[7; 32], "issuer", "audience", 1601).is_err());
        assert!(verify_token(&token, &[7; 32], "issuer", "audience", 900).is_err());
        let mut aged = claims(1000);
        aged.auth_time = 600;
        let aged_token = issue_token(&aged, &[7; 32]).unwrap();
        assert!(verify_token(&aged_token, &[7; 32], "issuer", "audience", 1000).is_err());
        let mut service = claims(1000);
        service.kind = OperatorKind::Service;
        service.assurance = Assurance::Service;
        service.expires_at = 1301;
        assert!(issue_token(&service, &[7; 32]).is_err());
    }

    #[test]
    fn totp_is_deterministic_and_windowed() {
        let secret = b"01234567890123456789";
        let code = totp_code(secret, 1_000).unwrap();
        assert_eq!(code.len(), 8);
        assert!(verify_totp(secret, &code, 1_000 + 30).is_ok());
        assert!(verify_totp(secret, &code, 1_000 + 60).is_err());
        assert!(verify_totp(secret, "00000000", 1_000).is_err());
        assert!(verify_totp(b"short", &code, 1_000).is_err());
        assert_eq!(
            totp_code(b"12345678901234567890123456789012", 59).unwrap(),
            "46119246"
        );
    }

    #[test]
    fn strict_dtos_and_hex_helpers() {
        let request: ModerationRequest = serde_json::from_str(
            r#"{"account_id":"00000000000000000000000000000001","disabled":true,"reason":"test","correlation_id":"00000000000000000000000000000002"}"#,
        )
        .unwrap();
        assert!(request.validate().is_ok());
        assert!(
            serde_json::from_str::<DrainRequest>(
                r#"{"reason":"x","correlation_id":"x","extra":1}"#
            )
            .is_err()
        );
        assert_eq!(encode_hex(&[0, 255]), "00ff");
        assert!(decode_hex_exact::<2>("00ff").is_ok());
        assert!(decode_hex_exact::<2>("00FF").is_err());
        assert_eq!(
            serde_json::to_string(&Capability::ActivateRelease).unwrap(),
            "\"activate_release\""
        );
        assert_eq!(
            serde_json::to_string(&Environment::Production).unwrap(),
            "\"production\""
        );
    }

    fn submission_fixture() -> (Vec<u8>, ReleaseSubmissionMetadata) {
        let artifact = worldwright_release::package_bytes(
            "studio-test",
            include_bytes!("../../../examples/two-room/world.json"),
            worldwright_release::PackageOptions {
                release_id: "studio-test".parse().unwrap(),
                source_revision: "studio-revision-1".into(),
                builder_identity: "studio-test".into(),
            },
        )
        .unwrap();
        let verified = worldwright_release::verify_bytes("studio-test", &artifact, None).unwrap();
        let metadata = ReleaseSubmissionMetadata {
            schema_version: RELEASE_SUBMISSION_VERSION,
            kind: worldwright_release::PublicationKind::Activate,
            target: worldwright_release::DeploymentTarget::Staging,
            expected_current: None,
            reason: "submit tested Studio artifact".into(),
            correlation_id: "00000000000000000000000000000041".into(),
            review_context: StudioReviewContext {
                schema_version: 1,
                draft_revision: 7,
                snapshot_sha256: verified.manifest().payload_sha256().to_string(),
                validation_diagnostic_count: 0,
                preview_artifact_sha256: verified.artifact_digest().to_string(),
                preview_completed_at: 1_700_000_000,
                change_summary: "Add the northern room".into(),
            },
        };
        (artifact, metadata)
    }

    #[test]
    fn release_submission_round_trips_exact_bytes() {
        let (artifact, metadata) = submission_fixture();
        let encoded = encode_release_submission(&metadata, &artifact).unwrap();
        let decoded = decode_release_submission(&encoded).unwrap();
        assert_eq!(decoded.metadata, metadata);
        assert_eq!(decoded.artifact, artifact.as_slice());
    }

    #[test]
    fn release_submission_rejects_every_header_and_metadata_truncation() {
        let (artifact, metadata) = submission_fixture();
        let encoded = encode_release_submission(&metadata, &artifact).unwrap();
        let offset = RELEASE_SUBMISSION_PREFIX.len();
        let metadata_len =
            u32::from_be_bytes(encoded[offset..offset + 4].try_into().unwrap()) as usize;
        for end in 0..=(offset + 4 + metadata_len) {
            assert!(
                decode_release_submission(&encoded[..end]).is_err(),
                "end={end}"
            );
        }
    }

    #[test]
    fn release_submission_rejects_malformed_envelopes() {
        let (artifact, metadata) = submission_fixture();
        let encoded = encode_release_submission(&metadata, &artifact).unwrap();
        let mut bad = encoded.clone();
        bad[0] ^= 1;
        assert!(matches!(
            decode_release_submission(&bad),
            Err(OperationsError::InvalidRequest)
        ));
        let mut zero = encoded.clone();
        zero[RELEASE_SUBMISSION_PREFIX.len()..RELEASE_SUBMISSION_PREFIX.len() + 4]
            .copy_from_slice(&0u32.to_be_bytes());
        assert!(decode_release_submission(&zero).is_err());
        let mut large = encoded[..RELEASE_SUBMISSION_PREFIX.len() + 4].to_vec();
        large[RELEASE_SUBMISSION_PREFIX.len()..].copy_from_slice(&4097u32.to_be_bytes());
        large.extend_from_slice(b"{}artifact");
        assert!(decode_release_submission(&large).is_err());
        let metadata_json = serde_json::to_vec(&metadata)
            .unwrap()
            .iter()
            .copied()
            .chain(br#" "#.iter().copied())
            .collect::<Vec<_>>();
        let mut trailing = RELEASE_SUBMISSION_PREFIX.to_vec();
        trailing.extend_from_slice(&(metadata_json.len() as u32).to_be_bytes());
        trailing.extend_from_slice(&metadata_json);
        assert!(decode_release_submission(&trailing).is_err());
        let mut unknown: serde_json::Value = serde_json::to_value(&metadata).unwrap();
        unknown["unknown"] = serde_json::json!(true);
        let unknown_json = serde_json::to_vec(&unknown).unwrap();
        let mut unknown_envelope = RELEASE_SUBMISSION_PREFIX.to_vec();
        unknown_envelope.extend_from_slice(&(unknown_json.len() as u32).to_be_bytes());
        unknown_envelope.extend_from_slice(&unknown_json);
        unknown_envelope.extend_from_slice(&artifact);
        assert!(decode_release_submission(&unknown_envelope).is_err());
        let mut huge_summary = metadata.clone();
        huge_summary.review_context.change_summary = "x".repeat(5000);
        assert!(encode_release_submission(&huge_summary, &artifact).is_err());
        assert!(decode_release_submission(&vec![0u8; MAX_RELEASE_SUBMISSION_BYTES + 1]).is_err());
    }

    #[test]
    fn release_submission_binds_snapshot_and_preview_to_artifact() {
        let (artifact, metadata) = submission_fixture();
        let mut preview = metadata.clone();
        preview.review_context.preview_artifact_sha256 = "11".repeat(32);
        assert!(encode_release_submission(&preview, &artifact).is_err());
        let mut snapshot = metadata.clone();
        snapshot.review_context.snapshot_sha256 = "22".repeat(32);
        assert!(encode_release_submission(&snapshot, &artifact).is_err());
        let other = worldwright_release::package_bytes(
            "studio-other",
            include_bytes!("../../../examples/two-room/world.json"),
            worldwright_release::PackageOptions {
                release_id: "studio-other".parse().unwrap(),
                source_revision: "other".into(),
                builder_identity: "other".into(),
            },
        )
        .unwrap();
        assert!(encode_release_submission(&metadata, &other).is_err());
    }

    #[test]
    fn release_submission_rejects_invalid_review_fields_and_bounds() {
        let (artifact, metadata) = submission_fixture();
        let mut invalid = metadata.clone();
        invalid.schema_version = 0;
        assert!(encode_release_submission(&invalid, &artifact).is_err());
        invalid = metadata.clone();
        invalid.review_context.schema_version = 0;
        assert!(encode_release_submission(&invalid, &artifact).is_err());
        invalid = metadata.clone();
        invalid.review_context.draft_revision = 0;
        assert!(encode_release_submission(&invalid, &artifact).is_err());
        invalid = metadata.clone();
        invalid.review_context.preview_completed_at = 0;
        assert!(encode_release_submission(&invalid, &artifact).is_err());
        invalid = metadata.clone();
        invalid.review_context.validation_diagnostic_count = 1;
        assert!(encode_release_submission(&invalid, &artifact).is_err());
        for value in ["", " trimmed ", "line\nbreak"] {
            invalid = metadata.clone();
            invalid.review_context.change_summary = value.into();
            assert!(encode_release_submission(&invalid, &artifact).is_err());
            invalid = metadata.clone();
            invalid.reason = value.into();
            assert!(encode_release_submission(&invalid, &artifact).is_err());
        }
        invalid = metadata.clone();
        invalid.review_context.change_summary = "x".repeat(1025);
        assert!(encode_release_submission(&invalid, &artifact).is_err());
        invalid = metadata.clone();
        invalid.reason = "x".repeat(513);
        assert!(encode_release_submission(&invalid, &artifact).is_err());
        invalid.correlation_id = "bad".into();
        assert!(encode_release_submission(&invalid, &artifact).is_err());
    }

    #[test]
    fn release_submission_rejects_tampered_and_oversized_artifacts() {
        let (artifact, metadata) = submission_fixture();
        let mut tampered = artifact.clone();
        *tampered.last_mut().unwrap() ^= 1;
        assert!(encode_release_submission(&metadata, &tampered).is_err());
        assert!(encode_release_submission(&metadata, b"not-an-artifact").is_err());
        let huge = vec![0u8; worldwright_release::MAX_ARTIFACT_BYTES + 1];
        assert!(encode_release_submission(&metadata, &huge).is_err());
        let mut metadata_huge = metadata;
        metadata_huge.reason = "x".repeat(1025);
        assert!(encode_release_submission(&metadata_huge, &artifact).is_err());
    }

    #[test]
    fn noncanonical_claims_encoding_and_break_glass_rules_are_rejected() {
        let base = claims(1000);
        let pretty = serde_json::to_string_pretty(&base).unwrap();
        let payload = URL_SAFE_NO_PAD.encode(pretty);
        let input = format!("{TOKEN_PREFIX}.{payload}");
        let signature = URL_SAFE_NO_PAD.encode(sign(input.as_bytes(), &[7; 32]).unwrap());
        let token = format!("{input}.{signature}");
        assert!(matches!(
            verify_token(&token, &[7; 32], "issuer", "audience", 1000),
            Err(OperationsError::InvalidToken)
        ));
        let mut break_glass = base;
        break_glass.kind = OperatorKind::BreakGlass;
        break_glass.assurance = Assurance::BreakGlass;
        assert!(issue_token(&break_glass, &[7; 32]).is_err());
        break_glass.break_glass_reason = Some(" incident ".into());
        assert!(issue_token(&break_glass, &[7; 32]).is_err());
        break_glass.break_glass_reason = Some("incident".into());
        assert!(issue_token(&break_glass, &[7; 32]).is_ok());
    }

    fn backup_manifest() -> BackupManifest {
        BackupManifest {
            schema_version: BACKUP_MANIFEST_VERSION,
            backup_id: "00000000000000000000000000000001".into(),
            world_id: "00000000000000000000000000000002".into(),
            created_at: 1_700_000_000,
            application_version: "0.1.0-dev.0".into(),
            active_release: Some("11".repeat(32)),
            authority_incarnation: "00000000000000000000000000000003".into(),
            writer_epoch: 1,
            backup_sha256: "22".repeat(32),
            rpo_target_seconds: RPO_TARGET_SECONDS,
            rto_target_seconds: RTO_TARGET_SECONDS,
            audit_retention_days: AUDIT_RETENTION_DAYS,
        }
    }

    #[test]
    fn backup_manifest_round_trip_and_tamper_rejection() {
        let manifest = backup_manifest();
        let signed = sign_backup_manifest(&manifest, &[9; 32]).unwrap();
        verify_backup_manifest(&signed, &[9; 32], [0x22; 32]).unwrap();
        assert!(verify_backup_manifest(&signed, &[8; 32], [0x22; 32]).is_err());
        assert!(verify_backup_manifest(&signed, &[9; 32], [0x33; 32]).is_err());
        let mut tampered = signed.clone();
        tampered.signature.push('a');
        assert!(verify_backup_manifest(&tampered, &[9; 32], [0x22; 32]).is_err());
        let mutations: &[fn(&mut BackupManifest)] = &[
            |m: &mut BackupManifest| m.schema_version += 1,
            |m: &mut BackupManifest| m.created_at = 0,
            |m: &mut BackupManifest| m.writer_epoch = 0,
            |m: &mut BackupManifest| m.backup_id = "AA".into(),
            |m: &mut BackupManifest| m.backup_sha256 = "zz".into(),
            |m: &mut BackupManifest| m.rpo_target_seconds = 1,
        ];
        for mutate in mutations {
            let mut value = manifest.clone();
            mutate(&mut value);
            assert!(sign_backup_manifest(&value, &[9; 32]).is_err());
        }
        assert!(sign_backup_manifest(&manifest, &[1; 31]).is_err());
    }

    #[test]
    fn backup_manifest_json_is_strict() {
        let value = serde_json::to_string(&SignedBackupManifest {
            manifest: backup_manifest(),
            signature: "x".into(),
        })
        .unwrap()
        .replace("}", ",\"unknown\":1}");
        assert!(serde_json::from_str::<SignedBackupManifest>(&value).is_err());
        assert!(
            serde_json::from_str::<SignedBackupManifest>(r#"{"manifest":{},"signature":"!"}"#)
                .is_err()
        );
    }
}
