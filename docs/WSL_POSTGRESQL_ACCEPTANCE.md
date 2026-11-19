# WSL2 / PostgreSQL acceptance continuation

## Selected scope

The operator selected **no existing databases (fresh tests only)** and confirmed established independent reviewers. Initial Ubuntu24.04 WSL2 inspection found failed private modes on DrvFS and a C:-backed Linux disk. The operator then approved a **separate repository-backed Ubuntu24.04 acceptance distro**, leaving existing distros unchanged. This enables preparation of a pristine corrected migration baseline; it does not authorize alteration of any subsequently discovered existing database history, production deployment, or fabricated acceptance signatures.

All generated project files, venvs, caches, database data/WAL/socket/log files, workspaces and evidence must remain backed by storage under `E:\Hold\Worldwright`. Existing system tools may be read/executed. Cooperative environment settings are not a filesystem sandbox and cannot promise that WSL/Windows system services never write OS-managed metadata.

## Current action — finish runner cleanup verification

Run `linux-next-10a5464cf79748ef8fa994f8c7bf39ca` passed **all34 native PostgreSQL tests**, **9 supplementary SQL tests** and **13 role tests**, after successful Linux build/check/Clippy and ordinary tests. The previously installed Rust/test corrections are now exercised successfully. The worker's final database inventory query then failed because a literal LIKE wildcard `%` appeared in the SQL text alongside Psycopg `%s` bindings. The cluster shut down successfully; no database reset is needed.

The query now binds both the LIKE pattern and the excluded database name as parameters. The existing six-test helper gate includes an AST-based regression checking parameterized literal queries for unescaped percent characters and the exact corrected cleanup query. No Rust, migrations, cleanup acceptance thresholds, schemas or caches changed in this repair. The old test passes remain evidence, but overall stage success cannot be declared until cleanup and final reporting complete. Rerun the cached-tools command below; the harness-only correction awaits execution.

## Prior PostgreSQL failure remediation (now exercised by the34-test pass)

The scoped trust repair `wsl-trust-repair-92903598a2be4655aff514f481afc99f` succeeded: RSTA-CA and its verified self-signed drsmaster-ca issuer were installed only in the acceptance distro, and all seven Linux HTTPS checks passed. Do not repeat that repair or provisioning.

Run `linux-next-6151b52e7ac94161a99671116edd06cf` then passed Linux formatting/check/scoped Clippy/build, regular workspace tests and doctests, all selected Python helpers, **12 real-binary launcher tests**, **9 supplementary SQL tests** and **13 role tests**. All nine migrations applied. The 34 native persistence tests ran with **17 passed / 17 failed**. The runner subsequently found leftover test schemas and stopped the owned cluster successfully; the cleanup error is not the primary native-test failure. The old cluster and all evidence are preserved.

Corrections installed after the first run and now exercised by the latest34-test pass:

- Fourteen activation failures shared a statement-order bug: maintenance became true before pending_activation_id existed. The implementation now sets both together after inserting the activation record, within the existing transaction/world lock. No constraint/migration change. The existing test adds injected-update-failure rollback assertions.
- Epoch zero is a valid seeded/restored no-writer state. Fence paths now classify mismatched old leases as StaleWriter while continuing to reject negative epochs as invalid data. Existing writer/restore assertions are preserved/strengthened.
- Reward intent excludes generated candidate item and operation IDs. The fixture now proves replay with changed generated IDs and tests a changed generation as the actual conflict. Legacy production submission is asserted to fail at submission, matching the existing gate.
- The previously blocked M6 fixture now proves a different admin cannot directly abort another executor's plan; it uses authorized recovery/takeover first. No authorization rule is relaxed.
- Failure lists, native result counts/failed cases and exact cleanup leftovers are retained together. Cleanup still fails for residual objects; no automatic schema deletion hides a test failure.

Run the existing stage with offline cache validation and **no reinstall, browser download, Cargo fetch or apt phase**:

