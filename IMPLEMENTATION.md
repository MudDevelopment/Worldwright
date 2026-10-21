# Implementation handoff

**Updated:** October 21, 2026 — alignment with the expanded [ROAD-MAP.md](ROAD-MAP.md), [DESIGN.md](DESIGN.md), and [ARCHITECTURE.md](ARCHITECTURE.md).

This file records the latest reported implementation, the next implementation priority, and historical increment notes. The roadmap owns detailed scope and acceptance criteria; this handoff does not duplicate its implementation examples or invent new source interfaces.

> **Evidence boundary:** This is a documentation-only update based on the supplied documents. Repository source, linked verification logs, migrations, and build artifacts were not independently inspected here. Implementation and test results below are reported historical evidence, not new runs. Planned work is not implemented simply because its design is now more detailed.

## Status at a glance

| Milestone | Handoff status |
|---|---|
| M1 — Foundation and Core | Reported baseline complete: typed contracts, workspace boundaries, deterministic test support |
| M2 — Runtime, Content, and Local Play | Reported complete; earlier Linux evidence retains its documented provenance limits |
| M3 — Multiplayer Transport v0.1 | Latest implemented increment; reported Windows-local verification |
| M4 — Durable Gameplay v0.1 | **Next active implementation priority; planned, not implemented** |
| M5 — Content Release and Preview v0.1 | Planned: immutable artifacts, controlled activation, isolated preview, CLI publication requests |
| M6 — Operational Control v0.1 | Planned: scoped administration, audit, observability, drain, restore and takeover procedures |
| M7 — Builder Workflow and Studio v0.1 | Planned: builder UI consumes M5 preview/release and M6 authorization/audit contracts |
| M8 — Toolkit Maturity | Planned: independent second-game reuse, SDK compatibility, tooling and measured performance |

Design and fixture work may overlap milestones, but later activation must respect prerequisite gates. No production readiness or legal approval follows from a local increment's completion.

<a id="current-increment-multiplayer-transport-v01"></a>

## Latest implemented increment: Milestone 3 — Multiplayer Transport v0.1

Implemented `worldwright-server` on top of the validated content loader and core runtime. The server includes strict account-registry loading, fixed-profile Argon2id login, process-local sessions, exact-Origin loopback HTTP/WebSocket routes, one owner task with bounded channels and cancellation, takeover-safe attachments, and a CSP-compatible browser client. `worldwright-protocol` contains the validated v1 DTO/envelope contract.

Run the local account/hash and server workflow in [SERVER.md](docs/SERVER.md). The exact contract is in [PROTOCOL_V1.md](docs/PROTOCOL_V1.md). Local Play remains available as the preceding single-player host.

### Implemented boundary

- One owner executes the in-memory world; network handlers do not directly mutate it.
- Account/character/connection/generation checks protect current attachments and stale cleanup.
- Look, Move by named exit, and Say use the runtime; Move is followed by a separate Look.
- Login sessions and attachment authority are process-local, not durable across restart.
- Protocol v1 request IDs correlate one stream. They are not durable idempotency keys; `accepted` is admission, and `command_result` is in-memory execution, not a database receipt.
- Loopback development does not establish public deployment, built-in TLS, or trusted proxy-header behavior.
- PostgreSQL, durable identity/revocation, saved valuable state, recovery, content activation, Studio/Console behavior, and stable SDK policy remain future work.

### Reported verification and remaining work

The roadmap records the final Windows-local gate on the pinned Rust 1.85.1 baseline:

| Evidence | Reported result |
|---|---|
| Workspace unit/integration tests | 164 passing |
| Compile-fail doctests | 2 passing |
| Server library / server smoke / protocol tests | 32 / 2 / 14 passing, included in the relevant reported totals |
| Real loopback WebSocket acceptance cases | Seven reported cases; do not add them as an independent extra test total |
| Other checks | Formatting, compilation, Clippy, build, and rustdoc reported passing |

See [VERIFICATION.md](docs/VERIFICATION.md) and the [transport report](docs/verification/transport-report.md) for authoritative commands, revisions, environment and provenance. This handoff does not claim those logs were reread or those commands rerun.

**Still pending:** Linux Transport rerun at a pinned revision, observed hosted CI, approved dependency-artifact acceptance as applicable to the verification record, and deployment-specific checks. Earlier Linux core/content/local-play results do not satisfy the Transport rerun. A configured CI workflow is not an observed successful run.

