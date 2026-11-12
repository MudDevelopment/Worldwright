# R5 — builder workflow and downstream SDK delivery

Authority: ROADMAP_v2.md R5 items 1–7 and exit. R5 development proceeds in
parallel; R1/R2 prerequisites, remaining full Windows workspace tests/build/
rustdoc/current full Clippy, Ubuntu/hosted checks and all outstanding R4
measurement/deployment/soak/adoption gates remain OPEN. No approval is inferred.

Inherited, narrowly scoped user-observed Windows evidence: terminal tests 7/7;
maturity tests 3/3; both relocated minimal/expedition projects compiled, validated
content and ran meaningful rules outside their project directories. This does
not close R5 or the complete workspace check.

| R5 unit | Source/local status | Actual acceptance |
|---|---|---|
| 1 authoritative capabilities and UTF-8 maximum drafts | Verified local Node capability bounds and real Chromium fixture: 1024 rooms, 64 exits, UTF-8 limits and async save race | Open: real Rust Studio/backend plus supported/manual browser matrix |
| 2 selected game across authoring/artifact workflow | Implemented read-only verified artifact identity; UI requires matching artifact/payload/revision and retains packaging choice across async save; no persisted schema rewrite | Open: actual native identity route and exact reviewed/activated artifact; game choice is not a persisted draft preference |
| 3 authorized publication, independent review, activation/status/rollback | Existing Studio submit/status and server CLI/DB contracts; pin actual interfaces and runbook | Open: real R1/R2 plus independent authorized publication exercise |
| 4 async results, token renewal, stale/conflict/crash recovery | Implemented UI guards, token renewal, known-ID reconciliation, local journals, cooperative session exclusion, operation gate and total network deadline; 18 Node cases passed; 21 native R5 tests authored | Open: native tests, auth/filesystem/Windows DACL, real crash/power-loss and lost-response reconciliation acceptance; unknown-ID discovery is not implemented |
| 5 reduced SDK integrity, relocation, lockfile/version policy | Prior source and focused Windows relocation pass retained | Open: full supported-host/installed-tool/private source acceptance |
| 6 meaningful reference/expedition consumers | Focused Windows builds/run and terminal fixes passed | Open: supported independent-developer workflow, current Ubuntu evidence |
| 7 retained compatibility/deprecation/version/discovery | Existing P2 contracts/retained v1 fixture; keep unchanged and document checks | Open: complete current native/retained fixture verification |

## First R5 implementation batch

1. Add read-only artifact identity derived from verified immutable bytes, without
   rewriting draft/revision/artifact-sidecar/publication schemas.
2. Add opt-in server-side token-file renewal (exclusive alternative to existing
   token-env startup mode), never browser credentials or automatic POST replay.
3. Add explicit bounded publication .next/.previous recovery using existing
   identity/transition checks; preserve invalid/ambiguous evidence and never
   manufacture remote approval or retry a lost initial submission.
4. Add revision/edit-generation and per-operation UI guards, bounded network
   waits, non-overlapping preview polling, recovery controls and auth guidance.
5. Execute local Node/browser fixtures and source checks; author native positive/
   negative regressions and required run commands. Record each batch honestly.

## Historical first-batch verification and remaining work

Executed locally on selected EWW readbacks of that first batch (not current native evidence):
- Node syntax and the existing capability adapter checks passed (1024-room,
  64-exit and multibyte bounds, invalid capabilities rejected).
- 14 new deterministic Studio workflow cases passed: delayed/duplicate/conflict
  saves, late reload/import/validation, preview poll/control ordering, input
  acknowledgement, artifact identity, expiry guidance/no retry, publication list
  ordering, timeout, response cap and preserved packaging choice.
- Real Chromium 141.0.7390.37 / Playwright 1.56.0 passed four fixture groups:
  1024-room/64-exit DOM, 4096-byte Unicode boundaries, real edit-during-save with
  subsequent save, disabled publication/labeled recovery UI/no page errors.
  The fixture uses HTTP/controlled responses, not Rust Studio or PostgreSQL.
  Its first run checked text inside collapsed details with innerText; the test
  now opens the details and waits for its text, and the repeat passed.
- Rust source grammar was checked, not compiled. Nine new r5_ native test
  functions are authored (one Unix-only), covering recovery states/conflicts,
  token file/TCP behavior, CLI exclusivity and HTTP identity/security gates.
  No native fmt/check/Clippy/Rust test pass is claimed for this batch.
