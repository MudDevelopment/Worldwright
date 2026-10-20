# Worldwright Initial Rust Scaffold

This document describes the files actually supplied in this package. The larger designs in [DESIGN.md](../DESIGN.md) and [ARCHITECTURE.md](../ARCHITECTURE.md) remain targets, not implemented features.

## Scope

- Ten Cargo members: four application binaries and six libraries.
- Package version `0.1.0-dev.0`; all members have `publish = false` through workspace inheritance.
- Rust 1.85.1 selected in `rust-toolchain.toml`, edition 2024, resolver 3, manifest minimum 1.85. This is a bootstrap choice, not a verified MSRV/support matrix or latest-version claim.
- Only Rust standard-library and local workspace dependencies. No registry downloads are required for package dependency resolution; installing the Rust toolchain itself may require network access.
- Shared unsafe-code prohibition and Clippy configuration.
- Metadata-only application commands and explicit errors for missing functionality.
- Library stub tests and shared application smoke tests, authored but not executed here.
- Original project documentation retained and updated to distinguish scaffolding from future behavior.

No game server, UI, world simulator, database schema, content schema, protocol implementation, authentication, authorization, TLS, migration, deployment, or CI provider configuration is included.

## Workspace Map

| Package | Source entry | Present behavior |
|---|---|---|
| `worldwright` | `apps/worldwright/src/main.rs` | CLI help, version, scaffold status; no development actions |
| `worldwright-server` | `apps/worldwright-server/src/main.rs` | Metadata commands; default startup explicitly fails |
| `worldwright-studio` | `apps/worldwright-studio/src/main.rs` | Metadata-only builder placeholder; no window or web UI |
| `worldwright-console` | `apps/worldwright-console/src/main.rs` | Metadata-only operator placeholder; no administration |
| `worldwright-core` | `crates/worldwright-core/src/lib.rs` | `NotImplemented`, metadata-only `GameModule`, `WorldStub` |
| `worldwright-content` | `crates/worldwright-content/src/lib.rs` | `ContentPipelineStub`; validation always returns an error |
| `worldwright-protocol` | `crates/worldwright-protocol/src/lib.rs` | Human-readable status and an empty supported-version list |
| `worldwright-persistence` | `crates/worldwright-persistence/src/lib.rs` | `PersistenceStub`; connection always returns an error |
| `worldwright-sdk` | `crates/worldwright-sdk/src/lib.rs` | Provisional core/content re-exports and status |
| `worldwright-testkit` | `crates/worldwright-testkit/src/lib.rs` | Fixture-facing re-exports and `ScenarioStub`; run always errors |

The server's `src/application.rs` is the future composition root. Today it only calls the refusing world-start stub. A metadata-only `GameModule` trait is not an implemented rule registration/effect system.

Studio and Console have separate binary packages because the requested scaffold includes the full suite. This does not settle whether future UI routes will run inside the server or as independently packaged applications.

## Dependency Boundaries

- Core and protocol have no Worldwright dependencies.
- Content and persistence depend on core.
- SDK depends on core and content, never persistence or an application.
- Testkit depends on core, content, and SDK, never a server application.
- Server composes core, content, protocol, persistence, and SDK.
- CLI depends on content and SDK.
- Studio depends on content and protocol; Console depends only on protocol.

Some composition dependencies are reserved and not yet used by executable statements. These are local boundary declarations, not installed networking/database functionality. Applications are never dependencies of libraries.

## Build on a Rust Development Machine

The generation environment had no Cargo, rustc, rustfmt, or Clippy. These commands are the required local verification procedure, not reported successful runs.

Install the pinned toolchain through your approved Rust distribution process. For a rustup-managed workstation:

```bash
rustup toolchain install 1.85.1 --profile minimal --component rustfmt --component clippy
```

From the extracted `worldwright/` directory:

```bash
rustc --version
cargo --version
cargo generate-lockfile --offline
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked --offline
cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
cargo test --workspace --locked --offline
cargo build --workspace --locked --offline
```

`Cargo.lock` is intentionally not hand-authored. Generate it using Cargo, review it, and commit it. This virtual workspace contains applications, so the lockfile belongs in version control. The package has only local dependencies, but an absent Rust toolchain can still cause rustup to attempt a toolchain download before Cargo starts.

If rustfmt requests edits, run `cargo fmt --all`, inspect the diff, then repeat the verification loop. Source parsing in this environment does not establish rustfmt compliance or successful type checking.

Convenience aliases after lockfile generation:

```bash
cargo fmt-check
cargo check-all
cargo lint
cargo test-all
```

## Application Behavior

Each executable accepts exactly one argument:

| Argument | Expected exit code | Meaning |
|---|---|---|
| `--help` or `-h` | 0 | Print scaffold help |
| `--version` or `-V` | 0 | Print Cargo package version |
| `status` | 0 | Report scaffold status, not service health |
| No argument | 2 | Requested default behavior is not implemented |
| Unknown argument or extra arguments | 2 | Reject unsupported use |

Examples after bootstrapping Cargo.lock:

```bash
cargo run -p worldwright --locked --offline -- --help
cargo run -p worldwright-server --locked --offline -- status
cargo run -p worldwright-studio --locked --offline -- status
cargo run -p worldwright-console --locked --offline -- status
```

No command listens on a port or contacts a database. `status` is a local metadata report, not a readiness probe. Default server startup refuses to succeed. There is no `serve`, `new`, `validate`, `publish`, or `deploy` subcommand yet.

## Test Placement

Library tests live beside the relevant stubs. The server composition root also has a refusal test.

Each application includes `tests/app_smoke.rs` through its own integration-test wrapper. This matters because Cargo does not discover integration tests directly at the root of a virtual workspace. The shared tests check metadata output and explicit rejection of unsupported operations.

These tests are intended to change as actual features replace stubs. A test requiring `NotImplemented` is a scaffold guard, not a permanent product requirement. Replace it with positive and negative feature tests in the same change, documenting the newly supported behavior.

## First Implementation Tasks

1. Run and record the real Cargo verification loop; generate and commit Cargo.lock.
2. Approve ownership, outbound licensing, and inbound contribution terms before public distribution.
3. Define distinct domain ID types and a minimal command/effect boundary in core.
4. Add a small versioned content schema and a two-room fixture with reference validation.
5. Implement a deterministic, in-process look/move vertical slice and Testkit assertions.
6. Add transport and persistence integrations separately with reviewed dependencies and failure tests.

Do not start by adding all planned dependencies or implementing several incomplete service stacks at once.

## Licensing

No license files, copyright attribution, or Cargo license grant have been invented. The root metadata records `MIT OR Apache-2.0` as a proposal only. Packages cannot be published through ordinary `cargo publish` while `publish = false` remains set; this is a guardrail, not a legal access-control mechanism.

After authorized adoption, add complete license texts and required notices, then declare the adopted license in workspace package metadata and inherit it in each package. The code of conduct, private reporting routes, and maintainer appointments still require operational adoption.

## Verification

See [VERIFICATION.md](VERIFICATION.md) for checks actually performed and explicit gaps. Do not claim a compiled, tested, production-ready toolkit from the presence of this scaffold.
