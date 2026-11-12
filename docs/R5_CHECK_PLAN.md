# R5 check plan — current source, both hosts, no implied closure

Authority: [ROADMAP_v2.md](../ROADMAP_v2.md), R5 items 1–7. Status belongs in
[the R5 ledger](verification/r5-implementation-ledger.md). Recovery procedures
belong in [R5_ACCEPTANCE.md](R5_ACCEPTANCE.md). This plan does not authorize a
migration repair, production test, credential change, deployment or approval.

## Execution order and evidence

Run the pre-format integrity check below, then A–F. Stop at the first unexpected failure; retain its complete log and
fix the actual cause before continuing. Reuse the known Windows cache for this
development pass; do not repeatedly rebuild from scratch. Formal R0 acceptance
later requires a NEW external target/evidence directory and the same source on
both hosts. Never relabel the cached development pass as fresh R0 acceptance.

| ID | Check | Required result / execution boundary |
|---|---|---|
| A | Source, tools, formatting and inventory | Correct checkout; Rust/Cargo 1.85.1, native host, locked dependencies; reviewed fmt diff |
| B | Focused R5 native discovery and tests | Nonzero expected discovery; all applicable R5 tests actually execute; no unexpected skip |
| C | Entire Studio suite and Clippy | Unit/integration/doctests plus all-target/all-feature lint, warnings denied |
| D | Terminal, generated consumers and retained SDK | Both game profiles play; moved consumers resolve only relocated SDK paths; locked rebuild/integrity unchanged |
| E | Full workspace continuation | fmt/check/Clippy/tests/build/rustdoc pass; ignored DB tests are not counted as live passes |
| F | UI and source-contract fixtures | Node syntax/lifecycle/capabilities/races; real Chromium fixture; P2 positive and negative checks |
| G | Acceptance-tool regressions | R0/R1/R2/R3/R4 helpers, actual tool versions and platform skips recorded |
| H | Helm acceptance | Pinned Helm v3.19.0; schema/semantic, actual lint/render and all negative profiles; no cluster mutation |
| I | Disposable database/SQLx | Unchanged full migration inventory, all discovered ignored persistence tests, cleanup; currently blocked by known SQL syntax error |
| J | Actual native Studio/backend | Both games; maximum-size multibyte drafts; exact artifact/preview identity; real auth/HTTP responses and no replay |
| K | Native recovery and security | Session lifecycle/kill/restart, journal interruptions, permission failures, Windows DACL/reparse/path and Unix owner/link checks |
| L | Supported desktop/browser matrix | Actual Ubuntu X11/Wayland Chromium/Firefox, no-browser launch, native Windows, keyboard/zoom/screen reader |
| M | Same-source R0/R1 | Fresh paired-host R0, private installed package, actual R1 automation plus authenticated human observations |
| N | Same-source R2/R3 | Live transaction/provider recovery, approved identity adoption, pushed image/real roles/13 staging phases/service adoption |
| O | R4 measurement/adoption | Unchanged beta-250, seven separate scenarios including 1800–3000 s soak, private monitor, signed retention/performance adoption |
| P | Independent R5 builder exit | Outside original checkout, reviewed artifact equals previewed and activated artifact; actual status/rollback; independent sign-off |
| Q | Final evidence and documentation | Source/lock/host/binary/report/log identities, failures/skips, doc links and remaining gates retained |

A–Q are required checks, not a claim they all have automated runners or passed.
J–P require native/approved environments and some independently witnessed steps.

## Pre-format integrity check — do this before changing source

Use Python 3.11+ from the complete checkout. The verifier is read-only, requires
no packages, and never updates the report or closes a gate:

```powershell
Set-Location 'E:\Hold\Worldwright'
python scripts/test-r5-evidence.py
if ($LASTEXITCODE -ne 0) { throw 'Evidence verifier self-tests failed' }
python scripts/verify-r5-evidence.py --root .
if ($LASTEXITCODE -ne 0) { throw 'Retained evidence does not match; stop before formatting' }
```