- Main Ubuntu/Windows CI now includes the Node workflow fixture. Hosted CI and
  the controlled real builder/activation workflow have not executed here.

Remaining implementation/integration work is explicit:
- Execute the new native Studio tests/full workspace continuation; address
  actual failures without weakening security or behavior assertions.
- Exercise the real R1/R2 Studio publication -> independent review -> activation
  -> status/known-good rollback path from the authoritative runbook. No new
  broad publication authority or synthetic closure collector was substituted.
- Native kill/restart/power-loss and exclusive-workspace recovery checks; Windows
  credential ACL/reparse/path review; real known-ID lost-response reconciliation.
  Cooperative exclusion and known-ID source support were added in the continuation;
  they are no longer missing source features, but acceptance remains unrun.
- Current supported-host/retained SDK fixture/P2 acceptance and manual browser/
  keyboard/screen-reader/independent-builder evidence remain required. Prior
  focused Windows SDK and terminal passes retain their original scope.
- Packaging game selection is deliberately not persisted into old draft formats;
  a persisted preference, if required, needs a separately explicit schema change.

R5 is NOT CLOSED. The remaining full workspace and all R4 gates stay OPEN.

## Continuation batch — source installed; native/live acceptance open

- Added explicit known-request reconciliation using only the existing authorized
  operator status GET; local immutable artifact/revision/preview context must
  match. No request discovery, mutation replay, review or activation authority.
- Added a shared nonqueueing publication-operation gate and cooperative serve
  session sentinel. Second serve refuses; crashed/unknown sentinel is never
  automatically reclaimed. Explicit operator recovery is required after crash.
- Publication TCP I/O now uses one five-second connect/write/read deadline and
  capped chunk reads, not an indefinitely renewable per-read timeout.
- Publication update/recovery syncs each recoverable directory layout on Unix;
  synced next evidence is preserved on rename failure. Readers cap actual bytes
  and reject Unix hard links / Windows reparse metadata. Windows durability and
  hostile path/ACL review remain open.
- New native reconciliation/deadline/session tests authored, not executed.
- Final selected-source checks reran on November 11, 2026 at 21:01:56 EST
  (2026-11-12T02:01:56Z). Node 22.23.3, Python 3.12.3, Playwright 1.56.0,
  Chromium 141.0.7390.37: 18 workflow cases, capability checks and five Chromium
  fixture groups passed; JS/Python syntax and ten Rust grammar subjects passed.
- Retained `r5-local-observations.json` and `r5-local-contracts.log` bind 17 selected
  LF-normalized source/test/asset subjects. No complete checkout/commit, signed
  provenance, Rust types, native execution or deployment acceptance is implied.
- Source declares 21 R5 native tests, including two Unix-only cases (19 Windows).
  The helper `r5_status_server` is not a test; the child test helper's standalone
  no-op is not process-exclusion evidence. Cargo discovery and execution required.
- Cargo/rustc/rustfmt/PowerShell remain unavailable in this sandbox. Native
  fmt/check/Clippy/tests/build/rustdoc have NOT run for current R5 source.
- Documentation/check-plan synchronization is recorded below; R5 is NOT CLOSED.

## Current execution ledger — complete check-plan inventory

Authoritative commands, prerequisites, expected results and stop conditions:
[../R5_CHECK_PLAN.md](../R5_CHECK_PLAN.md). Status vocabulary is verified / gap
found / not yet checked. A source feature or an old pass is not current native
verification. A partially observed gate remains not yet checked as a whole.

