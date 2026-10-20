# World Runtime v0.1 — Two-Room Look/Move/Say

This increment provides **real, synchronous, in-memory command execution**. It is not a network server, login system, persistent game, or stable public SDK release. Package version remains the unpublished `0.1.0-dev.0` snapshot.

## Run the Reference Walkthrough

Use the existing workflow first:

```bash
bash scripts/dev.sh bootstrap
bash scripts/dev.sh check
```

Run the focused end-to-end suite or just its walkthrough:

```bash
cargo test -p worldwright-testkit --test runtime --locked --offline
cargo test -p worldwright-testkit --test runtime --locked --offline two_room_look_move_say_walkthrough -- --nocapture
```

PowerShell users can use `./scripts/dev.ps1 bootstrap` and `./scripts/dev.ps1 check`. The Linux run used Rust 1.85.0; exact pinned Rust 1.85.1 and Windows/hosted CI acceptance remain pending. See [VERIFICATION.md](VERIFICATION.md).

The walkthrough creates an Atrium and Observatory with explicit exits in both directions. Alice starts in the Atrium; Bob starts in the Observatory. Alice looks, moves to the Observatory, looks again, and speaks to both attached characters there. No files, sockets, credentials, database, or ambient random source are used.

No interactive terminal parser or new application subcommand is included. The server binary still refuses default service startup; its help now distinguishes the available local SDK runtime from the absent network service.

## Source Map

| File | Responsibility |
|---|---|
| `crates/worldwright-core/src/runtime/mod.rs` | Public runtime/result/snapshot types, limits, and typed errors |
| `crates/worldwright-core/src/runtime/state.rs` | Validated builder, private state, attachment management, actor-scoped queries |
| `crates/worldwright-core/src/runtime/execution.rs` | Command admission, staged effect validation/application, result release |
| `crates/worldwright-core/src/runtime/reference.rs` | Setting-neutral Look/Move/Say reference rules |
| `crates/worldwright-core/src/world.rs` | Read-only query trait and bounded room-description value |
| `crates/worldwright-sdk/src/lib.rs` | Re-exports of the runtime and description type |
| `crates/worldwright-testkit/src/two_room.rs` | Programmatic two-room fixture and local actor attachments |
| `crates/worldwright-testkit/tests/runtime.rs` | Entry point for the 24 end-to-end runtime tests |
| `crates/worldwright-testkit/tests/runtime_cases/` | Flow, policy, rollback, capacity, and helper tests |

All code still uses only Rust's standard library and local workspace dependencies. There is no new Cargo package or external dependency.

## Ownership and Construction

`WorldBuilder` accepts bounded room descriptions, directed exits, and characters with fixed account ownership and initial locations. It rejects duplicate IDs/edges, missing references, self-exits, empty worlds, and capacity violations. An exit is not automatically bidirectional.

`build` consumes the builder and a `RuleRegistry`, yielding a non-cloneable `WorldRuntime`. World state and the registry are private. There is no public arbitrary teleport, raw plan-application method, or rule replacement method on the live runtime.

A host can request an owned `WorldSnapshot` for inspection. It is detached from the runtime and cannot mutate the live world. Snapshot data and Debug output are **unfiltered administrative information**, not player-facing responses.

The current implementation clones bounded state for each command. That deliberately favors simple, testable atomicity over throughput. It is not a measured high-capacity design.

## Local Attachment Boundary

The trusted host calls `attach(account, character, connection)` after establishing its own authority to do so. Runtime checks the character's recorded owner and prevents one active connection from controlling two characters simultaneously. It does **not** log in or authenticate the supplied account.

Every successful attachment receives a fresh, monotonically increasing world-wide generation. Reattachment replaces the prior attachment. Detach requires the exact current context, so an old disconnect cannot detach a replacement session. Failed attachment/detachment checks leave state unchanged.

Execution requires the envelope's world and the complete account/character/connection/generation tuple to match current authoritative state. There is no general capability model or administrator bypass in this increment.

These counters are local to one runtime instance. The host must discard attachments when that instance is destroyed. Restart-safe session epochs, authentication, cross-process identity, writer fencing, and durable session invalidation remain future work. An ID or context object is not an unforgeable security token.

## Execution and Atomicity

`WorldRuntime::execute` performs:

1. Validate world, character ownership, and the exact current attachment.
2. Reject time earlier than the last successful execution.
3. For Move, verify the requested destination exists and has a directed exit from the current room.
4. Evaluate the registered rule with an actor-scoped read-only view and supplied time/randomness.
5. Recheck the attachment, then create a staged copy of world state.
6. Validate and apply each effect against that staged state, collecting events and resolved messages privately.
7. Reject a Move plan that did not move the actor exactly once.
8. Set the new revision/time, replace live state, and return the result.

**A normal `Err` leaves owned world state, revision, and last-successful time unchanged and returns no events or messages.** A late failure rolls back earlier staged movement and discards earlier buffered messages. Successful execution, attachment, and detachment increment a checked revision counter.

This is not a database transaction. Caller-owned RNG consumption and rule-owned state are not rolled back. Rust rules remain trusted native code and can perform ambient effects or panic if implemented incorrectly. No sandbox, panic recovery, out-of-memory recovery, or cross-resource transaction is promised.

Operation IDs are correlation fields only. Repeating an ID does not suppress a command or return a cached result. There is no idempotency receipt store.

## Supported Command and Effect Policy

| Command/effect | Current behavior |
|---|---|
| Look | Returns current room title and description as two private, bounded messages |
| Move | Must move the issuing character exactly once to the envelope's requested, reachable destination |
| Say | Reference rule emits the supplied text to attached characters in the actor's current room, including the actor |
| Direct message effect | May address only the issuing character |
| Room message effect | Must address the actor's staged current room; Look may not broadcast |
| Item-transfer effect | Rejected as `UnsupportedEffect`; never silently ignored |
| Scheduling effect | Rejected as `UnsupportedEffect`; the standalone scheduler is not wired into execution |

Rules cannot move a different character, substitute another destination, add movement to Look/Say, or move twice in one command. Room existence is not movement authorization: only the declared directed edge permits movement. Doors, locks, room-capacity gameplay rules, and richer permissions are not implemented.

`WorldView` now has conservative default methods for room descriptions and traversal, preserving older implementations. Runtime supplies a scoped implementation: the actor's location and description are visible; reachable room IDs may be queried; other character locations, distant rooms, adjacent-room descriptions, and items are hidden. No mutable state or database handle is exposed to a rule.

## Events, Messages, and Limits

A successful `ExecutionResult` includes the operation ID, world ID, in-memory revision, domain events, and addressed messages. Movement events describe actual staged movement that was accepted. Constructing these public data types elsewhere does not itself prove execution.

Each message includes the executor-selected source character and exact recipient attachment. Recipients are resolved **at that effect's position in the staged plan**, in deterministic character-ID order. Thus a message before a move can reach the original room, and one after it reaches the destination room. Detached characters receive no addressed messages.

A future transport must revalidate recipient connection/generation before sending a delayed result. Returned messages are not delivery acknowledgments. Text is not HTML-escaped or a complete Unicode safety policy; future presentation adapters must render it safely.

| Bound | Value |
|---|---:|
| Rooms per constructed world | 1024 |
| Characters per world | 256 |
| Directed exits per room | 64 |
| Effects per plan | 128, inherited from `EffectPlan` |
| Addressed message deliveries per command | 1024 |
| Title, description, or message text | 4096 UTF-8 bytes each, inherited from `PlayerText` |

A delivery-limit failure discards the entire command. Room title and description are emitted separately, so two individually valid maximum-length fields do not require an oversized concatenated message.

## Acceptance Coverage

The 24 new tests cover the reference walkthrough, room-only speech, detached recipients, staged audience resolution, detached snapshots, deterministic replay of fixtures, repeated operation-ID behavior, forged contexts, stale reconnect/disconnect cases, ownership and connection collisions, directed/unreachable movement, malformed movement plans, unauthorized audiences, filtered rule views, late-effect rollback, unsupported effects, missing/rejecting rules, clock behavior, fan-out limits, builder validation/capacities, and maximum-length descriptions.

The full workspace now has 72 passing tests on the recorded Linux toolchain: 71 unit/integration tests plus one compile-fail doctest. These are not login, network, database, or production-recovery tests.

## Next Work

Add a small, versioned content schema and loader that produces a validated `WorldBuilder`, keeping descriptive content separate from mutable runtime state. After that, add a local interactive adapter and then authenticated transport. Durable persistence must preserve the execution/validation boundary rather than turning a returned in-memory result into a misleading durable receipt.
