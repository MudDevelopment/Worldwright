//! Bounded in-memory queue, ordered by (deadline, insertion sequence).
//! No persistence, task execution, or authorization is supplied.

use crate::time::Tick;
use std::{collections::BTreeMap, error::Error, fmt, num::NonZeroUsize};

/// A token is meaningful only in the queue that returned it, not globally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScheduleToken(u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scheduled<T> {
    pub token: ScheduleToken,
    pub at: Tick,
    pub payload: T,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleError {
    InvalidCapacity,
    Full,
    SequenceExhausted,
    ClockWentBackwards,
}

impl fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidCapacity => "queue capacity must be positive",
            Self::Full => "scheduled queue is full",
            Self::SequenceExhausted => "schedule sequence exhausted",
            Self::ClockWentBackwards => "poll time cannot move backwards",
        })
    }
}
impl Error for ScheduleError {}

#[derive(Debug)]
pub struct Scheduler<T> {
    entries: BTreeMap<(Tick, ScheduleToken), T>,
    capacity: NonZeroUsize,
    sequence: u64,
    last_poll: Option<Tick>,
}

impl<T> Scheduler<T> {
    pub fn new(capacity: usize) -> Result<Self, ScheduleError> {
        Ok(Self {
            entries: BTreeMap::new(),
            capacity: NonZeroUsize::new(capacity).ok_or(ScheduleError::InvalidCapacity)?,
            sequence: 0,
            last_poll: None,
        })
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn schedule(&mut self, at: Tick, payload: T) -> Result<ScheduleToken, ScheduleError> {
        if self.len() >= self.capacity.get() {
            return Err(ScheduleError::Full);
        }
        let next = self
            .sequence
            .checked_add(1)
            .ok_or(ScheduleError::SequenceExhausted)?;
        let token = ScheduleToken(next);
        self.entries.insert((at, token), payload);
        self.sequence = next;
        Ok(token)
    }

    /// Linear scan bounded by capacity; tokens are not reused by this queue.
    pub fn cancel(&mut self, token: ScheduleToken) -> Option<Scheduled<T>> {
        let key = self.entries.keys().find(|(_, id)| *id == token).copied()?;
        self.entries.remove(&key).map(|payload| Scheduled {
            token,
            at: key.0,
            payload,
        })
    }

    /// Removes at most one due action. Overdue inserts are allowed and will be
    /// returned on the next poll at or after the previous poll time.
    pub fn pop_due(&mut self, now: Tick) -> Result<Option<Scheduled<T>>, ScheduleError> {
        if self.last_poll.is_some_and(|previous| now < previous) {
            return Err(ScheduleError::ClockWentBackwards);
        }
        self.last_poll = Some(now);
        let key = self.entries.first_key_value().map(|(key, _)| *key);
        match key {
            Some((at, _)) if at <= now => Ok(self
                .entries
                .pop_first()
                .map(|((at, token), payload)| Scheduled { token, at, payload })),
            _ => Ok(None),
        }
    }
}
