# P2 implementation ledger — November 9, 2026

Scope authority: ROADMAP_v2.md. Only WWV2-016 is explicitly P2 in the current
19-entry backlog. P0/P1 acceptance remains open; beginning P2 neither reclassifies
nor closes those gates. Work is performed directly in E:\Hold\Worldwright.

| Unit | Acceptance status | Scope / evidence required |
|---|---|---|
| WWV2-016.A | ⏳ not yet checked | Version the sequential-runtime percentile recipe; share overflow-safe nearest-rank selection with beta load; retain historical recipe identity and unchanged beta thresholds. |
| WWV2-016.B | ⏳ not yet checked | Derive CLI arrays, content versions, SDK package/bundle strings from authoritative structures; guard SDK's intentional transport-independent inventory against real protocol exports. |
| WWV2-016.C | ⏳ not yet checked | Retain an actual API-v1 downstream source fixture and version inventory; generate/run it from a relocated SDK bundle. No fictional previous package/minor or ABI guarantee. |
| WWV2-016.D | ⏳ not yet checked | Update roadmap, toolkit/design/README/acceptance docs and evidence; run current native Ubuntu/Windows gates and locked relocated downstream checks. |

## Contract before changes

- Core will supply a pure rank-index utility with no transport/storage dependency.
- Existing benchmark percentile values can change at small sample counts: old
  v1 index ceil((N-1)*percent/100), new v2 index ceil(N*percent/100)-1.
- Beta-250 already uses nearest rank; its schema/profile/thresholds remain unchanged.
- SDK keeps production independence from the transport crate. A non-bundled CLI
  development test can verify its declared protocol list against real exports.
- SDK supported API list stays [1]; package minor support is a policy, not evidence
  that another minor exists. Fixture source must not silently track API changes.
- No new database migration, auth change, public listener, package, license grant
  or production-readiness claim is intended.

## Environment and next batch

Read current roadmap/P1 ledger, benchmark.rs, SDK lib, CLI maturity/bundle,
load report, relevant manifests, core/content/protocol exports, toolkit/load docs
and existing generated-project regression. EWW does not execute commands.
Native Cargo/rustc availability must be rechecked before claiming execution.
Next: implement A/B, retained fixture C and focused regressions; update this
ledger after source/readback/static and native verification separately.
