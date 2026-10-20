//! Pure domain boundary for Worldwright. Only scaffold contracts exist here.
//!
//! No networking, database driver, authorization, or running world is provided.
//! Public types are provisional and must not be treated as a stable SDK.

pub mod error;
pub mod game;
pub mod world;

pub use error::NotImplemented;
pub use game::GameModule;
pub use world::WorldStub;
