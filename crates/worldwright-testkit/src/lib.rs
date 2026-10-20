//! Deterministic fixtures and explicit placeholder for a future scenario runner.
//!
//! Includes a two-room fixture backed by the real local runtime. No database,
//! production replay, or general scenario runner is implemented.

pub mod fixtures;
pub mod two_room;
pub use fixtures::{FakeClock, SequenceRandom, WorldFixture};
pub use two_room::{TwoRoomWorld, two_room_world};

pub use worldwright_content::ContentPipelineStub;
pub use worldwright_core::NotImplemented;
pub use worldwright_sdk::GameModule;

/// An explicit marker for a scenario runner that has not been implemented.
#[derive(Debug, Default)]
pub struct ScenarioStub;

impl ScenarioStub {
    /// Cannot report a gameplay test as passed when no simulation ran.
    pub fn run(&self) -> Result<(), NotImplemented> {
        Err(NotImplemented::new(
            "worldwright-testkit",
            "scenario execution",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::ScenarioStub;

    #[test]
    fn scenario_execution_is_not_faked() {
        let scenario = ScenarioStub;
        assert!(scenario.run().is_err());
    }
}
