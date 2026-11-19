ALTER TABLE worldwright_worlds
    ADD COLUMN authority_incarnation UUID NOT NULL DEFAULT gen_random_uuid();

ALTER TABLE worldwright_worlds
    ADD CONSTRAINT worldwright_authority_incarnation_nonzero CHECK (
        authority_incarnation <> '00000000-0000-0000-0000-000000000000'::UUID
    ),
    ADD CONSTRAINT worldwright_world_authority_key UNIQUE (world_id, authority_incarnation);

ALTER TABLE worldwright_release_activations
    ADD COLUMN writer_incarnation UUID;

UPDATE worldwright_release_activations AS activation
SET writer_incarnation = world.authority_incarnation
FROM worldwright_worlds AS world
WHERE world.world_id = activation.world_id;

ALTER TABLE worldwright_release_activations
    ALTER COLUMN writer_incarnation SET NOT NULL,
    ADD CONSTRAINT worldwright_activation_incarnation_nonzero CHECK (
        writer_incarnation <> '00000000-0000-0000-0000-000000000000'::UUID
    );

ALTER TABLE worldwright_accounts
    ADD COLUMN disabled_at TIMESTAMPTZ,
    ADD COLUMN disabled_reason TEXT;

UPDATE worldwright_accounts
SET disabled_at = clock_timestamp(),
    disabled_reason = 'legacy_disabled_account'
WHERE disabled = TRUE;

ALTER TABLE worldwright_accounts
    ADD CONSTRAINT worldwright_account_disabled_shape CHECK (
        (disabled = FALSE AND disabled_at IS NULL AND disabled_reason IS NULL)
        OR (
            disabled = TRUE
            AND disabled_at IS NOT NULL
            AND disabled_reason IS NOT NULL
            AND octet_length(disabled_reason) BETWEEN 1 AND 512
            AND btrim(disabled_reason) = disabled_reason
            AND disabled_reason !~ '[[:cntrl:]]'
        )
    );

CREATE TABLE worldwright_operators (
    subject TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    kind TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    tokens_valid_after TIMESTAMPTZ NOT NULL DEFAULT 'epoch'::TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT worldwright_operator_subject_valid CHECK (
        octet_length(subject) BETWEEN 1 AND 128
        AND subject ~ '^[a-z][a-z0-9._-]*$'
    ),
    CONSTRAINT worldwright_operator_display_name_valid CHECK (
        octet_length(display_name) BETWEEN 1 AND 128
        AND btrim(display_name) = display_name
        AND display_name !~ '[[:cntrl:]]'
    ),
    CONSTRAINT worldwright_operator_kind_valid CHECK (
        kind IN ('human', 'service', 'break_glass')
    )
);

CREATE TABLE worldwright_operator_grants (
    grant_id UUID PRIMARY KEY,
    subject TEXT NOT NULL REFERENCES worldwright_operators(subject) ON DELETE RESTRICT,
    capability TEXT NOT NULL,
    target_environment TEXT NOT NULL,
    world_id UUID NOT NULL REFERENCES worldwright_worlds(world_id) ON DELETE RESTRICT,
    granted_by TEXT NOT NULL,
    reason TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    expires_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    revoked_by TEXT,
    revocation_reason TEXT,
    CONSTRAINT worldwright_operator_grant_id_nonzero CHECK (
        grant_id <> '00000000-0000-0000-0000-000000000000'::UUID
    ),
    CONSTRAINT worldwright_operator_capability_valid CHECK (
        capability IN (
            'view_status',
            'moderate_account',
            'request_release',
            'review_release',
            'activate_release',
            'drain_world',
            'initiate_recovery',
            'manage_operators',
            'backup_world',
            'restore_world'
        )
    ),
    CONSTRAINT worldwright_operator_grant_environment_valid CHECK (
        target_environment IN ('staging', 'production')
    ),
    CONSTRAINT worldwright_operator_granted_by_valid CHECK (
        octet_length(granted_by) BETWEEN 1 AND 128
        AND btrim(granted_by) = granted_by
        AND granted_by !~ '[[:cntrl:]]'
    ),
    CONSTRAINT worldwright_operator_grant_reason_valid CHECK (
        octet_length(reason) BETWEEN 1 AND 512
        AND btrim(reason) = reason
        AND reason !~ '[[:cntrl:]]'
    ),
    CONSTRAINT worldwright_operator_grant_expiry_valid CHECK (
        expires_at IS NULL OR expires_at > created_at
    ),
    CONSTRAINT worldwright_operator_grant_revocation_shape CHECK (
        (
            revoked_at IS NULL
            AND revoked_by IS NULL
            AND revocation_reason IS NULL
        )
        OR (
            revoked_at IS NOT NULL
            AND revoked_at >= created_at
            AND revoked_by IS NOT NULL
            AND octet_length(revoked_by) BETWEEN 1 AND 128
            AND btrim(revoked_by) = revoked_by
            AND revoked_by !~ '[[:cntrl:]]'
            AND revocation_reason IS NOT NULL
            AND octet_length(revocation_reason) BETWEEN 1 AND 512
            AND btrim(revocation_reason) = revocation_reason
            AND revocation_reason !~ '[[:cntrl:]]'
        )
    )
);

