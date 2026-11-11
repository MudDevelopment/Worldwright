# R2 — live PostgreSQL, gameplay and identity acceptance

Authority: ROADMAP_v2.md R2. Ubuntu 24.04 LTS is the acceptance baseline;
native Windows R0 evidence remains required. Implementation, helper tests,
controlled fixtures, actual provider observations and authorized adoption are
separate evidence classes. No command here closes R1 or approves distribution.

## Complete matrix

| Requirement | Executable coverage | Acceptance gate |
|---|---|---|
| Migration identities | `database_tests/r2.rs` fresh/exact-source upgrade/future/checksum/failed ledger; existing migrations suite | Complete current eight-file inventory and SQLx checksums; never modify applied SQL |
| Existing PostgreSQL suite | `r2-verify.py collect` discovers and executes every ignored persistence test | Exact discovered/executed count, no required skips, database/role/engine recorded |
| Both game-state contracts | `r2_both_game_contracts_state_replay_conflict_and_authority`, concurrency and rollback tests | State, rooms, counters, revisions, receipt identity, stale writer/version/session fencing and reconstructed store |
| Real failure/recovery | `r2-live.py process` + `r2-wire.py` + test-only `r2_fixture_process` | Kill before forwarded COMMIT; kill after backend CommandComplete; lost COMMIT acknowledgement; no speculative output, owner not ready on uncertainty, receipt/audit and independent entitlement cardinality |
| Controlled identity | `r2-provider.py`, signed Rust vectors, `r2-live.py controlled` | Real HTTPS discovery/JWKS/token exchange/S256; issuer/audience/nonce/state/signature/expiry, rotation, denied/reused/expired invite, revocation/expiry/disabled/outage/restart |
| Approved identity | `r2-approved.py`, provider-specific controlled exercises and signed owner evidence | Actual approved HTTPS provider and deployment; full matrix; independently provisioned Ed25519 approval key |
| Browser resume | Actual Chromium callback, command, cookie-only GET probe, reload, logout | Playable attachment without a password POST, HttpOnly cookie not exposed, rejected callbacks never mint a cookie |
| Idle clock/RNG | Lock-latency DB test, timer rollback test, real overdue idle restart, owner RNG unit | Equal deadlines, persistence latency not extending deadlines, expired schedules applied once, rejected/replayed candidates rewind RNG, no constant fallback |

## Native/live collection

Use a complete checkout and the pinned Rust 1.85.1 toolchain. Bootstrap is an
explicit prerequisite, never an automatic compiler substitution. Install the
pinned helper requirements into a virtual environment. Use an existing Chromium
executable; missing browser/tooling is failure, not a successful skip.

```bash
python3 -m venv /var/tmp/worldwright-r2-venv
/var/tmp/worldwright-r2-venv/bin/python -m pip install -r scripts/r2-requirements.txt -r scripts/r1-requirements.txt -r deploy/helm/worldwright/tests/requirements.txt
source /var/tmp/worldwright-r2-venv/bin/activate
bash scripts/dev.sh bootstrap
python scripts/test-r2.py
python scripts/test-r2-live.py
python scripts/r0-verify.py --output /var/tmp/worldwright-r2-r0
python scripts/r2-verify.py collect --r0-report /var/tmp/worldwright-r2-r0/r0-report.json --browser-executable /usr/bin/google-chrome --output /var/tmp/worldwright-r2-live
```

Set `WORLDWRIGHT_TEST_DATABASE_URL` privately before running. Its actual database
name must contain `worldwright_test`. The subprocess driver additionally requires
loopback PostgreSQL and no caller-supplied URL query options; it generates its own
unique `ww_r2_` schema/search path and uses plaintext only on this isolated local
fault-proxy hop. This is NOT permission to disable TLS for deployment databases.
Use a disposable role authorized to create/drop only test schemas. Never run this
against a production database. Test credentials, codes and cookies must not be
pasted into chat or uploaded as evidence.

