# Local Play v0.1 verification

## Scope and baseline

Local Play was implemented against the byte-verified `Worldwright-content-loader-v0.1.zip` baseline. This adds the local terminal host only; the content crate, core runtime, content schema, Cargo.lock, package version, and pinned toolchain are unchanged.

## Observed checks

A clean rebuild followed `cargo clean` before the final full workspace workflow. Bootstrap and check ran from outside the repository through the existing Bash script.

| Check | Result |
|---|---|
| Rust compiler/Cargo | 1.85.1 on x86_64 Linux |
| Bootstrap and locked dependency provisioning | Passed with the verification-only dependency cache described below |
| Formatting check | Passed |
| Workspace all-target compilation | Passed |
| Clippy, all targets/features, warnings denied | Passed |
| Unit/integration tests | 125 passed |
| Compile-fail doctests | 2 passed |
| Total | **127 passed; zero failures** |
| Debug workspace build | Passed |
| API documentation, warnings denied | Passed |
| Real Linux pseudo-terminal | Prompt displayed; go north executed; quit exited 0 |

Evidence:

- [Bootstrap log](local-play-bootstrap-linux-rust-1.85.1.log)
- [Full clean-build workflow log](local-play-linux-rust-1.85.1.log)
- [Linux pseudo-terminal transcript](local-play-terminal-smoke.log)

Local Play adds **24 tests** (13 application unit tests and 11 subprocess tests). The baseline's 103 tests remain passing, with CLI smoke assertions extended for play support.

Coverage includes initial view and entry room, complete walkthroughs, runtime state/revisions, nondecreasing time, checked operation-ID exhaustion, exact-attachment delivery, attempted reverse travel, deliberately corrupted lookup metadata, quit/EOF/final lines, invalid command syntax, LF/CRLF, blank lines, oversized input draining, invalid UTF-8, controls, complete 4096-byte Unicode speech, working-directory/path handling, cleanup on I/O failure, and terminal versus piped prompts.

## Toolchain/dependency provenance

The same temporary verification environment as the content milestone was used:

- `rustc 1.85.1 (4eb161250 2025-03-15)` and `cargo 1.85.1 (d73d2caf9 2024-12-31)`.
- Compiler, Cargo, standard library, Clippy and rustfmt were provisioned from third-party `@rustbin/*-1.85.1-x86_64-unknown-linux-gnu` npm packages with install scripts disabled. Binary signatures were not independently verified.
- crates.io DNS was unavailable during environment provisioning. Dependencies come from temporary matching upstream GitHub source snapshots, not downloaded registry `.crate` artifacts. The detailed dependency versions and normalization are recorded in the [content report](content-report.md).
- Cargo.lock remains byte-identical to the content milestone and retains normal registry package identities/checksums. No new dependencies or registry patches were added.
- Temporary toolchains, source snapshots, Cargo source overrides and build outputs are not part of the source ZIP. Logs retain the actual temporary build paths.

## Remaining verification limits

Windows/PowerShell, hosted CI, approved-toolchain signatures, normal registry-artifact acceptance, release builds, performance, and long-running production behavior were not verified. Run the supplied bootstrap/check workflow with approved tools and registry artifacts before release acceptance.

The PTY test exercises Linux terminal prompt, movement and quit, not every terminal emulator or OS. Ctrl-C handling is unchanged OS behavior, not graceful detach. There is no authentication, network protocol, persistence, or durability claim. An output failure after successful execution does not roll back the in-memory action.
