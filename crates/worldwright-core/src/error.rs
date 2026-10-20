//! Explicit failure for operations that have not been implemented.

use std::error::Error;
use std::fmt;

/// A stub did not perform the requested operation. This is never a success receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotImplemented {
    pub component: &'static str,
    pub operation: &'static str,
}

impl NotImplemented {
    pub const fn new(component: &'static str, operation: &'static str) -> Self {
        Self {
            component,
            operation,
        }
    }
}

impl fmt::Display for NotImplemented {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {} is not implemented (scaffold only)",
            self.component, self.operation
        )
    }
}

impl Error for NotImplemented {}

#[cfg(test)]
mod tests {
    use super::NotImplemented;

    #[test]
    fn error_names_the_missing_operation() {
        let error = NotImplemented::new("core", "world startup");
        assert_eq!(
            error.to_string(),
            "core: world startup is not implemented (scaffold only)"
        );
    }
}