Ubuntu equivalents are `python3 scripts/test-r5-evidence.py` and
`python3 scripts/verify-r5-evidence.py --root .`, under `set -euo pipefail`.
Expected: seven verifier regressions, 17 source MATCH lines, one evidence-log
MATCH, exit 0. Exit 1 means mismatch; exit 2 means missing/invalid/unsupported
input. This verifies agreement with a supplied unsigned report, not authenticity,
complete checkout, current native execution or a release signature.

Sources are decoded as UTF-8, universal-newline normalized, then hashed without
trimming whitespace/final LF/BOM. The log hash is BYTE-EXACT. `.gitattributes`
currently pins LF for shell/Rust/TOML/YAML but not `.log`; a Windows checkout may
convert a text log. The verifier diagnoses an LF-equivalent log but still fails
its exact-byte check. Preserve the received file and record `git ls-files --eol`
and `git check-attr text eol -- docs/verification/r5-local-contracts.log`; do not
edit the report hash or silently convert the original. Exact-byte export/retention
policy remains an open portability decision. Verify before `cargo fmt --all`;
legitimate later formatting will make this historical source snapshot mismatch.

## A–E: Windows development pass (PowerShell 7)

Use the previously identified cache only if it is still the intended native cache.
If absent, choose a new private external target deliberately. Bootstrap only if
the approved pinned toolchain/dependency cache is missing; do not regenerate the
lockfile or silently replace the compiler to get a pass. Commands below set the
current shell's Cargo target/output variables intentionally.

```powershell
Set-Location 'E:\Hold\Worldwright'
$ErrorActionPreference = 'Stop'
function Invoke-R5Checked {
    param([string]$Program, [string[]]$Arguments)
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Program failed: $LASTEXITCODE" }
}
$targetDir = Join-Path $env:TEMP 'worldwright-native-check-e73e9337f80540e5a64ef73e18782200'
$env:CARGO_TARGET_DIR = $targetDir
$env:CARGO_BUILD_TARGET = 'x86_64-pc-windows-msvc'
Invoke-R5Checked git @('rev-parse','--show-toplevel')
Invoke-R5Checked git @('status','--short')
Invoke-R5Checked git @('rev-parse','HEAD')
./scripts/dev.ps1 doctor
if ((& rustc --version) -notmatch '^rustc 1\.85\.1(\s|$)') { throw 'Wrong rustc' }
if ((& cargo --version) -notmatch '^cargo 1\.85\.1(\s|$)') { throw 'Wrong Cargo' }
if ((& rustc -vV) -notcontains 'host: x86_64-pc-windows-msvc') { throw 'Wrong native host' }
if (Test-Path Env:WORLDWRIGHT_R5_SESSION_TEST_ROOT) { throw 'Remove unintended test-helper override before testing' }

# Intentional source formatting; review the diff, preserving existing user edits.
Invoke-R5Checked cargo @('fmt','--all')
Invoke-R5Checked git @('diff','--check')
Invoke-R5Checked cargo @('fmt','--all','--','--check')
Invoke-R5Checked cargo @('metadata','--format-version','1','--no-deps','--locked','--offline')

# Discover first. A zero-test invocation is not evidence.
Invoke-R5Checked cargo @('test','-p','worldwright-studio','--lib','--locked','--offline','r5_','--','--list')
Invoke-R5Checked cargo @('test','-p','worldwright-studio','--lib','--locked','--offline','r5_','--','--nocapture')
Invoke-R5Checked cargo @('test','-p','worldwright-studio','--locked','--offline')
Invoke-R5Checked cargo @('clippy','-p','worldwright-studio','--all-targets','--all-features','--locked','--offline','--','-D','warnings')
Invoke-R5Checked cargo @('test','-p','worldwright-expedition','--lib','--locked','--offline','terminal::tests','--','--nocapture')
Invoke-R5Checked cargo @('test','-p','worldwright','--test','maturity','--locked','--offline','--','--test-threads=1','--nocapture')
Invoke-R5Checked cargo @('test','-p','worldwright','--test','sdk_compatibility','--locked','--offline','--','--test-threads=1','--nocapture')

# Full current workspace gate, reusing the explicit native cache above.
./scripts/dev.ps1 check
Invoke-R5Checked git @('diff','--check')
Invoke-R5Checked git @('diff','--exit-code','--','Cargo.lock')
```

