# R5 — builder workflow and downstream SDK delivery

R5 implementation proceeds in parallel with R3/R4. This is not full R5 closure.
The remaining full workspace test/build/rustdoc/current Clippy gates, R1/R2,
Ubuntu/hosted checks, R4 beta/soak/deployment/retention/adoption and independent
builder/manual accessibility acceptance remain OPEN. See
`verification/r5-implementation-ledger.md` for scoped evidence. The complete
ordered Windows/Ubuntu, fixture, native/manual and live check matrix is
[R5_CHECK_PLAN.md](R5_CHECK_PLAN.md); run its focused native pass before the full
workspace continuation.

## Implemented Studio batches

- Same-page edit/snapshot generations prevent late reload/import/validation/save
  responses from discarding edits or approving a newer draft. Save acknowledges
  its submitted revision while keeping newer local edits dirty and selected.
- Per-operation busy guards prevent duplicate UI operations; preview polling does
  not overlap and old polls cannot overwrite a start/stop result. Preview input
  acknowledgement does not erase newly typed input. List responses cannot erase
  newer publication/artifact results. API waits are bounded to 30 seconds with
  bounded response streaming and redirect refusal, never automatic POST retry.
- Artifact identity is derived from verified immutable bytes, not from a stale
  package selector. UI checks artifact/payload/revision before enabling preview
  or publication. The selected packaging game survives an asynchronous save.
- Optional server-side token-file renewal avoids exposing bearer tokens in the
  browser or requiring restart solely for token replacement. Original token-env
  behavior remains supported and intentionally reads the startup environment.
- Explicit recovery reconciles the existing publication .next/.previous file
  protocol under the workspace process mutex. Invalid, ambiguous or regressive
  records are preserved, not discarded or converted to approval.

## Configuration and persistence compatibility

This batch does **not** change workspace markers, draft/revision JSON,
artifact-sidecar, preview-receipt or publication record schemas. Drafts remain
game-neutral. Packaging game selection is not a persisted draft preference;
recheck the selector after restarting Studio. Existing artifacts expose the
actual game from their hashed manifests. No old artifact is silently relabeled.

New option (absolute private path, paired with endpoint):

```text
worldwright-studio serve --workspace PATH --preview-command WORLDWRIGHT_BINARY
  --release-endpoint http://127.0.0.1:PORT --operator-token-file ABSOLUTE_PRIVATE_FILE
```

The file and `--operator-token-env` are mutually exclusive. File mode reads a
fresh bounded token for each explicit request and accepts a single final LF or
CRLF. Missing/invalid files fail closed. Symlinks/reparse points are refused;
Unix checks private mode and single-link identity, with Linux owner checks.
Protect the parent directory and use the approved OS credential-update process.
Windows DACL/multi-user/race review remains a separate open gate. The file holds
only a scoped bearer token, never an issuer signing key. The browser receives
only configuration/renewal-mode information. 401/403 produces renewal guidance;
no expired request is automatically replayed or self-approved.

## Verified source REST contracts

Actual source: Studio `web.rs`, `artifact.rs`, `workspace.rs`, `publication.rs`;
server `operator.rs`. No endpoint was inferred from an unavailable OpenAPI file.

| Endpoint | Shape / checks | Status |
|---|---|---|
| GET /api/v1/artifacts/{digest}/identity | schema_version=1, artifact_digest, payload_digest, draft_revision, game; verified bytes | New additive route, source wired; native test authored |
| GET /api/v1/publication-auth | schema_version=1, configured, renewal, automatic_retry=false; no token/path | New additive route, source wired |
| GET /api/v1/publication-recovery | Bounded canonical request-ID array from existing leftover filenames | New additive route, source wired |
| POST /api/v1/publications/{request}/recover | Empty JSON object; local journal recovery only | Additive route, exact Host/Origin/media type and 4 KiB gate |
| POST /api/v1/publications/{request}/reconcile | Empty JSON object; fetch known authorized remote status and insert/update matching local record | Additive route, same gates; no remote mutation |
| POST /api/v1/operator/releases | Existing release-submission-v1 binary envelope, bearer auth; request is not activation | Existing server contract, unchanged |
| GET /api/v1/operator/releases/{request} | Existing PublicationView after current authorization | Existing server contract, unchanged |

Existing `/api/v1/artifacts` and `/api/v1/package` response schemas are unchanged.
New routes use the same exact Host and same-Origin mutation gate as other Studio
operations; no public operator surface or new signing authority is introduced.

