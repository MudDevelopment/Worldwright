//! Deterministic, in-memory fixtures. Never a production world or secure RNG.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use worldwright_core::{
    WorldView,
    effect::ItemLocation,
    ids::{CharacterId, ItemId, RoomId, WorldId},
    time::{Clock, RandomExhausted, RandomSource, Tick, TimeError},
};

#[derive(Debug, Default)]
pub struct FakeClock {
    now: Tick,
}
impl FakeClock {
    pub fn new(now: Tick) -> Self {
        Self { now }
    }
    pub fn advance(&mut self, millis: u64) -> Result<(), TimeError> {
        self.now = self.now.checked_add(millis)?;
        Ok(())
    }
    pub fn set(&mut self, now: Tick) -> Result<(), TimeError> {
        if now < self.now {
            return Err(TimeError::WentBackwards);
        }
        self.now = now;
        Ok(())
    }
}
impl Clock for FakeClock {
    fn now(&self) -> Tick {
        self.now
    }
}

#[derive(Debug, Default)]
pub struct SequenceRandom {
    values: VecDeque<u64>,
}
impl SequenceRandom {
    pub fn new(values: impl IntoIterator<Item = u64>) -> Self {
        Self {
            values: values.into_iter().collect(),
        }
    }
}
impl RandomSource for SequenceRandom {
    fn next_u64(&mut self) -> Result<u64, RandomExhausted> {
        self.values.pop_front().ok_or(RandomExhausted)
    }
}

/// Unfiltered test data, deliberately unsuitable for a player-facing query view.
#[derive(Debug)]
pub struct WorldFixture {
    world: WorldId,
    rooms: BTreeSet<RoomId>,
    characters: BTreeMap<CharacterId, RoomId>,
    items: BTreeMap<ItemId, ItemLocation>,
}
impl WorldFixture {
    pub fn new(world: WorldId) -> Self {
        Self {
            world,
            rooms: BTreeSet::new(),
            characters: BTreeMap::new(),
            items: BTreeMap::new(),
        }
    }
    pub fn add_room(&mut self, room: RoomId) {
        self.rooms.insert(room);
    }
    /// Returns false without mutation if the room was not added to the fixture.
    pub fn place_character(&mut self, character: CharacterId, room: RoomId) -> bool {
        if !self.rooms.contains(&room) {
            return false;
        }
        self.characters.insert(character, room);
        true
    }
    /// Allows intentionally invalid fixtures for future validation tests.
    pub fn set_item_location_unchecked(&mut self, item: ItemId, location: ItemLocation) {
        self.items.insert(item, location);
    }
}
impl WorldView for WorldFixture {
    fn world_id(&self) -> WorldId {
        self.world
    }
    fn contains_room(&self, room: RoomId) -> bool {
        self.rooms.contains(&room)
    }
    fn character_room(&self, character: CharacterId) -> Option<RoomId> {
        self.characters.get(&character).copied()
    }
    fn item_location(&self, item: ItemId) -> Option<ItemLocation> {
        self.items.get(&item).copied()
    }
}
