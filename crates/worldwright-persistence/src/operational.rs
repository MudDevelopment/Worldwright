use crate::{
    PersistenceError,
    operational_queries::*,
    store::PostgresStore,
    types::{
        BootstrapOperator, OperationalStatus, OperatorContext, RecoveryJob, WriterLease,
        u64_from_i64, uuid_of,
    },
};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use worldwright_core::ids::{
    AccountId, AuthorityIncarnationId, CorrelationId, GrantId, OperationId, OperatorSessionId,
    RecoveryJobId, WriterId,
};
use worldwright_operations::{Assurance, Capability, Environment, OperatorKind};
use worldwright_release::Sha256Digest;

type OperationalStatusRow = (Option<Vec<u8>>, i64, uuid::Uuid, i64, Option<i64>);

impl PostgresStore {
    pub async fn authorize_operator(
        &self,
        context: &OperatorContext<'_>,
        capability: Capability,
    ) -> Result<(), PersistenceError> {
        let mut tx = self.pool().begin().await?;
        authorize_tx(&mut tx, context, capability, false).await?;
        Ok(())
    }

    pub async fn bootstrap_operator(
        &self,
        input: &BootstrapOperator<'_>,
        operation: OperationId,
        grant_ids: &[GrantId],
    ) -> Result<(), PersistenceError> {
        validate_subject(input.subject)?;
        validate_text(input.display_name)?;
        validate_text(input.reason)?;
        if input.capabilities.is_empty()
            || grant_ids.len() != input.capabilities.len()
            || !unique_capabilities(input.capabilities)
        {
            return Err(PersistenceError::InvalidData);
        }
        let mut tx = self.pool().begin().await?;
        sqlx::query(LOCK_OPERATOR_BOOTSTRAP)
            .execute(&mut *tx)
            .await?;
        let (count,): (i64,) = sqlx::query_as(BOOTSTRAP_OPERATOR_COUNT)
            .fetch_one(&mut *tx)
            .await?;
        if count != 0 {
            return Err(PersistenceError::OperatorDenied);
        }
        let kind = spell(input.kind)?;
        let environment = spell(input.environment)?;
        sqlx::query(INSERT_OPERATOR)
            .bind(input.subject)
            .bind(input.display_name)
            .bind(&kind)
            .execute(&mut *tx)
            .await?;
        for (capability, grant) in input.capabilities.iter().zip(grant_ids) {
            sqlx::query(INSERT_GRANT)
                .bind(uuid_of(*grant))
                .bind(input.subject)
                .bind(spell(*capability)?)
                .bind(&environment)
                .bind(uuid_of(input.world))
                .bind(input.subject)
                .bind(input.reason)
                .bind(Option::<i64>::None)
                .execute(&mut *tx)
                .await?;
        }
        insert_audit(
            &mut tx,
            operation,
            input.correlation,
            input.subject,
            None,
            "bootstrap".to_owned(),
            "operator_bootstrapped",
            input.environment,
            input.world,
            "operator",
            input.subject,
            input.reason,
            "confirmed",
            json!({"capabilities": input.capabilities.iter().map(|value| spell(*value)).collect::<Result<Vec<_>, _>>()?}),
            None,
            Some(json!({"kind": kind, "display_name": input.display_name})),
        )
        .await?;
        tx.commit()
            .await
            .map_err(|_| PersistenceError::UnknownOutcome)
    }

    pub async fn create_operator(
        &self,
        context: &OperatorContext<'_>,
        subject: &str,
        display_name: &str,
        kind: OperatorKind,
        operation: OperationId,
    ) -> Result<(), PersistenceError> {
        validate_subject(subject)?;
        validate_text(display_name)?;
        let mut tx = self.pool().begin().await?;
        let authorization =
            authorize_tx(&mut tx, context, Capability::ManageOperators, true).await?;
        sqlx::query(INSERT_OPERATOR)
            .bind(subject)
            .bind(display_name)
            .bind(spell(kind)?)
            .execute(&mut *tx)
            .await?;
        insert_audit(
            &mut tx,
            operation,
            context.correlation,
            context.claims.subject.as_str(),
            Some(operator_session(context)?),
            spell(context.claims.assurance)?,
            "operator_created",
            context.environment,
            context.world,
            "operator",
            subject,
            context.reason,
            "confirmed",
            authorization,
            None,
            Some(json!({"kind": spell(kind)?, "display_name": display_name, "enabled": true})),
        )
        .await?;
        tx.commit()
            .await
            .map_err(|_| PersistenceError::UnknownOutcome)
    }