CREATE UNIQUE INDEX worldwright_operator_active_grant
    ON worldwright_operator_grants (
        subject,
        capability,
        target_environment,
        world_id
    )
    WHERE revoked_at IS NULL;

CREATE INDEX worldwright_operator_grants_by_scope
    ON worldwright_operator_grants (
        target_environment,
        world_id,
        capability,
        subject
    )
    WHERE revoked_at IS NULL;

CREATE TABLE worldwright_operator_revoked_sessions (
    session_id UUID PRIMARY KEY,
    subject TEXT NOT NULL REFERENCES worldwright_operators(subject) ON DELETE RESTRICT,
    revoked_by TEXT NOT NULL,
    reason TEXT NOT NULL,
    revoked_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT worldwright_revoked_operator_session_nonzero CHECK (
        session_id <> '00000000-0000-0000-0000-000000000000'::UUID
    ),
    CONSTRAINT worldwright_operator_session_revoker_valid CHECK (
        octet_length(revoked_by) BETWEEN 1 AND 128
        AND btrim(revoked_by) = revoked_by
        AND revoked_by !~ '[[:cntrl:]]'
    ),
    CONSTRAINT worldwright_operator_session_reason_valid CHECK (
        octet_length(reason) BETWEEN 1 AND 512
        AND btrim(reason) = reason
        AND reason !~ '[[:cntrl:]]'
    )
);

CREATE TABLE worldwright_operator_audit (
    audit_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    operation_id UUID NOT NULL UNIQUE,
    correlation_id UUID NOT NULL,
    actor_subject TEXT NOT NULL,
    actor_session_id UUID,
    assurance TEXT NOT NULL,
    action TEXT NOT NULL,
    target_environment TEXT NOT NULL,
    world_id UUID NOT NULL REFERENCES worldwright_worlds(world_id) ON DELETE RESTRICT,
    target_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    reason TEXT NOT NULL,
    outcome TEXT NOT NULL,
    authorization JSONB NOT NULL,
    before_state JSONB,
    after_state JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT worldwright_operator_audit_operation_nonzero CHECK (
        operation_id <> '00000000-0000-0000-0000-000000000000'::UUID
    ),
    CONSTRAINT worldwright_operator_audit_correlation_nonzero CHECK (
        correlation_id <> '00000000-0000-0000-0000-000000000000'::UUID
    ),
    CONSTRAINT worldwright_operator_audit_session_nonzero CHECK (
        actor_session_id IS NULL
        OR actor_session_id <> '00000000-0000-0000-0000-000000000000'::UUID
    ),
    CONSTRAINT worldwright_operator_audit_actor_valid CHECK (
        octet_length(actor_subject) BETWEEN 1 AND 128
        AND btrim(actor_subject) = actor_subject
        AND actor_subject !~ '[[:cntrl:]]'
    ),
    CONSTRAINT worldwright_operator_audit_assurance_valid CHECK (
        assurance IN ('bootstrap', 'mfa', 'service', 'break_glass')
    ),
        CONSTRAINT worldwright_operator_audit_action_valid CHECK (
        action IN (
            'operator_created',
            'operator_bootstrapped',
            'operator_enabled',
            'operator_disabled',
            'grant_created',
            'grant_revoked',
            'operator_session_revoked',
            'account_suspended',
            'account_reinstated',
            'release_requested',
            'release_approved',
            'release_rejected',
            'activation_started',
            'activation_succeeded',
            'activation_failed',
            'drain_started',
            'drain_completed',
            'backup_started',
            'backup_succeeded',
            'backup_failed',
            'restore_started',
            'restore_verified',
            'restore_failed',
            'takeover_completed'
        )
    ),
    CONSTRAINT worldwright_operator_audit_environment_valid CHECK (
        target_environment IN ('staging', 'production')
    ),
    CONSTRAINT worldwright_operator_audit_target_type_valid CHECK (
        target_type IN (
            'operator',
            'grant',
            'session',
            'account',
            'release',
            'world',
            'backup',
            'restore'
        )
    ),
    CONSTRAINT worldwright_operator_audit_target_id_valid CHECK (
        octet_length(target_id) BETWEEN 1 AND 192
        AND btrim(target_id) = target_id
        AND target_id !~ '[[:cntrl:]]'
    ),
    CONSTRAINT worldwright_operator_audit_reason_valid CHECK (
        octet_length(reason) BETWEEN 1 AND 512
        AND btrim(reason) = reason
        AND reason !~ '[[:cntrl:]]'
    ),
    CONSTRAINT worldwright_operator_audit_outcome_valid CHECK (
        outcome IN ('started', 'confirmed', 'failed')
    ),
    CONSTRAINT worldwright_operator_audit_authorization_valid CHECK (
        jsonb_typeof(authorization) = 'object'
        AND octet_length(authorization::TEXT) <= 2048
    ),
    CONSTRAINT worldwright_operator_audit_before_valid CHECK (
        before_state IS NULL
        OR (
            jsonb_typeof(before_state) = 'object'
            AND octet_length(before_state::TEXT) <= 4096
        )
    ),
    CONSTRAINT worldwright_operator_audit_after_valid CHECK (
        after_state IS NULL
        OR (
            jsonb_typeof(after_state) = 'object'
            AND octet_length(after_state::TEXT) <= 4096
        )
    ),
    CONSTRAINT worldwright_operator_audit_bootstrap_shape CHECK (
        (action = 'operator_bootstrapped' AND actor_session_id IS NULL AND assurance = 'bootstrap')
        OR (action <> 'operator_bootstrapped' AND actor_session_id IS NOT NULL AND assurance <> 'bootstrap')
    )
);

