# R3 — restricted staging deployment acceptance ledger

Authority: ROADMAP_v2.md, R3 items 1–7 and exit. R0 and R2 remain OPEN.
R2 currently fails fresh migration SQL at operational_control.sql:188; migration
source/checksums are immutable. This pass does not rewrite them or approve a repair.
No real staging installation, image digest, provider, base image, CNI, ingress,
service owner or deployment authority is inferred from synthetic examples.

| Unit | Status | Required evidence |
|---|---|---|
| R3.1 actual image | ⏳ not yet checked | Approved digest-pinned builder/runtime, same source, real build/run, registry manifest versus config identity, pinned native toolchain/provenance |
| R3.2 chart profiles | ⏳ not yet checked (full pinned/hosted gate) | Local supplemental Helm v3.10.1 verified all four profiles and 22 negatives; configured v3.19.0 Ubuntu/Windows CI is still unobserved. See current executed evidence below. |
| R3.3 ingress/startup/termination | ⏳ not yet checked | Actual HTTPS readiness plus active artifact/state reconstruction, one Recreate writer, native SIGTERM/drain/controlled exit and real restart |
| R3.4 DB identities | ⏳ not yet checked | Distinct migration/runtime/recovery logins and live grants/ownership; no exception implied by separate Secret names |
| R3.5 access/rotation/WebSocket | ⏳ not yet checked | Private DB access, pre-provisioned rotated runtime credential and restart, real wss upgrade/idle, public operator route denial |
| R3.6 policy/CNI | ⏳ not yet checked | Release-scoped selectors, DNS+HTTPS destinations, allowed and denied real probes with positive control; YAML alone cannot prove enforcement |
| R3.7 rollout/rollback | ⏳ not yet checked | Real failed rollout/restart, controlled same-schema rollback, independently reviewed content rollback/schema compatibility/Helm limits |
| R3.exit | ❌ gap found | Closure was attempted and correctly refused. Docker/kubectl, complete current R0/R2, actual staging/image proofs and authenticated service adoption are unavailable; known R2 migration failure remains. No full R3 closure. |

Chart values format 2 is a BREAKING security tightening: immutable digest only,
separate postgres.migrationSecret/migrationKey, required enabled NetworkPolicy,
explicit ingress/DNS peer selectors and scoped HTTPS egress CIDRs. An explicit
values migration must take operator-selected identities/peers; no shared secret or
unrestricted egress is silently inferred. Chart version becomes 0.2.0; application,
protocol, content and SQL migration identities do not change.

## Phase/documentation reconciliation batch — November 10, 2026

The closure inventory now requires all 13 staging phases, including
`rotated-pod-image` and `rollback-pod-image`. New helper regressions compare
AST-discovered phase emissions with that inventory, validate complete/missing/
tampered/duplicate records, and reject boolean/string/signal-killed or mismatched
expected outcomes. This is an internal evidence correction, not a relaxed gate.
No Pod evidence, native run or service adoption is invented by a checker fixture.

`deploy/README.md` now describes required pinned Docker inputs, values format 2,
explicit values migration and separate DB identities instead of the obsolete bare
Docker build/shared-secret/mutable-tag instructions. `docs/R3_ACCEPTANCE.md`
records all 13 phases, six scripted/four independent cases, private plan fields,
manual gate procedures, proof/signature bindings and failure recovery boundary.
README, DESIGN and SECURITY now distinguish current R3 source from historical
milestone claims and incomplete native/staging acceptance. DESIGN's document
version is 0.4; no application/SQL/protocol/content version is changed here.

This batch entry originally preceded execution; the final observed results below
supersede that intermediate state. All real image/cluster, native shutdown,
DB grant and owner-adoption gates still remain unverified.

## Final observed reconciliation checks — November 10, 2026

Executed from current selected EWW text readbacks using Python 3.12.3 and the
repository's pinned test dependencies (PyYAML 6.0.2, jsonschema 4.23.0 and current
scripts/r2-requirements.txt). This is not a complete checkout or native acceptance.
Counts describe distinct suites; repeated schema invocation is not additive.

