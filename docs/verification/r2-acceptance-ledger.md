# R2 acceptance ledger — November 10, 2026

Authority: ROADMAP_v2.md R2 — Live PostgreSQL, gameplay and identity correctness.
Prerequisite R0 is OPEN. No full R2 gate closed. Source changes, test discovery,
helper checks and configured CI do not substitute for live DB/provider/process
execution or authorized provider adoption. Current source inventory has eight
migration identities (actual applied history must be verified separately): original seven plus the additive P1 timer audit migration.
Existing migration files must not be rewritten.

| Unit | Status | Required evidence / scope |
|---|---|---|
| R2.1 migration ledger | ❌ gap found | Real PostgreSQL fresh-apply fails in 20261106000100_operational_control.sql:188, unquoted authorization; SQLstate 42601. No applied source/checksum rewritten. SQLx upgrade/corrupt/future/failed-ledger runs remain unverified. |
| R2.2 existing DB suite | ⏳ not yet checked | All discovered ignored tests actually executed, disposable database-name guard, engine/role/schema cleanup recorded |
| R2.3 reference/expedition state | ⏳ not yet checked | Initial state, counters/rooms/schedules/revisions, replay/conflict/authority, concurrency and reconstructed store/owner |
| R2.4 faults and process recovery | ⏳ not yet checked | Pre-commit rollback, unknown-COMMIT, real kill/restart, one receipt/audit and independent entitlement uniqueness, no speculative success or historical broadcast |
| R2.5 controlled OIDC | ⏳ not yet checked | Issuer/audience/nonce/state/PKCE, signing/key rotation, link/invite expiry/consumption, revocation/outage/restart with controlled fixture |
| R2.5 approved provider | ⏳ not yet checked | Real approved provider/browser/account fixture, exact source/artifact/environment and authenticated owner adoption |
| R2.6 browser resume | ⏳ not yet checked | OIDC callback reaches attached gameplay without password fallback, expiry/revocation/disabled/outage and no cookie exposure |
| R2.7 clock/RNG | ⏳ not yet checked | Independent ticks, deadline reconstruction/ordering/latency/downtime/drift, RNG rejection/replay/reset policy |
| R2.exit | ⏳ not yet checked | Actual current Ubuntu store/owner invariants and retained Windows gates, zero successful required skips, R0 accepted tuple |

Historical initial source review found a one-shot-invite regression disagreement,
per-row schedule clock sampling, and OIDC commit errors not classified as uncertain.
Source corrections and named regressions now exist for all three: one invited
callback wins, later login starts a new no-invite flow; deadline sampling is stable;
OIDC COMMIT failures are UnknownOutcome. Their native/live Rust execution remains
unverified; the auth contract was not weakened to make fixtures pass.
This ledger is updated batch by batch; source implementation is not acceptance.

## Expiry correction batch — November 10, 2026

**Verified source correction:** the existing SQL lifetime constraints are the
contract, not the faulty test setup. `scripts/r2-live.py` now gives expired Bob
invites both historical `created_at` and later-but-expired `expires_at`; its
session-expiry exercise adjusts both timestamps and scopes the update to Alice's
unrevoked sessions. The existing `durable_sessions_restart_lookup_revocation_and_expiry`
fixture is corrected and asserts `created_at < expires_at < statement_timestamp()`.
The retention fixture already adjusted both timestamps and now uses a stable
statement timestamp. No applied SQL migration was edited or constraint weakened.

Added `r2_expiry_fixtures_preserve_lifetime_and_reject_expired_identity`: first
requires the old invalid setup to fail specifically with PostgreSQL CHECK error
23514, then asserts valid expired session/invite rejection without a new identity,
session, account or audit entry. Both the new test and existing durable-session
expiry regression are mandatory discovery entries in `r2-verify.py`.

At the time of this batch entry, checks and current readback hashes had not yet
been recorded; the live PostgreSQL and final results entries below supersede that
intermediate state. All native/live/provider acceptance rows above remain unverified until
the corresponding current evidence is actually produced. This batch entry will
be followed by a results entry, not silently promoted to full R2 acceptance.

## Live PostgreSQL batch — new migration blocker

Executed on real PostgreSQL 16.11 in an isolated, loopback-only, non-root Ubuntu
24.04 sandbox cluster. Binary provenance: test-only npm package
`@embedded-postgres/linux-x64@16.11.0-beta.16`, not an adopted service deployment.
The exact corrected Python and Rust fixture SQL passed against immutable
session/invite CHECK constraints. Actual pre-forward-COMMIT, post-COMMIT and lost
acknowledgement proxy cases also passed against that PostgreSQL server. These
prove the SQL/proxy components, NOT compiled Rust/SQLx/current-owner acceptance.

