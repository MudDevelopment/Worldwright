//! Provisional public facade for game authors.
//!
//! This scaffold is not a stable SDK. Core owns the traits; this facade exports
//! them without introducing a core-to-SDK dependency or a database dependency.

pub use worldwright_content::ContentPipelineStub;
pub use worldwright_core::world::RoomDescription;
pub use worldwright_core::{
    GameModule, ModuleRegistrationError, NotImplemented, WorldStub, WorldView,
};
pub use worldwright_core::{command, effect, ids, rule, runtime, scheduler, time};

/// Human-readable scaffold status, not an SDK compatibility promise.
pub const STATUS: &str =
    "unstable SDK; local world execution only, no network service or durability";

#[cfg(test)]
mod tests {
    use super::GameModule;

    struct ExampleGame;

    impl GameModule for ExampleGame {
        fn name(&self) -> &str {
            "Example game metadata"
        }
    }

    #[test]
    fn game_metadata_can_be_implemented_through_the_facade() {
        let game = ExampleGame;
        assert_eq!(game.name(), "Example game metadata");
    }
}
