# Durable Gameplay Persistence v1

## Status and scope

This document is the implementation contract for Milestone 4. The schema is defined by `migrations/20261020000100_durable_gameplay.sql`. PostgreSQL is authoritative only when the server is started in explicit durable mode. Existing local mode remains available and continues to use the bounded JSON account registry and process-local sessions.

The first durable slice contains:

- PostgreSQL-backed accounts and revocable sessions.
- One explicit writer epoch for world `00000000000000000000000000000001`.
- Durable character balances and inventory ownership.
- A one-time `welcome` reward that grants 100 credits and one namespaced welcome-token item.
- Transactional item transfer between character inventories.
- Scoped operation receipts, audit rows, and outbox rows.
- Inventory/balance reconstruction and receipt replay after process restart.

Room movement remains ephemeral in this increment. A restart reconstructs identities, balances, and inventory from PostgreSQL but places characters at the validated content entry room. Persisting location requires the runtime prepare/commit boundary described under Deferred Work; it must not be approximated by committing after an in-memory Move.

## Deployment boundary

Database credentials are read from the explicitly named environment variable supplied through `--database-url-env`. The URL value must never be accepted as a command-line argument, printed, included in diagnostics, or written to configuration examples.

Durable commands are enabled only when all of these have succeeded:

1. Connect to PostgreSQL with bounded pool and acquisition timeouts.
2. Verify that the migration set is current.
3. Verify that the seeded world's namespace and content schema match the loaded content.
4. Explicitly acquire a new writer epoch using `--acquire-writer`.
5. Load active accounts and characters and map every durable room definition to the current validated content.

A stale writer, corrupt durable result, or unresolved commit outcome stops durable admission and marks the world not ready. It must never fall back to local success.

Every pooled PostgreSQL connection applies these fixed v0.1 bounds before use:

```sql
SET statement_timeout = '5s'
SET lock_timeout = '2s'
SET idle_in_transaction_session_timeout = '10s'
```

Pool acquisition is also bounded to five seconds. A timeout is an explicit database failure; it never permits speculative success.

## Identifier mapping

All Worldwright 128-bit instance IDs map to PostgreSQL `UUID` values by preserving the exact unsigned 128-bit value. Zero UUIDs are rejected by both Rust constructors and database constraints. Protocol-facing IDs use the existing 32-character lowercase hexadecimal form without hyphens.

Session cookies contain a random 32-byte token encoded as 64 lowercase hexadecimal characters. PostgreSQL stores only `SHA-256(token_bytes_as_hex_text)` as a 32-byte `BYTEA`; it never stores or returns the bearer token.

## Migration policy

- SQLx migrations are explicit operator actions; normal server startup verifies but never runs migrations.
- The initial migration is forward-only. No automatic destructive down migration is supplied.
- Seed is insert-only or exact-idempotent. Existing mismatched world, account, username, character, ownership, content, or password-hash data causes the whole seed transaction to roll back.
- Migration and seed commands are intended only for a specifically provisioned database identity. Runtime credentials should not have schema-change privileges.
- Tests create an isolated generated schema only when `WORLDWRIGHT_TEST_DATABASE_URL` is set and the parsed database name contains `worldwright_test`. Tests never target a database that fails that guard.

## Seed transaction

The seed input is the existing strict account-registry v1 plus validated world content. Accounts are processed in ascending account-ID order.

Lock or inspect the world:

```sql
SELECT content_namespace, content_schema_version, writer_epoch, active_writer_id
FROM worldwright_worlds
WHERE world_id = $1
FOR UPDATE
```

If absent, insert it:

```sql
INSERT INTO worldwright_worlds (
    world_id,
    content_namespace,
    content_schema_version
)
VALUES ($1, $2, $3)
```

An existing world is accepted only when namespace/schema match and no writer has ever been activated (`writer_epoch = 0`, `active_writer_id IS NULL`).

For each account, detect either ID or username conflicts:

```sql
SELECT account_id, username, password_hash, disabled
FROM worldwright_accounts
WHERE account_id = $1 OR username = $2
FOR UPDATE
```

