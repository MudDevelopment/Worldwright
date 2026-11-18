# Python security update and comprehensive test survey

## Status and scope

**The candidate installation succeeded on Windows. The first comprehensive run failed, and its evidence has been reviewed.** Preparation: `.worldwright-local/evidence/python-security-prepare-88b14fe025674421a759c4775cd19539/`. Test survey: `.worldwright-local/evidence/python-security-tests-608e98952e564273aae955cfb65f805b/`.

The survey verified the exact installed profile and pip consistency, passed all eight original security regressions, native collection/browser workflows, retained-subject integrity (including after workspace tests), and workspace doctests. Source was unchanged during that run. PyPI reported no active advisories for its 20 installed distributions at query time. The cryptography backend was OpenSSL 4.0.3; Python stdlib SSL used OpenSSL 3.5.9. This is not proof of vulnerability freedom or GitHub alert closure.

Four Python modules had actual failures/errors, two additional modules had only POSIX skips, and the workspace release target had one real CLI failure. Four whole-script prerequisites were blocked and 35 Rust tests were ignored. Subsequent fixes below are installed but **not yet executed**; the old reports remain unchanged.

The change addresses the 21 Python alerts in the retained Dependabot export. Current stable PyPI metadata was checked for **PyJWT 2.15.1** and **cryptography 50.0.2**. These are exact selected releases covering that advisory set, not a guarantee against unknown/future vulnerabilities. Unrelated package versions are not blindly floated to latest.

| Change | Reason |
|---|---|
| R2 PyJWT 2.10.1 → **2.15.1** | Covers all 16 exported PyJWT alerts, including the critical issue and fixes first released in 2.15.0 |
| R7 cryptography 46.0.6 → **50.0.2** | Covers the five exported cryptography alert boundaries and matches R2's existing 50.0.2 pin |
| R2 psycopg[binary] 3.2.3 → **3.3.9** | Reconciles the R2/review profiles and supports the current Python 3.14 environment; not claimed as one of the 21 alert fixes |
| Helm-test PyYAML 6.0.2 → **6.0.3** | Binary wheel installed successfully on Python 3.14; six Helm checker unit tests passed. This is not actual Helm render/deployment acceptance |
| Exact guard derives from requirements | `python_dependencies.py` replaces the collector's duplicated cryptography46.0.6 expectation, detects conflicting pins and retains security floors |

Existing Playwright1.56.0/Chromium cache, psutil7.2.2, websockets15.0.1 and jsonschema4.23.0 pins are retained. The fresh package-metadata survey reports any additional published advisories rather than assuming unaffected packages are safe. Optional SQL/parser-review requirements are not silently installed or run on an unsupported host. No Rust dependency, compiler pin, migration, JWT acceptance policy, signature domain or approval requirement is changed.

## First-run diagnosis and installed corrections

| Failure | Source-grounded correction / authority |
|---|---|
| R2 provider: `Missing Authority Key Identifier` | Fix generated CA/leaf certificates with SKI/AKI, critical constraints/key usage, and leaf serverAuth EKU. Preserve TLS verification; tests now explicitly enable strict X.509 validation and check wrong-hostname/untrusted-root refusal |
| R2 child log hash mismatch | Fixture writes explicit LF bytes rather than Windows-translated text. Raw-byte verifier remains authoritative; added CRLF-tamper rejection regression |
| R0/R1 synthetic package mode mismatch | Fixture inventory records the actual host's stat mode. Production mode comparison and Linux-specific mode tests remain unchanged; Windows stat is not ACL proof |
| R0 missing-tools and R4 failed-collection tests | Update the two collector entry points to shared repository-local environment/output guards. Superseded external-output rules must not force generated data outside the checkout. Actual tool/live acceptance remains separately gated |
| Rust public preview selected-game test | Fix `parse_package`: `--game` was advancing the option index three positions rather than two, skipping the following option. Existing integration test stays unchanged; new parser test checks both games at all five option positions and negative values/duplicates |
| Workspace report omitted failed-target summary | Retain both `ok` and `FAILED` libtest summary lines; the nonzero process exit already prevented a false pass. Added a summary-parser regression, bringing the security module to nine tests |

