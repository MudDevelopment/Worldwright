# R6 — security/recovery/retention execution and evidence

Authority: ROADMAP_v2.md R6. Integrated proof requires current R2/R3. This first
implementation batch supplies real-binary browser validation, native-test
orchestration and evidence integrity, not complete R6 implementation or closure.
All eight roadmap items remain tracked in verification/r6-implementation-ledger.md.
A browser pass is not a backup/restore drill or independent security approval.

## Implemented collection tools

| Tool | Contract |
|---|---|
| scripts/r6-browser.py | Own new sample workspace; actual supplied Rust Studio/preview binaries and actual Chromium; no fixture HTTP server, remote target or publication credentials |
| scripts/r6-native.py | Pinned native source/config/build tuple; actual browser first after build, then discovered/executed native suites and Clippy; failure stops later phases |
| scripts/r6_support.py | Shared bounded phase capture via r3_io, external create-new evidence, fsynced progress journal, manifest and source snapshot |
| scripts/r6-evidence.py | Read-only structural/hash/test-count/native-browser tuple validation; never approves or signs R6 |
| scripts/r6-check.ps1 | Single-command Windows orchestration without reliance on previously set shell variables |
| scripts/test-r6.py | Fourteen synthetic collector/validator regressions; these are not native game/browser/security acceptance |

Browser cases: real HTTP readiness (a printed URL is insufficient), DOM import/
edit/save/reload/validate, both game packages and meaningful native preview
commands, unchanged artifact bytes and matching game/payload/revision, missing
publication authority, hostile Host/Origin/wrong content type rejection, and no
browser script errors/external HTTP requests. Screenshots, artifacts, draft and
process logs remain in the private new evidence directory. This two-room smoke
is NOT maximum-size, full keyboard/screen-reader, OIDC or actual publication proof;
continue the complete R5_CHECK_PLAN matrix separately.

Unix adds actual SIGINT with active preview, child-exit observation, sentinel
removal and same-workspace reopen. Windows forcibly cleans up only the NEW test
process after completed preview workflows and records shutdown as MANUAL PENDING.
It does NOT mislabel CTRL_BREAK/TerminateProcess as the app's Ctrl+C contract.
The disposable test workspace's sentinel can remain. Do not remove it as an
automatic cleanup step or reuse that workspace as fresh evidence.

The runner is not a process-isolation certification: hostile reparenting/escape,
OS multi-user ACL/race/power-loss review and interrupted collector cleanup still
need native/manual testing. It does not touch an already running user Studio.
Stop your existing development Studio gracefully BEFORE rebuilding its binaries
on Windows, or the executable may be locked. No cargo clean is needed.

## Windows: prepare once, then run one command

Use native Windows, Python 3.11+, Rust/Cargo 1.85.1 with rustfmt/Clippy, Node 20+
and the approved locked crate cache. Only install packages/browser after local
approval. This prepares an external venv, not a directory under the checkout:

```powershell
Set-Location 'E:\Hold\Worldwright'
$venv = Join-Path $env:LOCALAPPDATA 'Worldwright-R6-Python'
python -m venv $venv
if ($LASTEXITCODE -ne 0) { throw 'venv creation failed' }
$python = Join-Path $venv 'Scripts\python.exe'
& $python -m pip install -r scripts/r6-requirements.txt
if ($LASTEXITCODE -ne 0) { throw 'dependency installation failed' }
& $python -m playwright --version
if ($LASTEXITCODE -ne 0) { throw 'Playwright unavailable' }
```

