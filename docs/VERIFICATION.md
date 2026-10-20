# World Runtime v0.1 Verification

**Scope:** Real Linux development checks for this source snapshot. This is not verification of a deployed MUD, authorization system, or production service.

## Environment and Toolchain Provenance

- Actual compiler: `rustc 1.85.0 (4d91de4e4 2025-02-17)` on x86_64 Linux.
- Actual Cargo: `cargo 1.85.0 (d73d2caf9 2024-12-31)`.
- Rustfmt, Clippy, compiler, Cargo, and standard library came from the matching third-party `@rustbin/*-1.85.0-x86_64-unknown-linux-gnu` npm packages in a temporary sandbox toolchain. Package install scripts were disabled.
- Those tools were used only for local checks, not adopted as the project's approved distribution channel. Their binary provenance was not independently signature-verified. The toolchain and npm packages are not included in the archive.
- The project still pins **Rust 1.85.1**, edition 2024, resolver 3. Exact pinned-toolchain and Windows acceptance remain pending.
- The workspace has no external Rust dependencies. Cargo generated the supplied lockfile and checked it with offline metadata resolution.

## Observed Results

The shared Bash `bootstrap` and `check` tasks completed successfully for this runtime increment. See the captured [runtime Linux workflow log](verification/runtime-linux-rust-1.85.0.log). The older core-interface log remains historical evidence for the preceding snapshot.

| Check | Result |
|---|---|
| Cargo lock generation and locked metadata | Passed; Cargo.lock is included |
| `cargo fmt --all -- --check` | Passed after applying rustfmt |
| `cargo check --workspace --all-targets --locked --offline` | Passed |
| Clippy, all targets/features, warnings denied | Passed |
| Workspace tests | 71 unit/integration tests passed; zero failures |
| New runtime coverage | 24 end-to-end tests: actions, attachments, policy, staging, rollback, routing, and limits |
| Rustdoc compile-fail test | 1 passed, confirming typed-ID separation |
| Total tests | **72 passed; 0 failed** |
| Workspace debug build | Passed |
| API documentation, warnings denied | Passed |
| Application smoke tests | Passed for all four binaries; metadata succeeds and unsupported/default startup fails explicitly |
| Bash workflow | Actual bootstrap/full check passed; help, doctor, invalid task/argument, and outside-workspace invocation checked |
| Workspace/package structure | 10 members; local acyclic dependencies; aligned application names; lockfile package set matches |
| CI configuration | YAML parsed; Linux/Windows jobs, pinned checkout, time limit, and read-only permissions checked |
| Documentation | 22 Markdown documents checked; 237 local links/anchors resolve; table blocks, fences, and JSON examples validate |

The local world runtime is a real implementation exercised by the new tests. It does not replace content validation, database operations, account authentication, or network-service startup with fake success; their stub guards remain. Normal execution errors leave owned world state unchanged and return no output. Caller RNG and native rule-owned side effects are outside that rollback guarantee.

## CI and Platform Limits

- GitHub Actions configuration is supplied for Linux and Windows, but no remote workflow run is claimed.
- The PowerShell script was inspected, not executed in this Linux environment.
- Rust 1.85.1, Windows builds, release-mode behavior, benchmarks, authentication, networking, SQL migrations, persistence recovery, UI, and deployed security controls were not tested here.
- Runtime ownership/attachment and effect-policy checks were tested; account login, capability models, restart-safe sessions, and durable execution were not. An in-memory result is not a persisted receipt.

Run [DEVELOPMENT.md](DEVELOPMENT.md) with the approved pinned toolchain and inspect both remote jobs before establishing a merge/release gate. No operational support commitment, license approval, or production-readiness claim is created by these checks.
