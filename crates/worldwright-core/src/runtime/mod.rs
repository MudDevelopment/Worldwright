//! Single-owner, in-memory command execution. No login, networking, or durability.
//!
//! Administrative construction/attachment APIs are for a trusted local host.
//! Rules remain trusted native code. Plans are validated before state is replaced.

mod execution;
mod reference;
mod state;

pub use reference::ReferenceGame;
pub use state::WorldBuilder;

use crate::{
    command::{ActorContext, PlayerText},
    effect::DomainEvent,
    ids::{CharacterId, OperationId, RoomId, WorldId},
    rule::{RuleError, RuleRegistry},
    time::Tick,
};
use state::State;
use std::{error::Error, fmt};

pub const MAX_ROOMS: usize = 1024;
pub const MAX_CHARACTERS: usize = 256;
pub const MAX_EXITS_PER_ROOM: usize = 64;
pub const MAX_DELIVERIES: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldError {
    EmptyWorld,
    DuplicateRoom,
    UnknownRoom,
    DuplicateCharacter,
    UnknownCharacter,
    DuplicateExit,
    SelfExit,
    WorldCapacity,
    ExitCapacity,
    WrongWorld,
    OwnerMismatch,
    ConnectionInUse,
    NotAttached,
    StaleAttachment,
    Unreachable,
    IntentViolation,
    MissingMovement,
    AudienceDenied,
    UnsupportedEffect,
    DeliveryLimit,
    ClockRegression,
    CounterExhausted,
    Rule(RuleError),
}
impl fmt::Display for WorldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "world operation rejected: {self:?}")
    }
}
impl Error for WorldError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Rule(error) => Some(error),
            _ => None,
        }
    }
}
impl From<RuleError> for WorldError {
    fn from(error: RuleError) -> Self {
        Self::Rule(error)
    }
}

/// Resolved recipient at the effect's position in the staged plan.
/// A later transport must revalidate this attachment before delivering it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressedMessage {
    pub recipient: ActorContext,
    pub source: CharacterId,
    pub text: PlayerText,
}

/// Successful *in-memory* execution, not a durable receipt or delivery proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionResult {
    pub operation: OperationId,
    pub world: WorldId,
    pub revision: u64,
    pub events: Vec<DomainEvent>,
    pub messages: Vec<AddressedMessage>,
}

/// Detached host inspection value. Changing/dropping it cannot change the world.
/// This is an unfiltered administrative snapshot, not a player-facing view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldSnapshot(State);
impl WorldSnapshot {
    pub fn world_id(&self) -> WorldId {
        self.0.id
    }
    pub fn revision(&self) -> u64 {
        self.0.revision
    }
    pub fn room_of(&self, character: CharacterId) -> Option<RoomId> {
        self.0.characters.get(&character).map(|record| record.room)
    }
    pub fn attachment(&self, character: CharacterId) -> Option<ActorContext> {
        self.0.attachments.get(&character).copied()
    }
    pub fn last_execution(&self) -> Option<Tick> {
        self.0.last_execution
    }
}

/// No public state mutation, rule replacement, or raw plan-application API.
/// All changes after construction occur through attachment APIs or execute().
/// This type is not Clone: cloning a live authority is intentionally unsupported.
pub struct WorldRuntime {
    state: State,
    rules: RuleRegistry,
}
impl WorldRuntime {
    pub fn snapshot(&self) -> WorldSnapshot {
        WorldSnapshot(self.state.clone())
    }
}
