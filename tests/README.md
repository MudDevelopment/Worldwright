# Shared Test Support

`app_smoke.rs` is included by Server, Studio, and Console through `tests/smoke.rs`. Console's previously undiscovered `test/smoke.rs` has been moved into `tests/`. The developer CLI uses feature-specific smoke and content-validation tests. A virtual workspace does not discover root-level integration tests automatically.

Local Play adds application unit tests and `apps/worldwright/tests/play.rs` subprocess tests for runtime-backed commands, input/EOF handling, prompt selection, safe output, and cleanup. Content tests exercise strict input/shape/version validation, resource limits, graph checks, diagnostics, mapping, and named exits. Testkit exercises both the independent programmatic fixture and JSON-loaded Look/Move/Look/Say scenarios. No networking, account login, persistence, or operational readiness is claimed.

Run `cargo test --workspace --locked --offline` after bootstrap. See [verification](../docs/VERIFICATION.md) for observed totals, dependency/toolchain provenance, and platform limits.
