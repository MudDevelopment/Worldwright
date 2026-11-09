# R0/R1 acceptance ledger — November 9, 2026

Authority: ROADMAP_v2.md section 5 (release tracks R0/R1, NOT P0/P1 priority labels).
Detailed procedures: ../R0_R1_ACCEPTANCE.md. Scope of local execution: selected
current text readback files in a Linux sandbox, not execution in the Windows
working tree. Neither full R0 nor full R1 is closed. A configured or tested gate
is not a passing supported-host run, and a template is not human acceptance.

## Disposition of the requested six remaining tasks

| Task | Result | What is / is not completed |
|---|---|---|
| 1. Recheck latest changes and regressions | ✅ verified at helper/syntax scope | 14 helper tests passed; 10 synthetic launcher tests passed; 2 real-binary tests explicitly skipped. Includes installed no-Cargo mode, private existing-workspace rejection, malformed records, null-digest/source rejection, inventory recomputation and real pidfd cleanup of a cooperative Python child. NOT Rust/Studio acceptance. |
| 2. Documentation, roadmap and detailed ledger | ✅ verified source update | README, DESIGN, SECURITY, LINUX_DESKTOP and roadmap synchronized; R0_R1_ACCEPTANCE guide and this ledger installed. No gate status promoted by documentation. |
| 3. Actual paired Ubuntu/Windows R0 | ❌ evidence blocker | Local attempt returned exit 1 at `missing required R0 tool: cargo`, before any native phase. Cargo/rustc/rustfmt/Helm/PowerShell/Docker absent here. Ubuntu and reviewed/default-branch Windows workflows are wired, not observed executing. No current paired host reports accepted. |
| 4. Build/exercise real private package | ❌ prerequisite blocker | Actual builder attempt returned exit 1 because R0 did not pass; no archive created. Actual R1 attempt returned exit 1 (`ValueError: real package directory required`), with zero real cases. Synthetic package/launcher tests are NOT a replacement. |
| 5. Perform/authenticate manual observations | ⏳ not yet checked | All 6 template cases remain `not_checked`, reviewer/digest fields empty. No X11/Wayland/Firefox/Windows credential observations or reviewer authority fabricated. Actual authorized humans and exact-package evidence still required. |
| 6. Validate same-tuple evidence before closure | ✅ fail-closed behavior verified; full closure ⏳ | Actual validator invocation returned exit 1 / NOT CLOSED on failed R0 evidence. Inventory/log/host/binary/package validation is implemented and strengthened, but a successful full paired tuple/manual review remains unavailable. |

Tasks 3–6 cannot honestly be described as accepted or fully complete. Their
execution/approval prerequisites are missing, not Rust bugs inferred from tests.

## Full gate coverage (acceptance status, not implementation status)

| ID | Status | Source/gate mapping and required remaining evidence |
|---|---|---|
| R0.1 | ⏳ not yet checked | r0-verify records commit/worktree/source inventory, native pin/host/sysroot, locked graph, override/config paths/hashes, binary hashes; real complete tuple still needed. |
| R0.2 | ❌ evidence blocker | Native P0 tests, real first-run/game package/preview, browser receipt + live PostgreSQL and required Helm matrix are wired; no supported-host full run here. |
| R0.3 | ❌ evidence blocker | Ubuntu/Windows fmt/check/Clippy/test/build/rustdoc/unchanged-source and canonical lock must pass on same commit; neither current paired report provided. |
| R0.4 | ⏳ not yet checked | Fresh explicit targets and HOME/XDG/spaces/permissions covered by synthetic tests; actual supplied-native cases still required. |
| R0.5 | ⏳ full gate not yet checked | Regression discovery/failure policy/source/docs implemented; helper tests/YAML passed. Full R0 run and authenticated evidence must establish all exits. |
| R1.1 | ⏳ not yet checked | r1-desktop-verify covers fresh/existing HOME/XDG/external cwd/permissions; synthetic installed/workspace tests passed but real package is missing. |
| R1.2 | ⏳ not yet checked | Real Chromium import/edit/save/validate/package/exact-game preview/quit/reopen, artifact/snapshot identity + immutable bytes require actual package execution. |
| R1.3 | ⏳ not yet checked | Real INT/TERM/HUP, timeout exact supervisor exit/message, crash/session/pipe/PID cleanup required. Only helper child + synthetic launcher termination tested here. |
| R1.4 | ⏳ not yet checked | Actual Chromium/Firefox, X11/Wayland, no-browser and keyboard/scaling/accessibility versions/screenshots/observer review remain unobserved. |
| R1.5 | ❌ prerequisite blocker | Private builder verifies full R0/log/source/binary identity and library resolution; runtime integrity/installed path implemented. No real archive exists from this attempt. |
| R1.6 | ⏳ not yet checked | ci.yml Windows regressions retained; r0-windows.yml requires reviewed disposable DB environment. Current native smoke/preview/credentials/discovery and ACL/key review remain required. |
| R1.exit | ⏳ not yet checked | R0 closed, exact real package, automated/manual evidence and authenticated owner review mandatory. No success issued. |

