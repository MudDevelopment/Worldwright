# P0 implementation and acceptance

Implemented source scope: WWV2-001, WWV2-002, WWV2-003, WWV2-004, WWV2-008,
WWV2-009. Source fixes are installed in the repository; full acceptance is still
pending. See verification/p0-implementation-ledger.md for observed results.

## Default workspace policy (001)

`bash scripts/desktop.sh` now creates the missing managed default parent under
XDG_DATA_HOME or HOME/.local/share with umask 077. The managed parent must belong
to the current user and have no group/other permissions. Existing folders are
not silently chmodded. Symlink/dangling-link/non-directory/dot-dot components
are rejected before/after preparation; custom workspace parents must already
exist and be writable/searchable. Existing workspace ownership is checked.

This is a security tightening of filesystem policy, not a JSON format change.
An existing non-private managed folder fails with a specific message: inspect
it and deliberately chmod 700 it, or supply an existing trusted real custom
path. Do not use sudo. These shell checks assume trusted user-controlled
HOME/XDG ancestry and do not eliminate hostile concurrent path races or create
an OS sandbox. Shared preview-slot hardening remains WWV2-018.

## Native baseline diagnostics (002)

`doctor` prints compiler/Cargo paths, native host, sysroot, installed targets and
presence of ancestor/user Cargo configs. Override values are REDACTED, not
printed. Review those local configs/flags yourself if an override is present;
the script does not clear arbitrary flags or rewrite persistent user settings.

New optional task `check-native` requires the rustc/Cargo versions in the
repository pin. It chooses the host from rustc, creates a fresh temporary build
directory, and passes --target/--target-dir explicitly to compiler phases.
Formatting, check, Clippy, tests, build and rustdoc must all pass. Stop at the
first error; failed output directories are retained for diagnosis. No target
folder or lockfile is deleted. The ordinary check task remains available.

### Windows

From the repository root using the approved pinned toolchain:

```powershell
rustup run 1.85.1 cargo fmt --all
./scripts/dev.ps1 doctor
./scripts/dev.ps1 check-native
node scripts/test-expedition-browser.cjs
```

If ambient RUSTUP_TOOLCHAIN selects another installation, select the approved
pin explicitly; review reported RUSTC/wrapper/flag overrides rather than
assuming toolchain selection overrides every Cargo setting. The current patch
has not been run under PowerShell here.

### Ubuntu

```bash
bash scripts/dev.sh bootstrap
cargo fmt --all
bash scripts/dev.sh doctor
bash scripts/dev.sh check-native
node scripts/test-expedition-browser.cjs
python3 scripts/test-desktop.py -v
```

The last command uses synthetic compiler/application fixtures and explicitly
skips real-binary tests. It does not demonstrate Rust compilation or actual GUI
operation. To run real-binary first-run tests, build the native binaries and
pass them explicitly:

```bash
host=$(rustc -vV | sed -n 's/^host: //p')
cargo build --workspace --locked --offline --target "$host" --target-dir "$PWD/target"
python3 scripts/test-desktop.py --real-studio "$PWD/target/$host/debug/worldwright-studio" --real-preview "$PWD/target/$host/debug/worldwright" -v
```

This runs both fresh default and XDG Studio startup, package selection,
unsupported-game denial without artifact writes, reopen and SIGTERM child
cleanup. Physical Wayland/X11 browser/accessibility acceptance remains manual.

## Selected-game preview and receipt identity (003/004)

New Rust cases exercise the PUBLIC preview supervisor, not an untrusted direct
worker invocation. They package reference/expedition artifacts, run their
selected rules, check immutability and reject unsupported game packaging.
Studio HTTP cases verify omitted/reference/expedition selection and explicit
invalid-game errors. The generated executable/multiplayer regression tests
from EXPEDITION_PLAYABLE.md are retained.