Keep private per-phase logs (or a PowerShell transcript for this NON-credential
pass), including failures and test counts. Do not include real token/DB values.
`dev.ps1 check` repeats focused coverage intentionally as one coherent workspace
result: formatting, all-target check, Clippy, tests/doctests, build and rustdoc
with warnings denied. It does not run ignored DB tests or make an R0 report.

The current source declares 21 `r5_` tests: 19 on Windows, 21 on Unix (two are
Unix-only). Confirm compiler discovery against the actual source if it changes.
`r5_status_server` is a helper, not a test. `session_process_helper` is exercised
by the cross-process parent test; its standalone no-op is not crash evidence.

## A–E: Ubuntu 24.04 native counterpart

Run as an ordinary user from the complete checkout, with the approved toolchain
and cache. Use a deliberately chosen absolute external target path; do not reuse
Windows outputs. This is a development pass; fresh R0 is separate.

```bash
set -euo pipefail
bash scripts/dev.sh doctor
rust_version="$(rustc --version)"
cargo_version="$(cargo --version)"
[[ "$rust_version" == 'rustc 1.85.1' || "$rust_version" == 'rustc 1.85.1 '* ]] || { echo 'Wrong rustc'; exit 1; }
[[ "$cargo_version" == 'cargo 1.85.1' || "$cargo_version" == 'cargo 1.85.1 '* ]] || { echo 'Wrong Cargo'; exit 1; }
rust_details="$(rustc -vV)"
native_host="$(printf '%s\n' "$rust_details" | sed -n 's/^host: //p')"
[[ "$native_host" == *-linux-* ]] || { echo 'Not a native Linux host'; exit 1; }
export CARGO_BUILD_TARGET="$native_host"
: "${CARGO_TARGET_DIR:?Set an absolute private native development target directory}"
[[ "$CARGO_TARGET_DIR" == /* ]] || { echo 'Target must be absolute'; exit 1; }
test -z "${WORLDWRIGHT_R5_SESSION_TEST_ROOT:-}"
cargo fmt --all
git diff --check
cargo fmt --all -- --check
cargo metadata --format-version 1 --no-deps --locked --offline
cargo test -p worldwright-studio --lib --locked --offline r5_ -- --list
cargo test -p worldwright-studio --lib --locked --offline r5_ -- --nocapture
cargo test -p worldwright-studio --locked --offline
cargo clippy -p worldwright-studio --all-targets --all-features --locked --offline -- -D warnings
cargo test -p worldwright-expedition --lib --locked --offline terminal::tests -- --nocapture
cargo test -p worldwright --test maturity --locked --offline -- --test-threads=1 --nocapture
cargo test -p worldwright --test sdk_compatibility --locked --offline -- --test-threads=1 --nocapture
bash scripts/dev.sh check
git diff --check
git diff --exit-code -- Cargo.lock
```

Record actual host and verify Rust AND Cargo 1.85.1 before proceeding. Never
turn Linux syntax/grammar or cross-compilation into native Windows evidence.

## F: browser and P2 fixtures (both hosts)

Node 20+ and Python 3.11+ are required. Run each command and stop on nonzero exit
(use `Invoke-R5Checked` in PowerShell; `set -euo pipefail` in Bash).

```bash
node --version
node -e "if (Number(process.versions.node.split('.')[0]) < 20) process.exit(1)"
node --check apps/worldwright-server/assets/app.js
node --check apps/worldwright-studio/assets/app.js
node scripts/test-expedition-browser.cjs
node scripts/test-studio-capabilities.cjs
node scripts/test-studio-workflow.cjs
python scripts/test-p2-contracts.py
python scripts/test-p2-negatives.py
```

Create an approved external Python venv and use its interpreter for all Python
commands. Install `scripts/r5-requirements.txt` into it, then run:

```bash
python -m pip install -r scripts/r5-requirements.txt
python scripts/test-studio-browser.py
```

