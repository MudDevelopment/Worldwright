# R3 — restricted staging acceptance and evidence

Authority: `ROADMAP_v2.md`, R3 items 1–7 and exit. **R0 and R2 must pass against
current source before staging mutation. R3 is not currently closed.** The existing
fresh-migration failure in `20261106000100_operational_control.sql:188` remains
an R2 blocker; no applied SQL/checksum repair is authorized by this guide.

This guide separates implemented tooling, synthetic checker fixtures, actual
image/Helm execution, live restricted staging observations and authenticated
service adoption. The ledger in `verification/r3-acceptance-ledger.md` records
which have actually run. EWW file operations do not execute commands on the
Windows repository; selected sandbox readbacks are not a full native checkout.

## Contracts and prerequisites

- Application `0.1.0-dev.0`, Rust 1.85.1, edition 2024; no package/protocol/content
  or SQL identity changes in the R3 phase-inventory correction.
- Chart 0.2.0, **values `configVersion: 2`**. This is a breaking security change:
  digest-only image, distinct migration/runtime Secret refs, mandatory enabled
  NetworkPolicy, explicit ingress/DNS selectors and scoped HTTPS CIDRs.
- The staging-plan format is separately **`configVersion: 1`**. Do not use chart
  version 2 for the plan. Signed adoption/evidence versions are separate again.
- One actual ready `Recreate` writer and a dedicated pre-seeded, quiescent
  `worldwright_test` database/world with no pending scheduled work. The guide is
  not a command to clear schedules or remove player data to satisfy a test.
- Reviewed ingress/CNI, TLS hostname/certificate, canonical private DB network,
  Linux/amd64 image, migration/runtime/recovery logins and credential delivery.
- An existing isolated `worldwright-r3-...` namespace labeled
  `worldwright.io/acceptance=r3`, explicit context and approved maintenance scope.
  A label is a runner precondition, not a self-granted security/service approval.
- Read-only DB audit access sufficient for ownership/grant/ledger and actual
  `pg_stat_activity` peer observations. The current profile requires a direct
  private Pod-IP DB path; an unreviewed NAT/SNAT exception is not inferred.
- Approved pinned builder/runtime refs, a real pushed image report, complete
  current Ubuntu/Windows R0 and R2 proof paths, and independent approval trust root.

Do not put bearer tokens, password values, DB URLs or private signing keys in
committed values, plans, workflow inputs, logs or chat. Plan fields name private
credential environment variables; they do not carry credentials themselves.

## Component verification without installing a cluster

```bash
python -m pip install -r deploy/helm/worldwright/tests/requirements.txt
python -m pip install -r scripts/r2-requirements.txt
python deploy/helm/worldwright/tests/test_verify.py
python scripts/test-r3.py
python scripts/test-r3-local.py
python scripts/test-r3-io.py
python deploy/helm/worldwright/tests/verify.py
```

The shared chart harness runs schema/semantic checks, actual Helm lint/render
for reference/expedition with OIDC disabled/enabled, and negative render cases.
Synthetic values are deliberately not deployment approval. CI selects Helm
v3.19.0; any local Helm version is recorded exactly and is not relabeled as that
hosted/pinned pass. Missing native tools fail rather than successfully skip.

`test-r3.py` also compares AST-discovered `run`, `upgrade` and `observe_pod`
phase emissions from the actual staging source against the closure inventory.
This catches the previously missing rotated/rollback Pod evidence automatically.
It exercises complete/missing/tampered/duplicate inventories and rejects boolean,
string, signal-killed or mismatched expected-exit values. A fake fixture passing
these validators is not a staging report or service-owner adoption.

See `../deploy/README.md` for format-1 values migration, image-build inputs and
render commands. The values migrator leaves the old file untouched and writes a
new private file. It cannot create logins, Secrets, grants or certificates.

### Isolated real PostgreSQL grant-logic fixture

`scripts/test-r3-role-fixture.py` exercises actual PostgreSQL privileges and setup
transaction rollback on test-owned databases/roles. Use a separately owned,
disposable loopback test cluster and an existing `worldwright_test` URL in
`WORLDWRIGHT_TEST_DATABASE_URL`. Its test administrator needs database/role-create
rights only on that isolated test cluster; never give those rights to runtime.
Explicit opt-in is mandatory:

```bash
WORLDWRIGHT_R3_ROLE_FIXTURES=1 python scripts/test-r3-role-fixture.py
```

The suite executes its own one-file synthetic permission fixture and temporarily
patches the audit module's ROOT only inside the test call. It does NOT stamp or
repair the current eight application migrations, exercise game/SQLx transactions,
authenticate real runtime credentials or produce a consumable deployment role
report. It verifies setup, missing schema/read/insert/column/invite/sequence
permissions, inherited forbidden writes, role/ownership/checksum failures and
atomic grant rollback. All test-owned DBs and roles are removed after each case;
missing prerequisites fail, not successfully skip. POSIX link tests in the local
guard suite are distinct from still-unverified Windows ACL/link acceptance.

## Actual image and DB identities

Run the image collector only with real operator-approved refs and explicit push:

```bash
python scripts/r3-image.py --output "$R3_IMAGE_EVIDENCE" \
  --builder-image "$R3_RUST_IMAGE" --runtime-image "$R3_RUNTIME_IMAGE" \
  --tag "$R3_REGISTRY_TAG" --confirm-registry-push
```

`image.json` must be `kind=image-build,status=passed`; source must match current
R0/R2 and staging. Registry manifest bytes must match the build metadata digest.
An image config ID is not an OCI deployable digest. Runtime/linkage is exercised
as UID/GID 65532, read-only, without network/capabilities or privilege escalation.
This does not establish the correctness of a running server or license approval.

The live identity/grant audit is separate from different Secret names:

```bash
python scripts/r3-roles.py --database-url-env R3_AUDIT_DATABASE_URL \
  --runtime-role "$R3_RUNTIME_ROLE" --migration-role "$R3_MIGRATION_ROLE" \
  --recovery-role "$R3_RECOVERY_ROLE" --schema "$R3_DB_SCHEMA" \
  --output "$R3_ROLE_REPORT"
```

The private URL must already exist in the named environment variable. The audit
requires three distinct existing non-superuser logins, no inherited migration/
recovery authority for runtime, current successful migration checksums, reviewed
migration ownership, no runtime schema creation and restricted authority tables.
It now verifies every prerequisite in the existing grant recipe: schema USAGE,
SELECT on reviewed tables, full-DML transaction grants, audit/outbox INSERT,
invite-consumption UPDATE, each required column UPDATE and owned-sequence USAGE.
Missing privileges must fail even if separate Secret names and other DML checks
pass. Account password/offboarding updates remain forbidden. The optional `--apply-runtime-grants` is an explicit mutation restricted
to a disposable `worldwright_test` DB after inventory/ownership checks. It revokes
PUBLIC/runtime grants in that reviewed scope; plan the effects on other clients.
It never creates users/passwords, runs migrations or adopts recovery authority.

## Private staging plan

Prepare a bounded regular JSON file outside the checkout. Required keys:

| Key | Actual input |
|---|---|
| `configVersion` | Integer 1 |
| `context`, `namespace`, `release` | Explicit approved context and isolated namespace/release |
| `values_file`, `image_report` | Actual format-2 values and current passed image report paths |
| `r2_arguments` | Object with all seven current R2 close argument paths listed below |
| `rotation_secret` | Pre-provisioned replacement runtime Secret, distinct from both initial references |
| `failed_digest` | Reviewed digest expected to fail this rollout; not an invented successful test image |
| `database_url_env`, `credentials_env` | Environment variable names for audit URL and one test-account JSON (`username`,`password`) |
| `world_id`, `artifact_digest` | Nonzero actual world UUID and exact active artifact SHA-256 hex |
| `runtime_role`, `migration_role`, `recovery_role`, `schema` | Actual reviewed DB identities/schema |
| `ca_file` | Optional real trusted CA file; otherwise verified system TLS roots |

The `r2_arguments` keys are `collection`, `ubuntu-r0`, `windows-r0`, `process-report`,
`controlled-report`, `provider-report`, `approval-public-key`. Values are private
proof-file paths, not pass/fail booleans. The runner reruns `r2-verify.py close`;
R3 cannot waive its checks or substitute historical evidence for current source.

```bash
python scripts/r3-staging.py --plan "$R3_PLAN" \
  --output "$R3_STAGING_EVIDENCE" --confirm-staging-mutations
```

