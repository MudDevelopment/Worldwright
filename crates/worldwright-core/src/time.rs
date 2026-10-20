//! Logical time and injected randomness, independent of wall-clock and OS APIs.

use std::{error::Error, fmt};

/// Milliseconds on a caller-owned monotonic simulation timeline, not UTC.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tick(u64);

impl Tick {
    pub const ZERO: Self = Self(0);
    pub const fn from_millis(value: u64) -> Self {
        Self(value)
    }
    pub const fn as_millis(self) -> u64 {
        self.0
    }
    pub fn checked_add(self, millis: u64) -> Result<Self, TimeError> {
        self.0
            .checked_add(millis)
            .map(Self)
            .ok_or(TimeError::Overflow)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeError {
    Overflow,
    WentBackwards,
}

impl fmt::Display for TimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Overflow => "logical time overflow",
            Self::WentBackwards => "logical time cannot move backwards",
        })
    }
}
impl Error for TimeError {}

pub trait Clock {
    fn now(&self) -> Tick;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RandomExhausted;

impl fmt::Display for RandomExhausted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("random source exhausted")
    }
}
impl Error for RandomExhausted {}

/// A gameplay randomness seam, NOT a cryptographic randomness guarantee.
/// Range sampling and distribution correctness belong to a reviewed adapter.
pub trait RandomSource {
    fn next_u64(&mut self) -> Result<u64, RandomExhausted>;
}
