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

/// An explicit minimal policy: authentication material only. No gameplay,
/// entitlement, replay identity, audit, identity-link or outbox deletion.
pub const RETENTION_POLICY_VERSION: &str = "expired-authentication-only-v1";

#[derive(Debug, serde::Serialize)]
pub struct RetentionStatus {
    pub policy_version: &'static str,
    pub expired_sessions: u64,
    pub expired_unused_invites: u64,
    pub protected_receipts: u64,
    pub protected_outbox_rows: u64,
    pub external_dispatch_supported: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct RetentionResult {
    pub operation_id: String,
    pub policy_version: &'static str,
    pub sessions_deleted: u64,
    pub invites_deleted: u64,
    pub replayed: bool,
}

impl PostgresStore {
    pub async fn retention_status(&self, context: &crate::OperatorContext<'_>) -> Result<RetentionStatus, PersistenceError> {
        use crate::{operational::authorize_tx, types::uuid_of};
        let mut tx = self.pool().begin().await?;
        authorize_tx(&mut tx, context, worldwright_operations::Capability::ViewStatus, false).await?;
        let row: (i64, i64, i64, i64) = sqlx::query_as(
            "SELECT (SELECT COUNT(*) FROM worldwright_sessions WHERE world_id=$1 AND expires_at<=clock_timestamp()), \
            (SELECT COUNT(*) FROM worldwright_player_invites i WHERE i.world_id=$1 AND i.expires_at<=clock_timestamp() \
             AND i.consumed_at IS NULL AND NOT EXISTS (SELECT 1 FROM worldwright_oidc_audit a WHERE a.invite_id=i.invite_id)), \
            (SELECT COUNT(*) FROM worldwright_operation_receipts WHERE world_id=$1), \
            (SELECT COUNT(*) FROM worldwright_outbox WHERE world_id=$1)")
            .bind(uuid_of(context.world)).fetch_one(&mut *tx).await?;
        Ok(RetentionStatus { policy_version: RETENTION_POLICY_VERSION,
            expired_sessions: crate::types::u64_from_i64(row.0, false)?,
            expired_unused_invites: crate::types::u64_from_i64(row.1, false)?,
            protected_receipts: crate::types::u64_from_i64(row.2, false)?,
            protected_outbox_rows: crate::types::u64_from_i64(row.3, false)?,
            external_dispatch_supported: EXTERNAL_OUTBOX_DISPATCH_SUPPORTED })
    }

    /// Bounded, scoped and audited. Caller retains the operation ID on uncertain
    /// COMMIT and retries the SAME request; changed-intent reuse is rejected.
    pub async fn prune_expired_authentication(
        &self, context: &crate::OperatorContext<'_>, batch: usize,
        operation: worldwright_core::ids::OperationId,
    ) -> Result<RetentionResult, PersistenceError> {
        use crate::{operational::authorize_tx, types::uuid_of};
        if !(1..=MAX_RETENTION_BATCH).contains(&batch) || operation.get() == 0 {
            return Err(PersistenceError::InvalidData);
        }
        let batch = i32::try_from(batch).map_err(|_| PersistenceError::InvalidData)?;
        let environment = match context.environment {
            worldwright_operations::Environment::Staging => "staging",
            worldwright_operations::Environment::Production => "production",
        };
        let mut tx = self.pool().begin().await?;
        let authorization = authorize_tx(&mut tx, context, worldwright_operations::Capability::ManageOperators, true).await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(format!("r6-retention:{operation}")).execute(&mut *tx).await?;
        type Prior = (uuid::Uuid, String, String, String, i32, i32, i32);
        let prior: Option<Prior> = sqlx::query_as("SELECT world_id, target_environment, actor_subject, reason, \
            batch_limit, sessions_deleted, invites_deleted FROM worldwright_retention_runs WHERE operation_id=$1")
            .bind(uuid_of(operation)).fetch_optional(&mut *tx).await?;
        if let Some((world, env, actor, reason, limit, sessions, invites)) = prior {
            if world != uuid_of(context.world) || env != environment || actor != context.claims.subject
                || reason != context.reason || limit != batch {
                return Err(PersistenceError::RecoveryConflict);
            }
            return Ok(RetentionResult { operation_id: operation.to_string(), policy_version: RETENTION_POLICY_VERSION,
                sessions_deleted: u64::try_from(sessions).map_err(|_| PersistenceError::InvalidData)?,
                invites_deleted: u64::try_from(invites).map_err(|_| PersistenceError::InvalidData)?, replayed: true });
        }
        let sessions = sqlx::query("WITH candidates AS (SELECT token_hash FROM worldwright_sessions \
            WHERE world_id=$1 AND expires_at<=clock_timestamp() ORDER BY expires_at,token_hash \
            LIMIT $2 FOR UPDATE SKIP LOCKED) DELETE FROM worldwright_sessions s USING candidates c \
            WHERE s.token_hash=c.token_hash")
            .bind(uuid_of(context.world)).bind(i64::from(batch)).execute(&mut *tx).await?.rows_affected();
        let left = i64::from(batch) - i64::try_from(sessions).map_err(|_| PersistenceError::InvalidData)?;
        if left < 0 { return Err(PersistenceError::InvalidData); }
        let invites = sqlx::query("WITH candidates AS (SELECT i.invite_id FROM worldwright_player_invites i \
            WHERE i.world_id=$1 AND i.expires_at<=clock_timestamp() AND i.consumed_at IS NULL \
            AND NOT EXISTS (SELECT 1 FROM worldwright_oidc_audit a WHERE a.invite_id=i.invite_id) \
            ORDER BY i.expires_at,i.invite_id LIMIT $2 FOR UPDATE OF i SKIP LOCKED) \
            DELETE FROM worldwright_player_invites i USING candidates c WHERE i.invite_id=c.invite_id")
            .bind(uuid_of(context.world)).bind(left).execute(&mut *tx).await?.rows_affected();
        sqlx::query("INSERT INTO worldwright_retention_runs (operation_id,world_id,target_environment,actor_subject, \
            actor_session_id,correlation_id,policy_version,reason,batch_limit,sessions_deleted,invites_deleted,authorization_snapshot) \
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)")
            .bind(uuid_of(operation)).bind(uuid_of(context.world)).bind(environment).bind(&context.claims.subject)
            .bind(uuid_of(context.claims.session_id().map_err(|_| PersistenceError::InvalidData)?))
            .bind(uuid_of(context.correlation)).bind(RETENTION_POLICY_VERSION).bind(context.reason).bind(batch)
            .bind(i32::try_from(sessions).map_err(|_| PersistenceError::InvalidData)?)
            .bind(i32::try_from(invites).map_err(|_| PersistenceError::InvalidData)?)
            .bind(authorization).execute(&mut *tx).await?;
        tx.commit().await.map_err(|_| PersistenceError::UnknownOutcome)?;
        Ok(RetentionResult { operation_id: operation.to_string(), policy_version: RETENTION_POLICY_VERSION,
            sessions_deleted: sessions, invites_deleted: invites, replayed: false })
    }
}
