# Worldwright

**Build worlds. Write the rules.**

Worldwright is a proposed Rust-based development suite for creating, testing, deploying, and operating **MUDs—multi-user domains**: persistent, multiplayer, primarily text-driven worlds.

The effort brings together an authoritative game engine, reusable Rust libraries, world-building tools, developer utilities, automated testing, and operational interfaces. Its purpose is to help teams build complete games without repeatedly rebuilding the foundations of networking, world simulation, content management, persistence, and live-service support.

Worldwright is **a toolkit for building games, not a single game or setting**. A science-fiction frontier, a fantasy realm, and a roleplay-focused social world should be able to share the same technical foundation while retaining their own rules, content, and identity.

> **Project status:** Milestones 1–2 provide the reported foundation, runtime, strict JSON content loading, and Local Play baseline. **Milestone 3 — Multiplayer Transport v0.1** is the latest implemented increment, with reported Windows-local verification: authenticated, process-local two-player browser/WebSocket play, bounded sessions, exact-origin checks, and takeover-safe attachments. Linux Transport rerun, hosted CI, and deployment verification remain pending. **Milestone 4 — Durable Gameplay v0.1 is next active, but still planned**; no PostgreSQL-backed gameplay, durable sessions, restart recovery, public deployment, or built-in TLS termination is claimed. See [road map](ROAD-MAP.md), [implementation handoff](IMPLEMENTATION.md), [Local Play](docs/LOCAL_PLAY.md), [protocol v1](docs/PROTOCOL_V1.md), [server guide](docs/SERVER.md), and [verification](docs/VERIFICATION.md).
>
> **Documentation alignment — October 21, 2026:** This README is aligned with the expanded eight-milestone roadmap and revised DESIGN/ARCHITECTURE. Existing commands and implementation claims are carried forward from the supplied documents; this documentation update did not change source code or rerun repository tests.
>
> **License status:** MIT OR Apache-2.0 is recommended, pending approval by the authorized copyright owner. This README does not itself grant that license or establish ownership. See [Licensing](#licensing).

## Contents

- [Mission](#mission)
- [Who Worldwright Is For](#who-worldwright-is-for)
- [The Worldwright Suite](#the-worldwright-suite)
- [Planned Capabilities](#planned-capabilities)
- [Architecture and Technical Direction](#architecture-and-technical-direction)
- [Proposed Repository Layout](#proposed-repository-layout)
- [Development Workflow](#development-workflow)
- [Organization and Responsibilities](#organization-and-responsibilities)
- [Delivery Roadmap](#delivery-roadmap)
- [Getting Started](#getting-started)
- [Contributing](#contributing)
- [Security and Responsible Operations](#security-and-responsible-operations)
- [Documentation](#documentation)
- [Licensing](#licensing)
- [Long-Term Vision](#long-term-vision)

## Mission

Provide a reliable, understandable, and extensible foundation for building persistent multiplayer worlds in Rust.

Worldwright aims to make it easier for developers, designers, builders, testers, and operators to work together across the full game lifecycle:

**Design → Build → Validate → Playtest → Publish → Operate → Improve**

The development effort follows six principles:

1. **Game identity belongs to the game.** Keep setting, lore, balance, and progression separate from reusable engine infrastructure.
2. **The server owns the rules.** Clients submit intentions; authoritative server logic validates actions and updates world state.
3. **Content is a maintained product.** Rooms, quests, NPCs, and item definitions need validation, versioning, review, and safe publication.
4. **Correctness comes before scale.** Protect player progress and item ownership before adding distributed-world complexity.
5. **Operations are part of development.** Recovery, moderation, observability, and deployment are product capabilities, not afterthoughts.
6. **Claims require evidence.** Document proposed, implemented, and verified behavior separately.

## Who Worldwright Is For

| Audience | Intended value |
|---|---|
| Rust developers | Typed APIs and reusable infrastructure for custom game systems |
| Game designers | A foundation for defining rules, progression, encounters, and interactions |
| World builders and writers | Tools for creating and validating connected, playable content |
| Test engineers | Repeatable scenarios, simulated players, and persistence verification |
| Server operators | Deployment visibility, recovery procedures, and controlled administration |
| Community moderators | Permission-scoped tools for reports, conduct enforcement, and investigation |
| Small studios and community teams | A shared workflow connecting engineering, content, and live operations |

Initial development should focus on a small playable reference world rather than attempting to satisfy every possible multiplayer genre at once.

## The Worldwright Suite

The following components define the intended product family. They are responsibility boundaries; they do not require separate services or repositories at the outset.

| Component | Purpose | Initial direction |
|---|---|---|
| **Worldwright Engine** | Authoritative world simulation, entities, command execution, scheduling, and game-system integration | Modular server runtime |
| **Worldwright SDK** | Rust interfaces for commands, systems, content, and controlled extensions | Reusable library crates |
| **Worldwright Studio** | Authoring and preview of rooms, zones, NPCs, items, quests, and dialogue | Start with validated files; add an editor after the content model stabilizes |
| **Worldwright CLI** | Project scaffolding, content checks, development utilities, and build orchestration | One developer-facing command-line entry point |
| **Worldwright Testkit** | Scenario tests, simulated players, repeatable fixtures, and replay diagnostics where supported | Test support integrated with the core runtime |
| **Worldwright Console** | Server health, session visibility, moderation, and authorized administration | Restricted operator interface |

**Naming convention:** Use descriptive component names under the Worldwright umbrella rather than unrelated brands for each tool.

## Planned Capabilities

This is the target product scope, not a list of features all present today. M1–M3 already provide typed core/runtime contracts, strict room/exit content loading, Local Play, and a bounded authenticated loopback browser slice. Inventory/progression, durable state, immutable release/preview workflows, operational administration, and Studio remain later milestone work. See [Delivery Roadmap](#delivery-roadmap) for status and dependencies.

### World and Gameplay Foundations

- Rooms, exits, zones, objects, characters, and non-player characters.
- Stable identifiers separating reusable definitions from live instances.
- Typed commands with explicit validation and permission checks.
- Movement, observation, communication, inventory, and basic interactions.
- Scheduling for combat rounds, cooldowns, effects, and NPC actions.
- Extension points for game-specific combat, quests, progression, and economies.
- Separation of account identity, character identity, and connection sessions.

### Content Development

- Versioned definitions for world content.
- Validation of references, required fields, navigation, and basic rule constraints.
- Preview environments isolated from production.
- Reviewable changes and identifiable content releases.
- Controlled publication with documented compatibility and recovery behavior.
- Explicit safety limits for any future scripting support.

### Player Access

- A browser-accessible, keyboard-first text interface.
- Secure WebSocket communication for interactive play.
- HTTPS account and session management.
- Clear command feedback, help, and reconnect behavior.
- Screen-reader-conscious output and optional color formatting.
- A possible traditional-client gateway after browser access is stable; compatibility is not an initial guarantee.

### Persistence and Operations

- Durable character state and progression.
- Transactional handling of valuable items and currency.
- Explicit treatment of retries, disconnects, and process failures.
- Structured logs, health checks, and operational metrics.
- Tested backups, restoration, and deployment procedures.
- Least-privilege administration and audited moderator actions.

### Deliberately Outside the Initial Scope

- Seamless multi-region active-active worlds.
- Arbitrary player- or builder-supplied native plugins.
- A universal visual editor before the underlying content model is proven.
- Real-money trading or payment systems.
- Unlimited procedural content generation.
- Guaranteed support for every legacy MUD protocol.
- Production capacity or availability claims without measured evidence.

## Architecture and Technical Direction

### Reference Stack

The table separates reported local implementation from target architecture. Detailed decisions and boundaries are in [DESIGN.md](DESIGN.md) and [ARCHITECTURE.md](ARCHITECTURE.md).

| Area | Technology or approach | Status |
|---|---|---|
| Primary language | Rust | Reported workspace baseline |
| Asynchronous I/O | Tokio | Integrated for the local Transport slice |
| HTTP and WebSocket service | Axum | Integrated for loopback development, not a public deployment |
| Relational persistence | PostgreSQL | Planned for M4 |
| Database access | SQLx | Planned; not integrated in the reported baseline |
| Web surfaces | Static HTML/CSS/JavaScript player client; Askama for future server-rendered surfaces | Local player client implemented; Askama deferred |
| World content | Strict JSON schema v1; later immutable content-release artifacts | Loader/validation implemented in M2; packaging/activation/preview planned for M5 |
| Server configuration | Versioned TOML with validated settings and secret references | Proposed; existing CLI settings do not establish this configuration schema |
| Deployment | Single authoritative process initially; private database for durable operation | M3 is process-local and database-free; supported production topology remains undecided |
| Packaging | Reproducible binaries or containers | Deployment packaging and verification remain planned |

The workspace pins Rust 1.85.1, uses edition 2024, and versions packages as `0.1.0-dev.0`. Content loading uses pinned Serde, serde_json, and serde_path_to_error with a committed lockfile. Tokio, Axum, Argon2, getrandom, and futures are installed for the development server; SQLx/PostgreSQL and Askama remain planned rather than integrated.

### Runtime Model

Use a **modular monolith** and one authoritative owner of mutable simulation state. Keep the system internally modular without introducing distributed services solely to match component names.

The diagram is the **target architecture across multiple milestones**, not the current runnable composition. M3 has no PostgreSQL, reviewed-release activation, operational Console, or integrated durable scheduler. Its loopback gateway does not terminate TLS itself. Database work begins in M4, release/preview in M5, and scoped operational control in M6.

```text
Player Browser
    |
    | HTTPS / secure WebSocket
    v
Transport and Session Layer
    |
    | Authenticated, bounded command messages
    v
Authoritative World Runtime
    |-- Command validation and game rules
    |-- World entities and scheduled events
    |-- Content definitions and game-specific systems
    |
    | Explicit persistence operations
    v
PostgreSQL

Builder Tools --> Validation --> Reviewed Content Release --> World Runtime

Operator Console --> Authorization --> Audited Administrative Commands
```

Network handlers should not freely mutate shared world structures. Slow clients must not create unbounded memory growth or stall unrelated players.

For valuable state, define transaction and recovery boundaries before acknowledging success. A single simulation owner helps coordinate activity, but it does not replace database transactions or crash-recovery design.

### Engine, Tools, and Games

Keep three layers distinguishable:

- **Engine and SDK:** Reusable runtime mechanisms and contracts.
- **Development tools:** Authoring, validation, tests, and operator workflows.
- **Individual games:** Setting, world content, game-specific rules, and player-facing identity.

A game may use only part of the suite. Studio and Console should use documented interfaces rather than bypassing engine invariants with unrestricted database writes.

## Proposed Repository Layout

The application and library directories contain the content, runtime, protocol, and local transport increments. `worldwright-server` and `worldwright-protocol` are implemented development slices; Studio, Console, and persistence remain stubs. Cargo.lock is supplied. The local workflows live in `scripts/`, CI in `.github/workflows/ci.yml`, and the implemented boundaries are documented in [CORE_INTERFACES.md](docs/CORE_INTERFACES.md), [PROTOCOL_V1.md](docs/PROTOCOL_V1.md), and [SERVER.md](docs/SERVER.md). Other deployment/content directories remain guidance placeholders.

```text
worldwright/
├── Cargo.toml
├── Cargo.lock                  # Cargo-generated; keep committed
├── scripts/                    # dev.sh and dev.ps1
├── .github/workflows/ci.yml     # Linux/Windows checks
├── rust-toolchain.toml
├── .cargo/config.toml
├── README.md
├── ROAD-MAP.md                 # Milestones, dependencies, decisions and exit gates
├── IMPLEMENTATION.md           # Latest implementation, next work and historical evidence
├── DESIGN.md
├── ARCHITECTURE.md
├── SECURITY.md
├── CONTRIBUTING.md
├── CODE_OF_CONDUCT.md
├── DEFINITIONS.md
├── apps/
│   ├── worldwright/             # Developer CLI
│   ├── worldwright-server/      # Authoritative server binary
│   ├── worldwright-studio/      # Builder application, when introduced
│   └── worldwright-console/     # Operator application, if separately packaged
├── crates/
│   ├── worldwright-core/        # Domain types and reusable simulation logic
│   ├── worldwright-content/     # Content schemas and validation
│   ├── worldwright-protocol/    # Versioned transport message contracts
│   ├── worldwright-persistence/ # Database mappings and durable operations
│   ├── worldwright-sdk/         # Supported game-development interfaces
│   └── worldwright-testkit/     # Test fixtures and scenario helpers
├── examples/                   # Small reference games and sample content
├── migrations/                 # Ordered database migrations
├── tests/                      # Cross-component verification
├── docs/                       # Guides, decisions, and operating procedures
└── deploy/                     # Deployment definitions and environment examples
```

Application binaries live under `apps/`. Each application's directory, Cargo package, and executable names are aligned. The CLI package and binary are both named `worldwright`.

License files should be added after the licensing decision is approved; the recommended filenames are described below.

## Development Workflow

The intended workflow is:

1. **Define the game.** Establish the setting, audience, core loop, and first playable milestone.
2. **Create a project.** Use the CLI scaffolding once implemented, or a documented reference project.
3. **Author a small world.** Build a few connected rooms, an NPC, an item, and a complete interaction.
4. **Validate content.** Check identifiers, references, constraints, and schema compatibility.
5. **Implement game rules.** Add focused systems through supported Rust interfaces.
6. **Run repeatable tests.** Verify actions, authorization, ownership, and recovery behavior.
7. **Package and preview.** In M5, identify immutable bytes with a manifest/digest and test that exact artifact in an isolated environment; Studio integration follows in M7.
8. **Request controlled publication.** Bind review to the tested digest and target environment, check the expected current release and compatibility, then use the authorized staging/activation workflow. A save or successful validation is not publication approval.
9. **Operate and improve.** Record code, content, configuration and migration versions together; use M6 authorization, audit, monitoring, drain and recovery procedures before operational exposure.

This is a target workflow. Existing content-validation, Local Play and local Transport commands are documented under [Getting Started](#getting-started). Scaffolding, packaging, preview, publication and administration commands must not be inferred from these steps; document their actual syntax only when implemented and verified. Changed artifact bytes require refreshed review, and a supported content rollback must account for durable-state/schema compatibility.

## Organization and Responsibilities

Worldwright needs ownership across engineering, content, verification, and operations. One person may fill several roles, but responsibilities should remain explicit.

| Responsibility | Accountable role |
|---|---|
| Product scope and priorities | Project or product lead |
| Runtime architecture and SDK contracts | Engineering lead |
| Developer experience and tooling | Tooling maintainer |
| Reference game rules and content model | Game design/content lead |
| Testing and release evidence | Verification owner |
| Deployment, recovery, and monitoring | Service owner |
| Authentication, authorization, and vulnerability handling | Security owner |
| Community standards and moderation workflows | Community lead |
| Copyright, dependency licensing, and release authority | Authorized project owner, with legal review as needed |

Maintainers should record architectural decisions, review public API changes, and define a release approval process. Production access should be granted by responsibility, not automatically to every contributor.

## Delivery Roadmap

[ROAD-MAP.md](ROAD-MAP.md) is authoritative for the **eight dependency-ordered milestones**. This summary replaces the earlier six-phase outline. [IMPLEMENTATION.md](IMPLEMENTATION.md) records the latest implemented slice, next work, and increment-specific historical evidence.

| Milestone | Focus | Status and exit evidence |
|---|---|---|
| **M1 — Foundation and Core** | Workspace, typed identifiers/contracts, rules/effects, scheduling primitives, deterministic tests | Reported complete baseline; configured CI is not proof of successful hosted runs |
| **M2 — Runtime, Content, and Local Play** | Single-owner runtime, attachment generations, staged Look/Move/Say, strict JSON v1, named exits, local terminal host | Reported complete with earlier Linux evidence and provenance limits |
| **M3 — Multiplayer Transport v0.1** | Bounded protocol, local registry/sessions, loopback HTTP/WebSocket gateway, browser client | Implemented with reported Windows-local verification; Linux Transport rerun, hosted CI and deployment remain pending |
| **M4 — Durable Gameplay v0.1** | PostgreSQL schema/migrations, writer fencing, identity decision, one valuable operation, scoped receipts, atomic audit/outbox, uncertain-outcome recovery | **Next active, planned:** one item/reward operation survives duplicate, concurrency, failure and restart tests without duplicate value, false success or accepted stale writers |
| **M5 — Content Release and Preview v0.1** | Immutable manifests/artifacts, compatibility, staging/maintenance activation, supported rollback, isolated preview, CLI publication requests | Planned: exact reviewed/tested artifact activates in staging; stale/incompatible artifacts and production access from preview are refused |
| **M6 — Operational Control v0.1** | Capability-scoped control, privileged assurance, audit, observability, graceful drain, backup/restore and takeover runbooks | Planned: unauthorized actions fail; allowed changes are attributable; documented restore produces a coherent playable world |
| **M7 — Builder Workflow and Studio v0.1** | Schema-aware editing, shared diagnostics, M5 preview/release integration, M6-controlled publication, accessibility | Planned: builder creates, validates and previews a zone, then submits exact reviewed bytes without production database credentials |
| **M8 — Toolkit Maturity** | Independent second game, SDK compatibility/deprecation, justified gameplay expansion, scaffolding/migrations, measured performance | Planned: reuse without game-specific transport/core-invariant branches; upgrade and performance evidence |

### Next implementation priority

M4 is a narrow durable vertical slice, not a commitment to implement all inventory, combat, progression or economic systems at once. Select the first item/reward operation, decide local durable identity versus identity-provider integration, and review transaction/schema/fencing and recovery contracts before coding. Preserve the existing local commands and M3 authority, bounds, origin and text-safety guarantees.

Protocol v1 request IDs remain stream-local correlation; `accepted` means admission and `command_result` means in-memory execution. Durable idempotency, receipt lookup and reconnect reconciliation require explicit new contracts, not silent changes to those meanings. Planned database work does not make current sessions or gameplay restart-safe.

### Dependency and release boundaries

M5 consumes M4's durable-reference and writer contracts. M6 consumes transaction and release mechanisms. M7 consumes shared validation, isolated preview and operational controls rather than introducing a second validator or direct database editing. M8 uses a genuinely independent second game to distinguish reusable infrastructure from first-game assumptions.

Design and test-fixture work can overlap milestones, but activation cannot bypass prerequisite gates. Security applies from the earliest affected increment. Neither M3 local success nor M5 staging activation constitutes production readiness. Licensing, owner adoption, identity, retention, deployment and measured recovery targets remain explicit decisions; no legal approval, production port or service-level target is selected by this roadmap summary.

## Getting Started

Extract the framework archive and open its `worldwright/` directory. Local content validation and Local Play need no database or external service. Transport development additionally uses an untracked local account registry; see [SERVER.md](docs/SERVER.md).

The workspace selects **Rust 1.85.1**, edition **2024**, and locked local/registry dependencies. Install that toolchain with rustfmt and Clippy through your approved tooling process; `rust-toolchain.toml` declares it for rustup. See the [scaffold guide](docs/SCAFFOLD.md) and [development guide](docs/DEVELOPMENT.md) for installation details.

**Repeatable local workflow:**

```bash
bash scripts/dev.sh bootstrap
bash scripts/dev.sh check
```

On Windows with PowerShell 7:

```powershell
./scripts/dev.ps1 bootstrap
./scripts/dev.ps1 check
```

The full check runs formatting, all-target compilation, Clippy, tests, debug build, and API documentation. `fmt`, `test`, `docs`, and `doctor` are also available. See [DEVELOPMENT.md](docs/DEVELOPMENT.md) for behavior and [ci.yml](.github/workflows/ci.yml) for Linux/Windows CI.

Cargo.lock is supplied and was generated by Cargo. Keep it committed. Bootstrap installs the pinned toolchain when rustup is available and runs `cargo fetch --locked`; subsequent checks use locked, offline resolution. Both initial dependency fetching and toolchain installation may need network access. The current Transport gate passed on Windows Rust 1.85.1; older Linux increments have evidence, while a Linux Transport rerun, hosted CI, and approved registry-artifact acceptance remain pending as detailed in the verification record.

Validate the editable reference world:

```bash
cargo run -p worldwright --locked --offline -- content validate examples/two-room/world.json
cargo test -p worldwright-testkit --test content_runtime --locked --offline
```

The CLI returns 0 for validated content and successful builder conversion, 1 for file/content errors, and 2 for usage errors. Validation does not launch a game. See [schema v1](docs/content/SCHEMA_V1.md).

Run the local two-room execution tests or the focused walkthrough:

```bash
cargo test -p worldwright-testkit --test runtime --locked --offline
cargo test -p worldwright-testkit --test runtime --locked --offline two_room_look_move_say_walkthrough -- --nocapture
```

Play the same JSON world interactively:

```bash
cargo run -p worldwright --locked --offline -- play examples/two-room/world.json
```

Use `look` (`l`), `go north`, `say Hello`, `help`, and `quit` (`exit`). EOF also ends cleanly. The starting room appears automatically; successful movement is followed by a runtime-backed Look. Piped input is supported without prompts. This is a single-player, in-memory terminal host. See [LOCAL_PLAY.md](docs/LOCAL_PLAY.md) for its trust boundary, then [SERVER.md](docs/SERVER.md) for authenticated transport setup.

For the authenticated local transport, generate one hash per user without passing passwords as arguments, create the untracked `accounts.local.json` schema from [SERVER.md](docs/SERVER.md), then run:

```text
cargo run -p worldwright-server --locked --offline -- hash-password
cargo run -p worldwright-server --locked --offline -- serve --content examples/two-room/world.json --accounts accounts.local.json --origin http://127.0.0.1:3000 --allow-insecure-http
```

Visit `http://127.0.0.1:3000`. The insecure flag is local development only; keep account registries and password hashes untracked.

Inspect application status separately:

```bash
cargo run -p worldwright --locked --offline -- --help
cargo run -p worldwright-server --locked --offline -- status
cargo run -p worldwright-studio --locked --offline -- status
cargo run -p worldwright-console --locked --offline -- status
```

All four applications support `--help`/`-h`, `--version`/`-V`, and `status`; the server additionally supports `hash-password` and `serve`. The developer CLI additionally supports `content validate <path>` and `play <path>`. Invalid command shapes exit with code 2. Server runtime/startup failures exit with code 1, and `status` is not a service-readiness check. Studio, Console, and persistence remain stubs.

Generate local account hashes and start the development transport with the exact workflow in [SERVER.md](docs/SERVER.md). The server is loopback-only, process-local, and intentionally not a public deployment or in-process TLS service. See [docs/RUNTIME.md](docs/RUNTIME.md), [docs/PROTOCOL_V1.md](docs/PROTOCOL_V1.md), [docs/SCAFFOLD.md](docs/SCAFFOLD.md), and [docs/VERIFICATION.md](docs/VERIFICATION.md). Persistence is the next major architecture step.

## Contributing

The proposed [contribution and governance guide](CONTRIBUTING.md) covers engineering, content, documentation, accessibility, testing, and operations. The [Code of Conduct](CODE_OF_CONDUCT.md) defines proposed community expectations and conflict-aware reporting. These drafts still require owner adoption, named maintainers, and working reporting channels.

Expected standards:

- Discuss significant features and architectural changes before implementation.
- Ground changes in actual source, interfaces, and documented contracts.
- Keep work scoped and reviewable.
- Add tests for intended behavior, failure paths, and regressions.
- Update affected documentation in the same change as code or content.
- State exactly which checks were run and which were not.
- Preserve source and asset provenance; do not import material without appropriate rights.
- Never commit passwords, private keys, access tokens, or production player data.
- Submit contributions only when authorized to license them under the project's approved terms.

CONTRIBUTING.md recommends an inbound-equals-outbound model and Developer Certificate of Origin 1.1 sign-off, both pending owner approval. Before accepting external contributions, publish the adopted inbound terms and DCO/contributor-agreement decision. A sign-off is an attestation of contribution rights; it is not a copyright transfer or a substitute for the project's license terms.

## Security and Responsible Operations

Security and player safety apply to every milestone, not only operational beta.

**Reported M3 development safeguards:**

- Server-side local-account authentication, authorized character attachment and execution-time generation checks.
- Fixed-profile Argon2id password hashes and bounded process-local sessions.
- Exact-Origin checks on login, logout and WebSocket upgrade; no wildcard authenticated CORS. CORS alone is not authentication or CSRF defense.
- Bounded input, queues, expensive password-verification work and connection/output paths; saturated critical output routes disconnect.
- Text-safe browser rendering, a CSP-compatible client and sanitized logging that excludes credentials, raw gameplay frames and chat bodies.
- Loopback-only service; the documented plain-HTTP option is explicit local development, not a public deployment mode. Secure-cookie/HTTPS-origin defaults do not supply built-in TLS termination.

**Still planned or requiring owner decisions and verification:**

- Durable identity/session or identity-provider integration and documented revocation/restart behavior in M4.
- Scoped privileged operations, stronger privileged authentication policy, required audit, observability and exercised recovery controls in M6.
- Supported production TLS termination, trusted proxy policy, certificate lifecycle and secret delivery before public exposure.
- Protected backups, restore/takeover drills, restore-safe writer authority, and approved recovery targets.
- Retention and access rules for accounts, receipts, chat, audit, outbox and backups; retention choices must precede the affected durable storage/cleanup behavior.
- A designated private vulnerability-reporting channel, named owners and an adopted support policy before public release.

Do not use a public issue to disclose credentials, sensitive player information, or an unmitigated exploit. [SECURITY.md](SECURITY.md) provides the proposed reporting process and explicitly marks private contact routes, support coverage, and supported software versions as not yet established; no address or operational service has been invented.

## Documentation

This package retains the foundational policy documents and now includes the implemented local transport guides: [ROAD-MAP.md](ROAD-MAP.md), [docs/PROTOCOL_V1.md](docs/PROTOCOL_V1.md), [docs/SERVER.md](docs/SERVER.md), and [docs/VERIFICATION.md](docs/VERIFICATION.md). Runtime, content, and authenticated transport are implemented only within their documented local boundaries; persistence and public deployment remain future work. Contribution, conduct, and security policies still require owner adoption and real reporting contacts. Deeper documentation directories contain guidance placeholders, not completed operational procedures.

| Document or area | Intended purpose |
|---|---|
| [README.md](README.md) | Project overview, current local build/run commands, limitations and concise delivery summary |
| [ROAD-MAP.md](ROAD-MAP.md) | Eight milestones, detailed work packages, dependencies, owner decisions, failure scenarios and exit gates |
| [IMPLEMENTATION.md](IMPLEMENTATION.md) | Latest implemented increment, next planned work, concrete handoff and scoped historical evidence |
| [DESIGN.md](DESIGN.md) | Behavioral decisions, invariants, state ownership, durable/release contracts, compatibility and decision crosswalks |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Component ownership, crate dependencies, runtime/data/control flows, milestone boundaries and lifecycle |
| [SECURITY.md](SECURITY.md) | Proposed security/reporting policy, support status, threat model, privacy, and release gates |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Proposed governance, contribution workflow, review standards, inbound terms, and DCO recommendation |
| [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) | Proposed conduct expectations, private reporting, conflict handling, and appeals |
| [DEFINITIONS.md](DEFINITIONS.md) | Detailed MUD, MUSH, MUCK, MUX, MOO, and MU* comparisons, glossary, and compatibility boundaries |
| `docs/decisions/` | Architecture decision records |
| `docs/content/` | World schemas, builder guidance, and publication procedures |
| `docs/testing/` | Scenario design, fixtures, invariants, and verification procedures |
| `docs/operations/` | Deployment, monitoring, backup, restoration, and incident runbooks |

Behavioral changes, public contracts, schema changes, and deployment changes should update the relevant documentation in the same pull request.

## Licensing

### Proposed Recommendation: MIT OR Apache-2.0

For broad adoption of the engine and developer tools, the recommended software license expression is **`MIT OR Apache-2.0`**.

This means a recipient may choose either license. It does **not** mean the recipient must satisfy both licenses simultaneously.

Both licenses permit commercial use and allow proprietary applications and modifications, subject to their respective conditions. Neither requires downstream users to contribute improvements back. Apache-2.0 includes an explicit, limited contributor patent grant and patent-litigation termination provisions; MIT does not contain equivalent express provisions. Because dual licensing permits the MIT alternative, choose **Apache-2.0 alone** if requiring Apache's terms for every recipient is a project objective.

Authoritative terms:

- [MIT License — Open Source Initiative](https://opensource.org/license/mit)
- [Apache License, Version 2.0 — Apache Software Foundation](https://www.apache.org/licenses/LICENSE-2.0)

If preserving source availability for distributed changes to covered Worldwright files is more important than maximum permissive reuse, evaluate [MPL-2.0](https://www.mozilla.org/en-US/MPL/2.0/FAQ/) instead. MPL is file-level copyleft; it permits separate proprietary code but imposes source obligations for covered software when distributed. Hosting a modified server alone generally does not trigger those distribution obligations.

### Separate Software, Content, and Branding

| Material | Recommended treatment |
|---|---|
| Engine, SDK, CLI, Studio, Console, and Testkit source | Selected software license, consistently applied |
| Starter code copied into new projects | Explicit permissive terms compatible with commercial and proprietary games |
| Code examples in documentation | Clearly include under the software license |
| Sample worlds, stories, artwork, audio, and other assets | Explicit per-asset or per-package terms, recorded alongside the material |
| Third-party dependencies and imported content | Preserve and comply with their own licenses and notices |
| Games created using Worldwright | Creators choose terms for their original work, subject to incorporated code and content |
| Project name and logo | Separate branding/trademark policy; do not imply endorsement rights |

Using a development tool does not, by itself, transfer ownership of a creator's original game to the tool's maintainers. Generated output that incorporates templates, bundled runtime code, or third-party assets may carry those materials' license obligations. Document those boundaries explicitly.

### Steps Required Before Publishing a Licensed Release

1. Verify ownership and obtain any necessary employer, contributor, or organizational approval.
2. Confirm the final license choice and its scope.
3. Review dependency, template, sample-content, and asset licenses for compatibility.
4. For the dual-license option, add unmodified license texts as `LICENSE-MIT` and `LICENSE-APACHE`, filling the MIT copyright notice with the correct owner and year.
5. Set the appropriate Cargo package license metadata. For the proposed dual-license model, the value is `MIT OR Apache-2.0`; workspace members must declare or inherit it correctly.
6. Include applicable license files in distributed packages and preserve required third-party notices, including relevant Apache NOTICE attributions where present.
7. Publish the inbound contribution policy and ensure contributors have granted the rights the project offers downstream.
8. Update this README to replace the proposal with an accurate statement of the approved license.

**Until these decisions and files are in place, do not advertise Worldwright as already licensed under MIT, Apache-2.0, or another open-source license. Public repository visibility alone does not grant general permission to reuse the code.**

This section is a project-planning recommendation, not a substitute for review of the actual license terms or legal advice on ownership, patents, or compatibility.

## Long-Term Vision

Worldwright should let teams spend more effort on the worlds they want to create and less on rebuilding essential infrastructure.

Success means more than an engine that accepts connections. It means developers can extend the rules, builders can publish coherent content, testers can verify behavior, and operators can support a persistent community with confidence.

**One toolkit. Many worlds. Built in Rust.**
