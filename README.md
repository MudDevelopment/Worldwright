# Worldwright

**Build worlds. Write the rules.**

Worldwright is a proposed Rust-based development suite for creating, testing, deploying, and operating **MUDs—multi-user domains**: persistent, multiplayer, primarily text-driven worlds.

The effort brings together an authoritative game engine, reusable Rust libraries, world-building tools, developer utilities, automated testing, and operational interfaces. Its purpose is to help teams build complete games without repeatedly rebuilding the foundations of networking, world simulation, content management, persistence, and live-service support.

Worldwright is **a toolkit for building games, not a single game or setting**. A science-fiction frontier, a fantasy realm, and a roleplay-focused social world should be able to share the same technical foundation while retaining their own rules, content, and identity.

> **Project status:** Milestones 1–3 are implemented development slices. **Milestone 4 — Durable Gameplay v0.1**, **Milestone 5 — Content Release and Preview v0.1**, **Milestone 6 — Operational Control v0.1**, and **Milestone 7 — Builder Workflow and Studio v0.1** are implemented awaiting their stated live evidence. M6/M7 are not production ready. See [road map](ROAD-MAP.md), [Studio guide](docs/STUDIO_V1.md), [operations contract](docs/OPERATIONS_V1.md), [release contract](docs/RELEASE_V1.md), and [M7 verification](docs/verification/milestone-7-report.md).
>
> **Evidence alignment — November 9, 2026:** Older milestone reports retain
> their original Windows/Linux scope. Current source includes P0/P1/P2 and R0/R1
> follow-through; helper checks are not native Rust or desktop acceptance.
> Current native PostgreSQL, paired-host and hosted/manual evidence remains pending.
>
> **License status:** MIT OR Apache-2.0 is recommended, pending approval by the authorized copyright owner. This README does not itself grant that license or establish ownership. See [Licensing](#licensing).

## Playable expedition update — November 9, 2026

The generated expedition executable now runs the game instead of printing only
its ID. Official preview dispatches the artifact's game contract; Studio adds a
reference/expedition packaging selector. The browser accepts `forage`,
`encounter`, `strike`, `rest`, and status/rank aliases through the existing
multiplayer owner. Supplies, threats, victories and ranks remain server/rule
owned, not browser state. See [playable expedition](docs/EXPEDITION_PLAYABLE.md)
for startup, timing, schema compatibility and regression commands.

**Verification:** current P0/P1 edits have not yet been compiled/tested with Cargo
here; earlier Windows passes do not verify them. The multiplayer owner now has
independent bounded idle-timer commits. The blocking terminal adapter remains
command-driven; live PostgreSQL recovery acceptance is still open.

## P0 source follow-through — November 9, 2026

The selected P0 source fixes and regression gates are installed: private XDG
workspace-parent preparation, fuller doctor diagnostics, pinned fresh-native
checks, selected-game preview/HTTP tests, browser/receipt lifecycle tests, and
conditional OIDC Helm validation with required parsed-YAML render gates.
Use `scripts/dev.sh check-native` or `scripts/dev.ps1 check-native`; run Cargo
formatting first. Chart tests now require Python 3.10+, test-only requirements
and Helm 3; missing tools fail rather than skip successfully. Node fixture/CI
uses Node 20+. Existing non-private managed parents fail with an actionable
permission message instead of being silently chmodded.

See [P0 acceptance](docs/P0_ACCEPTANCE.md) and the
[implementation ledger](docs/verification/p0-implementation-ledger.md).
**All six source tracks are implemented; their full acceptance tracks remain
open.** Synthetic/schema checks passed; current Cargo/PowerShell/real Studio,
Helm/PostgreSQL and hosted-CI passes have not been observed here.

## P1 continuation — November 9, 2026

Current backlog/status authority is [ROADMAP v2](ROADMAP_v2.md), with the
[P1 ledger](docs/verification/p1-implementation-ledger.md) and
[P1 acceptance guide](docs/P1_ACCEPTANCE.md). Older milestone reports remain
historical evidence, not proof of these edits.

- Browser startup probes `GET /api/v1/session` using the existing HttpOnly cookie;
  a valid session connects without another password login. No pending command
  history is automatically resent.
- Studio reads `GET /api/v1/capabilities`; room/exit bounds and UTF-8 text hints
  come from backend constants rather than the old 1000/256 UI limits.
- For optional metrics, set `WORLDWRIGHT_TELEMETRY_BIND=127.0.0.1:9090` (or an
  explicit nonzero IPv6 loopback port). This separate listener exposes only
  `/metrics`, is disabled by default and does not enable operator routes.
- New generated projects carry relative `sdk/` dependencies. To generate from an
  installed CLI after removing its original checkout, set `WORLDWRIGHT_SDK_BUNDLE`
  to a trusted, verified private bundle of the matching version. This is private
  source delivery, not a license grant or a dependency cache.
- Preview uses run-scoped capture and a bounded input queue. Unix process groups
  and Windows taskkill supply cleanup paths; process-tree/ACL certification is
  still pending. These are not an OS sandbox or an egress firewall.
- PostgreSQL requires the additive `20261118000400_gameplay_timer_audit.sql`
  migration before updated startup. Startup verifies migrations; apply them
  explicitly through the existing database migrate workflow.
- External outbox dispatch is unsupported. Bounded expired-player-session
  cleanup preserves receipts, business uniqueness, OIDC links and audits.
- CI runs the full ignored PostgreSQL suite and includes a separate development
  container CLI smoke job. Fixed beta-250 load is manual, default-branch-only,
  and requires the protected `worldwright-acceptance` environment and
  acceptance-only credentials. Configured jobs are not observed passes.

Release evidence is collected with `scripts/release-evidence.py`; named owner
approvals and exact-digest live evidence remain required. `publish = false`,
licensing/support/reporting decisions and production-readiness restrictions remain.

## P2 toolkit consistency — WWV2-016

See [P2 acceptance](docs/P2_ACCEPTANCE.md) and the
[P2 ledger](docs/verification/p2-implementation-ledger.md).

- `worldwright compatibility --json` derives inventories from the SDK matrix
  and reports `sdk_package_version` separately from SDK API version.
- `worldwright benchmark --json` now identifies schema 2 and recipe
  `runtime-look-nearest-rank-v2`. Latencies remain integer nanoseconds.
  Historical reports without recipe metadata retain their original v1 formula;
  they are not automatically comparable optimization evidence.
- Runtime and beta-load statistics share an overflow-safe integer nearest-rank
  selector. Beta-250 was already nearest rank: no dimensions or thresholds changed.
- Bundle metadata follows SDK package/toolchain authority; a retained API-v1
  consumer tests relocated private-bundle source compatibility, not a fictional
  previous release or Rust ABI guarantee.
- Run `python scripts/test-p2-contracts.py` and `python scripts/test-p2-negatives.py`
  with Python 3.11+. Eight source/TOML/fixture checks and six mutation-rejection
  cases passed against current readback files. Native P2 Rust, actual
  bundle/consumer execution and hosted acceptance are still pending.

## R0/R1 execution and private desktop package

R0/R1 are release tracks, NOT the P0/P1 priority labels. See the
[R0/R1 acceptance guide](docs/R0_R1_ACCEPTANCE.md) and
[ledger](docs/verification/r0-r1-acceptance-ledger.md).

- `python scripts/r0-verify.py --output <new-private-directory>` runs the full
  pinned/native/integration gates and retains failure evidence. It requires
  approved cache/tools and a disposable PostgreSQL test database; no successful skips.
- `scripts/package-desktop.py build` requires complete, same-source Ubuntu R0
  evidence and matching binaries. The private archive records debug profile,
  runtime libraries, binary/file hashes and original R0 identity; it is not a
  licensed or signed supported release, AppImage, .deb or native GUI installer.
- Installed `bin/worldwright-desktop` uses bundled binaries and needs Python
  3.11+, Bash/coreutils, setsid/pidfds and compatible libraries, not Cargo or the
  original source checkout. Studio assets are compiled into its Rust binary.
  Game source compilation still requires a matching development toolchain/SDK.
- Existing workspace/package directories must be owned/private; unsafe paths
  fail rather than being silently chmodded. TERM/INT/HUP drive session cleanup;
  failed/forced cleanup is not reported as graceful success. The browser is not killed.
- `scripts/r1-desktop-verify.py` exercises the installed package with real
  Chromium/HTTP/process checks. Its passing automation status still leaves
  Firefox/X11/Wayland/accessibility and Windows credential/discovery review pending.
- `scripts/r0-r1-close.py` checks complete log/source/lock/host/binary/package
  tuples and manual record structure. Humans must authenticate observations and
  adopt closure; this checker cannot grant reviewer authority or license rights.

Ubuntu CI is in `r0-r1.yml`. `r0-windows.yml` is explicitly dispatched on the
  default branch with an acceptance-only database secret; configure required
  reviewers on `worldwright-r0-windows` BEFORE provisioning it. YAML does not
  configure those protections or establish a hosted pass. No full R0/R1 exit is
  currently closed.

## Current implementation

The workspace contains the current Rust workspace packages. M6 provides scoped operator authorization,
local MFA credentials/tokens, a loopback operator client, audited release
control, lifecycle drain, and recovery/takeover tooling. M5 provides deterministic `.wwr`
packaging, strict inspection, isolated in-memory preview, and offline staging
publication/activation workflows. PostgreSQL tests remain compile-only evidence.
Studio adds a local append-only draft workspace, deterministic artifact
packaging, official preview integration, and authenticated publication
requests. Preview has no OS sandbox or network egress firewall. Production
activation requires auth-v1 request/review and current MFA-backed
authorization.

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
| **Worldwright Studio** | Structured schema-v1 rooms/exits authoring, shared validation, immutable packaging, preview, and publication requests | Arbitrary entities such as NPCs, items, quests, and dialogue remain deferred |
| **Worldwright CLI** | Project scaffolding, content checks, development utilities, and build orchestration | One developer-facing command-line entry point |
| **Worldwright Testkit** | Scenario tests, simulated players, repeatable fixtures, and replay diagnostics where supported | Test support integrated with the core runtime |
| **Worldwright Console** | Server health, session visibility, moderation, and authorized administration | Restricted operator interface |

**Naming convention:** Use descriptive component names under the Worldwright umbrella rather than unrelated brands for each tool.

## Planned Capabilities

This is the target product scope, not a list of features all present today.
M1–M3 provide core/runtime/content/transport slices; M4 durable state and M5
immutable release/preview workflows are implemented awaiting live PostgreSQL
verification. M6 operational administration and production authorization are
implemented awaiting live PostgreSQL/recovery evidence. See [Delivery Roadmap](#delivery-roadmap).

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
| Relational persistence | PostgreSQL | Implemented in M4/M5; live verification pending |
| Database access | SQLx | Integrated for M4/M5 durable paths; live PostgreSQL verification pending |
| Web surfaces | Static HTML/CSS/JavaScript player client; Askama for future server-rendered surfaces | Local player client implemented; Askama deferred |
| World content | Strict JSON schema v1 and immutable content-release artifacts | Packaging/inspect/preview implemented; live PostgreSQL activation evidence pending |
| Server configuration | Versioned TOML with validated settings and secret references | Proposed; existing CLI settings do not establish this configuration schema |
| Deployment | Single authoritative process initially; private database for durable operation | Local mode is process-local; durable mode is implemented; production topology remains undecided |
| Packaging | Private native desktop archive and development container | Builders/gates implemented; actual R0/R1 acceptance and approved redistribution remain open |

The workspace pins Rust 1.85.1, uses edition 2024, and versions packages as
`0.1.0-dev.0`. Content loading and release packaging use pinned Serde/JSON;
the server integrates Tokio, Axum, Argon2, getrandom, futures, and
SQLx/PostgreSQL. Askama remains deferred. Scoped operator authorization is
implemented; its live acceptance, owner adoption and production exposure remain open.

### Runtime Model

Use a **modular monolith** and one authoritative owner of mutable simulation state. Keep the system internally modular without introducing distributed services solely to match component names.

The diagram is the **target architecture across multiple milestones**, not the
production composition. M4/M5 PostgreSQL/release paths are implemented but
await live database verification. The loopback gateway does not terminate TLS
itself; scoped operational control is implemented but not deployment-verified.

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

The application and library directories contain the content, runtime, protocol,
local transport, persistence, release, and operational-control increments.
Studio is an implemented loopback workspace; M4-M7 live behavior awaits live
PostgreSQL evidence. Cargo.lock is supplied. See [RELEASE_V1.md](docs/RELEASE_V1.md),
[SERVER.md](docs/SERVER.md), and [verification](docs/VERIFICATION.md).

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
│   ├── worldwright-studio/      # Implemented loopback builder application
│   └── worldwright-console/     # Operator application, if separately packaged
├── crates/
│   ├── worldwright-core/        # Domain types and reusable simulation logic
│   ├── worldwright-content/     # Content schemas and validation
│   ├── worldwright-release/     # Deterministic artifact packaging and verification
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
2. **Create a project.** Use the implemented CLI project templates, or a documented reference project.
3. **Author a small world.** Build a few connected rooms, an NPC, an item, and a complete interaction.
4. **Validate content.** Check identifiers, references, constraints, and schema compatibility.
5. **Implement game rules.** Add focused systems through supported Rust interfaces.
6. **Run repeatable tests.** Verify actions, authorization, ownership, and recovery behavior.
7. **Package and preview.** M5 identifies immutable bytes with a
   manifest/digest and tests exact artifacts in an isolated environment; the
   implemented M7 Studio integrates that workflow.
8. **Request controlled publication.** Bind review to the tested digest and target environment, check the expected current release and compatibility, then use the authorized staging/activation workflow. A save or successful validation is not publication approval.
9. **Operate and improve.** Record code, content, configuration and migration versions together; use M6 authorization, audit, monitoring, drain and recovery procedures before operational exposure.

Current package/inspect/preview syntax and the staging release workflow are in
[RELEASE_V1.md](docs/RELEASE_V1.md) and
[RELEASE_ACTIVATION.md](docs/operations/RELEASE_ACTIVATION.md). Changed artifact
bytes require refreshed review, and rollback must account for durable-state and
schema compatibility.

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

[ROADMAP_v2.md](ROADMAP_v2.md) is authoritative for current priorities and acceptance.
[ROAD-MAP.md](ROAD-MAP.md) retains the historical **eight dependency-ordered milestones**. This summary replaces the earlier six-phase outline. [IMPLEMENTATION.md](IMPLEMENTATION.md) records the latest implemented slice, next work, and increment-specific historical evidence.

| Milestone | Focus | Status and exit evidence |
|---|---|---|
| **M1 — Foundation and Core** | Workspace, typed identifiers/contracts, rules/effects, scheduling primitives, deterministic tests | Reported complete baseline; configured CI is not proof of successful hosted runs |
| **M2 — Runtime, Content, and Local Play** | Single-owner runtime, attachment generations, staged Look/Move/Say, strict JSON v1, named exits, local terminal host | Reported complete with earlier Linux evidence and provenance limits |
| **M3 — Multiplayer Transport v0.1** | Bounded protocol, local registry/sessions, loopback HTTP/WebSocket gateway, browser client | Implemented with reported Windows-local verification; Linux Transport rerun, hosted CI and deployment remain pending |
| **M4 — Durable Gameplay v0.1** | PostgreSQL schema/migrations, writer fencing, identity/session, valuable state, receipts, audit/outbox, uncertain-outcome recovery | **Implemented awaiting live PostgreSQL verification** |
| **M5 — Content Release and Preview v0.1** | Immutable manifests/artifacts, compatibility, staging/maintenance activation, supported rollback, isolated preview, CLI publication requests | **Implemented awaiting live PostgreSQL verification** |
| **M6 — Operational Control v0.1** | Capability-scoped control, privileged assurance, audit, observability, graceful drain, backup/restore and takeover runbooks | **Implemented awaiting live PostgreSQL and recovery execution** |
| **M7 — Builder Workflow and Studio v0.1** | Schema-aware editing, shared diagnostics, M5 preview/release integration, M6-controlled publication, accessibility | Implemented awaiting live PostgreSQL/operator publication and manual browser evidence |
| **M8 - Toolkit Maturity** | Independent second game, SDK compatibility/deprecation, project templates/migrations, measured performance | Historical Windows-local evidence; current expedition durability and P2 consistency source work need live/native acceptance; broader capacity and deployment remain open |

### M6 verification handoff

M4/M5 await live PostgreSQL verification. M6 is implemented awaiting live
PostgreSQL and recovery execution. M8 is implemented with Windows-local
evidence; current expedition durability needs live acceptance and broader capacity remains open. Production
activation uses auth-v1 request/review and current MFA-backed grants; M6 is not
production readiness evidence.

`accepted` remains admission, not commit. In local mode `command_result` describes
in-memory execution. In PostgreSQL mode the general gameplay path commits scoped
receipts/state before final output and completion; same-intent receipt reuse is
subject to current authority and changed-intent reuse conflicts. Browser page IDs
are unique and no old command history is blindly resent. Full live recovery,
provider, deployment and capacity evidence remains open.

### Dependency and release boundaries

M5 consumes M4's durable-reference and writer contracts. M6 consumes transaction and release mechanisms. M7 consumes shared validation, isolated preview and operational controls rather than introducing a second validator or direct database editing. M8 uses a genuinely independent second game to distinguish reusable infrastructure from first-game assumptions.

Design and test-fixture work can overlap milestones, but activation cannot bypass prerequisite gates. Security applies from the earliest affected increment. Neither M3 local success nor M5 staging activation constitutes production readiness. Licensing, owner adoption, identity, retention, deployment and measured recovery targets remain explicit decisions; no legal approval, production port or service-level target is selected by this roadmap summary.

## Getting Started

**Primary platform: Ubuntu 24.04 LTS / Linux. Windows support is retained.**
Cargo uses the native host target; the workspace no longer forces Windows MSVC.
The main `worldwright` application is a native CLI and Studio is a Rust-hosted
loopback browser application, not a native GUI window. See
[Ubuntu desktop setup](docs/LINUX_DESKTOP.md) for dependencies, desktop launch,
storage paths, shutdown behavior, and the remaining verification gates.

After bootstrap, launch Studio from your Ubuntu desktop user account:

```bash
bash scripts/desktop.sh
# Optional: skip browser opening and choose a workspace (absolute path).
bash scripts/desktop.sh --no-browser "$HOME/Worldwright Drafts"
```

The launcher builds native Linux CLI/Studio executables, uses an ephemeral
loopback port, and opens your default browser when a desktop session and
`xdg-open` are available. Keep the launcher running while using Studio.
Ubuntu/Windows CI configuration is supplied; these changes do not establish
successful hosted CI, a real Ubuntu desktop run, or production readiness.

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

All four applications support their help/version/status surfaces. The developer
CLI supports `content validate/package/inspect`, `play`, and `preview`. The
server supports database migrate/seed and release request/review/status/
activate/recover commands. Invalid command shapes exit with code 2. Console
and Studio are real loopback clients; neither bypasses server authority. Live
PostgreSQL, recovery, and operator-publication evidence still await execution.

Generate local account hashes and start the development transport with the
exact workflow in [SERVER.md](docs/SERVER.md). Local mode is loopback-only and
process-local; durable mode uses the verified active PostgreSQL artifact. The
server is intentionally not a public deployment or in-process TLS service.
See [docs/RUNTIME.md](docs/RUNTIME.md), [docs/PROTOCOL_V1.md](docs/PROTOCOL_V1.md),
[docs/SCAFFOLD.md](docs/SCAFFOLD.md), [docs/RELEASE_V1.md](docs/RELEASE_V1.md),
and [docs/VERIFICATION.md](docs/VERIFICATION.md). M6 authorization is
implemented; live database, recovery, deployment, and owner-adoption evidence remains pending.

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

- Durable identity/session and capability authorization are implemented awaiting PostgreSQL evidence;
  authentication-outage classification remains a gap.
- Scoped privileged operations, MFA-backed assertions, required audit, observability, and recovery tooling are implemented; live recovery exercise remains pending.
- Supported production TLS termination, trusted proxy policy, certificate lifecycle and secret delivery before public exposure.
- Protected backups, restore/takeover drills, restore-safe writer authority, and approved recovery targets.
- Retention and access rules for accounts, receipts, chat, audit, outbox and backups; retention choices must precede the affected durable storage/cleanup behavior.
- A designated private vulnerability-reporting channel, named owners and an adopted support policy before public release.

Do not use a public issue to disclose credentials, sensitive player information, or an unmitigated exploit. [SECURITY.md](SECURITY.md) provides the proposed reporting process and explicitly marks private contact routes, support coverage, and supported software versions as not yet established; no address or operational service has been invented.

## Documentation

This package retains the foundational policy documents and includes current
[RELEASE_V1.md](docs/RELEASE_V1.md),
[RELEASE_ACTIVATION.md](docs/operations/RELEASE_ACTIVATION.md), and
[milestone-5-report.md](docs/verification/milestone-5-report.md), alongside the
server/persistence guides. Contribution, conduct, M6 authorization, and
production deployment policies still require owner adoption and real reporting
contacts.

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

## M8 commands and evidence

```text
cargo run -p worldwright -- content validate examples/expedition/world.json
cargo run -p worldwright -- project new ./expedition --name expedition --template expedition
cargo run -p worldwright -- content migrate examples/expedition/world.json --output ./tmp-world.json --dry-run
cargo run -p worldwright -- benchmark --clients 100 --commands-per-client 100 --json
```

See [Toolkit v1](docs/TOOLKIT_V1.md), [M8 summary](Milestone-8-summary.md),
and [M8 evidence](docs/verification/milestone-8-report.md). M8 uses the
intended current-plus-previous 0.x compatibility window with one-minor deprecation;
only actual API-v1 support is advertised today, not a fictional previous release.
The 100-client figure is a development objective, not a beta commitment.

## Current M7 status

Milestones 4-7 are implemented awaiting their stated live evidence. M7 adds
the real loopback Studio draft workspace, immutable artifacts, official
preview integration, and authenticated publication requests. It is not
production ready. See [Studio v1](docs/STUDIO_V1.md) and
[M7 verification](docs/verification/milestone-7-report.md). M8 is implemented
with Windows-local evidence and explicit durability/capacity limits.

## R2 executable acceptance tooling

[R2 acceptance](docs/R2_ACCEPTANCE.md) now provides live PostgreSQL collection,
real subprocess/COMMIT-acknowledgement fault exercises, a controlled HTTPS OIDC
provider/browser harness, an actual-provider browser observer and independently
signed owner-adoption verification. `scripts/r2-verify.py collect` requires a
passed same-source R0 report, records the production server and separate test
fixture hashes, runs every discovered ignored persistence test, and fails on
missing tooling, required skips, leaked schemas or failed exercises. Use a new
private evidence directory outside the checkout and a loopback disposable
`worldwright_test` database; never production credentials.

The configured `r2-acceptance.yml` workflow does not itself constitute live
acceptance or approve a provider. The observer produces `observed_not_adopted`;
full R2 closure also requires retained current Windows R0 evidence and a complete
real-provider matrix signed by an independently trusted owner key. The current
migration inventory is eight immutable SQL files, not the historical seven.
OIDC links only an invited, pre-seeded account on first login; later logins start
without an invite. It does not create accounts or silently fall back to password
login. Multiplayer timers run independently; terminal turns remain command-driven.
See the [R2 ledger](docs/verification/r2-acceptance-ledger.md) for observed results
and open prerequisites. No R2 release acceptance is claimed by these source edits.

R2 expiry-fixture follow-through: `scripts/test-r2-expiry-sql.py` extracts and
executes the actual corrected fixture SQL against an explicitly supplied loopback
`worldwright_test` database, tests real COMMIT fault boundaries, and separately
requires all eight unchanged source migrations to apply. CI retains its private
preflight report/log; it is not a substitute for native Rust/SQLx acceptance.
The current full-migration preflight fails at
`20261106000100_operational_control.sql:188` (unquoted reserved `authorization`).
The corrected expiry cases pass; fresh-database/R2 acceptance remains blocked.
See the R2 ledger for exact evidence and the immutable-migration repair boundary.
