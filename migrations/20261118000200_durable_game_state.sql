ALTER TABLE worldwright_worlds
    ADD COLUMN game_id TEXT NOT NULL DEFAULT 'worldwright-reference',
    ADD COLUMN game_contract_version BIGINT NOT NULL DEFAULT 1,
    ADD COLUMN game_state_version BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN runtime_revision BIGINT NOT NULL DEFAULT 0;

ALTER TABLE worldwright_worlds
    ADD CONSTRAINT worldwright_world_game_contract_valid CHECK (
        (game_id = 'worldwright-reference' AND game_contract_version = 1)
        OR (game_id = 'worldwright-expedition' AND game_contract_version = 1)
    ),
    ADD CONSTRAINT worldwright_game_state_version_nonnegative CHECK (
        game_state_version >= 0
    ),
    ADD CONSTRAINT worldwright_runtime_revision_nonnegative CHECK (
        runtime_revision >= 0
    );

CREATE TABLE worldwright_game_counters (
    world_id UUID NOT NULL,
    character_id UUID NOT NULL,
    counter_key TEXT NOT NULL,
    counter_value BIGINT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (world_id, character_id, counter_key),
    CONSTRAINT worldwright_game_counter_character_fk
        FOREIGN KEY (character_id, world_id)
        REFERENCES worldwright_characters(character_id, world_id)
        ON DELETE CASCADE,
    CONSTRAINT worldwright_game_counter_key_valid CHECK (
        octet_length(counter_key) BETWEEN 3 AND 192
        AND counter_key ~ '^[a-z][a-z0-9_-]*:[a-z][a-z0-9_-]*(\.[a-z][a-z0-9_-]*)*$'
    ),
    CONSTRAINT worldwright_game_counter_value_valid CHECK (
        counter_value BETWEEN 0 AND 1000000
    )
);

CREATE TABLE worldwright_scheduled_counter_adjustments (
    world_id UUID NOT NULL REFERENCES worldwright_worlds(world_id) ON DELETE CASCADE,
    schedule_ordinal SMALLINT NOT NULL,
    character_id UUID NOT NULL,
    counter_key TEXT NOT NULL,
    delta BIGINT NOT NULL,
    due_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (world_id, schedule_ordinal),
    CONSTRAINT worldwright_scheduled_counter_character_fk
        FOREIGN KEY (character_id, world_id)
        REFERENCES worldwright_characters(character_id, world_id)
        ON DELETE CASCADE,
    CONSTRAINT worldwright_schedule_ordinal_valid CHECK (
        schedule_ordinal BETWEEN 0 AND 127
    ),
    CONSTRAINT worldwright_scheduled_counter_key_valid CHECK (
        octet_length(counter_key) BETWEEN 3 AND 192
        AND counter_key ~ '^[a-z][a-z0-9_-]*:[a-z][a-z0-9_-]*(\.[a-z][a-z0-9_-]*)*$'
    ),
    CONSTRAINT worldwright_scheduled_counter_delta_valid CHECK (
        delta BETWEEN -1000000 AND 1000000
        AND delta <> 0
    )
);

CREATE INDEX worldwright_scheduled_counter_due
    ON worldwright_scheduled_counter_adjustments (world_id, due_at, schedule_ordinal);

CREATE TABLE worldwright_gameplay_receipts (
    world_id UUID NOT NULL REFERENCES worldwright_worlds(world_id) ON DELETE RESTRICT,
    account_id UUID NOT NULL,
    character_id UUID NOT NULL,
    request_id TEXT NOT NULL,
    operation_id UUID NOT NULL UNIQUE,
    intent_kind TEXT NOT NULL,
    intent_fingerprint BYTEA NOT NULL,
    result_version SMALLINT NOT NULL DEFAULT 1,
    result JSONB NOT NULL,
    state_version BIGINT NOT NULL,
    committed_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (world_id, account_id, character_id, request_id),
    CONSTRAINT worldwright_gameplay_receipt_character_fk
        FOREIGN KEY (character_id, account_id, world_id)
        REFERENCES worldwright_characters(character_id, account_id, world_id)
        ON DELETE RESTRICT,
    CONSTRAINT worldwright_gameplay_request_id_valid CHECK (
        octet_length(request_id) BETWEEN 1 AND 128
        AND request_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]*$'
    ),
    CONSTRAINT worldwright_gameplay_operation_id_nonzero CHECK (
        operation_id <> '00000000-0000-0000-0000-000000000000'::UUID
    ),
    CONSTRAINT worldwright_gameplay_intent_kind_valid CHECK (
        intent_kind IN ('look', 'move', 'say', 'game_action')
    ),
    CONSTRAINT worldwright_gameplay_intent_fingerprint_length CHECK (
        octet_length(intent_fingerprint) = 32
    ),
    CONSTRAINT worldwright_gameplay_result_version_v1 CHECK (result_version = 1),
    CONSTRAINT worldwright_gameplay_result_object CHECK (jsonb_typeof(result) = 'object'),
    CONSTRAINT worldwright_gameplay_receipt_state_version_positive CHECK (
        state_version > 0
    )
);

CREATE INDEX worldwright_gameplay_receipts_by_commit
    ON worldwright_gameplay_receipts (world_id, committed_at, operation_id);

CREATE TABLE worldwright_gameplay_audit (
    audit_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    world_id UUID NOT NULL,
    account_id UUID NOT NULL,
    character_id UUID NOT NULL,
    operation_id UUID NOT NULL UNIQUE
        REFERENCES worldwright_gameplay_receipts(operation_id) ON DELETE RESTRICT,
    request_id TEXT NOT NULL,
    action TEXT NOT NULL,
    state_version BIGINT NOT NULL,
    summary JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT worldwright_gameplay_audit_character_fk
        FOREIGN KEY (character_id, account_id, world_id)
        REFERENCES worldwright_characters(character_id, account_id, world_id)
        ON DELETE RESTRICT,
    CONSTRAINT worldwright_gameplay_audit_action_valid CHECK (
        action IN ('look', 'move', 'say', 'game_action')
    ),
    CONSTRAINT worldwright_gameplay_audit_state_version_positive CHECK (
        state_version > 0
    ),
    CONSTRAINT worldwright_gameplay_audit_summary_object CHECK (
        jsonb_typeof(summary) = 'object'
    )
);

CREATE INDEX worldwright_gameplay_audit_by_world_time
    ON worldwright_gameplay_audit (world_id, created_at, audit_id);