This fixture uses Playwright 1.56.0 and expects an approved Chromium installation
available through Playwright's browser cache/path. It does not download a browser.
Do not assume an arbitrary system Chrome is discovered automatically. Missing
browser is a failed prerequisite, not a successful skip. Five groups cover actual
DOM bounds, Unicode bytes, async saves, disabled auth/recovery labels and keyboard
reconciliation; 18 Node workflow cases cover deterministic races. Neither runs
Rust Studio, PostgreSQL or a real operator and neither certifies accessibility.

## G–H: adjacent helper and chart checks

Use an external venv. Install the actual test-only requirements, not new runtime
dependencies. G's R0/R1/desktop commands below target an ordinary Linux user;
record Windows-specific skips where a supported helper is also run on Windows.

```bash
python -m pip install -r scripts/r2-requirements.txt -r deploy/helm/worldwright/tests/requirements.txt
python scripts/test-r0-r1.py
python scripts/test-desktop.py -q
python scripts/test-r2.py
python scripts/test-r2-live.py
python scripts/test-r3.py
python scripts/test-r3-local.py
python scripts/test-r3-io.py
python scripts/test-r4.py
python -m unittest discover -s deploy/helm/worldwright/tests -p test_verify.py -v
helm version --short
python deploy/helm/worldwright/tests/verify.py
```

For H use Helm v3.19.0, as pinned in current CI. The final harness includes actual
lint/render and negative cases; schema-only checks are insufficient. Optional
wrapper equivalents are `bash deploy/helm/worldwright/tests/render.sh` or
`./deploy/helm/worldwright/tests/render.ps1`; do not count wrappers as additional
independent coverage. The synthetic desktop run skips its two native cases until
real binaries are supplied. No PostgreSQL or cluster is used by G/H's fixtures;
local TLS/process fixtures are not approved-provider or deployed-role evidence.

## I: disposable PostgreSQL — blocked prerequisite, not routine development

Get explicit approval for a disposable loopback `worldwright_test` database and
private credential delivery. The test role can create/drop test schemas; never
use production. Requirements and cleanup are in [R2_ACCEPTANCE.md](R2_ACCEPTANCE.md).
The full unchanged migration inventory currently fails at
`migrations/20261106000100_operational_control.sql:188`, reserved `authorization`,
SQLstate 42601. Do not edit migration bytes/checksums, omit that test or manufacture
a successful ledger. Record the failure until an authorized repair is defined.

After approved provisioning, commands for the SQL/native gate are:

```bash
python scripts/test-r2-expiry-sql.py
cargo test -p worldwright-persistence --locked --offline -- --ignored --list
cargo test -p worldwright-persistence --locked --offline -- --ignored --test-threads=1
```

`WORLDWRIGHT_TEST_DATABASE_URL` must already be privately configured. Compare the
actual discovered/executed ignored-test inventory; no successful skip is allowed.
Stop after the migration failure rather than running doomed dependent collectors.
The separate `test-r3-role-fixture.py` requires its own approved disposable
role/database-create cluster and explicit `WORLDWRIGHT_R3_ROLE_FIXTURES=1`; follow
[R3_ACCEPTANCE.md](R3_ACCEPTANCE.md). Its synthetic permission schema never repairs
application migrations or proves deployed credentials.

## J–L: native/manual matrix not replaced by fixture tests

Use private disposable workspaces and exact newly built binaries, not stale
installed assets. Do not fault-inject a real builder's working workspace.

