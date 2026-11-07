pub(crate) const AUTHORIZE_OPERATOR: &str = "
SELECT
    o.kind,
    o.enabled,
    o.tokens_valid_after <= to_timestamp($2::DOUBLE PRECISION) AS issued_after_revocation,
    NOT EXISTS (
        SELECT 1
        FROM worldwright_operator_revoked_sessions AS revoked
        WHERE revoked.session_id = $3
          AND revoked.subject = o.subject
    ) AS session_active,
    EXISTS (
        SELECT 1
        FROM worldwright_operator_grants AS grant
        WHERE grant.subject = o.subject
          AND grant.capability = $4
          AND grant.target_environment = $5
          AND grant.world_id = $6
          AND grant.revoked_at IS NULL
          AND (grant.expires_at IS NULL OR grant.expires_at > clock_timestamp())
    ) AS capability_active
FROM worldwright_operators AS o
WHERE o.subject = $1
FOR SHARE
";

pub(crate) const AUTHORIZE_OPERATOR_FOR_UPDATE: &str = "
SELECT
    o.kind,
    o.enabled,
    o.tokens_valid_after <= to_timestamp($2::DOUBLE PRECISION) AS issued_after_revocation,
    NOT EXISTS (
        SELECT 1
        FROM worldwright_operator_revoked_sessions AS revoked
        WHERE revoked.session_id = $3
          AND revoked.subject = o.subject
    ) AS session_active,
    EXISTS (
        SELECT 1
        FROM worldwright_operator_grants AS grant
        WHERE grant.subject = o.subject
          AND grant.capability = $4
          AND grant.target_environment = $5
          AND grant.world_id = $6
          AND grant.revoked_at IS NULL
          AND (grant.expires_at IS NULL OR grant.expires_at > clock_timestamp())
    ) AS capability_active
FROM worldwright_operators AS o
WHERE o.subject = $1
FOR UPDATE
";

pub(crate) const BOOTSTRAP_OPERATOR_COUNT: &str =
    "SELECT COUNT(*)::BIGINT FROM worldwright_operators";
pub(crate) const LOCK_OPERATOR_BOOTSTRAP: &str =
    "LOCK TABLE worldwright_operators IN EXCLUSIVE MODE";

pub(crate) const INSERT_OPERATOR: &str = "
INSERT INTO worldwright_operators (subject, display_name, kind)
VALUES ($1, $2, $3)
";

pub(crate) const INSERT_GRANT: &str = "
INSERT INTO worldwright_operator_grants (
    grant_id,
    subject,
    capability,
    target_environment,
    world_id,
    granted_by,
    reason,
    expires_at
)
VALUES (
    $1,
    $2,
    $3,
    $4,
    $5,
    $6,
    $7,
    CASE
        WHEN $8::BIGINT IS NULL THEN NULL
        ELSE clock_timestamp() + ($8::DOUBLE PRECISION * INTERVAL '1 second')
    END
)
";

#[allow(dead_code)]
pub(crate) const REVOKE_GRANT: &str = "
UPDATE worldwright_operator_grants
SET revoked_at = clock_timestamp(),
    revoked_by = $2,
    revocation_reason = $3
WHERE grant_id = $1
  AND revoked_at IS NULL
RETURNING subject, capability, target_environment, world_id
";

#[allow(dead_code)]
pub(crate) const OPERATOR_FOR_UPDATE: &str = "
SELECT kind, enabled
FROM worldwright_operators
WHERE subject = $1
FOR UPDATE
";

pub(crate) const SET_OPERATOR_ENABLED: &str = "
UPDATE worldwright_operators
SET enabled = $2,
    tokens_valid_after = CASE WHEN $2 THEN clock_timestamp() ELSE tokens_valid_after END,
    updated_at = clock_timestamp()
WHERE subject = $1
";

pub(crate) const REVOKE_OPERATOR_SESSION: &str = "
INSERT INTO worldwright_operator_revoked_sessions (
    session_id,
    subject,
    revoked_by,
    reason
)
VALUES ($1, $2, $3, $4)
ON CONFLICT (session_id) DO NOTHING
";

