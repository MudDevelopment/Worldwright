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