```powershell
& 'E:\Hold\Worldwright\.worldwright-local\python\Worldwright-Python-Security\Scripts\python.exe' -B `
    'E:\Hold\Worldwright\scripts\linux-next-stage.py' --fresh-databases-only --reuse-tools
```

`--reuse-tools` validates installed Rust/Node hashes and versions, Python pins/pip-check, an actual cached Chromium launch and locked offline Cargo metadata. It then performs the normal incremental rebuild/test stages using a new private disposable cluster. It never restarts or cleans the old failed cluster. The native test inventory remains34. Cached-tool verification and corrected native tests have passed; the latest harness-only cleanup-query correction still awaits execution and the full stage remains incomplete.

The cryptography serial-number deprecation warning came from Windows certificate enumeration and did not stop the successful trust repair; it is not this PostgreSQL failure. No blanket warning/TLS suppression was added.

## Resolved prior blocker — Linux HTTPS issuer trust

Run `linux-next-c7a67f2f54d1484789ae5422f8eb3235` passed all six helper tests, verified guest ownership, **applied the fresh-baseline correction** and installed the verified ext4 source snapshot. Corrected operational migration SHA384 is `0c654cf48f5f48e8f74cddc01bc551ce7c02fed9bbb2bb7ddea32bffdb89bc2fefb89ece503d4db2ab81c4d343e4dc69`; all other migration hashes are unchanged. No SQLx DB history was touched. Tool setup stopped at the first Rust checksum HTTPS request: `CERTIFICATE_VERIFY_FAILED: unable to get local issuer certificate`. No tool installer/build/DB phase ran; the exported guest worker lock is absent.

The read-only diagnostic was executed as `wsl-tls-1095c66fe87d4825817b6c28b19121f8`: all seven Windows connections passed, while Linux failed the five enterprise-inspected hosts and passed both PyPI hosts. RSTA-CA is absent from Linux. Its DER SHA256 is `93d1dbce2c583b226bbd80c6d4480ce99bf8c9de6f9bca25f2929ecc38f05741`; it is a Windows trusted-store anchor issued by drsmaster-ca, not self-signed. The operator explicitly approved a complete-chain repair in the acceptance distro only and resumption after HTTPS verification.

**Prior repair-and-resume command (already completed; do not rerun for the PostgreSQL failures):**

```powershell
& 'E:\Hold\Worldwright\.worldwright-local\python\Worldwright-Python-Security\Scripts\python.exe' -B `
    'E:\Hold\Worldwright\scripts\repair-wsl-trust.py' --approve-rsta-chain --resume
```

It first runs six synthetic in-memory CA-policy tests. It re-reads existing Windows ROOT/CA public certificates, requires the exact RSTA fingerprint to remain server-auth trusted, and resolves a unique signature-checked issuer path ending at a valid self-signed root already trusted by Windows. Missing, ambiguous, expired, non-CA or invalid-signature paths stop without installing anything. Certificates are not downloaded. It verifies the affected hostnames from Windows against that complete chain with partial-chain behavior disabled, then rechecks owned-distro registration, nonce, root-disk backing and absence of an active worker lock.

The privileged step is confined to `Worldwright-Acceptance-24.04`: hash-checked public certificates are staged under its root-owned `/var/lib/worldwright-provision`, validated with OpenSSL, and added under `/usr/local/share/ca-certificates/worldwright-enterprise/`. `update-ca-certificates` updates that distro's system trust. `/etc/worldwright-enterprise/chain.pem` and its root-owned receipt record exact fingerprints and bundle hash. This system trust applies to clients using the distro's system CA store, not just the seven diagnostic hosts. Existing differing trust files are refused rather than overwritten. Windows trust, other distros, WSL global settings and production application deployments are unchanged.

All seven Linux HTTPS probes must then pass, with hostname checks and no partial-chain fallback, before `--resume` invokes the existing `linux-next-stage.py --fresh-databases-only` command. Repair evidence goes under `.worldwright-local/evidence/wsl-trust-repair-<id>/`; the resumed stage creates its usual separate `linux-next-<id>/` evidence. If repair fails after installation starts, preserve the report: some CA files may already be installed, and no automatic rollback or source/distro reset occurs. Equal existing certificate files are reusable on an inspected retry.