| Plan unit | Status | Evidence / next action |
|---|---|---|
| A source/toolchain/fmt | not yet checked | Selected source readback verified; actual Git/worktree/native tools/fmt still required |
| B focused R5 native | not yet checked | Discover 19 Windows / 21 Unix R5 tests and execute, no unexpected skips |
| C full Studio/lint | not yet checked | Run entire Studio suite and all-target/all-feature Clippy |
| D terminal/SDK | not yet checked | Historical Windows 7 terminal + 3 maturity passes retained; current both-host retained-consumer evidence required |
| E full workspace | not yet checked | Reuse known Windows cache for development; full check/build/rustdoc still open |
| F Studio local fixtures | verified | Current 18 Node cases, capability checks and five Chromium fixture groups; retained JSON/log |
| F remaining browser/P2 | not yet checked | Player adapter and P2 source/negative fixtures not rerun in this documentation pass |
| G acceptance helpers | not yet checked | Historical results retain scope; execute current R0/R1/R2/R3/R4 helpers |
| H pinned Helm | not yet checked | Require actual v3.19.0 lint/render/schema/negative profiles, not historical alternative version |
| I full migration/native SQLx | gap found | Known reserved authorization syntax failure, SQLstate 42601 at operational_control.sql:188; authorized repair absent |
| J real Studio/backend | not yet checked | Maximum-size multibyte/native auth/identity/preview/reconciliation workflow |
| K recovery/security | not yet checked | Native session/journal/process faults and Windows DACL/Unix ownership/power-loss matrix |
| L desktop/accessibility | not yet checked | Supported real browser/display/keyboard/screen-reader/Windows observations |
| M paired R0/R1 | not yet checked | Fresh exact-source native reports, private installed tools and human evidence |
| N R2/R3 | not yet checked | Current live provider/transaction/image/role/staging proof and independent adoption |
| O R4 | not yet checked | Actual unchanged beta-250/scenarios/soak/private measurements/retention and signed adoption |
| P independent builder | not yet checked | Exact previewed/reviewed/activated artifact, current status and supported rollback outside original checkout |
| Q scoped docs/evidence readback | verified | 17 docs read back, balanced fences, five Bash blocks syntax-checked, all 11 plan link references verified; retained report/log and 17 source hashes match |
| Q native/release provenance | not yet checked | Actual Git diff/lock/migration/host/binary and complete accepted report tuples still required; scoped text hashes are not full checkout proof |

## Documentation synchronization scope

Updated README, DESIGN, SECURITY, ARCHITECTURE, IMPLEMENTATION and ROADMAP_v2;
Studio, R5 acceptance/recovery, development, Linux desktop, verification, Toolkit
and R0/R1 guides; release-activation handoff; R4 and R5 ledgers. Added the complete
R5_CHECK_PLAN and retained local observation/log files. Historical milestone
reports, old evidence hashes, legal policy and acceptance thresholds were not
rewritten as current passes. Application source/lock/migrations were not changed
in this documentation pass.

Final readback notes: the first integrity comparison detected the EWW multi-file
response's extra final framing newline, not a changed repository log. Actual file
sizes (2887-byte log; 102-byte requirements file) confirmed the boundary. The
scratch extractor was corrected, checks rerun and the observation regenerated;
final EWW report content, log digest and all 17 selected-source hashes match.
No application source or historical acceptance record was altered to obtain that
match. PowerShell/native Rust and a full-repository link/anchor audit were not run.

Recovery now distinguishes: (1) operator-only session-sentinel recovery after
confirmed writer shutdown, (2) validated local journal recovery, (3) authorized
known-request reconciliation, and (4) independently authorized remote activation/
rollback. None implies another or licenses blind retry. R1 crash cleanup followed
by fresh XDG is explicitly not same-workspace crash recovery evidence.

## Follow-up final readback audit — November 11, 2026 (EST)

This is a new integrity/documentation audit, not a rerun of the application tests
and not gate closure. Fresh EWW reads covered the 17 recorded subjects, report,
log, the 17 previously synchronized documents/ledgers, dev.ps1/dev.sh, CI, toolchain,
SDK test filenames, DB fixture contracts and .gitattributes. No application source,
Cargo.lock, migration, acceptance threshold or prior observation hash was changed.

| Item | Status | Finding / disposition |
|---|---|---|
| R5-Q1 recorded subjects | verified | All 17 fresh UTF-8/LF subject hashes match the retained report |
| R5-Q2 retained log | verified | Exact staged log digest is 0ae9ac77045433afd51617a6ba194a392d30476507c5f2784940d86df809f5ae; matches retained report; no rebaseline |
| R5-Q3 roadmap authority wording | gap found | README tree/index and DESIGN boundary still presented historical ROAD-MAP as current authority; corrected to ROADMAP_v2 with historical scope retained |
| R5-Q4 Ubuntu execution instructions | gap found | Earlier block printed tools without enforcing the pinned versions; corrected to require Rust/Cargo 1.85.1, successful host query, Linux host and absolute target |
| R5-Q5 pre-format evidence check | gap found | Instructions lacked a reproducible read-only comparator before source formatting; added verify-r5-evidence.py and documented ordering/exit codes |
| R5-Q6 Windows log portability | gap found | .gitattributes has no .log LF rule; future checkout conversion can invalidate raw log hash. No current mismatch observed; native exact-byte export/retention policy remains OPEN |
| R5-Q7 verifier regressions | verified | Seven synthetic tests passed: match/no mutation, source CRLF/content drift, raw log newline drift, missing/path escape, duplicate inventory/hash, wrong profile/closure claim, duplicate JSON keys |
| R5-Q8 native test inventory | verified | Source declarations confirm 21 R5 tests, two Unix-only, expected 19 Windows. This is not Cargo discovery/execution |
| R5-Q9 native/live/security/adoption | not yet checked | Existing A–P native/live gates, migration blocker, Windows DACL/power-loss and independent R4/R5 acceptance stay OPEN |