The Node fixture compares IDs with matching sequence numbers across page
reload/new-tab contexts; a different counter alone cannot falsely pass it.
Same-page reconnect preserves uniqueness, stale socket events/timers cannot
change the replacement, and replay/conflict messages cannot manufacture retries.
This is a JavaScript adapter fixture, not actual browser/PostgreSQL proof.

A focused PostgreSQL test, gameplay_browser_request_id_lifecycle, applies the
real migration/release/writer/session workflows and then tests original receipt
replay after a new connection/session, changed-intent conflicts, fresh page/tab
Look IDs, disjoint actors, and receipt/audit cardinality. It is ignored in
ordinary local runs unless explicitly requested. It does not replace the full
WWV2-006 durability/identity/failure suite.

Use only an isolated disposable database whose parsed name contains
worldwright_test; supply WORLDWRIGHT_TEST_DATABASE_URL via an approved local
secret mechanism. The harness creates/drops test schemas.

```bash
cargo test -p worldwright-persistence --locked --offline gameplay_browser_request_id_lifecycle -- --ignored --list
cargo test -p worldwright-persistence --locked --offline gameplay_browser_request_id_lifecycle -- --ignored --test-threads=1
```

Confirm the named test actually appears and runs. Zero tests is not acceptance.

## Required Helm/schema matrix (008/009)

The OIDC schema is now conditional on an EXPLICIT enabled=true. Absent/disabled
configuration accepts unused empty placeholders; enabled requires complete,
canonical HTTPS/secret-reference settings. Unknown keys/non-boolean flags still
fail. Mandatory image, origin, ingress and PostgreSQL settings are not relaxed:
the unconfigured default deployment is intentionally invalid.

The chart wrappers now call one Python 3.10+ harness. Install test-only
requirements in a venv; these are not application runtime dependencies:

```bash
python3 -m venv .venv-chart
source .venv-chart/bin/activate
python -m pip install -r deploy/helm/worldwright/tests/requirements.txt
python -m unittest discover -s deploy/helm/worldwright/tests -p test_verify.py -v
bash deploy/helm/worldwright/tests/render.sh
```

On Windows activate the corresponding Scripts/Activate.ps1 and call render.ps1.
Helm 3 MUST be available. Missing Helm/Python/dependencies or failed positive
native commands fail the gate. The wrappers never turn absence into a pass.
The CI tooling pin is Helm v3.19.0, not a latest-version or production security
approval. Changing tool pins requires independent maintenance review.

- Four valid lint/render profiles: reference/expedition × OIDC disabled/enabled.
- Sixteen actual Helm negative cases: required settings, invalid digests,
  origin/callback mismatch, ingress/game/secret and partial OIDC failures.
- Actual YAML resource checks cover single-writer strategy, pod/container
  security, probes, config game profile and Ingress spec.tls.secretName.
- Database/OIDC credentials must be secretKeyRef references; plaintext values,
  rendered Secret resources and public operator flags are forbidden.
- Twenty-seven local schema cases and five checker unit tests are separately
  labeled. --schema-only explicitly does NOT constitute Helm acceptance.

The complete synthetic inputs are tests/acceptance-values.yaml. Neither those
inputs nor successful rendering imply a deployed cluster, real TLS certificate,
provider login or live database authorization.

## CI and release status

.github/workflows/ci.yml now contains native Ubuntu/Windows fresh checks,
required Node fixture checks, actual Ubuntu Studio smoke using built binaries,
independent Ubuntu/Windows Helm gates, and a disposable PostgreSQL receipt job.
The receipt job verifies the test name before running so a stale filter cannot
silently pass zero tests. The Helm install action commit was verified against
its official tag/action.yml; third-party action/security review is still policy.

These jobs are CONFIGURED, not observed passing. Nothing was pushed/committed,
no hosted run was triggered, and branch protection settings were not changed.
Require the jobs externally before calling them protected release gates.
Earlier Windows passes predate this patch. Run cargo fmt, current compile/lint/
tests/build/rustdoc, real startup, actual Helm and PostgreSQL checks before
closing all six P0 acceptance tracks.
