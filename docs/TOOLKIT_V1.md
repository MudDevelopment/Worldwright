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

SDK API, protocol, content schema, and expedition game-contract versions are
currently 1. Package version `0.1.0-dev.0` is a separate identity, not API version
1 or a released previous minor. Support inventories advertise only implemented
versions; adding a version requires reviewed dispatch and compatibility evidence.
See `P2_ACCEPTANCE.md` and `verification/p2-implementation-ledger.md`.

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

Core/protocol mechanics retain generic game actions. Server composition selects
the module and initializes game state; rules own resource/progression policy.
The browser adapter supports `action <namespaced-id>` plus expedition-friendly
aliases without owning game state. See `EXPEDITION_PLAYABLE.md`.

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
Current PostgreSQL mode supports artifact-selected expedition counters,
schedules, scoped receipts and reconstruction. P1 also supplies fenced independent
idle-timer commits; the blocking terminal adapter still advances on commands.
These source paths are not live PostgreSQL/provider/recovery acceptance evidence.
Historical M8 local-only results remain valid for their original increment.

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
New projects include a bounded verified private SDK under `sdk/`, with relative
dependencies and playable embedded-content entry points for both templates.
Automated regressions move both projects before native compilation and execute
the binaries outside their project directories. Installed CLIs may use a trusted
matching `WORLDWRIGHT_SDK_BUNDLE`; a verified hash manifest is not publisher
authentication or a registry dependency cache. Native acceptance remains pending.

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

The harness measures sequential in-process runtime-backed Look commands after
attaching 100 actors. New JSON reports are schema 2 and identify
`runtime-look-nearest-rank-v2`, recipe version 2 and percentile recipe
`nearest-rank-integer-v1`. Existing latency/count field names and integer
nanosecond units are retained. Percentiles now select index `ceil(N*percent/100)-1`
from sorted samples, using the shared overflow-safe core utility. Historical v1
reports select `ceil((N-1)*percent/100)` and must not be silently relabeled or
compared as an optimization. The beta-250 network recipe already used nearest
rank; its schema, dimensions and thresholds are unchanged. The harness records
p50/p95/p99/max latency, elapsed time and delivered messages. It does not measure sockets, heartbeats,
queue occupancy, CPU, memory, PostgreSQL, slow consumers, reward contention,
outbox recovery, activation, or long-running retention. See
`docs/verification/milestone-8-performance.md`.

## P2 compatibility and drift guard

`worldwright compatibility --json` serializes the real SDK matrix, including
SDK/API, protocol and content inventories; it adds schema version 1 and
`sdk_package_version` without removing prior fields. Content loader dispatch,
validator and validated inventory share `CONTENT_SCHEMA_VERSION`. SDK bundle
package version derives from `SDK_PACKAGE_VERSION`; toolchain metadata derives
from the SDK matrix. Python source/TOML checks guard remaining registry literals.

The SDK does not link transport: a CLI **dev-only** protocol dependency compares
its declared protocol inventory with actual protocol exports. The new local
Cargo.lock edge introduces no registry upgrade. SDK API-v1 retained source and
its LF-normalized SHA-256 live under `apps/worldwright/tests/fixtures/sdk-api-v1`.
A native regression creates a real private bundle, moves it, creates an independent
consumer, bootstraps a consumer lock offline, then reruns with `--locked` and
verifies the bundle remains unchanged. It does not fabricate a released previous
SDK minor, ABI compatibility, signing, or a historical binary.

`python scripts/test-p2-contracts.py` and `python scripts/test-p2-negatives.py`
need Python 3.11+ and standard-library modules only. Negative cases first demand
a passing baseline and mutate bounded source copies only. Source parser changes
fail closed for review; these checks are not a Rust compiler. Retained consumer updates need explicit version/inventory review,
not silent rewriting to make an upgrade pass.

## Security and support limits

Native Rust modules are trusted process code, not a sandbox. Generated
projects use local path dependencies and are development fixtures, not
published registry packages. Protocol v1's additive command does not create a
general compatibility promise beyond the documented matrix. No Linux/hosted
CI, public deployment, live PostgreSQL, or production capacity claim is part
of this M8 evidence.
