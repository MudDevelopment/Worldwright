# Worldwright

**Build worlds. Write the rules.**

Worldwright is a proposed Rust-based development suite for creating, testing, deploying, and operating **MUDs—multi-user domains**: persistent, multiplayer, primarily text-driven worlds.

The effort brings together an authoritative game engine, reusable Rust libraries, world-building tools, developer utilities, automated testing, and operational interfaces. Its purpose is to help teams build complete games without repeatedly rebuilding the foundations of networking, world simulation, content management, persistence, and live-service support.

Worldwright is **a toolkit for building games, not a single game or setting**. A science-fiction frontier, a fantasy realm, and a roleplay-focused social world should be able to share the same technical foundation while retaining their own rules, content, and identity.

> **Project status:** Core interfaces and development workflow implemented on the four-app/six-crate scaffold. Typed values, rule planning, a bounded logical scheduler, and deterministic fixtures are present; a running game, UI, networking, authentication, and persistence are not. Linux Rust 1.85.0 compilation, Clippy, 48 tests, debug build, and documentation passed. The pinned Rust 1.85.1 / Windows CI jobs are configured but not remotely verified. See [Getting Started](#getting-started), [core interfaces](docs/CORE_INTERFACES.md), and [verification](docs/VERIFICATION.md).
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

These are proposed starting choices, subject to architecture review and implementation evidence.

| Area | Proposed technology or approach |
|---|---|
| Primary language | Rust |
| Asynchronous I/O | Tokio |
| HTTP and WebSocket service | Axum |
| Relational persistence | PostgreSQL |
| Database access | SQLx |
| Server-rendered web surfaces | Askama and HTML, with focused client-side interaction |
| World content | Proposed versioned JSON definitions and immutable content-release artifacts |
| Server configuration | Proposed versioned TOML with validated settings and secret references |
| Deployment | Initially a single authoritative server deployment with a private database |
| Packaging | Reproducible binaries or containers, selected during implementation |

The scaffold pins Rust 1.85.1, uses edition 2024, and versions local packages as `0.1.0-dev.0`. External dependency versions, network ports, and public endpoints remain unassigned; Tokio/Axum/SQLx/Askama are planned, not installed dependencies.

### Runtime Model

Begin with a **modular monolith** and one authoritative owner of mutable simulation state. Keep the system internally modular without introducing distributed services solely to match component names.

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

The application and library directories contain the workspace and core-interface increment. Studio and Console remain metadata-only stubs. Cargo.lock is supplied. The local workflows live in `scripts/`, CI in `.github/workflows/ci.yml`, and the implemented boundary is documented in [CORE_INTERFACES.md](docs/CORE_INTERFACES.md). Other deployment/content directories remain guidance placeholders.

```text
worldwright/
├── Cargo.toml
├── Cargo.lock                  # Cargo-generated; keep committed
├── scripts/                    # dev.sh and dev.ps1
├── .github/workflows/ci.yml     # Linux/Windows checks
├── rust-toolchain.toml
├── .cargo/config.toml
├── README.md
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
7. **Playtest in staging.** Review the experience with designers, builders, and operators.
8. **Publish a versioned release.** Record code, content, configuration, and migration versions together.
9. **Operate and improve.** Monitor the service, handle reports, and use evidence to guide changes.

This is a target workflow. CLI subcommands and installation instructions will be documented only after they exist and are verified.

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

All phases are proposed. Completion should be demonstrated through acceptance evidence rather than elapsed time alone.

| Phase | Focus | Exit evidence |
|---|---|---|
| **1. Foundation** | Ownership, licensing decision, workspace, architectural contracts, and CI | Reproducible baseline build and approved project boundaries |
| **2. Playable slice** | Browser connection, character entry, look, movement, and communication | Two players can interact in a small authoritative world |
| **3. Durable gameplay** | Inventory, progression, a simple encounter, rewards, and reconnect | Restart and retry tests preserve documented state invariants |
| **4. Builder workflow** | Content validation, preview, publication, and reference content | A builder can safely introduce and test a small playable zone |
| **5. Operational beta** | Moderation, authentication hardening, metrics, deployment, and recovery | Restricted beta with tested restore and documented support ownership |
| **6. Toolkit maturity** | SDK stabilization, richer Studio/Console features, templates, and compatibility policy | A second game reuses the foundation without extensive engine changes |

The second game is an important architectural test: it helps distinguish genuinely reusable infrastructure from assumptions embedded in the first game's design.

## Getting Started

Extract the framework archive and open its `worldwright/` directory. No clone URL, database, environment file, or external service is required for this scaffold.

The workspace selects **Rust 1.85.1**, edition **2024**, and only local dependencies. Install that toolchain with rustfmt and Clippy through your approved tooling process; `rust-toolchain.toml` declares it for rustup. See the [scaffold guide](docs/SCAFFOLD.md) for installation details.

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

Cargo.lock is supplied and was generated by Cargo. Keep it committed; checks use locked, offline dependency resolution. Bootstrap installs the pinned toolchain through rustup when available, or reports installed tools for manual version verification. Toolchain installation can require network access. The local Linux run used Rust 1.85.0; the Rust 1.85.1 Windows/hosted CI jobs still need verification.

After successful verification, inspect the stubs:

```bash
cargo run -p worldwright --locked --offline -- --help
cargo run -p worldwright-server --locked --offline -- status
cargo run -p worldwright-studio --locked --offline -- status
cargo run -p worldwright-console --locked --offline -- status
```

All four applications support `--help`/`-h`, `--version`/`-V`, and `status`. Those metadata commands exit successfully; no arguments, unknown operations, or extra arguments exit with code 2. Default server startup refuses to run. No port is opened, UI displayed, or database contacted. `status` is not a service-readiness check.

See [docs/SCAFFOLD.md](docs/SCAFFOLD.md) for the implemented stub inventory and first coding tasks, and [docs/VERIFICATION.md](docs/VERIFICATION.md) for checks actually performed. Approve ownership/licensing before public distribution; the next coding goal is a minimal in-process look/move slice, not deployment.

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

Security and player safety are part of the toolkit's requirements.

Planned safeguards include:

- Server-side authentication and authorization.
- Modern password hashing where password accounts are supported.
- TLS for public connections and secure session management.
- Validation of WebSocket origins and protection for state-changing browser requests; CORS alone is not an authorization or CSRF defense.
- Limits on input size, command rate, queues, and expensive operations.
- Safe rendering of player-generated text.
- Audited, least-privilege administrative actions.
- Protected backups and tested restoration.
- Documented retention and access rules for account, chat, and audit data.
- A designated private vulnerability-reporting channel before public release.

Do not use a public issue to disclose credentials, sensitive player information, or an unmitigated exploit. [SECURITY.md](SECURITY.md) provides the proposed reporting process and explicitly marks private contact routes, support coverage, and supported software versions as not yet established; no address or operational service has been invented.

## Documentation

This scaffold retains the seven foundational documents: README.md, DESIGN.md, ARCHITECTURE.md, CONTRIBUTING.md, CODE_OF_CONDUCT.md, DEFINITIONS.md, and SECURITY.md. [docs/SCAFFOLD.md](docs/SCAFFOLD.md) describes the supplied code; [docs/VERIFICATION.md](docs/VERIFICATION.md) records performed checks and gaps. Runtime design remains proposed beyond the explicit stubs. Contribution, conduct, and security policies still require owner adoption and real reporting contacts. Deeper documentation directories contain guidance placeholders, not completed operational procedures.

| Document or area | Intended purpose |
|---|---|
| [README.md](README.md) | Project overview and contributor/developer entry points; runnable instructions remain pending |
| [DESIGN.md](DESIGN.md) | Design decisions, invariants, state ownership, subsystem contracts, and compatibility |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Component boundaries, crate dependencies, runtime flows, deployment topology, and lifecycle |
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
