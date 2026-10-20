//! Content loading, schema validation, and packaging belong here.
//!
//! No JSON schema, parser, file access, validator, or release format is supplied.
//! A successful parse must never later be confused with complete validation.

use worldwright_core::NotImplemented;

/// Placeholder, deliberately incapable of approving any content.
#[derive(Debug, Default)]
pub struct ContentPipelineStub;

impl ContentPipelineStub {
    /// Accepts text only to reserve the boundary; does not inspect or approve it.
    pub fn validate(&self, _source: &str) -> Result<(), NotImplemented> {
        Err(NotImplemented::new(
            "worldwright-content",
            "content validation",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::ContentPipelineStub;

    #[test]
    fn content_is_never_reported_as_valid_by_the_stub() {
        let pipeline = ContentPipelineStub;
        assert!(pipeline.validate("{}").is_err());
        assert!(pipeline.validate("not json").is_err());
    }
}
