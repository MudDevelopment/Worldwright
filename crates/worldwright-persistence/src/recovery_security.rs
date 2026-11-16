//! Explicit restore-time fencing and minimal recovery-administrator reconciliation.
//! Uses CURRENT control-DB authority, never restored grants as authorization.
use crate::{operational::{authorize_tx, insert_audit}, types::uuid_of, OperatorContext,
    PersistenceError, PostgresStore};
use worldwright_core::ids::{OperationId, RecoveryJobId};
use worldwright_operations::{Capability, OperatorKind};

impl PostgresStore {
    /// Verify the signed manifest describes the actual restored database snapshot.
    /// A concurrent source activation/writer change must not silently mislabel it.
    pub async fn verify_restored_manifest(&self, manifest: &worldwright_operations::BackupManifest) -> Result<(), PersistenceError> {
        let world: worldwright_core::ids::WorldId = manifest.world_id.parse().map_err(|_| PersistenceError::InvalidData)?;
        let row: Option<(Option<Vec<u8>>, uuid::Uuid, i64)> = sqlx::query_as(
            "SELECT active_release_digest,authority_incarnation,writer_epoch FROM worldwright_worlds WHERE world_id=$1")
            .bind(uuid_of(world)).fetch_optional(self.pool()).await?;
        let (release, incarnation, epoch) = row.ok_or(PersistenceError::NotFound)?;
        if release.map(|value| worldwright_operations::encode_hex(&value)) != manifest.active_release
            || format!("{:032x}",incarnation.as_u128()) != manifest.authority_incarnation
            || u64::try_from(epoch).map_err(|_| PersistenceError::InvalidData)? != manifest.writer_epoch {
            return Err(PersistenceError::RecoveryConflict);
        }
        Ok(())
    }

    pub async fn reconcile_restored_security(
        &self, control: &PostgresStore, context: &OperatorContext<'_>,
        job: RecoveryJobId, operation: OperationId,
    ) -> Result<(), PersistenceError> {
        if matches!(context.claims.kind, OperatorKind::Service) { return Err(PersistenceError::OperatorDenied); }
        // Locks remain held in the control DB through target commit. Revocation
        // there serializes with this reconciliation; no control writes are made.
        let mut authority = control.pool().begin().await?;
        let capabilities = [Capability::ViewStatus, Capability::ManageOperators,
            Capability::RestoreWorld, Capability::InitiateRecovery];
        let mut proof = serde_json::Value::Null;
        for capability in capabilities {
            proof = authorize_tx(&mut authority, context, capability, true).await?;
        }
        let (display, kind): (String, String) = sqlx::query_as(
            "SELECT display_name,kind FROM worldwright_operators WHERE subject=$1 AND enabled=TRUE")
            .bind(&context.claims.subject).fetch_one(&mut *authority).await?;
        let environment = match context.environment {
            worldwright_operations::Environment::Staging => "staging",
            worldwright_operations::Environment::Production => "production",
        };
        let mut target = self.pool().begin().await?;
        let worlds: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM worldwright_worlds")
            .fetch_one(&mut *target).await?;
        if worlds != 1 { return Err(PersistenceError::RecoveryConflict); }
        let fenced = sqlx::query("UPDATE worldwright_worlds SET authority_incarnation=gen_random_uuid(), \
            active_writer_id=NULL, writer_epoch=writer_epoch+1, updated_at=clock_timestamp() \
            WHERE world_id=$1 AND writer_epoch<9223372036854775807 AND pending_activation_id IS NULL \
            AND release_maintenance=FALSE")
            .bind(uuid_of(context.world)).execute(&mut *target).await?;
        if fenced.rows_affected()!=1 { return Err(PersistenceError::RecoveryConflict); }
        // Invalidate all restored login capability; keep accounts/identity links,
        // consumed invite evidence, receipts and business uniqueness untouched.
        sqlx::query("DELETE FROM worldwright_sessions WHERE world_id=$1")
            .bind(uuid_of(context.world)).execute(&mut *target).await?;
        sqlx::query("DELETE FROM worldwright_player_invites i WHERE i.world_id=$1 AND i.consumed_at IS NULL \
            AND NOT EXISTS (SELECT 1 FROM worldwright_oidc_audit a WHERE a.invite_id=i.invite_id)")
            .bind(uuid_of(context.world)).execute(&mut *target).await?;
        // Operators are global: a restored database is quarantined as a whole.
        // 31 seconds also invalidates tokens issued within the allowed clock skew.
        sqlx::query("UPDATE worldwright_operators SET enabled=FALSE, \
            tokens_valid_after=clock_timestamp()+INTERVAL '31 seconds', updated_at=clock_timestamp()")
            .execute(&mut *target).await?;
        sqlx::query("UPDATE worldwright_operator_grants SET revoked_at=clock_timestamp(), \
            revoked_by=$1, revocation_reason='restore security reconciliation' WHERE revoked_at IS NULL")
            .bind(&context.claims.subject).execute(&mut *target).await?;
        sqlx::query("INSERT INTO worldwright_operators(subject,display_name,kind,enabled,tokens_valid_after) \
            VALUES($1,$2,$3,TRUE,clock_timestamp()+INTERVAL '31 seconds') ON CONFLICT(subject) DO UPDATE \
            SET display_name=EXCLUDED.display_name,kind=EXCLUDED.kind,enabled=TRUE, \
            tokens_valid_after=EXCLUDED.tokens_valid_after,updated_at=clock_timestamp()")
            .bind(&context.claims.subject).bind(display).bind(kind).execute(&mut *target).await?;
        for capability in ["view_status","manage_operators","restore_world","initiate_recovery"] {
            sqlx::query("INSERT INTO worldwright_operator_grants(grant_id,subject,capability,target_environment,world_id,granted_by,reason) \
                VALUES(gen_random_uuid(),$1,$2,$3,$4,$1,'restore minimal recovery authority')")
                .bind(&context.claims.subject).bind(capability).bind(environment)
                .bind(uuid_of(context.world)).execute(&mut *target).await?;
        }
        sqlx::query("UPDATE worldwright_recovery_jobs SET state='failed',completed_at=clock_timestamp(), \
            summary=jsonb_build_object('phase','interrupted_by_restore') WHERE state='running'")
            .execute(&mut *target).await?;
        insert_audit(&mut target, operation, context.correlation, &context.claims.subject,
            Some(context.claims.session_id().map_err(|_| PersistenceError::InvalidData)?),
            serde_json::to_value(context.claims.assurance).map_err(|_| PersistenceError::InvalidData)?
                .as_str().ok_or(PersistenceError::InvalidData)?.to_owned(),
            "restore_verified", context.environment, context.world, "restore", &job.to_string(),
            context.reason, "confirmed", proof, None,
            Some(serde_json::json!({"phase":"security_reconciled","authority_source":"live_control_database",
                "fresh_token_delay_seconds":31,"old_operators_disabled":true,"old_grants_revoked":true}))).await?;
        // Re-check time/current control authority immediately before target commit.
        authorize_tx(&mut authority, context, Capability::RestoreWorld, true).await?;
        target.commit().await.map_err(|_| PersistenceError::UnknownOutcome)
    }
}
