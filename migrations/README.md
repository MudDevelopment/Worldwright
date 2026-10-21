# Database Migrations

The first durable gameplay migration is `20261020000100_durable_gameplay.sql`. It creates the world, account, character, session, inventory, receipt, reward-claim, audit, and outbox tables used by `worldwright-persistence`.

`PostgresStore::migrate` applies the embedded SQLx migration explicitly. Normal durable startup should call `verify_migrations` and compare the embedded migration ledger without applying schema changes. Seed is exact-idempotent and transactional; writer epochs fence durable mutations.

PostgreSQL integration tests are ignored unless `WORLDWRIGHT_TEST_DATABASE_URL` is explicitly supplied and its parsed database name contains `worldwright_test`. Tests create and drop only a generated schema. No database is created or dropped by the harness, and no URL or credential is printed.