The output directory must be new and outside the checkout. The runner first
checks tools, plan/values/image/source and R0/R2 before Helm mutation. It does not
create a namespace, provider, credential or adopted runner. It retains failure
reports and sanitized phase logs. Private API response bodies are omitted from
logs; actual observations retain only needed identities/projection hashes.

### Complete staging phase inventory: 13

1. `r0-r2-prerequisites`
2. `chart-matrix`
3. `namespace-approval`
4. `db-role-audit`
5. `install`
6. `deployment`
7. `actual-pod-image`
8. `rotate-runtime-secret`
9. `rotated-pod-image`
10. `helm-status`
11. `failed-rollout`
12. `helm-rollback`
13. `rollback-pod-image`

The expected failed rollout must have an **integer positive nonzero** process
exit. A signal-killed Helm process, missing result, boolean or zero exit is not
that proof. Every other phase must exit zero. Each phase has a bounded retained
`name.log` and SHA-256; absent/tampered/duplicate/unknown phases prevent closure.
The image collector has its own eight phases; do not mix the inventories.

### Six automated observed cases

| Case | Required actual observation |
|---|---|
| `startup-tls` | Verified HTTPS certificate/host and readiness after actual install |
| `single-writer-reconstruction` | One ready Recreate writer, built image identity, runtime DB peer and unchanged projection |
| `db-role-audit` | Live ownership/grants/current ledger, not just distinct references |
| `websocket-idle-origin-operator-denial` | Real protocol/version/sequence/epoch attachment, four 20-second idle intervals with application pings, Look completion, foreign-origin 403 and absent public operator routes |
| `secret-rotation-restart` | New actual Pod UID, matching built image and runtime DB role, advanced writer epoch, stable projection and non-rewinding versions |
| `failed-rollout-helm-rollback` | Actual failed rollout/not-ready, reviewed Helm revision rollback, new actual Pod/image/role and non-rewinding durable reconstruction |

A successful collection is `collected_not_closed`. It still has
`full_r3_closed=false`; these cases do not constitute the four independent cases.
The 80-second idle exercise is not R4 load/soak or a general capacity guarantee.

## Four independently witnessed gates

These need an authorized operator, real platform controls, retained bounded
noncredential evidence and an independently trusted service-adoption signature.
No tool fills them as passed from source or a synthetic unit key.

### NetworkPolicy/CNI positive and negative controls

Use reviewed hardened probe workloads in the isolated namespace: non-root,
read-only, no service-account token/capabilities/escalation, bounded lifetime.
Verify the ingress-controller namespace AND pod labels against the actual cluster.
Prove allowed ingress, private DB access, scoped DNS and (when used) approved OIDC
HTTPS endpoints. Then prove a different release/unapproved ingress peer and an
unapproved egress target are denied. Each denial needs a positive control that
can reach the same functioning target; a wrong address, dead listener or absent
route is not a policy denial. Record CNI/controller versions and applied policy.

A policy-selected probe must not become a ready gameplay Service endpoint or a
second writer. Keep it explicitly unready for that selector, preserve the real
writer's endpoints and remove only identified test-owned probes afterward.
Changing another release's labels or cluster-wide policy is outside this exercise.

### Controlled termination

Capture the real workload's Pod UID/status/termination events, sanitized server
logs and readiness/admission behavior. Issue approved normal Pod termination;
prove SIGTERM enters drain, the process exits normally within the reviewed grace
budget, admitted durable work is settled or explicitly left uncertain, and the
replacement reconstructs before becoming ready. Exercise the idle and active
cases required by the owner's acceptance plan. Forced SIGKILL, a missing exit
observation or a new Pod alone is not graceful termination evidence. Do not
rewrite migration/data state to manufacture successful restart.

### Old-secret revocation

After the scripted rotation verifies a new Pod, image and DB peer under the
runtime role, use the approved secret manager/DB process to revoke the old
credential. Prove it fails to authenticate without printing its value and that
the replacement stays healthy. Distinct Secret names alone do not prove old
credentials stopped working. Never rotate/revoke migration or recovery identity
as a side effect of this runtime exercise. Record safe rollback/forward-repair
steps; do not roll back to a revoked credential accidentally.

