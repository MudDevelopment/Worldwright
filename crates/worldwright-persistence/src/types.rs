use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use uuid::Uuid;
use worldwright_core::ids::{
    AccountId, ActivationId, AuthorityIncarnationId, CharacterId, CorrelationId, GrantId, InviteId,
    ItemId, OperationId, OperatorSessionId, PublicationId, RecoveryJobId, WorldId, WriterId,
};
use worldwright_core::runtime::DurableWorldState;
use worldwright_core::time::Tick;
use worldwright_operations::{
    Capability, Environment, OperatorClaims, OperatorKind, StudioReviewContext,
};
use worldwright_release::{
    DeploymentTarget, GameContract, PublicationKind, PublicationStatus, ReleaseId, Sha256Digest,
};

pub const MAX_DURABLE_INVENTORY_ITEMS: usize = 256;
pub const MAX_DURABLE_SESSIONS: usize = 1024;
pub const SESSION_TTL_SECONDS: u64 = 43_200;
pub const RECEIPT_FORMAT_VERSION: i16 = 1;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct WriterLease {
    pub(crate) world: WorldId,
    pub(crate) writer: WriterId,
    pub(crate) epoch: u64,
    pub(crate) incarnation: AuthorityIncarnationId,
}
impl WriterLease {
    pub fn world(self) -> WorldId {
        self.world
    }
    pub fn writer(self) -> WriterId {
        self.writer
    }
    pub fn epoch(self) -> u64 {
        self.epoch
    }
    pub fn incarnation(self) -> AuthorityIncarnationId {
        self.incarnation
    }
}

/// Holds a shared lock on the world's release row while an in-memory command
/// executes, preventing activation from crossing that command boundary.
pub struct ReleaseReadGuard {
    pub(crate) _transaction: sqlx::Transaction<'static, sqlx::Postgres>,
}

pub struct SeedAccount {
    pub account: AccountId,
    pub username: String,
    pub password_hash: String,
    pub character: CharacterId,
    pub character_name: String,
}

pub struct SeedWorld {
    pub world: WorldId,
    pub content_namespace: String,
    pub content_schema_version: u64,
    pub game: GameContract,
    pub entry_room_definition: String,
    pub accounts: Vec<SeedAccount>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurableCharacterRoom {
    pub character: CharacterId,
    pub room_definition: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurableGameSnapshot {
    pub game: GameContract,
    pub state_version: u64,
    pub runtime: DurableWorldState,
    pub rooms: Vec<DurableCharacterRoom>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameplayIntentKind {
    Look,
    Move,
    Say,
    GameAction,
}

impl GameplayIntentKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Look => "look",
            Self::Move => "move",
            Self::Say => "say",
            Self::GameAction => "game_action",
        }
    }
}

#[derive(Clone, Copy)]
pub struct GameplayState<'a> {
    pub runtime: &'a DurableWorldState,
    pub rooms: &'a [DurableCharacterRoom],
    pub now: Tick,
}

pub struct GameplayMutation<'a> {
    pub lease: WriterLease,
    pub expected_release: Sha256Digest,
    pub game: GameContract,
    pub actor: AuthenticatedActor<'a>,
    pub request_id: &'a str,
    pub operation: OperationId,
    pub kind: GameplayIntentKind,
    pub intent: &'a [u8],
    pub expected_state_version: u64,
    pub state: GameplayState<'a>,
    pub result: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GameplayCommit {
    pub operation: OperationId,
    pub state_version: u64,
    pub result: serde_json::Value,
    pub replayed: bool,
}