No cached binaries/environments were deleted, no TLS validation was disabled, and no migration or dependency/compiler pin was changed in this correction. Host test/format/lint results for these corrections remain pending. R3 skip-only results remain incomplete, not failures or passes for the skipped cases. Older collectors/launchers beyond the two named entry points still require their own storage/tool review before live use.

## One-command Windows installation and test attempt

Close development Studio and stop other builds. Run from an ordinary PowerShell window, with approved package-index access. Paste the whole command:

```powershell
& 'E:\Hold\Worldwright\.worldwright-local\python\Worldwright-R6-Python\Scripts\python.exe' -B `
    'E:\Hold\Worldwright\scripts\python-security.py' all
```

The old interpreter is only the bootstrap driver. It creates a **separate** environment at:

`E:\Hold\Worldwright\.worldwright-local\python\Worldwright-Python-Security`

It does not upgrade or delete `Worldwright-R6-Python`. The installed base Python is read/executed from its existing system location; generated venv/temp/cache/evidence data stays inside the repository. `pip` uses exact direct requirements and `--only-binary=:all:`. It does not weaken TLS validation, download Rust, run cargo clean, create a DB, or automatically provision a new browser. The previously verified Playwright cache is reused.

The runner configures repository-local storage itself; a fresh PowerShell window is supported. Approved index settings can be supplied through the existing pip environment variables. Pip configuration files are disabled to prevent an inherited install target outside the repository. Do not paste credentials into chat or command arguments.

If the default candidate directory already exists, its versions and dependency consistency are checked, not blindly overwritten. A partial or different environment fails with evidence. Use a new direct child of `.worldwright-local/python` via `--candidate` if a separate retry is needed; do not manually merge environments.

### Separate stages / retry without reinstalling

```powershell
# Installation only, using the existing driver:
& 'E:\Hold\Worldwright\.worldwright-local\python\Worldwright-R6-Python\Scripts\python.exe' -B 'E:\Hold\Worldwright\scripts\python-security.py' prepare

