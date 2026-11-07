# Milestone 5 Summary

> Historical close-out captured before Milestone 6. Its production-activation
> denial was superseded by the authenticated M6 auth-v1 gate; the M5 evidence
> counts below remain historical rather than current workspace totals.

## Status

Milestone 5 - Content Release and Preview v0.1 is implemented awaiting live
PostgreSQL verification. Artifact/package/inspect/preview and non-DB paths are
Windows-locally verified. Eight Milestone 5 database tests compile but have not
run. Milestone 4 - Durable Gameplay v0.1 has the same status, with eight
Milestone 4 database tests compiled but not run.

Current evidence on 2026-11-06 is from an uncommitted working tree based on
branch HEAD `9558dd5`, Rust/Cargo 1.85.1 x86_64-pc-windows-msvc:

- 200 test-binary tests passed, 0 failed, 16 ignored.
- 2 doctests passed separately.
- `./scripts/dev.ps1 check`, locked offline workspace tests, persistence
  ignored-test no-run, and `git diff --check` passed.
- No PostgreSQL, server/manual browser, hosted CI, Linux, deployment, or
  production-host test ran.

## Implemented slice

Milestone 5 provides deterministic `.wwr` packaging, strict manifest and
payload verification, immutable create-new output, inspect text/JSON, isolated
in-memory preview with bounded capacity/lifetime, and offline staging
publication/review/activation/recovery/rollback persistence workflows.
Production activation remains refused in code.

The gate remains pending live PostgreSQL evidence. Milestone 6 is planned as
the next dependency-ordered increment and is blocked on live Milestone 4/5
PostgreSQL evidence and owner decisions.
