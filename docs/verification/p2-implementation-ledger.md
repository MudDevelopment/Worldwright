# P2 implementation and acceptance ledger — November 9, 2026

Scope authority: ROADMAP_v2.md. WWV2-016 is the sole explicitly P2 item in the
current backlog. All four source/documentation units below are implemented;
FULL NATIVE ACCEPTANCE IS STILL OPEN. P0/P1 statuses are not promoted by P2.
Repository: E:\Hold\Worldwright through EWW, which provides file operations,
not native command execution. Sandbox recheck found Python 3.12.3 but no Cargo,
rustc or rustfmt. Earlier Windows/M8 results do not validate these changes.

| Unit | Source status | Acceptance status | Installed work / remaining evidence |
|---|---|---|---|
| WWV2-016.A | Implemented | ⏳ native not yet checked | Shared overflow-safe core nearest rank; runtime recipe v2/schema2; beta helper consolidation; positive boundary/wide-integer/native regressions. Needs actual native tests and identified new runtime report. |
| WWV2-016.B | Implemented | ⏳ native not yet checked | CLI matrix serialization, SDK package identity, content dispatch/validation/inventory authority, dynamic bundle metadata; dev-only actual-protocol export test; source/TOML/registry drift checks. Needs actual CLI/bundle metadata execution. |
| WWV2-016.C | Implemented | ⏳ native not yet checked | Retained real API-v1 consumer and LF integrity inventory; relocated bundle bootstrap/locked rerun; Cargo metadata checks local-manifest containment and excludes transport. Needs actual Ubuntu/Windows relocated compilation/execution. |
| WWV2-016.D | Implemented | ⏳ hosted/native not yet checked | Roadmap/README/DESIGN/toolkit/development/measurement docs and P2_ACCEPTANCE complete for P2; CI positive/negative logs with retention and missing-log failure. Needs actual current native/hosted runs, not YAML inference. |

## Decisions preserved

- Runtime historical recipe v1 selects ceil((N-1)*percent/100); new reports use
  schema 2/runtime-look-nearest-rank-v2 and index ceil(N*percent/100)-1.
  Old fields/units remain; the original M8 report is preserved with an explicit
  historical recipe label, not silently relabeled or treated as speedup proof.
- Shared percentile recipe is nearest-rank-integer-v1. Quotient/remainder arithmetic
  avoids count*percent overflow; invalid dimensions return None.
- Beta-250 already used nearest rank. Its schema/profile, 250 clients/7500
  completions/two-second cadence and zero-failure/p95/p99/max/wall gates are fixed.
- Package version 0.1.0-dev.0, SDK API1, content1 and protocol1 are separate.
  No previous released package/minor, stable ABI or universal API coverage is claimed.
- SDK retains production independence from transport. Only CLI dev dependencies
  compare actual protocol exports. Cargo.lock adds that local CLI dependency edge;
  no registry pin/checksum changes were made by P2.
- Retained consumer is captured from actual API-v1 source, not a previous release.
  Its source_sha256_lf is cdad147072a69bb3937e19c0a5dbc17c3d29c844cac13733caf912bcb7b42e11.
  Do not rewrite the fixture/inventory merely to make an incompatible upgrade pass.
- No P2 DB migration, authentication/listener change, additional workspace package,
  license grant, signing decision or production-readiness promotion.

## Fixes in this continuation

1. Corrected a formatting-sensitive Python assertion so rustfmt's multiline
   --locked conditional remains accepted. Unknown source structures still fail.
2. Added runtime CLI benchmark assertions for schema2/recipe/recipe_version/
   percentile_recipe, not merely counts and numeric latency fields.
3. Added locked/offline Cargo metadata inspection to the retained consumer test:
   canonical local manifests must remain inside the relocated consumer, the four
   expected local package names are checked, and protocol cannot enter the graph.
4. Added reproducible test-p2-negatives.py: passing baseline required; six mutations
   use only 17 allowlisted bounded source files in disposable copies. No .git,
   target, secrets or complete repository is copied; original source is never mutated.
5. CI uses pipefail, runs positive/negative checks on Ubuntu/Windows, appends both
   outputs to non-secret P2 logs, retains for 14 days and refuses missing logs.
   GitHub.com v4 and Enterprise v3 artifact-action pins remain separate.
6. Completed P2 documentation and reconciled toolkit's stale expedition-local-only
   and old path/recipe claims. This is a scoped P2 docs pass, NOT closure of the
   separate unfinished WWV2-017 whole-documentation audit.

## Observed checks in this continuation

Current selected source readbacks were staged in a fresh sandbox directory;
no historical snapshot was used for the final suite. Re-read tool outputs:
194514-zzBfVSt8J3FERucQes6bHzIH5eT6ZR2rxbAO,
194523-A751EBAnsXUnMoOucAzAXSrnFj54XxmqILWi,
194932-WSdZeosdAco4kmjTDuuV75YKb1uCsJxAp0NP (27 selected files, not the full repo).

| Check | Actual result | Does not establish |
|---|---|---|
| Python test-p2-contracts.py | PASS, all 8 cases on current text readbacks; rerun after guard changes passed | Rust compilation or an executed generated SDK bundle |
| Python test-p2-negatives.py | PASS, all 6 mutation rejection cases after passing baseline | Native runtime behavior or security certification |
| Python compile checks for both scripts | PASS | mypy/ruff/hosted policy checks; none claimed |
| Rust tree-sitter grammar for 12 selected files | PASS, zero grammar errors | Type/borrow checking, rustfmt, Clippy, linkage or Rust tests |
| CI YAML/job wiring | PASS: Ubuntu/Windows matrix, P2 pipefail, separate artifact uploads, retention and missing-log refusal | Hosted job execution or ruleset/environment adoption |
| Fixture LF-normalized digest | PASS through source suite | Publisher/authorship trust or historical binary/minor compatibility |
| Reduced/root registry checksum consistency | PASS through source suite | Registry supply-chain certification or locked Cargo acceptance |

A temporary sandbox venv installed tree-sitter/tree-sitter-rust for grammar
inspection only; no parser dependency was added to product manifests. Positive
and negative source tests use the Python standard library (3.11+). Native Rust
and current benchmark/SDK/consumer/hosted jobs were NOT executed here.

## Documents synchronized

ROADMAP_v2.md; README.md; DESIGN.md; docs/TOOLKIT_V1.md;
docs/DEVELOPMENT.md; docs/P2_ACCEPTANCE.md;
docs/verification/milestone-8-performance.md (historical report preserved);
docs/verification/milestone-9-load-contract.md (beta recipe unchanged); this ledger.

## Concrete remaining acceptance work

1. Use approved Rust 1.85.1/native linker/cache in the actual source tree; run fmt
   then check-native on Ubuntu and Windows. Repair and retain the FIRST failure.
2. Execute actual core/testkit/load/CLI metadata/bundle/retained-consumer tests.
   Inspect the relocated Cargo graph and capture its lock/manifest/fixture identities.
3. Capture the schema2 runtime report with source/toolchain/OS/hardware/profile;
   compare recipes honestly. Keep beta250's separate approved live-PG acceptance.
4. Inspect actual hosted native/P2 artifacts; adopt approved required checks/action
   mirrors independently. Do not close any unit from source/grammar checks alone.

Full source implementation/documentation work for P2 is complete at this ledger
snapshot. Only observed environment-specific acceptance may close WWV2-016.