# Test retry, using the candidate itself:
& 'E:\Hold\Worldwright\.worldwright-local\python\Worldwright-Python-Security\Scripts\python.exe' -B 'E:\Hold\Worldwright\scripts\python-security.py' run
```

`--without-native` is a deliberately incomplete Python-only diagnostic, not a full run. Current `r6-check.ps1` defaults to the candidate interpreter. Older source versions and retained reports remain associated with their original environment.

## What actually executes

1. Exact profile merge/conflict check, candidate creation, pinned wheel installation, `pip check`, and an existing Chromium-cache check.
2. Candidate version verification, `pip freeze --all`, recorded cryptography backend and separate Python stdlib OpenSSL versions.
3. Git-visible Python-source inventory and AST parsing, with a stale-vulnerable-pin guard. This is syntax/source checking, not an unconfigured Ruff/mypy pass.
4. Discovery of all Git-visible `test-*.py` / `test_*.py` entry points. Currently that is 21 script tests plus one Helm test module after the new security tests were added.
5. Each supported unittest module runs in its own process through `run-python-unit.py`: actual discovered test IDs, executed count, failures/errors, skips, expected failures and unexpected successes are retained. No zero-test or import failure is called a pass. Existing platform-specific skips are not removed.
6. Both known standalone Python fixtures run with their actual invocation and required terminal success marker: P2 mutation checks and the five-group Chromium/DOM fixture. An unfamiliar new non-unittest entry point is explicitly blocked pending invocation review.
7. Explicit draft creation and expected unapproved-draft NO-GO, with the negative exit code interpreted deliberately rather than ignored.
8. Fresh PyPI version metadata for the installed public profile/transitives. Active advisories are reported as failures; unavailable metadata is unverified/incomplete. Unknown extra distribution names are not sent to a public index without review. This is a published-advisory snapshot, not proof against unknown vulnerabilities or a complete artifact SBOM scan.
9. The real `r6-native.py` collection and `r6-evidence.py --check-local` under the candidate interpreter: retained executable subjects, actual browser workflows, nine selected native suites, Clippy and Node fixtures.
10. Additional `cargo test --workspace --all-targets --all-features` and workspace doctests, locked/offline, with `--no-fail-fast`, native host and repository-local target cache. Failed and ignored tests stay visible. Overlapping Rust invocations are not summed as unique test coverage.
11. Before/after source and requirement identity comparison. The runner does not format or repair source while collecting evidence.

Independent test modules continue after ordinary test failures so one run can expose multiple issues. Prerequisite/inventory errors, interruptions, or unsafe setup may stop the survey; partial reports are retained. The source is not altered to make assertions pass.

## The expanded Python coverage

The survey includes the existing R2 PyJWT vector tests and actual loopback HTTPS provider tests, not only the successful R7/Ed25519 tests from the earlier native collector. The broader review also confirms that `test-r2.py` and `test-r2-live.py` decode synthetic JWTs with trusted keys and an explicit RS256-only policy. This is consistent with the vulnerability report's limited assessment of the encode-only provider implementation; it is not a mixed-family production verifier.

The original eight security regressions passed and cover installed exact versions, conflicting/unsupported pins, Psycopg's binary extra, guard mismatch refusal, RS256/JWK roundtrip compatibility, PyJWK algorithm-policy rejection, refusal to use asymmetric PEM as an HMAC secret, and Ed25519 signature/tamper behavior. A ninth, not-yet-run regression now verifies successful and failed Rust summary accounting. They use synthetic keys and data. They are not exploit reproductions for every advisory.

Other discovered helper coverage includes R0/R1, P2 contracts/negatives, R3 plan/schema/signature/IO tests, R4 monitor/contract tests, R5 evidence and DOM tests, R6 integrity/retained-subject tests, native path tests, R7 signed-decision tests, follow-through tests, storage tests and Helm checker units. The test report, not a guessed aggregate count, records the actual executed cases.

## Explicit full-suite blockers

| Entry / coverage | Why it is not silently run or passed on this host |
|---|---|
| `test-desktop.py` | Requires non-root Linux, process groups, POSIX mode/symlink semantics and launcher acceptance |
| `test-r2-expiry-sql.py` | Requires explicitly authorized disposable PostgreSQL; also contains an old eight-migration assertion while the repository has nine, and older external-report-path policy |
| `test-r3-role-fixture.py` | Requires permission to create/drop test-owned databases and roles in an explicitly owned disposable cluster |
| `test-r6-sql-rehearsal.py` | Requires a supported pgserver host, optional review dependencies and output-path review; a proposed-baseline rehearsal is not actual migration/restore acceptance |
| Existing POSIX-only unittest cases | Their skips are recorded as unverified coverage, not converted to passes |
| Ignored Rust SQLx/integration tests | Counted as ignored; no `--ignored` run against guessed credentials or a production DB |
| Actual Helm render/cluster/provider/load/manual acceptance | A checker unit test or ordinary native run is not deployment/acceptance evidence |

These are not dependency-upgrade successes or justification to change migrations/permissions. The runner clears inherited Worldwright/database selection variables before tests so it cannot accidentally use a previously configured deployment. Live tests require their own approved configuration and follow-up execution. Historical CI external-path mismatches are also not silently waived or fixed by this package update.

## Result interpretation

The command prints separate preparation and test evidence directories beneath `.worldwright-local/evidence`.

- **Exit 0 / `all-requested-tests-passed`:** all enumerated requested coverage actually passed, without blocked/skipped/ignored cases. This is not expected from the current Windows-only survey while the above prerequisites remain absent.
- **Exit 1 / `failed`:** an executed check, test, source identity, dependency check or known-advisory check failed. Inspect the retained report/logs; no automatic reinstall/cache reset.
- **Exit 2 / `available-checks-passed-full-suite-incomplete`:** executed checks passed, but blocked/skipped/ignored/unverified coverage remains. This is explicitly **not** a full-suite pass and must not close a release gate.

`r6-report.json` is the existing Collector's filename; the new report kind identifies the Python-security preparation or test survey. It contains test rows, per-unit JSON references/results, phase logs, pinned/resolved versions, OpenSSL identity, advisory responses, blockers and source identity. The dependency resolver records actual transitive versions; this is not yet a hash-locked, hermetic Python distribution.

Return the final status and evidence path for MCP review. Do not rerun the old setup instructions or submit private keys/tokens. A fresh Dependabot export after reviewed changes are committed/pushed is still needed before claiming the 21 alerts closed.
