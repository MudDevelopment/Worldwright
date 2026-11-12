# Release Activation Runbook

**Status:** Milestones 5 and 6 are implemented awaiting live PostgreSQL and
recovery execution. The commands below are staging examples for fixed world ID
1; production uses the same auth-v1 path only after its separate checklist.

## 1. Record state and stop the durable server

`release status` reports a publication request, not world active/known-good
pointers. Record current active and known-good release IDs/digests from an
approved change record or database operator view, and use `release status` for
the known request. Do not copy secret database URLs into logs.

Stop the durable server and confirm that no second durable writer is running.
Activation is not hot reload. A running server may hold the release read guard
needed by in-memory commands.

## 2. Prepare the database and accounts

```powershell
$env:WORLDWRIGHT_DATABASE_URL = "<secret PostgreSQL URL>"

worldwright-server database migrate `
  --database-url-env WORLDWRIGHT_DATABASE_URL

worldwright-server database seed `
  --database-url-env WORLDWRIGHT_DATABASE_URL `
  --content examples/two-room/world.json `
  --accounts accounts.local.json
```

`seed` is exact-idempotent and does not start a writer. The database must be
migrated and seeded before release submission.

## 3. Package, inspect, and preview

```powershell
New-Item -ItemType Directory -Force .\artifacts | Out-Null

worldwright content package examples/two-room/world.json `
  --output .\artifacts\reference-v1.wwr `
  --release-id reference-v1 `
  --source-revision working-tree `
  --builder local

worldwright content inspect .\artifacts\reference-v1.wwr
worldwright content inspect .\artifacts\reference-v1.wwr --json
worldwright preview .\artifacts\reference-v1.wwr
```

Inspect output supplies the artifact and payload digests. Preview is local,
in-memory, and has no persistence or production credentials. It is not an OS
sandbox or network-egress firewall.

## 4. Request and review staging activation

Use the artifact digest from inspection when recording the request. For an
initial world with no active release, use `none`; otherwise use the exact
current active digest.

```powershell
worldwright-server release request `
  --database-url-env WORLDWRIGHT_DATABASE_URL `
  --operator-token-env WORLDWRIGHT_OPERATOR_TOKEN `
  --operator-key-env WORLDWRIGHT_OPERATOR_SIGNING_KEY `
  --operator-issuer worldwright --operator-audience worldwright-ops `
  --environment staging `
  --artifact .\artifacts\reference-v1.wwr `
  --kind activate `
  --target staging `
  --expected-current none `
  --reason "request staging release"

worldwright-server release status `
  --database-url-env WORLDWRIGHT_DATABASE_URL `
  --operator-token-env WORLDWRIGHT_OPERATOR_TOKEN `
  --operator-key-env WORLDWRIGHT_OPERATOR_SIGNING_KEY `
  --operator-issuer worldwright --operator-audience worldwright-ops `
  --environment staging `
  --request <request-id>

worldwright-server release review `
  --database-url-env WORLDWRIGHT_DATABASE_URL `
  --operator-token-env WORLDWRIGHT_REVIEW_TOKEN `
  --operator-key-env WORLDWRIGHT_OPERATOR_SIGNING_KEY `
  --operator-issuer worldwright --operator-audience worldwright-ops `
  --environment staging `
  --request <request-id> `
  --decision approve `
  --reason "staging approval"
```

Approval is separate from submission. The requester cannot approve its own
request by string comparison. Record the returned durable request identifier in
the change record.

## 5. Activate and start

```powershell
worldwright-server release activate `
  --database-url-env WORLDWRIGHT_DATABASE_URL `
  --operator-token-env WORLDWRIGHT_OPERATOR_TOKEN `
  --operator-key-env WORLDWRIGHT_OPERATOR_SIGNING_KEY `
  --operator-issuer worldwright --operator-audience worldwright-ops `
  --environment staging `
  --request <request-id> `
  --reason "activate reviewed release"

worldwright-server serve `
  --database-url-env WORLDWRIGHT_DATABASE_URL `
  --acquire-writer `
  --operator-key-env WORLDWRIGHT_OPERATOR_SIGNING_KEY `
  --operator-issuer worldwright `
  --operator-audience worldwright-ops `
  --environment staging `
  --origin http://127.0.0.1:3000 `
  --allow-insecure-http
