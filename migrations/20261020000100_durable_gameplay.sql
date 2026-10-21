CREATE TABLE worldwright_worlds (
    world_id UUID PRIMARY KEY,
    content_namespace TEXT NOT NULL,
    content_schema_version BIGINT NOT NULL,
    writer_epoch BIGINT NOT NULL DEFAULT 0,
    active_writer_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT worldwright_world_id_nonzero CHECK (world_id <> '00000000-0000-0000-0000-000000000000'::UUID),
    CONSTRAINT worldwright_content_namespace_valid CHECK (
        octet_length(content_namespace) BETWEEN 1 AND 192
        AND content_namespace ~ '^[a-z][a-z0-9_-]*$'
    ),
    CONSTRAINT worldwright_content_schema_version_positive CHECK (content_schema_version > 0),
    CONSTRAINT worldwright_writer_epoch_nonnegative CHECK (writer_epoch >= 0),
    CONSTRAINT worldwright_writer_state_consistent CHECK (
        (writer_epoch = 0 AND active_writer_id IS NULL)
        OR (writer_epoch > 0 AND active_writer_id IS NOT NULL)
    ),
    CONSTRAINT worldwright_writer_id_nonzero CHECK (
        active_writer_id IS NULL
        OR active_writer_id <> '00000000-0000-0000-0000-000000000000'::UUID
    )
);

CREATE TABLE worldwright_accounts (
    account_id UUID PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    disabled BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT worldwright_account_id_nonzero CHECK (account_id <> '00000000-0000-0000-0000-000000000000'::UUID),
    CONSTRAINT worldwright_username_valid CHECK (
        octet_length(username) BETWEEN 1 AND 64
        AND username ~ '^[a-z][a-z0-9_-]*$'
    ),
    CONSTRAINT worldwright_password_hash_bounded CHECK (octet_length(password_hash) BETWEEN 1 AND 512)
);

CREATE TABLE worldwright_characters (
    character_id UUID PRIMARY KEY,
    account_id UUID NOT NULL REFERENCES worldwright_accounts(account_id) ON DELETE RESTRICT,
    world_id UUID NOT NULL REFERENCES worldwright_worlds(world_id) ON DELETE RESTRICT,
    character_name TEXT NOT NULL,
    room_definition TEXT NOT NULL,
    balance BIGINT NOT NULL DEFAULT 0,
    version BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT worldwright_character_id_nonzero CHECK (character_id <> '00000000-0000-0000-0000-000000000000'::UUID),
    CONSTRAINT worldwright_character_name_valid CHECK (
        octet_length(character_name) BETWEEN 1 AND 64
        AND btrim(character_name) <> ''
        AND character_name !~ '[[:cntrl:]]'
    ),
    CONSTRAINT worldwright_room_definition_valid CHECK (
        octet_length(room_definition) BETWEEN 3 AND 192
        AND room_definition ~ '^[a-z][a-z0-9_-]*:[a-z][a-z0-9_-]*(\.[a-z][a-z0-9_-]*)*$'
    ),
    CONSTRAINT worldwright_balance_nonnegative CHECK (balance >= 0),
    CONSTRAINT worldwright_character_version_positive CHECK (version > 0),
    CONSTRAINT worldwright_one_character_per_account UNIQUE (world_id, account_id),
    CONSTRAINT worldwright_character_world_key UNIQUE (character_id, world_id),
    CONSTRAINT worldwright_character_authority_key UNIQUE (character_id, account_id, world_id)
);

CREATE TABLE worldwright_sessions (
    token_hash BYTEA PRIMARY KEY,
    account_id UUID NOT NULL,
    character_id UUID NOT NULL,
    world_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    CONSTRAINT worldwright_session_hash_length CHECK (octet_length(token_hash) = 32),
    CONSTRAINT worldwright_session_lifetime CHECK (expires_at > created_at),
    CONSTRAINT worldwright_session_revocation_time CHECK (revoked_at IS NULL OR revoked_at >= created_at),
    CONSTRAINT worldwright_session_character_fk
        FOREIGN KEY (character_id, account_id, world_id)
        REFERENCES worldwright_characters(character_id, account_id, world_id)
        ON DELETE CASCADE
);

CREATE INDEX worldwright_sessions_active_expiry
    ON worldwright_sessions (expires_at)
    WHERE revoked_at IS NULL;

CREATE TABLE worldwright_item_instances (
    item_id UUID PRIMARY KEY,
    world_id UUID NOT NULL,
    definition_id TEXT NOT NULL,
    owner_character_id UUID NOT NULL,
    version BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT worldwright_item_id_nonzero CHECK (item_id <> '00000000-0000-0000-0000-000000000000'::UUID),
    CONSTRAINT worldwright_item_definition_valid CHECK (
        octet_length(definition_id) BETWEEN 3 AND 192
        AND definition_id ~ '^[a-z][a-z0-9_-]*:[a-z][a-z0-9_-]*(\.[a-z][a-z0-9_-]*)*$'
    ),
    CONSTRAINT worldwright_item_version_positive CHECK (version > 0),
    CONSTRAINT worldwright_item_world_key UNIQUE (item_id, world_id),
    CONSTRAINT worldwright_item_owner_fk
        FOREIGN KEY (owner_character_id, world_id)
        REFERENCES worldwright_characters(character_id, world_id)
        ON DELETE RESTRICT
);

CREATE INDEX worldwright_items_by_owner
    ON worldwright_item_instances (world_id, owner_character_id, item_id);

