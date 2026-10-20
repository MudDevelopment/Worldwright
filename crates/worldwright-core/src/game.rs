//! Registration boundary for a trusted, compiled game module.

use crate::rule::{DuplicateRule, RuleRegistry};
use std::{error::Error, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleRegistrationError {
    NotImplemented,
    Duplicate(DuplicateRule),
}
impl fmt::Display for ModuleRegistrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotImplemented => {
                f.write_str("game module has not implemented rule registration")
            }
            Self::Duplicate(error) => fmt::Display::fmt(error, f),
        }
    }
}
impl Error for ModuleRegistrationError {}
impl From<DuplicateRule> for ModuleRegistrationError {
    fn from(error: DuplicateRule) -> Self {
        Self::Duplicate(error)
    }
}

pub trait GameModule {
    /// A display name, not an authenticated or persistent identifier.
    fn name(&self) -> &str;

    /// Implementors may register pure rules. The default refuses registration,
    /// preserving compatibility with the earlier metadata-only implementation.
    /// Build a fresh registry and discard it if registration returns an error;
    /// multi-rule registration is not transactional and may partially populate it.
    fn register_rules(&self, _registry: &mut RuleRegistry) -> Result<(), ModuleRegistrationError> {
        Err(ModuleRegistrationError::NotImplemented)
    }
}
