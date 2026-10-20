//! Bounded rule output. Plans are intentions, never committed results.

use crate::{
    command::PlayerText,
    ids::{CharacterId, ItemId, RoomId},
    time::Tick,
};
use std::{error::Error, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemLocation {
    Room(RoomId),
    Inventory(CharacterId),
    Container(ItemId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audience {
    Character(CharacterId),
    Room(RoomId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduledAction {
    WakeCharacter(CharacterId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    MoveCharacter {
        character: CharacterId,
        destination: RoomId,
    },
    TransferItem {
        item: ItemId,
        expected_from: ItemLocation,
        to: ItemLocation,
    },
    EmitMessage {
        audience: Audience,
        text: PlayerText,
    },
    Schedule {
        at: Tick,
        action: ScheduledAction,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanFull;
impl fmt::Display for PlanFull {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("effect plan reached its size limit")
    }
}
impl Error for PlanFull {}

/// The executor must validate scope, current state, ownership, capacity,
/// authorization, visibility, and durability requirements before application.
/// The local runtime validates its supported subset; transfers and scheduling
/// remain unsupported. This type intentionally has no raw `apply` method.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EffectPlan {
    effects: Vec<Effect>,
}

impl EffectPlan {
    pub const MAX_EFFECTS: usize = 128;
    pub fn new() -> Self {
        Self::default()
    }
    pub fn push(&mut self, effect: Effect) -> Result<(), PlanFull> {
        if self.effects.len() >= Self::MAX_EFFECTS {
            return Err(PlanFull);
        }
        self.effects.push(effect);
        Ok(())
    }
    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }
    pub fn into_effects(self) -> Vec<Effect> {
        self.effects
    }
}

/// Completed-event DTOs for a future executor. Creating a value is not evidence
/// of persistence or delivery; no event stream or receipt store exists yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainEvent {
    CharacterMoved {
        character: CharacterId,
        from: RoomId,
        to: RoomId,
    },
    ItemTransferred {
        item: ItemId,
        from: ItemLocation,
        to: ItemLocation,
    },
}
