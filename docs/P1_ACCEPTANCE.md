# P1 implementation and acceptance handoff

Updated November 9, 2026. Current priorities: ROADMAP_v2.md. Progress/evidence:
verification/p1-implementation-ledger.md. P1 is not fully accepted or production-ready.

## Native validation first

Run from the actual repository with approved Rust 1.85.1, registry artifacts and
native linker. Resolve the FIRST failure before interpreting later results.

```bash
bash scripts/dev.sh bootstrap
cargo fmt --all
bash scripts/dev.sh check-native
node scripts/test-expedition-browser.cjs
node scripts/test-studio-capabilities.cjs
```

Windows retains PowerShell 7 and its independent native gate:

```powershell
./scripts/dev.ps1 bootstrap
rustup run 1.85.1 cargo fmt --all
./scripts/dev.ps1 check-native
node scripts/test-expedition-browser.cjs
node scripts/test-studio-capabilities.cjs
```

The generated-project integration test now relocates BOTH templates before
nested native build/test and runs their embedded-content executable outside the
project. It uses explicit target directories. A supplied trusted matching private
bundle is selected by WORLDWRIGHT_SDK_BUNDLE; this is not a registry cache or
license grant. Retain and review the project's initially generated Cargo.lock.

## Database gates and additive migration

Use an approved disposable database whose name contains worldwright_test. The
harness creates/drops generated schemas only and must never target production.
Supply WORLDWRIGHT_TEST_DATABASE_URL through approved secret delivery; do not
paste or retain credentials in reports.

```bash
cargo test -p worldwright-persistence --locked --offline -- --ignored --list
cargo test -p worldwright-persistence --locked --offline -- --ignored --test-threads=1
```

Required new tests include gameplay_browser_request_id_lifecycle,
timer_commit_fences_reconstructs_and_rolls_back_atomically,
oidc_link_concurrency_collision_revocation_and_restart, and
session_retention_preserves_receipts_and_active_sessions. CI verifies these names
exist before running EVERY ignored database test, not only the focused receipt case.

Apply 20261118000400_gameplay_timer_audit.sql through the existing explicit migrate
workflow before updated durable startup. Startup verifies the migration ledger;
it does not silently migrate. Preserve old migrations and review binary rollback
compatibility. Timer writes have a dedicated audit, not invented player receipts.

These tests do not substitute for network-process crash/unknown-COMMIT/restore
and provider acceptance. Exercise committed-but-undelivered results, delayed SQL,
old writer after takeover, overdue recovery after restart and real OIDC callbacks
under the deployed release. Retain exact artifact/binary/migration identities.

## Browser, preview and telemetry

Use real browser/Ubuntu desktop acceptance in addition to Node DOM fixtures:
valid cookie page reload attaches without POST login; anonymous/expired/revoked
sessions stay disabled; real OIDC callback reaches play without a second password;
logout/reconnect/late responses never re-enable stale gameplay. Concurrent
Set-Cookie races remain a separate security acceptance case.

Studio must show all 1024 permitted rooms, cap each room at 64 exits, respect UTF-8
byte boundaries, and remain keyboard/screen-reader usable at the limits. Backend
validation stays authoritative; capabilities/hints are not permission checks.

Preview acceptance must prove final output is drained before success receipt,
no stale output crosses runs, bounded busy-input errors, stop/crash/SIGTERM
cleanup and no supervisor/worker/orphan/blocked-pipe leak. Linux uses process
groups; Windows uses taskkill. Windows ACL, PID reuse and hostile same-user
processes still need review. No OS sandbox/egress confinement is supplied.

WORLDWRIGHT_TELEMETRY_BIND=127.0.0.1:9090 enables a separate local /metrics listener;
default is disabled, non-loopback and port zero are refused. Scraping must not
require enabling operator routes. Record fixed queue/owner/planning/SQL/delivery/
idle_tick histograms before changing completion or concurrency contracts.

## CI and fixed load evidence

ci.yml contains native Ubuntu/Windows, Node fixtures, real Ubuntu Studio, chart
and full ignored-PostgreSQL jobs. p1-acceptance.yml adds development container
CLI/linkage smoke and manual protected beta load. A CLI smoke pass is not a
running-service/deployed-container pass. The image config ID is not a registry
manifest digest; base image tags remain an unresolved distribution gate.

Configure worldwright-acceptance with REQUIRED REVIEWERS and acceptance-only
WORLDWRIGHT_ACCEPTANCE_CREDENTIALS_JSON. Run only against an approved disposable
HTTPS/PostgreSQL deployment with 250 provisioned identities. Dispatch the default
branch with run_beta_load=true and independently verified base_url,
server_revision, artifact_digest and environment_label. No pull request executes
this credentialed job. Credentials are removed and never uploaded.

The unchanged beta-250-v1 profile is 250 clients, 7500 completions, 2-second
per-client cadence, zero failures, p95 <=750ms, p99 <=1500ms, maximum <10s and
wall time <=90s. Do not tune down the dimensions or thresholds to obtain a pass.
The runner records operator-supplied metadata; independently verify the deployed
state before using its report as proof. GitHub.com uses pinned upload-artifact
v4; Enterprise uses pinned v3. Validate approved mirrors/runner compatibility.

## Retention and release authority

External outbox dispatch is unsupported. prune_expired_player_sessions accepts
1..1000 rows per call and erases expired hashes only. It is a trusted maintenance
API, not an unauthenticated HTTP endpoint or adopted automatic deletion policy.
Audit/receipt/OIDC/business uniqueness retention remains unchanged; no 90-day
purge promise is made. Broader retention requires named privacy/service approval.

scripts/release-evidence.py requires --output (new external directory),
--artifact-digest (64 lowercase hex), --container-digest (sha256:64 lowercase hex),
and optionally --decisions. It runs git and locked/offline Cargo metadata, emits
Cargo-declaration CycloneDX data and an exact release tuple, and returns failure
for missing tools/evidence/approvals or dirty source. It does not run an advisory
scanner, sign artifacts, prove authorship, or supply a license grant.

The decisions JSON has schema_version=1, the exact release tuple, named legal/
security/engineering/service owner approval references, and passed evidence files
for native-ubuntu, native-windows, database-identity, database-recovery,
browser-oidc, preview-tree, beta250-load, container, notices, advisories and
signing-policy. Each contained relative evidence path is bound by SHA-256.
Independently authenticate decision provenance and approval authority; record
hashes alone are not trusted signatures. publish=false remains mandatory until
actual legal/security/service/support/reporting/signing gates are approved.