    pub async fn set_account_disabled(
        &self,
        context: &OperatorContext<'_>,
        account: AccountId,
        disabled: bool,
        operation: OperationId,
    ) -> Result<(), PersistenceError> {
        let mut tx = self.pool().begin().await?;
        let authorization =
            authorize_tx(&mut tx, context, Capability::ModerateAccount, true).await?;
        let before: Option<(String, bool)> = sqlx::query_as(ACCOUNT_FOR_UPDATE)
            .bind(uuid_of(account))
            .fetch_optional(&mut *tx)
            .await?;
        let Some((username, was_disabled)) = before else {
            return Err(PersistenceError::NotFound);
        };
        let row: Option<(String, bool)> = sqlx::query_as(SET_ACCOUNT_DISABLED)
            .bind(uuid_of(account))
            .bind(disabled)
            .bind(context.reason)
            .fetch_optional(&mut *tx)
            .await?;
        let Some((updated_username, _)) = row else {
            return Err(PersistenceError::NotFound);
        };
        if updated_username != username {
            return Err(PersistenceError::InvalidData);
        }
        if disabled {
            sqlx::query(REVOKE_ACCOUNT_SESSIONS)
                .bind(uuid_of(account))
                .execute(&mut *tx)
                .await?;
        }
        insert_audit(
            &mut tx,
            operation,
            context.correlation,
            context.claims.subject.as_str(),
            Some(operator_session(context)?),
            spell(context.claims.assurance)?,
            if disabled {
                "account_suspended"
            } else {
                "account_reinstated"
            },
            context.environment,
            context.world,
            "account",
            &account.to_string(),
            context.reason,
            "confirmed",
            authorization,
            Some(json!({"username": username, "disabled": was_disabled})),
            Some(json!({"username": username, "disabled": disabled})),
        )
        .await?;
        tx.commit()
            .await
            .map_err(|_| PersistenceError::UnknownOutcome)
    }

    pub async fn revoke_operator_session(
        &self,
        context: &OperatorContext<'_>,
        subject: &str,
        session: OperatorSessionId,
        operation: OperationId,
    ) -> Result<(), PersistenceError> {
        validate_subject(subject)?;
        let mut tx = self.pool().begin().await?;
        let authorization =
            authorize_tx(&mut tx, context, Capability::ManageOperators, true).await?;
        let changed = sqlx::query(REVOKE_OPERATOR_SESSION)
            .bind(uuid_of(session))
            .bind(subject)
            .bind(context.claims.subject.as_str())
            .bind(context.reason)
            .execute(&mut *tx)
            .await?;
        if changed.rows_affected() != 1 {
            return Err(PersistenceError::OperatorDenied);
        }
        insert_audit(
            &mut tx,
            operation,
            context.correlation,
            context.claims.subject.as_str(),
            Some(operator_session(context)?),
            spell(context.claims.assurance)?,
            "operator_session_revoked",
            context.environment,
            context.world,
            "session",
            &session.to_string(),
            context.reason,
            "confirmed",
            authorization,
            None,
            Some(json!({"subject": subject})),
        )
        .await?;
        tx.commit()
            .await
            .map_err(|_| PersistenceError::UnknownOutcome)
    }

