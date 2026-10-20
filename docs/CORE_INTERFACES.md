# Core Interfaces — Current Implementation

The core now includes reusable interfaces and a real **in-process Look/Move/Say runtime**. JSON content loading, Local Play, and authenticated Transport v0.1 are implemented in their owning crates/applications; this core remains independent of the network gateway. [RUNTIME.md](RUNTIME.md) documents execution, attachment validation, atomicity, routing, and limits. Persistence and durable reconciliation remain future work in [DESIGN.md](../DESIGN.md).

## Module Map

| Core module | Present behavior |
|---|---|
| `ids` | Distinct account, character, world, room, item, connection, and operation IDs; validated definition IDs |
| `command` | Bounded text, nonzero connection generations, actor context, Look/Move/Say intents and envelopes |
| `effect` | Typed movement, transfer, message, and scheduling intentions; bounded plans; completed-event DTOs |
| `world` | Read-only queries with conservative description/traversal defaults; service-start `WorldStub` still refuses |
| `runtime` | Private world builder/state, local attachments, atomic Look/Move/Say execution, scoped views, and addressed output |
| `rule` | Object-safe rule trait; duplicate-safe registry; same-world guard and rule dispatch to produce a plan |
| `game` | Display metadata and fallible module registration; default registration explicitly refuses |
| `time` | Logical ticks, checked arithmetic, clock and gameplay-randomness interfaces |
| `scheduler` | Capacity-limited, stable-order in-memory delayed-work queue |
| `error` | Existing explicit `NotImplemented` error for missing integrations |

Core has no external crates and no dependency on SDK, protocol, persistence, applications, or Testkit. SDK re-exports the modules and public game/world interfaces. Testkit supplies deterministic fixtures and depends inward on these contracts.

## IDs and Input

Instance IDs wrap nonzero 128-bit integers. Each category is a different Rust type. Constructors validate representation only; callers remain responsible for allocation and uniqueness. An ID is not a capability, proof of ownership, or authentication token.

The provisional text parser accepts exactly 32 ASCII hexadecimal digits, including uppercase. Display emits lowercase, zero-padded hexadecimal. These are internal conveniences, **not a finalized wire/database format**. A compile-fail doctest verifies that a room ID cannot be assigned to a character ID.

`DefinitionId` accepts a namespace followed by a colon and dot-separated path, such as `frontier:rooms.arrival`. Each segment begins with a lowercase ASCII letter and continues with lowercase ASCII letters, digits, underscores, or hyphens. Total input length is at most 192 bytes. Empty segments, additional colons, slash paths, and uppercase names are rejected. This does not load or validate a content release.

`PlayerText` preserves Unicode and limits text to 4096 UTF-8 bytes. Empty/whitespace-only input and Unicode control characters are rejected; tabs, newlines, and ESC therefore fail. This is a single-line value, not a complete chat policy. It does **not** escape HTML, filter all Unicode formatting characters, or make text safe to render directly.

Connection generations must be nonzero. `ActorContext` and `CommandEnvelope` are ordinary application-supplied data, not proof of account login. The local runtime checks world, recorded character ownership, and the current connection/generation before evaluating a command. A trusted host establishes attachments; remote authentication and restart-safe session epochs remain future work.

## Queries, Rules, and Plans

`WorldView` exposes world identity, room existence, character location, and item location. It does not expose mutation or a database connection. Implementations must supply a coherent, appropriately filtered view; the trait cannot enforce those policies by itself.

`Rule::evaluate` receives a read-only context, a typed intent, and explicit mutable gameplay randomness. It returns an `EffectPlan` or `RuleError`. Native Rust rules remain trusted code: trait bounds do not prevent blocking, ambient I/O, unbounded work, or side effects. Review and future executor controls remain necessary.

`RuleRegistry` stores one rule per command kind and rejects replacement on duplicate registration. `plan` checks the envelope's world against the view, reports an unregistered command, and invokes the selected rule. It **does not execute the command**, authenticate its actor, enforce game invariants, or commit output.

`GameModule::register_rules` can use ordinary `?` propagation for duplicate errors through `ModuleRegistrationError`. Metadata-only implementations retain a default that returns `NotImplemented`. Registration of several rules is not transactional: use a fresh registry and discard it if module registration fails.

An effect plan holds at most 128 intentions and refuses an extra append without changing the existing plan. Movement and item transfer fields are explicit; expected item location is a precondition for a future executor, not an enforced transfer today. Capacity, containment cycles, conservation of value, authorization, current revisions, and durability still require execution-time validation.

There is no public raw-plan `apply` method. `WorldRuntime::execute` validates its supported command/effect subset against staged state and releases events plus addressed messages only after in-memory replacement. It rejects transfers and scheduling effects. The plan-only registry remains an unvalidated lower-level interface. There are no durable receipts, persistent event stream, or actual network deliveries; constructing a DTO elsewhere proves none of these.

## Time and Scheduling

`Tick` is milliseconds on a caller-owned monotonic simulation timeline, not UTC. Arithmetic is checked. `Clock` supplies that timeline. `RandomSource` supplies gameplay values with visible exhaustion; it makes no cryptographic or range-distribution guarantees.

`Scheduler<T>`:

- Requires a positive slot capacity and allocates entries as needed.
- Orders by deadline, then a checked, monotonically increasing insertion sequence.
- Preserves FIFO order among equal deadlines.
- Returns an error when full or when its sequence is exhausted.
- Cancels by queue-local token without reusing that token in the same queue.
- Removes at most one due action per poll.
- Rejects backwards poll time without removing queued work.
- Accepts overdue inserts, returned by a subsequent nonregressing poll.

Tokens belong only to their originating queue; another queue can have the same sequence value. They are not global IDs or security capabilities. Slot capacity is **not** a byte/memory limit on arbitrary generic payloads. Application payload limits are still required. The queue is not persistent, concurrent, autonomous, or a timer thread; the future world owner decides when to poll and revalidate an action.

## Testkit Fixtures

- `FakeClock` supports checked advances and nonregressing set operations.
- `SequenceRandom` returns supplied values in order, then an explicit exhaustion error.
- `WorldFixture` supplies in-memory query data. Character placement requires an added room; item placement has an explicitly unchecked test helper to construct invalid fixtures.
- `ScenarioStub` continues to refuse execution rather than claiming a game simulation occurred.

Fixtures are unfiltered test data, not production views or durable storage. They are deliberately absent from the production SDK dependency graph.

## Compatibility and Next Work

This remains an unpublished `0.1.0-dev.0` snapshot, not a stable SDK release. Existing metadata-only `GameModule` implementations and application stub behavior are retained. No deployed schema or wire format is migrated by this increment.

The runtime, versioned content loader, Local Play adapter, and authenticated Transport v0.1 now exist; see [RUNTIME.md](RUNTIME.md), [LOCAL_PLAY.md](LOCAL_PLAY.md), and [PROTOCOL_V1.md](PROTOCOL_V1.md). Persistence and durable reconciliation are next. `two_room_world()` in Testkit builds the real local runtime programmatically. Keep execution authority out of network handlers and retain the distinction between in-memory success, durable commitment, and delivery.

See [development workflow](DEVELOPMENT.md) for checks and [verification](VERIFICATION.md) for measured results and remaining gaps.