CREATE TABLE worldwright_operation_receipts (
    world_id UUID NOT NULL REFERENCES worldwright_worlds(world_id) ON DELETE RESTRICT,
    account_id UUID NOT NULL,
    character_id UUID NOT NULL,
    request_id TEXT NOT NULL,
    operation_id UUID NOT NULL UNIQUE,
    intent_kind TEXT NOT NULL,
    intent_fingerprint BYTEA NOT NULL,
    outcome TEXT NOT NULL,
    result_version SMALLINT NOT NULL DEFAULT 1,
    result JSONB NOT NULL,
    committed_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (world_id, account_id, character_id, request_id),
    CONSTRAINT worldwright_receipt_character_fk
        FOREIGN KEY (character_id, account_id, world_id)
        REFERENCES worldwright_characters(character_id, account_id, world_id)
        ON DELETE RESTRICT,
    CONSTRAINT worldwright_request_id_valid CHECK (
        octet_length(request_id) BETWEEN 1 AND 128
        AND request_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]*$'
    ),
    CONSTRAINT worldwright_operation_id_nonzero CHECK (operation_id <> '00000000-0000-0000-0000-000000000000'::UUID),
    CONSTRAINT worldwright_intent_kind_valid CHECK (intent_kind IN ('claim_reward', 'transfer_item')),
    CONSTRAINT worldwright_intent_fingerprint_length CHECK (octet_length(intent_fingerprint) = 32),
    CONSTRAINT worldwright_receipt_outcome_valid CHECK (
        outcome IN (
            'reward_granted',
            'already_claimed',
            'inventory_full',
            'item_transferred',
            'not_owned',
            'same_owner',
            'target_not_found'
        )
    ),
    CONSTRAINT worldwright_result_version_v1 CHECK (result_version = 1),
    CONSTRAINT worldwright_receipt_result_object CHECK (jsonb_typeof(result) = 'object')
);

CREATE INDEX worldwright_receipts_by_commit
    ON worldwright_operation_receipts (world_id, committed_at, operation_id);

CREATE TABLE worldwright_reward_claims (
    world_id UUID NOT NULL,
    character_id UUID NOT NULL,
    reward_key TEXT NOT NULL,
    completion_generation BIGINT NOT NULL,
    operation_id UUID NOT NULL UNIQUE REFERENCES worldwright_operation_receipts(operation_id) ON DELETE RESTRICT,
    credits BIGINT NOT NULL,
    item_id UUID NOT NULL,
    claimed_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (world_id, character_id, reward_key, completion_generation),
    CONSTRAINT worldwright_reward_character_fk
        FOREIGN KEY (character_id, world_id)
        REFERENCES worldwright_characters(character_id, world_id)
        ON DELETE RESTRICT,
    CONSTRAINT worldwright_reward_item_fk
        FOREIGN KEY (item_id, world_id)
        REFERENCES worldwright_item_instances(item_id, world_id)
        ON DELETE RESTRICT,
    CONSTRAINT worldwright_reward_key_valid CHECK (
        octet_length(reward_key) BETWEEN 1 AND 64
        AND reward_key ~ '^[a-z][a-z0-9_-]*$'
    ),
    CONSTRAINT worldwright_reward_generation_positive CHECK (completion_generation > 0),
    CONSTRAINT worldwright_reward_operation_nonzero CHECK (operation_id <> '00000000-0000-0000-0000-000000000000'::UUID),
    CONSTRAINT worldwright_reward_credits_positive CHECK (credits > 0)
);

CREATE TABLE worldwright_audit_log (
    audit_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    world_id UUID NOT NULL,
    account_id UUID NOT NULL,
    character_id UUID NOT NULL,
    operation_id UUID NOT NULL UNIQUE REFERENCES worldwright_operation_receipts(operation_id) ON DELETE RESTRICT,
    request_id TEXT NOT NULL,
    action TEXT NOT NULL,
    outcome TEXT NOT NULL,
    summary JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT worldwright_audit_character_fk
        FOREIGN KEY (character_id, account_id, world_id)
        REFERENCES worldwright_characters(character_id, account_id, world_id)
        ON DELETE RESTRICT,
    CONSTRAINT worldwright_audit_action_valid CHECK (action IN ('claim_reward', 'transfer_item')),
    CONSTRAINT worldwright_audit_summary_object CHECK (jsonb_typeof(summary) = 'object')
);

CREATE INDEX worldwright_audit_by_world_time
    ON worldwright_audit_log (world_id, created_at, audit_id);

CREATE TABLE worldwright_outbox (
    operation_id UUID PRIMARY KEY REFERENCES worldwright_operation_receipts(operation_id) ON DELETE RESTRICT,
    world_id UUID NOT NULL REFERENCES worldwright_worlds(world_id) ON DELETE RESTRICT,
    topic TEXT NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    delivered_at TIMESTAMPTZ,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    CONSTRAINT worldwright_outbox_topic_valid CHECK (topic IN ('reward_claimed', 'item_transferred')),
    CONSTRAINT worldwright_outbox_payload_object CHECK (jsonb_typeof(payload) = 'object'),
    CONSTRAINT worldwright_outbox_attempt_nonnegative CHECK (attempt_count >= 0),
    CONSTRAINT worldwright_outbox_delivery_time CHECK (delivered_at IS NULL OR delivered_at >= created_at)
);

CREATE INDEX worldwright_outbox_pending
    ON worldwright_outbox (created_at, operation_id)
    WHERE delivered_at IS NULL;