The worker validates the root-owned receipt and bundle before setting process-scoped `NODE_EXTRA_CA_CERTS=/etc/worldwright-enterprise/chain.pem`; this extends Node's built-in public roots, not disables validation. After Node installation, a seven-host Node TLS test must pass before Python/browser setup continues. Playwright's root-only dependency-install subprocess gets the same explicit extra CA path. No insecure TLS environment variables, blanket Windows trust-store copies, private-key exports or fake acceptance signatures are used.

The repair and its six policy regressions passed in the retained run above. For later read-only diagnosis, the original command remains:

```powershell
& 'E:\Hold\Worldwright\.worldwright-local\python\Worldwright-Python-Security\Scripts\python.exe' -B `
    'E:\Hold\Worldwright\scripts\wsl-tls-check.py'
```

This tests the reviewed Rust/Node/PyPI/crates/Playwright download hosts from Windows and the owned WSL distro with normal certificate and hostname verification. On Windows, it uses Python3.14's public verified-chain API and exports only public certificates obtained after successful verification. Candidate anchors must also be CA certificates already present in the Windows ROOT store with server-auth trust. Linux reports its unchanged default trust configuration and loaded CA fingerprints. Direct TLS probes do not use an HTTP proxy; they are diagnostics, not proof that every downloader/proxy route is identical. The cause may be enterprise inspection, chain configuration, or another trust difference; the existing error alone does not establish the issuer.

All reports/public PEM files stay in `.worldwright-local/evidence/wsl-tls-<id>/`. The tool never installs a root, disables TLS checks, copies an entire trust store, exports private keys or retrieves credentials. The diagnostic itself never authorizes an installation. Its actual run and the separately approved scoped repair are described above. Preserve the applied migration and provisioned distro; no reset, reprovision or checksum rollback is needed.

## Full stage — tools, exact source, fresh baseline, Linux build and PostgreSQL tests

Provisioning succeeded in `wsl-provision-4fd89e8c56bc47d8929ede905782c35c`. The signed image, E:-backed WSL2 registration, non-root ext4 permissions, OS packages and empty initial cluster inventory were verified. Do not rerun provisioning.

Run the installed next stage from Windows PowerShell:

```powershell
& 'E:\Hold\Worldwright\.worldwright-local\python\Worldwright-Python-Security\Scripts\python.exe' -B `
    'E:\Hold\Worldwright\scripts\linux-next-stage.py' --fresh-databases-only
```

This command performs real work rather than another inventory-only preflight:

1. Runs six local stage guards, verifies the owned distro/backing path and guest nonce, then applies the reviewed three-site migration correction **to the Windows source**. It preserves the old bytes in the run evidence and in `tests/fixtures/migration-history/20261106000100_operational_control.sql`; all other migration bytes remain unchanged. The switch explicitly selects the operator-authorized fresh-database-only path. No `_sqlx_migrations` checksum or existing database is edited. Unknown source hashes/inventories stop the operation.
2. Captures actual Git HEAD/history in a local bundle and copies tracked plus nonignored untracked source into `/home/worldwright/Worldwright` on ext4. Tar/manifest/member hashes are verified. HEAD stays the original commit; uncommitted changes remain dirty, not a manufactured clean release. R6-normalized text uses LF; `.sql` and `.log` bytes remain exact. Existing owned source is backed up on resync only after confirming no user edits/unrecorded files; caches/evidence remain intact. Do not edit either checkout during this run.
3. Installs **Rust1.85.1** (Cargo/rustfmt/Clippy included) and **Node24.22.3** beneath the Linux checkout's `.worldwright-local`. Node's exact archive digest is pinned from its official 2026-11-11 release page; Rust's versioned archive is checked against its HTTPS-published SHA256. These checks are recorded tool-acquisition integrity, not a claim of independent detached-signature verification or advisory clearance. No system/default Rust upgrade occurs.
4. Creates a separate copied-binary Linux Python venv and installs the existing exact security/test profile. Records `pip check`, pin checks and full distribution inventory. Downloads its Playwright1.56 Chromium into the local browser cache. Uses Playwright's actual `install-deps chromium` command as root **only in the new repository-backed distro** for system libraries; build/test phases explicitly use non-root `worldwright`.
5. Fetches dependencies against the actual Cargo.lock for the Linux target, verifies the lock is unchanged, then checks/builds/tests offline. Runs formatting, cargo check, scoped Clippy, app builds, full-workspace tests, doctests, selected Linux/POSIX helpers and the real-binary desktop launcher tests. Tool/target caches are reused; no cargo clean or Windows venv rebuild.
6. Creates one new isolated PostgreSQL16 cluster in the run's private ext4 evidence directory, with a random SCRAM password, loopback-only listener and local socket directory. Verifies it is pristine; creates only its own `worldwright_test_stage` database. Applies all nine current migrations, discovers exactly34 ignored persistence tests and executes them serially, then runs9 supplementary expiry/COMMIT SQL tests and13 synthetic grant/role tests. Checks for leftover test schemas/databases/roles and stops the owned cluster in a finally block. Password/data/raw PostgreSQL logs stay in the private Linux evidence tree; never paste them into chat.
7. Retains source/tool/test identities and failures. The host exports stage JSON and redacted phase logs back under its new `.worldwright-local/evidence/linux-next-<id>/guest-evidence/` so the file connection can inspect results without direct access inside the VHD. Exported file hashes are recorded; database/password files and raw PG log are excluded.

The reusable bounded process wrapper limits each outer tools/checks phase to3600seconds and browser-library installation to1800seconds. During those phases PowerShell can be quiet while child output is captured. If it fails or times out, preserve the reported directory and locks: killing a Windows WSL client does not prove the Linux worker stopped. No blind lock clearing, reset or reimport is provided.

The first host attempt, `linux-next-9fde7e3d69654f4e8e2829472cc59d81`, stopped in the six-test helper gate (five errors, one pass), before guest operations or canonical migration changes. `fresh_baseline.py` incorrectly counted substring occurrences inside longer identifiers. It now matches only the three standalone identifiers with a byte-level word-boundary expression. SHA384 validation is unchanged and now has a separate diagnostic. The existing regression independently constructs the exact three-site patch and checks that authorization_version and constraint names are untouched. The corrected helper has not yet been host-executed; the same next-stage command reruns the gate. Successful exit means this stage passed, not full acceptance. The separately orchestrated35th ignored server/COMMIT case, optional literal-SQL R6 rehearsal, full native browser evidence collector, installed desktop/manual matrix, live staging/load/recovery and independently signed closures still require their own execution/evidence. The rehearsal now distinguishes the preserved historical failing baseline from corrected current migrations; its logic is not silently removed.

### Historical provisioning/download recovery


```powershell
& 'E:\Hold\Worldwright\.worldwright-local\python\Worldwright-Python-Security\Scripts\python.exe' -B `
    'E:\Hold\Worldwright\scripts\provision-acceptance-wsl.py' --confirm-create
