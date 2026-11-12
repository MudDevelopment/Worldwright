# R0/R1 execution and acceptance

Authority: ROADMAP_v2.md section 5. These are release-track gates, not the P0/P1
priority groups. R0/R1 are NOT closed by implementation, synthetic tests, grammar
checks, configured workflows, or this guide. The ledger is
`verification/r0-r1-acceptance-ledger.md`.

## Prerequisites and evidence policy

- Ubuntu 24.04 LTS, ordinary non-root desktop user; native Windows for the paired
  R0 run and current credential/discovery checks. Python 3.11+, Rust/Cargo 1.85.1
  with rustfmt/Clippy, Node 20+, Helm 3 and approved test dependencies/cache.
- Linux runtime also requires Bash/coreutils, util-linux setsid, Python pidfd
  support, glibc/libgcc libraries reported by ldd. xdg-utils is optional for
  browser opening. Studio remains a loopback browser application, not egui.
- Supply WORLDWRIGHT_TEST_DATABASE_URL through approved secret delivery for a
  disposable database whose name contains `worldwright_test`; never production.
  PostgreSQL test fixtures create/drop schemas. Do not print a secret URL.
- Keep new evidence/output directories OUTSIDE the checkout, owned/private.
  Do not overwrite old reports or reuse old target directories. Review/commit
  changes before acceptance, or use --allow-dirty explicitly for a reviewed
  worktree recorded with its fingerprint. Formatting is done before a new run.
- Source inventories normalize CRLF/LF only for recognized text files; binary
  hashes remain byte-exact. Different content/lock/toolchain/package needs a
  new run. The paired reports must match commit, source and canonical lock.
- Missing tools, missing test discovery, failed stages, changed logs, empty or
  malformed digests and missing manual observations reject closure. A failure
  before prerequisite discovery may have no source tuple; it is not acceptance.

## Step 1 — current regression checks (Linux)

```bash
python scripts/test-r0-r1.py
python scripts/test-desktop.py -q
bash -n scripts/desktop.sh scripts/dev.sh
```

These include integrity/negative records, missing-tool refusal, source-inventory
checks, a real kernel pidfd check with a cooperative Python child, and synthetic
launcher contracts. Synthetic installed executables prove only launcher/tool
isolation; they do not prove Rust, native Studio or package acceptance. The two
real-binary cases explicitly skip unless both native paths are supplied.

## Step 2 — native R0 on both hosts

Use approved bootstrap/cache procedures from DEVELOPMENT.md. Apply formatting
before recording a tuple. Example Ubuntu commands, with a NEW output path:

```bash
rustup run 1.85.1 cargo fmt --all
python scripts/r0-verify.py --output "$HOME/Worldwright Evidence/r0-ubuntu"
# For deliberately reviewed uncommitted changes, add --allow-dirty.
```

Create the trusted private evidence parent explicitly first; do not chmod an
unknown existing directory automatically. On Windows, from the same source:

```powershell
Set-Location 'E:\Hold\Worldwright'
rustup run 1.85.1 cargo fmt --all
python scripts/r0-verify.py --output "$env:USERPROFILE\Worldwright Evidence\r0-windows"
if ($LASTEXITCODE -ne 0) { throw 'R0 failed; retain the report and fix the first failure' }
```

The runner records executable paths/versions, rustc host/sysroot, config hashes,
override presence without values, full locked all-feature dependency metadata,
fresh explicit host/target output and byte hashes of CLI/Studio. It rejects
compiler-substitution/bootstrap overrides. Required phases are fmt/check/Clippy/
test/build/rustdoc; browser ID and Studio bound fixtures; P2 positive/negative
checks; discovered live PostgreSQL receipt regression; real Helm matrix; and
on Linux real first-run/package/shutdown/reopen tests using those exact binaries.
Commands and private per-phase logs are hashed; source/lock are compared again
at the end. Correct the FIRST actual native failure, then make a new run rather
than reusing an old pass. Earlier Windows reports do not validate current edits.

## Step 3 — private package from passing Linux R0

```bash
python scripts/package-desktop.py build \
  --r0-report "$HOME/Worldwright Evidence/r0-ubuntu/r0-report.json" \
  --output "$HOME/Worldwright Evidence/worldwright-private.tar.gz"
mkdir -m 700 "$HOME/Worldwright Evidence/installed"
tar -xzf "$HOME/Worldwright Evidence/worldwright-private.tar.gz" \
  -C "$HOME/Worldwright Evidence/installed"
```

Use only your trusted freshly built archive, verify its externally retained
SHA-256, and use a private fresh extraction directory. The archive contains two
native debug binaries, the installed entry point, launcher/verifier/cleanup,
embedded Rust-compiled Studio assets, README and manifest. It excludes source
checkout, Cargo, database/account secrets, Rust SDK/cache and production assets.
ldd must resolve runtime libraries; their output is recorded. Stable archive
metadata/empty gzip filename avoid output-name/timestamp drift, not differences
between independently compiled binaries or tool/zlib versions. There is no
AppImage/.deb/menu installer, native GUI, signature or supported license grant.
Redistribution still requires applicable rights/notice/owner review.

Set PACKAGE to the single extracted `worldwright-private-...` directory:

```bash
python scripts/package-desktop.py verify --directory "$PACKAGE"
"$PACKAGE/bin/worldwright-desktop" --no-browser
```