## Actual observed checks

- Current selected-source readback: Python compileall and Bash syntax passed.
- test-r0-r1.py: 14 tests, 14 passed, no skips. Most are metadata/integrity/
  negative-policy tests; one exercises pidfds with a cooperative Python child.
- test-desktop.py: 12 tests, 10 synthetic passed, 2 real tests skipped explicitly
  because no native paths supplied. Installed branch uses synthetic executables
  and removes Cargo.lock/toolchain/target fixture to test tool/path isolation only.
- Ubuntu/Windows workflow YAML and default-branch/manual secret gate wiring passed
  static checks. Environment protection/adoption and hosted runs NOT verified.
- Actual prerequisite sequence: R0 exit 1; package build exit 1/no output archive;
  R1 exit 1/zero cases; closure exit 1/NOT CLOSED. No timeout success or blank
  digest/source tuple was substituted for evidence.
- Manual template verified to retain exactly 6 unobserved cases with no reviewers.
- No current Cargo/rustfmt/Clippy/Rust tests, live PostgreSQL, Helm rendering,
  real Studio/Chromium package, physical display or Windows execution claimed.

Captured scope/facts: r0-r1-local-observations.json. Explicit failed report copies:
r0-local-prerequisite-failure.json and r1-local-prerequisite-failure.json. These
remain failure observations, not accepted native reports. Log digests identify
the sandbox observation; real acceptance needs fresh reports AND phase logs.

## Source fixes in this completion pass

- Closure requires full phase inventory, actual source/lock inventory hash,
  native host/Ubuntu distribution, real binary digests, valid package digest,
  all (not merely last duplicate) automated outcomes and unique nonempty manual
  records. Empty/None values cannot compare equal to create approval.
- Package verifier rejects malformed/scalar/bool byte/mode/digest fields and
  setuid-style modes. Build calls the complete R0 verifier. Existing output is
  preserved; empty gzip filename removes output-name header drift.
- R1 failure paths clean any identified session; output cannot mutate package/
  checkout. Timeout must match actual supervisor exit 1/lifetime message and
  tolerates stderr capture prefixes, not unrelated failures.
- Actual Ubuntu 24.04 distribution is recorded/required; another Linux host
  cannot be relabeled as accepted baseline. Existing workspace privacy failure
  is deliberate and never silently chmodded.
- Reviewed Windows workflow added: workflow_dispatch/default branch only,
  worldwright-r0-windows environment, acceptance-only test DB secret, required
  log retention. Configure protection BEFORE secrets; YAML does not enforce
  organizational reviewer adoption by itself.

## Next admissible acceptance actions

Use the completed guide on actual approved hosts. Commit/review the exact source
(or explicitly allow a fingerprinted reviewed worktree), run both native R0
reports including disposable DB/Helm, fix the FIRST genuine failure, then build
and exercise the exact private package. Perform/authenticate all manual cases
without inventing versions/reviewers. Bring original report bytes and all logs
for paired validation. Only authentic complete evidence can change these states.
R0/R1 do not grant licensing, security certification, sandboxing, SDK ABI,
production capacity, or R2+ readiness. P0/P1/P2 acceptance remains independently open.
