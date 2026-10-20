use super::{MAX_CHARACTERS, MAX_EXITS_PER_ROOM, MAX_ROOMS, WorldError, WorldRuntime};
use crate::{
    command::{ActorContext, ConnectionGeneration, PlayerText},
    effect::ItemLocation,
    ids::{AccountId, CharacterId, ConnectionId, ItemId, RoomId, WorldId},
    rule::RuleRegistry,
    time::Tick,
    world::{RoomDescription, WorldView},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RoomRecord {
    pub(super) description: RoomDescription,
    pub(super) exits: BTreeSet<RoomId>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CharacterRecord {
    pub(super) owner: AccountId,
    pub(super) room: RoomId,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct State {
    pub(super) id: WorldId,
    pub(super) rooms: BTreeMap<RoomId, RoomRecord>,
    pub(super) characters: BTreeMap<CharacterId, CharacterRecord>,
    pub(super) attachments: BTreeMap<CharacterId, ActorContext>,
    pub(super) generation: u64,
    pub(super) revision: u64,
    pub(super) last_execution: Option<Tick>,
}

/// Trusted, programmatic setup. No content format or untrusted loader is implied.
pub struct WorldBuilder {
    state: State,
}
impl WorldBuilder {
    pub fn new(id: WorldId) -> Self {
        Self {
            state: State {
                id,
                rooms: BTreeMap::new(),
                characters: BTreeMap::new(),
                attachments: BTreeMap::new(),
                generation: 0,
                revision: 0,
                last_execution: None,
            },
        }
    }
    pub fn add_room(
        &mut self,
        id: RoomId,
        title: PlayerText,
        description: PlayerText,
    ) -> Result<(), WorldError> {
        if self.state.rooms.contains_key(&id) {
            return Err(WorldError::DuplicateRoom);
        }
        if self.state.rooms.len() >= MAX_ROOMS {
            return Err(WorldError::WorldCapacity);
        }
        self.state.rooms.insert(
            id,
            RoomRecord {
                description: RoomDescription { title, description },
                exits: BTreeSet::new(),
            },
        );
        Ok(())
    }
    /// Directed edge: add both directions explicitly for a two-way connection.
    pub fn add_exit(&mut self, from: RoomId, to: RoomId) -> Result<(), WorldError> {
        if !self.state.rooms.contains_key(&from) || !self.state.rooms.contains_key(&to) {
            return Err(WorldError::UnknownRoom);
        }
        if from == to {
            return Err(WorldError::SelfExit);
        }
        let room = self
            .state
            .rooms
            .get_mut(&from)
            .ok_or(WorldError::UnknownRoom)?;
        if room.exits.contains(&to) {
            return Err(WorldError::DuplicateExit);
        }
        if room.exits.len() >= MAX_EXITS_PER_ROOM {
            return Err(WorldError::ExitCapacity);
        }
        room.exits.insert(to);
        Ok(())
    }
    pub fn add_character(
        &mut self,
        id: CharacterId,
        owner: AccountId,
        room: RoomId,
    ) -> Result<(), WorldError> {
        if self.state.characters.contains_key(&id) {
            return Err(WorldError::DuplicateCharacter);
        }
        if !self.state.rooms.contains_key(&room) {
            return Err(WorldError::UnknownRoom);
        }
        if self.state.characters.len() >= MAX_CHARACTERS {
            return Err(WorldError::WorldCapacity);
        }
        self.state
            .characters
            .insert(id, CharacterRecord { owner, room });
        Ok(())
    }
    pub fn build(self, rules: RuleRegistry) -> Result<WorldRuntime, WorldError> {
        if self.state.rooms.is_empty() {
            return Err(WorldError::EmptyWorld);
        }
        Ok(WorldRuntime {
            state: self.state,
            rules,
        })
    }
}

impl State {
    pub(super) fn validate_actor(&self, actor: &ActorContext) -> Result<(), WorldError> {
        let character = self
            .characters
            .get(&actor.character)
            .ok_or(WorldError::UnknownCharacter)?;
        if character.owner != actor.account {
            return Err(WorldError::OwnerMismatch);
        }
        let current = self
            .attachments
            .get(&actor.character)
            .ok_or(WorldError::NotAttached)?;
        if current != actor {
            return Err(WorldError::StaleAttachment);
        }
        Ok(())
    }
    pub(super) fn next_revision(&self) -> Result<u64, WorldError> {
        self.revision
            .checked_add(1)
            .ok_or(WorldError::CounterExhausted)
    }
    pub(super) fn current_room(&self, character: CharacterId) -> Result<RoomId, WorldError> {
        self.characters
            .get(&character)
            .map(|record| record.room)
            .ok_or(WorldError::UnknownCharacter)
    }
    pub(super) fn can_traverse(&self, from: RoomId, to: RoomId) -> bool {
        self.rooms
            .get(&from)
            .is_some_and(|room| room.exits.contains(&to))
    }
}

impl WorldRuntime {
    /// Trusted local host only. Ownership is checked, but account login is NOT.
    /// Every successful attachment gets a new world-wide generation. A stale
    /// disconnect cannot remove a replacement attachment.
    pub fn attach(
        &mut self,
        account: AccountId,
        character: CharacterId,
        connection: ConnectionId,
    ) -> Result<ActorContext, WorldError> {
        let record = self
            .state
            .characters
            .get(&character)
            .ok_or(WorldError::UnknownCharacter)?;
        if record.owner != account {
            return Err(WorldError::OwnerMismatch);
        }
        if self
            .state
            .attachments
            .values()
            .any(|actor| actor.connection == connection && actor.character != character)
        {
            return Err(WorldError::ConnectionInUse);
        }
        let generation = self
            .state
            .generation
            .checked_add(1)
            .ok_or(WorldError::CounterExhausted)?;
        let revision = self.state.next_revision()?;
        let actor = ActorContext {
            account,
            character,
            connection,
            generation: ConnectionGeneration::new(generation)
                .map_err(|_| WorldError::CounterExhausted)?,
        };
        self.state.attachments.insert(character, actor);
        self.state.generation = generation;
        self.state.revision = revision;
        Ok(actor)
    }
    pub fn detach(&mut self, actor: ActorContext) -> Result<(), WorldError> {
        self.state.validate_actor(&actor)?;
        let revision = self.state.next_revision()?;
        self.state.attachments.remove(&actor.character);
        self.state.revision = revision;
        Ok(())
    }
}

/// Only the actor's location/description and reachable room IDs are visible.
/// Item queries are unavailable because item execution is deferred.
pub(super) struct ScopedView<'a> {
    pub(super) state: &'a State,
    pub(super) actor: CharacterId,
}
impl WorldView for ScopedView<'_> {
    fn world_id(&self) -> WorldId {
        self.state.id
    }
    fn contains_room(&self, room: RoomId) -> bool {
        self.state
            .current_room(self.actor)
            .is_ok_and(|current| current == room || self.state.can_traverse(current, room))
    }
    fn character_room(&self, character: CharacterId) -> Option<RoomId> {
        if character != self.actor {
            return None;
        }
        self.state
            .characters
            .get(&character)
            .map(|record| record.room)
    }
    fn item_location(&self, _item: ItemId) -> Option<ItemLocation> {
        None
    }
    fn room_description(&self, room: RoomId) -> Option<RoomDescription> {
        if self.character_room(self.actor) != Some(room) {
            return None;
        }
        self.state
            .rooms
            .get(&room)
            .map(|record| record.description.clone())
    }
    fn can_traverse(&self, from: RoomId, to: RoomId) -> bool {
        self.character_room(self.actor) == Some(from) && self.state.can_traverse(from, to)
    }
}
