# Milestone 5 Verification Report

**Evidence date:** 2026-11-06  
**Tree:** uncommitted working tree based on branch HEAD `9558dd5`  
**Platform:** Windows 10.0.26200 / Windows 11 Enterprise, x86_64-pc-windows-msvc  
**Rust/Cargo:** 1.85.1

## Current gate

Milestone 4 - Durable Gameplay v0.1 and Milestone 5 - Content Release and
Preview v0.1 are implemented awaiting live PostgreSQL verification. No
PostgreSQL server, manual browser session, hosted CI, Linux, deployment, or
production-host test ran. Eight Milestone 4 database tests and eight Milestone
5 database tests compile but have not run.

## Commands

Passed:

```text
./scripts/dev.ps1 check
cargo test --workspace --all-targets --locked --offline
cargo test -p worldwright-persistence --no-run --locked --offline -- --ignored
git diff --check
```

The workspace gate reports **200 test-binary tests passed, 0 failed, 16
ignored**. The 16 ignored tests are the eight Milestone 4 persistence tests
plus eight Milestone 5 release/publication tests. They were compiled, not
executed.

## Per-package evidence

| Package | Key test-binary result |
|---|---|
| `worldwright` | 43 passed across unit/content/play/release/smoke binaries |
| `worldwright-server` | 40 passed across library/main/smoke binaries |
| `worldwright-persistence` | 6 passed, 16 ignored |
| `worldwright-release` | 11 passed |
| workspace total | 200 passed, 0 failed, 16 ignored |

Other workspace binaries account for the remaining current total: console 5,
content 25, core 15, protocol 16, SDK 1, studio 5, and testkit 33.

Doctests are separate and are not included in the 200 test-binary total:
`worldwright-content` 1 passed and `worldwright-core` 1 passed; all other
workspace doctest groups had 0 tests. Thus **2 doctests passed separately**.

## What is locally verified

The `.wwr` bounded non-archive container, canonical manifest, exact payload
preservation, deterministic packaging, digest checks, immutable writer,
inspection JSON, preview timeout/tamper/environment tests, four-slot capacity,
stale retry, token ownership cleanup, release parser, and non-DB activation
smoke paths are Windows-locally verified.

The release smoke path proves Look -> Move -> Look and rejects a valid single
room with no destination. The durable server uses the exact active artifact and
read guards, but no live PostgreSQL execution proves the database lock,
transaction, fencing, or restart behavior.

## Limits of evidence

The guarded database tests were run with `--no-run -- --ignored` only. No
PostgreSQL query, migration, constraint, lock, activation, recovery, rollback,
or durable server startup claim is runtime evidence. The current gate therefore
does not call either Milestone 4 or Milestone 5 complete or locally verified.
