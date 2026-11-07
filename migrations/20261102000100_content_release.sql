CREATE TABLE worldwright_content_artifacts (
    artifact_digest BYTEA PRIMARY KEY,
    release_id TEXT NOT NULL UNIQUE,
    artifact_bytes BYTEA NOT NULL,
    manifest JSONB NOT NULL,
    payload_digest BYTEA NOT NULL,
    content_schema_version BIGINT NOT NULL,
    game_id TEXT NOT NULL,
    game_contract_version BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT worldwright_artifact_digest_length CHECK (octet_length(artifact_digest) = 32),
    CONSTRAINT worldwright_release_id_valid CHECK (octet_length(release_id) BETWEEN 1 AND 64 AND release_id ~ '^[a-z][a-z0-9._-]*$'),
    CONSTRAINT worldwright_artifact_bytes_bounded CHECK (octet_length(artifact_bytes) BETWEEN 1 AND 33600000),
    CONSTRAINT worldwright_artifact_manifest_object CHECK (jsonb_typeof(manifest) = 'object'),
    CONSTRAINT worldwright_artifact_payload_digest_length CHECK (octet_length(payload_digest) = 32),
    CONSTRAINT worldwright_artifact_schema_v1 CHECK (content_schema_version = 1),
    CONSTRAINT worldwright_artifact_game_v1 CHECK (game_id = 'worldwright-reference' AND game_contract_version = 1)
);

ALTER TABLE worldwright_worlds
    ADD COLUMN active_release_digest BYTEA REFERENCES worldwright_content_artifacts(artifact_digest) ON DELETE RESTRICT,
    ADD COLUMN known_good_release_digest BYTEA REFERENCES worldwright_content_artifacts(artifact_digest) ON DELETE RESTRICT,
    ADD COLUMN release_maintenance BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE worldwright_worlds
    ADD CONSTRAINT worldwright_active_release_digest_length CHECK (active_release_digest IS NULL OR octet_length(active_release_digest) = 32),
    ADD CONSTRAINT worldwright_known_good_release_digest_length CHECK (known_good_release_digest IS NULL OR octet_length(known_good_release_digest) = 32);

CREATE TABLE worldwright_publication_requests (
    request_id UUID PRIMARY KEY,
    world_id UUID NOT NULL REFERENCES worldwright_worlds(world_id) ON DELETE RESTRICT,
    artifact_digest BYTEA NOT NULL REFERENCES worldwright_content_artifacts(artifact_digest) ON DELETE RESTRICT,
    kind TEXT NOT NULL,
    target_environment TEXT NOT NULL,
    expected_current_digest BYTEA,
    requested_by TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'requested',
    reviewed_by TEXT,
    review_reason TEXT,
    activation_executor TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    reviewed_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    CONSTRAINT worldwright_publication_request_id_nonzero CHECK (request_id <> '00000000-0000-0000-0000-000000000000'::UUID),
    CONSTRAINT worldwright_publication_kind_valid CHECK (kind IN ('activate', 'rollback')),
    CONSTRAINT worldwright_publication_target_valid CHECK (target_environment IN ('staging', 'production')),
    CONSTRAINT worldwright_publication_expected_digest_length CHECK (expected_current_digest IS NULL OR octet_length(expected_current_digest) = 32),
    CONSTRAINT worldwright_publication_requester_valid CHECK (octet_length(requested_by) BETWEEN 1 AND 128 AND btrim(requested_by) = requested_by AND requested_by !~ '[[:cntrl:]]'),
    CONSTRAINT worldwright_publication_status_valid CHECK (status IN ('requested', 'approved', 'rejected', 'activating', 'activated', 'failed')),
    CONSTRAINT worldwright_publication_review_shape CHECK ((status = 'requested' AND reviewed_by IS NULL AND review_reason IS NULL AND reviewed_at IS NULL) OR (status <> 'requested' AND reviewed_by IS NOT NULL AND review_reason IS NOT NULL AND reviewed_at IS NOT NULL)),
    CONSTRAINT worldwright_publication_reviewer_valid CHECK (reviewed_by IS NULL OR (octet_length(reviewed_by) BETWEEN 1 AND 128 AND btrim(reviewed_by) = reviewed_by AND reviewed_by !~ '[[:cntrl:]]')),
    CONSTRAINT worldwright_publication_reason_valid CHECK (review_reason IS NULL OR (octet_length(review_reason) BETWEEN 1 AND 512 AND btrim(review_reason) = review_reason AND review_reason !~ '[[:cntrl:]]')),
    CONSTRAINT worldwright_publication_executor_valid CHECK (activation_executor IS NULL OR (octet_length(activation_executor) BETWEEN 1 AND 128 AND btrim(activation_executor) = activation_executor AND activation_executor !~ '[[:cntrl:]]')),
    CONSTRAINT worldwright_publication_completion_shape CHECK ((status IN ('activated', 'failed') AND completed_at IS NOT NULL) OR (status NOT IN ('activated', 'failed') AND completed_at IS NULL))
);

