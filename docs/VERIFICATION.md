# Verification

## Current: Local Play v0.1

**127 tests passed (125 unit/integration + 2 compile-fail doctests)** on Linux Rust 1.85.1 after a clean rebuild. Formatting, all-target checks, Clippy, debug build, and API documentation passed. A real Linux pseudo-terminal smoke test verified prompts, movement, and quit.

See the [Local Play verification report](verification/local-play-report.md) and captured logs for actual results and provenance limits. Windows/hosted CI and approved registry-artifact acceptance remain pending; verification still uses the temporary dependency-source cache described there.

---

## Historical: Content Loading v0.1

**103 tests passed (101 unit/integration + 2 compile-fail doctests), with formatting, all-target compilation, Clippy, debug build, and API documentation passing on Linux Rust 1.85.1.** The Bash bootstrap/check tasks also passed from outside the workspace.

Read the [historical content verification report](verification/content-report.md) for exact evidence and important provenance limits: dependencies were built from temporary upstream GitHub source snapshots because registry DNS was unavailable. Normal registry-artifact acceptance, Windows, and hosted CI remain unverified. No temporary source replacement ships in the project.

---

## Historical: World Runtime v0.1

The remainder is retained verbatim as the preceding snapshot's report, not current verification. Its toolchain, dependency, and test-count claims do not describe Content Loading v0.1.

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
