use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;
pub const STATUS: &str = "protocol v1 implemented; transport sessions are process-local";
pub const SUPPORTED_PROTOCOL_VERSIONS: &[u32] = &[PROTOCOL_VERSION];
pub const MAX_FRAME_BYTES: usize = 16 * 1024;
pub const MAX_OFFERED_VERSIONS: usize = 8;
pub const MAX_REQUEST_ID_BYTES: usize = 128;
pub const MAX_EXIT_LABEL_BYTES: usize = 64;
pub const MAX_TEXT_BYTES: usize = 4096;
pub const MAX_ERROR_TEXT_BYTES: usize = 512;
pub const MAX_CHARACTER_NAME_BYTES: usize = 64;
pub const MAX_EXITS: usize = 64;
pub const MAX_REWARD_KEY_BYTES: usize = 64;
pub const MAX_USERNAME_BYTES: usize = 64;
pub const MAX_DEFINITION_ID_BYTES: usize = 192;
pub const MAX_INVENTORY_ITEMS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClientMessage {
    Hello {
        supported_versions: Vec<u32>,
    },
    Attach {
        request_id: String,
    },
    Command {
        request_id: String,
        command: ClientCommand,
    },
    Ping {
        nonce: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClientCommand {
    Look,
    Move { exit: String },
    Say { text: String },
    Inventory,
    ClaimReward { reward: String },
    TransferItem { item_id: String, recipient: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerEnvelope {
    pub protocol_version: u32,
    pub stream_epoch: String,
    pub sequence: u64,
    pub message: ServerMessage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ServerMessage {
    Welcome {
        heartbeat_interval_ms: u64,
    },
    Accepted {
        request_id: String,
    },
    Attached {
        request_id: String,
        character_name: String,
    },
    Output {
        request_id: Option<String>,
        kind: OutputKind,
        speaker: Option<String>,
        text: String,
    },
    Exits {
        request_id: Option<String>,
        exits: Vec<String>,
    },
    CommandResult {
        request_id: String,
        revision: u64,
    },
    Error {
        request_id: Option<String>,
        code: ErrorCode,
        message: String,
        fatal: bool,
    },
    RewardResult {
        request_id: String,
        operation_id: String,
        outcome: RewardOutcome,
        balance: u64,
        item_id: Option<String>,
        replayed: bool,
    },
    TransferResult {
        request_id: String,
        operation_id: String,
        outcome: TransferOutcome,
        item_id: String,
        owner_character_id: Option<String>,
        recipient_name: Option<String>,
        replayed: bool,
    },
    Inventory {
        request_id: String,
        balance: u64,
        character_version: u64,
        items: Vec<InventoryItem>,
    },
    Pong {
        nonce: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardOutcome {
    RewardGranted,
    AlreadyClaimed,
    InventoryFull,
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
pub struct InventoryItem {
    pub item_id: String,
    pub definition_id: String,
    pub version: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputKind {
    Room,
    Movement,
    Speech,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidMessage,
    UnsupportedVersion,
    ProtocolState,
    Unauthenticated,
    Unauthorized,
    InvalidCommand,
    NotFound,
    Overloaded,
    StaleAttachment,
    SessionReplaced,
    InternalError,
    DurabilityUnavailable,
    Conflict,
    OperationPending,
    WriterFenced,
    TemporarilyUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolErrorKind {
    FrameTooLarge,
    InvalidJson,
    InvalidMessage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolError {
    kind: ProtocolErrorKind,
}

impl ProtocolError {
    pub const fn kind(&self) -> ProtocolErrorKind {
        self.kind
    }

    const fn frame_too_large() -> Self {
        Self {
            kind: ProtocolErrorKind::FrameTooLarge,
        }
    }

    const fn invalid_json() -> Self {
        Self {
            kind: ProtocolErrorKind::InvalidJson,
        }
    }

    const fn invalid_message() -> Self {
        Self {
            kind: ProtocolErrorKind::InvalidMessage,
        }
    }
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self.kind {
            ProtocolErrorKind::FrameTooLarge => "protocol frame exceeds 16384 bytes",
            ProtocolErrorKind::InvalidJson => "protocol frame is not valid JSON",
            ProtocolErrorKind::InvalidMessage => "protocol message violates the v1 contract",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ProtocolError {}

pub fn decode_client(frame: &str) -> Result<ClientMessage, ProtocolError> {
    if frame.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::frame_too_large());
    }

    let message = serde_json::from_str(frame).map_err(|_| ProtocolError::invalid_json())?;
    validate_client(&message)?;
    Ok(message)
}

pub fn encode_server(envelope: &ServerEnvelope) -> Result<String, ProtocolError> {
    validate_server(envelope)?;
    let frame = serde_json::to_string(envelope).map_err(|_| ProtocolError::invalid_message())?;
    if frame.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::frame_too_large());
    }
    Ok(frame)
}

pub fn negotiate_version(offered: &[u32]) -> Option<u32> {
    SUPPORTED_PROTOCOL_VERSIONS
        .iter()
        .copied()
        .filter(|version| offered.contains(version))
        .max()
}

fn validate_client(message: &ClientMessage) -> Result<(), ProtocolError> {
    match message {
        ClientMessage::Hello { supported_versions } => {
            if supported_versions.is_empty() || supported_versions.len() > MAX_OFFERED_VERSIONS {
                return Err(ProtocolError::invalid_message());
            }
            for (index, version) in supported_versions.iter().enumerate() {
                if *version == 0 || supported_versions[..index].contains(version) {
                    return Err(ProtocolError::invalid_message());
                }
            }
        }
        ClientMessage::Attach { request_id } => validate_request_id(request_id)?,
        ClientMessage::Command {
            request_id,
            command,
        } => {
            validate_request_id(request_id)?;
            match command {
                ClientCommand::Look => {}
                ClientCommand::Move { exit } => validate_exit_label(exit)?,
                ClientCommand::Say { text } => validate_text(text, MAX_TEXT_BYTES)?,
                ClientCommand::Inventory => {}
                ClientCommand::ClaimReward { reward } => validate_reward_key(reward)?,
                ClientCommand::TransferItem { item_id, recipient } => {
                    validate_instance_id(item_id)?;
                    validate_username(recipient)?;
                }
            }
        }
        ClientMessage::Ping { .. } => {}
    }
    Ok(())
}

fn validate_server(envelope: &ServerEnvelope) -> Result<(), ProtocolError> {
    if envelope.protocol_version != PROTOCOL_VERSION
        || !valid_stream_epoch(&envelope.stream_epoch)
        || envelope.sequence == 0
    {
        return Err(ProtocolError::invalid_message());
    }

    match &envelope.message {
        ServerMessage::Welcome {
            heartbeat_interval_ms,
        } => {
            if !(1_000..=120_000).contains(heartbeat_interval_ms) {
                return Err(ProtocolError::invalid_message());
            }
        }
        ServerMessage::Accepted { request_id } => validate_request_id(request_id)?,
        ServerMessage::Attached {
            request_id,
            character_name,
        } => {
            validate_request_id(request_id)?;
            validate_text(character_name, MAX_CHARACTER_NAME_BYTES)?;
        }
        ServerMessage::Output {
            request_id,
            speaker,
            text,
            ..
        } => {
            if let Some(request_id) = request_id {
                validate_request_id(request_id)?;
            }
            if let Some(speaker) = speaker {
                validate_text(speaker, MAX_CHARACTER_NAME_BYTES)?;
            }
            validate_text(text, MAX_TEXT_BYTES)?;
        }
        ServerMessage::Exits { request_id, exits } => {
            if let Some(request_id) = request_id {
                validate_request_id(request_id)?;
            }
            if exits.len() > MAX_EXITS
                || exits.iter().any(|exit| validate_exit_label(exit).is_err())
            {
                return Err(ProtocolError::invalid_message());
            }
        }
        ServerMessage::CommandResult {
            request_id,
            revision,
        } => {
            validate_request_id(request_id)?;
            if *revision == 0 {
                return Err(ProtocolError::invalid_message());
            }
        }
        ServerMessage::Error {
            request_id,
            message,
            ..
        } => {
            if let Some(request_id) = request_id {
                validate_request_id(request_id)?;
            }
            validate_text(message, MAX_ERROR_TEXT_BYTES)?;
        }
        ServerMessage::RewardResult {
            request_id,
            operation_id,
            outcome,
            balance: _balance,
            item_id,
            ..
        } => {
            validate_request_id(request_id)?;
            validate_instance_id(operation_id)?;
            if let Some(item_id) = item_id {
                validate_instance_id(item_id)?;
            }
            match outcome {
                RewardOutcome::RewardGranted | RewardOutcome::AlreadyClaimed
                    if item_id.is_none() =>
                {
                    return Err(ProtocolError::invalid_message());
                }
                RewardOutcome::InventoryFull if item_id.is_some() => {
                    return Err(ProtocolError::invalid_message());
                }
                _ => {}
            }
        }
        ServerMessage::TransferResult {
            request_id,
            operation_id,
            item_id,
            owner_character_id,
            recipient_name,
            outcome,
            ..
        } => {
            validate_request_id(request_id)?;
            validate_instance_id(operation_id)?;
            validate_instance_id(item_id)?;
            if let Some(owner) = owner_character_id {
                validate_instance_id(owner)?;
            }
            if let Some(name) = recipient_name {
                validate_text(name, MAX_CHARACTER_NAME_BYTES)?;
            }
            let valid = match outcome {
                TransferOutcome::ItemTransferred => {
                    owner_character_id.is_some() && recipient_name.is_some()
                }
                TransferOutcome::NotOwned => {
                    owner_character_id.is_none() && recipient_name.is_some()
                }
                TransferOutcome::SameOwner => {
                    owner_character_id.is_some() && recipient_name.is_some()
                }
                TransferOutcome::TargetNotFound => {
                    owner_character_id.is_none() && recipient_name.is_none()
                }
                TransferOutcome::InventoryFull => {
                    owner_character_id.is_some() && recipient_name.is_some()
                }
            };
            if !valid {
                return Err(ProtocolError::invalid_message());
            }
        }
        ServerMessage::Inventory {
            request_id,
            character_version,
            items,
            ..
        } => {
            validate_request_id(request_id)?;
            if *character_version == 0
                || items.len() > MAX_INVENTORY_ITEMS
                || items
                    .iter()
                    .any(|item| validate_inventory_item(item).is_err())
            {
                return Err(ProtocolError::invalid_message());
            }
        }
        ServerMessage::Pong { .. } => {}
    }
    Ok(())
}

fn validate_request_id(value: &str) -> Result<(), ProtocolError> {
    let bytes = value.as_bytes();
    if bytes.is_empty()
        || bytes.len() > MAX_REQUEST_ID_BYTES
        || !bytes[0].is_ascii_alphanumeric()
        || bytes[1..]
            .iter()
            .any(|byte| !byte.is_ascii_alphanumeric() && !matches!(byte, b'.' | b'_' | b':' | b'-'))
    {
        return Err(ProtocolError::invalid_message());
    }
    Ok(())
}

fn validate_reward_key(value: &str) -> Result<(), ProtocolError> {
    let bytes = value.as_bytes();
    if bytes.is_empty()
        || bytes.len() > MAX_REWARD_KEY_BYTES
        || !bytes[0].is_ascii_lowercase()
        || bytes[1..].iter().any(|byte| {
            !byte.is_ascii_lowercase() && !byte.is_ascii_digit() && !matches!(byte, b'_' | b'-')
        })
    {
        return Err(ProtocolError::invalid_message());
    }
    Ok(())
}

fn validate_username(value: &str) -> Result<(), ProtocolError> {
    let bytes = value.as_bytes();
    if bytes.is_empty()
        || bytes.len() > MAX_USERNAME_BYTES
        || !bytes[0].is_ascii_lowercase()
        || bytes[1..].iter().any(|byte| {
            !byte.is_ascii_lowercase() && !byte.is_ascii_digit() && !matches!(byte, b'_' | b'-')
        })
    {
        return Err(ProtocolError::invalid_message());
    }
    Ok(())
}

fn validate_instance_id(value: &str) -> Result<(), ProtocolError> {
    if value.len() != 32
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || value.bytes().all(|byte| byte == b'0')
    {
        return Err(ProtocolError::invalid_message());
    }
    Ok(())
}

fn validate_definition_id(value: &str) -> Result<(), ProtocolError> {
    if value.len() < 3 || value.len() > MAX_DEFINITION_ID_BYTES {
        return Err(ProtocolError::invalid_message());
    }
    let Some((namespace, path)) = value.split_once(':') else {
        return Err(ProtocolError::invalid_message());
    };
    if !valid_definition_segment(namespace) || !path.split('.').all(valid_definition_segment) {
        return Err(ProtocolError::invalid_message());
    }
    Ok(())
}

fn valid_definition_segment(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes[0].is_ascii_lowercase()
        && bytes[1..].iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

fn validate_inventory_item(item: &InventoryItem) -> Result<(), ProtocolError> {
    validate_instance_id(&item.item_id)?;
    validate_definition_id(&item.definition_id)?;
    if item.version == 0 {
        return Err(ProtocolError::invalid_message());
    }
    Ok(())
}

fn validate_exit_label(value: &str) -> Result<(), ProtocolError> {
    let bytes = value.as_bytes();
    if bytes.is_empty()
        || bytes.len() > MAX_EXIT_LABEL_BYTES
        || !bytes[0].is_ascii_lowercase()
        || bytes[1..].iter().any(|byte| {
            !byte.is_ascii_lowercase() && !byte.is_ascii_digit() && !matches!(byte, b'_' | b'-')
        })
    {
        return Err(ProtocolError::invalid_message());
    }
    Ok(())
}

fn validate_text(value: &str, max_bytes: usize) -> Result<(), ProtocolError> {
    if value.is_empty()
        || value.len() > max_bytes
        || value.trim().is_empty()
        || value.chars().any(char::is_control)
    {
        return Err(ProtocolError::invalid_message());
    }
    Ok(())
}

fn valid_stream_epoch(value: &str) -> bool {
    value.len() == 32
        && value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::*;

    fn valid_envelope(message: ServerMessage) -> ServerEnvelope {
        ServerEnvelope {
            protocol_version: PROTOCOL_VERSION,
            stream_epoch: "0123456789abcdef0123456789abcdef".to_owned(),
            sequence: 1,
            message,
        }
    }

    fn valid_output() -> ServerMessage {
        ServerMessage::Output {
            request_id: Some("req-1".to_owned()),
            kind: OutputKind::Speech,
            speaker: Some("Ada".to_owned()),
            text: "hello".to_owned(),
        }
    }

    fn invalid_client(frame: &str) -> ProtocolErrorKind {
        decode_client(frame)
            .expect_err("message should be rejected")
            .kind()
    }

    #[test]
    fn valid_client_messages_decode() {
        assert_eq!(
            decode_client(r#"{"type":"hello","supported_versions":[2,1]}"#),
            Ok(ClientMessage::Hello {
                supported_versions: vec![2, 1]
            })
        );
        assert_eq!(
            decode_client(r#"{"type":"attach","request_id":"attach-1"}"#),
            Ok(ClientMessage::Attach {
                request_id: "attach-1".to_owned()
            })
        );
        assert_eq!(
            decode_client(r#"{"type":"command","request_id":"look-1","command":{"type":"look"}}"#),
            Ok(ClientMessage::Command {
                request_id: "look-1".to_owned(),
                command: ClientCommand::Look
            })
        );
        assert_eq!(
            decode_client(
                r#"{"type":"command","request_id":"move-1","command":{"type":"move","exit":"north-1"}}"#
            ),
            Ok(ClientMessage::Command {
                request_id: "move-1".to_owned(),
                command: ClientCommand::Move {
                    exit: "north-1".to_owned()
                }
            })
        );
        assert_eq!(
            decode_client(
                r#"{"type":"command","request_id":"say-1","command":{"type":"say","text":"hello"}}"#
            ),
            Ok(ClientMessage::Command {
                request_id: "say-1".to_owned(),
                command: ClientCommand::Say {
                    text: "hello".to_owned()
                }
            })
        );
        assert_eq!(
            decode_client(r#"{"type":"ping","nonce":18446744073709551615}"#),
            Ok(ClientMessage::Ping { nonce: u64::MAX })
        );
        assert_eq!(
            decode_client(
                r#"{"type":"command","request_id":"inventory-1","command":{"type":"inventory"}}"#
            ),
            Ok(ClientMessage::Command {
                request_id: "inventory-1".to_owned(),
                command: ClientCommand::Inventory
            })
        );
        assert_eq!(
            decode_client(
                r#"{"type":"command","request_id":"reward-1","command":{"type":"claim_reward","reward":"welcome"}}"#
            ),
            Ok(ClientMessage::Command {
                request_id: "reward-1".to_owned(),
                command: ClientCommand::ClaimReward {
                    reward: "welcome".to_owned()
                }
            })
        );
        assert_eq!(
            decode_client(
                r#"{"type":"command","request_id":"transfer-1","command":{"type":"transfer_item","item_id":"00000000000000000000000000000001","recipient":"alice_1"}}"#
            ),
            Ok(ClientMessage::Command {
                request_id: "transfer-1".to_owned(),
                command: ClientCommand::TransferItem {
                    item_id: "00000000000000000000000000000001".to_owned(),
                    recipient: "alice_1".to_owned()
                }
            })
        );
    }

    #[test]
    fn version_negotiation_selects_highest_mutual_version() {
        assert_eq!(negotiate_version(&[9, 1, 4]), Some(PROTOCOL_VERSION));
        assert_eq!(negotiate_version(&[9, 4]), None);
    }

    #[test]
    fn malformed_non_object_trailing_unknown_and_duplicate_json_are_rejected() {
        assert_eq!(invalid_client("{"), ProtocolErrorKind::InvalidJson);
        assert_eq!(invalid_client("[]"), ProtocolErrorKind::InvalidJson);
        assert_eq!(
            invalid_client(r#"{"type":"ping","nonce":1} null"#),
            ProtocolErrorKind::InvalidJson
        );
        assert_eq!(
            invalid_client(r#"{"type":"ping","nonce":1,"extra":true}"#),
            ProtocolErrorKind::InvalidJson
        );
        assert_eq!(
            invalid_client(r#"{"type":"ping","nonce":1,"nonce":2}"#),
            ProtocolErrorKind::InvalidJson
        );
    }

    #[test]
    fn hello_versions_are_bounded_nonzero_and_unique() {
        assert_eq!(
            invalid_client(r#"{"type":"hello","supported_versions":[]}"#),
            ProtocolErrorKind::InvalidMessage
        );
        assert_eq!(
            invalid_client(r#"{"type":"hello","supported_versions":[0]}"#),
            ProtocolErrorKind::InvalidMessage
        );
        assert_eq!(
            invalid_client(r#"{"type":"hello","supported_versions":[1,1]}"#),
            ProtocolErrorKind::InvalidMessage
        );
        assert_eq!(
            invalid_client(r#"{"type":"hello","supported_versions":[1,2,3,4,5,6,7,8,9]}"#),
            ProtocolErrorKind::InvalidMessage
        );
        assert!(
            decode_client(r#"{"type":"hello","supported_versions":[1,2,3,4,5,6,7,8]}"#).is_ok()
        );
    }

    #[test]
    fn request_ids_enforce_byte_boundaries_and_alphabet() {
        let valid = "a".to_owned() + &".".repeat(MAX_REQUEST_ID_BYTES - 1);
        assert!(decode_client(&format!(r#"{{"type":"attach","request_id":"{valid}"}}"#)).is_ok());
        let too_long = "a".to_owned() + &"b".repeat(MAX_REQUEST_ID_BYTES);
        assert_eq!(
            invalid_client(&format!(r#"{{"type":"attach","request_id":"{too_long}"}}"#)),
            ProtocolErrorKind::InvalidMessage
        );
        for request_id in ["", "_bad", "-bad", "a/b", "é"] {
            assert_eq!(
                invalid_client(&format!(
                    r#"{{"type":"attach","request_id":"{request_id}"}}"#
                )),
                ProtocolErrorKind::InvalidMessage
            );
        }
    }

    #[test]
    fn exit_labels_enforce_boundaries_and_alphabet() {
        let valid = "a".to_owned() + &"b".repeat(MAX_EXIT_LABEL_BYTES - 1);
        let frame = format!(
            r#"{{"type":"command","request_id":"r","command":{{"type":"move","exit":"{valid}"}}}}"#
        );
        assert!(decode_client(&frame).is_ok());
        for exit in [
            "",
            "A",
            "1north",
            "north.",
            "north/",
            &("a".to_owned() + &"b".repeat(64)),
        ] {
            let frame = format!(
                r#"{{"type":"command","request_id":"r","command":{{"type":"move","exit":"{exit}"}}}}"#
            );
            assert_eq!(invalid_client(&frame), ProtocolErrorKind::InvalidMessage);
        }
    }

    #[test]
    fn speech_text_enforces_blank_control_and_utf8_byte_bounds() {
        for text in ["", "   ", "hello\nworld", "\u{0085}"] {
            let frame = serde_json::json!({
                "type": "command",
                "request_id": "say",
                "command": {"type": "say", "text": text}
            })
            .to_string();
            assert_eq!(invalid_client(&frame), ProtocolErrorKind::InvalidMessage);
        }
        let at_limit = "é".repeat(MAX_TEXT_BYTES / 2);
        let frame = serde_json::json!({
            "type": "command",
            "request_id": "say",
            "command": {"type": "say", "text": at_limit}
        })
        .to_string();
        assert!(decode_client(&frame).is_ok());
        let over_limit = "é".repeat(MAX_TEXT_BYTES / 2 + 1);
        let frame = serde_json::json!({
            "type": "command",
            "request_id": "say",
            "command": {"type": "say", "text": over_limit}
        })
        .to_string();
        assert_eq!(invalid_client(&frame), ProtocolErrorKind::InvalidMessage);
    }

    #[test]
    fn durable_messages_round_trip_and_enforce_bounds() {
        let operation_id = "00000000000000000000000000000001".to_owned();
        let item_id = "00000000000000000000000000000002".to_owned();
        let reward = ServerMessage::RewardResult {
            request_id: "reward".to_owned(),
            operation_id: operation_id.clone(),
            outcome: RewardOutcome::RewardGranted,
            balance: 100,
            item_id: Some(item_id.clone()),
            replayed: false,
        };
        let transfer = ServerMessage::TransferResult {
            request_id: "transfer".to_owned(),
            operation_id: operation_id.clone(),
            outcome: TransferOutcome::ItemTransferred,
            item_id: item_id.clone(),
            owner_character_id: Some("00000000000000000000000000000003".to_owned()),
            recipient_name: Some("Alice".to_owned()),
            replayed: false,
        };
        let inventory = ServerMessage::Inventory {
            request_id: "inventory".to_owned(),
            balance: 0,
            character_version: 1,
            items: vec![InventoryItem {
                item_id: item_id.clone(),
                definition_id: "reference:items.token".to_owned(),
                version: 1,
            }],
        };
        for message in [reward, transfer, inventory] {
            let frame = encode_server(&valid_envelope(message.clone())).expect("durable frame");
            let decoded: ServerEnvelope = serde_json::from_str(&frame).expect("durable JSON");
            assert_eq!(decoded.message, message);
        }
        let items = vec![
            InventoryItem {
                item_id: item_id.clone(),
                definition_id: "reference:items.token".to_owned(),
                version: 1
            };
            MAX_INVENTORY_ITEMS
        ];
        let at_limit = valid_envelope(ServerMessage::Inventory {
            request_id: "i".to_owned(),
            balance: 0,
            character_version: 1,
            items,
        });
        assert!(validate_server(&at_limit).is_ok());
        assert_eq!(
            encode_server(&at_limit).unwrap_err().kind(),
            ProtocolErrorKind::FrameTooLarge
        );
        assert!(
            encode_server(&valid_envelope(ServerMessage::RewardResult {
                request_id: "r".to_owned(),
                operation_id: "00000000000000000000000000000001".to_owned(),
                outcome: RewardOutcome::InventoryFull,
                balance: 100,
                item_id: None,
                replayed: false
            }))
            .is_ok()
        );
        let too_many = vec![
            InventoryItem {
                item_id: item_id.clone(),
                definition_id: "reference:items.token".to_owned(),
                version: 1
            };
            MAX_INVENTORY_ITEMS + 1
        ];
        assert_eq!(
            encode_server(&valid_envelope(ServerMessage::Inventory {
                request_id: "i".to_owned(),
                balance: 0,
                character_version: 1,
                items: too_many
            }))
            .unwrap_err()
            .kind(),
            ProtocolErrorKind::InvalidMessage
        );
        for command in [
            serde_json::json!({"type":"command","request_id":"r","command":{"type":"claim_reward","reward":""}}),
            serde_json::json!({"type":"command","request_id":"r","command":{"type":"claim_reward","reward":"A"}}),
            serde_json::json!({"type":"command","request_id":"r","command":{"type":"transfer_item","item_id":"00000000000000000000000000000000","recipient":"alice"}}),
            serde_json::json!({"type":"command","request_id":"r","command":{"type":"transfer_item","item_id":"00000000000000000000000000000001","recipient":"Alice"}}),
        ] {
            assert_eq!(
                invalid_client(&command.to_string()),
                ProtocolErrorKind::InvalidMessage
            );
        }
        assert!(serde_json::from_str::<InventoryItem>(r#"{"item_id":"00000000000000000000000000000001","definition_id":"reference:item","version":1,"extra":true}"#).is_err());
        assert!(serde_json::from_str::<ServerEnvelope>(r#"{"protocol_version":1,"stream_epoch":"0123456789abcdef0123456789abcdef","sequence":1,"message":{"type":"pong","nonce":1},"extra":true}"#).is_err());
        assert_eq!(
            serde_json::to_string(&ErrorCode::DurabilityUnavailable).unwrap(),
            "\"durability_unavailable\""
        );
        assert_eq!(
            serde_json::to_string(&ErrorCode::TemporarilyUnavailable).unwrap(),
            "\"temporarily_unavailable\""
        );
    }

    #[test]
    fn durable_result_shapes_reject_impossible_combinations() {
        let operation_id = "00000000000000000000000000000001".to_owned();
        let item_id = "00000000000000000000000000000002".to_owned();
        let invalid = [
            ServerMessage::RewardResult {
                request_id: "r".to_owned(),
                operation_id: operation_id.clone(),
                outcome: RewardOutcome::RewardGranted,
                balance: 1,
                item_id: None,
                replayed: false,
            },
            ServerMessage::RewardResult {
                request_id: "r".to_owned(),
                operation_id: operation_id.clone(),
                outcome: RewardOutcome::InventoryFull,
                balance: 1,
                item_id: Some(item_id.clone()),
                replayed: false,
            },
            ServerMessage::TransferResult {
                request_id: "r".to_owned(),
                operation_id: operation_id.clone(),
                outcome: TransferOutcome::NotOwned,
                item_id: item_id.clone(),
                owner_character_id: Some(operation_id.clone()),
                recipient_name: Some("Alice".to_owned()),
                replayed: false,
            },
            ServerMessage::TransferResult {
                request_id: "r".to_owned(),
                operation_id: operation_id.clone(),
                outcome: TransferOutcome::TargetNotFound,
                item_id: item_id.clone(),
                owner_character_id: None,
                recipient_name: Some("Alice".to_owned()),
                replayed: false,
            },
            ServerMessage::TransferResult {
                request_id: "r".to_owned(),
                operation_id,
                outcome: TransferOutcome::InventoryFull,
                item_id,
                owner_character_id: None,
                recipient_name: None,
                replayed: false,
            },
        ];
        for message in invalid {
            assert_eq!(
                encode_server(&valid_envelope(message)).unwrap_err().kind(),
                ProtocolErrorKind::InvalidMessage
            );
        }
    }

    #[test]
    fn server_envelope_round_trips_with_snake_case_tags() {
        let envelope = valid_envelope(valid_output());
        let frame = encode_server(&envelope).expect("valid envelope");
        assert!(frame.contains(r#""type":"output""#));
        assert!(frame.contains(r#""kind":"speech""#));
        let decoded: ServerEnvelope = serde_json::from_str(&frame).expect("valid JSON");
        assert_eq!(decoded, envelope);
    }

    #[test]
    fn every_server_message_variant_has_valid_bounds() {
        let messages = [
            ServerMessage::Welcome {
                heartbeat_interval_ms: 1_000,
            },
            ServerMessage::Accepted {
                request_id: "r".to_owned(),
            },
            ServerMessage::Attached {
                request_id: "r".to_owned(),
                character_name: "A".to_owned(),
            },
            valid_output(),
            ServerMessage::Exits {
                request_id: Some("r".to_owned()),
                exits: vec!["north".to_owned()],
            },
            ServerMessage::CommandResult {
                request_id: "r".to_owned(),
                revision: 1,
            },
            ServerMessage::Error {
                request_id: None,
                code: ErrorCode::InternalError,
                message: "failure".to_owned(),
                fatal: false,
            },
            ServerMessage::Pong { nonce: u64::MAX },
        ];
        for message in messages {
            assert!(encode_server(&valid_envelope(message)).is_ok());
        }
    }

    #[test]
    fn server_rejects_invalid_version_epoch_sequence_and_heartbeat() {
        let mut envelope = valid_envelope(ServerMessage::Pong { nonce: 1 });
        envelope.protocol_version = 2;
        assert_eq!(
            encode_server(&envelope).unwrap_err().kind(),
            ProtocolErrorKind::InvalidMessage
        );
        envelope.protocol_version = PROTOCOL_VERSION;
        envelope.stream_epoch = "0123456789abcdef0123456789abcdeF".to_owned();
        assert_eq!(
            encode_server(&envelope).unwrap_err().kind(),
            ProtocolErrorKind::InvalidMessage
        );
        envelope.stream_epoch = "short".to_owned();
        assert_eq!(
            encode_server(&envelope).unwrap_err().kind(),
            ProtocolErrorKind::InvalidMessage
        );
        envelope.stream_epoch = "0123456789abcdef0123456789abcdef".to_owned();
        envelope.sequence = 0;
        assert_eq!(
            encode_server(&envelope).unwrap_err().kind(),
            ProtocolErrorKind::InvalidMessage
        );
        for heartbeat_interval_ms in [0, 999, 120_001] {
            envelope.sequence = 1;
            envelope.message = ServerMessage::Welcome {
                heartbeat_interval_ms,
            };
            assert_eq!(
                encode_server(&envelope).unwrap_err().kind(),
                ProtocolErrorKind::InvalidMessage
            );
        }
    }

    #[test]
    fn server_rejects_each_semantic_bound() {
        let mut envelope = valid_envelope(ServerMessage::Attached {
            request_id: "r".to_owned(),
            character_name: "name".to_owned(),
        });
        let invalid_names = [
            "",
            "   ",
            "name\n",
            &"a".repeat(MAX_CHARACTER_NAME_BYTES + 1),
        ];
        for name in invalid_names {
            envelope.message = ServerMessage::Attached {
                request_id: "r".to_owned(),
                character_name: name.to_owned(),
            };
            assert_eq!(
                encode_server(&envelope).unwrap_err().kind(),
                ProtocolErrorKind::InvalidMessage
            );
        }

        envelope.message = ServerMessage::Output {
            request_id: Some("bad/id".to_owned()),
            kind: OutputKind::Room,
            speaker: None,
            text: "room".to_owned(),
        };
        assert_eq!(
            encode_server(&envelope).unwrap_err().kind(),
            ProtocolErrorKind::InvalidMessage
        );
        for text in ["", " ", "text\u{0000}", &"a".repeat(MAX_TEXT_BYTES + 1)] {
            envelope.message = ServerMessage::Output {
                request_id: None,
                kind: OutputKind::Room,
                speaker: None,
                text: text.to_owned(),
            };
            assert_eq!(
                encode_server(&envelope).unwrap_err().kind(),
                ProtocolErrorKind::InvalidMessage
            );
        }

        envelope.message = ServerMessage::Exits {
            request_id: None,
            exits: vec!["north".to_owned(); MAX_EXITS + 1],
        };
        assert_eq!(
            encode_server(&envelope).unwrap_err().kind(),
            ProtocolErrorKind::InvalidMessage
        );
        envelope.message = ServerMessage::Exits {
            request_id: None,
            exits: vec!["North".to_owned()],
        };
        assert_eq!(
            encode_server(&envelope).unwrap_err().kind(),
            ProtocolErrorKind::InvalidMessage
        );

        envelope.message = ServerMessage::CommandResult {
            request_id: "r".to_owned(),
            revision: 0,
        };
        assert_eq!(
            encode_server(&envelope).unwrap_err().kind(),
            ProtocolErrorKind::InvalidMessage
        );
        envelope.message = ServerMessage::Error {
            request_id: None,
            code: ErrorCode::InternalError,
            message: "a".repeat(MAX_ERROR_TEXT_BYTES + 1),
            fatal: true,
        };
        assert_eq!(
            encode_server(&envelope).unwrap_err().kind(),
            ProtocolErrorKind::InvalidMessage
        );
    }

    #[test]
    fn oversized_frames_are_rejected_before_parse_and_after_semantic_validation() {
        let inbound = "{".to_owned() + &" ".repeat(MAX_FRAME_BYTES);
        let error = decode_client(&inbound).unwrap_err();
        assert_eq!(error.kind(), ProtocolErrorKind::FrameTooLarge);
        assert_eq!(error.to_string(), "protocol frame exceeds 16384 bytes");

        let outbound = valid_envelope(ServerMessage::Output {
            request_id: None,
            kind: OutputKind::Speech,
            speaker: None,
            text: "a".repeat(MAX_TEXT_BYTES + 1),
        });
        let error = encode_server(&outbound).unwrap_err();
        assert_eq!(error.kind(), ProtocolErrorKind::InvalidMessage);
        assert_eq!(
            error.to_string(),
            "protocol message violates the v1 contract"
        );
    }

    #[test]
    fn compatibility_status_identifies_protocol_v1() {
        assert_eq!(
            STATUS,
            "protocol v1 implemented; transport sessions are process-local"
        );
    }

    #[test]
    fn protocol_error_display_is_static_and_kind_specific() {
        assert_eq!(
            ProtocolError::invalid_json().to_string(),
            "protocol frame is not valid JSON"
        );
        assert_eq!(
            ProtocolError::invalid_message().to_string(),
            "protocol message violates the v1 contract"
        );
        assert!(ProtocolError::invalid_json().source().is_none());
    }
}
