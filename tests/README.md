# Shared Test Support

`app_smoke.rs` is included by each application's `tests/smoke.rs`. A virtual Cargo workspace does not discover root-level integration tests automatically; the application wrappers make these tests part of each package's integration-test target.

The tests check help/version/status output and fail-closed startup/unsupported operations. Library tests check explicit stub errors and the provisional SDK metadata facade. They do not test networking, authentication, persistence, gameplay, or operational readiness.

Run `cargo test --workspace --locked --offline` using the supplied Cargo.lock, or the shared developer workflow. Local Linux Rust 1.85.0 tests passed. Core tests additionally cover IDs, bounded values, scheduling, and rule planning; Testkit tests cover fixtures and registration. See [verification](../docs/VERIFICATION.md) for exact results and platform limits.
