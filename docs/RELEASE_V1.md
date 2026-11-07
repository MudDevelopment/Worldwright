## Studio publication continuity

Studio submits the exact immutable `.wwr` bytes with the
`worldwright-release-submission-v1` envelope. Its review context pins the
draft revision, payload digest, zero validation diagnostics, preview artifact
digest/time, and bounded change summary. The M6 server remains the grant,
expected-current, review, activation, and audit authority. A Studio submission
is a request, never approval or activation.

# Content Release v1

**Status:** Milestone 5 - Content Release and Preview v0.1 is implemented
awaiting live PostgreSQL verification. The artifact, packaging, inspection,
preview, and non-database paths are Windows-locally verified. Eight Milestone
5 database tests compile but have not run. This document is the authoritative
v1 release contract.

## Artifact container

A `.wwr` file is one bounded, non-archive byte container:

```text
WORLDWRIGHT-RELEASE-V1\n
<8 lowercase hexadecimal manifest-length bytes>\n
<canonical compact JSON manifest>
<exact original validated JSON payload bytes>
```

The payload is 1..=32 MiB. The manifest is 1..=16 KiB. The maximum artifact
size is the public `worldwright_release::MAX_ARTIFACT_BYTES` constant. There is
no archive extraction, path traversal, symlink extraction, decompression, or
plugin execution surface.

Packaging first validates the source with the strict content loader and then
preserves the source JSON bytes exactly. Identical source/options produce
byte-identical artifacts. `write_new` uses create-new semantics: identical
bytes are reused, different bytes return immutable-conflict exit code 3, and
existing files are never overwritten.

## Manifest v1

The canonical manifest has these fields and nesting:

```json
{
  "manifest_version": 1,
  "release_id": "reference-v1",
  "content_schema_version": 1,
  "game_compatibility": {
    "game_id": "worldwright-reference",
    "contract_version": 1
  },
  "payload": {
    "media_type": "application/vnd.worldwright.world+json",
    "byte_length": 123,
    "sha256": "64 lowercase hexadecimal characters"
  },
  "provenance": {
    "source_revision": "source-revision",
    "builder_identity": "builder"
  }
}
```

Manifest v1 supports release IDs, content schema 1, game ID
`worldwright-reference`, contract version 1, payload media type/length/digest,
and source revision/builder identity. The complete artifact has a separate
SHA-256 digest. A digest proves integrity and identity of bytes; it does not
prove authorship, approval, or deployment authority. v1 has no timestamp,
signature, or attestation.

## Developer commands

```powershell
worldwright content package examples/two-room/world.json `
  --output .\artifacts\reference-v1.wwr `
  --release-id reference-v1 `
  --source-revision working-tree `
  --builder local

worldwright content inspect .\artifacts\reference-v1.wwr
worldwright content inspect .\artifacts\reference-v1.wwr --json
worldwright preview .\artifacts\reference-v1.wwr
worldwright preview .\artifacts\reference-v1.wwr --max-seconds 60
```

Package and inspect errors exit 1, usage errors exit 2, and immutable
conflicts exit 3. Preview defaults to 900 seconds and has a hard maximum of
3600 seconds.

Preview re-verifies the exact artifact digest in a child process and runs only
the in-memory ReferenceGame path. It clears inherited environment state and
restores only sanitized `TMP`, `TEMP`, `TMPDIR`, and
`WORLDWRIGHT_PREVIEW` values. It has
no persistence configuration, production credentials, network service, or
database access. It allows at most four local workers and uses ownership-safe
temporary slots with 3665-second stale recovery. Preview is application
isolation, not an operating-system sandbox or network-egress firewall. Content
is data; executable plugins are not run.

Automated tests cover environment sentinel clearing, timeout cleanup, tamper
rejection, slot capacity, stale retry, and ownership-safe cleanup. No real
production database denial test or hosted/browser/manual production test has
run.

