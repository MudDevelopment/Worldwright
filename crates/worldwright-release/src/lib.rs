//! Deterministic, content-validated Worldwright release artifacts.
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, Visitor},
};
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::Path,
    str::FromStr,
};
use worldwright_content::{ContentReport, MAX_INPUT_BYTES, ValidatedWorldContent, load_bytes};

pub const MANIFEST_VERSION: u32 = 1;
pub const GAME_ID: &str = "worldwright-reference";
pub const GAME_CONTRACT_VERSION: u32 = 1;
pub const PAYLOAD_MEDIA_TYPE: &str = "application/vnd.worldwright.world+json";
pub const MAX_MANIFEST_BYTES: usize = 16 * 1024;
const HEADER_PREFIX: &[u8] = b"WORLDWRIGHT-RELEASE-V1\n";
const HEADER_BYTES: usize = 8 + 1;
pub const MAX_ARTIFACT_BYTES: usize =
    HEADER_PREFIX.len() + HEADER_BYTES + MAX_MANIFEST_BYTES + MAX_INPUT_BYTES;

macro_rules! strict_string_enum {
    ($name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name {
            $($variant),+
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(match self {
                    $(Self::$variant => $wire),+
                })
            }
        }

        impl FromStr for $name {
            type Err = ReleaseError;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($wire => Ok(Self::$variant),)+
                    _ => Err(ReleaseError::invalid_metadata("invalid release enum value")),
                }
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.to_string())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                value.parse().map_err(de::Error::custom)
            }
        }
    };
}

strict_string_enum!(DeploymentTarget {
    Staging => "staging",
    Production => "production",
});
strict_string_enum!(PublicationKind {
    Activate => "activate",
    Rollback => "rollback",
});
strict_string_enum!(PublicationStatus {
    Requested => "requested",
    Approved => "approved",
    Rejected => "rejected",
    Activating => "activating",
    Activated => "activated",
    Failed => "failed",
});
strict_string_enum!(ReviewDecision {
    Approve => "approve",
    Reject => "reject",
});

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReleaseId(String);

impl ReleaseId {
    pub const MAX_BYTES: usize = 64;

    pub fn parse(value: &str) -> Result<Self, ReleaseError> {
        let bytes = value.as_bytes();
        if bytes.is_empty()
            || bytes.len() > Self::MAX_BYTES
            || !bytes[0].is_ascii_lowercase()
            || bytes[1..].iter().any(|byte| {
                !byte.is_ascii_lowercase() && !byte.is_ascii_digit() && !b"._-".contains(byte)
            })
            || value == "."
            || value == ".."
        {
            return Err(ReleaseError::invalid_metadata("invalid release ID"));
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ReleaseId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for ReleaseId {
    type Err = ReleaseError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl Serialize for ReleaseId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ReleaseId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct IdVisitor;
        impl Visitor<'_> for IdVisitor {
            type Value = ReleaseId;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a valid release ID string")
            }
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                ReleaseId::parse(value).map_err(|error| E::custom(error.to_string()))
            }
        }
        deserializer.deserialize_str(IdVisitor)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sha256Digest([u8; 32]);

impl Sha256Digest {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    pub fn from_digest_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn parse(value: &str) -> Result<Self, ReleaseError> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(ReleaseError::invalid_metadata("invalid SHA-256 digest"));
        }
        let mut bytes = [0u8; 32];
        for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
            bytes[index] = (hex_value(pair[0]) << 4) | hex_value(pair[1]);
        }
        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

