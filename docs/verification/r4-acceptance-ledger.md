# R4 — fixed beta-load proof and measured optimization

Authority: ROADMAP_v2.md R4. Predeclared beta-250-v1 recipe and thresholds remain
unchanged. Source work can proceed; actual R4 depends on closed R2/R3 and approved
accounts, isolated PostgreSQL, protected telemetry and adopted retention policy.
No local helper/mock/supplemental protocol test is 250-client deployment proof.

| R4 unit | Source/local status | Actual acceptance |
|---|---|---|
| 1 smoke then immutable HTTPS/WS/PG beta | ✅ verified source/local: smoke-specific result, bounded unchanged native recipe and R4 collector wired; native execution unverified | ⏳ current native binaries and R2/R3 prerequisites |
| 2 successful/failed evidence and DB/deployment provenance | ✅ verified source/local: private independent observer, create-new failure evidence, signed same-source/R3-tuple closure; actual observation unverified | ⏳ approved deployment/monitor identities |
| 3 queue/owner/pool/commit/output, CPU/RSS, transactions/timer/saturation | ✅ verified source/local: eight owner/three persistence stages, CPU/RSS/PG sampler, pool/event observations; native/live measurements unverified | ⏳ actual sampled performance measurements |
| 4 Look write amplification and snapshot cost | ✅ verified source/local: attempted replacement cardinalities, clone timing and matched-report comparison; original SQL preserved | ⏳ observed baseline, not a guessed optimization |
| 5 separate fanout/reward/transfer/replay/slow/reconnect/idle/soak | ✅ verified source/local: seven separate protocol/soak drivers and bounded records; deployed scenario/30–50 minute soak unverified | ⏳ real run including preseeded fixture and adopted bounds |
| 6 measured optimization and unchanged integrity/recipe rerun | ⏳ no measured bottleneck permits algorithm/concurrency changes; comparison/current integrity rerun gates implemented | ⏳ reviewed baseline/candidate plus current integrity suite |
| R4 exit | ⏳ NOT CLOSED | Full same-tuple beta, separate soak, retention/service approval |

Implementation batch: preserve fixed Rust Look recipe, add independent report
validation and source-bound orchestration, supplement real protocol scenarios,
observational Rust instrumentation, bounded private PG/CPU/RSS/telemetry sampling,
strict closure/comparison and guarded CI/documentation. Immutable SQL, protocol,
content, beta workload/thresholds, operator exposure and retention are not changed.

## Observed local completion checks

- 16 R4 tests passed, including fixed boundary/overage/failure/schema/type/plan,
  synthetic-key signature/tamper/expiry, private create-new/duplicate-JSON/path,
  mismatched comparison, protocol tracker fixture, actual failed collection and
  CLI refusal, and real loopback telemetry with fixture DB/process identities.
- Adjacent R3: 12 base + 9 guard + 17 I/O methods and chart checker 6 passed.
  Total 60 executed Python methods, zero skips here; counts are not game capacity.
- Actual supplemental Helm v3.10.1: original four game/OIDC profiles plus one
  opt-in loopback telemetry profile; 25 negatives; 42 schema cases. Repeated
  schema calls are not added to totals. Hosted v3.19.0 remains unobserved.
- Five R4 Python subjects passed AST, nine selected Rust subjects passed grammar
  only, and guarded CI YAML/Bash bodies parsed. No Cargo/fmt/Clippy/native tests
  are counted as passed; this sandbox lacks Cargo/rustc/rustfmt/Docker/kubectl.
- Test-only Python/Helm packages were installed explicitly after sandbox reset.
  The first combined chart check hit missing PyYAML; pinned chart requirements
  were installed and the complete checker/real Helm matrix passed afterward.
- No approved plan/key/actual namespace/process/monitor credentials are invented.
  No real beta run, native SQLx/application run, 30–50 minute soak or independent
  performance/retention approval occurred. Original eight-file SQL remains
  immutable; the known operational_control.sql:188 syntax blocker is unchanged.

## Remaining deployment/acceptance gates

