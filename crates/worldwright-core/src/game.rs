//! Provisional metadata boundary for a trusted, compiled game module.

/// Identifies a game module; it does not register commands or execute rules yet.
///
/// The returned value is a display name, not an authenticated identity or a
/// validated persistent identifier. Rule contexts/effect plans are future work.
pub trait GameModule {
    fn name(&self) -> &str;
}