| Available check | Status | Actual scope/result |
|---|---|---|
| Phase mismatch + current collector inventory | ✅ verified | All 13 AST-discovered phase emissions equal closure requirements; complete helper inventory passes and both missing/tampered replacement Pod logs fail. |
| R3 helper regression suite | ✅ verified | 12 tests passed, including duplicate/invalid phases and rejection of boolean/string/signal-killed or wrong expected exit codes. |
| Chart checker regression suite | ✅ verified | 6 tests passed with synthetic YAML/subprocess fixtures; not Helm or CNI by itself. |
| Schema/semantic profiles | ✅ verified | 35 cases passed; version-2 tightening/explicit migration refusals retained. |
| Actual supplemental Helm | ✅ verified | v3.10.1+g9f88ccb: all 4 game/OIDC lint+render profiles and 22 actual negative render cases passed. npm helm-binary-linux@3.10.1-1 is test-only provenance, not an adopted dependency/runtime image. |
| Missing Helm behavior | ✅ verified | Shared required verifier exited 1 with missing-tool diagnostic; no successful skip. |
| Adjacent R2 helpers/harness | ✅ verified | 12 + 8 tests passed under current dependency pins; no live approved-provider/Rust acceptance inferred. |
| Browser-resume adapter | ✅ verified | Existing Node fixture passed; not an actual deployed browser/ingress proof. |
| Python and Rust source checks | ✅ verified | 8 R3/chart Python files parsed; changed server cli.rs/lifecycle.rs parsed by Rust grammar. Not Cargo formatting/compilation/Clippy/tests. |
| Documentation + CI static reconciliation | ✅ verified | Documented phase list equals source; Bash snippets/local links and both workflow YAML/Bash parses checked; read-only permissions, retained Windows, v3.19.0 CI pin, reviewed default-branch staging and independent key path checked. No hosted execution/adoption claimed. |
| Supplemental PostgreSQL expiry/proxy suite | ❌ gap found | 9 tests ran: 8 passed, 1 error, 0 skipped. Unchanged complete migration inventory again fails at operational_control.sql:188 near authorization. Both owned schemas dropped, cleanup independently verified, test server stopped. Not live R3 DB grants/SQLx owner acceptance. |
| Cargo format/check/Clippy/test attempts | ⏳ not yet checked | cargo is absent. No native command executed/passed; EWW exposes filesystem operations, not remote Windows execution. |
| Actual image/staging collectors | ❌ gap found | Both attempted and failed prerequisites, with zero promoted phases/cases. Docker/buildx and kubectl plus approved actual inputs are unavailable. |
| Closure attempt | ❌ gap found | Exit 1 / R3 NOT CLOSED: actual image, staging, prerequisite and adoption proofs are unavailable. No synthetic passes inserted. |

The temporary aggregate-check script initially used the wrong field name for
SQL failure metadata. The actual retained SQL report was re-read, that scratch
aggregation was corrected to its real errors list, and the entire available-check
sequence reran successfully as a verification procedure while retaining the real
failed migration gate. Neither migrations nor the failing SQL test were changed.

### Retained artifacts and verification boundary

- `r3-reconciliation-observations.json`: actual counts, tool/dependency versions,
  selected LF-readback hashes, blocked checks, failed collector bodies and log hashes.
- `r3-reconciliation-r3-helpers.log`: captured 12-test R3 regression output.
- `r3-reconciliation-real-helm.log`: captured real supplemental Helm results.
- `r3-reconciliation-supplementary-sql.log`: captured unchanged migration failure
  and schema-created/dropped markers, not a passed database gate.
- `r3-reconciliation-closure-refusal.log`: captured explicit NOT CLOSED result.

These are local reconciliation observations, NOT signed staging/adoption reports.
No image digest, active real deployment, CNI denial, owner/provider approval,
controlled Worldwright termination, old-secret revocation or content rollback is
invented. Pinned v3.19.0 hosted execution, native Rust/paired-host runs, complete
current R0/R2, actual image/staging and all four independent gates remain open.
Documentation/runbooks/gates are now reconciled; **full R3 release acceptance is
not closed**. Rebuild and reverify the exact future source/lock/image tuple before
using actual infrastructure; historical/source-only checks are not a substitute.

## Additional locally executable sweep — November 10, 2026

New coverage units are enumerated before execution:

| Local unit | Status | Boundary |
|---|---|---|
| Malformed plan versions, reviewer metadata and collector exit outcomes | ✅ verified | New guard regressions passed after source corrections; exact integer/printable-text and collector/closure agreement checked, not deployment/adoption. |
| Regular/symlink/oversized report inputs and platform-manifest variants | ✅ verified | Linux local file/OCI-index/newline/hash checks passed; no pushed image or Windows ACL claim. |
| Role/grant SQL on an isolated PostgreSQL fixture | ✅ verified | 13 real PostgreSQL synthetic-fixture tests passed, including setup and SQL-grant-failure rollback. This is not the current eight-file app ledger/deployed role gate. |
| Negative audit behavior for missing runtime prerequisites | ✅ verified | All six previously missed permission categories are now refused in live fixtures; no actual staging Secret/password authentication is inferred. |
| Existing R3/chart and neighboring local regressions | ✅ verified | R3 12, new guards 9, chart checker 6, schema 35, actual supplemental Helm 4/22 and adjacent R2 12+8 reran; exact versions recorded. |

