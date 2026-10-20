//! Typed input only. Constructors do not authenticate a caller.

use crate::ids::{AccountId, CharacterId, ConnectionId, OperationId, RoomId, WorldId};
use std::{error::Error, fmt, num::NonZeroU64};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputError {
    EmptyText,
    TextTooLong,
    ControlCharacter,
    ZeroGeneration,
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::EmptyText => "text cannot be empty or whitespace only",
            Self::TextTooLong => "text exceeds the byte limit",
            Self::ControlCharacter => "control characters are not allowed",
            Self::ZeroGeneration => "connection generation cannot be zero",
        })
    }
}
impl Error for InputError {}

/// Bounded, single-line text. This is NOT HTML escaping or terminal rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerText(String);

impl PlayerText {
    pub const MAX_BYTES: usize = 4096;

    pub fn new(text: &str) -> Result<Self, InputError> {
        if text.len() > Self::MAX_BYTES {
            return Err(InputError::TextTooLong);
        }
        if text.chars().any(char::is_control) {
            return Err(InputError::ControlCharacter);
        }
        if text.trim().is_empty() {
            return Err(InputError::EmptyText);
        }
        Ok(Self(text.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionGeneration(NonZeroU64);

impl ConnectionGeneration {
    pub fn new(value: u64) -> Result<Self, InputError> {
        NonZeroU64::new(value)
            .map(Self)
            .ok_or(InputError::ZeroGeneration)
    }
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Application-supplied context; the future executor must revalidate it.
/// Possessing these values does not grant access or prove session validity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActorContext {
    pub account: AccountId,
    pub character: CharacterId,
    pub connection: ConnectionId,
    pub generation: ConnectionGeneration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CommandKind {
    Look,
    Move,
    Say,
}

/// Move uses a resolved room ID. Syntax parsing and visibility resolution are
/// adapter responsibilities; destination authorization remains an engine check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandIntent {
    Look,
    Move { destination: RoomId },
    Say { text: PlayerText },
}

impl CommandIntent {
    pub const fn kind(&self) -> CommandKind {
        match self {
            Self::Look => CommandKind::Look,
            Self::Move { .. } => CommandKind::Move,
            Self::Say { .. } => CommandKind::Say,
        }
    }
}

/// An operation ID is a correlation/retry identity, not a durable receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandEnvelope {
    pub operation: OperationId,
    pub world: WorldId,
    pub actor: ActorContext,
    pub intent: CommandIntent,
}