```

The owned name is `Worldwright-Acceptance-24.04`; its backing/install directory is `.worldwright-local/wsl/Worldwright-Acceptance-24.04`. Acquisition files stay in `.worldwright-local/wsl/downloads`; each run records a new `.worldwright-local/evidence/wsl-provision-<id>/provision.json`. The script verifies the actual registered backing location before guest configuration and refuses unrelated existing names/locations. It never unregisters, terminates, relocates or changes the default of either existing distro.

The first actual run, `wsl-provision-075d084c423b4186ac70d7da3497617a`, passed eight helper tests and Canonical signature verification, then timed out downloading the rootfs. It retained a roughly340MiB partial file. There was **no import phase, no pending mutating phase, and no new distro directory**. Original reports, signatures, partial bytes and provision state are preserved.

### Recovery behavior now installed (host execution pending)

- Fourteen helper tests run before provisioning; six new tests exercise mocked response ranges, completion/corruption and timeout retention. Only their disposable fixtures use new repository-local temp directories; no test performs real downloads/imports/DB work.
- Cached public metadata is reused and its GPG signature checked again. GnuPG must validate the pinned Canonical primary fingerprint `D2EB44626FDDC30B513D5BB71A5D6C4C7DB87C81`; the image must match the exact SHA256 entry from that signed manifest. There is no checksum-only or TLS-verification bypass.
- Before another image request, candidate partials are hashed. A complete checksum-matching partial is copied and verified before promotion, without network access for the image. Otherwise the largest recognized partial is used as a prefix in a **new** continuation file; original partials are never appended to or deleted.
- Resume requires HTTP206 and an exact Content-Range start, consistent length, bounded total and identity encoding. Ignored/wrong ranges, short responses and final hash mismatches stop without importing. No automatic full redownload or retry loop is added. The read timeout is120seconds, with a30-minute transfer deadline. Known-length bodies stop at their declared end rather than making an unnecessary EOF read.
- Only signed-checksum-bound image transfers may resume. Public-key/manifest downloads retain their separate validation paths. Successful whole-image hash verification remains mandatory before decompression/import.

### What successful provisioning establishes

The official Ubuntu24.04 amd64 WSL image is imported with explicit location and `--version 2`. A non-root `worldwright` user and private `/home/worldwright/Worldwright` workspace live inside the repository-backed disk; rootfs/system tools there are separate from the Windows-mounted checkout. No password or sudo privilege is granted. Commands explicitly choose the user, without needing a distro restart.

The **new distro only** receives apt updates/upgrades and foundational C build tools, pkg-config/OpenSSL headers, Git/cURL/xz, Python/venv and PostgreSQL16 packages. Exact acquired package versions are recorded. Service startup and automatic PostgreSQL cluster creation are disabled before installation; `pg_lsclusters` must be empty. Setup verifies non-root private modes, denied writes and links on ext4. It does not yet populate a source snapshot, install pinned Rust/Node/browser tooling, create a DB, repair migrations, run the application suite, or establish independent acceptance.

Completed setup can be checked without reimporting/upgrading. Failed/interrupted **mutating** phases retain a pending marker and block blind retries; never clear the marker or lock without verifying actual WSL-side process/state cleanup. The reviewed download-only timeout has no such pending phase.

References: [official Ubuntu24.04 image directory](https://cloud-images.ubuntu.com/wsl/releases/24.04/current/), [Canonical verification and signing fingerprint](https://ubuntu.com/docs/public-images/public-images-how-to/verify-image-checksum/), [Microsoft explicit-location import](https://learn.microsoft.com/en-us/windows/wsl/use-custom-distro).

## Completed diagnostic — bounded host/storage preflight

Run in Windows PowerShell, using the already prepared Windows Python:

```powershell
& 'E:\Hold\Worldwright\.worldwright-local\python\Worldwright-Python-Security\Scripts\python.exe' -B `
    'E:\Hold\Worldwright\scripts\wsl-acceptance-preflight.py'
```

The script runs seven pure helper regressions before launching WSL. It enumerates actual installed distro names and selects the sole name containing Ubuntu; if ambiguous, it stops with the inventory retained. An exact actual name can be supplied with `--distro`. It does not assume that a distro's name establishes its OS version.

The Linux probe records `/etc/os-release`, WSL registration version/backing directory, kernel/architecture/UID, actual project mount, available disk capacity, and known tool locations/versions. It uses `wslpath` rather than guessing `/mnt/e`; binds the visible script bytes and new evidence directory across Windows/WSL using a SHA256/nonce; bypasses shell startup files; and uses a clean Linux child environment and Python `-I -B`. Known direct native Rust executables are inspected, not rustup shims that could download tools. It creates only a new local permission/link probe, retaining it for inspection. It never chmods/deletes an existing project path.