The real R3.4 deployed-role/current-migration gate and R3 exit remain open.
No test-owned fixture ledger or synthetic signing key can advance them.

### Newly exposed local gaps and source corrections

The first new guard run executed 8 tests and failed (7 failing subcases and one
missing new phase helper). It demonstrated boolean/float plan-version acceptance
and invalid reviewer metadata; source inspection also confirmed the collector
accepted any nonzero exit while closure required a positive nonzero exit.
The source now enforces exact integer version 1, printable nonblank bounded
reviewer text, and a directly tested collector `phase_outcome` matching closure.

The first real PostgreSQL fixture run executed 13 tests: 7 passed and 6 failed.
Six deliberately revoked permission categories were incorrectly accepted by the
old audit: schema USAGE, artifact SELECT, audit INSERT, world-column UPDATE,
invite UPDATE and owned-sequence USAGE. `r3-roles.py` now checks every prerequisite
in its existing setup recipe, not only the full-DML tables. The grant recipe and
application migration files were not changed to make these tests pass.

Baseline failure logs are retained locally for the final evidence archive.
The permission suite uses an actually executed one-file synthetic fixture under
an explicitly patched test ROOT, with test-owned DBs/roles. It cannot certify the
current eight-file app migration, live credential authentication or deployed
roles. Cluster-role fixture creation now requires explicit opt-in and an owned
isolated test cluster; no successful skip or automatic production provisioning.
Corrections are installed; after-fix execution and retained evidence are recorded below.

### Final observed additional-local results — November 10, 2026

Current source readbacks were rerun after the corrections and documentation/CI
updates. No broad exception or migration/test rewrite was used to hide failure.

- **9 new local guard methods passed** (Linux, 0 skipped): strict plan versions,
  bounded printable reviewer, collector/closure exit equivalence, report file/
  symlink/size/JSON refusals, OCI index/platform/newline checks, CLI help and role
  proof type/count/identity binding. The original before-fix run had 8 methods;
  a later explicit synthetic-role-proof rejection test was added.
- **13 real PostgreSQL permission-fixture methods passed** (0 errors/failures/
  skips). Missing schema USAGE, artifact SELECT, audit INSERT, world-column UPDATE,
  invite UPDATE and serial-sequence USAGE now fail audit. Role flags/inheritance,
  forbidden inherited password updates, ownership/checksum errors, real runtime
  insert/denied writes and atomic rollback on grant SQL error were exercised.
- **12 existing R3 + 6 chart-checker tests passed**: 40 R3-focused test methods
  across distinct scopes, not 40 deployment or native application checks.
- **35 schema cases**, **4 actual Helm game/OIDC profiles**, **22 actual Helm
  negative cases** passed using supplemental Helm v3.10.1. The configured
  v3.19.0 hosted baseline is still unobserved; no relabeling of versions.
- Adjacent R2 helper/harness suites (12 + 8) and Node browser-resume fixture passed.
  Ten current Python subjects parsed; server CLI/lifecycle Rust grammar, Bash/doc
  links/phase inventory and CI static checks passed. No Rust compilation/format/
  Clippy or Windows-native pass is claimed.
- The separate **unchanged full app migration gate still fails**: 9 SQL methods
  ran, 8 passed, 1 syntax error at operational_control.sql:188 near authorization.
  The synthetic permission fixture temporarily targets its own executed one-file
  migration ROOT; it is expressly not a fix/pass of that eight-file app gate.
- Independent cleanup queries found **zero remaining test-owned R3 databases or
  roles** and no R2 test schemas. The temporary test PostgreSQL server was stopped.
- Missing role-fixture opt-in fails before DB/role creation. Missing Helm still
  exits 1. Actual image/staging/closure attempts still fail prerequisites/NOT
  CLOSED with no promoted phases/cases. Cargo, Docker/kubectl, approved inputs,
  current R0/R2 and all real owner/CNI/termination/rotation/content gates remain open.