### Content rollback/schema compatibility review

Review the intended old binary/content against the current immutable SQL ledger,
game contract/state encoding and durable room/item/counter references. Exercise
the supported maintenance publication/activation/recovery workflow on this
isolated world; retain actual old/new digests, coherent readiness/gameplay and
reviewed outcome. Unsupported/lossy rollback must fail rather than silently
rewrite data. The automated failed-image Helm rollback is not this content test.
Helm does not undo SQL, receipts, incarnations or content activation. Never edit
applied migrations/checksums to force compatibility. R6 disaster recovery and
R4 performance claims are outside R3.

## Authenticated same-tuple closure

An adoption JSON is `kind=r3-service-adoption,status=adopted`, with an actual
printable, nonblank reviewer string bounded to 256 characters and integer `expires_at` in the future and at most 30 days.
It binds `source_sha256`, `collection_sha256`, `image_report_sha256`,
`image_reference`, `artifact_digest`, `namespace_uid` and `deployment_uid` exactly
to the current source/collection/image. Its `cases` array contains each of the
four independent case names exactly once with `status=passed` and a nonempty
`evidence` array of contained relative `path`/SHA-256 references to actual files.

An independently authorized Ed25519 key signs UTF-8 JSON of all fields except
`signature`, sorted keys and compact separators; `signature` is strict base64.
The public-key trust root is installed independently of the report, not generated
by this workflow. A synthetic helper key does not adopt a service.

```bash
python scripts/r3-close.py \
  --collection "$R3_STAGING_EVIDENCE/staging.json" \
  --image "$R3_IMAGE_EVIDENCE/image.json" --adoption "$R3_ADOPTION" \
  --r2-arguments "$R3_R2_ARGUMENTS_FILE" \
  --approval-public-key "$R3_APPROVAL_PUBLIC_KEY"
```

Closure verifies all inventories/log hashes, source/image tuple, role report,
contained proof bytes and signature, then reruns every current R0/R2 prerequisite.
Failure returns `R3 NOT CLOSED`; no R4 capacity or production readiness is implied.

## CI and adoption boundary

`r3-acceptance.yml` runs Ubuntu/Windows chart/helper/local-guard jobs with Helm
v3.19.0. A separate Ubuntu job explicitly opts into the synthetic permission
fixture on its disposable PostgreSQL service and retains fixture reports/logs;
that is not current app-migration, credential or deployment acceptance. Its
real staging job is default-branch manual only, in `worldwright-r3-staging`, on an
explicitly approved isolated ephemeral nonproduction runner. Configure required
reviewers, protected branch, private plan/proof/credential provisioning and the
independently installed public-key path before use. YAML does not create/adopt
those controls. P1's smoke uses pinned base-image repository variables; it is not
an R3 pushed-image or staging acceptance job. Record real hosted results rather
than promoting workflow definitions into a pass.

Failure may leave this intentionally disrupted staging deployment not ready.
Retain the report/logs and recover through the authorized maintenance plan. Never
silently delete a namespace, player data, shared Secrets or retained evidence.

### Role-proof type and identity binding

The collector now includes the expected runtime/migration/recovery roles and
schema in its evidence. Closure requires a complete real `database-role-audit`
report with successful identity checks/current migration count, not merely a
`status=passed` file with a matching hash. Role/schema identities must equal the
collection, recovery must remain unadopted, and counts/flags must have exact
types. This rejects the intentionally synthetic local fixture report even when
its tests pass; positive unit metadata shapes are still not deployment approval.

### Bounded/interrupted tool phases and redirect refusal

The image/staging collectors now use the shared R3 I/O helper with a 16 MiB raw
capture/log cap, bounded queue, noninteractive stdin and process-tree cancellation.
An interrupted phase retains its partial private log, actual exit, elapsed time
and interruption marker. It is not an accepted expected failed rollout, even if
Windows cancellation returns a positive exit; closure checks the marker too.
Private API bodies stay omitted; configured credentials/password values are
masked in retained logs. Keep evidence private; exact masking is not a general
secret-classification guarantee. HTTPS API redirects fail, rather than changing
the target of readiness or login. `test-r3-io.py` runs real local subprocess/pipe/
loopback-HTTP cases; it is not actual TLS/Worldwright/CNI/Windows acceptance.