CREATE INDEX worldwright_operator_audit_by_world_time
    ON worldwright_operator_audit (world_id, created_at, audit_id);

CREATE INDEX worldwright_operator_audit_by_actor_time
    ON worldwright_operator_audit (actor_subject, created_at, audit_id);

CREATE TABLE worldwright_operator_security_events (
    event_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    correlation_id UUID NOT NULL,
    actor_subject TEXT,
    code TEXT NOT NULL,
    target_environment TEXT NOT NULL,
    world_id UUID NOT NULL REFERENCES worldwright_worlds(world_id) ON DELETE RESTRICT,
    summary JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT worldwright_operator_security_correlation_nonzero CHECK (
        correlation_id <> '00000000-0000-0000-0000-000000000000'::UUID
    ),
    CONSTRAINT worldwright_operator_security_actor_valid CHECK (
        actor_subject IS NULL
        OR (
            octet_length(actor_subject) BETWEEN 1 AND 128
            AND btrim(actor_subject) = actor_subject
            AND actor_subject !~ '[[:cntrl:]]'
        )
    ),
    CONSTRAINT worldwright_operator_security_code_valid CHECK (
        code IN (
            'missing_token',
            'invalid_token',
            'expired_token',
            'revoked_token',
            'operator_disabled',
            'capability_denied',
            'scope_denied',
            'assurance_denied',
            'invalid_request'
        )
    ),
    CONSTRAINT worldwright_operator_security_environment_valid CHECK (
        target_environment IN ('staging', 'production')
    ),
    CONSTRAINT worldwright_operator_security_summary_valid CHECK (
        jsonb_typeof(summary) = 'object'
        AND octet_length(summary::TEXT) <= 2048
    )
);

CREATE INDEX worldwright_operator_security_by_time
    ON worldwright_operator_security_events (created_at, event_id);