pub struct IdentityRecord {
    pub(crate) account: AccountId,
    pub(crate) character: CharacterId,
    pub(crate) username: String,
    pub(crate) character_name: String,
    pub(crate) room_definition: String,
    pub(crate) balance: u64,
    pub(crate) version: u64,
    pub(crate) password_hash: String,
}
impl IdentityRecord {
    pub fn account(&self) -> AccountId {
        self.account
    }
    pub fn character(&self) -> CharacterId {
        self.character
    }
    pub fn username(&self) -> &str {
        &self.username
    }
    pub fn character_name(&self) -> &str {
        &self.character_name
    }
    pub fn room_definition(&self) -> &str {
        &self.room_definition
    }
    pub fn balance(&self) -> u64 {
        self.balance
    }
    pub fn version(&self) -> u64 {
        self.version
    }
    pub fn password_hash(&self) -> &str {
        &self.password_hash
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionPrincipal {
    pub account: AccountId,
    pub character: CharacterId,
    pub character_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OidcSession {
    pub principal: SessionPrincipal,
    pub linked: bool,
}

pub struct OidcInviteSpec<'a> {
    pub invite: InviteId,
    pub world: WorldId,
    pub username: &'a str,
    pub issuer: &'a str,
    pub token_hash: &'a [u8; 32],
    pub ttl_seconds: u64,
}

pub struct OidcLoginSpec<'a> {
    pub world: WorldId,
    pub issuer: &'a str,
    pub subject: &'a str,
    pub invite_hash: Option<&'a [u8; 32]>,
    pub session_hash: &'a [u8; 32],
    pub session_ttl_seconds: u64,
    pub max_sessions: usize,
}

pub struct AuthenticatedActor<'a> {
    pub world: WorldId,
    pub account: AccountId,
    pub character: CharacterId,
    pub token_hash: &'a [u8; 32],
}

