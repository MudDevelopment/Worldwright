use crate::{
    PersistenceError,
    store::PostgresStore,
    types::{
        ActivationPlan, DurableContentReferences, OperatorContext, PublicationRecord,
        ReleaseReadGuard, ReleaseState, StoredArtifact, WriterLease, i64_from_u64, u64_from_i64,
        uuid_of,
    },
};
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use std::str::FromStr;
use worldwright_core::ids::OperationId;
use worldwright_core::ids::{ActivationId, PublicationId, WorldId, WriterId};
use worldwright_operations::{Capability, Environment};
use worldwright_operations::{
    ReleaseSubmissionMetadata, StudioReviewContext, encode_release_submission,
};
use worldwright_release::{
    DeploymentTarget, PublicationKind, PublicationStatus, ReleaseId, ReviewDecision, Sha256Digest,
    verify_bytes,
};

const MAX_RELEASE_TEXT_BYTES: usize = 128;
const MAX_RELEASE_REASON_BYTES: usize = 512;

fn target_environment(target: DeploymentTarget) -> Environment {
    match target {
        DeploymentTarget::Staging => Environment::Staging,
        DeploymentTarget::Production => Environment::Production,
    }
}

fn target_environment_str(value: &str) -> Result<Environment, PersistenceError> {
    serde_json::from_value(Value::String(value.to_owned()))
        .map_err(|_| PersistenceError::InvalidData)
}

fn assurance(context: &OperatorContext<'_>) -> Result<String, PersistenceError> {
    serde_json::to_value(context.claims.assurance)
        .map_err(|_| PersistenceError::InvalidData)?
        .as_str()
        .map(str::to_owned)
        .ok_or(PersistenceError::InvalidData)
}

fn operator_session(
    context: &OperatorContext<'_>,
) -> Result<worldwright_core::ids::OperatorSessionId, PersistenceError> {
    context
        .claims
        .session_id()
        .map_err(|_| PersistenceError::InvalidData)
}

#[derive(Debug)]
struct ReleaseWorldRow {
    active: Option<Sha256Digest>,
    known_good: Option<Sha256Digest>,
    pending: Option<ActivationId>,
    maintenance: bool,
}

type ReleaseWorldSqlRow = (Option<Vec<u8>>, Option<Vec<u8>>, Option<uuid::Uuid>, bool);
type ReleaseReadSqlRow = (
    i64,
    Option<uuid::Uuid>,
    uuid::Uuid,
    Option<Vec<u8>>,
    Option<uuid::Uuid>,
    bool,
);
type PublicationSqlRow = (
    uuid::Uuid,
    Vec<u8>,
    Vec<u8>,
    String,
    String,
    String,
    Option<Vec<u8>>,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<serde_json::Value>,
);
type BeginActivationSqlRow = (
    uuid::Uuid,
    Vec<u8>,
    String,
    String,
    Option<Vec<u8>>,
    String,
    i16,
    Option<uuid::Uuid>,
    Option<uuid::Uuid>,
);
type ActivationSqlRow = (String, uuid::Uuid, Option<Vec<u8>>, Vec<u8>, uuid::Uuid);
type RecoveryActivationSqlRow = (uuid::Uuid, Vec<u8>, Option<Vec<u8>>, String, uuid::Uuid);

#[derive(Debug)]
struct ActivationRow {
    state: String,
    request: PublicationId,
    previous: Option<Sha256Digest>,
    target: Sha256Digest,
    writer_incarnation: uuid::Uuid,
}

