# Worldwright — Design

**Build worlds. Write the rules.**

| Attribute | Value |
|---|---|
| Document version | 0.1 — proposed baseline |
| Prepared | October 20, 2026 |
| Product | Rust development suite for MUDs: multi-user domains |
| Audience | Engine and tooling maintainers, game developers, designers, QA, security, and operators |
| Implementation status | Local runtime, JSON content loading/validation, and single-player terminal play implemented; see verification for evidence and provenance limits; network authentication, persistence, and hosted CI remain unverified |
| Companion documents | [README.md](README.md) · [ARCHITECTURE.md](ARCHITECTURE.md) |

> **Scope and authority:** This document describes the target system; “Must” is a requirement, not evidence of implementation. [RUNTIME.md](docs/RUNTIME.md) defines the implemented single-owner local execution subset: validated construction, current attachment checks, scoped rule queries, staged atomic effects, and addressed messages. This is not login authentication, database commitment, exactly-once delivery, or deployed capacity. [CORE_INTERFACES.md](docs/CORE_INTERFACES.md) and [VERIFICATION.md](docs/VERIFICATION.md) describe the supporting contracts and successful Linux checks. The broader network/persistence/recovery design remains future work.
>
> **Document boundary:** The README explains the product. This file explains the decisions and behavioral contracts. ARCHITECTURE maps those decisions to components, processes, interfaces, and deployment. [SECURITY.md](SECURITY.md), [CONTRIBUTING.md](CONTRIBUTING.md), and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) provide proposed policies, pending operational adoption; [DEFINITIONS.md](DEFINITIONS.md) explains MU* terminology and compatibility limits. Detailed operating runbooks remain future work.

## Contents

