//! Versioned, bounded JSON content loading and validated WorldBuilder conversion.
//! No live activation, release packaging, account authority, or persistence.
mod diagnostic;
mod instantiate;
mod loader;
mod schema;
mod validated;
mod validation;

pub use diagnostic::{ContentReport, Diagnostic, Severity, terminal_safe};
pub use instantiate::{WorldInstantiation, WorldLookup, instantiate};
pub use loader::{
    ContentLimits, MAX_DEPTH, MAX_DIAGNOSTICS, MAX_INPUT_BYTES, load_bytes, load_file,
};
pub use validated::{ValidatedRoom, ValidatedWorldContent};
