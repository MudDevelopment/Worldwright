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
    ContentLimits, MAX_DEPTH, MAX_DIAGNOSTICS, MAX_INPUT_BYTES, load_bytes, load_draft_bytes,
    load_file, serialize_draft, validate_draft,
};
/// The loader and validator dispatch only this implemented schema.
pub const CONTENT_SCHEMA_VERSION: u64 = 1;
pub const SUPPORTED_CONTENT_SCHEMA_VERSIONS: &[u64] = &[CONTENT_SCHEMA_VERSION];

pub use schema::{DraftRoomV1, DraftWorldV1};
pub use validated::{ValidatedRoom, ValidatedWorldContent};