## Next active increment: Milestone 4 — Durable Gameplay v0.1 — Planned

### Goal and entry decisions

Implement one PostgreSQL-backed valuable-state flow: a bounded item or reward operation that survives retry and restart without duplicate value, false success, or two accepted world writers. Do not begin by persisting every runtime field or implementing the entire inventory, combat, economy, and progression backlog.

Before the first transaction:

1. Confirm the source revision, manifests, runtime/transport contracts, and M3 regression baseline.
2. Select the valuable operation and its independent ownership or entitlement invariant.
3. Decide local durable accounts/sessions versus identity-provider integration, including stable mappings and restart/revocation behavior. The current registry is not already a durable identity service.
4. Review the PostgreSQL schema, migration mechanism, supported version/isolation assumptions, writer-fencing protocol, and uncertain-commit recovery contract.
5. Define durable operation identity and canonical intent fingerprints separately from v1 request correlation. Any wire extension needs an explicit version/negotiation contract before clients depend on it.
6. Record required owner choices using the roadmap's decision records. Do not invent retention periods, production ports, identity providers, or recovery targets.

### Planned work packages

The following correspond to ROAD-MAP sections 4.1–4.9. They are work to implement, not claims about existing tables, modules, or endpoints.

| Work package | Implementation focus | Required evidence |
|---|---|---|
| 4.1 Relational schema and migration policy | Define authoritative facts, reconstructed projections, constraints, migrations and compatibility checks | Fresh database plus supported upgrades; incompatible/future schema fails safely |
| 4.2 World-writer epochs and transactional fencing | Serialize takeover against each protected transaction; generation exhaustion fails closed | Competing writers cannot both commit; stale authority is refused, including after recovery |
| 4.3 Durable accounts/sessions or identity-provider integration | Approved identity ownership, stable account/character mapping, revocation and restart semantics | Cross-account access denied; session policy tested; attachments are freshly established |
| 4.4 One item or reward operation | Exclusive ownership or one grant per entitlement; game rule produces a bounded effect plan | Concurrent/duplicate attempts preserve the business invariant, including attempts with fresh operation keys |
| 4.5 Scoped receipts and intent fingerprints | Trusted world/actor/domain scope, canonical intent, durable result retrieval and retention policy | Same key/same intent returns the prior outcome; different intent conflicts; unauthorized lookups fail |
| 4.6 Atomic effects, receipt, required audit and outbox | Persist all required records in one transaction; apply runtime projection only after confirmed commit | Failure injection shows no partial valuable state, missing required audit, or pre-commit success |
| 4.7 Confirmed, rejected and uncertain outcomes | Treat ambiguous commit as unknown, not failed; resolve original operation before conflicting execution | Disconnect at commit produces no speculative success, invented rollback, or unconditional new-key retry |
| 4.8 Restart reconstruction and stale-writer refusal | Rebuild compatible durable state with current content and valid authority before readiness | Missing references fail explicitly; post-commit projection failure recovers from database; old writers cannot resume |
| 4.9 Failure testing and exit gate | Unit, real PostgreSQL integration, concurrency, failure-injection, restart and transport-regression tests | Recorded commands/environment and observed invariants prove the whole slice, not only its happy path |

**Illustrative acceptance scenario:** submit the selected reward operation with durable key K; interrupt the connection around commit; restart/reconnect and reconcile K under the same authorized scope. The result is either one committed grant or a confirmed non-commit, never two grants or success without commitment. Reusing K with a different intent conflicts. A new key cannot bypass the reward's business-uniqueness rule. These are conceptual names, not existing DTO fields.

### Handoff and non-goals

Preserve M3 queue bounds, exact attachment authority, origin checks, safe text rendering, and slow-client isolation. Database workers do not become additional world owners, and the owner does not wait on individual socket writes.

M4's exit gate requires the selected valuable operation to withstand duplicate requests, concurrency, forced failures, and restart with verified writer exclusion and consistent receipts/audit/outbox. Document what remains process-local. Do not mark the increment complete on the strength of an illustrative schema or mocked database alone.

M4 does not promise full-world persistence, durable chat, universal replay, event sourcing, active-active worlds, or production backup readiness. M5 consumes its durable-reference and writer contracts for release/preview. M6 adds exercised operational backup/restore and takeover policy, including preventing restored authority from matching a pre-restore process's old credentials. M7 consumes those mechanisms through Studio rather than bypassing them.

## Previous increment: Local Play v0.1