fn hex_value(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => unreachable!("digest parser validates hexadecimal input"),
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for Sha256Digest {
    type Err = ReleaseError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl Serialize for Sha256Digest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Sha256Digest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct DigestVisitor;
        impl Visitor<'_> for DigestVisitor {
            type Value = Sha256Digest;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a lowercase hexadecimal SHA-256 digest")
            }
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Sha256Digest::parse(value).map_err(|error| E::custom(error.to_string()))
            }
        }
        deserializer.deserialize_str(DigestVisitor)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageOptions {
    pub release_id: ReleaseId,
    pub source_revision: String,
    pub builder_identity: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseManifest {
    manifest_version: u32,
    release_id: ReleaseId,
    content_schema_version: u64,
    game_compatibility: GameCompatibility,
    payload: PayloadMetadata,
    provenance: Provenance,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GameCompatibility {
    game_id: String,
    contract_version: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PayloadMetadata {
    media_type: String,
    byte_length: usize,
    sha256: Sha256Digest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Provenance {
    source_revision: String,
    builder_identity: String,
}

impl ReleaseManifest {
    pub fn manifest_version(&self) -> u32 {
        self.manifest_version
    }
    pub fn release_id(&self) -> &ReleaseId {
        &self.release_id
    }
    pub fn content_schema_version(&self) -> u64 {
        self.content_schema_version
    }
    pub fn game_id(&self) -> &str {
        &self.game_compatibility.game_id
    }
    pub fn contract_version(&self) -> u32 {
        self.game_compatibility.contract_version
    }
    pub fn media_type(&self) -> &str {
        &self.payload.media_type
    }
    pub fn payload_media_type(&self) -> &str {
        self.media_type()
    }
    pub fn payload_byte_length(&self) -> usize {
        self.payload.byte_length
    }
    pub fn payload_length(&self) -> usize {
        self.payload_byte_length()
    }
    pub fn payload_sha256(&self) -> Sha256Digest {
        self.payload.sha256
    }
    pub fn payload_digest(&self) -> Sha256Digest {
        self.payload_sha256()
    }
    pub fn source_revision(&self) -> &str {
        &self.provenance.source_revision
    }
    pub fn builder_identity(&self) -> &str {
        &self.provenance.builder_identity
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedArtifact {
    manifest: ReleaseManifest,
    payload: Vec<u8>,
    content: ValidatedWorldContent,
    artifact_digest: Sha256Digest,
}

impl VerifiedArtifact {
    pub fn manifest(&self) -> &ReleaseManifest {
        &self.manifest
    }
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
    pub fn content(&self) -> &ValidatedWorldContent {
        &self.content
    }
    pub fn artifact_digest(&self) -> Sha256Digest {
        self.artifact_digest
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteOutcome {
    Created,
    Reused,
}

#[derive(Debug)]
pub enum ReleaseError {
    Io {
        operation: &'static str,
        source: io::Error,
    },
    InvalidFormat(&'static str),
    InvalidMetadata(&'static str),
    DigestMismatch {
        expected: Sha256Digest,
        actual: Sha256Digest,
    },
    ContentValidation(ContentReport),
    ImmutableConflict,
}

impl ReleaseError {
    fn invalid_metadata(message: &'static str) -> Self {
        Self::InvalidMetadata(message)
    }
    fn io(operation: &'static str, source: io::Error) -> Self {
        Self::Io { operation, source }
    }
}

impl fmt::Display for ReleaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { operation, .. } => write!(formatter, "release I/O error during {operation}"),
            Self::InvalidFormat(message) => write!(formatter, "invalid release format: {message}"),
            Self::InvalidMetadata(message) => {
                write!(formatter, "invalid release metadata: {message}")
            }
            Self::DigestMismatch { .. } => formatter.write_str("release digest mismatch"),
            Self::ContentValidation(_) => formatter.write_str("release content validation failed"),
            Self::ImmutableConflict => formatter.write_str("immutable release conflict"),
        }
    }
}

impl Error for ReleaseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::ContentValidation(report) => Some(report),
            _ => None,
        }
    }
}

pub fn package_bytes(
    source: &str,
    payload: &[u8],
    options: PackageOptions,
) -> Result<Vec<u8>, ReleaseError> {
    validate_payload(source, payload)?;
    validate_provenance(&options.source_revision, &options.builder_identity)?;
    let manifest = ReleaseManifest {
        manifest_version: MANIFEST_VERSION,
        release_id: options.release_id,
        content_schema_version: 1,
        game_compatibility: GameCompatibility {
            game_id: GAME_ID.to_owned(),
            contract_version: GAME_CONTRACT_VERSION,
        },
        payload: PayloadMetadata {
            media_type: PAYLOAD_MEDIA_TYPE.to_owned(),
            byte_length: payload.len(),
            sha256: Sha256Digest::from_bytes(payload),
        },
        provenance: Provenance {
            source_revision: options.source_revision,
            builder_identity: options.builder_identity,
        },
    };
    let manifest_bytes = serde_json::to_vec(&manifest)
        .map_err(|_| ReleaseError::InvalidFormat("manifest serialization failed"))?;
    if manifest_bytes.is_empty() || manifest_bytes.len() > MAX_MANIFEST_BYTES {
        return Err(ReleaseError::InvalidFormat("manifest exceeds size limit"));
    }
    let length = manifest_bytes.len();
    if length > 99_999_999 {
        return Err(ReleaseError::InvalidFormat(
            "manifest length cannot be encoded",
        ));
    }
    let total = HEADER_PREFIX
        .len()
        .checked_add(HEADER_BYTES)
        .and_then(|size| size.checked_add(length))
        .and_then(|size| size.checked_add(payload.len()))
        .ok_or(ReleaseError::InvalidFormat("artifact length overflow"))?;
    if total > MAX_ARTIFACT_BYTES {
        return Err(ReleaseError::InvalidFormat("artifact exceeds size limit"));
    }
    let mut artifact = Vec::with_capacity(total);
    artifact.extend_from_slice(HEADER_PREFIX);
    artifact.extend_from_slice(format!("{length:08x}\n").as_bytes());
    artifact.extend_from_slice(&manifest_bytes);
    artifact.extend_from_slice(payload);
    verify_bytes(source, &artifact, None)?;
    Ok(artifact)
}

pub fn package_file(
    content_path: impl AsRef<Path>,
    options: PackageOptions,
) -> Result<Vec<u8>, ReleaseError> {
    let path = content_path.as_ref();
    let payload = read_regular_bounded(path, MAX_INPUT_BYTES)?;
    package_bytes(&path.to_string_lossy(), &payload, options)
}

pub fn verify_bytes(
    source: &str,
    artifact: &[u8],
    expected_artifact_digest: Option<Sha256Digest>,
) -> Result<VerifiedArtifact, ReleaseError> {
    if artifact.len() > MAX_ARTIFACT_BYTES {
        return Err(ReleaseError::InvalidFormat("artifact exceeds size limit"));
    }
    let minimum = HEADER_PREFIX.len() + HEADER_BYTES + 1 + 1;
    if artifact.len() < minimum || !artifact.starts_with(HEADER_PREFIX) {
        return Err(ReleaseError::InvalidFormat("invalid release header"));
    }
    let length_start = HEADER_PREFIX.len();
    let length_end = length_start + HEADER_BYTES;
    let length_bytes = &artifact[length_start..length_end];
    if length_bytes[8] != b'\n'
        || !length_bytes[..8]
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(ReleaseError::InvalidFormat("invalid manifest length"));
    }
    let manifest_length = length_bytes[..8]
        .iter()
        .try_fold(0usize, |value, byte| {
            value
                .checked_mul(16)?
                .checked_add(usize::from(if byte.is_ascii_digit() {
                    byte - b'0'
                } else {
                    byte - b'a' + 10
                }))
        })
        .ok_or(ReleaseError::InvalidFormat("manifest length overflow"))?;
    if !(1..=MAX_MANIFEST_BYTES).contains(&manifest_length) {
        return Err(ReleaseError::InvalidFormat("manifest length out of range"));
    }
    let manifest_start = length_end;
    let manifest_end = manifest_start
        .checked_add(manifest_length)
        .ok_or(ReleaseError::InvalidFormat("manifest length overflow"))?;
    if manifest_end >= artifact.len() {
        return Err(ReleaseError::InvalidFormat("truncated release artifact"));
    }
    let manifest_bytes = &artifact[manifest_start..manifest_end];
    let manifest: ReleaseManifest = serde_json::from_slice(manifest_bytes)
        .map_err(|_| ReleaseError::InvalidFormat("invalid release manifest"))?;
    let canonical = serde_json::to_vec(&manifest)
        .map_err(|_| ReleaseError::InvalidFormat("manifest serialization failed"))?;
    if canonical != manifest_bytes {
        return Err(ReleaseError::InvalidFormat("manifest is not canonical"));
    }
    validate_manifest(&manifest)?;
    let payload = &artifact[manifest_end..];
    if payload.is_empty() || payload.len() > MAX_INPUT_BYTES {
        return Err(ReleaseError::InvalidFormat("payload length out of range"));
    }
    if manifest.payload.byte_length != payload.len() {
        return Err(ReleaseError::InvalidFormat(
            "payload length does not match manifest",
        ));
    }
    let payload_digest = Sha256Digest::from_bytes(payload);
    if manifest.payload.sha256 != payload_digest {
        return Err(ReleaseError::DigestMismatch {
            expected: manifest.payload.sha256,
            actual: payload_digest,
        });
    }
    let content =
        load_bytes(source, payload, Default::default()).map_err(ReleaseError::ContentValidation)?;
    let artifact_digest = Sha256Digest::from_bytes(artifact);
    if let Some(expected) = expected_artifact_digest {
        if expected != artifact_digest {
            return Err(ReleaseError::DigestMismatch {
                expected,
                actual: artifact_digest,
            });
        }
    }
    Ok(VerifiedArtifact {
        manifest,
        payload: payload.to_owned(),
        content,
        artifact_digest,
    })
}

pub fn load_file(
    artifact_path: impl AsRef<Path>,
    expected_artifact_digest: Option<Sha256Digest>,
) -> Result<VerifiedArtifact, ReleaseError> {
    let path = artifact_path.as_ref();
    let artifact = read_regular_bounded(path, MAX_ARTIFACT_BYTES)?;
    verify_bytes(&path.to_string_lossy(), &artifact, expected_artifact_digest)
}

pub fn write_new(path: impl AsRef<Path>, artifact: &[u8]) -> Result<WriteOutcome, ReleaseError> {
    verify_bytes("artifact output", artifact, None)?;
    let path = path.as_ref();
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            let result = (|| {
                file.write_all(artifact)
                    .map_err(|error| ReleaseError::io("writing release", error))?;
                file.sync_all()
                    .map_err(|error| ReleaseError::io("syncing release", error))?;
                Ok(WriteOutcome::Created)
            })();
            if result.is_err() {
                let _ = fs::remove_file(path);
            }
            result
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let existing = read_regular_bounded(path, MAX_ARTIFACT_BYTES)?;
            if existing == artifact {
                Ok(WriteOutcome::Reused)
            } else {
                Err(ReleaseError::ImmutableConflict)
            }
        }
        Err(error) => Err(ReleaseError::io("creating release", error)),
    }
}

fn validate_payload(source: &str, payload: &[u8]) -> Result<(), ReleaseError> {
    if payload.is_empty() || payload.len() > MAX_INPUT_BYTES {
        return Err(ReleaseError::InvalidFormat("payload length out of range"));
    }
    load_bytes(source, payload, Default::default())
        .map(|_| ())
        .map_err(ReleaseError::ContentValidation)
}

fn validate_manifest(manifest: &ReleaseManifest) -> Result<(), ReleaseError> {
    if manifest.manifest_version != MANIFEST_VERSION
        || manifest.content_schema_version != 1
        || manifest.game_compatibility.game_id != GAME_ID
        || manifest.game_compatibility.contract_version != GAME_CONTRACT_VERSION
        || manifest.payload.media_type != PAYLOAD_MEDIA_TYPE
    {
        return Err(ReleaseError::InvalidMetadata(
            "unsupported manifest metadata",
        ));
    }
    validate_provenance(
        &manifest.provenance.source_revision,
        &manifest.provenance.builder_identity,
    )
}

fn validate_provenance(source_revision: &str, builder_identity: &str) -> Result<(), ReleaseError> {
    for value in [source_revision, builder_identity] {
        if value.is_empty()
            || value.len() > 128
            || value.trim() != value
            || value.chars().any(|character| {
                character.is_control()
                    || matches!(
                        character as u32,
                        0x061c | 0x200e | 0x200f | 0x2028..=0x202e | 0x2066..=0x2069
                    )
            })
        {
            return Err(ReleaseError::InvalidMetadata("invalid provenance value"));
        }
    }
    Ok(())
}

fn read_regular_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>, ReleaseError> {
    let metadata =
        fs::metadata(path).map_err(|error| ReleaseError::io("reading release", error))?;
    if !metadata.is_file() {
        return Err(ReleaseError::InvalidFormat("expected a regular file"));
    }
    let file = File::open(path).map_err(|error| ReleaseError::io("opening release", error))?;
    if !file
        .metadata()
        .map_err(|error| ReleaseError::io("reading release", error))?
        .is_file()
    {
        return Err(ReleaseError::InvalidFormat("expected a regular file"));
    }
    let mut bytes = Vec::new();
    file.take((maximum as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| ReleaseError::io("reading release", error))?;
    if bytes.len() > maximum {
        return Err(ReleaseError::InvalidFormat("file exceeds size limit"));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, fs};

    const CONTENT: &[u8] = include_bytes!("../../../examples/two-room/world.json");

    fn options() -> PackageOptions {
        PackageOptions {
            release_id: "release-1".parse().unwrap(),
            source_revision: "abc123".into(),
            builder_identity: "builder".into(),
        }
    }

    #[test]
    fn deterministic_and_exact() {
        let first = package_bytes("test", CONTENT, options()).unwrap();
        let second = package_bytes("test", CONTENT, options()).unwrap();
        assert_eq!(first, second);
        let verified = verify_bytes("test", &first, None).unwrap();
        assert_eq!(verified.payload(), CONTENT);
        assert_eq!(verified.content().namespace(), "reference");
        let manifest = verified.manifest();
        assert_eq!(manifest.manifest_version(), 1);
        assert_eq!(manifest.release_id().as_str(), "release-1");
        assert_eq!(manifest.content_schema_version(), 1);
        assert_eq!(manifest.game_id(), GAME_ID);
        assert_eq!(manifest.contract_version(), GAME_CONTRACT_VERSION);
        assert_eq!(manifest.media_type(), PAYLOAD_MEDIA_TYPE);
        assert_eq!(manifest.payload_byte_length(), CONTENT.len());
        assert_eq!(manifest.payload_sha256(), Sha256Digest::from_bytes(CONTENT));
        assert_eq!(manifest.source_revision(), "abc123");
        assert_eq!(manifest.builder_identity(), "builder");
    }

    #[test]
    fn digest_vector_and_strict_parsing() {
        let digest = Sha256Digest::from_bytes(b"abc");
        assert_eq!(
            digest.to_string(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(Sha256Digest::parse(&digest.to_string()).is_ok());
        assert!(Sha256Digest::parse(&digest.to_string().to_uppercase()).is_err());
        assert!(Sha256Digest::parse("00").is_err());
        assert!(serde_json::from_str::<Sha256Digest>("\"ABC\"").is_err());
    }

    #[test]
    fn deployment_enums_use_strict_lowercase_strings() {
        assert_eq!(DeploymentTarget::Staging.to_string(), "staging");
        assert_eq!(
            serde_json::to_string(&PublicationKind::Rollback).unwrap(),
            "\"rollback\""
        );
        assert_eq!(
            "approved".parse::<PublicationStatus>().unwrap(),
            PublicationStatus::Approved
        );
        assert_eq!(
            serde_json::from_str::<ReviewDecision>("\"reject\"").unwrap(),
            ReviewDecision::Reject
        );
        assert!("PRODUCTION".parse::<DeploymentTarget>().is_err());
        assert!(serde_json::from_str::<PublicationKind>("\"activate_now\"").is_err());
    }

    #[test]
    fn artifact_limit_matches_container_formula() {
        assert_eq!(
            MAX_ARTIFACT_BYTES,
            HEADER_PREFIX.len() + HEADER_BYTES + MAX_MANIFEST_BYTES + MAX_INPUT_BYTES
        );
    }

    #[test]
    fn provenance_and_payload_change_artifacts() {
        let first = package_bytes("test", CONTENT, options()).unwrap();
        let mut changed_options = options();
        changed_options.builder_identity = "another-builder".into();
        let second = package_bytes("test", CONTENT, changed_options).unwrap();
        assert_ne!(
            Sha256Digest::from_bytes(&first),
            Sha256Digest::from_bytes(&second)
        );
        let mut changed_content: serde_json::Value = serde_json::from_slice(CONTENT).unwrap();
        changed_content["rooms"][0]["title"] = serde_json::json!("Changed");
        let changed_payload = serde_json::to_vec(&changed_content).unwrap();
        let third = package_bytes("test", &changed_payload, options()).unwrap();
        assert_ne!(first, third);
        assert_ne!(
            Sha256Digest::from_bytes(&first),
            Sha256Digest::from_bytes(&third)
        );
    }

    fn manifest_bounds(artifact: &[u8]) -> (usize, usize) {
        let start = HEADER_PREFIX.len() + HEADER_BYTES;
        let length = artifact[HEADER_PREFIX.len()..HEADER_PREFIX.len() + 8]
            .iter()
            .fold(0usize, |value, byte| {
                value * 16
                    + usize::from(if byte.is_ascii_digit() {
                        byte - b'0'
                    } else {
                        byte - b'a' + 10
                    })
            });
        (start, start + length)
    }

    fn artifact_with_manifest(raw_manifest: &[u8], payload: &[u8]) -> Vec<u8> {
        let mut artifact = Vec::new();
        artifact.extend_from_slice(HEADER_PREFIX);
        artifact.extend_from_slice(format!("{:08x}\n", raw_manifest.len()).as_bytes());
        artifact.extend_from_slice(raw_manifest);
        artifact.extend_from_slice(payload);
        artifact
    }

    #[test]
    fn tampering_and_expected_digest_are_rejected() {
        let artifact = package_bytes("test", CONTENT, options()).unwrap();
        let digest = Sha256Digest::from_bytes(&artifact);
        assert!(verify_bytes("test", &artifact, Some(digest)).is_ok());
        let mut tampered = artifact.clone();
        *tampered.last_mut().unwrap() ^= 1;
        assert!(verify_bytes("test", &tampered, None).is_err());
        assert!(verify_bytes("test", &artifact, Some(Sha256Digest::from_bytes(b"wrong"))).is_err());
    }

    #[test]
    fn malformed_headers_lengths_and_manifest_encoding_are_rejected() {
        let artifact = package_bytes("test", CONTENT, options()).unwrap();
        let (start, end) = manifest_bounds(&artifact);
        let manifest = &artifact[start..end];
        let payload = &artifact[end..];
        let mut uppercase = artifact.clone();
        uppercase[HEADER_PREFIX.len()] = b'A';
        assert!(verify_bytes("test", &uppercase, None).is_err());
        let mut nonhex = artifact.clone();
        nonhex[HEADER_PREFIX.len()] = b'g';
        assert!(verify_bytes("test", &nonhex, None).is_err());
        let mut overflow = artifact.clone();
        overflow[HEADER_PREFIX.len()..HEADER_PREFIX.len() + 8].copy_from_slice(b"ffffffff");
        assert!(verify_bytes("test", &overflow, None).is_err());
        assert!(verify_bytes("test", &artifact[..artifact.len() - 1], None).is_err());
        let mut trailing = artifact.clone();
        trailing.push(b'x');
        assert!(verify_bytes("test", &trailing, None).is_err());
        let mut spaced = Vec::with_capacity(manifest.len() + 1);
        spaced.push(b'{');
        spaced.push(b' ');
        spaced.extend_from_slice(&manifest[1..]);
        assert!(verify_bytes("test", &artifact_with_manifest(&spaced, payload), None).is_err());
    }

    #[test]
    fn manifest_fields_are_strict_and_supported_values_are_checked() {
        let artifact = package_bytes("test", CONTENT, options()).unwrap();
        let (start, end) = manifest_bounds(&artifact);
        let payload = &artifact[end..];
        let value: serde_json::Value = serde_json::from_slice(&artifact[start..end]).unwrap();
        for (field, replacement) in [
            ("manifest_version", serde_json::json!(2)),
            ("content_schema_version", serde_json::json!(2)),
            (
                "game_compatibility",
                serde_json::json!({"game_id": "other", "contract_version": 1}),
            ),
            (
                "payload",
                serde_json::json!({"media_type": "other", "byte_length": payload.len(), "sha256": Sha256Digest::from_bytes(payload).to_string()}),
            ),
        ] {
            let mut changed = value.clone();
            changed[field] = replacement;
            let raw = serde_json::to_vec(&changed).unwrap();
            assert!(verify_bytes("test", &artifact_with_manifest(&raw, payload), None).is_err());
        }
        let mut unknown = value.clone();
        unknown["unknown"] = serde_json::json!(true);
        assert!(
            verify_bytes(
                "test",
                &artifact_with_manifest(&serde_json::to_vec(&unknown).unwrap(), payload),
                None
            )
            .is_err()
        );
        let mut missing = value;
        missing.as_object_mut().unwrap().remove("provenance");
        assert!(
            verify_bytes(
                "test",
                &artifact_with_manifest(&serde_json::to_vec(&missing).unwrap(), payload),
                None
            )
            .is_err()
        );
    }

    #[test]
    fn invalid_content_and_file_operations_are_rejected() {
        let invalid = br#"{"schema_version":1}"#;
        assert!(matches!(
            package_bytes("test", invalid, options()),
            Err(ReleaseError::ContentValidation(_))
        ));
        let root =
            env::temp_dir().join(format!("worldwright-release-files-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let content_path = root.join("world.json");
        fs::write(&content_path, CONTENT).unwrap();
        let artifact = package_file(&content_path, options()).unwrap();
        let artifact_path = root.join("world.wr");
        fs::write(&artifact_path, &artifact).unwrap();
        assert_eq!(
            load_file(&artifact_path, Some(Sha256Digest::from_bytes(&artifact)))
                .unwrap()
                .payload(),
            CONTENT
        );
        assert!(package_file(&root, options()).is_err());
        assert!(load_file(&root, None).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn metadata_and_provenance_boundaries_are_rejected() {
        let too_long = "a".repeat(65);
        for id in ["", "A", "_x", too_long.as_str(), ".", ".."] {
            assert!(ReleaseId::parse(id).is_err());
        }
        for value in [
            "",
            " leading",
            "trailing ",
            "\u{061c}",
            "\u{2028}",
            "\u{1b}",
        ] {
            let mut bad = options();
            bad.source_revision = value.into();
            assert!(package_bytes("test", CONTENT, bad).is_err());
        }
    }

    #[test]
    fn write_is_immutable() {
        let root = env::temp_dir().join(format!("worldwright-release-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let path = root.join("release.wr");
        let artifact = package_bytes("test", CONTENT, options()).unwrap();
        assert_eq!(write_new(&path, &artifact).unwrap(), WriteOutcome::Created);
        assert_eq!(write_new(&path, &artifact).unwrap(), WriteOutcome::Reused);
        let mut changed_options = options();
        changed_options.builder_identity = "other-builder".into();
        let other = package_bytes("test", CONTENT, changed_options).unwrap();
        assert!(matches!(
            write_new(&path, &other),
            Err(ReleaseError::ImmutableConflict)
        ));
        let malformed_path = root.join("malformed.wr");
        let malformed = b"not a release";
        assert!(write_new(&malformed_path, malformed).is_err());
        assert!(!malformed_path.exists());
        let existing_malformed_path = root.join("existing-malformed.wr");
        fs::write(&existing_malformed_path, malformed).unwrap();
        assert!(write_new(&existing_malformed_path, malformed).is_err());
        assert!(write_new(&root, &artifact).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
