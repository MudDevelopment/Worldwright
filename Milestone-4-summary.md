# Milestone 4 Summary - Durable Gameplay v0.1

## Status

**Milestone 4 is implemented awaiting live PostgreSQL verification.**

The repository now contains a substantial PostgreSQL-backed durable gameplay
slice, including its schema, persistence adapter, server integration, protocol
messages, browser commands, and a guarded database test suite. The Rust code
compiles and the non-database tests pass.

The exit gate is not proven because PostgreSQL tests have not run against a
live database. Current evidence is 200 test-binary tests passed, 0 failed, 16
ignored, plus 2 doctests passed separately. The 16 ignored tests split into
eight M4 and eight M5 database tests and were compiled, not executed.

Implementation was introduced on the `milestone-4` branch in commit
`0d6f2bc` and is present at the current merge commit `9558dd5`.

## Product Decisions Made

This increment selected the following concrete boundaries:

- PostgreSQL owns durable local accounts, characters, sessions, balances,
  inventories, reward claims, operation receipts, audit records, and outbox
  work.
- Existing JSON-registry local mode remains available for database-free local
  development.
- Durable mode is opt-in and requires an explicitly named database URL
  environment variable plus `--acquire-writer`.
- PostgreSQL serve loads the exact active artifact; it does not accept
  `--content`.
- The first valuable operations are a one-time `welcome` reward and an item
  transfer.
- The `welcome` reward grants 100 credits and one namespaced welcome-token
  item.
- Room movement remains ephemeral. The durable room reference is validated at
  startup, but Move does not write a new room to PostgreSQL.
- The first durable slice uses fixed world ID
  `00000000000000000000000000000001`.
- PostgreSQL transactions fail closed on stale writer authority, invalid
  durable data, or an uncertain commit outcome.
- M5 artifact publication and staging activation are prerequisites described in
  [docs/RELEASE_V1.md](docs/RELEASE_V1.md) and
  [docs/operations/RELEASE_ACTIVATION.md](docs/operations/RELEASE_ACTIVATION.md).

## What Was Implemented

### 1. Schema and Migration

`migrations/20261020000100_durable_gameplay.sql` defines:

- `worldwright_worlds` for content identity and active writer epoch.
- `worldwright_accounts` and `worldwright_characters`.
- `worldwright_sessions`, storing only token hashes.
- `worldwright_item_instances` with exclusive character ownership.
- `worldwright_operation_receipts` for durable replay and conflict detection.
- `worldwright_reward_claims` for one grant per entitlement/generation.
- `worldwright_audit_log`.
- `worldwright_outbox`.

The schema uses foreign keys, uniqueness constraints, nonzero UUID checks,
bounded text checks, nonnegative value checks, and ownership/world consistency
constraints. SQLx migrations are an explicit operator action; ordinary server
startup verifies migrations but does not apply them.

### 2. Persistence Adapter

`crates/worldwright-persistence` now provides a typed `PostgresStore` with:

- Bounded PostgreSQL pooling and connection setup.
- Five-second statement and pool-acquisition timeouts.
- Two-second lock timeout.
- Ten-second idle-in-transaction timeout.
- Migration application and exact migration-ledger verification.
- Insert-only or exact-idempotent world/account/character seeding.
- Durable identity lookup, session creation, session lookup, expiry, and
  revocation.
- Writer acquisition with a monotonically increasing epoch.
- Inventory reconstruction.
- Transactional reward claim and item transfer operations.
- Typed persistence outcomes and sanitized error boundaries.

Worldwright's typed 128-bit identifiers map exactly to PostgreSQL UUIDs. Session
cookies retain random bearer tokens, while PostgreSQL stores only SHA-256 token
hashes.

### 3. Writer Fencing

Durable mutations lock the world authority row and verify both:

- The expected writer epoch.
- The expected active writer ID.

The fence is checked inside the same transaction as the business mutation.
Taking over a world increments the epoch, so a process retaining an older lease
cannot start a later valid durable write.

This is implemented in source but has not yet been exercised against real
PostgreSQL contention.

### 4. Durable Reward

`claim welcome` performs one fenced transaction that:

1. Validates the authenticated account, character, world, and live session.
2. Serializes the scoped request.
3. Resolves an existing receipt or rejects changed intent.
4. Enforces one reward per character, reward key, and generation.
5. Checks the bounded inventory limit.
6. Creates the item and adds credits when eligible.
7. Writes the terminal receipt and audit row.
8. Writes an outbox row when value is granted.
9. Commits before returning success.