pub struct RewardSpec<'a> {
    pub key: &'a str,
    pub generation: u64,
    pub credits: u64,
    pub item_definition: &'a str,
    pub item: ItemId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardOutcome {
    RewardGranted,
    AlreadyClaimed,
    InventoryFull,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RewardResult {
    #[serde(with = "operation_id_serde")]
    pub operation: OperationId,
    pub outcome: RewardOutcome,
    pub balance: u64,
    #[serde(with = "option_item_id_serde")]
    pub item: Option<ItemId>,
    pub replayed: bool,
}

pub struct TransferSpec<'a> {
    pub item: ItemId,
    pub recipient_username: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferOutcome {
    ItemTransferred,
    NotOwned,
    SameOwner,
    TargetNotFound,
    InventoryFull,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransferResult {
    #[serde(with = "operation_id_serde")]
    pub operation: OperationId,
    pub outcome: TransferOutcome,
    #[serde(with = "item_id_serde")]
    pub item: ItemId,
    #[serde(with = "option_character_id_serde")]
    pub owner: Option<CharacterId>,
    pub recipient_name: Option<String>,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryItem {
    pub item: ItemId,
    pub definition: String,
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventorySnapshot {
    pub balance: u64,
    pub character_version: u64,
    pub items: Vec<InventoryItem>,
}

pub struct OperatorContext<'a> {
    pub claims: &'a OperatorClaims,
    pub environment: Environment,
    pub world: WorldId,
    pub correlation: worldwright_core::ids::CorrelationId,
    pub reason: &'a str,
}

pub struct BootstrapOperator<'a> {
    pub subject: &'a str,
    pub display_name: &'a str,
    pub kind: OperatorKind,
    pub environment: Environment,
    pub world: WorldId,
    pub capabilities: &'a [Capability],
    pub reason: &'a str,
    pub correlation: worldwright_core::ids::CorrelationId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationalStatus {
    pub active_release: Option<Sha256Digest>,
    pub writer_epoch: u64,
    pub authority_incarnation: worldwright_core::ids::AuthorityIncarnationId,
    pub outbox_backlog: u64,
    pub outbox_oldest_seconds: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryJob {
    pub job: worldwright_core::ids::RecoveryJobId,
    pub source_incarnation: worldwright_core::ids::AuthorityIncarnationId,
    pub active_release: Option<Sha256Digest>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicationRecord {
    pub(crate) request: PublicationId,
    pub(crate) world: WorldId,
    pub(crate) artifact_digest: Sha256Digest,
    pub(crate) release_id: ReleaseId,
    pub(crate) kind: PublicationKind,
    pub(crate) target: DeploymentTarget,
    pub(crate) expected_current: Option<Sha256Digest>,
    pub(crate) requested_by: String,
    pub(crate) status: PublicationStatus,
    pub(crate) reviewed_by: Option<String>,
    pub(crate) review_reason: Option<String>,
    pub(crate) activation_executor: Option<String>,
    pub(crate) review_context: Option<StudioReviewContext>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseState {
    pub(crate) active: Option<Sha256Digest>,
    pub(crate) known_good: Option<Sha256Digest>,
    pub(crate) pending: Option<ActivationId>,
    pub(crate) maintenance: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredArtifact {
    pub(crate) bytes: Vec<u8>,
    pub(crate) digest: Sha256Digest,
    pub(crate) release_id: ReleaseId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivationPlan {
    pub(crate) activation: ActivationId,
    pub(crate) request: PublicationId,
    pub(crate) world: WorldId,
    pub(crate) artifact: StoredArtifact,
    pub(crate) kind: PublicationKind,
    pub(crate) target: DeploymentTarget,
    pub(crate) previous: Option<Sha256Digest>,
    pub(crate) executor: String,
    pub(crate) authorization_version: i16,
    pub(crate) lease: WriterLease,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurableContentReferences {
    pub(crate) room_definitions: Vec<String>,
    pub(crate) item_definitions: Vec<String>,
}

impl PublicationRecord {
    pub fn request(&self) -> PublicationId {
        self.request
    }
    pub fn world(&self) -> WorldId {
        self.world
    }
    pub fn artifact_digest(&self) -> Sha256Digest {
        self.artifact_digest
    }
    pub fn release_id(&self) -> &ReleaseId {
        &self.release_id
    }
    pub fn kind(&self) -> PublicationKind {
        self.kind
    }
    pub fn target(&self) -> DeploymentTarget {
        self.target
    }
    pub fn expected_current(&self) -> Option<Sha256Digest> {
        self.expected_current
    }
    pub fn requested_by(&self) -> &str {
        &self.requested_by
    }
    pub fn status(&self) -> PublicationStatus {
        self.status
    }
    pub fn reviewed_by(&self) -> Option<&str> {
        self.reviewed_by.as_deref()
    }
    pub fn review_reason(&self) -> Option<&str> {
        self.review_reason.as_deref()
    }
    pub fn activation_executor(&self) -> Option<&str> {
        self.activation_executor.as_deref()
    }
    pub fn review_context(&self) -> Option<&StudioReviewContext> {
        self.review_context.as_ref()
    }
}

impl ReleaseState {
    pub fn active(&self) -> Option<Sha256Digest> {
        self.active
    }
    pub fn known_good(&self) -> Option<Sha256Digest> {
        self.known_good
    }
    pub fn pending(&self) -> Option<ActivationId> {
        self.pending
    }
    pub fn maintenance(&self) -> bool {
        self.maintenance
    }
}

impl StoredArtifact {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn digest(&self) -> Sha256Digest {
        self.digest
    }
    pub fn release_id(&self) -> &ReleaseId {
        &self.release_id
    }
}

impl ActivationPlan {
    pub fn activation(&self) -> ActivationId {
        self.activation
    }
    pub fn request(&self) -> PublicationId {
        self.request
    }
    pub fn world(&self) -> WorldId {
        self.world
    }
    pub fn artifact(&self) -> &StoredArtifact {
        &self.artifact
    }
    pub fn kind(&self) -> PublicationKind {
        self.kind
    }
    pub fn target(&self) -> DeploymentTarget {
        self.target
    }
    pub fn previous(&self) -> Option<Sha256Digest> {
        self.previous
    }
    pub fn executor(&self) -> &str {
        &self.executor
    }
    pub fn authorization_version(&self) -> i16 {
        self.authorization_version
    }
    pub fn lease(&self) -> WriterLease {
        self.lease
    }
}

impl DurableContentReferences {
    pub fn room_definitions(&self) -> &[String] {
        &self.room_definitions
    }
    pub fn item_definitions(&self) -> &[String] {
        &self.item_definitions
    }
}

macro_rules! id_serde {
    ($module:ident, $ty:ty, $option_module:ident) => {
        #[allow(dead_code)]
        mod $module {
            use super::*;
            pub fn serialize<S: serde::Serializer>(
                value: &$ty,
                serializer: S,
            ) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&value.to_string())
            }
            pub fn deserialize<'de, D: serde::Deserializer<'de>>(
                deserializer: D,
            ) -> Result<$ty, D::Error> {
                let value = String::deserialize(deserializer)?;
                value.parse().map_err(serde::de::Error::custom)
            }
        }
        #[allow(dead_code)]
        mod $option_module {
            use super::*;
            pub fn serialize<S: serde::Serializer>(
                value: &Option<$ty>,
                serializer: S,
            ) -> Result<S::Ok, S::Error> {
                value
                    .as_ref()
                    .map(ToString::to_string)
                    .serialize(serializer)
            }
            pub fn deserialize<'de, D: serde::Deserializer<'de>>(
                deserializer: D,
            ) -> Result<Option<$ty>, D::Error> {
                Option::<String>::deserialize(deserializer)?
                    .map(|value| value.parse().map_err(serde::de::Error::custom))
                    .transpose()
            }
        }
    };
}
id_serde!(operation_id_serde, OperationId, option_operation_id_serde);
id_serde!(item_id_serde, ItemId, option_item_id_serde);
id_serde!(character_id_serde, CharacterId, option_character_id_serde);

pub(crate) fn uuid_of<T: IdValue>(id: T) -> Uuid {
    Uuid::from_u128(id.get_u128())
}

pub(crate) fn account_id(value: Uuid) -> Result<AccountId, crate::PersistenceError> {
    AccountId::from_u128(value.as_u128()).map_err(|_| crate::PersistenceError::InvalidData)
}
pub(crate) fn character_id(value: Uuid) -> Result<CharacterId, crate::PersistenceError> {
    CharacterId::from_u128(value.as_u128()).map_err(|_| crate::PersistenceError::InvalidData)
}
pub(crate) fn item_id(value: Uuid) -> Result<ItemId, crate::PersistenceError> {
    ItemId::from_u128(value.as_u128()).map_err(|_| crate::PersistenceError::InvalidData)
}
#[allow(dead_code)]
pub(crate) fn operation_id(value: Uuid) -> Result<OperationId, crate::PersistenceError> {
    OperationId::from_u128(value.as_u128()).map_err(|_| crate::PersistenceError::InvalidData)
}
#[allow(dead_code)]
pub(crate) fn world_id(value: Uuid) -> Result<WorldId, crate::PersistenceError> {
    WorldId::from_u128(value.as_u128()).map_err(|_| crate::PersistenceError::InvalidData)
}
#[allow(dead_code)]
pub(crate) fn writer_id(value: Uuid) -> Result<WriterId, crate::PersistenceError> {
    WriterId::from_u128(value.as_u128()).map_err(|_| crate::PersistenceError::InvalidData)
}

pub(crate) trait IdValue {
    fn get_u128(self) -> u128;
}
macro_rules! id_value { ($($ty:ty),+) => { $(impl IdValue for $ty { fn get_u128(self) -> u128 { self.get() } })+ }; }
id_value!(
    AccountId,
    CharacterId,
    ItemId,
    OperationId,
    WorldId,
    WriterId,
    PublicationId,
    ActivationId,
    AuthorityIncarnationId,
    CorrelationId,
    GrantId,
    OperatorSessionId,
    RecoveryJobId,
    InviteId
);

pub(crate) fn i64_from_u64(value: u64, positive: bool) -> Result<i64, crate::PersistenceError> {
    if positive && value == 0 {
        return Err(crate::PersistenceError::InvalidData);
    }
    i64::try_from(value).map_err(|_| crate::PersistenceError::InvalidData)
}
pub(crate) fn u64_from_i64(value: i64, positive: bool) -> Result<u64, crate::PersistenceError> {
    if (positive && value <= 0) || value < 0 {
        return Err(crate::PersistenceError::InvalidData);
    }
    u64::try_from(value).map_err(|_| crate::PersistenceError::InvalidData)
}

pub(crate) fn validate_username(value: &str) -> Result<(), crate::PersistenceError> {
    let bytes = value.as_bytes();
    if bytes.is_empty()
        || bytes.len() > 64
        || !bytes[0].is_ascii_lowercase()
        || bytes[1..]
            .iter()
            .any(|b| !b.is_ascii_lowercase() && !b.is_ascii_digit() && !matches!(b, b'_' | b'-'))
    {
        return Err(crate::PersistenceError::InvalidData);
    }
    Ok(())
}
pub(crate) fn validate_request_id(value: &str) -> Result<(), crate::PersistenceError> {
    let bytes = value.as_bytes();
    if bytes.is_empty()
        || bytes.len() > 128
        || !bytes[0].is_ascii_alphanumeric()
        || bytes[1..]
            .iter()
            .any(|b| !b.is_ascii_alphanumeric() && !matches!(b, b'.' | b'_' | b':' | b'-'))
    {
        return Err(crate::PersistenceError::InvalidData);
    }
    Ok(())
}
pub(crate) fn validate_name(value: &str) -> Result<(), crate::PersistenceError> {
    if value.is_empty()
        || value.len() > 64
        || value.trim().is_empty()
        || value.chars().any(char::is_control)
    {
        return Err(crate::PersistenceError::InvalidData);
    }
    Ok(())
}
pub(crate) fn validate_namespace(value: &str) -> Result<(), crate::PersistenceError> {
    let bytes = value.as_bytes();
    if bytes.is_empty()
        || bytes.len() > 192
        || !bytes[0].is_ascii_lowercase()
        || bytes[1..]
            .iter()
            .any(|b| !b.is_ascii_lowercase() && !b.is_ascii_digit() && !matches!(b, b'_' | b'-'))
    {
        return Err(crate::PersistenceError::InvalidData);
    }
    Ok(())
}
pub(crate) fn validate_definition(value: &str) -> Result<(), crate::PersistenceError> {
    value
        .parse::<worldwright_core::ids::DefinitionId>()
        .map(|_| ())
        .map_err(|_| crate::PersistenceError::InvalidData)
}
pub(crate) fn validate_reward_key(value: &str) -> Result<(), crate::PersistenceError> {
    let bytes = value.as_bytes();
    if bytes.is_empty()
        || bytes.len() > 64
        || !bytes[0].is_ascii_lowercase()
        || bytes[1..]
            .iter()
            .any(|b| !b.is_ascii_lowercase() && !b.is_ascii_digit() && !matches!(b, b'_' | b'-'))
    {
        return Err(crate::PersistenceError::InvalidData);
    }
    Ok(())
}

pub fn session_token_hash(token: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hasher.finalize().into()
}

pub(crate) fn fingerprint(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"worldwright-durable-intent-v1\0");
    for part in parts {
        hasher.update((part.len() as u32).to_be_bytes());
        hasher.update(part);
    }
    hasher.finalize().into()
}
pub(crate) fn u128_bytes<T: IdValue>(id: T) -> [u8; 16] {
    id.get_u128().to_be_bytes()
}
pub(crate) fn u64_bytes(value: u64) -> [u8; 8] {
    value.to_be_bytes()
}

impl fmt::Debug for WriterLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WriterLease")
            .field("world", &self.world)
            .field("writer", &self.writer)
            .field("epoch", &self.epoch)
            .field("incarnation", &self.incarnation)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ids_preserve_uuid_bits_and_fingerprints_are_length_prefixed() {
        let id = WorldId::from_u128(7).unwrap();
        assert_eq!(world_id(uuid_of(id)).unwrap(), id);
        assert_ne!(fingerprint(&[b"ab", b"c"]), fingerprint(&[b"a", b"bc"]));
        assert_eq!(session_token_hash("token").len(), 32);
    }
    #[test]
    fn typed_receipts_round_trip_and_reject_corruption() {
        let reward = RewardResult {
            operation: OperationId::from_u128(1).unwrap(),
            outcome: RewardOutcome::RewardGranted,
            balance: 100,
            item: Some(ItemId::from_u128(2).unwrap()),
            replayed: false,
        };
        let encoded = serde_json::to_string(&reward).unwrap();
        assert_eq!(
            serde_json::from_str::<RewardResult>(&encoded).unwrap(),
            reward
        );
        let corrupt = encoded.replace("reward_granted", "unknown");
        assert!(serde_json::from_str::<RewardResult>(&corrupt).is_err());
        let transfer = TransferResult {
            operation: OperationId::from_u128(3).unwrap(),
            outcome: TransferOutcome::ItemTransferred,
            item: ItemId::from_u128(4).unwrap(),
            owner: Some(CharacterId::from_u128(5).unwrap()),
            recipient_name: Some("Bob".to_owned()),
            replayed: true,
        };
        let encoded = serde_json::to_string(&transfer).unwrap();
        assert_eq!(
            serde_json::from_str::<TransferResult>(&encoded).unwrap(),
            transfer
        );
    }

    #[test]
    fn validators_reject_bad_durable_values() {
        assert!(validate_username("Alice").is_err());
        assert!(validate_request_id("bad/id").is_err());
        assert!(validate_name(" ").is_err());
        assert!(validate_definition("bad").is_err());
        assert!(validate_reward_key("Bad").is_err());
    }
}
