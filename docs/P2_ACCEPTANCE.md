# P2 acceptance — WWV2-016

Updated November 9, 2026. ROADMAP_v2.md defines one P2 item; its four units and
observed checks are in verification/p2-implementation-ledger.md. Source and
documentation are implemented. Full native/relocated/hosted acceptance remains
open. P0/P1 gates, publish=false and owner/security/release restrictions remain.

## Contract and compatibility

| Surface | Current contract | Preserve |
|---|---|---|
| Sequential runtime benchmark | JSON schema 2; recipe runtime-look-nearest-rank-v2; recipe_version 2; percentile_recipe nearest-rank-integer-v1 | Existing clients/commands/elapsed/p50/p95/p99/max/delivered fields; integer nanoseconds; no network-capacity claim |
| Historical runtime benchmark | Unversioned v1 reports select ceil((N-1)*percent/100) | Original report/hardware/revision/recipe; do not relabel or invent raw samples |
| Beta-250 load | Existing schema/profile/units and nearest-rank calculation | 250 clients, 7500 completions, two-second cadence, zero failures, p95<=750ms, p99<=1500ms, max<10s, wall<=90s |
| CLI compatibility | Existing matrix fields plus schema_version 1 and sdk_package_version | Separate package/API/protocol/content identities; no fictional previous minor |
| Bundle | SDK package/minimum Rust authority; private source manifest | Immutable prior bundles, exact version match, reduced/source lock provenance, no signed publisher claim |
| Retained consumer | Captured API-v1 subset from 0.1.0-dev.0, not a released package | Frozen source/inventory SHA-256, intentional review for changes; no blanket API/ABI promise |

Shared index: ceil(N*percent/100)-1, for nonempty ascending samples and integer
percent 1..100. Quotient/remainder arithmetic avoids multiplication overflow.
Invalid dimensions return None. Empty beta stats retain zero count/zero values.
Testkit and network load are different workloads despite shared arithmetic.

## Check current source first

Python 3.11+ standard-library checks require no registry downloads:

```bash
python scripts/test-p2-contracts.py
python scripts/test-p2-negatives.py
```

Eight checks cover generated package/members/private policy, registry template/
checksum consistency, metadata authority wiring, content dispatch inventory,
shared arithmetic, retained fixture integrity, SDK transport independence and
real-export test wiring, and fixed beta/runtime recipe identities. They parse
source/TOML and fail on unsupported source shapes. They are not Rust execution.
Six negative cases first require a passing baseline, then reject fixture, registry,
CLI inventory, beta threshold, unknown-source-shape and relocation-guard mutations.
They copy only 17 allowlisted small source files into temporary folders, not
build output, secrets, .git or the complete repository; production source is never
mutated. The retained consumer digest normalizes LF/CRLF; never update the digest merely
to silence an incompatible SDK change. Keep a new fixture/version entry when
reviewed support evolves.

## Native Ubuntu and Windows gates

Use the actual current checkout, approved Rust 1.85.1, native linker and approved
registry cache. Initial fetch/bootstrap may need approved network access.
Repair the FIRST failure, preserving its log; stale older passes do not count.

```bash
bash scripts/dev.sh bootstrap
cargo fmt --all
bash scripts/dev.sh check-native
python scripts/test-p2-contracts.py
```

```powershell
./scripts/dev.ps1 bootstrap
rustup run 1.85.1 cargo fmt --all
./scripts/dev.ps1 check-native
python scripts/test-p2-contracts.py
```

The ordinary all-target/native gate includes new measurement, benchmark/report,
compatibility/bundle and retained downstream tests. Formatting alone is not a
compile pass. Python source assertions must remain tolerant of rustfmt layout.
The frozen fixture is data included by the integration test, not a new workspace
package or release.

Focused reruns after resolving the full gate:

```bash
cargo test -p worldwright-core --locked --offline measurement::tests
cargo test -p worldwright-testkit --locked --offline runtime_percentiles_use_versioned_nearest_rank
cargo test -p worldwright-load --locked --offline beta_percentiles_match_shared_small_and_full_sample_recipe
cargo test -p worldwright --locked --offline --test sdk_compatibility -- --test-threads=1
cargo test -p worldwright --locked --offline --test maturity compatibility_and_hundred_client_workload_are_machine_readable
```

List tests first when selecting an exact case and retain output proving it
actually ran; a zero-test command is not evidence. Do not weaken dimensions or
invent prior versions to make tests green.

## Relocated downstream acceptance

The native sdk_compatibility test creates a real source bundle, verifies it,
moves it into a separate consumer tree, creates a separate package with only
relative SDK dependency paths and retained API-v1 source/content, and selects
an explicit native target/output directory. It bootstraps a consumer Cargo.lock
offline, then reruns with --locked and verifies that both consumer/bundle locks
and the bundle manifest remain unchanged. It requires the committed source
checkout only for INITIAL bundle creation; consumer compilation uses relocated
SDK paths. This is not hermetic compilation or registry vendoring.

The regression also inspects locked/offline Cargo metadata for the explicit host:
all local manifests must remain under the canonical relocated consumer root,
expected local crates are checked and transport must not enter the SDK graph.
Independently retain/review that inspection as acceptance evidence; confirm no
dependency resolves to the original source checkout. Repeat on
Ubuntu and Windows. Installed CLI supplied-bundle usage still requires a
trusted matching WORLDWRIGHT_SDK_BUNDLE. Do not remove user source trees merely
to prove relocation. No package has been published and no previous released
SDK minor is available for historical binary/minor testing.

## Runtime report and history acceptance

```bash
cargo run -p worldwright --release --locked --offline -- benchmark --clients 100 --commands-per-client 100 --json
cargo run -p worldwright --locked --offline -- compatibility --json
```

Confirm benchmark schema/recipe metadata and integer units; retain current
revision, OS/hardware, profile, source/dependency identities and new report.
Compare like-for-like recipes/workloads only. The historical milestone-8 report
is preserved as v1 evidence; no P2 performance run or optimization is inferred.
The beta-load contract in verification/milestone-9-load-contract.md is unchanged
and still needs its separate approved live PostgreSQL/deployment run.

## CI and acceptance evidence

ci.yml runs required positive and negative P2 Python checks in the Ubuntu/Windows native matrix,
uses pipefail, retains non-secret contract logs for 14 days, and fails on missing
logs. GitHub.com/Enterprise artifact-action pins remain separate. Native tests
run in check-native; passing Python logs do not prove downstream compilation.
Hosted execution, approved mirrors/tool installation and required branch checks
must be observed/adopted independently, not assumed from YAML.

Before marking any P2 unit accepted, retain native formatting/check/Clippy/test/
build/docs results, exact fixture digest, relocation/dependency inspection,
versioned runtime output and hosted artifacts for the same source revision.
No current native Rust, actual SDK bundle/consumer build or hosted run has been
executed through EWW or the sandbox. P2 does not add a DB migration, auth/port,
package license grant, signing approval or production readiness.