Use an approved Playwright Chromium cache (PLAYWRIGHT_BROWSERS_PATH if needed),
or pass -BrowserExecutable with an approved Chromium executable. Installing the
Python wheel alone does not provision Chromium. Where policy explicitly allows
native-machine browser provisioning, use that venv's `python -m playwright install
chromium`; no browser download is performed by the collectors or chat sandbox.
Do not weaken execution policy if PowerShell refuses a script.

Close the development Studio with Ctrl+C. The following SINGLE command then
runs collector unit tests, the native/browser collection and integrity validation:

```powershell
& 'E:\Hold\Worldwright\scripts\r6-check.ps1' -Python "$env:LOCALAPPDATA\Worldwright-R6-Python\Scripts\python.exe" -AllowDirty
```

-AllowDirty explicitly records the current uncommitted worktree instead of
pretending it is a clean release. Omit it when collecting reviewed clean source.
Do not edit the repository, reformat source, replace binaries or run another build
against that target while collection is running. It reuses the established
external native development cache; this is not fresh R0 acceptance.

Each phase name prints before execution. Output is bounded in its private phase
log; do not interpret a quiet compiler phase as a hang before its deadline.
If format fails, retain that failed collection, deliberately run the pinned
formatter and review the diff, then start a NEW collection. Never rewrite the
old source hashes or continue only the later phases into a passing old report.
If offline dependencies are missing, obtain the approved cache without changing
the lockfile, then rerun. Expected focused R5 discovery: 19 Windows / 21 Unix.
Unexpected zero/duplicate/ignored required tests fail, rather than successful skip.

The script prints a unique LOCALAPPDATA/Worldwright-R6-* output directory.
Inspect its Windows ACL under local policy BEFORE sharing or using sensitive
evidence. Mode 0600/0700 requests are not proof of a protected Windows DACL.

## Ubuntu 24.04 native execution

Use an ordinary non-root user and approved dependencies/browser. Create the venv
and evidence parent outside the repository, with private ownership. Then:

```bash
set -euo pipefail
umask 077
export PYTHONDONTWRITEBYTECODE=1
export CARGO_TERM_COLOR=never
python3 scripts/test-r6.py
python3 scripts/r6-native.py --output "$HOME/r6-evidence-new" --target-dir "$HOME/r6-native-cache" --allow-dirty
python3 scripts/r6-evidence.py --report "$HOME/r6-evidence-new/r6-report.json" --check-local
```

Activate the approved venv first if its Python is not python3. The output directory
must NOT already exist; use a new name for each run. Target/evidence paths must be
disjoint and outside checkout. The native collector enforces actual Ubuntu 24.04
and rejects non-native/compiler substitution overrides. Use R0 for the full fresh
paired-host workspace/DB/chart/rustdoc gate; this collector is narrower.

## Actual phase and report boundaries

Native order: pinned tool/OS/Node checks -> full tracked/untracked-not-ignored
source inventory plus Cargo-config hashes -> fmt --check -> locked graph -> two
binary build -> real browser -> discovered/executed R5, entire Studio, operations,
release, console, terminal, maturity, retained SDK -> selected-package Clippy ->
Studio Node fixtures -> unchanged source/config/binary check. No ignored SQLx tests
are silently counted: this is explicitly the NON-DB prerequisite collection.
The separate full-workspace check and all R2 database/provider/fault suites remain.

- r6-report.json: report with full_r6_closed=false and independent_adoption=false.
- events.jsonl: fsynced started/finished/case/failure progress; interrupted phases
  lacking a finish record are not presumed successful.
- source-before.json / source-after.json: actual source inventories; equality is
  required, but the R6 recipe is not interchangeable with an existing R0 digest.
- phase-name.log: bounded output, digest, actual exit and interruption reason.
- browser/r6-report.json: linked binary identities, actual cases/version,
  screenshots and artifacts. Windows manual shutdown is explicitly pending.

Exit 0 means the specified collection/validator completed, NOT R6 acceptance.
Failed runs remain failed reports and logs. Missing tools are failures, never
substitutes. A browser-only direct run has binary hashes but no native build/source
binding; only the native collector links its own build/source to that browser run.
A process kill of the collector may leave only partial events/files: preserve
those and verify native processes have stopped before recovery. A file/log bound
failure is not a truncated success. Known environment secrets are masked on a
best-effort basis; use only synthetic data and review logs before sharing.

`r6-evidence.py --check-local` additionally requires the actual current source,
binaries and known Cargo configs to match the collection. Without it, only the
contained evidence bundle is checked. Raw evidence hashes are byte-exact; preserve
line endings. These reports are unsigned. A maliciously rewritten report and
matching hashes are NOT authenticated acceptance; independent review/signing and
trusted provenance remain required. The collector refuses closure flags and has
no command that issues a security/retention approval.

## Remaining R6 requirements: execution and adoption checklist

| R6 requirement | Required next work and evidence; not supplied by this batch |
|---|---|
| 1 Issuer/key trust | Read current console/operations authority flows; inventory signing-key holders separately from bearer-token holders. Demonstrate grants cannot confer issuer signing authority. Review issuer rotation/revocation, storage and named custodians. Keep signing keys out of evidence/browser/preview. |
| 2 Grants/session/recovery authority | On approved isolated R2/R3 deployment run current-grant, suspended/revoked/offboarded account, assurance/MFA, break-glass, automation and interrupted-job cases. Record denied versus committed effects and required audit. Console/operations unit tests alone are insufficient. |
| 3 OS isolation | Execute existing Linux private-slot checks with different real users; permissions/links/ancestor races/timeout/orphan cases. Verify Windows token/key DACL and reparse/race behavior with designated security reviewer. Complete real browser/auth and R5 maximum-size/manual matrix. |
| 4 Real backup/restore/takeover | Follow docs/operations/BACKUP_RESTORE.md with real server/pg_dump/pg_restore tools, current accepted migrations and an approved isolated EMPTY target. Record dump/manifest/source/artifact/schema identity, corruption rejection, restore checks, old authority fencing and explicit takeover. Never use production or automatically choose a DB URL. |
| 5 Recovery objectives | Owner explicitly adopts or revises the implemented 24h RPO / 4h RTO / 90-day audit targets. Measure UTC capture/failure/restore/verification times, loss window and actual downtime in drills. Configured constants are not observed guarantees. |
| 6 Retention/privacy | Inventory ordinary logs, rendered receipts, OIDC identity/security audit, backups and evidence. Assign purpose/access/encryption/retention/deletion/legal holds and owners. Exercise cleanup and restored replay/business-entitlement uniqueness; do not delete receipts or business-uniqueness records by an arbitrary age-only script. |
| 7 External effects | External dispatch remains unsupported. Obtain a named defer/out-of-scope decision or implement/review bounded retry/idempotent sink/deduplication before changing that claim. Restore must not replay previously delivered effects. A backlog metric is not a dispatcher. |
| 8 Supply chain/support | Use existing release-evidence.py for real Cargo SBOM data at the actual artifact/container tuple. Independently review advisory freshness, notices/licenses/provenance/signing policy. Name private reporting route, responders and realistic coverage. Existing hashes/SBOM declarations do not grant a license or authenticate an approval. |

Existing references: docs/operations/OPERATOR_CONTROL.md,
docs/operations/BACKUP_RESTORE.md, SECURITY.md, docs/R2_ACCEPTANCE.md,
docs/R3_ACCEPTANCE.md and docs/R5_CHECK_PLAN.md. This batch inspected the R6
roadmap, Studio/preview/HTTP sources and existing collector/runbook contracts;
it did NOT complete a file-by-file audit of all security/persistence code.

Integrated R6 acceptance is blocked by the known full-migration SQLstate 42601 at
20261106000100_operational_control.sql:188, missing current R2/R3 closed tuples,
native/manual evidence and unadopted owner/security/privacy/support decisions.
No applied SQL/checksum repair or policy approval is inferred from this request.
No changes to the existing workspace/artifact/config schema, beta thresholds,
HMAC authority, retention behavior or production deployment are included here.

## Independent handoff and stop conditions

An independent operator must receive the exact source/lock/binary/game/artifact/
DB/schema/deployment tuple, complete native/browser/live drill logs, declared
failures/skips, cleanup observations and a bounded residual-risk list. They must
execute the applicable runbooks and record their own identity, authority, time,
evidence references and decision through the approved signing/review process.
Blank owner roles, developer-written assertions, unsigned JSON or an exit-0
collector are not adoption. No R6 final closure/signing tool is implemented in
this first batch. R0–R5 and R7 gates retain their existing independent meanings.
