# Toolkit maturity contract

Worldwright's M8 toolkit baseline is source-level Rust integration. It does not
promise a stable Rust ABI, dynamic plug-ins, untrusted native-code isolation, or
production support.

## Selected contracts

- Second game: `worldwright-expedition`, a turn-based resource and encounter
  game rather than another set of reference-room descriptions.
- Compatibility: the current and immediately previous `0.x` SDK minor are the
  support window once a previous minor exists. A public item is deprecated for
  one supported minor before removal, except for urgent security/correctness
  removals.
- Performance objective: a reproducible 100-client development workload. It is
  evidence, not a beta capacity or service-level promise.
- Toolchain: Rust 1.85.1, edition 2024. `worldwright_sdk::COMPATIBILITY` and
  `worldwright compatibility --json` expose the current matrix.

The initial SDK API, protocol, content schema, and expedition game-contract
versions are all 1. There is no fictional previous release: the support lists
contain only version 1 until version 2 exists.

## Reusable game seam

Core adds a bounded namespaced `GameAction`, per-character counters, atomic
counter effects, and delayed counter adjustments. Rules still receive a
read-only actor-scoped view and return an uncommitted plan. The single owner
validates the exact attachment, stages every effect, and either commits the
whole command or none of it. Scheduled batches are also staged; a failing due
adjustment remains queued and retryable.

Protocol v1 adds an optional `game_action` command with a validated
`DefinitionId` and optional bounded `PlayerText`. It adds no game names or
game-specific output shape. Existing clients need not send it and continue to
receive the existing output/result envelopes. An old server safely rejects the
unknown command.

The server selects `reference` or `expedition` only at its composition root:

```text
worldwright-server serve ... --game expedition
```

Core, protocol framing, authentication, attachment generations, bounded owner
queues, slow-client handling, and browser text rendering contain no expedition
branch. The browser command is `action <namespaced-id>`.

## Expedition proof

`worldwright-expedition` independently registers Look, Move, Say, and
GameAction rules. Its original three-location content is under
`examples/expedition/`.

- Supplies have capacity 9. Travel and strikes each spend one supply.
- `expedition:actions.forage` gains a deterministic injected-random 1-3 supply.
- `expedition:actions.encounter` starts threat 2-4.
- `expedition:actions.strike` reduces threat; resolution increments victories.
- `expedition:actions.rest` and encounter resolution schedule recovery.
- Recovery uses a pending counter so forage cannot violate capacity before the
  delayed adjustment runs.
- Rank is Scout (0-2 victories), Pathfinder (3-5), or Trailblazer (6+).

The server uses process-seeded gameplay randomness only for the expedition
profile. The reference profile retains visibly unavailable randomness.
Expedition state is intentionally local-only: PostgreSQL startup with
`--game expedition` refuses before connecting because durable game counters,
scheduled jobs, receipts, and restart reconstruction are not implemented.
This is a documented game-package gap, not silent data loss.

## Testkit

`RuntimeScenario` replaces the refusing `ScenarioStub`. It executes at most
1,024 typed steps through the real `WorldRuntime`, creates bounded operation
identities, and reports the failing step with its `WorldError`.
`ScenarioStub` remains a deprecated alias until the earliest 0.3 removal.

`expedition_world()` loads the committed expedition JSON, initializes two
authorized characters, registers the real module, and supports deterministic
encounter/recovery scenarios with `SequenceRandom`.

## Project generation

```text
worldwright project new <path> --name <matching-directory> \
  --template <minimal|expedition> [--dry-run]
```

Both templates create aligned directory/package/binary names, `src/main.rs`, a
validated `content/world.json`, a real integration test, README, and gitignore.
They contain no inferred license grant. Existing destinations are refused.
Automated acceptance generates both templates in fresh directories and runs
their tests against the pinned host target.

## Content migration

```text
worldwright content migrate <input> --output <new-path> \
  [--backup <create-new-path>] [--dry-run]
```

Only schema v1 exists, so this tool performs strict validation and canonical v1
rewriting; it does not invent a v2 conversion. Dry-run writes nothing. Output
and optional byte-for-byte source backup use create-new semantics and never
overwrite reviewed bytes. A future schema conversion must add an explicit
source/target pair and round-trip fixtures.

## Performance

```text
worldwright benchmark --clients 100 --commands-per-client 100 --json
```

The current harness measures sequential in-process runtime-backed Look
commands after attaching 100 actors. It records p50/p95/p99/max command latency,
elapsed time, and delivered messages. It does not measure sockets, heartbeats,
queue occupancy, CPU, memory, PostgreSQL, slow consumers, reward contention,
outbox recovery, activation, or long-running retention. See
`docs/verification/milestone-8-performance.md`.

## Security and support limits

Native Rust modules are trusted process code, not a sandbox. Generated
projects use local path dependencies and are development fixtures, not
published registry packages. Protocol v1's additive command does not create a
general compatibility promise beyond the documented matrix. No Linux/hosted
CI, public deployment, live PostgreSQL, or production capacity claim is part
of this M8 evidence.