1. Current pinned native formatting/compiler/Clippy/application tests and image,
   paired-host baseline, full live migration/identity/integrity R2 and real R3.
2. Same-source approved restricted staging, exact image/artifact/deployment,
   verified server PID/Pod/runtime-role mapping, private telemetry forwarding and
   verified-TLS statistics access with approved monitoring identity.
3. 250 controlled beta accounts plus two independent same-room fresh scenario
   accounts; actual smoke and unchanged 7500-completion/latency/failure recipe.
4. Actual source-bound measurements and all seven protocol scenarios, especially
   deliberate transport backpressure and separately scoped long-duration soak.
5. Owner-adopted resource/retention bounds and independently signed exact-report
   performance adoption; CI reviewers/runner/trust path and hosted job execution.
6. Only after actual baseline: justify any optimization with measured evidence,
   rerun integrity/failure suites and unchanged beta and soak on the candidate.

All six actual R4 work units and exit remain open. The five source wiring rows
are scoped source/helper verification, not native/operational certification.

## Subsequent user-observed native Windows build

The user supplied terminal output from `E:\hold\Worldwright` for
`cargo build --target x86_64-pc-windows-msvc`: Finished dev profile in 46.26s,
including worldwright-persistence, worldwright-load and worldwright-server.
This is a successful user-observed Windows compilation, with one unused_mut
warning at apps/worldwright/src/commands/release.rs:169. It is not an assistant-
executed build, all-targets/test/Clippy/formatting pass, toolchain-version proof,
exact source/binary digest collection, runtime/load test or full R0/R4 closure.

The warning was source-verified: the builder is mutated only by Unix mode(0700).
The implementation now keeps mutability inside cfg(unix), preserving private
Unix directory creation and existing Windows behavior, without suppressing the
lint. The pasted successful build predates this small warning cleanup; a fresh
Windows rebuild is required to verify it. Cargo/rustc/rustfmt remain absent from
the assistant sandbox. No README/DESIGN contract change is needed for this
internal warning-only cleanup.

This append intentionally changes the ledger hash from the prior local snapshot.
The old r4-local-observations.json remains historical and is not rewritten or
promoted into native-build evidence. All actual load/soak/adoption gates stay open.

## User-observed fresh Windows check — 2026-11-11 09:28 run

User transcript worldwright-native-20261111-092815.log establishes Rust/Cargo
1.85.1, x86_64-pc-windows-msvc, reported relevant environment overrides unset,
and a new explicit target directory ending e73e9337f80540e5a64ef73e18782200.
Formatting and locked/offline workspace all-target check passed (1m59s).
All-target/all-feature Clippy then failed on blocks_in_conditions at
apps/worldwright-server/src/world.rs:1352, introduced by R4 timing instrumentation.
The script correctly stopped: tests, build and rustdoc stages were not reached.

Source fix: evaluate the same timed snapshot/export block into
runtime_state_result, then match that value. The timing guard still drops at
block exit before result handling; error/restore behavior is unchanged. No lint
suppression, dependency/SQL changes or public contract change. README/DESIGN
need no behavioral change. Native post-fix Clippy/tests/build/rustdoc still need
execution; reuse the existing native target directory to avoid rebuilding
unchanged dependencies. R5 development can remain parallel to R3/R4 acceptance.

Formatter changes and this patch intentionally supersede affected earlier text
hash snapshots. Historical observations are not rewritten into fresh passes.

## User-observed Windows continuation — generated-project defect

The subsequent cached-native check supplied by the user passed formatting,
workspace/all-target compilation (57.56s) and all-target/all-feature Clippy with
warnings denied (8.07s). Workspace tests then started: 24 worldwright unit tests,
4 content-validation tests and 2 maturity tests passed. The third maturity test,
generated_minimal_and_expedition_projects_build_and_test, failed for the relocated
minimal consumer because the nested SDK crate inherited edition from the wrong
workspace. This is 30 shown passes and 1 shown failure, not a complete workspace
test pass; later tests/build/rustdoc were not reached by the fail-fast script.

