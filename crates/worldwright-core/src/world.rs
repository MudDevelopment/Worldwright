//! Placeholder for the future authoritative world owner.

use crate::NotImplemented;

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