## PostgreSQL release workflow

PostgreSQL stores exact artifact bytes keyed by artifact SHA-256 and unique
release ID. It also stores publication requests, activation attempts and
audit, world active/known-good/pending pointers, and a maintenance flag.

Requests use these statuses:

```text
requested -> approved | rejected
approved  -> activating
activating -> activated | failed
```

The requester cannot approve its own request. Auth-v1 tokens, current database
grants, assurance, environment, and fixed-world scope authorize operations;
database credentials alone are insufficient. Submission and activation compare
the expected current digest; approval binds the exact stored artifact digest.
Production requests may be recorded and reviewed; activation requires the
auth-v1 production authorization path.

The release CLI uses fixed world ID 1:

```powershell
$env:WORLDWRIGHT_DATABASE_URL = "<secret PostgreSQL URL>"

worldwright-server database migrate `
  --database-url-env WORLDWRIGHT_DATABASE_URL
worldwright-server database seed `
  --database-url-env WORLDWRIGHT_DATABASE_URL `
  --content examples/two-room/world.json `
  --accounts accounts.local.json
worldwright-server release request `
  --database-url-env WORLDWRIGHT_DATABASE_URL `
  --operator-token-env WORLDWRIGHT_OPERATOR_TOKEN `
  --operator-key-env WORLDWRIGHT_OPERATOR_KEY `
  --operator-issuer worldwright --operator-audience worldwright-ops `
  --environment staging --reason "request staging release" `
  --artifact .\artifacts\reference-v1.wwr `
  --kind activate --target staging --expected-current none
worldwright-server release review `
  --database-url-env WORLDWRIGHT_DATABASE_URL `
  --operator-token-env WORLDWRIGHT_REVIEW_TOKEN `
  --operator-key-env WORLDWRIGHT_OPERATOR_KEY `
  --operator-issuer worldwright --operator-audience worldwright-ops `
  --environment staging --reason "staging approval" `
  --request <request-id> --decision approve `
  
worldwright-server release activate `
  --database-url-env WORLDWRIGHT_DATABASE_URL `
  --operator-token-env WORLDWRIGHT_OPERATOR_TOKEN `
  --operator-key-env WORLDWRIGHT_OPERATOR_KEY `
  --operator-issuer worldwright --operator-audience worldwright-ops `
  --environment staging --reason "activate reviewed release" `
  --request <request-id>
```

The request ID, activation ID, and writer ID are generated by the CLI. Status
and conflict errors are sanitized and do not print the database URL.

## Activation, recovery, and rollback

Activation is an offline maintenance command, not hot reload:

1. begin fences the old writer, sets maintenance and pending intent;
2. stored bytes, release identity, namespace/schema, durable room references,
   and the current welcome-token item convention are validated;
3. headless smoke runs Look -> Move -> Look using fixed nonzero test IDs;
4. complete updates active/known-good pointers, status, and audit in one
   transaction.

A missing entry-room destination fails smoke. Compatibility or smoke failure
marks the activation failed, clears pending/maintenance, and leaves the active
pointer unchanged. Commit uncertainty leaves pending maintenance for
`worldwright-server release recover`, which acquires a new writer and reruns
checks. Rollback is a separately reviewed request and accepts only the current
known-good digest.

A durable server starts only from the exact active artifact bytes. It refuses
no-active, pending, maintenance, invalid, or incompatible state. Shared release
row guards prevent activation crossing in-memory durable commands, and a
one-second authority monitor fences a running durable server. The operational
runbook still requires stopping the durable server before staging activation.

## Fixed v0.1 boundaries

- Fixed world ID 1; no multi-world selection.
- No hot reload.
- No content/data migration.
- No artifact signing.
- No production authorization; production activation is refused in code.
- No retention/deletion policy.
- No production readiness claim.
- Live PostgreSQL evidence is still required before calling Milestones 4 or 5
  complete.
