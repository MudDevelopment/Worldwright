//! Independent home for future transport DTOs and protocol validation.
//!
//! No wire protocol, version negotiation, frame parser, serializer, network
//! listener, or authentication exists. Do not expose domain state from here.

/// Human-readable status, not a wire message or protocol-version identifier.
pub const STATUS: &str = "scaffold only; no wire protocol implemented";

/// No protocol version may be advertised as supported by this placeholder.
pub const SUPPORTED_PROTOCOL_VERSIONS: &[u32] = &[];

#[cfg(test)]
mod tests {
    #[test]
    fn protocol_support_is_not_advertised() {
        assert_eq!(super::SUPPORTED_PROTOCOL_VERSIONS, &[] as &[u32]);
    }
}