## Publication recovery boundary

The supported `serve` path now acquires a create-new cooperative session sentinel
before binding a listener. A second updated Studio process refuses the same
workspace. Within that process, submit/refresh/reconcile/recover share a
nonqueueing gate (409 if busy); file operations retain their workspace mutex.
Older binaries and direct library/file writers do not participate: stop them
before using this workflow. This is not a certified power-loss protocol or a
lock against hostile same-user code. See session recovery below.

| Existing files | Explicit recovery |
|---|---|
| current only | Validate and return, idempotently |
| current + next | Validate identity/forward transition, complete replacement |
| previous + next | Validate identity/forward transition, finish installation |
| current + previous | Validate identity/forward transition, remove superseded previous |
| previous only | Restore prior local record; refresh remote status afterward |
| next only, all three, malformed/conflicting/unsafe files | Refuse and preserve evidence |

Recovery does not submit, review, approve or activate. It cannot reconstruct a
lost initial submission response with an unknown server-assigned request ID.
An authorized operator must first establish the exact existing request ID; the
new known-ID reconciliation control can then restore matching local history.
It cannot discover an unknown ID or prove that a timeout rolled back. Do not
blindly resubmit. Existing pending/corrupt/full local history is checked before
another remote submission. Unix updates/recovery sync each recoverable directory
layout in order and preserve synced next evidence on a rename/sync failure.
Actual filesystem power-loss/Windows durability acceptance remains open.

## Known-request reconciliation

Open **Publication connection and local recovery**, enter the canonical request
ID supplied/confirmed by the authorized operator, and choose **Reconcile existing
request**. Studio uses only the existing authenticated operator status GET. It
requires the exact local artifact digest/release ID, verified saved revision and
payload, successful preview digest and original preview completion time to match
the remote review context. An existing local record must also preserve world,
requester, target, kind, expected-current and all immutable review fields, with
only a permitted forward status transition. Repeating an identical result does
not create another record, including at the 100-record limit.

This explicitly adopts the server's current view of a known request; it is not
proof of which browser originally submitted it. It grants no review/activation
capability. Missing/invalid local evidence, wrong identity, denied grants and
corrupt/pending history fail closed. If the initial record is torn/malformed,
archive the workspace and use an authorized forensic repair before reconciliation;
this feature intentionally does not overwrite corrupt evidence.

The TCP client now applies one five-second connect/write/read deadline (local
credential-file access is outside that network deadline), caps actual response
bytes at 64 KiB, refuses redirects and never retries an uncertain mutation.
Windows token reads open the final component as a reparse point for rejection
and deny concurrent write/delete sharing during the bounded read. Unix opened
file mode/link/owner/identity checks are repeated. DACL and ancestor-race review
still require native security acceptance.

## Studio session recovery — deliberate startup tightening

`<workspace>/.worldwright-studio-session` is a new operational sentinel, headed
`worldwright-studio-session-v1`, not a draft/config schema rewrite. It contains a
PID and random instance nonce, no credential. New Unix files use 0600; Windows
holds a nonshared file handle. Normal return removes the matching sentinel;
crash, panic, failed preview cleanup, incomplete creation or failed removal may
leave it behind. Startup never reclaims a sentinel from PID absence or age.

After an interrupted session:

1. Stop and independently confirm termination of **all** Studio/preview writers
   and launchers for this workspace, including older binaries. Close browser tabs
   to prevent accidental new requests, but do not mistake tab closure for process
   termination. Use graceful shutdown first; forced termination is not a graceful
   pass. Check preview descendants separately. PID text alone is
   insufficient because PIDs are reused. Do not remove a live process's sentinel.
2. Record actual binary/source identity, OS/filesystem, exit/error and operator
   reason/time. Do not copy tokens into evidence. Preserve a private workspace
   copy outside the live workspace, including the sentinel and any
   `publication-records/*.next` / `*.previous` files. Do not edit those records.
3. With exclusive access established, archive/remove only
   `.worldwright-studio-session`; keep `.worldwright-studio` intact.
4. Restart exactly one updated Studio with the same workspace and trusted preview
   binary. Do not supply `--content` for an existing workspace. Confirm saved
   revision/artifact identities before editing. Use explicit local journal
   recovery (repeat on every listed pending request), then refresh authorized
   remote status or reconcile a known request. A request status of activated is
   not proof that the server currently runs that digest; use the activation
   runbook's active-state/readiness checks.