| Area | Positive / negative cases to record |
|---|---|
| Backend bounds | 1024 rooms, 64 exits, exact multibyte limits; reject one-over; save/import/export round trip and backend diagnostics |
| Artifact/game | Reference and expedition; save/validate/package; identity from immutable bytes; preview meaningful actions; stale draft/unsaved edits refuse or require documented confirmation; reopen verifies actual manifest game |
| Async/UI | Rapid room switching; edits during save/import; conflict; duplicate clicks; late validation/preview/list results; input acknowledgment; timeout/oversize/redirect failures |
| Auth | Token-file renewal and startup-only environment mode; invalid/missing/expired/revoked/wrong-grant token; 401/403; no credential in page/log/preview; no automatic POST replay |
| Reconciliation | Known exact ID matches artifact/revision/payload/original receipt; repeated status idempotent; reject mismatched ID/context, missing receipt, pending/corrupt/full history; unknown ID stays unresolved |
| Operation gate | Concurrent submit/refresh/reconcile/recover returns conflict rather than queued duplicate mutation |
| Session | Second updated serve refuses; normal exit releases; real forced kill and uncertain cleanup retain sentinel; operator-only recovery; no PID/age reclamation; simultaneous first-start reviewed separately |
| Journal | Every supported file layout; next-only/all-three/malformed/regressive/mismatched/oversize/link cases preserve evidence; failure at write/sync/each rename/removal; restart and authoritative status refresh |
| Filesystem | Full disk, permission denied and directory-sync failure; Linux owner/mode/symlink/hard-link; Windows DACL, junction/reparse, ancestor swap, share restrictions and cleanup; real power-loss is separate from killing a process |
| Desktop | Native exact-binary first run, spaces/fresh HOME/XDG, no-browser, graceful Ctrl+C/TERM/HUP on Unix, Windows Ctrl+C; preview tree cleanup and reopen |
| Accessibility | Actual Chromium/Firefox × X11/Wayland, Windows supported browser; keyboard-only focus/order/actions/errors, screen reader labels/live announcements, zoom/high-DPI/reflow/reduced motion |

The R5 native suite covers a subset of these cases; no claim is made that it
injects real disk-full/power-loss or certifies Windows DACLs. Preserve precise
unsupported/unobserved entries. Session recovery and local-record recovery do
not confer remote review/activation authority.

## M–P: real acceptance and independent adoption

Follow the linked authoritative runners with approved real inputs; do not invent
plan values or keys, and do not run these against production.

- **M:** [R0_R1_ACCEPTANCE.md](R0_R1_ACCEPTANCE.md): fresh `r0-verify.py` on both
  hosts; `package-desktop.py build/verify`; `r1-desktop-verify.py`; authenticated
  manual matrix and `r0-r1-close.py`. The existing R1 crash case checks session
  cleanup then uses a different fresh XDG workspace; it is NOT proof of recovery
  and reopen of the crashed workspace. Exercise K separately.
- **N:** [R2_ACCEPTANCE.md](R2_ACCEPTANCE.md): current native SQLx, process COMMIT
  fault matrix, controlled HTTPS/OIDC, actual approved provider and independent
  adoption; `r2-verify.py collect/close`. Then [R3_ACCEPTANCE.md](R3_ACCEPTANCE.md):
  approved immutable image build/push, real role/Secret audit, all 13 staging
  phases, independent CNI/termination/revocation/rollback and signed service
  adoption; `r3-image.py`, `r3-roles.py`, `r3-staging.py`, `r3-close.py`.
- **O:** [R4_ACCEPTANCE.md](R4_ACCEPTANCE.md): approved signed plan and current
  closed R2/R3; `r4-verify.py collect/close`, optionally `compare` on matched
  environments. Keep beta-250-v1 at 250 clients × 30 commands, 2-second periods,
  existing thresholds, zero failures. Tooling smoke is not beta. Run all seven
  separate scenarios, including the adopted 1800–3000-second soak, protected
  CPU/RSS/PG/metrics monitor, growth/retention and performance adoption. Do not
  relax thresholds or call a sequential benchmark concurrency evidence.
- **P:** [R5_ACCEPTANCE.md](R5_ACCEPTANCE.md) and
  [RELEASE_ACTIVATION.md](operations/RELEASE_ACTIVATION.md): independent builder
  outside original checkout uses verified private SDK/installed tools and both
  meaningful game consumers. Actual Studio request, different authorized reviewer,
  maintenance activation, active-state/readiness verification and separately
  reviewed rollback must retain the identical reviewed/previewed/activated digest.

R6 wider security/recovery/legal/support decisions remain separate required
release decisions; this R5 checklist neither closes them nor redefines their scope.

## Step-by-step execution and private evidence capture

### 1. Prepare the native machine, without credentials