**R2.1 GAP FOUND:** applying the complete unchanged eight-file inventory fails in
`migrations/20261106000100_operational_control.sql`, line 188:
`authorization JSONB NOT NULL`. PostgreSQL identifies `authorization` as a reserved
keyword (type/function category); the unquoted column declaration raises syntax
error 42601. The isolated migration transaction rolls back and the current-run
schema is dropped. No migration was edited, preprocessed or skipped to label the
full-inventory gate passed. The supplementary suite reports eight passed tests
and one full-migration error, and returns failure.

Remediation requires authoritative applied-migration history and an approved
migration/baseline repair decision; rewriting an immutable migration or its SQLx
checksum is not an automatic test fix. A later additive migration alone cannot
repair a syntax error encountered before it executes. This blocker now has an
executable CI preflight, retained logs, and remains an explicit failed acceptance
row rather than a successful helper result. Current native Rust and approved
provider/adoption evidence are still absent.

## Final observed checks for this pass

Retained in this directory: `r2-local-observations.json`,
`r2-local-helper-observations.log` (normalized check summary), and
`r2-local-sql-observations.log` (actual captured PostgreSQL suite output).
These are local selected-readback evidence, not an accepted native source tuple.
The JSON includes checked-source SHA-256, immutable SQL SHA-384, engine/provenance,
failed prerequisite attempts and explicit `full_r2_closed: false`.

| Check | Observed result | Acceptance boundary |
|---|---|---|
| R2 fixture/evidence helpers | 12 tests passed; includes both previously pending negative tests and new expiry guards | Not native Rust execution |
| HTTPS/proxy/adoption harness helpers | 8 tests passed | Real local HTTPS; synthetic PG unit frames; unit-generated approval key is not owner adoption |
| Browser resume adapter | Node contract harness passed | Not actual Worldwright server/provider browser integration |
| Corrected expiry fixture/proxy SQL | 8 supplementary tests passed on real PostgreSQL 16.11 | Includes three actual COMMIT boundaries, exact source-extracted SQL, CHECK 23514 negatives, scoped expiry/revocation preservation |
| Full unchanged migration inventory | 1 error, syntax 42601 at operational migration line 188; suite exit 1; zero skips | Failed gate, never a pass inferred from the other eight tests |
| SQL cleanup | Created schemas dropped; independent query found zero leftover ww_r2_/ww_test_ schemas; cluster stopped | Private ephemeral test cluster only |
| Rust grammar / Python syntax | 9 Rust readback files / 11 scripts passed | Grammar is NOT formatting, types, Clippy, compilation, tests or rustdoc |
| R2 workflow | YAML, baseline, permissions, ordered SQL preflight, both hosting retention branches checked; no duplicate paths | No hosted execution observed |
| Native fmt/check/Clippy/test | Attempted; native cargo executable absent | Blocked, not successful skips |
| R0 / R2 collector | Both executed and returned exit 1 for missing Cargo/rustc; zero native phases | Prerequisite gates remain open |
| Current-owner process / controlled browser drivers | Both executed and returned exit 1: compiled fixture absent; zero cases | Supplementary proxy/provider tests do not replace these cases |
| Approved-provider adoption | Actual issuer/deployment/account/authorized owner key and adoption not supplied | No real-provider login/adoption claimed |
| Same-tuple closure | Executed; exit 1, NOT CLOSED: live collection not accepted | No R0/R2 release closure |

### Remaining six-item work matrix

1. **Expiry defect and helper rerun:** corrected, source read back and supplementary
   SQL/helpers verified; the new native expiry test still awaits compilation/run.
2. **CI/docs/ledger:** current and checked; this pass adds the executable unchanged
   SQL preflight and retained observations rather than leaving them as a plan.
3. **Process/provider drivers:** implemented and component-tested; compiled current
   owner integration has not run and is not represented as complete acceptance.
4. **Native/live acceptance:** blocked by missing native Rust tooling and the newly
   observed immutable-migration fresh-apply syntax error. The full ignored SQLx
   suite/owner/restart invariants remain unverified.
5. **Approved provider:** observation and signature-verification tooling exists;
   actual provider/account exercises and authorized adoption remain external inputs.
6. **Closure:** fail-closed gate executed and refused the incomplete/failed evidence.

No gate was relaxed, migration/checksum edited, skipped case relabeled, historical
Windows pass promoted, or approval manufactured. The next required prerequisites
are authoritative applied-migration history/authorized repair, the pinned native
Rust build on the real current checkout and supported hosts, and actual approved
provider exercises/adoption. A later additive migration cannot by itself resolve
syntax in an earlier immutable migration.