- [Purpose and Scope](#purpose-and-scope)
- [Design Principles](#design-principles)
- [Decision Register](#decision-register)
- [Product Boundaries](#product-boundaries)
- [Domain Model](#domain-model)
- [Command and Event Model](#command-and-event-model)
- [Simulation and Scheduling](#simulation-and-scheduling)
- [Persistence and Recovery](#persistence-and-recovery)
- [Game SDK and Extensions](#game-sdk-and-extensions)
- [Content Lifecycle](#content-lifecycle)
- [Protocol and Player Experience](#protocol-and-player-experience)
- [Identity and Authorization](#identity-and-authorization)
- [Tool Contracts](#tool-contracts)
- [Configuration and Compatibility](#configuration-and-compatibility)
- [Failure and Overload Policy](#failure-and-overload-policy)
- [Verification Strategy](#verification-strategy)
- [Delivery Gates](#delivery-gates)
- [Open Decisions](#open-decisions)
- [Maintenance and Licensing](#maintenance-and-licensing)

## Purpose and Scope

Worldwright provides reusable infrastructure for creating, testing, deploying, and operating persistent multiplayer text worlds. It is not a particular fantasy setting, science-fiction universe, or mandatory combat ruleset.

The first implementation should prove a small game and an end-to-end development workflow. A second, meaningfully different game then tests whether the abstractions are genuinely reusable.

### Initial goals

- Authoritative multiplayer rooms, movement, observation, communication, and interactions.
- Durable characters, valuable item ownership, and game-defined progression.
- A browser client with secure interactive transport and accessible text output.
- Validated, immutable content releases with controlled activation.
- Rust extension contracts that do not expose unrestricted world mutation.
- Repeatable scenario and failure tests.
- Bounded resource use, auditable administration, and recoverable releases.
- A small organization able to understand and maintain the entire system.

### Initial non-goals

- A distributed active-active world or seamless cross-region simulation.
- Arbitrary untrusted native plugins or a public scripting marketplace.
- A universal visual editor before file-based content workflows work.
- Real-money trading, payments, or guaranteed legacy MUD-client support.
- Full event sourcing or historical replay of every production action.
- A hosted, multi-tenant platform serving mutually untrusted game operators.

Individual games may add capabilities later. Such additions do not automatically become supported toolkit contracts.

## Design Principles

1. **Intent is untrusted; effects are authoritative.** Clients describe desired actions. The server determines identity, permissions, applicable rules, and results.
2. **State has an owner.** One runtime task owns mutable simulation state in the initial deployment. Connections and tools submit messages rather than taking mutable references to the world.
3. **Persistence precedes durable success.** Valuable-state changes are acknowledged only after an authoritative database commit is established and the runtime can present the committed result.
4. **Mechanism is reusable; policy belongs to games.** The engine owns execution and persistence mechanisms. Games own lore, balance, death rules, and progression semantics.
5. **Content is released, not casually edited in production.** Validation, review, compatibility checks, and recovery are part of publication.
6. **Bounded work is a correctness requirement.** Messages, queues, expensive tasks, and retries have limits and defined overload responses.
7. **Public contracts require evidence.** Stabilize interfaces after working use cases and tests, not before.
8. **Operations and player safety are product features.** A release needs recovery and moderation capabilities, not merely a running process.

## Decision Register

All entries below are **proposed**, awaiting review and implementation evidence. These identifiers can be used in issues and future architecture decision records.

| ID | Proposed decision | Rationale | Consequence / alternative |
|---|---|---|---|
| WW-D01 | Modular monolith; one world per server process initially | Simpler ownership, debugging, and deployment | One process is a world availability boundary; distributed zones deferred |
| WW-D02 | One authoritative simulation owner | Deterministic ordering and fewer shared-state races | Long callbacks or database waits affect world latency |
| WW-D03 | PostgreSQL for durable operational state | Transactions, constraints, established backup tooling | Requires database operations discipline; not an embedded-only runtime |
| WW-D04 | Commit-before-success for valuable changes | Prevent acknowledged rewards or transfers from existing only in RAM | Adds persistence latency; batching cannot weaken guarantees silently |
| WW-D05 | Browser-first HTTPS and secure WebSockets | Accessible deployment and a controllable client contract | Traditional-client gateway becomes a separate adapter later |
| WW-D06 | JSON content definitions; TOML server configuration | Clear structured content validation and readable operator settings | Room content schema v1 implemented; server configuration remains proposed |
| WW-D07 | Compile-time Rust game extensions | Type checking and ordinary build/test tooling | Extensions are trusted code; no stable dynamic Rust ABI |
| WW-D08 | Pure core with explicit effects and adapter boundaries | Testable rules without network or database dependencies | The composition root must translate and execute effect plans |
| WW-D09 | Immutable content artifacts; maintenance activation initially | Avoid complex live-reference migration in the first release | Content updates initially interrupt world service |
| WW-D10 | Bounded ingress, output, scheduling, and expensive work | Prevent slow clients or expensive requests from exhausting the process | Admission rejection and disconnect policies must be player-visible |
| WW-D11 | Server-side revocable sessions; separate privileged capabilities | Explicit identity lifecycle and least privilege | Session storage and revocation become operational dependencies |
| WW-D12 | Transactional administrative audit; no direct tool writes to game tables | Preserve game invariants and attributable changes | Emergency repair is a separate reviewed procedure |
| WW-D13 | Materialized durable state, not full event sourcing | Smaller initial implementation and easier operational inspection | Historical reconstruction requires purpose-built evidence, not assumptions |
| WW-D14 | One public browser origin initially | Reduces browser integration and cookie complexity | Staff routes still need strong authorization and exposure controls |
| WW-D15 | Fail closed on unresolved durable state | Avoid mutation based on an uncertain transaction outcome | Recovery can interrupt gameplay rather than guessing |

A later decision should record what changed, alternatives considered, migration impact, and verification. Do not silently overwrite the rationale behind a deployed contract.

## Product Boundaries

| Layer | Owns | Must not assume |
|---|---|---|
| Engine | World execution, entity identity, scheduling, invariant enforcement | Fantasy classes, fixed damage types, mandatory levels, setting-specific economy |
| SDK | Supported registration points, typed contexts, commands, effects, queries | Access to a database connection or mutable global world |
| Individual game | Rules, content, balance, presentation vocabulary, game extensions | Permission to bypass engine ownership or transaction rules |
| Studio | Content authoring, validation, preview, publication requests | Permission to edit live inventories or production database rows |
| CLI | Local project operations and validation | Production authority simply because it runs on a developer's machine |
| Testkit | Controlled clocks, randomness, fixtures, failure injection, assertions | Production replay without the required recorded inputs |
| Console | Authorized operational and moderation workflows | Authority inferred from a hidden UI button or network location alone |

The first browser client may ship within the server package. Studio and Console may initially be server-rendered modules rather than independent services. Product names do not mandate separate processes.

## Domain Model

### Identity and entity types

The core now defines distinct nonzero ID newtypes and validated definition IDs; see [CORE_INTERFACES.md](docs/CORE_INTERFACES.md) for their provisional representation. These are not authentication capabilities or finalized wire/database schemas. Boundary parsing and execution-time authorization remain separate requirements.

| Concept | Meaning | Persistence expectation |
|---|---|---|
| Account | Login identity and account-level restrictions | Durable |
| Character | A player's persistent in-world identity | Durable |
| Session | Revocable authentication context | Server-side; survives/revokes according to security policy |
| Connection | One live transport attachment | Ephemeral |
| World | One authoritative game instance | Stable durable identity |
| Zone definition | Content grouping for navigation and authoring | Immutable release content |
| Room definition | Reusable place description and exits | Immutable release content |
| Room instance | An active room in a world or instance | Stable for persistent rooms; ephemeral instances require explicit policy |
| Item definition | Reusable item properties and game data | Immutable release content |
| Item instance | One ownable object, distinct from its definition | Durable when player-value-bearing |
| NPC definition / instance | Template versus active creature or actor | Definitions immutable; instance durability game-specific |
| Quest definition / progress | Rules versus a character's completion state | Progress and reward receipts durable |
| Content release | Identified immutable collection of definitions | Retained with hash and manifest |
| Operation receipt | Authoritative result of a retryable durable command | Durable within a documented replay-protection policy |

### Definition versus instance

A definition ID such as `frontier:rooms.arrival` belongs to a namespace and remains stable across compatible revisions. A live item instance has its own generated ID and references an item definition plus the relevant release or migration version.

Do not use display names as identity. Two players may encounter different instances of an object with the same name. Renaming a title must not silently create a new item or sever quest references.

Start with typed maps and explicit entity records. An entity-component-system framework is an option only if measured gameplay requirements justify it; an ECS is not necessary merely because this is a game engine.

### Location and containment

An ownable item has exactly one authoritative holder: a room, character inventory, container, or explicitly modeled system location. Represent this in the database using a constrained location/holder model; do not rely on several unrelated nullable columns with no exclusive-ownership constraint.

Required rules:

- Every persistent item has one valid holder.
- Transfers remove the previous ownership and establish the next within one transaction.
- Container membership is acyclic and has bounded nesting depth.
- Capacity and eligibility checks occur against the authoritative state used for the transfer.
- Destruction is an explicit transition, not an accidental missing parent.
- Quantity operations cannot underflow, overflow, or produce negative holdings.
- Stack split and merge preserve total quantity except for explicit mint or consume operations.

The exact relational layout should be selected in a schema implementation review. This document does not claim a completed SQL schema.

### Game policy separation

The engine supports scheduled actions, resources, effects, and transitions. A game supplies whether those mean combat rounds, oxygen depletion, magical wards, or diplomatic influence.

Reference-game defaults must be distinguishable from engine invariants. “An item has one owner” is an engine invariant. “Death costs ten percent of experience” is a game rule.

## Command and Event Model

### Processing pipeline

1. Enforce transport size and framing limits.
2. Establish the authenticated session and connection context.
3. Parse syntax into a typed intent.
4. Attach trusted account, character, world, and connection-generation information.
5. Admit the request through rate and queue limits.
6. Revalidate session generation, authorization, and game state when executing.
7. Resolve a game rule into a proposed effect plan.
8. Validate the plan against engine invariants.
9. Execute the required durable transaction, if any.
10. Apply the committed result to the in-memory projection.
11. Produce visibility-filtered domain events and client output.

Authentication at ingress is not sufficient by itself: a queued command may become unauthorized before execution.

### Command classes

| Class | Examples | Required behavior |
|---|---|---|
| Read-only | Look, help, inspect permitted state | No hidden mutation; apply visibility rules |
| Ephemeral | Local speech, transient emote | Bounded delivery; no implied durable transcript |
| Simulation transition | Move, begin an encounter, apply temporary effect | Ordered execution; explicitly defined persistence class |
| Durable value mutation | Transfer item, spend currency, claim reward | Transactional validation, commit, receipt, and recovery |
| Administrative mutation | Suspend account, relocate character, restore asset | Current capability check, reason, audit, and normal invariants |

Commands are data, not closures received from a client. Client input must never select arbitrary function names, SQL, shell commands, filesystem paths, or Rust symbols for execution.

### Resolution and ambiguity

Match names only within the command's visible scope. If `get key` matches multiple eligible items, return disambiguation rather than silently choosing an arbitrary database row.

Normalize command verbs consistently, but preserve ordinary Unicode in player text. Define byte and character limits separately. Reject forbidden controls and escape sequences without silently removing meaningful text.

### Effects and events

A rule produces an effect plan such as movement, ownership transfer, resource change, scheduled action, or a message intent. The engine validates the plan and controls when it becomes observable.

A domain event describes a completed transition. It is not proof that the event was delivered to a browser and is not automatically a durable event log.

Recipient selection and redaction occur server-side. Never broadcast private fields to every client and rely on the browser to hide them.

### Result states

Use stable machine-readable result categories with separate player-facing text:

- `invalid_input`, `not_authorized`, `not_found_or_not_visible`.
- `ambiguous_target`, `precondition_failed`, `conflict`.
- `rate_limited`, `overloaded`, `temporarily_unavailable`.
- `operation_pending`, `operation_succeeded`, `operation_rejected`.

An ingress acknowledgment only means accepted for processing. The durable result is separate. A timeout must not be represented as a definite failure when a transaction may already have committed.

## Simulation and Scheduling

### Initial ownership model

One task owns mutable state for one world. Async network tasks communicate through bounded message channels. Immutable content is shareable; mutable world maps are not exposed through a global lock to every subsystem.

Read requests that require a coherent world view are serialized through the owner or served from explicitly versioned snapshots. A diagnostic snapshot can be stale; a trading eligibility check cannot rely on it.

### Initial transaction strategy

For the first durable slice, the owner handles at most one durable world mutation at a time and waits for its transaction result before executing the next state-changing command. Socket I/O continues independently, subject to queue bounds.

This deliberately favors correctness over throughput:

- The owner does not optimistically publish uncommitted state.
- Conflicting effects cannot overtake the pending transaction inside the process.
- Database deadlines and queue limits bound the failure response.
- Long persistence stalls can pause the world; this is a known limitation to measure.

A future design may reserve affected aggregates and continue unrelated work while an operation is pending. That requires explicit conflict tracking, completion ordering, and crash recovery; it is not an implicit optimization allowed by this baseline.

### Scheduling

Use an injected monotonic clock for runtime deadlines and a priority queue ordered by deadline plus a stable tie-breaker. Use UTC timestamps for persisted business timestamps and operator records, not for measuring elapsed time within a process.

Every scheduled action includes an identity, target, generation/version precondition, and cancellation policy. A delayed attack against a destroyed or replaced target must become a defined no-op or rejection.

Process due work under a configured budget. Do not execute unlimited catch-up work after a long stall. The game defines whether missed periodic effects are coalesced, skipped, or reconciled from durable timestamps.

Durable deadlines such as an auction close would require a persistent scheduling model and recovery policy before that feature ships. They are not covered by merely keeping timers in memory.

### Randomness and determinism

Inject game randomness into rule evaluation. Tests use known seeds and recorded inputs. Account tokens and security identifiers use a cryptographically secure random source, separate from gameplay randomness.

Retrying a committed operation must return its original result, not reroll a reward. Store the committed random outcome with the durable transition or receipt. A crash before any commit may permit a new evaluation only if the game accepts that behavior and no success was exposed.

Deterministic test scenarios do not establish general production replay: release versions, ordering, external inputs, clocks, and random outcomes would also need to be captured.

## Persistence and Recovery

### State classes

| Class | Examples | Baseline contract |
|---|---|---|
| Durable correctness-critical | Ownership, currency, quest reward claims, progression | Commit before success; reconstruct from database |
| Durable security / control | Accounts, bans, roles, administrative audit, active release | Separate authorized services and transactions; never depend only on client state |
| Checkpointed gameplay | Position outside valuable transitions, optional noncritical status | A documented bounded-loss policy is required for each game |
| Ephemeral | Connection state, cosmetic messages, ordinary live timers | May be lost at restart; communicate recovery behavior |
| Derived | Metrics, online counts, search indexes, caches | Rebuildable; not authoritative for gameplay decisions |

For the first reference game, prefer persisting character location whenever it changes. Permit less durable checkpointing later only after its loss consequences are explicit. Any location change atomically coupled to a reward, cost, or irreversible transition belongs in that transaction.

### Database authority and the runtime projection

PostgreSQL is the source of truth for durable facts. Runtime memory is the execution projection of committed durable facts plus allowed ephemeral state.

Avoid a periodic “save the entire character” operation that can overwrite a newer item, currency, or moderation transaction. Persist scoped changes with expected versions or database locking as appropriate.

The baseline uses current-state tables, operation receipts, audit entries, and an outbox where required. It does not require replaying an event stream to reconstruct the whole world.

### Idempotent durable operations

Use a stable client request identifier for retryable mutations, scoped by trusted world and actor identity. Store:

- The scoped request key and server operation ID.
- A canonical intent fingerprint, including relevant targets and arguments.
- The authoritative committed or recorded rejected result.
- Commit timestamp, affected versions, and serialization format version.

A repeated key with the same intent returns the previous result after authorization checks. A repeated key with different intent is rejected. The key is never evidence of identity or entitlement.

Enforce independent business uniqueness as well. For example, a quest reward needs a claim key tied to character, quest, and repeatable-completion generation. A malicious client using new request IDs must not claim the same completion repeatedly.

Receipt retention is a launch decision. Expired receipt cleanup must not defeat independent ownership and business-uniqueness constraints. Clients must not blindly resubmit old pending requests under fresh IDs.

### Transaction procedure

For an ownership transfer or reward:

1. Lock and validate the world's writer epoch.
2. Find the scoped operation receipt; return it if the intent matches.
3. Lock affected persistent records in a documented order.
4. Revalidate ownership, balance, claim eligibility, and expected versions.
5. Apply all durable effects, including required quest/location/progression changes.
6. Record the receipt, required audit, and outbox records in the same transaction.
7. Commit using an explicitly reviewed database durability configuration.
8. Update the in-memory projection from the authoritative result.
9. Deliver the result to currently authorized recipients.

Do not claim universal exactly-once execution or delivery. The target is an at-most-once business effect under the documented idempotency and uniqueness rules, with retryable result retrieval.

### World writer fencing

Only one writer may mutate a world's durable gameplay state. Application deployment conventions alone are not a sufficient safeguard against two server processes starting accidentally.

Proposed baseline:

- A persistent world-control record carries an incrementing writer epoch.
- Every durable world mutation locks that record and checks the epoch in the transaction before changing game data.
- Acquisition of a new epoch is an explicitly authorized startup/takeover operation, not an automatic loop by competing replicas.
- Before takeover, operators fence or stop the previous process and its connections. Epoch checking protects database writes; it does not by itself prevent a stale process from emitting misleading ephemeral output.
- A stale-epoch result causes the old runtime to stop admitting mutations and shut down its world service.

The release controller must enforce a one-active-process deployment. Automatic failover, leases, and high-availability promotion require a separate tested design. Durable account/session services are distinct from world simulation but cannot use that distinction to modify protected game state.

### Failure windows

| Failure point | Required recovery |
|---|---|
| Before transaction begins | No durable effect; request may be retried |
| Transaction definitely rolls back | Return a safe rejection or retry instruction; do not apply effects |
| Connection fails during commit | Outcome is unknown; pause state-changing execution and resolve using the receipt and database state |
| Commit succeeds before memory update | Reload/reconcile the affected projection or restart the world before continuing mutation |
| Commit succeeds before response reaches client | Reconnect and retrieve the original result using the same scoped key |
| Outbox delivery succeeds but acknowledgment fails | Consumer must tolerate duplicate delivery |
| Old server resumes after takeover | Epoch validation rejects durable writes; stale process is fenced and stopped |

For an unresolved commit, use a fresh database connection and wait for the original transaction to resolve within an operational deadline. If resolution cannot be established, transition to failed/not-ready and recover through the controlled restart path. Never assume “no receipt observed yet” proves the original transaction cannot still commit.

### Outbox and audit

Use an outbox for durable side effects that must be dispatched after a transaction, such as external notifications or downstream indexing. Polling and retries are bounded; delivery is at least once unless a stronger consumer protocol is implemented.

Ordinary room text does not need an outbox in the initial system. Critical command results are recoverable from receipts rather than guaranteed by transient WebSocket delivery.

Administrative mutations include an audit record in the transaction. If the required audit cannot be persisted, reject the mutation. Audit records include actor, capability, reason, target, operation ID, and appropriate before/after summaries without secrets.

### Backup and restore boundary

A recovery set includes database backups and recovery logs where configured, immutable content artifacts, release manifests, migration state, and protected operational configuration. Credentials are recovered or rotated through the secret-management process, not copied into world content.

Database commit durability and disaster-recovery objectives are different promises. A transaction surviving process restart does not imply zero loss after losing the database host. RPO and RTO remain measurable release requirements, not guarantees supplied by this design.

## Game SDK and Extensions

### Contract shape

The SDK should expose a narrow, documented facade over core types and content facilities:

- Read-only query contexts restricted to appropriate world visibility.
- Typed command registration and parsing metadata.
- Rule evaluation that returns an effect plan or structured rejection.
- Declared namespaced game-state types and migration hooks.
- Controlled schedule creation and cancellation.
- Content-type registration and validation hooks.
- Test fixtures and injectable time/randomness.

Exact Rust trait signatures are deferred until the playable slice validates ownership, lifetimes, error types, and async boundaries. Do not publish placeholder APIs as stable contracts.

### Trusted extensions

Compiled Rust game modules run with process privileges. A trait interface can encourage disciplined access but is not a security sandbox. Native modules can still perform arbitrary process-level operations if malicious or careless.

Consequently:

- Only trusted, reviewed Rust modules are linked into production.
- Rule callbacks are expected to be bounded and avoid network, filesystem, and database I/O.
- Tokio timeouts cannot safely preempt arbitrary CPU-bound native code. If a callback hangs, recovery may require restarting the process.
- A future untrusted scripting facility needs real isolation, memory/fuel limits, restricted capabilities, and an explicit threat model.
- Game-specific SQL migrations require review and must not invalidate shared invariants.

Do not promise ABI-compatible hot loading of Rust libraries. The baseline extension model is source-level integration and rebuilding a game server against a compatible SDK version.

### Cross-game proof

Use a small fantasy reference world and a small science-fiction reference world to validate reusable abstractions. Both should implement movement, interaction, one encounter, and one progression outcome without changing core ownership or persistence logic.

Differences in combat vocabulary, resources, and content belong in game packages. If every new game requires modifying transport handlers or core tables, revisit the abstraction before declaring the SDK stable.

## Implemented Local Play Host

`worldwright play <path>` is an additive developer command, not a change to the JSON schema or core authority. It loads validated content, creates one ephemeral local actor at `entry_room`, registers ReferenceGame, and owns a WorldRuntime until quit/EOF. All Look/Move/Say actions go through `execute`; named-exit resolution only considers the actor's current room and cannot authorize a missing edge. Successful Move is followed by a separate Look execution, not an expanded transaction.

The adapter uses checked operation IDs and elapsed monotonic time. It displays only messages addressed to the exact current attachment and only that room's exit labels. It does not print host snapshots or lookup maps. Bounded line framing drains oversized input without executing its suffix, supports LF/CRLF and final lines at EOF, and preserves full valid Unicode text while escaping terminal controls. Input mistakes are recoverable; I/O and runtime invariant failures stop the session and attempt detach. See [Local Play](docs/LOCAL_PLAY.md) for exact semantics.

No accounts are authenticated and no state is saved. Fixed process-local IDs are not durable identities. The reference rules do not consume randomness; unexpected requests fail rather than receiving fabricated random values. The implementation is in the CLI application, with no new library dependency or core/content API change.

## Implemented Content Schema and Conversion

The normative local loader contract is [Schema v1](docs/content/SCHEMA_V1.md). Its fields, strict record shapes, duplicate-key policy, graph checks, input limits, and diagnostics override illustrative content examples elsewhere in this target-system design.

| Version | Change | Breaking? | Migration |
|---|---|---|---|
| Initial v1 | First implemented single-file room content loader; required version, namespace, entry room, rooms, text, explicit directed exits | No previous supported file format; earlier examples were illustrative | Missing/unsupported versions fail with a schema-document pointer; manually update drafts after review |

The unpublished Rust `ContentPipelineStub` API and re-exports are removed. Replace them with `load_file`/`load_bytes`, then `instantiate`; SDK exposes `content`. This is an explicit Rust API break, not an automatic file migration. Future version changes must classify compatibility and supply a migration or actionable refusal.

Validation produces private immutable types. No partially valid definitions or builders escape. Definitions are sorted by their validated namespaced IDs; instance room IDs are checked nonzero sequential values independent of JSON ordering. They are not stable across content-set changes and must not be used as durable identity. Named exits and the entry-room lookup are retained beside the builder. The host still supplies world identity, characters, rule registration, and attachments.

One-way edges are intentional; no reverse edge is synthesized. Unreachable rooms require `allow_unreachable: true`. Multiple labels targeting the same room are rejected to preserve the current unique-edge runtime contract. Diagnostics are capped, terminal-safe on display, and use JSON pointers for semantic errors. A valid file is neither an approved immutable release nor authority to activate a live world.

## Content Lifecycle

### Representation

Use JSON definitions with an explicit schema version and namespaced identifiers. TOML is reserved for operator configuration in this proposal; it is not a second competing content format.

Example content document following implemented schema v1 (not a published/approved release):

```json
{
  "schema_version": 1,
  "namespace": "frontier",
  "entry_room": "frontier:rooms.arrival",
  "rooms": [
    {
      "id": "frontier:rooms.arrival",
      "title": "Arrival Concourse",
      "description": "A quiet docking concourse overlooks the station ring.",
      "exits": {
        "north": "frontier:rooms.observation"
      }
    },
    {
      "id": "frontier:rooms.observation",
      "title": "Observation Deck",
      "description": "Distant beacons blink beyond the glass.",
      "exits": {
        "south": "frontier:rooms.arrival"
      }
    }
  ]
}
```

### Validation stages

1. Parse with size, nesting, and resource limits.
2. Validate schema version and required fields.
3. Reject duplicate IDs and conflicting namespaces.
4. Resolve references across the complete release, including game-defined content.
5. Check semantic constraints: numeric ranges, item definitions, quest transitions, spawn rules.
6. Check navigation and report unreachable rooms; allow intentional one-way exits and isolated areas only through explicit declarations.
7. Run reference scenarios and project-specific validation rules.
8. Produce a deterministic manifest and content digest.

Diagnostics include severity, code, source path, JSON pointer or equivalent location, and a human-readable explanation. Publication blocks on errors; warning overrides are explicit and recorded.

### Artifact and activation

The content artifact includes a manifest identifying release ID, schema version, compatible engine/game versions, content digests, and migration requirements. Reproducible digests require a defined canonical encoding or sorted bundle process; a timestamp-bearing ZIP file is not automatically reproducible.

Initial workflow:

**Draft → Validated → Reviewed → Packaged → Staged → Approved → Activated**

Initial activation occurs during maintenance:

- Drain and stop world mutation.
- Validate the candidate artifact and runtime compatibility.
- Verify existing persistent references have mappings or remain valid.
- Back up or otherwise establish the approved recovery point.
- Apply reviewed data migrations and update the active release record.
- Reconstruct runtime state against that exact release.
- Open admission only after validation and smoke checks.

A failed startup leaves the world unavailable, not half-running against mixed definitions. Binary rollback and content rollback are allowed only when persistent data remains compatible. A destructive migration may require a forward repair or approved restore rather than reverting files.

Hot reload, in-place entity migration, and live multi-release sessions are deferred.

### Spawn and reward identity

Persistent spawn generation and quest completion generation must have stable, durable identities where they create valuable assets. Restarting a process or reactivating the same content release must not blindly mint duplicate items.

Content templates describe what may exist. Durable spawn/claim records determine what has already been created or awarded.

## Protocol and Player Experience

### Browser-first interaction

HTTPS handles account and application pages; secure WebSockets carry interactive play. Authentication uses a server-side session established before interactive play. No credentials or long-lived bearer tokens belong in WebSocket URL query strings.

The wire protocol distinguishes:

- Handshake/version negotiation and session attachment.
- Command intent and optional idempotency identifier.
- Accepted-for-processing notification.
- Final command result or unresolved-operation status.
- Structured display events and state snapshots.
- Heartbeat, maintenance, and reconnect instructions.

Route names and port numbers are not fixed here. HTTP operations and WebSocket message contracts must be specified and tested before clients depend on them.

### Envelope proposal

A client command may resemble the following. Fields are illustrative and do not assert an implemented API:

```json
{
  "protocol_version": 1,
  "type": "command",
  "request_id": "client-request-00042",
  "command": "look",
  "arguments": []
}
```

The server attaches identity. A client-supplied character ID is at most a requested selection to authorize, never proof of ownership.

For ordering and resynchronization, server output carries a connection/stream epoch and monotonic sequence within that stream. These values detect gaps; they do not promise a persistent replay log. After restart or a gap that cannot be filled, send a fresh authorized snapshot and require reconciliation of unresolved durable requests.

### Reconnect and duplicate sessions

Baseline policy: one controlling connection per character. An explicit authenticated takeover increments the character's connection generation and invalidates the old attachment.

Queued commands carry their original generation. The runtime rejects stale-generation commands when executed. A reconnect does not automatically replay all commands in browser history.

On attachment:

1. Revalidate the account, character ownership, and restrictions.
2. Establish the new controlling generation.
3. Send current authorized state and negotiated capabilities.
4. Reconcile identified pending durable operations.
5. Resume ordinary command admission.

### Text rendering and accessibility

Send text and structured presentation metadata, not arbitrary executable HTML. Escape player, builder, and external content on every rendering path. Terminal escape sequences are rejected or mapped through a controlled formatter if a future terminal adapter is introduced.

The browser should support keyboard navigation, readable focus behavior, bounded scrollback, selectable text, optional color, and screen-reader-conscious update grouping. Screen-reader support requires user testing; ARIA annotations alone are not evidence of accessibility.

Chat history, command history, and audit logs have different privacy and retention requirements. Do not retain private conversations simply because diagnostic logging is convenient.

## Identity and Authorization

### Authentication baseline

Propose browser form login with revocable server-side sessions. For password accounts, use Argon2id with independent salts and full encoded hash parameters; select costs through security review and load testing. Bound password-hash work so login floods cannot exhaust the runtime.

Cookies require secure transport, HttpOnly, an appropriate SameSite policy, and deliberate scope. Rotate session identifiers on authentication and privilege changes. Logout, account suspension, and credential changes must invalidate relevant sessions and active gameplay attachments.

Automated operator authentication, organizational identity-provider integration, and account recovery delivery are open design decisions. Do not silently enable Basic Auth as a fallback or reuse a human's permanent session token for automation.

### Authorization

Roles are convenient groupings of explicit capabilities. Check capability and target scope for every operation, including at execution time.

| Principal / role | Typical capability boundary |
|---|---|
| Player | Own character actions and permitted communications |
| Builder | Edit authorized project drafts and use isolated preview |
| Publisher | Approve a particular content release for a specified environment |
| Moderator | Investigate reports and apply scoped conduct actions |
| Operator | Deploy, drain, inspect health, and perform approved recovery |
| Administrator | Manage grants and exceptional controls through audited workflows |

Production publication and destructive repair should support independent approval. Privileged users must not gain access to secrets or private player data merely because they can restart the world.

Require an approved stronger authentication policy for privileged production access, normally MFA through an identity provider or a reviewed equivalent, before operational beta.

### Browser and transport protections

- Same-origin browser operation by default; no wildcard authenticated CORS.
- Explicit allowed-origin checks for browser WebSocket upgrades.
- CSRF protection for cookie-authenticated state-changing HTTP requests.
- Authentication and authorization independent of CORS.
- Safe redirect validation and proxy-header trust configured for the actual deployment.
- Rate limits by relevant identity and network dimensions, without assuming shared IP means one player.
- Content Security Policy and escaping for browser surfaces.
- No sensitive request bodies, cookies, passwords, or tokens in normal logs.

A private reporting route and operational retention policy must be real, approved, and documented before public release. They are not invented by this design.

## Tool Contracts

### CLI

The developer CLI coordinates project initialization, validation, local test execution, and packaging through shared libraries. Exact command names are deferred until implemented.

Local operations must provide machine-readable diagnostics, stable exit semantics, and safe path handling. Scaffolding must not overwrite user files without an explicit conflict policy. Generated projects include the selected template/content license notices.

Remote publication is a distinct privileged workflow. Possession of the CLI does not grant access to any production server.

### Studio

Begin with structured files and a shared validator. Studio later adds editing, graph navigation, preview, and publication requests over the same content contracts.

Draft saves use revision tokens or equivalent optimistic concurrency to detect conflicting edits. Preview runs use isolated state and credentials. A preview may execute trusted game code and therefore requires process/environment isolation from production, not just a different UI tab.

Studio never uses production database credentials and does not patch live entity tables.

### Console

Console displays authorized runtime snapshots and submits typed control operations. Every mutation requires a capability and explicit target scope; sensitive actions require a reason and, where policy requires it, independent approval.

Do not expose a generic SQL console, arbitrary server shell, or unrestricted object editor as the default administration interface. Restoration of items or progression is a business operation with a unique repair ID and audit, not a database update disguised as a button.

### Testkit

Testkit offers scenario composition, seeded randomness, fake time, fixtures, invariant assertions, and fault injection around adapter boundaries. Its scenarios should be portable between headless core tests and full-stack integration tests where practical.

A headless test runtime is not a second implementation of the rules. It runs the same core and game modules with controlled adapters.

## Configuration and Compatibility

### Separate version axes

| Version | Purpose |
|---|---|
| Engine / package version | Code and supported SDK behavior |
| SDK compatibility version | Source integration contract for game packages |
| Protocol version | Client message and semantic compatibility |
| Content schema version | Definition encoding and interpretation |
| Content release ID | Exact immutable world material |
| Database migration version | Durable relational structure |
| Game-state schema version | Game-specific persisted fields and migrations |
| Configuration schema version | Operator settings and defaults |

A release manifest records the applicable versions together. Do not overload a single integer to represent all compatibility domains.

### Configuration policy

Propose TOML with `schema_version`, deployment identity, bind addresses, public origin, content-release reference, limits, and secret references. Do not store account passwords, API tokens, or database passwords in committed examples.

Loading order is explicit: schema-defined defaults, selected configuration file, then a documented allowlist of environment overrides. Secrets resolve through the approved provider after structural validation. Log only a redacted effective configuration and its nonsecret fingerprint.

Initial reload policy is restart-required unless a field is explicitly documented as safely reloadable. Database targets, writer ownership, security origins, schema versions, and active content changes are not casual in-memory reloads.

### Evolution rules

- New optional fields with unchanged semantics and sensible defaults may be additive.
- New required fields, renamed keys, changed encodings, and changed effective defaults are breaking.
- Reject unsupported future versions with an actionable message.
- Reject unknown operator-config keys by default to catch misspellings.
- Protocol readers follow their explicitly negotiated compatibility rules; do not apply config-key rejection blindly to extensible wire messages.
- Breaking changes ship with a tested migration chain or a specific, safe refusal explaining operator action.
- Automatic migration must preserve a recovery copy and must not log secrets.

### Proposed schema history

| Domain | Version | State | Change / migration |
|---|---|---|---|
| Operator configuration | 1 | Proposed, unpublished | Establish explicit version and validated settings; no legacy format assumed |
| Content definitions | 1 | Proposed, unpublished | Namespaced IDs and reference validation |
| Browser protocol | 1 | Proposed, unpublished | Typed commands/results and reconnect negotiation |
| Game-state schema | Not assigned | Open | Define after first reference-game persistence model |
| Database schema | Not assigned | Open | Create reviewed migrations from the persistence design |

There are no deployed migrations claimed by this table. When a version is released, record actual compatibility, examples, and tested upgrade paths here or in a linked authoritative history.

## Failure and Overload Policy

| Condition | Baseline response |
|---|---|
| Invalid command | Structured error; no world mutation |
| Ingress queue full | Reject admission or close abusive connection; never grow without bound |
| Slow client / output queue full | Coalesce safe replaceable updates or disconnect; do not silently drop critical results |
| Database unavailable | Stop admitting durable mutations; fail readiness as appropriate; no speculative success |
| Unknown transaction outcome | Suspend state-changing execution and resolve or restart safely |
| Invalid content artifact | Reject activation; keep known-good release or remain in maintenance |
| Missing durable content reference | Fail load or execute an approved migration; never delete valuable state silently |
| Audit persistence unavailable | Reject actions requiring that audit |
| Outbox destination unavailable | Retry with backoff and limits; alert on age/backlog |
| Game callback panic | Stop the affected world and recover from durable state; do not continue with uncertain invariants |
| Stale writer epoch | Reject writes and shut down stale runtime |
| Security revocation | Invalidate attachments and reject queued stale requests |

Liveness means the process can make supervisory progress. Readiness means it can safely serve the intended workload. A database outage must not cause endless liveness-triggered restart loops that obscure the incident.

Metrics must distinguish admission time, queue wait, rule evaluation, transaction time, and delivery lag. Avoid high-cardinality labels containing account IDs, character names, request IDs, or raw commands; use access-controlled logs for necessary correlation.

## Verification Strategy

### Invariant catalog

| ID | Required property | Evidence to build |
|---|---|---|
| WW-I01 | A persistent item has exactly one valid holder | Constraint, property, transfer, split/merge, and recovery tests |
| WW-I02 | Currency cannot be created except by an authorized mint/reward rule | Conservation tests and ledger/transaction assertions |
| WW-I03 | A reward completion generation is claimed at most once | Duplicate key, fresh key, concurrent attempt, and restart tests |
| WW-I04 | No durable success precedes established commit | Failure injection before/during/after commit |
| WW-I05 | Stale character connections cannot execute queued actions | Generation takeover tests |
| WW-I06 | Unprivileged tools cannot change production state | End-to-end authorization and scope tests |
| WW-I07 | Resource growth is bounded under overload | Slow-client, flood, task-budget, and queue saturation tests |
| WW-I08 | Active runtime state corresponds to one compatible content release | Activation and migration failure tests |
| WW-I09 | Stale writers cannot commit durable world changes | Two-process epoch and takeover tests |
| WW-I10 | Restored data and content can start a playable world | Isolated restore drill and scripted smoke play |
| WW-I11 | Private events are not delivered to unauthorized recipients | Visibility and permission-revocation tests |
| WW-I12 | Game rules run consistently in production and Testkit | Shared implementation scenarios with controlled adapters |

### Test layers

1. **Unit:** parsing, target resolution, effects, scheduling, and schema validation.
2. **Property:** ownership, quantity conservation, graph/reference constraints, and state-machine invariants.
3. **Integration:** real PostgreSQL transactions, isolation behavior, receipt uniqueness, migrations, and outbox behavior.
4. **Protocol:** handshake rejection, auth expiration, origin checks, sequence gaps, and reconnect.
5. **Scenario:** two players, one quest, one encounter, one reward, restart, and administrative investigation.
6. **Failure injection:** database disconnects, process termination, partial publication, stale writer, and slow clients.
7. **Security:** privilege boundaries, rendering injection, brute-force resistance, and secret/log redaction.
8. **Operational:** drain, backup, restore, deployment compatibility, and incident rehearsal.
9. **Cross-game:** a second game reuses the SDK without special cases in core infrastructure.

Before claiming a feature verified, record its actual toolchain and checks. The current core/workspace checks passed on Linux Rust 1.85.0, as detailed in [VERIFICATION.md](docs/VERIFICATION.md). They do not establish production end-to-end, persistence, authorization, Windows, or pinned remote CI behavior.

For test/implementation conflicts, first determine the intended contract. Update the incorrect side and record the reason; do not edit tests solely to obtain a green run.

## Delivery Gates

Aligned with the phases in the README:

| Phase | Design work that must be resolved | Demonstration |
|---|---|---|
| 1. Foundation | Ownership/license approval, crate boundaries, version policy, CI | Reviewed skeleton, reproducible checks, documented defaults |
| 2. Playable slice | Command pipeline, session attachment, state owner, browser messages | Two players can look, move, and communicate |
| 3. Durable gameplay | Schema, receipts, writer fencing, recovery, game-rule effects | Reward and inventory survive tested failure windows |
| 4. Builder workflow | Content schema, validation diagnostics, artifact compatibility | Reviewed zone staged and activated without dangling references |
| 5. Operational beta | Privileged auth, moderation, audit, secrets, backup/restore | Restricted beta with named service owner and measured recovery |
| 6. Toolkit maturity | SDK stability, editor ergonomics, migration support | Second game and independent developer complete the workflow |

Do not defer identity, ownership, or transaction invariants until the operational beta. Later phases strengthen earlier foundations rather than retrofitting basic correctness.

## Open Decisions

| ID | Decision still required | Accountable role | Must be resolved before |
|---|---|---|---|
| WW-O01 | Copyright owner, final license, inbound contribution terms | Authorized project owner | Public code publication |
| WW-O02 | Verify selected bootstrap Rust 1.85.1 / edition 2024; establish supported toolchain matrix and future external dependency versions | Engineering lead | Reproducible foundation build |
| WW-O03 | Account recovery and privileged MFA/identity-provider approach | Security owner | External beta |
| WW-O04 | Exact SDK traits, content schema, and protocol specification | Engineering/tooling leads | Publishing supported interfaces |
| WW-O05 | Database schema and durability/replication configuration | Engineering/service owners | Durable gameplay acceptance |
| WW-O06 | Receipt retention, chat privacy, audit retention | Security/community/product owners | Operational beta |
| WW-O07 | Hosting target, domains, ports, process supervisor, secret provider | Service owner | Deployment implementation |
| WW-O08 | Concurrency target, latency budget, RPO, RTO, support hours | Product/service owners | Operational beta go/no-go |
| WW-O09 | Reference-game rules and persistent versus ephemeral state | Game design lead | Durable gameplay |
| WW-O10 | Packaging of Studio and Console | Tooling/service owners | Shipping those interfaces |
| WW-O11 | Supported SDK compatibility window and deprecation policy | Maintainers | Stable SDK release |
| WW-O12 | Adoption of contribution/conduct/security policies; maintainers, primary/alternate reporting channels, appeals, and response coverage | Project/security/community owners | Opening external collaboration and beta operations |

An open decision is not permission for each component to invent a different answer. Resolve it once, document it, and test the resulting contract.

## Maintenance and Licensing

- Keep this document synchronized with code, schemas, migration behavior, and public contracts.
- Keep ARCHITECTURE synchronized with crate dependencies, process placement, deployment, and trust boundaries.
- Keep README entry points and component status accurate as implementations become available.
- Keep [SECURITY.md](SECURITY.md), [CONTRIBUTING.md](CONTRIBUTING.md), and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) aligned with these contracts; distinguish drafted policies from adopted, staffed procedures.
- Use [DEFINITIONS.md](DEFINITIONS.md) for MU* vocabulary and separate gameplay-style support from legacy-engine compatibility.
- Add actual interface specifications and operating runbooks as their owners establish authoritative procedures.
- Keep software, templates, sample content, and branding license scopes explicit.

The README's **MIT OR Apache-2.0 recommendation remains pending authorized approval**. This document does not create license files, grant rights, establish ownership, or change that status.

**Baseline success:** a small persistent world that is understandable, testable, recoverable, and reusable—not a collection of disconnected tools with impressive names.
