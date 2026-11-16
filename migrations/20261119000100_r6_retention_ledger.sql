-- Additive R6 maintenance ledger; no existing migration/checksum is edited.
-- This does not repair an earlier migration syntax failure.
CREATE TABLE worldwright_retention_runs (
    operation_id UUID PRIMARY KEY CHECK (operation_id <> '00000000-0000-0000-0000-000000000000'::UUID),
    world_id UUID NOT NULL REFERENCES worldwright_worlds(world_id) ON DELETE RESTRICT,
    target_environment TEXT NOT NULL CHECK (target_environment IN ('staging', 'production')),
    actor_subject TEXT NOT NULL CHECK (octet_length(actor_subject) BETWEEN 1 AND 128),
    actor_session_id UUID NOT NULL CHECK (actor_session_id <> '00000000-0000-0000-0000-000000000000'::UUID),
    correlation_id UUID NOT NULL,
    policy_version TEXT NOT NULL CHECK (policy_version = 'expired-authentication-only-v1'),
    reason TEXT NOT NULL CHECK (octet_length(reason) BETWEEN 1 AND 512 AND btrim(reason) = reason AND reason !~ '[[:cntrl:]]'),
    batch_limit INTEGER NOT NULL CHECK (batch_limit BETWEEN 1 AND 1000),
    sessions_deleted INTEGER NOT NULL CHECK (sessions_deleted >= 0),
    invites_deleted INTEGER NOT NULL CHECK (invites_deleted >= 0),
    authorization_snapshot JSONB NOT NULL CHECK (jsonb_typeof(authorization_snapshot) = 'object' AND octet_length(authorization_snapshot::TEXT) <= 2048),
    completed_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CHECK (sessions_deleted + invites_deleted <= batch_limit)
);
CREATE INDEX worldwright_retention_runs_by_world_time
    ON worldwright_retention_runs (world_id, completed_at, operation_id);