impl PostgresStore {
    #[allow(clippy::too_many_arguments)]
    pub async fn submit_publication_authorized(
        &self,
        context: &OperatorContext<'_>,
        request: PublicationId,
        artifact: &[u8],
        kind: PublicationKind,
        target: DeploymentTarget,
        expected: Option<Sha256Digest>,
        operation: OperationId,
    ) -> Result<PublicationRecord, PersistenceError> {
        self.submit_publication_authorized_inner(
            context, request, artifact, kind, target, expected, None, operation,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn submit_publication_authorized_inner(
        &self,
        context: &OperatorContext<'_>,
        request: PublicationId,
        artifact: &[u8],
        kind: PublicationKind,
        target: DeploymentTarget,
        expected: Option<Sha256Digest>,
        review_context: Option<StudioReviewContext>,
        operation: OperationId,
    ) -> Result<PublicationRecord, PersistenceError> {
        if target_environment(target) != context.environment {
            return Err(PersistenceError::OperatorDenied);
        }
        let verified = verify_bytes("stored release artifact", artifact, None)
            .map_err(|_| PersistenceError::InvalidData)?;
        let artifact_digest = verified.artifact_digest();
        let release_id = verified.manifest().release_id().clone();
        let manifest =
            serde_json::to_value(verified.manifest()).map_err(|_| PersistenceError::InvalidData)?;
        let mut tx = self.pool().begin().await?;
        let authorization =
            crate::operational::authorize_tx(&mut tx, context, Capability::RequestRelease, true)
                .await?;
        let conflicts: Vec<(Vec<u8>, String, Vec<u8>)> = sqlx::query_as(
            "SELECT artifact_digest, release_id, artifact_bytes FROM worldwright_content_artifacts WHERE artifact_digest = $1 OR release_id = $2 FOR UPDATE",
        )
        .bind(artifact_digest.as_bytes().as_slice())
        .bind(release_id.as_str())
        .fetch_all(&mut *tx)
        .await?;
        let mut artifact_present = false;
        for (stored_digest, stored_release, stored_bytes) in conflicts {
            let stored_digest =
                digest(Some(stored_digest))?.ok_or(PersistenceError::InvalidData)?;
            if stored_digest != artifact_digest
                || stored_release != release_id.as_str()
                || stored_bytes != artifact
            {
                return Err(PersistenceError::ReleaseConflict);
            }
            artifact_present = true;
        }
        if !artifact_present {
            sqlx::query(
                "INSERT INTO worldwright_content_artifacts (artifact_digest, release_id, artifact_bytes, manifest, payload_digest, content_schema_version, game_id, game_contract_version) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
            )
            .bind(artifact_digest.as_bytes().as_slice())
            .bind(release_id.as_str())
            .bind(artifact)
            .bind(manifest)
            .bind(verified.manifest().payload_sha256().as_bytes().as_slice())
            .bind(i64_from_u64(verified.manifest().content_schema_version(), true)?)
            .bind(verified.manifest().game_id())
            .bind(i64_from_u64(u64::from(verified.manifest().contract_version()), true)?)
            .execute(&mut *tx)
            .await?;
        }
        let current = lock_world(&mut tx, context.world).await?;
        if current.pending.is_some() || current.maintenance {
            return Err(PersistenceError::InvalidTransition);
        }
        if current.active != expected {
            return Err(PersistenceError::ExpectedReleaseConflict);
        }
        sqlx::query(
            "INSERT INTO worldwright_publication_requests (request_id, world_id, artifact_digest, kind, target_environment, expected_current_digest, requested_by, authorization_version, requested_session_id, request_reason, request_correlation_id, studio_review_context) VALUES ($1,$2,$3,$4,$5,$6,$7,1,$8,$9,$10,$11)",
        )
        .bind(uuid_of(request))
        .bind(uuid_of(context.world))
        .bind(artifact_digest.as_bytes().as_slice())
        .bind(kind.to_string())
        .bind(target.to_string())
        .bind(expected.map(|value| value.as_bytes().to_vec()))
        .bind(context.claims.subject.as_str())
        .bind(uuid_of(operator_session(context)?))
        .bind(context.reason)
        .bind(uuid_of(context.correlation))
        .bind(review_context.as_ref().map(serde_json::to_value).transpose().map_err(|_| PersistenceError::InvalidData)?)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO worldwright_release_audit (world_id, request_id, actor, action, artifact_digest, target_environment, summary) VALUES ($1,$2,$3,'publication_requested',$4,$5,$6)",
        )
        .bind(uuid_of(context.world))
        .bind(uuid_of(request))
        .bind(context.claims.subject.as_str())
        .bind(artifact_digest.as_bytes().as_slice())
        .bind(target.to_string())
        .bind(json!({"release_id": release_id.as_str(), "kind": kind.to_string()}))
        .execute(&mut *tx)
        .await?;
        crate::operational::insert_audit(
            &mut tx,
            operation,
            context.correlation,
            context.claims.subject.as_str(),
            Some(operator_session(context)?),
            assurance(context)?,
            "release_requested",
            target_environment(target),
            context.world,
            "release",
            &request.to_string(),
            context.reason,
            "confirmed",
            authorization,
            None,
            Some(json!({"release_id": release_id.as_str(), "kind": kind.to_string()})),
        )
        .await?;
        tx.commit()
            .await
            .map_err(|_| PersistenceError::UnknownOutcome)?;
        self.publication(request).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn submit_publication_authorized_with_context(
        &self,
        context: &OperatorContext<'_>,
        request: PublicationId,
        artifact: &[u8],
        kind: PublicationKind,
        target: DeploymentTarget,
        expected: Option<Sha256Digest>,
        review_context: StudioReviewContext,
        operation: OperationId,
    ) -> Result<PublicationRecord, PersistenceError> {
        let correlation = context.correlation.to_string();
        encode_release_submission(
            &ReleaseSubmissionMetadata {
                schema_version: 1,
                kind,
                target,
                expected_current: expected,
                reason: context.reason.to_owned(),
                correlation_id: correlation,
                review_context: review_context.clone(),
            },
            artifact,
        )
        .map_err(|_| PersistenceError::InvalidData)?;
        self.submit_publication_authorized_inner(
            context,
            request,
            artifact,
            kind,
            target,
            expected,
            Some(review_context),
            operation,
        )
        .await
    }

    pub async fn review_publication_authorized(
        &self,
        context: &OperatorContext<'_>,
        request: PublicationId,
        decision: ReviewDecision,
        operation: OperationId,
    ) -> Result<PublicationRecord, PersistenceError> {
        validate_reason(context.reason)?;
        let mut tx = self.pool().begin().await?;
        let authorization =
            crate::operational::authorize_tx(&mut tx, context, Capability::ReviewRelease, true)
                .await?;
        let row: Option<(uuid::Uuid, Vec<u8>, String, String, String, i16)> = sqlx::query_as(
            "SELECT world_id, artifact_digest, requested_by, status, target_environment, authorization_version FROM worldwright_publication_requests WHERE request_id = $1 FOR UPDATE",
        )
        .bind(uuid_of(request))
        .fetch_optional(&mut *tx)
        .await?;
        let Some((world_uuid, digest_bytes, requester, status, target, version)) = row else {
            return Err(PersistenceError::NotFound);
        };
        if version != 1 || status != "requested" || requester == context.claims.subject {
            return Err(PersistenceError::InvalidTransition);
        }
        let world =
            WorldId::from_u128(world_uuid.as_u128()).map_err(|_| PersistenceError::InvalidData)?;
        if world != context.world || target_environment_str(&target)? != context.environment {
            return Err(PersistenceError::OperatorDenied);
        }
        let artifact_digest = digest(Some(digest_bytes))?.ok_or(PersistenceError::InvalidData)?;
        let status = match decision {
            ReviewDecision::Approve => PublicationStatus::Approved,
            ReviewDecision::Reject => PublicationStatus::Rejected,
        };
        let changed = sqlx::query(
            "UPDATE worldwright_publication_requests SET status = $2, reviewed_by = $3, review_reason = $4, reviewed_at = clock_timestamp(), reviewed_session_id = $5, review_correlation_id = $6 WHERE request_id = $1 AND status = 'requested' AND authorization_version = 1",
        )
        .bind(uuid_of(request))
        .bind(status.to_string())
        .bind(context.claims.subject.as_str())
        .bind(context.reason)
        .bind(uuid_of(operator_session(context)?))
        .bind(uuid_of(context.correlation))
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() != 1 {
            return Err(PersistenceError::InvalidTransition);
        }
        let (action, release_action) = if decision == ReviewDecision::Approve {
            ("release_approved", "publication_approved")
        } else {
            ("release_rejected", "publication_rejected")
        };
        sqlx::query(
            "INSERT INTO worldwright_release_audit (world_id, request_id, actor, action, artifact_digest, target_environment, summary) VALUES ($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(world_uuid)
        .bind(uuid_of(request))
        .bind(context.claims.subject.as_str())
        .bind(release_action)
        .bind(artifact_digest.as_bytes().as_slice())
        .bind(&target)
        .bind(json!({"reason": context.reason, "status": status.to_string()}))
        .execute(&mut *tx)
        .await?;
        crate::operational::insert_audit(
            &mut tx,
            operation,
            context.correlation,
            context.claims.subject.as_str(),
            Some(operator_session(context)?),
            assurance(context)?,
            action,
            target_environment_str(&target)?,
            world,
            "release",
            &request.to_string(),
            context.reason,
            "confirmed",
            authorization,
            Some(json!({"requested_by": requester})),
            Some(json!({"status": status.to_string()})),
        )
        .await?;
        tx.commit()
            .await
            .map_err(|_| PersistenceError::UnknownOutcome)?;
        self.publication(request).await
    }

    pub async fn publication_authorized(
        &self,
        context: &OperatorContext<'_>,
        request: PublicationId,
    ) -> Result<PublicationRecord, PersistenceError> {
        let mut tx = self.pool().begin().await?;
        let scope: Option<(uuid::Uuid, String)> = sqlx::query_as(
            "SELECT world_id, target_environment FROM worldwright_publication_requests WHERE request_id = $1",
        )
        .bind(uuid_of(request))
        .fetch_optional(&mut *tx)
        .await?;
        let Some((world, environment)) = scope else {
            return Err(PersistenceError::NotFound);
        };
        if world_id(world)? != context.world
            || target_environment_str(&environment)? != context.environment
        {
            return Err(PersistenceError::OperatorDenied);
        }
        crate::operational::authorize_tx(&mut tx, context, Capability::ViewStatus, false).await?;
        publication_in_transaction(&mut tx, request).await
    }

    pub async fn begin_activation_authorized(
        &self,
        context: &OperatorContext<'_>,
        request: PublicationId,
        activation: ActivationId,
        writer: WriterId,
        operation: OperationId,
    ) -> Result<ActivationPlan, PersistenceError> {
        self.begin_activation_inner(
            request,
            activation,
            writer,
            context.claims.subject.as_str(),
            Some((context, operation)),
        )
        .await
    }

    pub async fn complete_activation_authorized(
        &self,
        context: &OperatorContext<'_>,
        plan: ActivationPlan,
        operation: OperationId,
    ) -> Result<(), PersistenceError> {
        self.complete_activation_inner(plan, Some((context, operation)))
            .await
    }

    pub async fn fail_activation_authorized(
        &self,
        context: &OperatorContext<'_>,
        plan: ActivationPlan,
        code: &str,
        operation: OperationId,
    ) -> Result<(), PersistenceError> {
        self.fail_activation_inner(plan, code, Some((context, operation)))
            .await
    }

    pub async fn recover_activation_authorized(
        &self,
        context: &OperatorContext<'_>,
        new_writer: WriterId,
        operation: OperationId,
    ) -> Result<ActivationPlan, PersistenceError> {
        self.recover_activation_inner(
            context.world,
            new_writer,
            context.claims.subject.as_str(),
            Some((context, operation)),
        )
        .await
    }

    pub async fn begin_release_read(
        &self,
        lease: WriterLease,
        expected_release: Sha256Digest,
    ) -> Result<ReleaseReadGuard, PersistenceError> {
        let mut transaction = self.pool().begin().await?;
        let row: Option<ReleaseReadSqlRow> = sqlx::query_as(
            "SELECT writer_epoch, active_writer_id, authority_incarnation, active_release_digest, pending_activation_id, release_maintenance FROM worldwright_worlds WHERE world_id = $1 FOR SHARE",
        )
        .bind(uuid_of(lease.world()))
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((epoch, writer, incarnation, active, pending, maintenance)) = row else {
            return Err(PersistenceError::StaleWriter);
        };
        if u64_from_i64(epoch, true)? != lease.epoch()
            || writer != Some(uuid_of(lease.writer()))
            || incarnation != uuid_of(lease.incarnation())
            || digest(active)? != Some(expected_release)
            || pending.is_some()
            || maintenance
        {
            return Err(PersistenceError::StaleWriter);
        }
        Ok(ReleaseReadGuard {
            _transaction: transaction,
        })
    }

    pub async fn release_state(&self, world: WorldId) -> Result<ReleaseState, PersistenceError> {
        let row: Option<ReleaseWorldSqlRow> =
            sqlx::query_as("SELECT active_release_digest, known_good_release_digest, pending_activation_id, release_maintenance FROM worldwright_worlds WHERE world_id = $1")
                .bind(uuid_of(world))
                .fetch_optional(self.pool())
                .await?;
        let row = row.ok_or(PersistenceError::NotFound)?;
        let pending = row.2.map(activation_id).transpose()?;
        Ok(ReleaseState {
            active: digest(row.0)?,
            known_good: digest(row.1)?,
            pending,
            maintenance: row.3,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn submit_publication(
        &self,
        request: PublicationId,
        world: WorldId,
        artifact: &[u8],
        kind: PublicationKind,
        target: DeploymentTarget,
        expected_current: Option<Sha256Digest>,
        requested_by: &str,
    ) -> Result<PublicationRecord, PersistenceError> {
        validate_actor(requested_by)?;
        if target != DeploymentTarget::Staging {
            return Err(PersistenceError::ProductionGateRequired);
        }
        let verified = verify_bytes("stored release artifact", artifact, None)
            .map_err(|_| PersistenceError::InvalidData)?;
        let artifact_digest = verified.artifact_digest();
        let release_id = verified.manifest().release_id().clone();
        let manifest =
            serde_json::to_value(verified.manifest()).map_err(|_| PersistenceError::InvalidData)?;
        let mut tx = self.pool().begin().await?;
        let conflicts: Vec<(Vec<u8>, String, Vec<u8>)> = sqlx::query_as(
            "SELECT artifact_digest, release_id, artifact_bytes FROM worldwright_content_artifacts WHERE artifact_digest = $1 OR release_id = $2 FOR UPDATE",
        )
        .bind(artifact_digest.as_bytes().as_slice())
        .bind(release_id.as_str())
        .fetch_all(&mut *tx)
        .await?;
        let mut artifact_present = false;
        for (stored_digest, stored_release, stored_bytes) in conflicts {
            let stored_digest =
                digest(Some(stored_digest))?.ok_or(PersistenceError::InvalidData)?;
            verify_bytes(
                "stored release artifact",
                &stored_bytes,
                Some(stored_digest),
            )
            .map_err(|_| PersistenceError::InvalidData)?;
            if stored_digest == artifact_digest
                && stored_release == release_id.as_str()
                && stored_bytes == artifact
            {
                artifact_present = true;
            } else {
                return Err(PersistenceError::ReleaseConflict);
            }
        }
        if !artifact_present {
            sqlx::query(
                "INSERT INTO worldwright_content_artifacts (artifact_digest, release_id, artifact_bytes, manifest, payload_digest, content_schema_version, game_id, game_contract_version) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
            )
            .bind(artifact_digest.as_bytes().as_slice())
            .bind(release_id.as_str())
            .bind(artifact)
            .bind(manifest)
            .bind(verified.manifest().payload_sha256().as_bytes().as_slice())
            .bind(i64_from_u64(
                verified.manifest().content_schema_version(),
                true,
            )?)
            .bind(verified.manifest().game_id())
            .bind(i64_from_u64(
                u64::from(verified.manifest().contract_version()),
                true,
            )?)
            .execute(&mut *tx)
            .await?;
        }
        let current = lock_world(&mut tx, world).await?;
        if current.pending.is_some() || current.maintenance {
            return Err(PersistenceError::InvalidTransition);
        }
        if current.active != expected_current {
            return Err(PersistenceError::ExpectedReleaseConflict);
        }
        sqlx::query(
            "INSERT INTO worldwright_publication_requests (request_id, world_id, artifact_digest, kind, target_environment, expected_current_digest, requested_by) VALUES ($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(uuid_of(request))
        .bind(uuid_of(world))
        .bind(artifact_digest.as_bytes().as_slice())
        .bind(kind.to_string())
        .bind(target.to_string())
        .bind(expected_current.map(|value| value.as_bytes().to_vec()))
        .bind(requested_by)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO worldwright_release_audit (world_id, request_id, actor, action, artifact_digest, target_environment, summary) VALUES ($1,$2,$3,'publication_requested',$4,$5,$6)",
        )
        .bind(uuid_of(world))
        .bind(uuid_of(request))
        .bind(requested_by)
        .bind(artifact_digest.as_bytes().as_slice())
        .bind(target.to_string())
        .bind(json!({"release_id": release_id.as_str(), "kind": kind.to_string()}))
        .execute(&mut *tx)
        .await?;
        tx.commit()
            .await
            .map_err(|_| PersistenceError::UnknownOutcome)?;
        Ok(PublicationRecord {
            request,
            world,
            artifact_digest,
            release_id,
            kind,
            target,
            expected_current,
            requested_by: requested_by.to_owned(),
            status: PublicationStatus::Requested,
            reviewed_by: None,
            review_reason: None,
            activation_executor: None,
            review_context: None,
        })
    }

    pub async fn review_publication(
        &self,
        request: PublicationId,
        reviewer: &str,
        decision: ReviewDecision,
        reason: &str,
    ) -> Result<PublicationRecord, PersistenceError> {
        validate_actor(reviewer)?;
        validate_reason(reason)?;
        let mut tx = self.pool().begin().await?;
        let row: Option<(uuid::Uuid, Vec<u8>, String, String, String, i16)> = sqlx::query_as(
            "SELECT world_id, artifact_digest, requested_by, status, target_environment, authorization_version FROM worldwright_publication_requests WHERE request_id = $1 FOR UPDATE",
        )
        .bind(uuid_of(request))
        .fetch_optional(&mut *tx)
        .await?;
        let Some((world_uuid, digest_bytes, requested_by, status, target, authorization_version)) =
            row
        else {
            return Err(PersistenceError::NotFound);
        };
        validate_actor(&requested_by)?;
        if status != "requested" || target != "staging" || authorization_version != 0 {
            return Err(PersistenceError::InvalidTransition);
        }
        if decision == ReviewDecision::Approve && reviewer == requested_by {
            return Err(PersistenceError::InvalidTransition);
        }
        let status = match decision {
            ReviewDecision::Approve => PublicationStatus::Approved,
            ReviewDecision::Reject => PublicationStatus::Rejected,
        };
        let changed = sqlx::query(
            "UPDATE worldwright_publication_requests SET status = $2, reviewed_by = $3, review_reason = $4, reviewed_at = clock_timestamp() WHERE request_id = $1 AND status = 'requested'",
        )
        .bind(uuid_of(request))
        .bind(status.to_string())
        .bind(reviewer)
        .bind(reason)
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() != 1 {
            return Err(PersistenceError::InvalidTransition);
        }
        let artifact_digest = digest(Some(digest_bytes))?.ok_or(PersistenceError::InvalidData)?;
        let action = match decision {
            ReviewDecision::Approve => "publication_approved",
            ReviewDecision::Reject => "publication_rejected",
        };
        sqlx::query(
            "INSERT INTO worldwright_release_audit (world_id, request_id, actor, action, artifact_digest, target_environment, summary) VALUES ($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(world_uuid)
        .bind(uuid_of(request))
        .bind(reviewer)
        .bind(action)
        .bind(artifact_digest.as_bytes().as_slice())
        .bind(&target)
        .bind(json!({"reason": reason, "status": status.to_string()}))
        .execute(&mut *tx)
        .await?;
        tx.commit()
            .await
            .map_err(|_| PersistenceError::UnknownOutcome)?;
        self.publication(request).await
    }

    pub async fn publication(
        &self,
        request: PublicationId,
    ) -> Result<PublicationRecord, PersistenceError> {
        let row: Option<PublicationSqlRow> = sqlx::query_as(
            "SELECT r.world_id, r.artifact_digest, a.payload_digest, a.release_id, r.kind, r.target_environment, r.expected_current_digest, r.requested_by, r.status, r.reviewed_by, r.review_reason, r.activation_executor, r.studio_review_context FROM worldwright_publication_requests r JOIN worldwright_content_artifacts a ON a.artifact_digest = r.artifact_digest WHERE r.request_id = $1",
        )
        .bind(uuid_of(request))
        .fetch_optional(self.pool())
        .await?;
        let Some((
            world_uuid,
            digest_bytes,
            payload_digest,
            release_id,
            kind,
            target,
            expected,
            requested_by,
            status,
            reviewed_by,
            review_reason,
            activation_executor,
            review_context,
        )) = row
        else {
            return Err(PersistenceError::NotFound);
        };
        validate_actor(&requested_by)?;
        if let Some(value) = reviewed_by.as_deref() {
            validate_actor(value)?;
        }
        if let Some(value) = review_reason.as_deref() {
            validate_reason(value)?;
        }
        if let Some(value) = activation_executor.as_deref() {
            validate_actor(value)?;
        }
        let review_context = review_context
            .map(|value| {
                serde_json::from_value::<StudioReviewContext>(value)
                    .map_err(|_| PersistenceError::InvalidData)
            })
            .transpose()?;
        if let Some(context) = review_context.as_ref() {
            let artifact_digest =
                digest(Some(digest_bytes.clone()))?.ok_or(PersistenceError::InvalidData)?;
            if payload_digest.len() != 32 {
                return Err(PersistenceError::InvalidData);
            }
            let mut snapshot = [0u8; 32];
            snapshot.copy_from_slice(&payload_digest);
            if context.snapshot_sha256 != worldwright_operations::encode_hex(&snapshot)
                || context.preview_artifact_sha256 != artifact_digest.to_string()
            {
                return Err(PersistenceError::InvalidData);
            }
        }
        Ok(PublicationRecord {
            request,
            world: world_id(world_uuid)?,
            artifact_digest: digest(Some(digest_bytes))?.ok_or(PersistenceError::InvalidData)?,
            release_id: release_id
                .parse()
                .map_err(|_| PersistenceError::InvalidData)?,
            kind: kind.parse().map_err(|_| PersistenceError::InvalidData)?,
            target: target.parse().map_err(|_| PersistenceError::InvalidData)?,
            expected_current: digest(expected)?,
            requested_by,
            status: status.parse().map_err(|_| PersistenceError::InvalidData)?,
            reviewed_by,
            review_reason,
            activation_executor,
            review_context,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn begin_activation(
        &self,
        request: PublicationId,
        activation: ActivationId,
        writer: WriterId,
        executor: &str,
    ) -> Result<ActivationPlan, PersistenceError> {
        self.begin_activation_inner(request, activation, writer, executor, None)
            .await
    }

    async fn begin_activation_inner(
        &self,
        request: PublicationId,
        activation: ActivationId,
        writer: WriterId,
        executor: &str,
        authorized: Option<(&OperatorContext<'_>, OperationId)>,
    ) -> Result<ActivationPlan, PersistenceError> {
        validate_actor(executor)?;
        let mut tx = self.pool().begin().await?;
        let authorization = if let Some((context, _)) = authorized {
            if context.claims.subject != executor {
                return Err(PersistenceError::OperatorDenied);
            }
            Some(
                crate::operational::authorize_tx(
                    &mut tx,
                    context,
                    Capability::ActivateRelease,
                    true,
                )
                .await?,
            )
        } else {
            None
        };
        let row: Option<BeginActivationSqlRow> =
            sqlx::query_as(
                "SELECT world_id, artifact_digest, kind, target_environment, expected_current_digest, status, authorization_version, reviewed_session_id, review_correlation_id FROM worldwright_publication_requests WHERE request_id = $1 FOR UPDATE",
            )
            .bind(uuid_of(request))
            .fetch_optional(&mut *tx)
            .await?;
        let Some((
            world_uuid,
            target_bytes,
            kind,
            target,
            expected,
            status,
            authorization_version,
            reviewed_session,
            review_correlation,
        )) = row
        else {
            return Err(PersistenceError::NotFound);
        };
        if status != "approved" {
            return Err(PersistenceError::InvalidTransition);
        }
        let world = world_id(world_uuid)?;
        let kind = PublicationKind::from_str(&kind).map_err(|_| PersistenceError::InvalidData)?;
        let target =
            DeploymentTarget::from_str(&target).map_err(|_| PersistenceError::InvalidData)?;
        match authorized {
            Some((context, _)) => {
                if authorization_version != 1
                    || reviewed_session.is_none()
                    || review_correlation.is_none()
                    || context.world != world
                    || context.environment != target_environment(target)
                {
                    return Err(PersistenceError::OperatorDenied);
                }
            }
            None => {
                if authorization_version != 0 || target != DeploymentTarget::Staging {
                    return Err(PersistenceError::ProductionGateRequired);
                }
            }
        }
        let target_digest = digest(Some(target_bytes))?.ok_or(PersistenceError::InvalidData)?;
        let expected = digest(expected)?;
        let current = lock_world(&mut tx, world).await?;
        if current.pending.is_some() || current.maintenance {
            return Err(PersistenceError::InvalidTransition);
        }
        if current.active != expected {
            return Err(PersistenceError::ExpectedReleaseConflict);
        }
        if kind == PublicationKind::Rollback && current.known_good != Some(target_digest) {
            return Err(PersistenceError::InvalidTransition);
        }
        let writer_row: Option<(uuid::Uuid, i64)> = sqlx::query_as(
            "UPDATE worldwright_worlds SET writer_epoch = writer_epoch + 1, active_writer_id = $2, release_maintenance = TRUE, updated_at = clock_timestamp() WHERE world_id = $1 AND writer_epoch < 9223372036854775807 RETURNING authority_incarnation, writer_epoch",
        )
        .bind(world_uuid)
        .bind(uuid_of(writer))
        .fetch_optional(&mut *tx)
        .await?;
        let (incarnation, epoch) = writer_row
            .map(|row| {
                Ok::<_, PersistenceError>((
                    worldwright_core::ids::AuthorityIncarnationId::from_u128(row.0.as_u128())
                        .map_err(|_| PersistenceError::InvalidData)?,
                    u64_from_i64(row.1, true)?,
                ))
            })
            .transpose()?
            .ok_or(PersistenceError::CounterExhausted)?;
        let artifact_row: Option<(Vec<u8>, String)> = sqlx::query_as(
            "SELECT a.artifact_bytes, a.release_id FROM worldwright_content_artifacts a WHERE a.artifact_digest = $1",
        )
        .bind(target_digest.as_bytes().as_slice())
        .fetch_optional(&mut *tx)
        .await?;
        let Some((bytes, release_id)) = artifact_row else {
            return Err(PersistenceError::NotFound);
        };
        let verified = verify_bytes("stored release artifact", &bytes, Some(target_digest))
            .map_err(|_| PersistenceError::InvalidData)?;
        let release_id: ReleaseId = release_id
            .parse()
            .map_err(|_| PersistenceError::InvalidData)?;
        if release_id != *verified.manifest().release_id() {
            return Err(PersistenceError::InvalidData);
        }
        let stored = StoredArtifact {
            bytes,
            digest: verified.artifact_digest(),
            release_id,
        };
        sqlx::query(
            "INSERT INTO worldwright_release_activations (activation_id, request_id, world_id, previous_release_digest, target_release_digest, writer_id, writer_epoch, writer_incarnation) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
        )
        .bind(uuid_of(activation))
        .bind(uuid_of(request))
        .bind(world_uuid)
        .bind(current.active.map(|value| value.as_bytes().to_vec()))
        .bind(target_digest.as_bytes().as_slice())
        .bind(uuid_of(writer))
        .bind(i64_from_u64(epoch, true)?)
        .bind(uuid_of(incarnation))
        .execute(&mut *tx)
        .await?;
        let changed = sqlx::query(
            "UPDATE worldwright_worlds SET pending_activation_id = $2 WHERE world_id = $1 AND pending_activation_id IS NULL",
        )
        .bind(world_uuid)
        .bind(uuid_of(activation))
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() != 1 {
            return Err(PersistenceError::InvalidTransition);
        }
        let changed = if let Some((context, _)) = authorized {
            sqlx::query(
                "UPDATE worldwright_publication_requests SET status = 'activating', activation_executor = $2, activation_session_id = $3, activation_correlation_id = $4 WHERE request_id = $1 AND status = 'approved' AND authorization_version = 1",
            )
            .bind(uuid_of(request))
            .bind(executor)
            .bind(uuid_of(operator_session(context)?))
            .bind(uuid_of(context.correlation))
            .execute(&mut *tx)
            .await?
        } else {
            sqlx::query(
                "UPDATE worldwright_publication_requests SET status = 'activating', activation_executor = $2 WHERE request_id = $1 AND status = 'approved' AND authorization_version = 0",
            )
            .bind(uuid_of(request))
            .bind(executor)
            .execute(&mut *tx)
            .await?
        };
        if changed.rows_affected() != 1 {
            return Err(PersistenceError::InvalidTransition);
        }
        sqlx::query(
            "INSERT INTO worldwright_release_audit (world_id, request_id, activation_id, actor, action, artifact_digest, target_environment, summary) VALUES ($1,$2,$3,$4,'activation_started',$5,$6,$7)",
        )
        .bind(world_uuid)
        .bind(uuid_of(request))
        .bind(uuid_of(activation))
        .bind(executor)
        .bind(target_digest.as_bytes().as_slice())
        .bind(target.to_string())
        .bind(json!({"release_id": stored.release_id.as_str(), "kind": kind.to_string()}))
        .execute(&mut *tx)
        .await?;
        if let Some((context, operation)) = authorized {
            crate::operational::insert_audit(
                &mut tx,
                operation,
                context.correlation,
                context.claims.subject.as_str(),
                Some(operator_session(context)?),
                assurance(context)?,
                "activation_started",
                context.environment,
                context.world,
                "release",
                &request.to_string(),
                context.reason,
                "started",
                authorization.ok_or(PersistenceError::InvalidData)?,
                None,
                Some(json!({"activation_id": activation.to_string(), "release_id": stored.release_id.as_str()})),
            )
            .await?;
        }
        tx.commit()
            .await
            .map_err(|_| PersistenceError::UnknownOutcome)?;
        Ok(ActivationPlan {
            activation,
            request,
            world,
            artifact: stored,
            kind,
            target,
            previous: current.active,
            executor: executor.to_owned(),
            authorization_version,
            lease: WriterLease {
                world,
                writer,
                epoch,
                incarnation,
            },
        })
    }

    pub async fn complete_activation(&self, plan: ActivationPlan) -> Result<(), PersistenceError> {
        if plan.authorization_version != 0 {
            return Err(PersistenceError::OperatorDenied);
        }
        self.complete_activation_inner(plan, None).await
    }

    async fn complete_activation_inner(
        &self,
        plan: ActivationPlan,
        authorized: Option<(&OperatorContext<'_>, OperationId)>,
    ) -> Result<(), PersistenceError> {
        let mut tx = self.pool().begin().await?;
        let authorization = match authorized {
            Some((context, _)) => {
                if plan.authorization_version != 1
                    || plan.executor != context.claims.subject
                    || plan.world != context.world
                    || target_environment(plan.target) != context.environment
                {
                    return Err(PersistenceError::OperatorDenied);
                }
                Some(
                    crate::operational::authorize_tx(
                        &mut tx,
                        context,
                        Capability::ActivateRelease,
                        true,
                    )
                    .await?,
                )
            }
            None => {
                if plan.authorization_version != 0 {
                    return Err(PersistenceError::OperatorDenied);
                }
                None
            }
        };
        let current = lock_world(&mut tx, plan.world).await?;
        if current.pending != Some(plan.activation) || !current.maintenance {
            return Err(PersistenceError::InvalidTransition);
        }
        fence_release(&mut tx, plan.lease).await?;
        let row = lock_activation(&mut tx, plan.activation).await?;
        if row.state != "activating"
            || row.request != plan.request
            || row.target != plan.artifact.digest
            || row.previous != plan.previous
            || row.writer_incarnation != uuid_of(plan.lease.incarnation())
        {
            return Err(PersistenceError::InvalidTransition);
        }
        let known_good = match plan.kind {
            PublicationKind::Activate => plan.previous.unwrap_or(plan.artifact.digest),
            PublicationKind::Rollback => plan.artifact.digest,
        };
        let changed = sqlx::query(
            "UPDATE worldwright_worlds SET active_release_digest = $2, known_good_release_digest = $3, pending_activation_id = NULL, release_maintenance = FALSE, updated_at = clock_timestamp() WHERE world_id = $1 AND pending_activation_id = $4",
        )
        .bind(uuid_of(plan.world))
        .bind(plan.artifact.digest.as_bytes().as_slice())
        .bind(known_good.as_bytes().as_slice())
        .bind(uuid_of(plan.activation))
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() != 1 {
            return Err(PersistenceError::InvalidTransition);
        }
        update_one(
            sqlx::query(
                "UPDATE worldwright_release_activations SET state = 'succeeded', completed_at = clock_timestamp() WHERE activation_id = $1 AND state = 'activating'",
            )
            .bind(uuid_of(plan.activation))
            .execute(&mut *tx)
            .await?,
        )?;
        update_one(
            sqlx::query(
                "UPDATE worldwright_publication_requests SET status = 'activated', completed_at = clock_timestamp() WHERE request_id = $1 AND status = 'activating'",
            )
            .bind(uuid_of(plan.request))
            .execute(&mut *tx)
            .await?,
        )?;
        release_audit(
            &mut tx,
            plan.world,
            plan.request,
            plan.activation,
            plan.executor.as_str(),
            "activation_succeeded",
            plan.artifact.digest,
            plan.target,
            json!({"release_id": plan.artifact.release_id.as_str()}),
        )
        .await?;
        if let Some((context, operation)) = authorized {
            crate::operational::insert_audit(
                &mut tx,
                operation,
                context.correlation,
                context.claims.subject.as_str(),
                Some(operator_session(context)?),
                assurance(context)?,
                "activation_succeeded",
                context.environment,
                context.world,
                "release",
                &plan.request.to_string(),
                context.reason,
                "confirmed",
                authorization.ok_or(PersistenceError::InvalidData)?,
                Some(json!({"status": "activating"})),
                Some(
                    json!({"status": "activated", "release_id": plan.artifact.release_id.as_str()}),
                ),
            )
            .await?;
        }
        tx.commit()
            .await
            .map_err(|_| PersistenceError::UnknownOutcome)
    }

    pub async fn fail_activation(
        &self,
        plan: ActivationPlan,
        code: &str,
    ) -> Result<(), PersistenceError> {
        if plan.authorization_version != 0 {
            return Err(PersistenceError::OperatorDenied);
        }
        self.fail_activation_inner(plan, code, None).await
    }

    async fn fail_activation_inner(
        &self,
        plan: ActivationPlan,
        code: &str,
        authorized: Option<(&OperatorContext<'_>, OperationId)>,
    ) -> Result<(), PersistenceError> {
        if !matches!(
            code,
            "artifact_invalid" | "durable_incompatible" | "smoke_failed" | "operator_aborted"
        ) {
            return Err(PersistenceError::InvalidData);
        }
        let mut tx = self.pool().begin().await?;
        let authorization = match authorized {
            Some((context, _)) => {
                if plan.authorization_version != 1
                    || plan.executor != context.claims.subject
                    || plan.world != context.world
                    || target_environment(plan.target) != context.environment
                {
                    return Err(PersistenceError::OperatorDenied);
                }
                Some(
                    crate::operational::authorize_tx(
                        &mut tx,
                        context,
                        Capability::ActivateRelease,
                        true,
                    )
                    .await?,
                )
            }
            None => {
                if plan.authorization_version != 0 {
                    return Err(PersistenceError::OperatorDenied);
                }
                None
            }
        };
        let current = lock_world(&mut tx, plan.world).await?;
        if current.pending != Some(plan.activation) || !current.maintenance {
            return Err(PersistenceError::InvalidTransition);
        }
        fence_release(&mut tx, plan.lease).await?;
        let row = lock_activation(&mut tx, plan.activation).await?;
        if row.state != "activating"
            || row.request != plan.request
            || row.target != plan.artifact.digest
            || row.previous != plan.previous
            || row.writer_incarnation != uuid_of(plan.lease.incarnation())
        {
            return Err(PersistenceError::InvalidTransition);
        }
        update_one(
            sqlx::query(
                "UPDATE worldwright_worlds SET pending_activation_id = NULL, release_maintenance = FALSE, updated_at = clock_timestamp() WHERE world_id = $1 AND pending_activation_id = $2",
            )
            .bind(uuid_of(plan.world))
            .bind(uuid_of(plan.activation))
            .execute(&mut *tx)
            .await?,
        )?;
        update_one(
            sqlx::query(
                "UPDATE worldwright_release_activations SET state = 'failed', failure_code = $2, completed_at = clock_timestamp() WHERE activation_id = $1 AND state = 'activating'",
            )
            .bind(uuid_of(plan.activation))
            .bind(code)
            .execute(&mut *tx)
            .await?,
        )?;
        update_one(
            sqlx::query(
                "UPDATE worldwright_publication_requests SET status = 'failed', completed_at = clock_timestamp() WHERE request_id = $1 AND status = 'activating'",
            )
            .bind(uuid_of(plan.request))
            .execute(&mut *tx)
            .await?,
        )?;
        release_audit(
            &mut tx,
            plan.world,
            plan.request,
            plan.activation,
            plan.executor.as_str(),
            "activation_failed",
            plan.artifact.digest,
            plan.target,
            json!({"failure_code": code}),
        )
        .await?;
        if let Some((context, operation)) = authorized {
            crate::operational::insert_audit(
                &mut tx,
                operation,
                context.correlation,
                context.claims.subject.as_str(),
                Some(operator_session(context)?),
                assurance(context)?,
                "activation_failed",
                context.environment,
                context.world,
                "release",
                &plan.request.to_string(),
                context.reason,
                "failed",
                authorization.ok_or(PersistenceError::InvalidData)?,
                Some(json!({"status": "activating"})),
                Some(json!({"status": "failed", "failure_code": code})),
            )
            .await?;
        }
        tx.commit()
            .await
            .map_err(|_| PersistenceError::UnknownOutcome)
    }

    pub async fn recover_activation(
        &self,
        world: WorldId,
        new_writer: WriterId,
        executor: &str,
    ) -> Result<ActivationPlan, PersistenceError> {
        self.recover_activation_inner(world, new_writer, executor, None)
            .await
    }

    async fn recover_activation_inner(
        &self,
        world: WorldId,
        new_writer: WriterId,
        executor: &str,
        authorized: Option<(&OperatorContext<'_>, OperationId)>,
    ) -> Result<ActivationPlan, PersistenceError> {
        validate_actor(executor)?;
        let mut tx = self.pool().begin().await?;
        let authorization = if let Some((context, _)) = authorized {
            if context.world != world || context.claims.subject != executor {
                return Err(PersistenceError::OperatorDenied);
            }
            Some(
                crate::operational::authorize_tx(
                    &mut tx,
                    context,
                    Capability::InitiateRecovery,
                    true,
                )
                .await?,
            )
        } else {
            None
        };
        let current = lock_world(&mut tx, world).await?;
        let activation = current.pending.ok_or(PersistenceError::InvalidTransition)?;
        if !current.maintenance {
            return Err(PersistenceError::InvalidTransition);
        }
        let activation_row = lock_activation_for_recovery(&mut tx, activation).await?;
        if activation_row.3 != "activating" {
            return Err(PersistenceError::InvalidTransition);
        }
        let request_row: Option<(String, String, String, i16)> = sqlx::query_as(
            "SELECT kind, target_environment, status, authorization_version FROM worldwright_publication_requests WHERE request_id = $1 FOR UPDATE",
        )
        .bind(uuid_of(activation_row.0))
        .fetch_optional(&mut *tx)
        .await?;
        let Some((kind, target, status, authorization_version)) = request_row else {
            return Err(PersistenceError::NotFound);
        };
        if status != "activating" {
            return Err(PersistenceError::InvalidTransition);
        }
        let kind = PublicationKind::from_str(&kind).map_err(|_| PersistenceError::InvalidData)?;
        let target =
            DeploymentTarget::from_str(&target).map_err(|_| PersistenceError::InvalidData)?;
        match authorized {
            Some((context, _)) => {
                if authorization_version != 1 || context.environment != target_environment(target) {
                    return Err(PersistenceError::OperatorDenied);
                }
            }
            None => {
                if authorization_version != 0 || target != DeploymentTarget::Staging {
                    return Err(PersistenceError::ProductionGateRequired);
                }
            }
        }
        let writer_row: Option<(uuid::Uuid, i64)> = sqlx::query_as(
            "UPDATE worldwright_worlds SET writer_epoch = writer_epoch + 1, active_writer_id = $2, release_maintenance = TRUE, updated_at = clock_timestamp() WHERE world_id = $1 AND writer_epoch < 9223372036854775807 RETURNING authority_incarnation, writer_epoch",
        )
        .bind(uuid_of(world))
        .bind(uuid_of(new_writer))
        .fetch_optional(&mut *tx)
        .await?;
        let (incarnation, epoch) = writer_row
            .map(|row| {
                Ok::<_, PersistenceError>((
                    worldwright_core::ids::AuthorityIncarnationId::from_u128(row.0.as_u128())
                        .map_err(|_| PersistenceError::InvalidData)?,
                    u64_from_i64(row.1, true)?,
                ))
            })
            .transpose()?
            .ok_or(PersistenceError::CounterExhausted)?;
        update_one(
            sqlx::query(
                "UPDATE worldwright_release_activations SET writer_id = $2, writer_epoch = $3, writer_incarnation = $4 WHERE activation_id = $1 AND state = 'activating'",
            )
            .bind(uuid_of(activation))
            .bind(uuid_of(new_writer))
            .bind(i64_from_u64(epoch, true)?)
            .bind(uuid_of(incarnation))
            .execute(&mut *tx)
            .await?,
        )?;
        let request_update = if let Some((context, _)) = authorized {
            sqlx::query(
                "UPDATE worldwright_publication_requests SET activation_executor = $2, activation_session_id = $3, activation_correlation_id = $4 WHERE request_id = $1 AND status = 'activating' AND authorization_version = 1",
            )
            .bind(uuid_of(activation_row.0))
            .bind(executor)
            .bind(uuid_of(operator_session(context)?))
            .bind(uuid_of(context.correlation))
            .execute(&mut *tx)
            .await?
        } else {
            sqlx::query(
                "UPDATE worldwright_publication_requests SET activation_executor = $2 WHERE request_id = $1 AND status = 'activating' AND authorization_version = 0",
            )
            .bind(uuid_of(activation_row.0))
            .bind(executor)
            .execute(&mut *tx)
            .await?
        };
        update_one(request_update)?;
        let artifact_row: Option<(Vec<u8>, String)> = sqlx::query_as(
            "SELECT a.artifact_bytes, a.release_id FROM worldwright_content_artifacts a WHERE a.artifact_digest = $1",
        )
        .bind(activation_row.2.as_bytes().as_slice())
        .fetch_optional(&mut *tx)
        .await?;
        let Some((bytes, release_id)) = artifact_row else {
            return Err(PersistenceError::NotFound);
        };
        let verified = verify_bytes("stored release artifact", &bytes, Some(activation_row.2))
            .map_err(|_| PersistenceError::InvalidData)?;
        let release_id: ReleaseId = release_id
            .parse()
            .map_err(|_| PersistenceError::InvalidData)?;
        if release_id != *verified.manifest().release_id() {
            return Err(PersistenceError::InvalidData);
        }
        let artifact = StoredArtifact {
            bytes,
            digest: verified.artifact_digest(),
            release_id,
        };
        if let Some((context, operation)) = authorized {
            crate::operational::insert_audit(
                &mut tx,
                operation,
                context.correlation,
                context.claims.subject.as_str(),
                Some(operator_session(context)?),
                assurance(context)?,
                "activation_started",
                context.environment,
                context.world,
                "release",
                &activation_row.0.to_string(),
                context.reason,
                "started",
                authorization.ok_or(PersistenceError::InvalidData)?,
                Some(json!({"recovery": true})),
                Some(json!({"activation_id": activation.to_string(), "writer": new_writer.to_string()})),
            )
            .await?;
        }
        tx.commit()
            .await
            .map_err(|_| PersistenceError::UnknownOutcome)?;
        Ok(ActivationPlan {
            activation,
            request: activation_row.0,
            world,
            artifact,
            kind,
            target,
            previous: activation_row.1,
            executor: executor.to_owned(),
            authorization_version,
            lease: WriterLease {
                world,
                writer: new_writer,
                epoch,
                incarnation,
            },
        })
    }

    pub async fn active_artifact(
        &self,
        world: WorldId,
    ) -> Result<StoredArtifact, PersistenceError> {
        let state = self.release_state(world).await?;
        let digest = state.active.ok_or(PersistenceError::NotFound)?;
        if state.pending.is_some() || state.maintenance {
            return Err(PersistenceError::InvalidTransition);
        }
        let row: Option<(Vec<u8>, String)> = sqlx::query_as(
            "SELECT a.artifact_bytes, a.release_id FROM worldwright_content_artifacts a WHERE a.artifact_digest = $1",
        )
        .bind(digest.as_bytes().as_slice())
        .fetch_optional(self.pool())
        .await?;
        let Some((bytes, release_id)) = row else {
            return Err(PersistenceError::NotFound);
        };
        let verified = verify_bytes("stored release artifact", &bytes, Some(digest))
            .map_err(|_| PersistenceError::InvalidData)?;
        let release_id: ReleaseId = release_id
            .parse()
            .map_err(|_| PersistenceError::InvalidData)?;
        if release_id != *verified.manifest().release_id() {
            return Err(PersistenceError::InvalidData);
        }
        Ok(StoredArtifact {
            bytes,
            digest: verified.artifact_digest(),
            release_id,
        })
    }

    pub async fn durable_content_references(
        &self,
        world: WorldId,
    ) -> Result<DurableContentReferences, PersistenceError> {
        let rooms: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT room_definition FROM worldwright_characters WHERE world_id = $1 ORDER BY room_definition",
        )
        .bind(uuid_of(world))
        .fetch_all(self.pool())
        .await?;
        if rooms.len() > 1024 {
            return Err(PersistenceError::InvalidData);
        }
        let items: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT definition_id FROM worldwright_item_instances WHERE world_id = $1 ORDER BY definition_id",
        )
        .bind(uuid_of(world))
        .fetch_all(self.pool())
        .await?;
        if items.len() > 1024 {
            return Err(PersistenceError::InvalidData);
        }
        Ok(DurableContentReferences {
            room_definitions: rooms,
            item_definitions: items,
        })
    }
}

async fn publication_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    request: PublicationId,
) -> Result<PublicationRecord, PersistenceError> {
    let row: Option<PublicationSqlRow> = sqlx::query_as(
        "SELECT r.world_id, r.artifact_digest, a.payload_digest, a.release_id, r.kind, r.target_environment, r.expected_current_digest, r.requested_by, r.status, r.reviewed_by, r.review_reason, r.activation_executor, r.studio_review_context FROM worldwright_publication_requests r JOIN worldwright_content_artifacts a ON a.artifact_digest = r.artifact_digest WHERE r.request_id = $1",
    )
    .bind(uuid_of(request))
    .fetch_optional(&mut **tx)
    .await?;
    let Some((
        world_uuid,
        digest_bytes,
        payload_digest,
        release_id,
        kind,
        target,
        expected,
        requested_by,
        status,
        reviewed_by,
        review_reason,
        activation_executor,
        review_context,
    )) = row
    else {
        return Err(PersistenceError::NotFound);
    };
    validate_actor(&requested_by)?;
    if let Some(value) = reviewed_by.as_deref() {
        validate_actor(value)?;
    }
    if let Some(value) = review_reason.as_deref() {
        validate_reason(value)?;
    }
    if let Some(value) = activation_executor.as_deref() {
        validate_actor(value)?;
    }
    let review_context = review_context
        .map(|value| {
            serde_json::from_value::<StudioReviewContext>(value)
                .map_err(|_| PersistenceError::InvalidData)
        })
        .transpose()?;
    if let Some(context) = review_context.as_ref() {
        let artifact_digest =
            digest(Some(digest_bytes.clone()))?.ok_or(PersistenceError::InvalidData)?;
        let mut snapshot = [0u8; 32];
        if payload_digest.len() != 32 {
            return Err(PersistenceError::InvalidData);
        }
        snapshot.copy_from_slice(&payload_digest);
        if context.snapshot_sha256 != worldwright_operations::encode_hex(&snapshot)
            || context.preview_artifact_sha256 != artifact_digest.to_string()
        {
            return Err(PersistenceError::InvalidData);
        }
    }
    Ok(PublicationRecord {
        request,
        world: world_id(world_uuid)?,
        artifact_digest: digest(Some(digest_bytes))?.ok_or(PersistenceError::InvalidData)?,
        release_id: release_id
            .parse()
            .map_err(|_| PersistenceError::InvalidData)?,
        kind: kind.parse().map_err(|_| PersistenceError::InvalidData)?,
        target: target.parse().map_err(|_| PersistenceError::InvalidData)?,
        expected_current: digest(expected)?,
        requested_by,
        status: status.parse().map_err(|_| PersistenceError::InvalidData)?,
        reviewed_by,
        review_reason,
        activation_executor,
        review_context,
    })
}

async fn lock_world(
    tx: &mut Transaction<'_, Postgres>,
    world: WorldId,
) -> Result<ReleaseWorldRow, PersistenceError> {
    let row: Option<ReleaseWorldSqlRow> =
        sqlx::query_as(
            "SELECT active_release_digest, known_good_release_digest, pending_activation_id, release_maintenance FROM worldwright_worlds WHERE world_id = $1 FOR UPDATE",
        )
        .bind(uuid_of(world))
        .fetch_optional(&mut **tx)
        .await?;
    let Some((active, known_good, pending, maintenance)) = row else {
        return Err(PersistenceError::NotFound);
    };
    Ok(ReleaseWorldRow {
        active: digest(active)?,
        known_good: digest(known_good)?,
        pending: pending.map(activation_id).transpose()?,
        maintenance,
    })
}

async fn fence_release(
    tx: &mut Transaction<'_, Postgres>,
    lease: WriterLease,
) -> Result<(), PersistenceError> {
    let row: Option<(i64, Option<uuid::Uuid>, uuid::Uuid)> = sqlx::query_as(
        "SELECT writer_epoch, active_writer_id, authority_incarnation FROM worldwright_worlds WHERE world_id = $1 FOR UPDATE",
    )
    .bind(uuid_of(lease.world()))
    .fetch_optional(&mut **tx)
    .await?;
    let Some((epoch, writer, incarnation)) = row else {
        return Err(PersistenceError::StaleWriter);
    };
    if u64_from_i64(epoch, true)? != lease.epoch()
        || writer != Some(uuid_of(lease.writer()))
        || incarnation != uuid_of(lease.incarnation())
    {
        return Err(PersistenceError::StaleWriter);
    }
    Ok(())
}

async fn lock_activation(
    tx: &mut Transaction<'_, Postgres>,
    activation: ActivationId,
) -> Result<ActivationRow, PersistenceError> {
    let row: Option<ActivationSqlRow> = sqlx::query_as(
        "SELECT state, request_id, previous_release_digest, target_release_digest, writer_incarnation FROM worldwright_release_activations WHERE activation_id = $1 FOR UPDATE",
    )
    .bind(uuid_of(activation))
    .fetch_optional(&mut **tx)
    .await?;
    let Some((state, request, previous, target, writer_incarnation)) = row else {
        return Err(PersistenceError::NotFound);
    };
    Ok(ActivationRow {
        state,
        request: publication_id(request)?,
        previous: digest(previous)?,
        target: digest(Some(target))?.ok_or(PersistenceError::InvalidData)?,
        writer_incarnation,
    })
}

async fn lock_activation_for_recovery(
    tx: &mut Transaction<'_, Postgres>,
    activation: ActivationId,
) -> Result<
    (
        PublicationId,
        Option<Sha256Digest>,
        Sha256Digest,
        String,
        uuid::Uuid,
    ),
    PersistenceError,
> {
    let row: Option<RecoveryActivationSqlRow> = sqlx::query_as(
        "SELECT request_id, target_release_digest, previous_release_digest, state, writer_incarnation FROM worldwright_release_activations WHERE activation_id = $1 FOR UPDATE",
    )
    .bind(uuid_of(activation))
    .fetch_optional(&mut **tx)
    .await?;
    let Some((request, target, previous, state, writer_incarnation)) = row else {
        return Err(PersistenceError::NotFound);
    };
    Ok((
        publication_id(request)?,
        digest(previous)?,
        digest(Some(target))?.ok_or(PersistenceError::InvalidData)?,
        state,
        writer_incarnation,
    ))
}

#[allow(clippy::too_many_arguments)]
async fn release_audit(
    tx: &mut Transaction<'_, Postgres>,
    world: WorldId,
    request: PublicationId,
    activation: ActivationId,
    actor: &str,
    action: &str,
    digest: Sha256Digest,
    target: DeploymentTarget,
    summary: Value,
) -> Result<(), PersistenceError> {
    sqlx::query(
        "INSERT INTO worldwright_release_audit (world_id, request_id, activation_id, actor, action, artifact_digest, target_environment, summary) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
    )
    .bind(uuid_of(world))
    .bind(uuid_of(request))
    .bind(uuid_of(activation))
    .bind(actor)
    .bind(action)
    .bind(digest.as_bytes().as_slice())
    .bind(target.to_string())
    .bind(summary)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn update_one(result: sqlx::postgres::PgQueryResult) -> Result<(), PersistenceError> {
    if result.rows_affected() == 1 {
        Ok(())
    } else {
        Err(PersistenceError::InvalidTransition)
    }
}

fn digest(value: Option<Vec<u8>>) -> Result<Option<Sha256Digest>, PersistenceError> {
    value
        .map(|bytes| {
            if bytes.len() != 32 {
                return Err(PersistenceError::InvalidData);
            }
            let mut array = [0u8; 32];
            array.copy_from_slice(&bytes);
            Ok(Sha256Digest::from_digest_bytes(array))
        })
        .transpose()
}

fn validate_actor(value: &str) -> Result<(), PersistenceError> {
    validate_text(value, MAX_RELEASE_TEXT_BYTES)
}

fn validate_reason(value: &str) -> Result<(), PersistenceError> {
    validate_text(value, MAX_RELEASE_REASON_BYTES)
}

fn validate_text(value: &str, maximum: usize) -> Result<(), PersistenceError> {
    if value.is_empty()
        || value.len() > maximum
        || value.trim() != value
        || value.chars().any(|character| {
            character.is_control()
                || matches!(
                    character as u32,
                    0x061c | 0x200e | 0x200f | 0x2028..=0x202e | 0x2066..=0x2069
                )
        })
    {
        return Err(PersistenceError::InvalidData);
    }
    Ok(())
}

fn world_id(value: uuid::Uuid) -> Result<WorldId, PersistenceError> {
    WorldId::from_u128(value.as_u128()).map_err(|_| PersistenceError::InvalidData)
}

fn publication_id(value: uuid::Uuid) -> Result<PublicationId, PersistenceError> {
    PublicationId::from_u128(value.as_u128()).map_err(|_| PersistenceError::InvalidData)
}

fn activation_id(value: uuid::Uuid) -> Result<ActivationId, PersistenceError> {
    ActivationId::from_u128(value.as_u128()).map_err(|_| PersistenceError::InvalidData)
}