All output directories must be new and outside the checkout. R0 now records the
production server binary in addition to the CLI and Studio. Collection checks the
same source/lock tuple and production binary digest before running. The Rust
fixture executable is separately discovered from Cargo JSON and separately hashed.
It uses real server/store/owner code under `cfg(test)`, not a mock adapter, but it
is not represented as the packaged production executable. The only extra trust
hook is a fixture CA compiled into test code; production builds cannot read it.

Collection runs formatting, check, Clippy, all ignored PostgreSQL tests, all owner
unit/network tests, browser regression helpers, harness tests and both live
drivers. Required tests are discovered before execution. The deliberately ignored
fixture entry point runs as a subprocess through the driver, rather than being
counted as a passed ordinary unit test. Reports remain `collected_not_closed`.

The proxy intercepts the actual SQLx 0.8.6 simple-query COMMIT path. If that path
changes or interception does not occur, the case fails. Backend CommandComplete is
withheld before it can reach the owner; an independent connection verifies the
transaction outcome after killing/restarting the actual fixture process. An
admission/Accepted message is not a successful gameplay result.

Run-specific schema-created markers are retained even when a phase times out.
The collector kills the subprocess tree, cleans only current-role schemas actually
created by that run and absent from its baseline, and fails acceptance if cleanup
was needed. It never drops arbitrary pre-existing `ww_test_` or `ww_r2_` schemas.
Retain failed reports too. There is no overwrite-and-relabel path to acceptance.

## Controlled OIDC versus approved-provider adoption

The controlled provider generates ephemeral RSA signing keys and a one-day local
CA, enforces exact client/redirect/code/S256 binding, and rotates signing keys.
Its runtime fault modes are test-harness objects, not production HTTP/admin routes.
Its private key and certificate files are deleted at teardown. Rust validates its
TLS chain using the test-only CA. The fixture Chromium context accepts its local
self-signed chain; this exception is confined to controlled-fixture testing.

The approved-provider observer uses a normal headed browser with TLS verification
ON. It handles no passwords/MFA secrets and does not capture browser storage,
authorization URLs, tokens or codes. The operator uses the existing OIDC form with
a pre-seeded approved account and, for first login, an invite entered in the
browser. Configure the real service separately with its actual approved issuer,
client, secret environment variable and HTTPS callback.

```bash
python scripts/r2-approved.py --origin https://worldwright-test.example.com --issuer https://identity-test.example.com --deployment-id approved-test-deployment --server-artifact /var/tmp/worldwright-r2-r0/native-target/x86_64-unknown-linux-gnu/debug/worldwright-server --output /var/tmp/worldwright-approved-observation --browser-executable /usr/bin/google-chrome
```

The domains above are usage examples, NOT configured or approved providers. Use
actual authorized values. This command produces `observed_not_adopted`, with only
the browser case; it deliberately cannot manufacture the full issuer/key/link/
revocation/outage matrix. The identity/service owner must exercise and review all
six provider-case groups, retain the `.log`, and supply a complete approved-provider
report with exact source/lock/server digests and named reviewer.

Authenticated adoption requires `adoption` fields: `decision: approved`,
`policy_reference`, `deployment_id`, and future timezone-qualified `expires_at`.
The owner signs the entire JSON object excluding `approval_signature`, serialized
with sorted keys, compact separators, UTF-8 and `ensure_ascii=False`. Store its
Ed25519 signature as base64 in `approval_signature`. Provision the public key
independently through the organization's trusted owner/key process. Do not trust
a public key supplied alongside an otherwise untrusted report; the verifier does
not establish the authority of a key on its own. Signing keys stay outside this
repository, tooling and chat. No key-generation/signing shortcut is provided.

## Final same-tuple closure check

```bash
python scripts/r2-verify.py close --collection /var/tmp/worldwright-r2-live/r2-collection.json --ubuntu-r0 /var/tmp/worldwright-r2-r0/r0-report.json --windows-r0 /var/tmp/worldwright-windows-r0/r0-report.json --process-report /var/tmp/worldwright-r2-live/process/process.json --controlled-report /var/tmp/worldwright-r2-live/controlled/controlled-provider.json --provider-report /var/tmp/worldwright-approved-observation/approved-provider.json --approval-public-key /var/tmp/trusted-worldwright-provider-owner.pem
```

