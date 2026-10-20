//! Provisional domain IDs; not a wire format or an ID-generation service.
//!
//! Distinct identifier categories cannot be interchanged accidentally:
//! ```compile_fail
//! use worldwright_core::ids::{RoomId, CharacterId};
//! let room = RoomId::from_u128(1).unwrap();
//! let character: CharacterId = room;
//! ```

use std::{error::Error, fmt, num::NonZeroU128, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdError {
    Zero,
    InvalidHex,
    InvalidDefinition,
}

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Zero => "an instance ID cannot be zero",
            Self::InvalidHex => "expected exactly 32 hexadecimal digits",
            Self::InvalidDefinition => "invalid namespaced definition ID",
        })
    }
}
impl Error for IdError {}

macro_rules! instance_id {
    ($($name:ident),+ $(,)?) => { $(
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(NonZeroU128);

        impl $name {
            /// Checks representation only, not uniqueness or authorization.
            pub fn from_u128(value: u128) -> Result<Self, IdError> {
                NonZeroU128::new(value).map(Self).ok_or(IdError::Zero)
            }
            pub const fn get(self) -> u128 { self.0.get() }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{:032x}", self.get())
            }
        }

        impl FromStr for $name {
            type Err = IdError;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                if value.len() != 32 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(IdError::InvalidHex);
                }
                let raw = u128::from_str_radix(value, 16).map_err(|_| IdError::InvalidHex)?;
                Self::from_u128(raw)
            }
        }
    )+ };
}

instance_id!(
    AccountId,
    CharacterId,
    WorldId,
    RoomId,
    ItemId,
    ConnectionId,
    OperationId
);

/// Content identity such as `frontier:rooms.arrival`; no persistence schema yet.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DefinitionId(String);

impl DefinitionId {
    pub const MAX_BYTES: usize = 192;

    pub fn parse(value: &str) -> Result<Self, IdError> {
        if value.len() > Self::MAX_BYTES {
            return Err(IdError::InvalidDefinition);
        }
        let (namespace, path) = value.split_once(':').ok_or(IdError::InvalidDefinition)?;
        if !valid_segment(namespace) || !path.split('.').all(valid_segment) {
            return Err(IdError::InvalidDefinition);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn valid_segment(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z'))
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

impl fmt::Display for DefinitionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
impl FromStr for DefinitionId {
    type Err = IdError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}
