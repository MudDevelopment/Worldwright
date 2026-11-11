//! Conservative maintenance: erase expired session hashes, never replay identities.
//! Audit/receipt/identity deletion is intentionally not a retention implementation.
use crate::{PersistenceError, PostgresStore};

pub const EXTERNAL_OUTBOX_DISPATCH_SUPPORTED: bool = false;
pub const MAX_RETENTION_BATCH: usize = 1000;

impl PostgresStore {
    /// Trusted maintenance API. Expired sessions already fail authentication.
    /// Keep receipts, reward uniqueness, OIDC links and authority/audit rows intact.
    pub async fn prune_expired_player_sessions(
        &self,
        batch: usize,
    ) -> Result<u64, PersistenceError> {
        if !(1..=MAX_RETENTION_BATCH).contains(&batch) {
            return Err(PersistenceError::InvalidData);
        }
        let batch = i64::try_from(batch).map_err(|_| PersistenceError::InvalidData)?;
        let result = sqlx::query(
            "WITH expired AS (SELECT token_hash FROM worldwright_sessions \
             WHERE expires_at <= clock_timestamp() ORDER BY expires_at, token_hash \
             LIMIT $1 FOR UPDATE SKIP LOCKED) \
             DELETE FROM worldwright_sessions AS sessions USING expired \
             WHERE sessions.token_hash = expired.token_hash",
        )
        .bind(batch)
        .execute(self.pool())
        .await?;
        Ok(result.rows_affected())
    }
}
