# P1 implementation ledger — November 9, 2026

Requested IDs: WWV2-005, 006, 007, 010, 011, 012, 013, 014, 015, 017, 018, 019.
Current repository: E:\Hold\Worldwright through EWW. All twelve FULL ACCEPTANCE
tracks remain open. Source implementation, authored regressions, synthetic/static
checks, native builds, live services and owner decisions are separate evidence.
EWW supplies file operations, not remote execution. The sandbox currently has
Node/Python but no Cargo/rustc; no current native Rust pass is claimed.

| Item | Acceptance status | Installed source / authored regressions | Still required |
|---|---|---|---|
| 005 | ⏳ not yet checked | Cookie-only GET session probe; startup connect and generation guards; Node resume/expiry/unavailability/late-probe/logout checks | Real provider/browser and concurrent Set-Cookie login/logout race acceptance |
| 006 | ⏳ not yet checked | Timer audit-failure rollback/reconstruction/stale-version/writer DB tests; OIDC link concurrency/collision/issuer/revocation/reconnect; receipt-preserving session pruning; full ignored suite CI | Actual DB runs, provider/network process matrix, unknown COMMIT and restore/cross-process evidence |
| 007 | ⏳ not yet checked | 100-ms owner due-work poll with Delay; atomic fenced state/timer audit commit; durable command RNG rollback | Real idle/crash/restart behavior, detailed RNG rejection/replay policy tests; no persisted deterministic RNG claim |
| 010 | ⏳ not yet checked | Backend capabilities; UTF-8 hints; HTTP regression; DOM checks for all 1024 rooms, 64 exits and multibyte text | Native HTTP test and real browser maximum-size/accessibility acceptance |
| 011 | ⏳ not yet checked | Per-run output, split-UTF-8 tests, bounded queued input, tracked/reaped I/O, success receipt waits for all workers, Unix process-group/Windows taskkill cleanup | Live pipe/orphan/PID/crash/SIGTERM tests and remaining platform cleanup review |
| 012 | ⏳ not yet checked | Narrow attachment/location inspection and six fixed timing stages | Detailed clone/lock/commit profiling and actual unchanged beta250 measurement; no speculative concurrency/completion change |
| 013 | ⏳ not yet checked | External dispatch explicitly unsupported; 1..1000 expired-session pruning preserving receipts/business uniqueness/OIDC/audits | Live cleanup/replay tests and named broader privacy/retention adoption; no automatic audit purge |
| 014 | ⏳ not yet checked | Optional separate loopback metrics; full PG CI; development image CLI/linkage smoke; protected/default-branch/manual fixed load; report uploads with GitHub.com/Enterprise pins | Hosted jobs, environment reviewers/rulesets, real service/deployment/load results; CLI image smoke is not deployed-service proof |
| 015 | ⏳ not yet checked | Reduced SDK dependency graph corrected for getrandom; relative bundled SDK; supplied matching bundle override; both playable templates; move-before-build regression | Locked native bundle/project compilation and execution away from original checkout on Ubuntu/Windows |
| 017 | ❌ gap found | README/DESIGN/SECURITY/ROADMAP/development/playable/migration docs reconciled; P1_ACCEPTANCE added; historical observations labeled | Architecture/handoff/status/helper sweep remains incomplete; no whole-tree documentation audit claimed |
| 018 | ❌ gap found | Linux per-owner directory 0700 checks, new slot mode0600, unsafe-existing-path refusal | Windows ACL/key authority and rotation review, hostile multi-user/symlink/race evidence remain unimplemented/unverified |
| 019 | ⏳ not yet checked | Release-evidence/Cargo-declaration SBOM collector, exact release tuple, named decision/evidence checks; publish=false retained | Actual advisory/notices/signing/legal/security/service/support/reporting decisions, pinned deployment image provenance; hashes do not authenticate approvals |

## Continuation batch recorded

- ci.yml now verifies four required database test names exist and executes ALL
  ignored PostgreSQL tests; retains test-list/log artifacts for 14 days.
- p1-acceptance.yml adds non-root/read-only/cap-drop/network-disabled container
  CLI help smoke and manual/default-branch-only reviewer-gated beta250 run.
  It preserves the profile and excludes the temporary credentials file from uploads.
- Upstream upload-artifact tags resolved this turn: v4
  ea165f8d65b6e75b540449e92b4886f43607fa02 for GitHub.com; v3
  ff15f0306b3f739f7b6fd43fb5d26cd321bd4de5 for Enterprise. Mirroring/organizational
  approval is still required. No branch protection/environment approval was applied.
- Preview review found success could precede I/O completion. Input sender closes
  after process exit; success receipt now waits for finished I/O workers. Timeout
  draining is Failed rather than successful receipt. Live leak tests still needed.
- Both generated templates now relocate BEFORE nested native test/build and run
  outside the project. Test code is authored, not observed passing here.
- Browser fixture adds valid/expired/unavailable resume and late-probe-after-logout
  cases. Added Studio DOM capability fixture and authoritative HTTP bound test.
- Updated README, DESIGN, SECURITY, ROADMAP_v2, DEVELOPMENT, EXPEDITION_PLAYABLE,
  migrations/README and P1_ACCEPTANCE. Existing milestone evidence was preserved.

## Checks actually observed in this continuation

Read current files from EWW, staged exact readback content in a separate sandbox:

| Check | Observed result | Does NOT establish |
|---|---|---|
| node test-expedition-browser.cjs | PASS: cookie resume/expiry/unavailability/logout race, action/status, ID/stream/timer/no-replay lifecycle | Real cookie/provider/browser or PostgreSQL behavior |
| node test-studio-capabilities.cjs | PASS: 1024 room visibility, 64-exit cap, UTF-8 boundary and invalid capability rejection | Real browser/accessibility or backend execution |
| Node syntax for both browser assets | PASS | Native app integration |
| Python compile of release-evidence.py | PASS | Advisory scan, authenticated approvals, SBOM/release gate execution |
| CI YAML/job wiring and protected credential-artifact exclusion | PASS | Hosted execution, environment adoption or target identity proof |
| Rust grammar parse of 8 readback files | PASS, no grammar errors | Type checking, borrow checking, Cargo linkage, rustfmt, Clippy or tests |

The first grammar invocation found a missing sandbox parser; a temporary venv
installed tree-sitter/tree-sitter-rust and the subsequent 8-file parse passed.
No parser or sandbox source replacement was installed into the repository.

## Exact next work

1. Run approved Rust 1.85.1 fmt/check-native; repair the first real failure against
   current source. Earlier Windows passes predate these changes.
2. Execute full disposable-PG suite and real provider/process/preview/desktop gates.
3. Finish WWV2-017 architecture/handoff/status/helper audit and WWV2-018 Windows
   ACL/key authority/rotation implementation. Review lingering preview tree races.
4. Inspect actual CI artifacts; run unchanged beta250 only after independently
   verifying the approved deployed release and configuring environment reviewers.
5. Supply authenticated legal/security/service/retention/signing/advisory evidence.
   Promote individual acceptance statuses only on observed results, not source edits.

## Source references

scripts/test-expedition-browser.cjs; scripts/test-studio-capabilities.cjs;
.github/workflows/ci.yml; .github/workflows/p1-acceptance.yml;
apps/worldwright-studio/src/{preview,web}.rs; apps/worldwright/tests/maturity.rs;
apps/worldwright-server/src/{application,metrics,world,web}.rs;
crates/worldwright-persistence/src/{game_state,lib,retention}.rs;
migrations/20261118000400_gameplay_timer_audit.sql; scripts/release-evidence.py.