The package root must be owned/private (0700). Verification rejects missing,
extra, symlink, invalid-mode or modified inventory files. Hashes do NOT authenticate
a publisher or a manifest edited together with its files; compare external
archive/evidence digests and authenticate the reviewer separately. Installed mode
never builds and needs no original checkout. Game source generation/compilation
still requires a matching SDK/development toolchain outside this runtime package.

## Step 4 — real installed R1 automation

Install the test-only scripts/r1-requirements.txt in an approved external venv.
Provide an approved existing Chromium/Chrome binary; no browser download is
performed by the harness. From outside the installed package/checkout:

```bash
python scripts/r1-desktop-verify.py --package-directory "$PACKAGE" \
  --browser-executable /usr/bin/google-chrome \
  --output "$HOME/Worldwright Evidence/r1-installed"
```

Use the actual installed browser path, not the example if absent. The harness
uses real browser events/HTTP and the verified native package: fresh HOME/XDG,
external cwd/spaces, import/edit/save/validate/package, both game contracts,
exact artifact/snapshot identity and immutable bytes, preview execution/quit,
supervisor timeout, TERM/INT/HUP, active-preview reopen, crash/session cleanup,
read-only/symlink failures, and failing Cargo/rustc guards. Screenshots, versions
and private logs are retained. Timeout must match the supervisor exit/message,
not an unrelated preview error. Failed paths receive cleanup and failed status.
Success is `automated_passed_manual_pending`, NEVER full R1 acceptance.

## Step 5 — human desktop/Windows observations

Copy verification/r0-r1-manual.template.json to the private evidence folder.
Keep unobserved fields `not_checked`/empty. An authorized observer must record
actual reviewer, timezone-aware observation time, version and external evidence
SHA for the exact package manifest digest:

| Case | Required observations |
|---|---|
| chromium-x11 | Above workflow in actual X11; browser opening, keyboard focus/output, scaling/high-DPI and close/reopen |
| chromium-wayland | Same on actual Wayland; record native browser mode and compositor/version |
| firefox-x11 | Actual Firefox/X11 form, Unicode/output, package/preview and keyboard/accessibility checks |
| firefox-wayland | Actual Firefox/Wayland equivalent, not a renamed headless Chromium screenshot |
| manual-no-browser | No display variables/xdg-open; printed loopback URL works, no hang or orphan, terminal lifecycle correct |
| windows-credentials-and-discovery | Current native CLI/Studio/preview smoke plus existing Console/credential/discovery workflow; no secret leakage, wrong checkout/binary, or unreviewed ACL/key authority |

Record browser unavailable/unsupported as an OPEN issue; the checker does not
turn availability gaps into fake passes. Authenticate reviewer authority and
records outside this structural checker. Do not fill template fields from AI
inference or use earlier unrelated screenshots. Manual gates remain pending here.

## Step 6 — paired same-tuple validation

Bring both authentic R0 reports AND their hash-matched phase logs to a private
review folder. Preserve the report bytes used by the package. Then run:

```bash
python scripts/r0-r1-close.py \
  --ubuntu "$HOME/Worldwright Evidence/r0-ubuntu/r0-report.json" \
  --windows "$HOME/Worldwright Evidence/r0-windows/r0-report.json" \
  --desktop "$HOME/Worldwright Evidence/r1-installed/r1-report.json" \
  --manual "$HOME/Worldwright Evidence/manual.json"
```

This checks complete required phases/log digests, source inventory hash and lock,
native pin/host/binary identities, paired source tuple, package's original R0
report/binaries, all automated cases and nonempty unique manual records/digests.
Exit 1 means NOT CLOSED. Exit 0 means evidence structure/integrity is consistent;
an authorized reviewer must still authenticate manual evidence and adopt closure.
No production, license, support-policy or R2+ readiness follows from R0/R1 alone.

## CI and explicit boundaries

r0-r1.yml supplies Ubuntu/disposable PostgreSQL/native/Helm/private-package/real
headless Chromium automation and restricted log/screenshot retention. It does
not fabricate manual acceptance or publish packages. r0-windows.yml is manually
dispatched on the default branch using the protected worldwright-r0-windows
environment; configure required reviewers/branch restrictions BEFORE provisioning
its acceptance-only WORLDWRIGHT_R0_TEST_DATABASE_URL secret. That environment is
not adopted by merely creating YAML. Existing ci.yml Windows regressions remain.
Hosted execution, protected settings and engine access must be observed/adopted;
workflow presence is not a pass. Native Windows and actual Ubuntu desktop
execution remain outside this Linux file-operation sandbox.

## R5 session-sentinel follow-through

Updated Studio acquires cooperative workspace serve exclusion. After a real
crash/uncertain cleanup, `.worldwright-studio-session` can remain; launcher process
cleanup does not reclaim it. Follow [R5 recovery](R5_ACCEPTANCE.md), confirming all
writers stopped and preserving evidence before removing only that sentinel.
Normal graceful close/reopen must remove it without manual intervention.

The existing automated R1 crash case checks session cleanup and subsequently
launches a different fresh XDG workspace. It does not exercise recovery/reopen
of the crashed workspace. The new native child-kill test and separate manual
matrix in [R5_CHECK_PLAN.md](R5_CHECK_PLAN.md) K remain required. Do not count a
fresh-XDG pass as that proof or modify a retained old package to include new code.