CREATE TABLE worldwright_recovery_jobs (
    job_id UUID PRIMARY KEY,
    world_id UUID NOT NULL REFERENCES worldwright_worlds(world_id) ON DELETE RESTRICT,
    kind TEXT NOT NULL,
    state TEXT NOT NULL,
    target_environment TEXT NOT NULL,
    actor_subject TEXT NOT NULL,
    actor_session_id UUID NOT NULL,
    correlation_id UUID NOT NULL,
    reason TEXT NOT NULL,
    backup_digest BYTEA,
    manifest_digest BYTEA,
    source_incarnation UUID,
    target_incarnation UUID,
    active_release_digest BYTEA,
    started_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    completed_at TIMESTAMPTZ,
    summary JSONB NOT NULL DEFAULT '{}'::JSONB,
    CONSTRAINT worldwright_recovery_job_nonzero CHECK (
        job_id <> '00000000-0000-0000-0000-000000000000'::UUID
    ),
    CONSTRAINT worldwright_recovery_session_nonzero CHECK (
        actor_session_id <> '00000000-0000-0000-0000-000000000000'::UUID
    ),
    CONSTRAINT worldwright_recovery_correlation_nonzero CHECK (
        correlation_id <> '00000000-0000-0000-0000-000000000000'::UUID
    ),
    CONSTRAINT worldwright_recovery_kind_valid CHECK (
        kind IN ('backup', 'restore')
    ),
    CONSTRAINT worldwright_recovery_state_valid CHECK (
        state IN ('running', 'succeeded', 'failed')
    ),
    CONSTRAINT worldwright_recovery_environment_valid CHECK (
        target_environment IN ('staging', 'production')
    ),
    CONSTRAINT worldwright_recovery_actor_valid CHECK (
        octet_length(actor_subject) BETWEEN 1 AND 128
        AND btrim(actor_subject) = actor_subject
        AND actor_subject !~ '[[:cntrl:]]'
    ),
    CONSTRAINT worldwright_recovery_reason_valid CHECK (
        octet_length(reason) BETWEEN 1 AND 512
        AND btrim(reason) = reason
        AND reason !~ '[[:cntrl:]]'
    ),
    CONSTRAINT worldwright_recovery_digest_lengths CHECK (
        (backup_digest IS NULL OR octet_length(backup_digest) = 32)
        AND (manifest_digest IS NULL OR octet_length(manifest_digest) = 32)
        AND (active_release_digest IS NULL OR octet_length(active_release_digest) = 32)
    ),
    CONSTRAINT worldwright_recovery_incarnations_nonzero CHECK (
        (source_incarnation IS NULL OR source_incarnation <> '00000000-0000-0000-0000-000000000000'::UUID)
        AND (target_incarnation IS NULL OR target_incarnation <> '00000000-0000-0000-0000-000000000000'::UUID)
    ),
    CONSTRAINT worldwright_recovery_completion_shape CHECK (
        (state = 'running' AND completed_at IS NULL)
        OR (state IN ('succeeded', 'failed') AND completed_at IS NOT NULL)
    ),
    CONSTRAINT worldwright_recovery_summary_valid CHECK (
        jsonb_typeof(summary) = 'object'
        AND octet_length(summary::TEXT) <= 4096
    )
);

CREATE INDEX worldwright_recovery_jobs_by_world_time
    ON worldwright_recovery_jobs (world_id, started_at, job_id);

ALTER TABLE worldwright_publication_requests
    ADD COLUMN authorization_version SMALLINT NOT NULL DEFAULT 0,
    ADD COLUMN requested_session_id UUID,
    ADD COLUMN request_reason TEXT,
    ADD COLUMN request_correlation_id UUID,
    ADD COLUMN reviewed_session_id UUID,
    ADD COLUMN review_correlation_id UUID,
    ADD COLUMN activation_session_id UUID,
    ADD COLUMN activation_correlation_id UUID;

ALTER TABLE worldwright_publication_requests
    ADD CONSTRAINT worldwright_publication_authorization_version_valid CHECK (
        authorization_version IN (0, 1)
    ),
    ADD CONSTRAINT worldwright_publication_authorization_shape CHECK (
        (
            authorization_version = 0
            AND requested_session_id IS NULL
            AND request_reason IS NULL
            AND request_correlation_id IS NULL
            AND reviewed_session_id IS NULL
            AND review_correlation_id IS NULL
            AND activation_session_id IS NULL
            AND activation_correlation_id IS NULL
        )
        OR (
            authorization_version = 1
            AND requested_session_id IS NOT NULL
            AND request_reason IS NOT NULL
            AND request_correlation_id IS NOT NULL
        )
    ),
    ADD CONSTRAINT worldwright_publication_request_reason_valid CHECK (
        request_reason IS NULL
        OR (
            octet_length(request_reason) BETWEEN 1 AND 512
            AND btrim(request_reason) = request_reason
            AND request_reason !~ '[[:cntrl:]]'
        )
    ),
    ADD CONSTRAINT worldwright_publication_operator_ids_nonzero CHECK (
        (
            requested_session_id IS NULL
            OR requested_session_id <> '00000000-0000-0000-0000-000000000000'::UUID
        )
        AND (
            request_correlation_id IS NULL
            OR request_correlation_id <> '00000000-0000-0000-0000-000000000000'::UUID
        )
        AND (
            reviewed_session_id IS NULL
            OR reviewed_session_id <> '00000000-0000-0000-0000-000000000000'::UUID
        )
        AND (
            review_correlation_id IS NULL
            OR review_correlation_id <> '00000000-0000-0000-0000-000000000000'::UUID
        )
        AND (
            activation_session_id IS NULL
            OR activation_session_id <> '00000000-0000-0000-0000-000000000000'::UUID
        )
        AND (
            activation_correlation_id IS NULL
            OR activation_correlation_id <> '00000000-0000-0000-0000-000000000000'::UUID
        )
    );