    pub async fn set_operator_enabled(
        &self,
        context: &OperatorContext<'_>,
        target_subject: &str,
        enabled: bool,
        operation: OperationId,
    ) -> Result<(), PersistenceError> {
        validate_subject(target_subject)?;
        let mut tx = self.pool().begin().await?;
        let authorization =
            authorize_tx(&mut tx, context, Capability::ManageOperators, true).await?;
        let before: Option<(String, bool)> = sqlx::query_as(OPERATOR_FOR_UPDATE)
            .bind(target_subject)
            .fetch_optional(&mut *tx)
            .await?;
        let Some((kind, old_enabled)) = before else {
            return Err(PersistenceError::NotFound);
        };
        let changed = sqlx::query(SET_OPERATOR_ENABLED)
            .bind(target_subject)
            .bind(enabled)
            .execute(&mut *tx)
            .await?;
        if changed.rows_affected() != 1 {
            return Err(PersistenceError::RecoveryConflict);
        }
        insert_audit(
            &mut tx,
            operation,
            context.correlation,
            context.claims.subject.as_str(),
            Some(operator_session(context)?),
            spell(context.claims.assurance)?,
            if enabled {
                "operator_enabled"
            } else {
                "operator_disabled"
            },
            context.environment,
            context.world,
            "operator",
            target_subject,
            context.reason,
            "confirmed",
            authorization,
            Some(json!({"kind": kind, "enabled": old_enabled})),
            Some(json!({"kind": kind, "enabled": enabled})),
        )
        .await?;
        tx.commit()
            .await
            .map_err(|_| PersistenceError::UnknownOutcome)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn grant(
        &self,
        context: &OperatorContext<'_>,
        grant_id: GrantId,
        target_subject: &str,
        capability: Capability,
        target_environment: Environment,
        target_world: worldwright_core::ids::WorldId,
        expires_seconds: Option<u64>,
        operation: OperationId,
    ) -> Result<(), PersistenceError> {
        validate_subject(target_subject)?;
        if target_world != context.world || target_environment != context.environment {
            return Err(PersistenceError::OperatorDenied);
        }
        if grant_id.get() == 0
            || expires_seconds.is_some_and(|value| !(1..=31_536_000).contains(&value))
        {
            return Err(PersistenceError::InvalidData);
        }
        let mut tx = self.pool().begin().await?;
        let authorization =
            authorize_tx(&mut tx, context, Capability::ManageOperators, true).await?;
        let target: Option<(String, bool)> = sqlx::query_as(OPERATOR_FOR_UPDATE)
            .bind(target_subject)
            .fetch_optional(&mut *tx)
            .await?;
        let Some((kind, enabled)) = target else {
            return Err(PersistenceError::NotFound);
        };
        if !enabled {
            return Err(PersistenceError::OperatorDenied);
        }
        sqlx::query(INSERT_GRANT)
            .bind(uuid_of(grant_id))
            .bind(target_subject)
            .bind(spell(capability)?)
            .bind(spell(target_environment)?)
            .bind(uuid_of(target_world))
            .bind(context.claims.subject.as_str())
            .bind(context.reason)
            .bind(
                expires_seconds
                    .map(|value| i64::try_from(value).map_err(|_| PersistenceError::InvalidData))
                    .transpose()?,
            )
            .execute(&mut *tx)
            .await?;
        insert_audit(
            &mut tx,
            operation,
            context.correlation,
            context.claims.subject.as_str(),
            Some(operator_session(context)?),
            spell(context.claims.assurance)?,
            "grant_created",
            context.environment,
            context.world,
            "grant",
            &grant_id.to_string(),
            context.reason,
            "confirmed",
            authorization,
            None,
            Some(
                json!({"subject": target_subject, "kind": kind, "capability": spell(capability)?}),
            ),
        )
        .await?;
        tx.commit()
            .await
            .map_err(|_| PersistenceError::UnknownOutcome)
    }

    pub async fn revoke_grant(
        &self,
        context: &OperatorContext<'_>,
        grant_id: GrantId,
        operation: OperationId,
    ) -> Result<(), PersistenceError> {
        let mut tx = self.pool().begin().await?;
        let authorization =
            authorize_tx(&mut tx, context, Capability::ManageOperators, true).await?;
        let row: Option<(String, String, String, uuid::Uuid)> = sqlx::query_as(REVOKE_GRANT)
            .bind(uuid_of(grant_id))
            .bind(context.claims.subject.as_str())
            .bind(context.reason)
            .fetch_optional(&mut *tx)
            .await?;
        let Some((subject, capability, environment, world)) = row else {
            return Err(PersistenceError::OperatorDenied);
        };
        let environment_value: Environment = serde_json::from_value(Value::String(environment))
            .map_err(|_| PersistenceError::InvalidData)?;
        let world = worldwright_core::ids::WorldId::from_u128(world.as_u128())
            .map_err(|_| PersistenceError::InvalidData)?;
        if environment_value != context.environment || world != context.world {
            return Err(PersistenceError::OperatorDenied);
        }
        insert_audit(
            &mut tx,
            operation,
            context.correlation,
            context.claims.subject.as_str(),
            Some(operator_session(context)?),
            spell(context.claims.assurance)?,
            "grant_revoked",
            context.environment,
            context.world,
            "grant",
            &grant_id.to_string(),
            context.reason,
            "confirmed",
            authorization,
            Some(json!({"subject": subject, "capability": capability})),
            None,
        )
        .await?;
        tx.commit()
            .await
            .map_err(|_| PersistenceError::UnknownOutcome)
    }

    pub async fn operational_status(
        &self,
        context: &OperatorContext<'_>,
    ) -> Result<OperationalStatus, PersistenceError> {
        let mut tx = self.pool().begin().await?;
        authorize_tx(&mut tx, context, Capability::ViewStatus, false).await?;
        let row: Option<OperationalStatusRow> = sqlx::query_as(OPERATOR_STATUS)
            .bind(uuid_of(context.world))
            .fetch_optional(&mut *tx)
            .await?;
        let Some((active, epoch, incarnation, backlog, oldest)) = row else {
            return Err(PersistenceError::NotFound);
        };
        Ok(OperationalStatus {
            active_release: digest(active)?,
            writer_epoch: u64_from_i64(epoch, false)?,
            authority_incarnation: AuthorityIncarnationId::from_u128(incarnation.as_u128())
                .map_err(|_| PersistenceError::InvalidData)?,
            outbox_backlog: u64_from_i64(backlog, false)?,
            outbox_oldest_seconds: oldest.map(|value| u64_from_i64(value, false)).transpose()?,
        })
    }

    pub async fn begin_recovery_job(
        &self,
        context: &OperatorContext<'_>,
        job: RecoveryJobId,
        kind: &str,
        operation: OperationId,
        summary: Value,
    ) -> Result<RecoveryJob, PersistenceError> {
        let capability = recovery_capability(kind)?;
        validate_summary(&summary, 4096)?;
        let mut tx = self.pool().begin().await?;
        let authorization = authorize_tx(&mut tx, context, capability, true).await?;
        let row: Option<(uuid::Uuid, Option<Vec<u8>>)> = sqlx::query_as(INSERT_RECOVERY_JOB)
            .bind(uuid_of(job))
            .bind(kind)
            .bind(spell(context.environment)?)
            .bind(context.claims.subject.as_str())
            .bind(uuid_of(operator_session(context)?))
            .bind(uuid_of(context.correlation))
            .bind(context.reason)
            .bind(summary)
            .bind(uuid_of(context.world))
            .fetch_optional(&mut *tx)
            .await?;
        let Some((incarnation, active)) = row else {
            return Err(PersistenceError::NotFound);
        };
        let action = if kind == "backup" {
            "backup_started"
        } else {
            "restore_started"
        };
        insert_audit(
            &mut tx,
            operation,
            context.correlation,
            context.claims.subject.as_str(),
            Some(operator_session(context)?),
            spell(context.claims.assurance)?,
            action,
            context.environment,
            context.world,
            if kind == "backup" {
                "backup"
            } else {
                "restore"
            },
            &job.to_string(),
            context.reason,
            "started",
            authorization,
            None,
            None,
        )
        .await?;
        tx.commit()
            .await
            .map_err(|_| PersistenceError::UnknownOutcome)?;
        Ok(RecoveryJob {
            job,
            source_incarnation: AuthorityIncarnationId::from_u128(incarnation.as_u128())
                .map_err(|_| PersistenceError::InvalidData)?,
            active_release: digest(active)?,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn complete_recovery_job(
        &self,
        context: &OperatorContext<'_>,
        kind: &str,
        job: RecoveryJobId,
        state: &str,
        backup_digest: Option<[u8; 32]>,
        manifest_digest: Option<[u8; 32]>,
        target_incarnation: Option<AuthorityIncarnationId>,
        operation: OperationId,
        summary: Value,
    ) -> Result<(), PersistenceError> {
        let capability = recovery_capability(kind)?;
        if !matches!(state, "succeeded" | "failed") {
            return Err(PersistenceError::InvalidData);
        }
        validate_summary(&summary, 4096)?;
        let mut tx = self.pool().begin().await?;
        let authorization = authorize_tx(&mut tx, context, capability, true).await?;
        let changed = sqlx::query(COMPLETE_RECOVERY_JOB)
            .bind(uuid_of(job))
            .bind(state)
            .bind(backup_digest.map(|value| value.to_vec()))
            .bind(manifest_digest.map(|value| value.to_vec()))
            .bind(target_incarnation.map(uuid_of))
            .bind(summary)
            .bind(kind)
            .bind(context.claims.subject.as_str())
            .bind(spell(context.environment)?)
            .bind(uuid_of(context.world))
            .execute(&mut *tx)
            .await?;
        if changed.rows_affected() != 1 {
            return Err(PersistenceError::RecoveryConflict);
        }
        let action = if kind == "backup" {
            if state == "succeeded" {
                "backup_succeeded"
            } else {
                "backup_failed"
            }
        } else if state == "succeeded" {
            "restore_verified"
        } else {
            "restore_failed"
        };
        insert_audit(
            &mut tx,
            operation,
            context.correlation,
            context.claims.subject.as_str(),
            Some(operator_session(context)?),
            spell(context.claims.assurance)?,
            action,
            context.environment,
            context.world,
            if kind == "backup" {
                "backup"
            } else {
                "restore"
            },
            &job.to_string(),
            context.reason,
            if state == "succeeded" {
                "confirmed"
            } else {
                "failed"
            },
            authorization,
            None,
            Some(json!({"state": state})),
        )
        .await?;
        tx.commit()
            .await
            .map_err(|_| PersistenceError::UnknownOutcome)
    }

    pub async fn rotate_authority(
        &self,
        context: &OperatorContext<'_>,
        writer: WriterId,
        operation: OperationId,
    ) -> Result<WriterLease, PersistenceError> {
        let mut tx = self.pool().begin().await?;
        let authorization =
            authorize_tx(&mut tx, context, Capability::InitiateRecovery, true).await?;
        let row: Option<(uuid::Uuid, i64)> = sqlx::query_as(ROTATE_AUTHORITY)
            .bind(uuid_of(context.world))
            .bind(uuid_of(writer))
            .fetch_optional(&mut *tx)
            .await?;
        let Some((incarnation, epoch)) = row else {
            return Err(PersistenceError::RecoveryConflict);
        };
        let incarnation = AuthorityIncarnationId::from_u128(incarnation.as_u128())
            .map_err(|_| PersistenceError::InvalidData)?;
        let epoch = u64_from_i64(epoch, true)?;
        insert_audit(
            &mut tx,
            operation,
            context.correlation,
            context.claims.subject.as_str(),
            Some(operator_session(context)?),
            spell(context.claims.assurance)?,
            "takeover_completed",
            context.environment,
            context.world,
            "world",
            &context.world.to_string(),
            context.reason,
            "confirmed",
            authorization,
            None,
            Some(json!({"writer": writer.to_string(), "epoch": epoch})),
        )
        .await?;
        tx.commit()
            .await
            .map_err(|_| PersistenceError::UnknownOutcome)?;
        Ok(WriterLease {
            world: context.world,
            writer,
            epoch,
            incarnation,
        })
    }

    pub async fn record_security_event(
        &self,
        context: &OperatorContext<'_>,
        code: &str,
        summary: Value,
    ) -> Result<(), PersistenceError> {
        validate_text(context.reason)?;
        if !matches!(
            code,
            "missing_token"
                | "invalid_token"
                | "expired_token"
                | "revoked_token"
                | "operator_disabled"
                | "capability_denied"
                | "scope_denied"
                | "assurance_denied"
                | "invalid_request"
        ) {
            return Err(PersistenceError::InvalidData);
        }
        validate_summary(&summary, 2048)?;
        sqlx::query(INSERT_SECURITY_EVENT)
            .bind(uuid_of(context.correlation))
            .bind(context.claims.subject.as_str())
            .bind(code)
            .bind(spell(context.environment)?)
            .bind(uuid_of(context.world))
            .bind(summary)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    pub async fn record_drain_started(
        &self,
        context: &OperatorContext<'_>,
        operation: OperationId,
    ) -> Result<(), PersistenceError> {
        let mut tx = self.pool().begin().await?;
        let authorization = authorize_tx(&mut tx, context, Capability::DrainWorld, true).await?;
        insert_audit(
            &mut tx,
            operation,
            context.correlation,
            context.claims.subject.as_str(),
            Some(operator_session(context)?),
            spell(context.claims.assurance)?,
            "drain_started",
            context.environment,
            context.world,
            "world",
            &context.world.to_string(),
            context.reason,
            "started",
            authorization,
            None,
            None,
        )
        .await?;
        tx.commit()
            .await
            .map_err(|_| PersistenceError::UnknownOutcome)
    }

    pub async fn record_drain_completed(
        &self,
        context: &OperatorContext<'_>,
        operation: OperationId,
    ) -> Result<(), PersistenceError> {
        let mut tx = self.pool().begin().await?;
        let authorization = authorize_tx(&mut tx, context, Capability::DrainWorld, true).await?;
        insert_audit(
            &mut tx,
            operation,
            context.correlation,
            context.claims.subject.as_str(),
            Some(operator_session(context)?),
            spell(context.claims.assurance)?,
            "drain_completed",
            context.environment,
            context.world,
            "world",
            &context.world.to_string(),
            context.reason,
            "confirmed",
            authorization,
            None,
            None,
        )
        .await?;
        tx.commit()
            .await
            .map_err(|_| PersistenceError::UnknownOutcome)
    }
}

pub(crate) async fn authorize_tx(
    tx: &mut Transaction<'_, Postgres>,
    context: &OperatorContext<'_>,
    capability: Capability,
    for_update: bool,
) -> Result<Value, PersistenceError> {
    let session = operator_session(context)?;
    let query = if for_update {
        AUTHORIZE_OPERATOR_FOR_UPDATE
    } else {
        AUTHORIZE_OPERATOR
    };
    let row: Option<(String, bool, bool, bool, bool)> = sqlx::query_as(query)
        .bind(context.claims.subject.as_str())
        .bind(context.claims.issued_at as f64)
        .bind(uuid_of(session))
        .bind(spell(capability)?)
        .bind(spell(context.environment)?)
        .bind(uuid_of(context.world))
        .fetch_optional(&mut **tx)
        .await?;
    let Some((kind, enabled, issued_after, session_active, capability_active)) = row else {
        return Err(PersistenceError::OperatorDenied);
    };
    let db_kind: OperatorKind =
        serde_json::from_value(Value::String(kind)).map_err(|_| PersistenceError::InvalidData)?;
    let service_allowed = matches!(
        capability,
        Capability::ViewStatus | Capability::RequestRelease | Capability::BackupWorld
    );
    if !enabled
        || !issued_after
        || !session_active
        || !capability_active
        || db_kind != context.claims.kind
        || (matches!(context.claims.kind, OperatorKind::Service) && !service_allowed)
        || (requires_mfa(capability)
            && !matches!(
                context.claims.assurance,
                Assurance::Mfa | Assurance::BreakGlass
            ))
        || (matches!(context.claims.kind, OperatorKind::BreakGlass)
            && context.claims.break_glass_reason.as_deref() != Some(context.reason))
    {
        return Err(PersistenceError::OperatorDenied);
    }
    Ok(json!({
        "capability": spell(capability)?,
        "kind": spell(context.claims.kind)?,
        "assurance": spell(context.claims.assurance)?,
    }))
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn insert_audit(
    tx: &mut Transaction<'_, Postgres>,
    operation: OperationId,
    correlation: CorrelationId,
    actor: &str,
    session: Option<OperatorSessionId>,
    assurance: String,
    action: &str,
    environment: Environment,
    world: worldwright_core::ids::WorldId,
    target_type: &str,
    target_id: &str,
    reason: &str,
    outcome: &str,
    authorization: Value,
    before: Option<Value>,
    after: Option<Value>,
) -> Result<(), PersistenceError> {
    sqlx::query(INSERT_OPERATOR_AUDIT)
        .bind(uuid_of(operation))
        .bind(uuid_of(correlation))
        .bind(actor)
        .bind(session.map(uuid_of))
        .bind(assurance)
        .bind(action)
        .bind(spell(environment)?)
        .bind(uuid_of(world))
        .bind(target_type)
        .bind(target_id)
        .bind(reason)
        .bind(outcome)
        .bind(authorization)
        .bind(before)
        .bind(after)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

fn spell<T: Serialize>(value: T) -> Result<String, PersistenceError> {
    serde_json::to_value(value)
        .map_err(|_| PersistenceError::InvalidData)?
        .as_str()
        .map(str::to_owned)
        .ok_or(PersistenceError::InvalidData)
}

fn recovery_capability(kind: &str) -> Result<Capability, PersistenceError> {
    match kind {
        "backup" => Ok(Capability::BackupWorld),
        "restore" => Ok(Capability::RestoreWorld),
        _ => Err(PersistenceError::InvalidData),
    }
}

fn requires_mfa(capability: Capability) -> bool {
    matches!(
        capability,
        Capability::ModerateAccount
            | Capability::ReviewRelease
            | Capability::ActivateRelease
            | Capability::DrainWorld
            | Capability::InitiateRecovery
            | Capability::ManageOperators
            | Capability::RestoreWorld
    )
}

fn validate_subject(value: &str) -> Result<(), PersistenceError> {
    let bytes = value.as_bytes();
    if bytes.is_empty()
        || bytes.len() > 128
        || !bytes[0].is_ascii_lowercase()
        || bytes[1..].iter().any(|byte| {
            !byte.is_ascii_lowercase() && !byte.is_ascii_digit() && !b"._-".contains(byte)
        })
    {
        Err(PersistenceError::InvalidData)
    } else {
        Ok(())
    }
}

fn operator_session(context: &OperatorContext<'_>) -> Result<OperatorSessionId, PersistenceError> {
    context
        .claims
        .session_id()
        .map_err(|_| PersistenceError::InvalidData)
}

fn validate_text(value: &str) -> Result<(), PersistenceError> {
    if value.is_empty()
        || value.len() > 512
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        Err(PersistenceError::InvalidData)
    } else {
        Ok(())
    }
}

fn validate_summary(value: &Value, max: usize) -> Result<(), PersistenceError> {
    if !value.is_object()
        || serde_json::to_vec(value)
            .map_err(|_| PersistenceError::InvalidData)?
            .len()
            > max
    {
        Err(PersistenceError::InvalidData)
    } else {
        Ok(())
    }
}

fn unique_capabilities(values: &[Capability]) -> bool {
    values
        .iter()
        .enumerate()
        .all(|(index, value)| !values[..index].contains(value))
}

fn digest(value: Option<Vec<u8>>) -> Result<Option<Sha256Digest>, PersistenceError> {
    value
        .map(|bytes| {
            if bytes.len() != 32 {
                return Err(PersistenceError::InvalidData);
            }
            let mut result = [0u8; 32];
            result.copy_from_slice(&bytes);
            Ok(Sha256Digest::from_digest_bytes(result))
        })
        .transpose()
}