Use PowerShell 7 on native Windows or an ordinary Ubuntu 24.04 account, a full
checkout and approved Rust/Cargo 1.85.1 with rustfmt/Clippy. Verify Python 3.11+
and Node 20+. Do not put production tokens, database URLs, signing keys or player
data in this first pass. The sandbox cannot substitute for these native runs.

For the Windows commands in A–E, create a new external evidence directory first:

```powershell
$evidence = Join-Path $env:LOCALAPPDATA ('Worldwright-R5-Evidence-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $evidence -ErrorAction Stop | Out-Null
Get-Acl $evidence | Format-List Owner,AccessToString
# Confirm the directory is private under your approved Windows ACL policy.
Start-Transcript -Path (Join-Path $evidence 'native-development.log') -NoClobber
Get-Date -Format o
```

Keep the same PowerShell session. Run pre-format verification, then A–E in order.
If any command throws, stop and retain the transcript with `Stop-Transcript`;
do not continue to a later stage and report only its success. Record which phase
failed. Do not overwrite a prior transcript when resuming. A first formatting
failure is a finding: retain the diff, then deliberately run the documented
formatting command and collect new native results. Do not promote the old hash
snapshot to the newly formatted source.

If pinned tools/dependencies are missing, use `./scripts/dev.ps1 bootstrap`
(or `bash scripts/dev.sh bootstrap`) ONLY after approved network/cache access.
Bootstrap uses the committed lockfile; it is not permission to regenerate it.
Then rerun doctor and verify the exact pin. For Ubuntu, set an explicit private
external native target directory, log phases under an external directory with
`umask 077`, and use `set -euo pipefail` when piping output through `tee`.

### 2. Read each native result, not just the shell exit

R5 discovery must list the applicable 19 Windows / 21 Unix tests. The next
command must actually run them; list-only output is not execution. Confirm the
cross-process parent test ran, not only its standalone helper. Full Studio must
also run its existing tests/doctests. Retain terminal, maturity and retained SDK
outputs, including relocated executable actions. Full `dev check` must reach
build and rustdoc; an early stop leaves all later phases unverified.

At the end, inspect `git status --short`, `git diff --stat`, `git diff --check`,
`git diff -- Cargo.lock`, and `git diff -- migrations`. Do not reset edits.
Hash the actual built `worldwright` and `worldwright-studio` binaries used in the
next step, not an unrelated installed copy. A development cache pass is not the
fresh R0 paired-host report.

### 3. Provision and run browser fixtures

In the same native Windows session, with the private evidence directory above:

```powershell
python -m venv (Join-Path $evidence 'python-venv')
if ($LASTEXITCODE -ne 0) { throw 'venv failed' }
$python = Join-Path $evidence 'python-venv\Scripts\python.exe'
& $python -m pip install -r scripts/r5-requirements.txt
if ($LASTEXITCODE -ne 0) { throw 'Playwright package installation failed' }
& $python -m playwright --version
if ($LASTEXITCODE -ne 0) { throw 'Playwright unavailable' }
# Use an approved preinstalled Playwright Chromium cache. If the organization
# explicitly permits browser download ON THIS NATIVE MACHINE, provision with:
# & $python -m playwright install chromium
# Check its exit code before continuing; never download through the chat sandbox.
& $python scripts/test-studio-browser.py
if ($LASTEXITCODE -ne 0) { throw 'Chromium fixture failed' }
```

Set `PLAYWRIGHT_BROWSERS_PATH` only to the actual approved cache if needed.
The Python package alone does not install its browser. Keep the reported actual
browser version. Run the Node/P2 commands in F with checked exit codes; expected
Studio counts are 18 Node cases and five Chromium groups. Other fixture counts
come from their actual output. For G/H, install the separate listed requirements
in this external venv and provision approved Helm v3.19.0 before execution.
No browser fixture pass replaces the native Studio workflow below.

### 4. Exercise actual Studio without publication credentials

**Preferred: run the installed launcher, not a pasted terminal transcript.**
Press Ctrl+C to cancel any partial console input, then paste only this command:

```powershell
& 'E:\Hold\Worldwright\scripts\start-studio-local.ps1'
```