The ordinary observation report fails this check until full real-provider evidence
and authenticated owner adoption exist. Closure rejects missing/duplicate phases,
missing/changed logs and child logs, mismatched current source/lock/binary/fixture,
changed migrations, non-Ubuntu collection, missing cleanup, incomplete provider
cases and expired/invalid adoption signatures. Current Windows R0 logs must be
transported intact. Older local Windows reports do not verify accumulated edits.

CI: `.github/workflows/r2-acceptance.yml` configures current Ubuntu R0 then live
R2 collection, retaining private JSON/logs on either GitHub hosting surface. It has
no production secrets, provider adoption, package publication or automatic closure.
Windows native gates remain in the existing Windows workflows. Configured CI is
not an observed hosted pass.

## Recovery policy and current evidence

Multiplayer polls idle work independently every 100 ms and commits timer state/audit
before visibility. Database wall-clock anchors are sampled once per candidate;
validation/pool/lock elapsed time is subtracted. Recovery orders schedules by
absolute deadline then ordinal, samples one stable statement timestamp, and clamps
overdue work to Tick zero. The database wall clock is authoritative across downtime;
large administrator clock steps can advance/delay deadlines, so synchronized clock
health and any deliberate clock-step tests must be reviewed for the environment.
The terminal host remains command-driven; it is not the multiplayer timer loop.

The gameplay PRNG is process-local, securely seeded and deliberately reseeded on
restart. Committed outcomes live in durable counters/receipts. Rejected candidates
and receipt replay restore the candidate RNG checkpoint; entropy failure never
falls back to a constant. This does not promise a serialized PRNG stream, replay
of all historical commands, deterministic future rolls after restart, or
cryptographic security of the gameplay xorshift stream.

See `verification/r2-acceptance-ledger.md` and retained local observations for the
actual current status. Helper/TLS/proxy checks do not substitute for compiled Rust,
real PostgreSQL, complete current-host CI or approved-provider human adoption.

## Expiry-fixture correction and mandatory SQL preflight

The session/invite lifetime CHECK remains `expires_at > created_at`. Expired
fixtures move `created_at` to two seconds before a stable statement timestamp and
`expires_at` to one second before it. The browser expiry UPDATE touches only
Alice's unrevoked sessions. The older durable-session test and retention fixture
are audited/corrected too. The native regression explicitly requires the old
invalid setup to raise PostgreSQL CHECK violation 23514 before testing corrected
expired session/invite rejection without new identity/session/account/audit data.

`python scripts/test-r2-expiry-sql.py` is a supplementary real PostgreSQL check,
with the same loopback disposable URL guard. It extracts the actual Python/Rust
fixture SQL rather than independently copying a corrected version, tests scoped
expiry and revocation preservation, and exercises COMMIT gates against real
PostgreSQL through the fault proxy. Authentication cases use only the exact two
source migrations needed for session/invite constraints; a SEPARATE mandatory test
applies the complete unchanged eight-file inventory. Running the default suite
never treats that separate migration failure as a successful authentication test.
This is not compiled Rust/SQLx/current-owner/provider evidence.

Optional `WORLDWRIGHT_R2_SQL_REPORT` names a new JSON output outside the checkout.
The report distinguishes pass counts, errors and scope and always has
`full_r2_closed: false`. CI retains this SQL preflight's JSON/log before R0; the
R2 collector also requires an `expiry-fixture-sql` phase. No successful required
skip or missing phase can close the track.

The November 10 local PostgreSQL 16.11 run reports eight passed SQL/proxy tests
and one full-migration error: `20261106000100_operational_control.sql:188` declares
unquoted `authorization JSONB NOT NULL`, which raises syntax error 42601. The
migration source was not rewritten or preprocessed. A later additive migration
cannot repair syntax encountered before it executes. Applied history and an
approved immutable-migration/baseline repair decision are required; see the
ledger and retained failure report. Current source must not be advertised as
fresh-database/R2 accepted while that error remains.