A same-key/same-intent retry returns the stored operation and item identity with
`replayed: true`. A different request key cannot bypass the reward-claim
business constraint.

### 5. Durable Item Transfer

`give <item-id> <username>` performs a fenced, authenticated transfer with
typed outcomes for:

- Successful transfer.
- Item not owned by the caller.
- Transfer to the current owner.
- Target not found.
- Recipient inventory full.

Successful ownership change, receipt, audit, and outbox records share one
transaction. Replays return the stored result rather than applying ownership a
second time.

### 6. Receipts, Audit, and Outbox

Receipts are scoped by world, account, character, and request ID. They contain:

- A server-generated operation ID.
- An intent kind.
- A versioned canonical SHA-256 intent fingerprint.
- A terminal outcome.
- A typed JSON result.

The implementation distinguishes request deduplication from business
uniqueness. Receipt uniqueness prevents reprocessing one key; reward and item
constraints prevent duplicate value through a different key.

Required audit insertion occurs in the business transaction. Outbox insertion
also occurs in that transaction for committed value changes. An outbox
dispatcher is not part of this increment.

### 7. Server, Protocol, and Browser Integration

The server now has explicit local and PostgreSQL storage modes. Durable startup:

1. Loads and validates content.
2. Connects to PostgreSQL.
3. Verifies the migration set.
4. Verifies the durable world's content namespace and schema version.
5. Acquires a new writer epoch.
6. Loads durable identities.
7. Requires every durable room definition to resolve in current content.
8. Builds the world owner and only then opens the listener.

Protocol DTOs and validation were added for:

- `inventory`
- `claim_reward`
- `transfer_item`
- Inventory snapshots.
- Reward results.
- Transfer results.
- Durability, writer-fence, and uncertain-operation errors.

The browser accepts:

- `inventory`
- `claim welcome`
- `give <item-id> <username>`

Durable operations remain inside the single world-owner task. A database wait
therefore serializes the world for the bounded duration of that operation. This
is a deliberate simple first slice, not a scalability claim.

### 8. Operator Commands

The intended durable setup flow is:

```powershell
$env:WORLDWRIGHT_DATABASE_URL = "<secret PostgreSQL URL>"

cargo run -p worldwright-server -- database migrate `
  --database-url-env WORLDWRIGHT_DATABASE_URL

cargo run -p worldwright-server -- database seed `
  --database-url-env WORLDWRIGHT_DATABASE_URL `
  --content examples/two-room/world.json `
  --accounts accounts.local.json

cargo run -p worldwright-server -- serve `
  --content examples/two-room/world.json `
  --database-url-env WORLDWRIGHT_DATABASE_URL `
  --acquire-writer `
  --origin http://127.0.0.1:3000 `
  --allow-insecure-http