CREATE INDEX worldwright_publication_by_world_status ON worldwright_publication_requests(world_id, status, created_at, request_id);

CREATE TABLE worldwright_release_activations (
    activation_id UUID PRIMARY KEY,
    request_id UUID NOT NULL UNIQUE REFERENCES worldwright_publication_requests(request_id) ON DELETE RESTRICT,
    world_id UUID NOT NULL REFERENCES worldwright_worlds(world_id) ON DELETE RESTRICT,
    previous_release_digest BYTEA REFERENCES worldwright_content_artifacts(artifact_digest) ON DELETE RESTRICT,
    target_release_digest BYTEA NOT NULL REFERENCES worldwright_content_artifacts(artifact_digest) ON DELETE RESTRICT,
    writer_id UUID NOT NULL,
    writer_epoch BIGINT NOT NULL,
    state TEXT NOT NULL DEFAULT 'activating',
    failure_code TEXT,
    started_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    completed_at TIMESTAMPTZ,
    CONSTRAINT worldwright_activation_id_nonzero CHECK (activation_id <> '00000000-0000-0000-0000-000000000000'::UUID),
    CONSTRAINT worldwright_activation_previous_digest_length CHECK (previous_release_digest IS NULL OR octet_length(previous_release_digest) = 32),
    CONSTRAINT worldwright_activation_target_digest_length CHECK (octet_length(target_release_digest) = 32),
    CONSTRAINT worldwright_activation_writer_nonzero CHECK (writer_id <> '00000000-0000-0000-0000-000000000000'::UUID),
    CONSTRAINT worldwright_activation_epoch_positive CHECK (writer_epoch > 0),
    CONSTRAINT worldwright_activation_state_valid CHECK (state IN ('activating', 'succeeded', 'failed')),
    CONSTRAINT worldwright_activation_failure_shape CHECK ((state = 'failed' AND failure_code IS NOT NULL AND completed_at IS NOT NULL) OR (state = 'succeeded' AND failure_code IS NULL AND completed_at IS NOT NULL) OR (state = 'activating' AND failure_code IS NULL AND completed_at IS NULL)),
    CONSTRAINT worldwright_activation_failure_code_valid CHECK (failure_code IS NULL OR failure_code IN ('artifact_invalid', 'durable_incompatible', 'smoke_failed', 'operator_aborted'))
);

ALTER TABLE worldwright_worlds ADD COLUMN pending_activation_id UUID REFERENCES worldwright_release_activations(activation_id) ON DELETE RESTRICT;

ALTER TABLE worldwright_worlds
    ADD CONSTRAINT worldwright_release_maintenance_shape CHECK (
        (pending_activation_id IS NULL AND release_maintenance = FALSE)
        OR (pending_activation_id IS NOT NULL AND release_maintenance = TRUE)
    );

CREATE TABLE worldwright_release_audit (
    audit_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    world_id UUID NOT NULL REFERENCES worldwright_worlds(world_id) ON DELETE RESTRICT,
    request_id UUID NOT NULL REFERENCES worldwright_publication_requests(request_id) ON DELETE RESTRICT,
    activation_id UUID REFERENCES worldwright_release_activations(activation_id) ON DELETE RESTRICT,
    actor TEXT NOT NULL,
    action TEXT NOT NULL,
    artifact_digest BYTEA NOT NULL,
    target_environment TEXT NOT NULL,
    summary JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT worldwright_release_audit_actor_valid CHECK (octet_length(actor) BETWEEN 1 AND 128 AND btrim(actor) = actor AND actor !~ '[[:cntrl:]]'),
    CONSTRAINT worldwright_release_audit_action_valid CHECK (action IN ('publication_requested', 'publication_approved', 'publication_rejected', 'activation_started', 'activation_succeeded', 'activation_failed')),
    CONSTRAINT worldwright_release_audit_digest_length CHECK (octet_length(artifact_digest) = 32),
    CONSTRAINT worldwright_release_audit_target_valid CHECK (target_environment IN ('staging', 'production')),
    CONSTRAINT worldwright_release_audit_summary_object CHECK (jsonb_typeof(summary) = 'object')
);

CREATE INDEX worldwright_release_audit_by_world_time ON worldwright_release_audit(world_id, created_at, audit_id);