**Historical M2 record — not the next active increment.**

Implemented `worldwright play <path>` on top of the unchanged content loader and core runtime. New application modules were `commands/play.rs`, `commands/play_input.rs`, and `commands/play/tests.rs`; subprocess coverage lives in `apps/worldwright/tests/play.rs`. Argument parsing, dispatch/help/status, and stage metadata were updated.

Run `cargo run -p worldwright --locked --offline -- play examples/two-room/world.json` after bootstrap. Use look/l, go, say, help, quit/exit, or EOF. The session is single-player and in-memory, with no login, listener, or saved state. See [Local Play](docs/LOCAL_PLAY.md) and [current verification](docs/VERIFICATION.md).

For that Local Play increment, core/content source, Cargo.lock, package versions, and the toolchain pin were unchanged from Content Loading v0.1. This is an increment-specific historical statement, not a claim that subsequent Transport work changed none of those files.

## Previous increment: Content Loading v0.1

**Historical M2 record.** All six requested implementation steps were reported complete in that source snapshot:

1. **Contract** — `docs/content/SCHEMA_V1.md`: explicit v1, strict JSON objects/fields, graph and text rules, limits, diagnostics, version policy.
2. **Parsing and validation** — `crates/worldwright-content/src/{schema,loader,validation,diagnostic}.rs`: bounded input, duplicate/depth preflight, version dispatch, capped errors and directed reachability.
3. **Validated types and conversion** — `validated.rs`, `instantiate.rs`: private content, deterministic checked IDs, named-exit lookup, all-room-before-exit builder construction, no partial return.
4. **SDK and runtime proof** — SDK/Testkit `content` exports, independent existing programmatic fixture, new `content_runtime` integration tests loading the committed JSON example.
5. **CLI** — `apps/worldwright/src/cli.rs` and `commands/`: `content validate <path>`, help, safe diagnostics, file and usage errors, binary-level tests.
6. **Provisioning, documentation, verification** — normal registry dependencies/lockfile, bootstrap fetch, offline checks, host-native target, README/design/architecture/security/schema updates, captured Linux workflow logs.

## Run

Existing commands are preserved. Run from the actual workspace root, not a directory containing only these Markdown files:

```bash
bash scripts/dev.sh bootstrap
bash scripts/dev.sh check
cargo run -p worldwright --locked --offline -- content validate examples/two-room/world.json
```

On Windows use `./scripts/dev.ps1 bootstrap` then `./scripts/dev.ps1 check` before the same Cargo command. Bootstrap may require approved network access; offline checks require the recorded dependency cache/source. See [README.md](README.md#getting-started) for Local Play and authenticated Transport commands. No M4 migration, reconciliation, or database-launch command is introduced by this document.

## Important notes

### Historical Content Loading v0.1 notes

The following statements belong to the earlier content-loading snapshot only; they do not override the M3 status above.

- `ContentPipelineStub` was removed; this was a breaking unpublished Rust API change. Use `load_file`/`load_bytes` then `instantiate`.
- Schema v1 was the first supported content format. Missing/future versions fail explicitly; no migration is guessed.
- The original Console smoke wrapper was under `test/`; it moved under Cargo's discoverable `tests/` directory with assertions unchanged.
- That content-loading increment changed no core runtime Rust source and added no listener, UI, persistence, login, or live activation. Later Transport work added the documented listener, login, and browser client; persistence and activation remain planned.
- **103 tests passed in the historical content gate**, plus formatting, compilation, Clippy, build, and rustdoc. See [content verification](docs/verification/content-report.md): local checks used Rust 1.85.1 npm repackages and GitHub dependency source snapshots, not approved registry tarballs. Windows and hosted CI were unverified for that historical gate; later Windows Transport evidence is recorded separately above. Do not combine the 103 count with the later workspace total.
- The earlier ZIP was described as containing source, lockfile, example, documentation, and logs, excluding `.git`, `target`, and temporary verification tooling/caches. This documentation-only update neither creates nor re-verifies that source archive.

### Current handoff discipline

Keep proposed, implemented, locally verified, hosted-CI verified, and deployment-verified states separate. Record exact revisions, toolchains, dependency provenance, commands, outcomes, and unresolved gaps before changing milestone status.

Update this handoff with ROAD-MAP, README, DESIGN, ARCHITECTURE and the relevant protocol, server, security and verification documents when behavior changes. Preserve historical evidence with its increment scope; do not overwrite it with later counts or copy it forward as a new run.
