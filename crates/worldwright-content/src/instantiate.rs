//! Definition IDs are content identities, not persistent runtime instance IDs.
use crate::{ContentReport, ValidatedWorldContent};
use std::collections::BTreeMap;
use worldwright_core::{
    ids::{DefinitionId, RoomId, WorldId},
    runtime::{WorldBuilder, WorldError},
};

pub struct WorldInstantiation {
    builder: WorldBuilder,
    room_ids: BTreeMap<DefinitionId, RoomId>,
    named_exits: BTreeMap<RoomId, BTreeMap<String, RoomId>>,
    entry_room: RoomId,
}
impl WorldInstantiation {
    pub fn room_ids(&self) -> &BTreeMap<DefinitionId, RoomId> {
        &self.room_ids
    }
    pub fn named_exits(&self) -> &BTreeMap<RoomId, BTreeMap<String, RoomId>> {
        &self.named_exits
    }
    pub fn entry_room(&self) -> RoomId {
        self.entry_room
    }
    /// Trusted host setup only; does not attach or authenticate characters.
    pub fn builder_mut(&mut self) -> &mut WorldBuilder {
        &mut self.builder
    }
    pub fn into_builder(self) -> WorldBuilder {
        self.builder
    }
    /// Preserve host lookup metadata while transferring builder ownership.
    pub fn into_parts(self) -> (WorldBuilder, WorldLookup) {
        (
            self.builder,
            WorldLookup {
                room_ids: self.room_ids,
                named_exits: self.named_exits,
                entry_room: self.entry_room,
            },
        )
    }
}
/// Host-only lookup information. Not an authorized player view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldLookup {
    pub room_ids: BTreeMap<DefinitionId, RoomId>,
    pub named_exits: BTreeMap<RoomId, BTreeMap<String, RoomId>>,
    pub entry_room: RoomId,
}

pub fn instantiate(
    content: &ValidatedWorldContent,
    world: WorldId,
) -> Result<WorldInstantiation, ContentReport> {
    let fail = |error: WorldError| {
        ContentReport::single("validated content", "WWC_BUILD", "", error.to_string())
    };
    let mut builder = WorldBuilder::new(world);
    let mut room_ids = BTreeMap::new();
    for (index, (definition, room)) in content.rooms().iter().enumerate() {
        let raw = (index as u128)
            .checked_add(1)
            .ok_or_else(|| fail(WorldError::CounterExhausted))?;
        let id = RoomId::from_u128(raw).map_err(|_| fail(WorldError::CounterExhausted))?;
        builder
            .add_room(id, room.title().clone(), room.description().clone())
            .map_err(fail)?;
        room_ids.insert(definition.clone(), id);
    }
    let lookup = |definition: &DefinitionId| {
        room_ids
            .get(definition)
            .copied()
            .ok_or_else(|| fail(WorldError::UnknownRoom))
    };
    let entry_room = lookup(content.entry_room())?;
    let mut named_exits = BTreeMap::new();
    for (definition, room) in content.rooms() {
        let from = lookup(definition)?;
        let mut exits = BTreeMap::new();
        for (label, destination) in room.exits() {
            let to = lookup(destination)?;
            builder.add_exit(from, to).map_err(fail)?;
            exits.insert(label.clone(), to);
        }
        named_exits.insert(from, exits);
    }
    Ok(WorldInstantiation {
        builder,
        room_ids,
        named_exits,
        entry_room,
    })
}