Source fix (R5 SDK delivery seam): exclude the nested SDK from the consumer
workspace and explicitly bind the five SDK crates to their two-level parent
workspace. Bundle bytes continue to be copied and hashed, not patched after
manifest creation. Added native locate-project/metadata assertions strengthen
rather than replace the existing relocated build-and-play test. Source/TOML
checks verified five inherited metadata/dependency/lint bindings, both consumer
templates and preservation of actual build/play assertions. Cargo/rustfmt are
absent from the assistant sandbox, so post-fix native confirmation is pending.
README/DESIGN/generated README describe the boundary and regeneration policy.
Prior hashes stay historical; no R4 load/soak/adoption gate is promoted.

## Correction to first SDK workspace fix — native follow-up failed

The user reran the focused Windows maturity suite: compilation succeeded,
2 tests passed, and generated-project relocation failed before building the
consumer with `member of the wrong workspace` (expected consumer, actual SDK).
The prior fix was incomplete; its TOML-only checks did not model Cargo membership
precedence. No successful native relocation result was established.

Cargo 1.85.1 source at the user-reported commit d73d2caf9 was fetched and read:
`src/cargo/core/workspace.rs`, find_members lines 757–769 and is_excluded lines
1855–1868. Explicit member paths are prefix matches and override exclusions;
`members = ["."]` therefore captures every SDK descendant. The root package is
visited automatically, and exclusions use literal path prefixes, not globbing.

Corrected generator to `members = []`, `exclude = ["sdk"]`, retaining resolver
3 and the five crates' explicit relative package.workspace bindings. Added a
regression assertion for this exact boundary, retaining all native root lookup,
metadata membership, relocated build and playable-run assertions. README and
DESIGN correct the earlier advice rather than leaving it as current guidance.
Native confirmation is still required; source-policy checks do not replace Cargo.

## User-observed workspace correction confirmation and terminal defect

Latest focused Windows run: 2/3 maturity tests passed; the generated-project test
now passed consumer/SDK root discovery, metadata membership, relocated minimal
compilation, generated content test (1 pass) and locked rebuild. It then failed
running minimal-game with Rule(MissingRule). The expedition template was not
reached, so neither the full relocation test nor full workspace suite is green.

Traced to shared terminal parsing: reference rules were correctly registered,
but expedition action verbs were unconditionally dispatched. Fixed Session to
retain its profile and gate game-specific aliases/actions/help; unknown reference
input is nonfatal and performs no runtime work. Unexpected missing-rule errors
remain fatal. Added three focused reference terminal regressions and strengthened
rather than weakened the generated reference runtime checks. README/DESIGN
updated. No API/dependency/SQL change; post-fix native tests remain required.

## User-observed Windows focused confirmation — terminal and relocated SDK PASS

The latest supplied Windows output establishes the following post-fix results
with --locked --offline and explicit x86_64-pc-windows-msvc target, reusing
worldwright-native-check-e73e9337f80540e5a64ef73e18782200:

- worldwright-expedition terminal::tests: 7 passed, 0 failed, 5 filtered out.
  Covers reference command isolation/common commands/MissingRule propagation,
  expedition recovery/progression, and bounded input.
- worldwright --test maturity: 3 passed, 0 failed, 100.47s.
- Both independently generated/relocated minimal and expedition consumers passed
  Cargo workspace/root/membership assertions, offline compilation/content tests
  (1 generated content test each), locked rebuild and actual executable checks
  from outside their project directories, including successful quit.
- The over-60-seconds line is informational; the final suite result is success.
  Zero-test generated main binaries are not additional test coverage; their
  playable execution is asserted by the outer integration test.

These focused Windows regressions are now confirmed, superseding the earlier
pending/failure notes for these same fixes. Evidence is user-observed console
output, not assistant execution or a newly hashed full native release collection.
The previously interrupted full dev.ps1 check still has remaining workspace
tests/build/rustdoc; post-fix full Clippy and paired-host/operational acceptance
are not inferred. R4 deployment/load/soak/adoption and full R5 acceptance stay
open. R5 implementation can proceed in parallel; no further targeted repair is
indicated by this output. Historical source hashes remain historical.