pub(crate) const ACCOUNT_FOR_UPDATE: &str = "
SELECT username, disabled
FROM worldwright_accounts
WHERE account_id = $1
FOR UPDATE
";

pub(crate) const SET_ACCOUNT_DISABLED: &str = "
UPDATE worldwright_accounts
SET disabled = $2,
    disabled_at = CASE WHEN $2 THEN clock_timestamp() ELSE NULL END,
    disabled_reason = CASE WHEN $2 THEN $3 ELSE NULL END,
    updated_at = clock_timestamp()
WHERE account_id = $1
RETURNING username, disabled
";

pub(crate) const REVOKE_ACCOUNT_SESSIONS: &str = "
UPDATE worldwright_sessions
SET revoked_at = COALESCE(revoked_at, clock_timestamp())
WHERE account_id = $1
  AND revoked_at IS NULL
";

pub(crate) const INSERT_OPERATOR_AUDIT: &str = "
INSERT INTO worldwright_operator_audit (
    operation_id,
    correlation_id,
    actor_subject,
    actor_session_id,
    assurance,
    action,
    target_environment,
    world_id,
    target_type,
    target_id,
    reason,
    outcome,
    authorization,
    before_state,
    after_state
)
VALUES (
    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15
)
";

pub(crate) const INSERT_SECURITY_EVENT: &str = "
INSERT INTO worldwright_operator_security_events (
    correlation_id,
    actor_subject,
    code,
    target_environment,
    world_id,
    summary
)
VALUES ($1, $2, $3, $4, $5, $6)
";

pub(crate) const OPERATOR_STATUS: &str = "
SELECT
    world.active_release_digest,
    world.writer_epoch,
    world.authority_incarnation,
    (
        SELECT COUNT(*)::BIGINT
        FROM worldwright_outbox AS outbox
        WHERE outbox.world_id = world.world_id
          AND outbox.delivered_at IS NULL
    ) AS outbox_backlog,
    (
        SELECT GREATEST(
            0,
            EXTRACT(EPOCH FROM (clock_timestamp() - MIN(outbox.created_at)))::BIGINT
        )
        FROM worldwright_outbox AS outbox
        WHERE outbox.world_id = world.world_id
          AND outbox.delivered_at IS NULL
    ) AS outbox_oldest_seconds
FROM worldwright_worlds AS world
WHERE world.world_id = $1
";

pub(crate) const INSERT_RECOVERY_JOB: &str = "
INSERT INTO worldwright_recovery_jobs (
    job_id,
    world_id,
    kind,
    state,
    target_environment,
    actor_subject,
    actor_session_id,
    correlation_id,
    reason,
    source_incarnation,
    active_release_digest,
    summary
)
SELECT
    $1,
    world_id,
    $2,
    'running',
    $3,
    $4,
    $5,
    $6,
    $7,
    authority_incarnation,
    active_release_digest,
    $8
FROM worldwright_worlds
WHERE world_id = $9
RETURNING source_incarnation, active_release_digest
";

pub(crate) const COMPLETE_RECOVERY_JOB: &str = "
UPDATE worldwright_recovery_jobs
SET state = $2,
    backup_digest = $3,
    manifest_digest = $4,
    target_incarnation = $5,
    completed_at = clock_timestamp(),
    summary = $6
WHERE job_id = $1
  AND state = 'running'
  AND kind = $7
  AND actor_subject = $8
  AND target_environment = $9
  AND world_id = $10
";

pub(crate) const ROTATE_AUTHORITY: &str = "
UPDATE worldwright_worlds
SET authority_incarnation = gen_random_uuid(),
    writer_epoch = 1,
    active_writer_id = $2,
    updated_at = clock_timestamp()
WHERE world_id = $1
  AND pending_activation_id IS NULL
  AND release_maintenance = FALSE
RETURNING authority_incarnation, writer_epoch
";
