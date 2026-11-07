# Backup and Restore Runbook

**Status:** M6 implementation awaiting live PostgreSQL/recovery execution.
Do not interpret this runbook as production readiness.

## Backup

Use a PostgreSQL service name configured by the operator. It must identify the
same database represented by the confirmed control context.

```powershell
$env:WORLDWRIGHT_DATABASE_URL = 'postgres://...'
$env:WORLDWRIGHT_OPERATOR_TOKEN = '<short-lived-token>'
$env:WORLDWRIGHT_OPERATOR_SIGNING_KEY = '<operator-token-signing-key>'
$env:WORLDWRIGHT_BACKUP_SIGNING_KEY = '<independent-backup-manifest-key>'
cargo run -p worldwright-server --locked --offline -- backup create `
  --database-url-env WORLDWRIGHT_DATABASE_URL `
  --operator-token-env WORLDWRIGHT_OPERATOR_TOKEN `
  --operator-key-env WORLDWRIGHT_OPERATOR_SIGNING_KEY `
  --operator-issuer worldwright `
  --operator-audience worldwright-ops `
  --environment staging `
  --reason 'scheduled backup' `
  --pg-service worldwright `
  --output .\backup.dump `
  --backup-signing-key-env WORLDWRIGHT_BACKUP_SIGNING_KEY
```

The command begins a durable backup job, runs `pg_dump --format=custom
--no-owner --no-privileges` directly, hashes the output, writes a signed
`.manifest.json` sidecar with create-new semantics, and completes the job.
Tools receive only approved process environment variables and never receive
tokens, keys, or database URLs as arguments.

## Verify

```powershell
cargo run -p worldwright-server --locked --offline -- backup verify `
  --input .\backup.dump `
  --manifest .\backup.dump.manifest.json `
  --backup-signing-key-env WORLDWRIGHT_BACKUP_SIGNING_KEY
```

Verification streams the dump hash, checks the signed manifest and exact digest,
and performs no database operation.

## Restore

Restore requires a separate empty target database. The confirmation text must
equal the target database name parsed from its SQLx connection configuration.

```powershell
$env:WORLDWRIGHT_CONTROL_DATABASE_URL = 'postgres://...'
$env:WORLDWRIGHT_TARGET_DATABASE_URL = 'postgres://...'
cargo run -p worldwright-server --locked --offline -- backup restore `
  --control-database-url-env WORLDWRIGHT_CONTROL_DATABASE_URL `
  --target-database-url-env WORLDWRIGHT_TARGET_DATABASE_URL `
  --operator-token-env WORLDWRIGHT_OPERATOR_TOKEN `
  --operator-key-env WORLDWRIGHT_OPERATOR_SIGNING_KEY `
  --operator-issuer worldwright `
  --operator-audience worldwright-ops `
  --environment staging `
  --reason 'restore verification' `
  --input .\backup.dump `
  --manifest .\backup.dump.manifest.json `
  --backup-signing-key-env WORLDWRIGHT_BACKUP_SIGNING_KEY `
  --pg-service worldwright `
  --confirm-empty-target worldwright_target
```

The command runs the exact empty-world assertion, closes the target pool,
invokes `pg_restore --exit-on-error --single-transaction --no-owner
--no-privileges`, reconnects, verifies migrations and M5 world compatibility,
and reports measured RPO/RTO before `restore verified; takeover not performed`.
It never establishes writer authority.

## Takeover

After independently isolating old writers:

```powershell
cargo run -p worldwright-server --locked --offline -- recovery takeover `
  --database-url-env WORLDWRIGHT_TARGET_DATABASE_URL `
  --operator-token-env WORLDWRIGHT_OPERATOR_TOKEN `
  --operator-key-env WORLDWRIGHT_OPERATOR_SIGNING_KEY `
  --operator-issuer worldwright `
  --operator-audience worldwright-ops `
  --environment staging `
  --reason 'old writer isolated' `
  --confirm-old-writers-isolated
```

Only takeover rotates the authority incarnation and establishes a new writer
lease. The old-writer isolation decision is an operator responsibility.