If absent, insert it:

```sql
INSERT INTO worldwright_accounts (
    account_id,
    username,
    password_hash
)
VALUES ($1, $2, $3)
```

The query must be consumed as a complete result set. Exactly one existing row is accepted only when all selected fields exactly match and `disabled = FALSE`; two rows means the requested ID and username conflict with different accounts and the seed fails.

For each character, detect either character-ID or one-character-per-account/world conflicts:

```sql
SELECT character_id, account_id, world_id, character_name, room_definition
FROM worldwright_characters
WHERE character_id = $1 OR (world_id = $3 AND account_id = $2)
FOR UPDATE
```

If absent, insert it:

```sql
INSERT INTO worldwright_characters (
    character_id,
    account_id,
    world_id,
    character_name,
    room_definition
)
VALUES ($1, $2, $3, $4, $5)
```

The query must be consumed as a complete result set. Exactly one existing character is accepted only when every selected identity/content field matches; two rows means the character ID and account/world key conflict with different characters and the seed fails. Seed never resets balance, version, or a later durable location.

## Migration-state check

Normal startup checks that both the application schema and SQLx migration ledger exist, then compares every embedded migration version/checksum with the successfully applied rows without applying missing migrations. The only discovery queries are:

```sql
SELECT to_regclass('worldwright_worlds')::TEXT
```

```sql
SELECT version, checksum, success
FROM _sqlx_migrations
ORDER BY version
```

Missing, failed, extra, or checksum-mismatched migrations are startup failures with sanitized diagnostics.

Durable startup verifies the loaded content and reconstructs active identities with:

```sql
SELECT content_namespace, content_schema_version
FROM worldwright_worlds
WHERE world_id = $1
```

```sql
SELECT a.account_id,
       a.username,
       a.password_hash,
       c.character_id,
       c.character_name,
       c.room_definition,
       c.balance,
       c.version
FROM worldwright_accounts AS a
JOIN worldwright_characters AS c ON c.account_id = a.account_id
WHERE c.world_id = $1
  AND a.disabled = FALSE
ORDER BY c.character_id
LIMIT 257
```

Zero identities or more than 256 rows is invalid durable state. Every room definition must resolve through the currently loaded validated content before player admission opens.

## Writer acquisition and fencing

The server generates a random nonzero writer ID and performs this statement only when the operator supplied `--acquire-writer`:

```sql
UPDATE worldwright_worlds
SET writer_epoch = writer_epoch + 1,
    active_writer_id = $2,
    updated_at = clock_timestamp()
WHERE world_id = $1
  AND writer_epoch < 9223372036854775807
RETURNING writer_epoch
```

No row means unknown world or exhausted epoch; startup fails. Every durable mutation begins a transaction and executes:

```sql
SELECT writer_epoch, active_writer_id
FROM worldwright_worlds
WHERE world_id = $1
FOR UPDATE
```

The returned epoch and writer ID must exactly equal the in-process lease. A mismatch returns `StaleWriter`; the world owner stops durable admission, becomes not-ready, and closes active routes with `writer_fenced`.

## Durable identity and sessions

Find a login identity for one world:

```sql
SELECT a.account_id, c.character_id, c.character_name, a.password_hash
FROM worldwright_accounts AS a
JOIN worldwright_characters AS c ON c.account_id = a.account_id
WHERE a.username = $1
  AND c.world_id = $2
  AND a.disabled = FALSE
```

Password verification remains bounded Argon2id work in the server. Unknown users use the same dummy-hash path as local mode.

Session creation is one transaction. It first serializes capacity enforcement and prunes dead rows:

```sql
LOCK TABLE worldwright_sessions IN SHARE ROW EXCLUSIVE MODE
```

```sql
DELETE FROM worldwright_sessions
WHERE revoked_at IS NOT NULL OR expires_at <= clock_timestamp()
```

```sql
SELECT COUNT(*)::BIGINT
FROM worldwright_sessions
WHERE revoked_at IS NULL AND expires_at > clock_timestamp()
```

The account/character pair is revalidated during insertion:

```sql
INSERT INTO worldwright_sessions (
    token_hash,
    account_id,
    character_id,
    world_id,
    expires_at
)
SELECT $1, a.account_id, c.character_id, c.world_id,
       clock_timestamp() + ($5::DOUBLE PRECISION * INTERVAL '1 second')
FROM worldwright_accounts AS a
JOIN worldwright_characters AS c ON c.account_id = a.account_id
WHERE a.account_id = $2
  AND c.character_id = $3
  AND c.world_id = $4
  AND a.disabled = FALSE
ON CONFLICT (token_hash) DO NOTHING
RETURNING expires_at
```

Zero returned rows mean collision or revoked authority. Distinguish them with this exact authority check, then retry a token collision at most eight times:

```sql
SELECT EXISTS (
    SELECT 1
    FROM worldwright_accounts AS a
    JOIN worldwright_characters AS c ON c.account_id = a.account_id
    WHERE a.account_id = $1
      AND c.character_id = $2
      AND c.world_id = $3
      AND a.disabled = FALSE
)
```

Session lookup is:

```sql
SELECT a.account_id, c.character_id, c.character_name
FROM worldwright_sessions AS s
JOIN worldwright_accounts AS a ON a.account_id = s.account_id
JOIN worldwright_characters AS c
  ON c.character_id = s.character_id
 AND c.account_id = s.account_id
 AND c.world_id = s.world_id
WHERE s.token_hash = $1
  AND s.world_id = $2
  AND s.revoked_at IS NULL
  AND s.expires_at > clock_timestamp()
  AND a.disabled = FALSE
```

Session revocation is idempotent:

```sql
UPDATE worldwright_sessions
SET revoked_at = COALESCE(revoked_at, clock_timestamp())
WHERE token_hash = $1
```

## Shared durable-operation prefix

Reward and transfer operations execute these steps inside one SQL transaction:

1. Lock and validate the writer row with the fencing query above.
2. Acquire a transaction-scoped advisory lock for the scoped request key:

```sql
SELECT pg_advisory_xact_lock(hashtextextended($1, 0))
```

The lock text is exactly `32:<world>|32:<account>|32:<character>|<request-byte-length>:<request>`, using lowercase 32-hex IDs and the validated ASCII request ID. The length prefix makes the final component unambiguous. Hash collisions only serialize unrelated requests.

3. Revalidate the current session and actor:

```sql
SELECT s.account_id
FROM worldwright_sessions AS s
JOIN worldwright_accounts AS a ON a.account_id = s.account_id
JOIN worldwright_characters AS c
  ON c.character_id = s.character_id
 AND c.account_id = s.account_id
 AND c.world_id = s.world_id
WHERE s.token_hash = $1
  AND s.account_id = $2
  AND s.character_id = $3
  AND s.world_id = $4
  AND s.revoked_at IS NULL
  AND s.expires_at > clock_timestamp()
  AND a.disabled = FALSE
FOR SHARE OF s, a, c
```

4. Lock and inspect an existing receipt:

```sql
SELECT operation_id, intent_kind, intent_fingerprint, outcome, result_version, result
FROM worldwright_operation_receipts
WHERE world_id = $1
  AND account_id = $2
  AND character_id = $3
  AND request_id = $4
FOR UPDATE
```

The same fingerprint returns the stored typed result with `replayed = true`. A different fingerprint returns `ReceiptConflict`. Corrupt kind/version/result data returns `InvalidData` and makes the world not ready.

Intent fingerprints use SHA-256 over this canonical binary encoding:

```text
"worldwright-durable-intent-v1\0"
for each component:
    4-byte unsigned big-endian byte length
    exact component bytes
```

Reward components are `claim_reward`, reward key, big-endian generation, big-endian credit amount, and item definition ID. Transfer components are `transfer_item`, the 16-byte item ID, and the recipient username. The request ID and trusted actor IDs scope the receipt key and are not repeated in the fingerprint.

## Claim reward transaction

The v0.1 server exposes only this server-defined reward:

```text
key: welcome
generation: 1
credits: 100
item definition: <content namespace>:items.welcome_token
```

Lock the actor's durable state:

```sql
SELECT balance, version
FROM worldwright_characters
WHERE character_id = $1
  AND account_id = $2
  AND world_id = $3
FOR UPDATE
```

Check for the business claim:

```sql
SELECT operation_id, credits, item_id
FROM worldwright_reward_claims
WHERE world_id = $1
  AND character_id = $2
  AND reward_key = $3
  AND completion_generation = $4
```

Check bounded inventory while the character row is locked:

```sql
SELECT COUNT(*)::BIGINT
FROM worldwright_item_instances
WHERE world_id = $1 AND owner_character_id = $2
```

For a new eligible claim, insert the generated item and update the balance/version:

```sql
INSERT INTO worldwright_item_instances (
    item_id,
    world_id,
    definition_id,
    owner_character_id
)
VALUES ($1, $2, $3, $4)
```

```sql
UPDATE worldwright_characters
SET balance = balance + $4,
    version = version + 1,
    updated_at = clock_timestamp()
WHERE character_id = $1
  AND account_id = $2
  AND world_id = $3
  AND balance <= 9223372036854775807 - $4
RETURNING balance, version
```

Then insert the receipt before the reward claim so its operation foreign key exists. Use the shared receipt/audit statements below, followed by:

```sql
INSERT INTO worldwright_reward_claims (
    world_id,
    character_id,
    reward_key,
    completion_generation,
    operation_id,
    credits,
    item_id
)
VALUES ($1, $2, $3, $4, $5, $6, $7)
```

A pre-existing claim creates a new committed `already_claimed` receipt with the existing item ID and current balance. Full inventory creates a committed `inventory_full` receipt. Neither changes value or emits an outbox event.

## Transfer item transaction

Resolve the target account/character inside the transaction:

```sql
SELECT c.character_id, c.character_name
FROM worldwright_accounts AS a
JOIN worldwright_characters AS c ON c.account_id = a.account_id
WHERE a.username = $1
  AND c.world_id = $2
  AND a.disabled = FALSE
```

A missing target creates a committed `target_not_found` receipt. A target equal to the actor creates a committed `same_owner` receipt.

Lock both character rows in UUID order:

```sql
SELECT character_id
FROM worldwright_characters
WHERE world_id = $1
  AND (character_id = $2 OR character_id = $3)
ORDER BY character_id
FOR UPDATE
```

Lock the item before inspecting recipient capacity:

```sql
SELECT owner_character_id, definition_id, version
FROM worldwright_item_instances
WHERE world_id = $1 AND item_id = $2
FOR UPDATE
```

A missing item or an item not owned by the actor produces the same committed `not_owned` outcome with no owner identity in the result. If the actor owns the item and the target is the actor, return `same_owner`. Only after ownership is established, check recipient inventory capacity:

```sql
SELECT COUNT(*)::BIGINT
FROM worldwright_item_instances
WHERE world_id = $1 AND owner_character_id = $2
```

A valid transfer updates ownership with an expected-version guard:

```sql
UPDATE worldwright_item_instances
SET owner_character_id = $3,
    version = version + 1,
    updated_at = clock_timestamp()
WHERE world_id = $1
  AND item_id = $2
  AND owner_character_id = $4
  AND version = $5
RETURNING version
```

The item update must return exactly one positive version; zero rows is invalid durable state. Update both affected character aggregate versions:

```sql
UPDATE worldwright_characters
SET version = version + 1,
    updated_at = clock_timestamp()
WHERE world_id = $1
  AND (character_id = $2 OR character_id = $3)
```

The character update must affect exactly two rows. A successful transfer creates an `item_transferred` receipt, audit row, and outbox row. Rejected outcomes create a receipt and audit row but no outbox event. `not_owned` and `target_not_found` results expose no current owner identity; `same_owner` and `inventory_full` may identify the unchanged actor owner.

## Receipt, audit, and outbox statements

Every newly decided durable operation inserts exactly one receipt:

```sql
INSERT INTO worldwright_operation_receipts (
    world_id,
    account_id,
    character_id,
    request_id,
    operation_id,
    intent_kind,
    intent_fingerprint,
    outcome,
    result,
    result_version
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 1)
```

Then insert one bounded audit summary:

```sql
INSERT INTO worldwright_audit_log (
    world_id,
    account_id,
    character_id,
    operation_id,
    request_id,
    action,
    outcome,
    summary
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
```

Successful value changes also insert one outbox record:

```sql
INSERT INTO worldwright_outbox (
    operation_id,
    world_id,
    topic,
    payload
)
VALUES ($1, $2, $3, $4)
```

All JSON values are constructed from typed server data. Passwords, raw session tokens, password hashes, and chat text are never included.

## Inventory reconstruction

An authorized inventory read reuses the session-validation query, then reads:

```sql
SELECT balance, version, room_definition
FROM worldwright_characters
WHERE character_id = $1
  AND account_id = $2
  AND world_id = $3
```

```sql
SELECT item_id, definition_id, version
FROM worldwright_item_instances
WHERE world_id = $1 AND owner_character_id = $2
ORDER BY item_id
LIMIT 257
```

More than 256 rows is `InvalidData`; the adapter never silently truncates authoritative inventory.

## Commit and error semantics

- Any error before `COMMIT` drops/rolls back the transaction and returns no success.
- A successful `COMMIT` returns a typed result and may then update the owner's in-memory non-authoritative display cache.
- An error returned by `COMMIT` is `UnknownOutcome`, not a definite failure. The owner stops durable admission and becomes not ready. Recovery uses a fresh connection and the scoped receipt query; it never retries the business effect under a new request ID.
- `StaleWriter`, `InvalidData`, and unresolved `UnknownOutcome` fail the durable world closed.
- Ordinary connection loss after a successful commit is recovered by resubmitting the same scoped request ID after authentication. The stored result is returned with `replayed = true`.
- There is no universal exactly-once delivery claim. Database constraints and receipts provide at-most-once business effects for the two implemented operations.

## Test contract

PostgreSQL integration tests are compiled but ignored unless explicitly run with `WORLDWRIGHT_TEST_DATABASE_URL`. The database name parsed from that URL must contain `worldwright_test`; otherwise tests fail before issuing schema or data statements. Each test creates a generated schema, sets that schema as its search path, runs migrations, and drops only that test-created schema during cleanup. The generated identifier is restricted to lowercase ASCII letters, digits, and underscore before interpolation; identifiers are never supplied by users. The harness uses only these dynamic statements:

```sql
CREATE SCHEMA "<generated_test_schema>"
SET search_path TO "<generated_test_schema>"
DROP SCHEMA "<generated_test_schema>" CASCADE
```

Test assertions may use only these additional scoped inspection statements:

```sql
SELECT COUNT(*)::BIGINT FROM worldwright_operation_receipts WHERE world_id = $1
SELECT COUNT(*)::BIGINT FROM worldwright_audit_log WHERE world_id = $1
SELECT COUNT(*)::BIGINT FROM worldwright_outbox WHERE world_id = $1
```

A session-expiry test may force only its own generated token to expire:

```sql
UPDATE worldwright_sessions
SET expires_at = clock_timestamp() - INTERVAL '1 second'
WHERE token_hash = $1
```

Required database cases are:

- Migration and exact-idempotent seed.
- Durable login, session lookup, expiry, revocation, and restart lookup.
- Writer acquisition and stale-writer rejection.
- Reward success, same-request replay, conflicting-intent rejection, fresh-request business duplicate, and inventory cap.
- Item transfer success, same-request replay, fresh-request non-owner rejection, self-transfer rejection, target-not-found rejection, and recipient inventory cap.
- Rollback before commit leaves no value/receipt/audit/outbox change.
- Restart reconstruction returns the committed balance/item owner and original receipt.
- Receipt, audit, and outbox cardinality matches the transaction result.

Until those ignored tests run against an approved disposable PostgreSQL instance, the implementation may be compile-verified but Milestone 4 database behavior must remain marked unverified.