# Implementation handoff

## Current increment: Multiplayer Transport v0.1

Implemented `worldwright-server` on top of the validated content loader and core runtime. The server includes strict account-registry loading, fixed-profile Argon2id login, process-local sessions, exact-Origin loopback HTTP/WebSocket routes, one owner task with bounded channels and cancellation, takeover-safe attachments, and a CSP-compatible browser client. `worldwright-protocol` contains the validated v1 DTO/envelope contract.

Run the local account/hash and server workflow in [SERVER.md](docs/SERVER.md). The exact contract is in [PROTOCOL_V1.md](docs/PROTOCOL_V1.md). Local Play remains available as the preceding single-player host. Persistence, durable identity/revocation, recovery, public deployment, built-in TLS termination, and stable SDK policy remain future work.

The final Windows local gate records formatting, server/protocol checks, workspace compilation, 32 server library tests plus 2 server smoke tests, and protocol tests. See [VERIFICATION.md](docs/VERIFICATION.md) and the [transport report](docs/verification/transport-report.md).

## Previous increment: Local Play v0.1

Implemented `worldwright play <path>` on top of the unchanged content loader and core runtime. New application modules are `commands/play.rs`, `commands/play_input.rs`, and `commands/play/tests.rs`; subprocess coverage lives in `apps/worldwright/tests/play.rs`. Argument parsing, dispatch/help/status, and stage metadata are updated.

Run `cargo run -p worldwright --locked --offline -- play examples/two-room/world.json` after bootstrap. Use look/l, go, say, help, quit/exit, or EOF. The session is single-player and in-memory, with no login, listener, or saved state. See [Local Play](docs/LOCAL_PLAY.md) and [current verification](docs/VERIFICATION.md).

Core/content source, Cargo.lock, package versions, and the toolchain pin are unchanged from Content Loading v0.1. The following section records that earlier increment.

## Previous increment: Content Loading v0.1

All six requested implementation steps are complete in this source snapshot:

1. **Contract** — `docs/content/SCHEMA_V1.md`: explicit v1, strict JSON objects/fields, graph and text rules, limits, diagnostics, version policy.
2. **Parsing and validation** — `crates/worldwright-content/src/{schema,loader,validation,diagnostic}.rs`: bounded input, duplicate/depth preflight, version dispatch, capped errors and directed reachability.
3. **Validated types and conversion** — `validated.rs`, `instantiate.rs`: private content, deterministic checked IDs, named-exit lookup, all-room-before-exit builder construction, no partial return.
4. **SDK and runtime proof** — SDK/Testkit `content` exports, independent existing programmatic fixture, new `content_runtime` integration tests loading the committed JSON example.
5. **CLI** — `apps/worldwright/src/cli.rs` and `commands/`: `content validate <path>`, help, safe diagnostics, file and usage errors, binary-level tests.
6. **Provisioning, documentation, verification** — normal registry dependencies/lockfile, bootstrap fetch, offline checks, host-native target, README/design/architecture/security/schema updates, captured Linux workflow logs.

## Run

```bash
bash scripts/dev.sh bootstrap
bash scripts/dev.sh check
cargo run -p worldwright --locked --offline -- content validate examples/two-room/world.json
```

On Windows use `./scripts/dev.ps1 bootstrap` then `./scripts/dev.ps1 check` before the same Cargo command.

## Important notes

- `ContentPipelineStub` was removed; this is a breaking unpublished Rust API change. Use `load_file`/`load_bytes` then `instantiate`.
- Schema v1 is the first supported content format. Missing/future versions fail explicitly; no migration is guessed.
- The original Console smoke wrapper was under `test/`; it is now under Cargo's discoverable `tests/` directory with assertions unchanged.
- No core runtime Rust source was changed. No listener, UI, persistence, login, or live activation was added.
- **103 tests passed**, plus formatting, compilation, Clippy, build, and rustdoc. Read [verification](docs/verification/content-report.md): local checks used Rust 1.85.1 npm repackages and GitHub dependency source snapshots, not approved registry tarballs. Windows and hosted CI remain unverified.
- ZIP contains source, lockfile, example, documentation, and logs; it excludes `.git`, `target`, and temporary verification tooling/caches.