The live audit now checks its existing setup recipe fully. The collector also
records all three expected role identities/schema; closure requires a real
complete database-role-audit report and equality with those fields, not a passed
status alone. A synthetic fixture result is explicitly rejected. This is internal
validation/additive evidence metadata, not an application/chart/protocol/SQL
version change or grant-recipe expansion. Real image/proofs must be rebuilt against
this new current source tuple before acceptance.

New tests are reproducible through scripts/test-r3-local.py and the explicitly
opted-in scripts/test-r3-role-fixture.py. The R3 workflow includes local guards and
an isolated disposable-PG permission-fixture job; its definitions are not a hosted
pass or cluster adoption. README/DESIGN/SECURITY/R3 guide were updated in the same
pass, including fixture mutation boundaries and unchanged real-gate requirements.

#### Additional retained artifacts

- r3-additional-local-observations.json — counts, exact versions/dependency pins,
  selected readback/workflow hashes, scope distinctions, blockers and log hashes.
- r3-additional-local-before.log — actual 8-method failing baseline with malformed
  versions/reviewers and absent new phase helper; synthetic validation data only.
- r3-additional-roles-before.log — actual 13-method baseline, 7 passes/6 failures.
- r3-additional-local-after.log — actual final 9-method passing local guard output.
- r3-additional-roles-after.log — actual final 13-method passing live SQL fixture
  output with created/removed test-database markers; not a deployment role report.

All prior observations remain historical scoped evidence. **Full R3 is still
NOT CLOSED**; this batch completes the enumerated locally executable checks,
not the unavailable real/native/adoption acceptance units in the top matrix.

## Final local closeout/status review — November 10, 2026

The sandbox expired between passes. This pass restored only the current selected
source readbacks and pinned Python/Helm helper dependencies; it did not fabricate
or replace native Rust, approved images, a cluster, DB credentials or adoption.
No repository behavior, migration, test contract or grant recipe changed.

Freshly executed: **12 R3 + 9 local-guard + 6 chart-checker methods passed**,
**35 schema cases**, **4 actual Helm profiles** and **22 actual Helm negative
cases** passed. Helm was explicitly v3.10.1, not the unobserved v3.19.0 hosted pin.
The before/after logs (4), all recorded source/document subjects (15), workflow
hash and retained observations were re-read and matched current repository
readbacks. Seven core R3 Python files also parsed. No configured Python lint/type
gate or Rust-native pass is invented by these checks.

Five negative closeout checks were actually run and correctly exited 1:
missing Helm; missing permission-fixture opt-in; image prerequisites; staging
prerequisites; and closure without real approved evidence. Missing fixture opt-in
failed before DB/role creation, not as a successful skipped test. Image/staging
collectors recorded no promoted real phases/cases; closure said R3 NOT CLOSED.

The **13 passing real PostgreSQL synthetic permission-fixture methods are retained
from the preceding executed pass**, with unchanged tested source hashes. That
expired temporary cluster was not restored/re-run in this pass. Their scope remains
permission/setup-rollback logic on their executed test fixture, not current app
migrations/SQLx, password authentication, deployed roles or service adoption.
Its previously independently verified cleanup and server stop remain historical
execution evidence, not a new Worldwright termination proof.

### Final ledger disposition

All five enumerated additional-local coverage units are **verified within their
stated local scope**. There is no unchecked item left in that enumerated local
sweep; this does not claim exhaustive security testing or an accepted release.
The top release matrix still has **seven unverified acceptance units (R3.1–R3.7)**
and a **failed/not-closed exit gate**:

1. R3.1: actual approved pinned bases, build/push/manifest/runtime/provenance.
2. R3.2: configured v3.19.0 Ubuntu/Windows hosted chart/helper evidence.
3. R3.3: real TLS startup/state reconstruction, single writer, native SIGTERM/
   controlled exit and restart.
4. R3.4: actual complete app migration and distinct deployed DB identities/grants/
   ownership, not the synthetic permission fixture.
5. R3.5: real private DB access, credential rotation and old-credential revocation,
   wss upgrade/idle/origin checks and public operator denial.
6. R3.6: actual ingress/CNI allowed/denied probes with positive controls.
7. R3.7: actual failed rollout and controlled Helm/content/schema rollback review.
8. Exit: complete same-source R0/R2 prerequisites and independently trusted signed
   service adoption bound to all actual image/world/environment/evidence fields.

