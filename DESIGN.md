# Worldwright — Design

**Build worlds. Write the rules.**

| Attribute | Value |
|---|---|
| Document version | 0.2 — expanded-roadmap alignment; proposed contracts remain subject to approval |
| Updated | October 21, 2026 |
| Delivery authority | [ROAD-MAP.md](ROAD-MAP.md): eight dependency-ordered milestones; M4 is next active, not implemented |
| Prepared | October 20, 2026 |
| Product | Rust development suite for MUDs: multi-user domains |
| Audience | Engine and tooling maintainers, game developers, designers, QA, security, and operators |
| Implementation status | Content loading, local runtime, Local Play, and authenticated process-local Transport v0.1 implemented; Windows local evidence is recorded in verification, while remote CI, deployment, persistence, and hosted operation remain unverified |
| Companion documents | [README.md](README.md) · [ARCHITECTURE.md](ARCHITECTURE.md) · [ROAD-MAP.md](ROAD-MAP.md) |

> **Scope and authority:** This document describes the target system; “must” states a requirement, not completion. The reported M1–M2 baseline includes single-owner in-memory execution, strict content loading, and Local Play. M3 adds authenticated, bounded loopback transport and has reported Windows-local evidence. Earlier Linux core/content/local-play evidence does not prove a Linux Transport rerun. Persistence, controlled releases, operations, Studio, and toolkit maturity remain planned M4–M8 work.
>
> **Synchronization basis:** This documentation-only revision compares the supplied DESIGN and ARCHITECTURE documents with the expanded ROAD-MAP. Repository source, manifests, migrations, and linked verification logs were not independently inspected. Retained implementation details and numerical limits are reported contracts, not newly verified code. New examples and state labels are design guidance, not finalized schemas or API constants.
>
> **Document boundary:** README covers product usage; this file owns behavioral contracts and rationale; ARCHITECTURE owns component and process placement. ROAD-MAP owns milestone scope, dependency order, blocking decisions, and exit evidence. Detailed local contracts remain in [RUNTIME.md](docs/RUNTIME.md), [CORE_INTERFACES.md](docs/CORE_INTERFACES.md), [PROTOCOL_V1.md](docs/PROTOCOL_V1.md), and the content schema. [SECURITY.md](SECURITY.md), [CONTRIBUTING.md](CONTRIBUTING.md), and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) remain proposed policies pending adoption; [DEFINITIONS.md](DEFINITIONS.md) covers terminology. Disagreements require explicit reconciliation, not silent promotion of a roadmap example into an implemented contract.

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
- [Implemented Local Play Host](#implemented-local-play-host)
- [Implemented Multiplayer Transport v0.1](#implemented-multiplayer-transport-v01)
- [Implemented Content Schema and Conversion](#implemented-content-schema-and-conversion)
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

Entries below record the target design position, not blanket implementation status or proof of owner adoption. Parts of WW-D01, WW-D02, WW-D05–WW-D08, WW-D10, WW-D11, and WW-D14 have reported M1–M3 implementations; durable and operational extensions remain planned. Use the implemented-subset sections and delivery gates to distinguish the two. Formal acceptance or amendment requires an accountable decision record.

The `WW-Dxx` design identifiers are **not** the roadmap's `D01`–`D07` owner-decision identifiers. Keep both namespaces in issues; use the crosswalk under [Open Decisions](#open-decisions), not number-based equivalence.

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

### Authority and identifier separation

| Identity / observation | Scope and lifetime | Must not be reused as |
|---|---|---|
| Authenticated account | Server-established principal under current identity policy | Proof of authority over every character/world |
| Runtime attachment generation | Current character/controller binding in the live runtime | A durable session, writer epoch, or restart-stable authority |
| Protocol v1 request ID | Bounded stream-local correlation | A durable deduplication key |
| Stream epoch and sequence | Connection identity and gap detection | A replay log offset or commit order |
| Process-local operation ID / runtime revision | Runtime execution and ordering | Database identity or durable commit evidence |
| Planned durable operation key | Approved principal/world/operation scope and retention window | Authentication or business entitlement |
| Writer authority | Current permitted database writer for a world | Permission to deliver stale ephemeral output |
| Content release identity / digest | Exact compatible immutable artifact | Proof that a publisher approved it |

The application translation boundary constructs trusted context; clients cannot supply authority by adding a field. Checked counters must fail closed on exhaustion instead of wrapping to a previously valid identity.

### Result states

The following categories are **target design vocabulary**, not additions to the implemented protocol-v1 error enum. Their exact wire representation requires a reviewed protocol contract. Use stable machine-readable result categories with separate player-facing text:

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

### M4 scope and design entry gate

[ROAD-MAP M4](ROAD-MAP.md#milestone-4--durable-gameplay-v01) is the next active increment and remains planned. It proves **one** PostgreSQL-backed item or reward operation, not persistence of every runtime field. Select the operation and identity/session approach before implementation; preserve M3 authority, bounded admission, exact routing, and separate Move/Look behavior.

A one-time item entitlement is a proposed first candidate, not an adopted reference-game rule. Inventory trading, a full economy, durable chat, universal command replay, event sourcing, automatic failover, and distributed world mutation are excluded unless separately approved. Development restart/retry evidence belongs to M4; production backup/restore and service readiness require M6.

Before writing migrations, document:

- Stable world, account, character, and content references for the chosen operation.
- Authoritative database facts versus ephemeral and derived state.
- Exclusive ownership or entitlement uniqueness and expected-version/locking rules.
- Writer transaction fence, receipt scope, canonicalization version, retention, and result policy.
- Required audit/outbox participation and behavior if either cannot be persisted.
- Projection reconstruction, migration compatibility, and uncertainty-resolution procedure.

The content loader's deterministic sequential room IDs are not durable references across content-set changes. Persist stable definition identity or an approved durable mapping; reconstruct runtime IDs from the selected compatible content. Do not reinterpret old saved numbers against a newly ordered room set.

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

Introduce a durable operation key for retryable mutations, scoped by trusted world/actor identity and the approved operation domain. It is **not** protocol v1's stream-local request ID, and v1 `command_result` must not silently become a durable receipt. Define the new negotiation/message contract before a client relies on cross-reconnect reconciliation. Store:

- The scoped request key and server operation ID.
- A canonical intent fingerprint, including relevant targets and arguments.
- The authoritative committed or recorded rejected result.
- Commit timestamp, affected versions, and serialization format version.

A repeated key with the same intent returns the previous result after authorization checks. A repeated key with different intent is rejected. The key is never evidence of identity or entitlement.

Enforce independent business uniqueness as well. For example, a quest reward needs a claim key tied to character, quest, and repeatable-completion generation. A malicious client using new request IDs must not claim the same completion repeatedly.

Receipt retention and the retry guarantee are M4 design decisions, not details to postpone until operational beta. Expired receipt cleanup must not defeat independent ownership and business-uniqueness constraints. Clients must not blindly resubmit old pending requests under fresh IDs.

| Attempt | Required outcome |
|---|---|
| Same scoped key, same canonical intent | Return the stored outcome without a second grant, subject to current authorization |
| Same scoped key, different intent | Explicit key-reuse conflict |
| Different key, same one-time entitlement | Business uniqueness still prevents duplicate value |
| Concurrent first use of one key | Serialize/resolve under a database constraint; one stable outcome |
| Key beyond retention window | Follow the documented expiry/tombstone policy; no unlimited retry promise |
| Lost COMMIT reply, no immediately visible receipt | Remain uncertain until the original transaction is resolved |

Canonicalize the server-resolved semantic intent, including value-determining target and scope. Version the canonicalization; raw JSON bytes are not a stable semantic fingerprint because field order and equivalent encodings can differ. Do not include a reconnect's stream epoch or incidental presentation text in the business fingerprint unless explicitly required. Rejected business outcomes may be recorded as terminal receipts under the chosen policy; infrastructure failures must not become permanent business rejections.

**Example:** a retry returns the original granted item ID, not a rerolled reward. A fresh key for that same one-time entitlement also cannot mint another item. Test semantic equivalence, changed-intent conflicts, concurrent first attempts, and cleanup boundaries against real PostgreSQL.

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

Specify the unique-key reservation/lookup strategy and lock order before coding the transaction. A concurrent receipt insertion can fail with a uniqueness violation; PostgreSQL does not let the transaction continue normally after that error without the appropriate rollback/savepoint recovery. Returning a successful prior receipt is distinct from blindly rerunning the business logic.

Conditional ownership changes must prove the expected holder/world and affect exactly the intended item. Foreign keys, exclusive ownership constraints, entitlement uniqueness, and nonnegative quantity checks reinforce application validation. SQL row counts do not replace authorization. The first slice need not implement every holder type, stack operation, or currency rule in the target domain model.

If a required audit or outbox insert fails, no associated business effect or terminal success receipt may commit. If projection application fails after a confirmed commit, stop affected mutation and reconcile/reconstruct; never invent a database rollback or grant the value again. Test failures after each transaction step and after commit but before projection/output.

### World writer fencing

Only one writer may mutate a world's durable gameplay state. Application deployment conventions alone are not a sufficient safeguard against two server processes starting accidentally.

Proposed baseline:

- A persistent world-control record carries an incrementing writer epoch.
- Every durable world mutation locks that record and checks the epoch in the transaction before changing game data.
- Acquisition of a new epoch is an explicitly authorized startup/takeover operation, not an automatic loop by competing replicas.
- Before takeover, operators fence or stop the previous process and its connections. Epoch checking protects database writes; it does not by itself prevent a stale process from emitting misleading ephemeral output.
- A stale-epoch result causes the old runtime to stop admitting mutations and shut down its world service.

The release controller must enforce a one-active-process deployment. Automatic failover, leases, and high-availability promotion require a separate tested design. Durable account/session services are distinct from world simulation but cannot use that distinction to modify protected game state.

The gameplay transaction retains the authority-row lock through commit. Takeover uses the **same lock**, increments authority with checked semantics, commits, and only then permits the replacement to become write-ready. An older transaction already holding the lock may finish before takeover commits; this is defined serialization, not a failed fence. Later transactions presenting the old epoch must fail.

All protected world writes—including administrative repair and background grants—must use the fence. Define database privileges and reviewed write paths accordingly. A check outside the business transaction, a process heartbeat, or a lease alone is insufficient. Apply bounded lock/statement deadlines and an authorized stuck-transaction procedure; a deadline expiring does not by itself prove rollback.

**Example:** writer A holds epoch 12. After B commits takeover to epoch 13, a resumed A cannot grant a reward under epoch 12. Tests must also cover A already inside the locked transaction when B requests takeover.

### Disaster-restore authority

M6 restore is not ordinary M4 takeover. Restoring a backup may restore a lower epoch; merely incrementing it can collide with an authority value held by a still-running pre-restore process. Before reconnecting a restored service, isolate/terminate old writers, prevent their credentials/network routes from reaching the restored authority domain, and establish restore-safe authority.

An independently managed incarnation plus epoch is a **candidate**, not an adopted schema. Any incarnation source used to prevent reuse must not itself rewind with the same backup. The selected mechanism, credential isolation, and failure tests belong to roadmap D04 / WW-O05 and the recovery runbook. Never claim that an ordinary epoch increment alone protects disaster restore.

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

The reported baseline already has provisional core/SDK interfaces documented in `docs/CORE_INTERFACES.md`; do not treat existing signatures as nonexistent. Stable SDK commitments and new persistence/extension signatures require source inspection, downstream use, and M8 compatibility evidence. Do not publish illustrative APIs as stable contracts.

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

M8 requires a second **meaningfully different game**, not merely new room descriptions. A fantasy/science-fiction pair remains a possible fixture, but the second game's choice is open: an investigation game or resource-driven expedition may expose different rule assumptions more effectively. Movement and interaction should reuse common infrastructure; encounters/progression are added only when the selected game requires them.

Produce a reuse/gap report covering commands/effects, attachment authority, durable operations and fencing, content packaging, preview, and operations. Identify which parts reused unchanged, which required general contract improvements, and which remain game-specific. Do not hide gaps with game-name branches in transport or core invariants.

Only then establish supported SDK/toolchain combinations, public exports, deprecation windows, migration guidance, and downstream compilation tests. An independent game must exercise an upgrade and the shared negative/recovery tests. Differences in vocabulary, resource policy, and rules belong in game packages; shared changes must generalize.

### M8 tooling and performance evidence

Generated projects should keep `apps/` binary directory/package/binary names aligned and exercise generate/build/test in repeatable checks. Templates must not overwrite user code silently; content/schema migrations provide dry-run output, a recovery copy, and a new immutable artifact rather than modifying approved bytes.

Benchmark idle heartbeats, representative Look/Move/Say, high fan-out with slow consumers, contended rewards/retries, outbox recovery, reconstruction, and long-running retained memory. Record hardware, OS, build profile, dependency/database configuration, dataset, workload, warm-up/duration, throughput, tail latency, memory, and queue depth. Compare like-for-like revisions and preserve invariant tests.

A per-world fence lock may become a measured bottleneck; removing it requires equivalent authority guarantees. Independent worlds are a preferable scaling boundary to speculative distribution of one mutable world. Numeric beta targets and support commitments remain owner decisions.

## Implemented Local Play Host

`worldwright play <path>` is an additive developer command, not a change to the JSON schema or core authority. It loads validated content, creates one ephemeral local actor at `entry_room`, registers ReferenceGame, and owns a WorldRuntime until quit/EOF. All Look/Move/Say actions go through `execute`; named-exit resolution only considers the actor's current room and cannot authorize a missing edge. Successful Move is followed by a separate Look execution, not an expanded transaction.

The adapter uses checked operation IDs and elapsed monotonic time. It displays only messages addressed to the exact current attachment and only that room's exit labels. It does not print host snapshots or lookup maps. Bounded line framing drains oversized input without executing its suffix, supports LF/CRLF and final lines at EOF, and preserves full valid Unicode text while escaping terminal controls. Input mistakes are recoverable; I/O and runtime invariant failures stop the session and attempt detach. See [Local Play](docs/LOCAL_PLAY.md) for exact semantics.

No accounts are authenticated and no state is saved in Local Play. Fixed process-local IDs are not durable identities. The reference rules do not consume randomness; unexpected requests fail rather than receiving fabricated random values. The implementation is in the CLI application, with no new library dependency or core/content API change.

## Implemented Multiplayer Transport v0.1

The server composition root now provides a bounded authenticated browser/WebSocket development slice. A strict local JSON account registry is checked against a fixed Argon2id v19 profile; four blocking password verifications are admitted at once; opaque server-side sessions expire after 12 hours and are revoked on logout. Exact Origin checks, loopback-only binding, Secure-by-default cookies, explicit local HTTP mode, CSP/security headers, and no query-string token keep the browser boundary narrow.

One Tokio owner task exclusively owns `WorldRuntime`, `WorldLookup`, operation/tick state, and exact attachment routes. Bounded gameplay, control, and per-connection output queues use admission deadlines and out-of-band cancellation for takeover, revocation, shutdown, slow consumers, and internal failures. The transport translates protocol DTOs into runtime intents, revalidates attachments and recipient routes, and keeps Move's follow-up Look as a separate execution. Browser assets render text through DOM text nodes and enforce envelope epoch/sequence checks.

This is implemented and tested only as a process-local, loopback development slice. It has no durable account/session state, restart recovery, database commitment, stable SDK/protocol compatibility policy, built-in TLS termination, proxy trust, public deployment assessment, or interactive accessibility assessment. See [protocol v1](docs/PROTOCOL_V1.md) and [server guide](docs/SERVER.md).

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

M5 converts validated definitions into immutable releases and isolated previews. M7 consumes those contracts through Studio. A manifest identifies the release, schema and engine/game compatibility, payload digest, provenance, and migration requirements. Exact manifest fields remain proposed.

Define the payload byte set and canonical encoding/sorted packaging rules before hashing. Avoid a digest over a manifest that contains that same digest, and do not assume timestamp-bearing archives are reproducible. Integrity verifies bytes relative to trusted metadata; it does not establish who approved them. Signing/attestation and trust-root requirements remain deployment/security decisions.

Proposed progression, aligned with the roadmap:

**Draft → Validated → Packaged → Reviewed → Staged → Activated**

These are workflow labels, not implemented database values. Source review can occur before packaging, but artifact approval must bind the exact packaged digest. Staging and production authorization are environment-specific; the simplified progression does not waive either approval. A preview may run against a validated snapshot, but the release demonstration must preview/test the exact artifact submitted for activation.

Initial activation occurs during maintenance:

1. Select approved bytes, target environment/world, and expected current release.
2. Revalidate digest, complete content graph, game/schema compatibility, and durable references in that target.
3. Close affected admission, settle/reconcile durable work, and establish the authorized writer/admission barrier.
4. Establish the approved recovery point and record activation intent plus required audit.
5. Apply reviewed migrations and the expected-release-checked active-release transition.
6. Reconstruct runtime definitions/state against that exact release.
7. Complete readiness and scripted gameplay checks before reopening admission and marking activation successful.
8. Retain the previous known-good artifact and the metadata/data compatibility needed to use it.

The database pointer and in-process content registry cannot be assumed to switch atomically. Define restart behavior for interruption before/after each durable transition. A failed reconstruction remains unavailable rather than serving mixed releases. Concurrent publication jobs based on the same old release must conflict, not silently overwrite each other.

| Recovery action | Required compatibility check |
|---|---|
| Content rollback | Old content still resolves current persistent rooms/items/progression |
| Binary rollback | Old executable understands current relational/game-state/config encodings |
| Data migration reversal | Reversal is reviewed, supported, and not silently lossy |
| Forward repair | New migration restores compatibility without pretending old state never committed |
| Backup restore | Approved data-loss window, compatible artifacts, isolated old writers, restored authority |

**Example:** a new package removes a room occupied by saved characters, or introduces an item the previous release cannot load. Reject activation/rollback or require an approved migration; do not silently teleport players, delete their value, or select an arbitrary fallback room.

Production activation capability in M5 is not production readiness: the exposure, identity, retention, ownership, and recovery gates in M6 must still pass. Hot reload, in-place entity migration, and live multi-release sessions remain deferred.

### Package and publication conflict rules

- Reject unsupported schema/game compatibility and mismatched digests before activation.
- If using archives, reject unsafe paths, symlink escape, duplicate entries, unbounded file counts/bytes, and decompression bombs.
- The same immutable release identity with different bytes is a conflict, not an overwrite.
- Identical-byte reuse follows a documented deduplication policy; it does not grant target-environment approval automatically.
- Edits after review produce a new digest and require refreshed approval.
- A stale expected current release requires refreshed validation/approval under policy.
- Publication submission returns a distinct request/status identity; submission is not approval or activation.

Record requester, reviewer/approver, and activation executor separately. Exact API and CLI names are not created by these requirements. Test tampering, incompatible durable references, stale release conflicts, crash-interrupted activation, and supported rollback.

### Spawn and reward identity

Persistent spawn generation and quest completion generation must have stable, durable identities where they create valuable assets. Restarting a process or reactivating the same content release must not blindly mint duplicate items.

Content templates describe what may exist. Durable spawn/claim records determine what has already been created or awarded.

## Protocol and Player Experience

### Implemented v1 contract

The development protocol is specified in [PROTOCOL_V1.md](docs/PROTOCOL_V1.md). It uses bounded JSON text frames, strict DTO validation, Hello → Welcome → Attach → Accepted/Attached/initial Look sequencing, exact request/exit/text limits, stream epochs, and per-stream sequences. Accepted is admission only; operation IDs, runtime revisions, and sessions are process-local. Standard WebSocket Ping/Pong is distinct from application `ping`/`pong`. Durable reconciliation, replay, snapshots after restart, and unresolved durable-operation handling remain future persistence work.

### Browser-first interaction

HTTPS handles account and application pages; secure WebSockets carry interactive play. Authentication uses a server-side session established before interactive play. No credentials or long-lived bearer tokens belong in WebSocket URL query strings.

The **target** protocol must distinguish the following. Items involving durable results, replay/reconciliation, or maintenance are future contracts, not implied v1 message support:

- Handshake/version negotiation and session attachment.
- Command intent and optional idempotency identifier.
- Accepted-for-processing notification.
- Final command result or unresolved-operation status.
- Structured display events and state snapshots.
- Heartbeat, maintenance, and reconnect instructions.

The implemented development routes and message contract are specified in [SERVER.md](docs/SERVER.md) and [PROTOCOL_V1.md](docs/PROTOCOL_V1.md). Stable public route, SDK, and compatibility policy remains open.

<a id="envelope-proposal"></a>

### Envelope and durable extension boundary

Use `docs/PROTOCOL_V1.md` and the actual DTO definitions for serialized v1 examples. This design deliberately avoids presenting invented fields under `protocol_version: 1`.

The carried-forward v1 contract includes a 16 KiB text-frame limit before JSON decoding; strict malformed/duplicate/unknown-field rejection; at most eight offered versions; request IDs of 1–128 bytes using the restricted visible ASCII alphabet; exit labels matching `[a-z][a-z0-9_-]*` with a 64-byte limit; and nonblank, control-free Say text of 1–4096 UTF-8 bytes. Binary gameplay frames and unsupported versions are refused. These reported limits are not changed by the roadmap expansion.

For ordering, server output carries a per-connection stream epoch and checked monotonic sequence. They detect gaps, not persistent replay offsets. The server establishes identity; M3 `attach` addresses the character already authorized by the session, not a new arbitrary character selector. Any future selection field would express a request to authorize, never ownership proof.

**Conceptual M4 extension flow, not serialized JSON or an existing endpoint:**

```text
New durable intent -> authenticated scope + durable key + semantic intent
Admission          -> bounded work accepted, no commit claim
Confirmed outcome  -> authoritative receipt and reconciled projection
Uncertain outcome  -> retain original key; resolve the original transaction
Reconnect          -> reauthenticate/reattach; authorized receipt reconciliation
Changed-key intent -> enforce business entitlement uniqueness independently
```

Choose an explicit protocol extension/version and supported-client behavior before implementing that flow. Do not redefine `accepted`, `command_result`, runtime revisions, or v1 request IDs. A gap/restart requires a fresh authorized view and, only for the future durable contract, reconciliation of pending durable operations; it never authorizes blindly replaying old Look/Move/Say history.

### Reconnect and duplicate sessions

Baseline policy: one controlling connection per character. An explicit authenticated takeover increments the character's connection generation and invalidates the old attachment.

Queued commands carry their original generation. The runtime rejects stale-generation commands when executed. A reconnect does not automatically replay all commands in browser history.

For the future durable-capable client, attachment/recovery must:

1. Revalidate the account, character ownership, and restrictions.
2. Establish a new controlling generation and stream context.
3. Send current authorized state under the negotiated contract.
4. Reconcile identified pending durable operations through their original scoped keys.
5. Resume admission only under the world's established readiness/uncertainty policy.

M3 implements attachment and current output, not step 4's durable lookup. A session that survives restart under a future M4 policy still needs a fresh attachment; it cannot revive an old generation. A result lookup is authorized independently of possession of an operation key.

### Text rendering and accessibility

Send text and structured presentation metadata, not arbitrary executable HTML. Escape player, builder, and external content on every rendering path. Terminal escape sequences are rejected or mapped through a controlled formatter if a future terminal adapter is introduced.

The browser should support keyboard navigation, readable focus behavior, bounded scrollback, selectable text, optional color, and screen-reader-conscious update grouping. Screen-reader support requires user testing; ARIA annotations alone are not evidence of accessibility.

Chat history, command history, and audit logs have different privacy and retention requirements. Do not retain private conversations simply because diagnostic logging is convenient.

## Identity and Authorization

### Authentication baseline

Transport v0.1 implements a bounded JSON account registry with unique typed account/character IDs, bounded names, and strict unknown/duplicate-field rejection. Password hashes are Argon2id v19 with the fixed v0.1 profile `m=19456,t=2,p=1`, 32-byte output, random salts, and no keyid/data. Password inputs are bounded to 1 KiB and four concurrent blocking verifications are admitted; unknown and wrong credentials share a generic 401.

Successful login creates a 32-byte opaque server-side token with a 12-hour absolute expiry and a 1024-session process-local cap. Cookies are HttpOnly/SameSite Strict and Secure by default. Logout, expiry, and authenticated takeover invalidate the relevant attachment. Recovery, MFA, durable identity, and durable revocation remain future decisions.

Cookies require secure transport, HttpOnly, an appropriate SameSite policy, and deliberate scope. Rotate session identifiers on authentication and privilege changes. Logout, account suspension, and credential changes must invalidate relevant sessions and active gameplay attachments.

Automated operator authentication, organizational identity-provider integration, and account recovery delivery are open design decisions. Do not silently enable Basic Auth as a fallback or reuse a human's permanent session token for automation.

### Durable identity choice — M4

Choose between PostgreSQL-owned local accounts/sessions and an external identity provider with stable issuer/subject-to-account mappings. This document does not silently select a provider or durable local session policy. Preserve explicit character/world authorization independently of display names and reusable email addresses.

The selected contract must define registry migration, account linking/conflicts, absolute session expiry, revocation, active-connection invalidation, disabled/deleted accounts, restart reauthentication, cleanup bounds, and security-data retention. A migration must not partially duplicate identities or copy credentials into world content/artifacts.

For locally persisted opaque sessions, storing a token verifier/hash rather than the bearer token is a proposed hardening measure; it is not an implemented schema. Password verification and high-entropy token lookup are different mechanisms—do not impose Argon2 on every ordinary session request. Preserve the existing cookie/exact-Origin protections and bounded verification/session resources.

**Example:** policy may allow an unexpired durable session after restart, but the character must attach anew. Test expiry across restart, live revocation, mapping conflicts, disabled accounts, and failed migration without partial identity creation.

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

### Privileged control and audit — M6

Capabilities are scoped by action and environment/world/target. Status viewing does not imply secret access; moderation does not imply arbitrary inventory repair; publication requests do not imply approval or activation; drain does not imply unrestricted SQL. Deny by default and test direct API requests that bypass hidden UI controls.

Specify trusted issuer/identity mapping, required authentication assurance, session/reauthentication policy, revocation propagation, automation credentials/rotation, and time-bounded break-glass approval/review. For a token-based provider, validate signature, issuer, audience, expiry, and provider-specific assurance evidence. A role claim alone does not prove MFA. Do not invent a provider or default privileged credential in implementation.

Every required audit records actor, action/capability, target scope, reason, correlation/operation identity, timestamp, and bounded safe before/after summaries. A database mutation and its audit commit together; audit failure aborts that mutation. External multi-step operations need durable intent/progress/outcome records rather than a claim that all steps share one SQL transaction. Denied attempts go through a bounded security-event path.

Revalidate grants for long-running work before irreversible steps under policy. A committed account restriction denies new admission and converges to live-attachment invalidation even if the initiating operator disconnects. Never include passwords, tokens, raw frames, or unrestricted chat bodies in audit/log records.

**Examples and evidence:** a staging moderator cannot activate production by changing a target ID; a revoked operator cannot finish an unauthorized irreversible step; required-audit failure cannot leave a repaired item committed; insufficient authentication assurance cannot authorize takeover. Exercise cross-scope denial, grant removal, automation rotation, emergency-access expiry, and independent review.

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

The reported implemented commands are `worldwright content validate <path>` and `worldwright play <path>`. M3's server is invoked through `worldwright-server serve`. Check actual flags and paths in their guides/source; this design does not introduce additional executable command names.

M5 adds validation/packaging, manifest inspection, isolated preview, and publication-request workflows; M8 adds improved scaffolding and migration tooling. Local operations need machine-readable diagnostics, stable exit semantics, safe paths, and explicit overwrite/conflict policy. Templates carry approved software/content notices and must be tested from a clean generated project.

Remote publication is a distinct privileged operation, scoped to artifact digest, target, and expected current release. A durable request/status does not imply approval or successful activation. Possession of the CLI does not grant production authority.

### Preview isolation contract — M5

Preview has its own controlled process, disposable state, filesystem area, test identities, and least-privilege credentials. Reject production connection strings/secrets inherited through environment or mounts. Enforce denied production network/control access and disable or redirect external side effects. Bound CPU/memory/storage, concurrent sessions/processes, lifetime, and cleanup work.

A process boundary alone is not a sandbox for untrusted native extensions; only reviewed trusted game code is supported. A preview reward stays in preview and cannot become production value. Test launch/stop, crashes, expired runs, quota rejection, and denied production access independently of the UI.

### Studio

M7 provides schema-aware room/exit/reference-game editing over M5 preview/publication and M6 authority/audit. It does not create a second validator, preview server, or privileged database editor.

| State/artifact | Required behavior |
|---|---|
| Editable draft | Stable entity IDs, explicit dirty/save state, optimistic revision conflict checks |
| Validated snapshot | Full shared validation; diagnostics identify the exact snapshot |
| Packaged artifact | Immutable digest; supported-schema round trip without invented fields |
| Running preview | Displays the pinned snapshot/digest, lifecycle and bounded text output |
| Publication request | Exact reviewed digest, target scope, expected current release and status |

A display-title change must not change entity identity. Deletion identifies incoming/outgoing references rather than silently rewriting them. Import/export and save/reload preserve all supported fields; strict schema v1 still rejects unknown fields. Asynchronous validation cannot be presented as applying to a later unsaved edit. Use the existing validator's JSON-pointer/diagnostic contract; illustrative roadmap diagnostic paths are not a new schema.

After an edit, an existing preview still runs its earlier snapshot until explicitly replaced. Changing packaged bytes invalidates earlier approval. Stale current-release conflicts require refreshed validation/review under policy, not last-write-wins publication. Show submission, approval, staging, and activation as distinct states.

The builder acceptance scenario creates a small zone, encounters/fixes a reference error, previews the exact artifact, handles a publication conflict, and submits the reviewed bytes. Include keyboard-only operation, field labels/error summaries, visible focus, zoom/reflow, non-color-only status, manual assistive-technology checks, and bounded announcements. An ARIA attribute is not completed accessibility evidence.

Destructive confirmations identify target, scope, consequences and recovery path: deleting a draft is not deleting published content. Preserve unsaved edits, restore focus after dialogs, and verify cancellation leaves state unchanged. Studio never uses production database credentials or patches live entity tables.

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

<a id="proposed-schema-history"></a>

### Schema and compatibility status

| Domain | Version | State | Change / migration |
|---|---|---|---|
| Operator configuration | 1 | Proposed, unpublished | TOML target design; current server flags are not evidence of this config schema |
| Content definitions | 1 | Reported implemented in M2 | Strict room/exit schema and deterministic conversion; authoritative contract in `docs/content/SCHEMA_V1.md` |
| Browser protocol | 1 | Reported implemented in M3, development contract | Strict DTOs, in-memory results, stream-local correlation; no durable replay |
| Durable protocol extension | Not assigned | M4 design required | Separate durable key/outcome/reconciliation semantics; no silent v1 reinterpretation |
| Artifact manifest | Not assigned | M5 proposal | Precisely defined payload digest, compatibility and provenance; roadmap JSON is illustrative |
| Game-state schema | Not assigned | M4 design required | Chosen valuable-operation projection and migration policy |
| Database schema | Not assigned | M4 design required | Reviewed constraints, writer authority, receipts, audit, outbox and migrations |
| SDK compatibility policy | Not assigned | M8 stabilization work | Provisional interfaces exist; supported windows/deprecation remain open |

This corrects the older table that called content/protocol v1 unpublished proposals. It does not claim any deployed database migrations or new protocol version. Record actual released schemas and tested upgrade paths in authoritative histories as they exist.

### M4 migration and retention requirements

Applied database migrations are immutable; corrections are new migrations. Test clean installation and upgrades from every supported prior schema on real PostgreSQL. Startup refuses unsupported schemas before admission. Use expand/migrate/contract where a supported compatibility window requires it; do not assume the first local durable slice already has a rolling-upgrade promise.

Classify semantic changes as breaking even when syntax is unchanged. For example, interpreting an ownership field as an account rather than a character requires a reviewed data migration, not just a renamed Rust type. Identity-registry migration must preserve stable mappings or fail with an actionable conflict; no silent account merging.

Receipt, audit, account, chat-if-collected, and network/security data need distinct retention/access/deletion policies. M4 specifies the retry window and preservation of business uniqueness; M6 implements and exercises the broader operating policy. The existing no-chat-body logging rule remains in force—retention decisions do not authorize collecting otherwise prohibited data.

## Failure and Overload Policy

| Condition | Baseline response |
|---|---|
| Invalid command | Structured error; no world mutation |
| Ingress queue full | Reject admission or close abusive connection; never grow without bound |
| Slow client / output queue full | M3 disconnects the saturated/closed route; future coalescing requires an explicit replaceable-update contract, never silent loss of critical results |
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

### Controlled drain and readiness — M6

The roadmap's conceptual progression is `Running → Draining → Quiescent → Stopping → Stopped`. ARCHITECTURE's Ready state corresponds to Running; Quiescent/Stopping are drain/teardown substates, not new claimed enum variants.

1. Mark not-ready and close new admission/attachments as specified.
2. Notify clients through bounded output without waiting indefinitely for readers.
3. Complete or explicitly reject admitted in-memory work under the drain budget.
4. Settle/reconcile the pending durable transaction; do not classify a wait timeout as rollback.
5. Stop new background durable work and preserve committed outbox entries/progress.
6. Close owner, connections, pools and supervisors under the deadline.
7. If forced to terminate, retain the original durable-key reconciliation path and require coherent restart before readiness.

Test idle shutdown, sustained traffic, stalled readers, dependency loss, and uncertainty during COMMIT. Quiescent means no unresolved/admitted mutation remains under policy; forced termination may occur without reaching it. Do not assert safe quiescence merely because the process exited.

M4 readiness includes coherent projection and current write authority; M5 includes activation recovery; M6 includes drain/operational dependency status. Liveness remains distinct so a database outage does not create a useless restart loop. Correlation distinguishes stream/request/runtime identifiers from durable operation, writer, and release identities. Metrics have bounded labels, and alerts require a named owner/action before support commitments are made.

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

Before claiming a feature verified, record its actual revision, toolchain, platform, dependency source, commands and unexecuted checks. Preserve separate historical evidence scopes:

| Evidence | Reported scope | Does not establish |
|---|---|---|
| Earlier Linux core/content/local-play record | Supplied design cites Rust 1.85.0 with provenance limits in `docs/VERIFICATION.md` | Linux Transport completion, hosted CI or production operation |
| Transport v0.1 Windows-local gate | Roadmap reports Rust 1.85.1, 164 unit/integration tests and 2 compile-fail doctests; 32 server library, 2 server smoke, 14 protocol tests and seven real loopback acceptance cases | Counts are not additive; no Linux Transport rerun, durable database, deployment or hosted-CI claim |
| This documentation revision | Supplied-document reconciliation and Markdown checks | No repository compilation, test execution, migration or deployment verification |

The reported Windows gate also includes formatting, compilation, Clippy, build and rustdoc. Do not carry historical counts into a new revision as if rerun. The shared completion commands remain:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked --offline
cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
cargo test --workspace --locked --offline
cargo build --workspace --locked --offline
cargo doc --workspace --no-deps --locked --offline
```

Run from the actual workspace root with the recorded cache/vendor source. Missing offline dependencies are a reported limitation, not a passing gate; record any preparatory retrieval separately without silently changing the lockfile. Persistence acceptance additionally requires real PostgreSQL tests and process-level interruption, not mocks alone. These commands were not run for this documentation update.

### Roadmap failure-coverage crosswalk

| Work package / invariant | Required negative or interruption case | Evidence boundary |
|---|---|---|
| M2–M3 / WW-I05, WW-I07, WW-I11 | Stale queued generation, stale detach, full ingress/output, malformed/duplicate fields, Unicode byte bounds | Deterministic race and real loopback acceptance tests |
| M4.2 / WW-I09 | Competing takeover, transaction already holding fence lock, resumed stale writer | Real PostgreSQL connection/process races |
| M4.4–M4.5 / WW-I01, WW-I03 | Same key/different intent, fresh keys/same entitlement, concurrent first use | Constraints, transaction tests and receipt assertions |
| M4.6–M4.8 / WW-I04 | Required audit failure, COMMIT reply loss, crash before projection, restart before output | Fault injection and process interruption; no false terminal failure |
| M5 / WW-I08 | Changed reviewed bytes, stale expected release, removed durable reference, activation interruption | Artifact/compatibility and staging recovery tests |
| M5/M7 / WW-I06 | Preview reaches production, stale validation/preview result mistaken for current draft | Isolation and workflow tests independent of UI controls |
| M6 / WW-I06, WW-I09, WW-I10 | Insufficient assurance, cross-world target, drain during ambiguity, restored old epoch, duplicate outbox delivery | Authorization tests and independently exercised recovery runbook |
| M8 / WW-I12 | Second game needs transport game-name branch or unsafe fence removal for speed | Reuse/gap audit, downstream compatibility and benchmark regression |

Use ROAD-MAP Appendix A for work-package/verification records and Appendix B for the full failure catalog. Each test maps to an observable invariant and includes actual database/version/isolation, fixture/artifact identity, failure point, outcome, and limitation where applicable.

For test/implementation conflicts, first determine the intended contract. Update the incorrect side and record the reason; do not edit tests solely to obtain a green run.

## Delivery Gates

The expanded [ROAD-MAP.md](ROAD-MAP.md) owns the **eight-milestone** sequence. This supersedes the older six-phase table; it does not renumber existing WW-Dxx/WW-Ixx identifiers or assert changes to the uninspected README.

| Milestone | Design/implementation scope | Evidence and status boundary |
|---|---|---|
| M1 — Foundation and Core | Pinned workspace, typed IDs/contracts, rules/effects, scheduling primitives, deterministic test support | Reported baseline complete; actual CI execution remains distinct from configured workflows |
| M2 — Runtime, Content, and Local Play | Single owner, attachment generations, staged Look/Move/Say, strict JSON v1, named exits and terminal host | Reported complete with earlier Linux provenance limits; no authentication or persistence in Local Play |
| M3 — Multiplayer Transport v0.1 | Bounded protocol, local registry/sessions, owner service, loopback gateway and reference browser | Reported Windows-local verification; Linux Transport rerun, hosted CI and deployment pending |
| M4 — Durable Gameplay v0.1 | Schema/migrations, transactional writer epochs, identity choice, one item/reward operation, scoped receipts, atomic audit/outbox, uncertain outcomes and restart | **Next active, planned**: one valuable operation survives duplicate/failure/restart tests without added value, false success or accepted stale writer |
| M5 — Content Release and Preview v0.1 | Immutable manifests, package/compatibility checks, staging/maintenance activation, known-good recovery, isolated preview, CLI publication requests | Planned: reviewed exact artifact previews/activates in staging; incompatible/stale/tampered candidates fail; supported rollback demonstrated |
| M6 — Operational Control v0.1 | Capability-scoped APIs, privileged assurance, transactional audit, observability, drain/shutdown, backup/restore/takeover runbooks | Planned: unauthorized changes fail, allowed changes are attributable, documented restore produces a coherent playable world |
| M7 — Builder Workflow and Studio v0.1 | Schema-aware editing, shared diagnostics, M5 preview and M6-controlled publication integration, accessibility and destructive-action handling | Planned: builder creates/validates/previews a zone and submits exact reviewed bytes without production credentials |
| M8 — Toolkit Maturity | Independent second game, compatibility/deprecation, justified gameplay systems, scaffolding/migrations, measured performance | Planned: reuse without game-specific transport/core-invariant branches; upgrade/tooling/performance evidence |

### Handoffs and non-goals

M4 consumes M3 without weakening authority, queue limits, or text safety. M5 consumes durable-reference compatibility and writer control. M6 consumes transaction and release mechanisms. M7 consumes shared validation, isolated preview/publication, authorization and audit rather than duplicating them. M8 uses real independent reuse to stabilize contracts.

M4 is one valuable-state slice, not a requirement for all inventory/combat/progression systems. M5 delivers preview infrastructure before Studio and does not certify production. M6 does not imply a completed Console interface simply because control APIs exist. M8 is not a promise of active-active simulation, untrusted plugins, or numerical capacity without measurement.

Security starts with the earliest affected slice; it is not deferred to M6. Design, fixtures, and UI research may overlap milestones, but activation must respect dependency gates. Licensing/governance approval remains an external-publication decision rather than a retroactive assertion that a completed local foundation has legal approval.

Use roadmap statuses: planned, in progress, blocked, implemented awaiting verification, locally verified, and verified for the stated environment. Every transition names revision/evidence, scope, remaining limitations, decisions, and migration impact. No new milestone is marked complete by this document.

## Open Decisions

| ID | Decision still required | Accountable role to assign | Gate |
|---|---|---|---|
| WW-O01 | Copyright owner, final license, inbound terms and template/content rights | Authorized project owner | External publication/contributions; no approval inferred from local M1 completion |
| WW-O02 | Supported toolchain/platform/dependency matrix beyond reported Rust 1.85.1 bootstrap and earlier verification exceptions | Engineering lead | Platform/CI claims and future supported releases |
| WW-O03 | Local durable identity versus provider, stable account mapping, recovery, privileged MFA and automation | Security/identity owner | M4 identity implementation; M6 privileged exposure |
| WW-O04 | New durable protocol semantics, future content/manifest extensions, provisional SDK evolution | Engineering/tooling leads | Before introducing new contracts; M2 content v1 and M3 protocol v1 already reported implemented |
| WW-O05 | PostgreSQL schema, constraints, isolation/durability, supported deployment, migrations, authority/takeover and restore safety | Engineering/data/service owners | M4 durable acceptance; M6 restore |
| WW-O06 | Receipt retry window/cleanup, audit/account/network-data retention, chat privacy and deletion/access rules | Security/privacy/product owners | M4 receipt guarantees; M6 operating policy |
| WW-O07 | Hosting, production domains/ports, edge/TLS/proxy trust, supervisor and secret provider/rotation | Service/security owner | Non-loopback deployment; production gate |
| WW-O08 | Capacity/workload/latency targets, RPO/RTO measurement boundaries and support coverage | Product/service owners | M6 operational go/no-go; M8 capacity/support claims |
| WW-O09 | First valuable operation and necessary persistent/ephemeral projection; meaningful second-game choice | Game design/product owner | M4 scope; M8 reuse exercise |
| WW-O10 | Studio/Console packaging and draft storage model | Tooling/service owners | M6/M7 interface implementation; no service split implied |
| WW-O11 | SDK/protocol supported windows, source compatibility, deprecation and migration policy | Maintainers | Public compatibility claims; M8 stabilization |
| WW-O12 | Policy adoption, maintainers, reporting/appeals, vulnerability response and service ownership | Project/security/community owners | External collaboration, beta operations and support commitments |

### Roadmap owner-decision crosswalk

Roadmap D01–D07 group ownership decisions; WW-Dxx records design rationale; WW-Oxx tracks unresolved choices. Preserve these namespaces rather than treating identical numbers as equivalent.

| Roadmap decision | Related open decisions | Required output |
|---|---|---|
| D01 — Ownership/licensing/contributions | WW-O01, WW-O12 | Authorized policies and scoped reuse/contribution rights |
| D02 — Deployment/secrets | WW-O07 | Reviewed topology, certificate/proxy/secret controls and deployment tests |
| D03 — Identity/privileged assurance | WW-O03 | Stable identity lifecycle, migration and assurance policy with tests |
| D04 — PostgreSQL/recovery | WW-O05, recovery portion of WW-O08 | Schema/fence/migration decision and measured restore-safe recovery plan |
| D05 — Retention/data handling | WW-O06 | Retry window, retention/access/deletion schedule and safe cleanup tests |
| D06 — Compatibility/deprecation | WW-O04, WW-O11; supported-toolchain input from WW-O02 | Supported-version matrix, migration/deprecation tests and documentation |
| D07 — Service/support/vulnerability reporting/capacity | WW-O08, WW-O12 | Named operating model, reporting routes and workload-backed capacity evidence |

WW-O09 and WW-O10 additionally resolve implementation/product scope within those gates. Role names here are responsibilities to assign, not named approvers or evidence of staffing.

Decision closure records context, alternatives, selected outcome, accountable approval, date, consequences, compatibility/migration impact, and linked tests/docs. Until approved, roadmap examples remain proposals. Changes that move/split scope update the roadmap chart and both companion documents in the same pass.

## Maintenance and Licensing

- Keep DESIGN synchronized with behavioral contracts, schemas, migration/retry semantics and rationale.
- Keep ARCHITECTURE synchronized with ownership, dependencies, placement, deployment and trust boundaries.
- Keep ROAD-MAP synchronized when a milestone starts, completes, splits, reorders, blocks, or is abandoned; use its work-package and evidence templates.
- Keep README build/run/configure/use instructions and IMPLEMENTATION source/status mapping current with actual code.
- Update `docs/PROTOCOL_V1.md`, `docs/SERVER.md`, schema histories and runbooks when their real contracts change; do not manufacture new endpoints or fields from examples.
- Record actual platform, revision, dependency source, commands/results and unexecuted checks in `docs/VERIFICATION.md`.
- Keep [SECURITY.md](SECURITY.md), [CONTRIBUTING.md](CONTRIBUTING.md), and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) aligned; drafted policy is not adopted, staffed enforcement.
- Use [DEFINITIONS.md](DEFINITIONS.md) for terminology and keep software, templates, sample content and branding license scopes explicit.

### Revision 0.2 alignment record

This edition replaces six-phase delivery with the expanded roadmap's M1–M8 sequence; corrects content/protocol v1 status and historical verification scope; separates stream-local correlation from durable keys; expands transaction/fencing/restart and restore authority; binds review/publication to immutable bytes; places preview in M5 and Studio integration in M7; and maps owner decisions without renumbering WW-Dxx/WW-Ixx/WW-Oxx identifiers.

Existing local content example and transport/auth limits are retained as supplied-document facts. No new runtime/API/schema implementation, provider, production port, compatibility window, legal decision, or recovery target is selected by this update. Only DESIGN and ARCHITECTURE are revised here; other referenced repository files were not inspected or modified. Source/build/integration/deployment verification remains a separate implementation task.

Before merging, verify actual source contracts, resolve documentary disagreements, classify breaking changes and migrations, keep diagrams/status consistent, and distinguish historical local evidence from hosted CI and production checks. Use observables, not “works” language, for exit gates.

The README's **MIT OR Apache-2.0 recommendation remains pending authorized approval**. This document does not create license files, grant rights, establish ownership, or change that status.

**Baseline success:** a small persistent world that is understandable, testable, recoverable, and reusable—not a collection of disconnected tools with impressive names.
