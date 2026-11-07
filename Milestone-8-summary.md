# Milestone 8 summary

Milestone 8 establishes a source-level toolkit maturity baseline around an
independent expedition game.

## Delivered

- Game-neutral namespaced actions, actor counters, transactional delayed
  adjustments, monotonic logical-time checks, and retryable due-work failure.
- `worldwright-expedition` with resource capacity, travel cost, random forage
  and encounters, turn resolution, delayed recovery, and rank progression.
- Generic protocol-v1 game actions routed through the existing authenticated,
  bounded single-owner server; composition selects reference or expedition.
- Explicit refusal of PostgreSQL expedition mode until game state has durable
  transactions, receipts, scheduling, and reconstruction.
- SDK v1 compatibility matrix, current-plus-previous-minor policy, and
  deprecation inventory.
- Real bounded Testkit scenarios and a committed expedition fixture.
- Minimal and expedition project templates that are generated, compiled, and
  tested from clean directories without overwriting existing paths.
- Canonical schema-v1 migration with dry-run, optional create-new backup, and
  create-new output.
- Reproducible 100-client release-profile runtime benchmark with tail latency.

## Evidence boundary

The Windows `dev.ps1 check` passed with **263 test-binary tests, 0 failures,
19 ignored PostgreSQL tests compile-checked, and 2 doctests passed**. The
separate performance report is the only workload evidence.

The independent game proves core, protocol, authentication, attachment,
single-owner, browser, content, and test seams without game-name branches in
core or protocol. It does not yet prove durable expedition state, shared
release-manifest game identity, PostgreSQL restart/retry, or production
operations for that game. The performance run is in-process Look traffic, not
a network/database capacity result.

See `docs/TOOLKIT_V1.md`,
`docs/verification/milestone-8-performance.md`, and
`docs/verification/milestone-8-report.md`.
