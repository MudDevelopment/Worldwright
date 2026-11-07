# Milestone 8 verification report

Evidence date: 2026-11-07. Uncommitted working tree based on `66dc092`,
Windows x86_64, Rust/Cargo 1.85.1.

The full `./scripts/dev.ps1 check` gate passed formatting, all-target checks,
Clippy with warnings denied, tests, build, and rustdoc: **263 test-binary tests
passed, 0 failed, 19 PostgreSQL tests were ignored/compile-checked, and 2
doctests passed separately**.

Focused evidence includes:

- Expedition rule, progression, rollback, takeover, and delayed recovery tests.
- Generic protocol action bounds and authenticated owner routing.
- Core negative tests for foreign effects, duplicate setup, time regression,
  and retryable failed due work.
- A real Testkit expedition scenario using committed JSON content.
- Fresh minimal/expedition project generation followed by `cargo test`.
- Migration dry-run, byte-for-byte backup, canonical output, and overwrite
  refusal.
- SDK compatibility/deprecation inventory and 100-client workload checks.
- The measured release-profile result in `milestone-8-performance.md`.

Unrun evidence: live PostgreSQL expedition behavior (explicitly unsupported),
Linux/hosted CI, deployment, browser matrix/manual accessibility, network load,
CPU/memory/queue instrumentation, long-duration retention, and production
capacity.
