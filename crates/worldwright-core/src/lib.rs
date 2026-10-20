//! Pure domain interfaces, bounded values, rule planning, and logical scheduling.
//!
//! No authenticated executor, database driver, or running world is provided.
//! All interfaces remain provisional. A valid ID or plan is not authorization.

pub mod command;
pub mod effect;
pub mod error;
pub mod game;
pub mod ids;
pub mod rule;
pub mod scheduler;
pub mod time;
pub mod world;

pub use error::NotImplemented;
pub use game::{GameModule, ModuleRegistrationError};
pub use world::{WorldStub, WorldView};
