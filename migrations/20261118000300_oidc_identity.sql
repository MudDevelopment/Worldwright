CREATE TABLE worldwright_player_invites (
    invite_id UUID PRIMARY KEY,
    world_id UUID NOT NULL REFERENCES worldwright_worlds(world_id) ON DELETE RESTRICT,
    account_id UUID NOT NULL REFERENCES worldwright_accounts(account_id) ON DELETE RESTRICT,
    issuer TEXT NOT NULL,
    token_hash BYTEA NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    consumed_subject TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT worldwright_player_invite_id_nonzero CHECK (
        invite_id <> '00000000-0000-0000-0000-000000000000'::UUID
    ),
    CONSTRAINT worldwright_player_invite_issuer_valid CHECK (
        octet_length(issuer) BETWEEN 8 AND 2048
        AND issuer = btrim(issuer)
        AND issuer !~ '[[:cntrl:]]'
    ),
    CONSTRAINT worldwright_player_invite_hash_length CHECK (
        octet_length(token_hash) = 32
    ),
    CONSTRAINT worldwright_player_invite_expiry_valid CHECK (
        expires_at > created_at
    ),
    CONSTRAINT worldwright_player_invite_consumption_shape CHECK (
        (consumed_at IS NULL AND consumed_subject IS NULL)
        OR (
            consumed_at IS NOT NULL
            AND consumed_subject IS NOT NULL
            AND consumed_at >= created_at
            AND octet_length(consumed_subject) BETWEEN 1 AND 255
            AND consumed_subject = btrim(consumed_subject)
            AND consumed_subject !~ '[[:cntrl:]]'
        )
    )
);

CREATE INDEX worldwright_player_invites_active
    ON worldwright_player_invites (world_id, expires_at, invite_id)
    WHERE consumed_at IS NULL;

CREATE TABLE worldwright_oidc_identities (
    issuer TEXT NOT NULL,
    subject TEXT NOT NULL,
    account_id UUID NOT NULL REFERENCES worldwright_accounts(account_id) ON DELETE RESTRICT,
    linked_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    last_login_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (issuer, subject),
    CONSTRAINT worldwright_oidc_identity_issuer_valid CHECK (
        octet_length(issuer) BETWEEN 8 AND 2048
        AND issuer = btrim(issuer)
        AND issuer !~ '[[:cntrl:]]'
    ),
    CONSTRAINT worldwright_oidc_identity_subject_valid CHECK (
        octet_length(subject) BETWEEN 1 AND 255
        AND subject = btrim(subject)
        AND subject !~ '[[:cntrl:]]'
    )
);

CREATE INDEX worldwright_oidc_identities_by_account
    ON worldwright_oidc_identities (account_id, issuer);

CREATE TABLE worldwright_oidc_audit (
    audit_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    world_id UUID NOT NULL REFERENCES worldwright_worlds(world_id) ON DELETE RESTRICT,
    account_id UUID NOT NULL REFERENCES worldwright_accounts(account_id) ON DELETE RESTRICT,
    issuer TEXT NOT NULL,
    subject TEXT NOT NULL,
    action TEXT NOT NULL,
    invite_id UUID REFERENCES worldwright_player_invites(invite_id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT worldwright_oidc_audit_action_valid CHECK (
        action IN ('identity_linked', 'identity_login')
    ),
    CONSTRAINT worldwright_oidc_audit_issuer_valid CHECK (
        octet_length(issuer) BETWEEN 8 AND 2048
        AND issuer = btrim(issuer)
        AND issuer !~ '[[:cntrl:]]'
    ),
    CONSTRAINT worldwright_oidc_audit_subject_valid CHECK (
        octet_length(subject) BETWEEN 1 AND 255
        AND subject = btrim(subject)
        AND subject !~ '[[:cntrl:]]'
    )
);

CREATE INDEX worldwright_oidc_audit_by_world_time
    ON worldwright_oidc_audit (world_id, created_at, audit_id);