If startup still fails, or recovery reports conflict/corruption/unsafe files,
stop and retain the evidence. Do not delete records, edit status JSON, weaken
permissions, reset the workspace, or retry a publication to clear an error.
A 409 busy response is not permission to start another process; wait for the
known in-flight operation and inspect status. A timeout is not proof of rollback.
Successful local recovery must leave the exact expected canonical record, preserve
artifact/revision hashes and permit explicit authorized status refresh. A known-ID
reconciliation cannot proceed without its original matching preview receipt.

No automatic stale-lock cleanup command is provided: safely establishing process
ownership after a crash requires the operator. Supported local filesystems only;
network filesystems/hostile writers and Windows DACL/power-loss guarantees are
not certified. The native `r5_session_cross_process_...` test authors a real child
kill/reap check on both hosts; it has not executed in this sandbox.

## Local checks and native follow-through

```bash
node --check apps/worldwright-studio/assets/app.js
node scripts/test-studio-capabilities.cjs
node scripts/test-studio-workflow.cjs
python -m pip install -r scripts/r5-requirements.txt
python scripts/test-studio-browser.py
```

The browser fixture requires an already installed/approved Playwright Chromium
cache. It runs actual HTML/JS/DOM with a loopback fixture service, **not** the
Rust Studio backend, PostgreSQL or the operator service. It checks 1024 rooms,
64 exit controls, multibyte limits, real asynchronous editing and no page errors.
Its fifth group exercises a real keyboard-triggered reconciliation control, not
comprehensive keyboard/screen-reader/Firefox/Wayland/manual accessibility acceptance.

Native checks, when the pinned toolchain is available:

```bash
cargo fmt --all
cargo test -p worldwright-studio --lib --locked --offline r5_ -- --nocapture
cargo test -p worldwright-studio --locked --offline
cargo clippy -p worldwright-studio --all-targets --all-features --locked --offline -- -D warnings
```

On Windows retain explicit `--target x86_64-pc-windows-msvc --target-dir PATH`
for Cargo test/Clippy, reusing the previously identified cache if desired.
The current source declares 21 R5 tests (19 Windows / 21 Unix; two Unix-only).
They cover recovery layouts/conflicts/capacity/unsafe records, unchanged sidecars,
CLI token-source exclusivity, credential renewal/no retry, network deadlines/caps,
known-request reconciliation, operation gating, HTTP security and a real child
kill/reap session-sentinel test. These tests are authored, not executed here.
They do not constitute disk-full/power-loss injection or Windows DACL certification.
See R5_CHECK_PLAN.md for discovery, exact commands and additional manual cases;
syntax parsing is not a compiler or test result.

## Real builder acceptance still required

Use real R1-installed tools and an isolated, fully migrated R2 environment.
Keep artifact and request identifiers in a private run record. For **each game**:

1. Import/edit/save/validate a draft with a usable outgoing entry-room exit.
   Validation alone is not activation smoke. Preserve canonical revision hash.
2. Select the game, package, verify identity and preview the exact digest. Finish
   preview successfully; save/edit afterward to check explicit stale handling.
3. Configure only an authorized loopback publication endpoint and scoped token.
   Test expiry, denied grants, renewed credentials and explicit status retry.
4. Request through Studio. Independently review the exact request/artifact using
   the existing `docs/operations/RELEASE_ACTIVATION.md` CLI contracts and a
   different authorized reviewer. Do not add review/activation authority to Studio.
5. Follow that runbook for controlled shutdown/activation/start/status and a
   separately reviewed known-good rollback. Verify actual active digest, game,
   state and current authorization; rendering/listing alone proves none of them.
6. Exercise real process interruption at each publication replacement boundary;
   recover only validated local states, then refresh authoritative remote status.
7. Record manual browser/accessibility evidence and independent builder sign-off.

## Downstream SDK and compatibility

Retain the focused user-observed Windows successes: seven terminal tests and
three maturity tests, with both relocated consumers compiled and played.
Those earlier passes do not verify new Studio code or close R5. Existing SDK
workspace bindings, lockfile policy, no-license-grant boundary and retained API-v1
fixture remain unchanged by this batch. Run the existing P2 contract/negative
checks and native retained-consumer suite on the supported hosts; no imaginary
previous SDK minor or stable ABI is promised. See `TOOLKIT_V1.md` and
`P2_ACCEPTANCE.md`. Full current workspace and all outstanding R4 gates stay open.
