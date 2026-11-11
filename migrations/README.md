# Database Migrations

The first durable gameplay migration is `20261020000100_durable_gameplay.sql`. It creates the world, account, character, session, inventory, receipt, reward-claim, audit, and outbox tables used by `worldwright-persistence`.

`PostgresStore::migrate` applies the embedded SQLx migration explicitly. Normal durable startup should call `verify_migrations` and compare the embedded migration ledger without applying schema changes. Seed is exact-idempotent and transactional; writer epochs fence durable mutations.

PostgreSQL integration tests are ignored unless `WORLDWRIGHT_TEST_DATABASE_URL` is explicitly supplied and its parsed database name contains `worldwright_test`. Tests create and drop only a generated schema. No database is created or dropped by the harness, and no URL or credential is printed.

## Additive P1 timer migration — November 9, 2026

`20261118000400_gameplay_timer_audit.sql` adds the audit table for owner-driven
idle timer commits. Timer state/revision/audit writes share the existing
writer/release/state-version transaction fence; no player session or player
receipt is invented. Apply explicitly before updated durable startup. Existing
migration files remain immutable and normal startup only verifies the ledger.
Review old-binary rollback compatibility before deployment. The new migration
and timer rollback/reconstruction tests have not been executed here against PostgreSQL.

## R2 migration acceptance

R2 uses the complete current inventory: the original seven plus
`20261118000400_gameplay_timer_audit.sql`, eight immutable SQL files. The historical
R2 roadmap's seven-file wording has been corrected, not implemented by deleting
the timer migration. The isolated R2 migration test builds an old fixture from
exact original SQL/checksums, upgrades with the real migrator, and rejects excess/
future, checksum-mismatch and unsuccessful ledger entries. Only disposable
fixture tables/ledgers are altered; applied source migrations are never edited.
`r2-verify.py collect` records SHA-384 per SQL file, all ignored test discovery and
execution counts, and bounded run-specific schema cleanup. A configured test is
not a passed migration gate. See `docs/R2_ACCEPTANCE.md` for live requirements.

### Observed fresh-apply blocker

The unchanged-source PostgreSQL 16.11 preflight now fails at
`20261106000100_operational_control.sql:188`: unquoted
`authorization JSONB NOT NULL` is rejected as a reserved identifier (SQLstate
42601). The current-run transaction/schema is rolled back/dropped, and the suite
returns failure even though corrected expiry/proxy cases pass independently.
No source migration/checksum is edited by the preflight. Confirm applied history
and obtain an authorized immutable-migration/baseline repair decision before
repairing this blocker; a later additive migration cannot fix preceding syntax.
Actual evidence is in `docs/verification/r2-acceptance-ledger.md`.
