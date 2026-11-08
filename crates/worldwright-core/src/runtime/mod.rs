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
    ids::{CharacterId, DefinitionId, OperationId, RoomId, WorldId},
    rule::{RuleError, RuleRegistry},
    time::Tick,
};
use state::State;
use std::{error::Error, fmt};

pub const MAX_ROOMS: usize = 1024;
pub const MAX_CHARACTERS: usize = 256;
pub const MAX_EXITS_PER_ROOM: usize = 64;
pub const MAX_DELIVERIES: usize = 1024;
pub const MAX_COUNTER_VALUE: u64 = 1_000_000;
pub const MAX_SCHEDULED_ACTIONS: usize = 128;
pub const MAX_DUE_ACTIONS: usize = 128;

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
    DuplicateCounter,
    CounterLimit,
    ScheduleLimit,
    ScheduleClock,
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
    pub fn characters(&self) -> impl Iterator<Item = CharacterId> + '_ {
        self.0.characters.keys().copied()
    }
    pub fn attachment(&self, character: CharacterId) -> Option<ActorContext> {
        self.0.attachments.get(&character).copied()
    }
    pub fn last_execution(&self) -> Option<Tick> {
        self.0.last_execution
    }
    pub fn counter(&self, character: CharacterId, key: DefinitionId) -> Option<u64> {
        self.0.counters.get(&(character, key)).copied()
    }
    pub fn scheduled_actions(&self) -> usize {
        self.0.scheduler.len()
    }
    pub fn durable_state(&self) -> Result<DurableWorldState, WorldError> {
        let counters = self
            .0
            .counters
            .iter()
            .map(|((character, key), value)| CounterState {
                character: *character,
                key: key.clone(),
                value: *value,
            })
            .collect();
        let mut scheduled = Vec::new();
        for (at, action) in self.0.scheduler.iter() {
            match action {
                crate::effect::ScheduledAction::AdjustCounter {
                    character,
                    key,
                    delta,
                } => scheduled.push(ScheduledCounterState {
                    at,
                    character: *character,
                    key: key.clone(),
                    delta: *delta,
                }),
                _ => return Err(WorldError::UnsupportedEffect),
            }
        }
        Ok(DurableWorldState {
            world: self.world_id(),
            revision: self.revision(),
            counters,
            scheduled,
        })
    }
}

/// No public state mutation, rule replacement, or raw plan-application API.
/// All changes after construction occur through attachment APIs or execute().
/// This type is not Clone: cloning a live authority is intentionally unsupported.
pub struct WorldRuntime {
    state: State,
    rules: RuleRegistry,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledResult {
    pub world: WorldId,
    pub revision: u64,
    pub events: Vec<DomainEvent>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CounterState {
    pub character: CharacterId,
    pub key: DefinitionId,
    pub value: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduledCounterState {
    pub at: Tick,
    pub character: CharacterId,
    pub key: DefinitionId,
    pub delta: i64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableWorldState {
    pub world: WorldId,
    pub revision: u64,
    pub counters: Vec<CounterState>,
    pub scheduled: Vec<ScheduledCounterState>,
}
impl WorldRuntime {
    pub fn snapshot(&self) -> WorldSnapshot {
        WorldSnapshot(self.state.clone())
    }
    /// Restores trusted host checkpoint state after an unobserved command.
    pub fn restore_checkpoint(&mut self, checkpoint: WorldSnapshot) -> Result<(), WorldError> {
        if checkpoint.world_id() != self.state.id {
            return Err(WorldError::WrongWorld);
        }
        self.state = checkpoint.0;
        Ok(())
    }
    pub fn advance(&mut self, now: Tick) -> Result<ScheduledResult, WorldError> {
        if self.state.last_observed.is_some_and(|last| now < last) {
            return Err(WorldError::ClockRegression);
        }
        let mut staged = self.state.clone();
        let mut events = Vec::new();
        for _ in 0..MAX_DUE_ACTIONS {
            let Some(item) = staged
                .scheduler
                .pop_due(now)
                .map_err(|_| WorldError::ScheduleClock)?
            else {
                break;
            };
            match item.payload {
                crate::effect::ScheduledAction::AdjustCounter {
                    character,
                    key,
                    delta,
                } => {
                    let before = staged
                        .counters
                        .get(&(character, key.clone()))
                        .copied()
                        .unwrap_or(0);
                    let after = if delta < 0 {
                        before.checked_sub(delta.unsigned_abs())
                    } else {
                        before.checked_add(delta as u64)
                    }
                    .ok_or(WorldError::CounterLimit)?;
                    if after > MAX_COUNTER_VALUE {
                        return Err(WorldError::CounterLimit);
                    }
                    staged.counters.insert((character, key.clone()), after);
                    events.push(DomainEvent::CounterAdjusted {
                        character,
                        key,
                        before,
                        after,
                    });
                }
                _ => return Err(WorldError::UnsupportedEffect),
            }
        }
        if !events.is_empty() {
            staged.revision = staged.next_revision()?;
            staged.last_execution = Some(now);
        }
        staged.last_observed = Some(now);
        let result = ScheduledResult {
            world: staged.id,
            revision: staged.revision,
            events,
        };
        self.state = staged;
        Ok(result)
    }
}
