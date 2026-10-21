use std::{error::Error, fmt};

#[derive(Debug)]
pub enum PersistenceError {
    Configuration,
    Database(sqlx::Error),
    Migration(sqlx::migrate::MigrateError),
    SeedConflict,
    StaleWriter,
    Unauthorized,
    SessionCapacity,
    SessionCollision,
    ReceiptConflict,
    NotFound,
    InventoryFull,
    CounterExhausted,
    InvalidData,
    UnknownOutcome,
}

impl fmt::Display for PersistenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Configuration => "persistence configuration is invalid",
            Self::Database(_) => "persistence database operation failed",
            Self::Migration(_) => "persistence migration verification failed",
            Self::SeedConflict => "persistence seed conflicts with durable state",
            Self::StaleWriter => "persistence writer lease is stale",
            Self::Unauthorized => "persistence identity is not authorized",
            Self::SessionCapacity => "persistence session capacity is full",
            Self::SessionCollision => "persistence session token collided",
            Self::ReceiptConflict => "persistence request receipt conflicts",
            Self::NotFound => "persistence record was not found",
            Self::InventoryFull => "persistence inventory is full",
            Self::CounterExhausted => "persistence counter is exhausted",
            Self::InvalidData => "persistence data is invalid",
            Self::UnknownOutcome => "persistence commit outcome is unknown",
        };
        formatter.write_str(message)
    }
}

impl Error for PersistenceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::Migration(error) => Some(error),
            _ => None,
        }
    }
}

impl From<sqlx::Error> for PersistenceError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

impl From<sqlx::migrate::MigrateError> for PersistenceError {
    fn from(error: sqlx::migrate::MigrateError) -> Self {
        Self::Migration(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn display_is_sanitized_but_source_is_retained() {
        let error = PersistenceError::Database(sqlx::Error::Protocol("sentinel-secret".into()));
        assert!(!error.to_string().contains("sentinel-secret"));
        assert!(error.source().is_some());
    }
}
