//! Read-only query seam and explicit placeholder for the future world owner.

use crate::{
    NotImplemented,
    effect::ItemLocation,
    ids::{CharacterId, ItemId, RoomId, WorldId},
};

/// A rule receives only queries, never mutable world state or database access.
/// Implementations must scope/redact results to the authorized caller. This
/// trait alone does not enforce visibility, identity, or snapshot consistency.
pub trait WorldView {
    fn world_id(&self) -> WorldId;
    fn contains_room(&self, room: RoomId) -> bool;
    fn character_room(&self, character: CharacterId) -> Option<RoomId>;
    fn item_location(&self, item: ItemId) -> Option<ItemLocation>;
}

/// No world state, scheduling, persistence, or player admission exists yet.
#[derive(Debug, Default)]
pub struct WorldStub;

impl WorldStub {
    /// Refuses startup instead of presenting an empty world as a working server.
    pub fn start(&mut self) -> Result<(), NotImplemented> {
        Err(NotImplemented::new("worldwright-core", "world startup"))
    }
}

#[cfg(test)]
mod tests {
    use super::WorldStub;

    #[test]
    fn world_startup_is_explicitly_unimplemented() {
        let mut world = WorldStub;
        assert!(world.start().is_err());
    }
}
