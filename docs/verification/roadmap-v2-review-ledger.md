# ROADMAP v2 repository review ledger

Started: 2026-11-08. Source: EWW, E:\Hold\Worldwright.
Purpose: repository-wide roadmap-level implementation review, not line-by-line
security certification. Generated target artifacts and .git internals excluded.
Full directory inventory and recursive Rust-source filename inventory obtained.
Each unit below is a work package; verified means sufficient source evidence for
roadmap conclusions, not that every source line or runtime behavior was tested.
Partial/large-file reads will be stated in the final review boundary.

| Unit | Status | Evidence / remaining scope |
|---|---|---|
| Root manifests, lockfiles, toolchain, provenance | ✅ verified | Root/all14 member manifests, toolchain, Cargo config, lock header/local-package block, gitignore/attributes reviewed. 14members, Rust1.85.1/edition2024, publish=false. No TRF path dependency in inspected manifests/local lock entries; not an exhaustive supply-chain certification. |
| apps/worldwright | ❌ gap found | Commands/play/release/sdk/project generation and maturity tests reviewed. Preview worker ignores artifact game contract and always uses ReferenceGame. Project-new embeds build-source absolute paths, emits no initial lockfile; nested tests inherit target-directory overrides. |
| apps/worldwright-server | ❌ gap found | Full owner/OIDC/lifecycle/metrics/release-runtime/release-CLI, composition and auth/web/operator/CLI sections reviewed. Durable gameplay wired. Scheduled work advances on commands; no idle tick. Browser durable-ID reuse/OIDC session resume and dedicated durable-failure tests remain open. Large-file partial reads noted. |
| apps/worldwright-studio | ❌ gap found | All eight source modules reviewed. Package hardcodes reference game. Workspace requires existing parent; desktop.sh does not create XDG default parent. Preview readers share status across runs; lifecycle/UTF-8/process-tree tests needed. |
| apps/worldwright-console | ✅ verified | Full source/manifest reviewed. Argon2+TOTP local issuer, bounded loopback HTTP control, Unix 0600 credentials and Windows manual-ACL warning. Signing key distribution remains security work. |
| apps/worldwright-load | ✅ verified | All five source files/manifest reviewed. beta-250-v1 fixed 250x30, HTTPS/cookie/stream gates, report evaluator, Unix 0600 sensitive fixture. No live profile evidence. |
| crates/worldwright-core | ✅ verified | Runtime execution/state/scheduler/rules/effects and legacy WorldStub reviewed. Counter scheduling supported; item transfer/WakeCharacter unsupported. Metadata/comments have stale scaffold wording. |
| crates/worldwright-content | ✅ verified | All seven source modules reviewed: bounded 32MiB/depth32/100diagnostics, strict schema-v1 object records, graph validation and deterministic instance mapping. No general v0/v2 migration claimed. |
| crates/worldwright-expedition | ✅ verified | Full game source reviewed: supply, threat, victory rank, four game actions, scheduled recovery and five tests. Do not repeat implementation as future work. |
| crates/worldwright-operations | ✅ verified | Manifest and contract sections reviewed: bounded HMAC capability token/MFA, release metadata, signed backups. Code targets RPO24h/RTO4h/audit90days; neither measured nor owner approval inferred. Remaining per-line crypto audit out of scope. |
| crates/worldwright-persistence | ❌ gap found | All module boundaries inspected; full game_state/OIDC, SQL constants/errors, library DB harness and store/operations/releases/types sections reviewed. New gameplay/OIDC needs dedicated real-PG integration evidence; old19 ignoredtests do not prove it. No outbox dispatcher evidenced in module tree. |
| crates/worldwright-protocol | ✅ verified | Wire/encode/decode sections reviewed: v1,16KiB,request128bytes,game_action and durable result DTOs. Status string claiming process-local sessions is stale for Postgres backend. |
| crates/worldwright-release | ✅ verified | Manifest/game/artifact/strict ID sections and CLI consumers reviewed: reference+expedition v1 supported, immutable digest artifacts. General third-party game registry absent. |
| crates/worldwright-sdk | ✅ verified | Manifest/full facade reviewed: SDK API v1, literal compatibility matrix, declared previous-minor policy, deprecated LegacyWorldView and ScenarioStub inventory. Needs compatibility CI proof, not a new facade. |
| crates/worldwright-testkit and root tests | ❌ gap found | All five source modules/root test helper and app smoke consumers reviewed; user test output provides scenario execution evidence. In-process percentile recipe differs from beta nearest-rank; root helper/docs still say scaffold/old includes while actual app smoke tests are independent. |
| Both browser asset sets | ❌ gap found | All six assets reviewed. Server browser resets browser-N IDs, lacks OIDC-return session reconnect. Studio MAX_ROOMS1000/MAX_EXITS256 differs from backend1024/64; title/description character limits differ from backend byte limits. Real UI tests remain open. |
| Seven SQL migrations | ✅ verified | All seven SQL files reviewed: valuable-state, releases, operations/incarnation, Studio review context, reference/expedition artifacts, gameplay counters/schedules/receipts, issuer+subject OIDC mapping. Filename timestamps are identifiers, not delivery evidence. |
| Development/desktop scripts and CI | ❌ gap found | All three scripts/CI/attributes reviewed. Native-target launcher does not create required XDG parent; doctor lacks target/sysroot/override context; CI has no dedicated DB/Helm/browser gates and hosted runs unseen. |
| Docker/Helm deployment | ❌ gap found | All chart templates/render scripts/values/schema/Docker reviewed. Four OIDC-disabled default violations reproduced via JSON Schema in sandbox. helm lint called without required values; render scripts search tlsSecretName instead of rendered secretName; PS lacks immediate native exit checks. No actual Helm/Docker/cluster run. |
| Existing roadmap/handoff/architecture/security/docs | ❌ gap found | Key roadmap/handoff/architecture/security/design/verification/deployment docs reviewed. Historical scaffold/package/session/DB/deployment claims conflict with current source; ROADMAP_v2 must separate corrected baseline and unverified gates. Historical docs retained. |
| Verification records and user execution evidence | ✅ verified | Historical M8/performance/VERIFICATION plus current Linux audit and user-supplied Windows check/Clippy/tests/build results reviewed. Formatter correction applied earlier but recheck unseen;19DBtests skipped. Historical Linux provenance not current full-workspace proof. |

Completed at roadmap-review unit level: 21 units considered, 12 verified for
roadmap evidence, 9 gap-found, 0 units not yet checked. The full source filename
inventory was enumerated; this is NOT a full line-by-line review of every Rust
file, all dependency sources, all historical logs or all docs. Large auth/CLI/
operator/protocol/release/SQL adapter files used targeted sections; representative
tests and supplied execution records supplemented source review. Independent
full static/security/supply-chain audit and real Ubuntu/DB/deployment verification
remain open. Gaps are tracked in ROADMAP_v2.md, not treated as fixed by prose.

## Playable second-game follow-through — November 9, 2026

Source update: new expedition terminal host; generated executable runtime;
manifest-aware preview dispatch; optional Studio game selection; friendly
multiplayer/browser actions and UUID page request namespaces. Regression code
added for host/generation/artifact/network progression and recovery. Nested
build targets are now explicit. WWV2-003/004 source gaps corrected, verification
pending; WWV2-015 partially addressed. Idle ticks, live durable game recovery,
SDK relocation and broader acceptance remain open. No current Cargo results
were observed through EWW; prior Windows test evidence predates this patch.