```

This is a development-only loopback example. Use an approved HTTPS edge and
canonical origin for a real environment. The server loads only the exact active
artifact from PostgreSQL. It refuses no-active, pending, maintenance, invalid,
or incompatible state. Check readiness after startup:

```powershell
Invoke-WebRequest http://127.0.0.1:3000/health/ready
```

Do not interpret a successful CLI command as live PostgreSQL evidence until the
guarded database tests have run against an approved disposable database.

## 6. Rollback

Rollback is a separate reviewed request targeting the current known-good
digest. It is not an arbitrary artifact selection.

```powershell
worldwright-server release request `
  --database-url-env WORLDWRIGHT_DATABASE_URL `
  --operator-token-env WORLDWRIGHT_OPERATOR_TOKEN `
  --operator-key-env WORLDWRIGHT_OPERATOR_SIGNING_KEY `
  --operator-issuer worldwright --operator-audience worldwright-ops `
  --environment staging `
  --artifact .\artifacts\known-good.wwr `
  --kind rollback `
  --target staging `
  --expected-current <current-active-digest> `
  --reason "request rollback"

worldwright-server release review `
  --database-url-env WORLDWRIGHT_DATABASE_URL `
  --operator-token-env WORLDWRIGHT_REVIEW_TOKEN `
  --operator-key-env WORLDWRIGHT_OPERATOR_SIGNING_KEY `
  --operator-issuer worldwright --operator-audience worldwright-ops `
  --environment staging `
  --request <request-id> `
  --decision approve `
  --reason "rollback approval"

worldwright-server release activate `
  --database-url-env WORLDWRIGHT_DATABASE_URL `
  --operator-token-env WORLDWRIGHT_OPERATOR_TOKEN `
  --operator-key-env WORLDWRIGHT_OPERATOR_SIGNING_KEY `
  --operator-issuer worldwright --operator-audience worldwright-ops `
  --environment staging `
  --request <request-id> `
  --reason "activate rollback"
```

Production activation is allowed only through auth-v1 authorization, separate
review, current MFA-backed grants, and the future production checklist. Live
PostgreSQL and recovery evidence remain pending.

## 7. Interrupted activation and recovery

If a process stops after begin, do not clear maintenance or pending state by
hand. Inspect durable status, confirm the old server is stopped, and recover:

```powershell
worldwright-server release recover `
  --database-url-env WORLDWRIGHT_DATABASE_URL `
  --operator-token-env WORLDWRIGHT_OPERATOR_TOKEN `
  --operator-key-env WORLDWRIGHT_OPERATOR_SIGNING_KEY `
  --operator-issuer worldwright --operator-audience worldwright-ops `
  --environment staging --reason "recover interrupted activation"
```

Recovery acquires a new writer, reloads the exact pending artifact, reruns
compatibility and Look -> Move -> Look smoke, and completes or fails the
activation. If commit outcome is uncertain, retain maintenance and investigate
before retrying. Never claim success from a command that returned an error.

## R5 Studio status and recovery handoff

Studio remains a request/status client, not a review or activation console.
Its known-ID reconciliation control only fetches an existing authorized request
and validates the local immutable artifact/revision/original preview context.
Obtain the exact request ID through the approved operator/audit workflow after
a lost response; never submit a duplicate because Studio timed out. Unknown IDs
cannot be discovered by this control. Corrupt local records require evidence
preservation and authorized repair, not status editing.

A locally recovered or remotely listed `activated` request does not establish
which release currently runs. Continue this runbook's active-state/readiness,
expected-current, independent reviewer and rollback checks. No Studio recovery
bypasses current grants, maintenance, database fences or migration compatibility.

For crashed Studio sessions, use [R5 recovery](../R5_ACCEPTANCE.md) before
restarting the same workspace. Keep the permanent marker and journals intact;
only the operational session sentinel may be removed after all writers are
confirmed stopped and evidence preserved. Full execution matrix:
[R5_CHECK_PLAN.md](../R5_CHECK_PLAN.md). Native/live/independent gates remain open.
