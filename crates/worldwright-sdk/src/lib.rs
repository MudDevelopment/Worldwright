//! Source-level game-development facade and compatibility inventory.

pub use worldwright_content as content;
pub use worldwright_core::world::RoomDescription;
pub use worldwright_core::{
    GameModule, ModuleRegistrationError, NotImplemented, WorldStub, WorldView,
};
pub use worldwright_core::{command, effect, ids, rule, runtime, scheduler, time};

pub const SDK_API_VERSION: u32 = 1;
pub const SUPPORTED_SDK_API_VERSIONS: &[u32] = &[SDK_API_VERSION];
pub const MINIMUM_RUST_VERSION: &str = "1.85.1";
pub const COMPATIBILITY_POLICY: &str =
    "current and immediately previous 0.x minor; one-minor deprecation before removal";
pub const STATUS: &str = "SDK API v1; source compatibility policy established from the M8 baseline";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompatibilityMatrix {
    pub sdk_current: u32,
    pub sdk_supported: &'static [u32],
    pub protocol_supported: &'static [u32],
    pub content_schema_supported: &'static [u64],
    pub minimum_rust: &'static str,
}

pub const COMPATIBILITY: CompatibilityMatrix = CompatibilityMatrix {
    sdk_current: SDK_API_VERSION,
    sdk_supported: SUPPORTED_SDK_API_VERSIONS,
    protocol_supported: worldwright_protocol_versions(),
    content_schema_supported: &[1],
    minimum_rust: MINIMUM_RUST_VERSION,
};

const fn worldwright_protocol_versions() -> &'static [u32] {
    // Kept literal so the SDK remains independent of the transport crate.
    &[1]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Deprecation {
    pub symbol: &'static str,
    pub replacement: &'static str,
    pub earliest_removal: &'static str,
}

pub const DEPRECATIONS: &[Deprecation] = &[Deprecation {
    symbol: "worldwright_testkit::ScenarioStub",
    replacement: "worldwright_testkit::RuntimeScenario",
    earliest_removal: "0.3",
}];

#[deprecated(since = "0.1.0", note = "use WorldView")]
pub use worldwright_core::WorldView as LegacyWorldView;

#[cfg(test)]
mod tests {
    use super::{COMPATIBILITY, DEPRECATIONS, GameModule};

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

    #[test]
    fn compatibility_inventory_is_bounded_and_explicit() {
        assert_eq!(COMPATIBILITY.sdk_current, 1);
        assert_eq!(COMPATIBILITY.sdk_supported, &[1]);
        assert_eq!(COMPATIBILITY.protocol_supported, &[1]);
        assert_eq!(COMPATIBILITY.content_schema_supported, &[1]);
        assert_eq!(DEPRECATIONS[0].earliest_removal, "0.3");
    }
}