Do not include `PS E:\Hold\Worldwright>`, `>>`, code-fence markers, or earlier
error/output lines. PowerShell treats `PS` as Get-Process and `>`/`>>` as file
redirection, so copying a transcript can execute unintended commands/writes.
If script execution is blocked by organizational policy, retain that error and
use the approved signing/execution process; do not weaken policy automatically.

The launcher initializes paths itself, requires native Rust/Cargo 1.85.1,
incrementally builds only the two required executables using the existing cache,
and starts a NEW sample workspace. It prints the workspace path. After confirmed
graceful shutdown, `-Workspace` with that exact existing path reopens it without
`--content`. It never removes a session sentinel or changes execution policy.
Native PowerShell execution of this newly installed launcher remains unverified.

The longer inline alternative below remains available for review.
The launch block is self-contained: do not rely on variables surviving from A–E
or another PowerShell session. A missing target environment variable previously
produced a wrong `target/debug` path, and an unset evidence variable prevented
workspace construction. Neither failure established a Studio runtime defect.

Paste this as ONE block. It uses syntax intended for Windows PowerShell 5.1 or
PowerShell 7 (native execution remains unverified here), sets every path
explicitly, checks the pinned native tools, and incrementally
builds only the two required binaries with the existing cache. It does not clean
the cache, change Cargo.lock, or replace full E/native acceptance. Stop on the
first build error; do not attempt launch afterward. Repeated use creates a NEW
disposable workspace; preserve the printed path for same-workspace reopen tests.

```powershell
. {
    $ErrorActionPreference = 'Stop'
    Set-Location -LiteralPath 'E:\Hold\Worldwright'
    $nativeTarget = 'x86_64-pc-windows-msvc'
    $targetDir = Join-Path $env:TEMP 'worldwright-native-check-e73e9337f80540e5a64ef73e18782200'
    $env:CARGO_BUILD_TARGET = $nativeTarget
    $env:CARGO_TARGET_DIR = $targetDir

    $rustDetails = & rustc -vV
    if ($LASTEXITCODE -ne 0) { throw 'rustc inspection failed' }
    if ($rustDetails -notcontains 'release: 1.85.1' -or
        $rustDetails -notcontains "host: $nativeTarget") {
        throw 'Require Rust 1.85.1 on native Windows MSVC'
    }
    $cargoVersion = & cargo --version
    if ($LASTEXITCODE -ne 0 -or $cargoVersion -notmatch '^cargo 1\.85\.1(\s|$)') {
        throw 'Require Cargo 1.85.1'
    }

    $binDir = Join-Path (Join-Path $targetDir $nativeTarget) 'debug'
    $studio = Join-Path $binDir 'worldwright-studio.exe'
    $preview = Join-Path $binDir 'worldwright.exe'
    Write-Host "Expected Studio: $studio"
    Write-Host "Expected preview: $preview"
    $buildArgs = @(
        'build', '--locked', '--offline',
        '--target', $nativeTarget, '--target-dir', $targetDir,
        '-p', 'worldwright-studio', '-p', 'worldwright',
        '--bin', 'worldwright-studio', '--bin', 'worldwright'
    )
    & cargo @buildArgs
    if ($LASTEXITCODE -ne 0) { throw 'Targeted build failed; do not launch' }
    foreach ($exe in @($studio, $preview)) {
        if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
            throw "Build returned success but expected binary is missing: $exe"
        }
    }
    Get-FileHash -Algorithm SHA256 -LiteralPath @($studio, $preview)

    $evidence = Join-Path $env:LOCALAPPDATA ('Worldwright-R5-Launch-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $evidence | Out-Null
    $workspace = Join-Path $evidence 'native-studio-workspace'
    $content = (Resolve-Path -LiteralPath 'examples/two-room/world.json').Path
    Write-Host "Keep this workspace path: $workspace"
    & $studio serve --workspace $workspace --content $content --preview-command $preview --bind 127.0.0.1:0
    if ($LASTEXITCODE -ne 0) { throw 'Studio exited with an error; preserve its output and workspace' }
}
```