The new verifier uses only the Python standard library, reads bounded files, and
never executes commands from the report or writes new expected hashes. Its pass
means agreement with a supplied unsigned report, not authentication or complete
checkout provenance. Source hashes normalize line endings; log hashes remain
byte-exact. A line-ending-only log mismatch is diagnosed, never accepted silently.
Its new script/test files are NOT retroactively added to the old 17-subject report.

R5_CHECK_PLAN now provides explicit pre-format checks, external private transcripts,
bootstrap conditions, ordered native results, external Python/browser provisioning,
exact-binary Studio start/close/reopen, isolated security/recovery procedures,
database report/cleanup evidence and the return package. The 17-document fence
and link checks from the earlier pass remain their original scope; this audit
does not claim a complete repository link/anchor audit or PowerShell execution.

Follow-up write/readback completion: corrected README/DESIGN/check-plan/ledger
and both new Python files were read again. Seven verifier tests reran successfully
from the installed readbacks; all 17 original subjects and the exact staged log
still match without changing the old report. All 17 scoped document fences and
five Bash snippet syntax checks passed. PowerShell/native execution and complete
repository link/anchor checking remain unrun. Actual remote metadata confirms the
log is 2887 bytes and r5-requirements.txt is 102 bytes (no framing newline included).

New verifier LF-text SHA-256:
- scripts/verify-r5-evidence.py: c686aa0a36d05c63fc7db1a501fc37e62054721955d8f197672ed77df2ef37fb
- scripts/test-r5-evidence.py: bbee28e3aa1624ba39e59aad9632e118b3350f309ae6156f8ba1c19edc0ab194

These are transfer/readback identities, not signed provenance or additions to
the old fixture report. Findings R5-Q3/Q4/Q5 have documented corrections; R5-Q6
portability and all existing native/live/security/independent gates remain OPEN.

## User-observed launch setup failure and instruction correction

The submitted Windows transcript shows the launch path lacked the target triple
(`...native-check-...\debug\worldwright-studio.exe`), consistent with an empty
CARGO_BUILD_TARGET when constructed. `$evidence` was null, so no workspace path
was constructed. Get-FileHash and invocation failed before Studio could start.
This does not establish a runtime defect or whether the correctly targeted binary
already exists; no directory listing/native build was available in that transcript.

The earlier isolated launch example incorrectly depended on preceding shell state.
R5_CHECK_PLAN section 4 now contains a self-contained, dot-sourced block that sets
target/cache/evidence/workspace paths, checks pinned native tools, runs a targeted
incremental locked/offline build for just worldwright and worldwright-studio,
verifies both output files, hashes them and starts a new sample workspace. It
preserves the existing cache and does not run cargo clean or reset user data.
Same-workspace reopening is separately described without --content.

Status: instruction defect corrected; native execution/startup still NOT VERIFIED.
No application code, migration, lockfile or existing evidence hash changed; no
native, R4, R5, security or independent acceptance gate closed.

## Transcript-as-command input failure

The latest user output includes literal `PS E:\Hold\Worldwright>`, `>>` and prior
error messages entered as commands. Get-Process alias errors, redirection errors
and AT usage output follow from that input; they do not show execution of the
corrected launch block or a new Studio runtime defect. Redirection may attempt
filesystem writes, so do not assume arbitrary transcript replay is harmless.

Installed scripts/start-studio-local.ps1 to replace multi-line console copying
with one command. It derives the repository path, initializes target/workspace
state, checks Rust/Cargo 1.85.1, incrementally builds only the two binaries and
launches fresh sample content; optional -Workspace reopens without --content.
README and the check plan document prompt-free invocation and policy boundaries.
No cargo clean, execution-policy bypass, sentinel removal, migration change or
old evidence-hash update was introduced. Native execution remains NOT VERIFIED;
all prior open gates remain open.