Evidence: `.worldwright-local/evidence/wsl-preflight-<run-id>/preflight.json` plus phase logs and binding/probe files. Exit 1 is a failed probe; exit 2 means observed blockers; exit 0 means the limited observations completed without those blockers, **not installation/test/acceptance readiness**. Missing tools, backing-store placement and later actual tests still require review. The operator executed this diagnostic in `wsl-preflight-0104407ad6b0429e969d25925acfb3af`: all phases passed, but real private-mode/DrvFS blockers were recorded. That is not Linux application acceptance.

### Filesystem boundary

Do not assume that `/mnt/e` has native POSIX behavior. The probe checks requested modes, actual non-root write denial, ownership, symlinks and hardlinks. DrvFS is flagged for explicit review even if a simple metadata probe passes. Full privacy/multi-user/process tests remain required.

Do not silently move the checkout into WSL's default home or use `/tmp`, default Docker volumes, or a default WSL disk for project data. If a native ext4 workspace is needed, establish a dedicated backing image beneath this repository and verify its mount/ownership before populating it. Do not relocate/unregister/terminate the existing WSL distro without separate approval. WSLg/headless success is not proof of the full independent X11/Wayland/Firefox/Chromium desktop matrix.

## Broader completion checklist

The installed next-stage runner above implements the initial fresh-baseline/build/persistence work. Until its results are reviewed, these execution obligations remain open:

1. Preserve exact old migration bytes and hashes. Prepare the existing reviewed three-site quoted `authorization` correction as a **fresh baseline**, with a new recorded migration digest. Apply no checksum edits to `_sqlx_migrations` and no upgrade to existing databases.
2. Update original-baseline proposal/rehearsal fixtures deliberately: historical syntax failure remains a historical negative fixture; corrected current source must actually apply. Do not simply delete that regression or claim an isolated proposed-SQL run as native acceptance.
3. Reconcile the expiry fixture's obsolete eight-file assertion against the reviewed complete nine-file inventory. Preserve fresh/upgrade/future/corrupt/failed-ledger coverage and immutable checks during execution.
4. Adapt remaining external-output/default-temp entry points to verified repository-local Linux storage. Provision a separate non-root disposable PostgreSQL cluster using approved installed tools or separately reviewed acquisition. Never connect to an existing service by guesswork.
5. Execute all 34 native persistence DB tests with exact discovery/count checks and schema cleanup. Execute the 35th ignored case via the real `r2-live.py` process harness, not a blanket server `--ignored` invocation.
6. Run supplementary expiry/COMMIT, role and current-baseline SQL rehearsal tests; retain their narrower scopes. Run Linux native build/test/docs plus launcher real-binary and POSIX-specific tests with matching evidence.
7. Retain successful and failed runs, cleanup results, actual DB/tool/source identities and secret-redacted logs. Baseline/source changes require fresh same-tuple evidence, including affected Windows coverage. Do not clean existing caches or recreate the accepted Windows venv.

No migrations, database data, WSL settings or toolchain pins have been changed by Step 1.

## Step 3 — independent acceptance handoff

Reviewer readiness was confirmed by the operator; no identities, trust-store bytes, public-key pin, signatures or reviewed decisions have yet been supplied to this continuation. Use the established process, not a newly invented signing authority.

- R1/R2/R5 closures: engineering; R3/R4: service; R6: security.
- Final decisions: project, engineering, security, service and legal, with distinct identities/keys separate from artifact builder.
- All six closures cover the 32 exact groups in `scripts/r7_core.py`; the local acceptance ledger records each group. Linux/PG tests alone do not supply staging, fixed beta/soak, restoration objectives, installed desktop accessibility or policy adoption.
- Supply the candidate/evidence locations and independently approved **public** trust-store path/hash through the approved review channel. Keep private signing keys out of chat and collector inputs. Preserve independent trust-store placement outside the candidate but within the required project storage boundary.
- Assemble unsigned requests only from genuine same-release evidence, obtain reviewer decisions independently, then run the existing signature/gate validators. No automated self-signing, waiver or GO flag is added.

Current status: preparation and source review, **not completed Linux/PG execution or independent acceptance**. The prior Windows available-checks pass remains historical evidence for its exact source tuple at `python-security-tests-edcc3ff6ad6f4e49b92a1a10a3ff8eb8`.