The dot-sourced block leaves its initialized variables available in this session.
For a SAME-workspace reopen after confirmed graceful shutdown, use the existing
variables and omit `--content`; do not rerun the initialization block. If the
session has changed, explicitly restore the printed workspace and exact binary
paths first. Verify private-directory permissions under your approved ACL policy
before using sensitive content; this example uses only the bundled sample.

Keep that terminal open while using the printed loopback URL. Confirm startup
remains running and HTTP loads; the printed URL alone is not proof startup
completed. Save/edit/validate/package reference, verify its digest/game, start
preview and exercise look/move/say/quit. Exercise expedition separately with its
appropriate draft, then the async/limits matrix J. Use browser developer tools
to observe actual requests/HTTP statuses without exposing credentials.

Close with Ctrl+C and confirm exit, preview-child cleanup and sentinel removal.
Restart the SAME workspace with the same command but WITHOUT `--content`; saved
revision/artifact identities must remain. In a second terminal, a second serve
for that workspace must fail without altering it. Forced-kill tests use only
separate disposable workspaces and K's recovery procedure; do not kill this
working workspace or equate forced exit with graceful cleanup.

### 5. Run recovery and authorization cases only in isolation

For each journal/process/security case in K, record the exact pre-state, action,
HTTP/error/exit, post-state hashes, files preserved and cleanup. The native R5
suite covers specified fixtures, not all manual disk-full/power-loss/ACL cases.
Stop all writers, preserve a private copy and establish ownership BEFORE any
manual sentinel removal; follow R5_ACCEPTANCE.md. Do not manually edit approval
records or repeatedly submit after a timeout. Native Windows DACL/ancestor-swap
and actual filesystem crash testing still need the designated security/operator
reviewer and approved fault-injection procedure.

Only after an approved isolated operator/R2 environment exists, add the scoped
loopback release endpoint and server-side token-file option from R5_ACCEPTANCE.
Do not paste the bearer token into a terminal command, browser or transcript.
Verify denied/expired/renewed access and known-ID reconciliation, then use the
separate authorized release-activation runbook with a different reviewer. Neither
Studio nor this check plan grants authority to approve, activate or sign adoption.

### 6. Database/live prerequisites and return package

Do not start I or M–P against production. Have the owner provision the guarded
loopback disposable DB and private credentials. Before running I, set
`WORLDWRIGHT_R2_SQL_REPORT` to a NEW file path in the external private evidence
directory so failed SQL output is retained too. Run the complete expiry/full-
migration script, retain the failure, and confirm every created-schema marker has
a matching dropped-schema marker. The known full-migration error blocks dependent
native SQLx/live collectors; do not rewrite the SQL/checksum or run around it.
R3 synthetic roles require separate create-role/database authority and their own
opt-in. R4 requires the real current closed prerequisite tuples and approved plan.

Return the private run path, timestamp/timezone, exact source/worktree identity,
tool/host versions, first failing phase if any, complete logs/counts/skips,
lock/migration/binary hashes and cleanup observations. Finish the non-credential
PowerShell transcript with `Stop-Transcript`. Sanitize any incidental secrets
before sharing; preserve the protected original under approved retention. Leave
all unexecuted matrix cells and independent approvals open.

## Q: what to return and retain

For each A–Q row record: status (verified / gap found / not yet checked), exact
command, timezone-qualified time, exit/counts/skips, source commit plus worktree
inventory, canonical lock hash, host/tool versions, relevant binary/artifact
hashes, private log/report paths and reason when blocked. Include failed output;
never overwrite an earlier report or transfer its pass to changed source.

Review `git diff --check`, `git diff --stat`, `git status --short`, lockfile and
migration diffs, local doc links, and recovery wording. Do not reset user edits.
Formatting changes invalidate earlier selected-source hashes; retain the old
record as historical and collect fresh native evidence after formatting.

The retained `verification/r5-local-observations.json` and
`verification/r5-local-contracts.log` describe only the enumerated LF-text
readbacks and local fixture execution. They are not full-checkout provenance,
signatures, native tests, CI or an acceptance-closing collector. Do not manually
edit their hashes to represent a later build. R5 and all outstanding R4 gates
remain OPEN until their actual evidence and independent adoption exist.