Current R0/R2 remain open. The unchanged operational migration's reserved
`authorization` syntax at line 188 is a known failure, not repaired or waived.
Cargo/rustc/rustfmt, Docker/kubectl and approved actual inputs are absent here.
The four independent real gates (CNI, controlled termination, old-credential
revocation, content/schema rollback review) are not replaced by runbooks or keys.
**Final R3 status: NOT CLOSED.** Further release progress needs the unresolved
prerequisites and authorized infrastructure/observations, not another synthetic
pass. Rebuild/reverify the exact current source/lock/image tuple before acceptance.

## R3 feature-completion batch — source gaps enumerated before changes

The current seven R3 tracks already have image/chart/signal/role/network/rotation/
rollout implementations. This pass found concrete gaps in those collectors, not
permission to waive real execution/independent review:

| Implementation unit | Status | Source evidence / intended completion |
|---|---|---|
| Bounded subprocess collection and interrupted-phase retention | ✅ verified (source/local execution) | Shared R3 I/O helper caps output, cancels/reaps processes, retains partial interrupted-phase logs/metadata and rejects interruption in collectors/closure. Actual local process/pipe/log cases passed; Windows native and actual Worldwright graceful termination remain unverified. |
| HTTPS deployment-target redirect isolation | ✅ verified (source/local transport execution) | Staging uses NoRedirects; a real loopback HTTP redirect with positive-control target was refused without contacting the target. Actual deployed TLS/ingress is still unverified. |
| Regressions, CI and feature/acceptance documentation | ✅ verified (local/source only) | 17 new I/O regressions plus R3 12, guards 9 and chart 6 passed; real supplemental Helm 4/22, schema 35 and CI wiring/AST checked. README/DESIGN/SECURITY/roadmap/guide updated; hosted/native/cluster/adoption not promoted. |

No SQL/schema/role grant change, dependency upgrade, generated approval, actual
image or new staging mutation is authorized/inferred by this implementation pass.

### R3 source feature disposition (not acceptance closure)

| R3 source feature | Concrete existing/current implementation | Disposition |
|---|---|---|
| Image build/provenance | Root Dockerfile and r3-image.py pinned inputs, build/push/manifest/runtime checks; bounded phase helper | Implemented/wired; actual image proof open |
| Supported chart profiles | Chart 0.2.0/values 2, four game/OIDC profiles, semantic/render negatives and explicit old-values migration | Implemented/local supplemental Helm verified; pinned/hosted gate open |
| Single writer/startup/termination | Chart one replica/Recreate/readiness/grace and server Unix signal/drain path, staging reconstruction checks | Implemented/wired; native/actual controlled termination open |
| Identity separation | Distinct init/runtime refs and r3-roles.py ownership/ledger/grant audit/setup guard | Implemented/local permission logic previously verified; real app ledger/deployment open |
| Private access/rotation/WebSocket | Scoped DB peer observation, new-Secret restart, idle/protocol/origin/operator checks; no API redirects | Implemented/wired; real access/rotation/revocation/ingress open |
| Network restrictions | Mandatory release-scoped policy, ingress/DNS selectors and bounded HTTPS destinations | Implemented/render verified; actual CNI probes/review open |
| Failed rollout/rollback | Bounded failed-image rollout and Helm rollback/fenced projection checks; existing maintenance workflow plus independent content/schema review | Implemented/wired; real fault/content/schema exercise open |

This is a scoped R3 roadmap implementation review, not an exhaustive file-level
security/behavior audit. No further source omission was identified within that
review after the collector corrections. All seven top acceptance units and exit
remain open/not closed; full R3 is not relabeled completed from this source table.

The new regression baseline initially had one local fixture error (`http` name
shadowing), which was corrected without weakening the redirect assertion. Final
17 I/O tests passed with zero skips on this Linux sandbox: real child exit/stderr,
partial timeout/overflow, closed-pipe wait, descendant-held pipe cancellation,
private/exclusive log recording and masking, interruption refusal and an actual
loopback redirect with a functioning target. This is not graceful server shutdown
or deployed TLS/CNI/Windows proof. Existing 12+9+6 methods and schema 35/real
supplemental Helm 4+22 passed; no duplicated schema invocation is added to totals.
Native compiler/fmt/Clippy/app tests and approved hosted/image/staging/manual
service adoption remain unavailable/unexecuted. The original eight-file SQL
failure remains unchanged. New current source/image/lock evidence must be built
before real acceptance; earlier local records retain their original hash scope.

The closure's final R0/R2 subprocess revalidation also now uses the bounded I/O
helper and rejects any interruption. Its private stdout stays bounded in memory,
not printed as credential evidence. No R2 prerequisite is removed or waived.
