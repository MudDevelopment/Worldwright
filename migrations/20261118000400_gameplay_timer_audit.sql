-- Additive migration: background timer transitions have writer authority, not
-- a player session or a fabricated player receipt. Existing migrations stay immutable.
CREATE TABLE worldwright_gameplay_timer_audit (
    world_id UUID NOT NULL REFERENCES worldwright_worlds(world_id) ON DELETE RESTRICT,
    state_version BIGINT NOT NULL CHECK (state_version > 0),
    runtime_revision BIGINT NOT NULL CHECK (runtime_revision >= 0),
    writer_id UUID NOT NULL,
    writer_epoch BIGINT NOT NULL CHECK (writer_epoch > 0),
    authority_incarnation UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (world_id, state_version)
);
