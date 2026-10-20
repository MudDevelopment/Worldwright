//! Future SQLx/PostgreSQL adapter boundary. No storage is implemented.
//!
//! Do not replace this with an in-memory backend that implies durability.
//! Transactions, migrations, receipts, writer epochs, and recovery remain TODOs.

use worldwright_core::NotImplemented;

/// Placeholder that cannot open a database or acknowledge a durable write.
#[derive(Debug, Default)]
pub struct PersistenceStub;

impl PersistenceStub {
    /// Does not read credentials, open sockets, or create a database.
    pub fn connect(&self) -> Result<(), NotImplemented> {
        Err(NotImplemented::new(
            "worldwright-persistence",
            "database connection",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::PersistenceStub;

    #[test]
    fn database_connection_is_not_faked() {
        let persistence = PersistenceStub;
        assert!(persistence.connect().is_err());
    }
}