```

The database URL value is read from the named environment variable and is not
accepted directly as a command-line value.

## Failure Semantics

- An error before commit rolls back and returns no durable success.
- A commit error is classified as `UnknownOutcome`; it is not reported as a
  definite failure.
- `StaleWriter`, invalid durable data, and unresolved `UnknownOutcome` stop
  durable admission and make the world not ready.
- A PostgreSQL operation error disconnects active routes with a temporary
  unavailability reason rather than inventing local success.
- A stored receipt is validated before replay; malformed or contradictory
  durable results fail closed.
- Inventory reads reject more than 256 rows rather than silently truncating
  authoritative state.

There is not yet a complete reconciliation surface for an uncertain operation.
The current behavior stops durable admission; operational recovery and
client-visible reconciliation still need to be completed.

## Verification Evidence

The current Windows verification reported:

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | Passed |
| `cargo check --workspace --all-targets` | Passed |
| Server Clippy, all targets/features, warnings denied | Passed |
| `worldwright-protocol` tests | 16 passed |
| `worldwright-server` library tests | 38 passed |
| `worldwright-server` smoke tests | 2 passed |
| `worldwright-persistence` non-database tests | 6 passed |
| Test-binary workspace total | 200 passed, 0 failed, 16 ignored |
| Doctests | 2 passed separately |
| `git diff --check` before this summary | Passed |

No PostgreSQL server or Docker runtime was available. Consequently:

- No migration was applied to a live PostgreSQL instance.
- No SQL query, constraint, lock, timeout, or transaction behavior was
  runtime-verified.
- No live two-writer contention or stale-writer test ran.
- No database-backed server login/reward/transfer/restart flow ran.
- No commit-ambiguity behavior was induced against PostgreSQL.

Historical `docs/VERIFICATION.md` totals are not included in current totals.

## Authored PostgreSQL Test Coverage

Eight M4 ignored tests exist for:

1. Migration and exact-idempotent seed.
2. Session lookup across reconstructed stores, expiry, and revocation.
3. Writer acquisition and stale-writer rejection.
4. Reward replay, changed-intent conflict, business duplicate, and capacity.
5. Transfer replay, not-owned, self-transfer, missing target, and capacity.
6. Pre-commit fault rollback.
7. Restart inventory reconstruction.
8. Receipt, audit, and outbox cardinality.

Eight M5 ignored tests cover artifact/release identity, review separation,
expected-current conflicts, activation/recovery/failure, rollback, audit
cardinality, and release read guards.

The harness requires `WORLDWRIGHT_TEST_DATABASE_URL`, rejects database names
that do not contain `worldwright_test`, creates a generated per-test schema,
sets that schema as the search path, and drops only that generated schema.

These are useful acceptance tests, but authored and compiled tests are not
evidence that PostgreSQL behavior passed.

## Known Gaps and Risks

### Must Resolve Before Calling Milestone 4 Complete

1. **Run the real PostgreSQL suite.** The milestone's core claims concern SQL
   constraints, transaction rollback, advisory locks, row locks, and fencing;
   compilation cannot prove them.
2. **Separate durable operation identity from stream request identity.** The
   current server uses protocol v1 `request_id` as the durable receipt key.
   `ROAD-MAP.md` explicitly requires a durable key distinct from stream-local
   correlation. This contract must be reconciled before clients depend on
   restart/reconnect retry.
3. **Complete backend-unavailable authentication semantics.** PostgreSQL auth
   failures currently collapse into `AuthError::Internal`, and periodic session
   checks can treat a database outage like session revocation. Add an explicit
   unavailable state and consistently map it to HTTP 503 or protocol temporary
   unavailability.
4. **Add owner behavior tests.** There are no deterministic mock-store tests
   proving `accepted` ordering, reward/inventory/transfer result mapping,
   receipt conflict handling, session expiry, and fail-closed world shutdown.
5. **Add a database-backed server acceptance test.** Exercise migrate, seed,
   start, login, WebSocket attach, reward, lost/retried request, transfer,
   restart, inventory reconstruction, and stale-writer refusal through the
   server boundary.
6. **Complete uncertain-outcome recovery.** The adapter classifies commit
   ambiguity correctly, but there is no complete protocol/client/operator path
   to reconcile the original durable key and reopen admission safely.

### Important Cleanup

- Decide and document the supported PostgreSQL versions and isolation
  assumptions.
- Decide whether outbox dispatch belongs in this milestone or remains deferred;
  rows are durable, but nothing currently delivers them.
- Record an operational recovery procedure for a world left not-ready after
  stale authority, invalid data, or uncertain commit.
- Retain the explicit limitation that movement, chat, runtime attachments, and
  most simulation state are not durable.

## Recommended Completion Order

1. Define a first-class durable operation key and retry/reconciliation contract.
2. Fix authentication outage classification and live-connection behavior.
3. Add mock durable-owner tests for deterministic server behavior.
4. Provision an approved disposable PostgreSQL test database.
5. Run and fix all eight guarded persistence tests.
6. Add and run the full database-backed server restart/retry acceptance case.
7. Run the complete workspace formatting, check, Clippy, test, build, and
   rustdoc gate.
8. Update all status and protocol documentation with exact PostgreSQL,
   platform, toolchain, command, and test-count evidence.
9. Mark Milestone 4 implemented only when the road-map exit assertion is
    demonstrated: one valuable operation survives retry and restart without
    duplicate value, false success, or an accepted stale writer.

## Final Assessment

Milestone 4 has moved beyond a placeholder database connection. It has a
coherent schema, typed adapter, transactional valuable-state flows, writer
fencing, receipts, audit/outbox writes, durable sessions, startup
reconstruction, and a browser-visible vertical slice.

The remaining work is evidence and boundary correctness, not merely polish.
Until live PostgreSQL execution, durable-key semantics, outage handling, and
server-level restart/retry acceptance are complete, the accurate status is:

**Durable Gameplay v0.1 foundation implemented; Milestone 4 exit gate not yet
proven.**
