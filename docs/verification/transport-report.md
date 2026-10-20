# Multiplayer Transport v0.1 Verification Report

## Environment

- Host: Windows (`x86_64-pc-windows-msvc` native target)
- Rust: `rustc 1.85.1 (4eb161250 2025-03-15)`
- Cargo: `cargo 1.85.1 (d73d2caf9 2024-12-31)`
- Workspace: Rust edition 2024, resolver 3, package version `0.1.0-dev.0`
- Dependency mode: Cargo lockfile, `--locked --offline` in the project-native gate after dependencies were available locally
- Linux rerun: not performed for this increment
- Hosted CI: not run

## Commands and results

The project-native Windows gate passed through PowerShell:

```text
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/dev.ps1 check
```

It passed:

- `cargo fmt --all -- --check`
- `cargo check --workspace --all-targets --locked --offline`
- `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings`
- `cargo test --workspace --locked --offline`
- `cargo build --workspace --locked --offline`
- `cargo doc --workspace --no-deps --locked --offline` with warnings denied

The direct `./scripts/dev.ps1 check` spelling was also attempted by the Bash tool wrapper and failed because Bash interpreted the PowerShell file; the same script passed when invoked by PowerShell as shown above. This is a shell invocation distinction, not a project check failure.

Follow-up commands passed:

```text
cargo test -p worldwright-server
cargo test -p worldwright-protocol
git diff --check
```

## Test totals

The workspace gate observed **166 passing tests: 164 unit/integration tests and 2 compile-fail doctests**, with zero failures.

Focused totals:

- `worldwright-server`: 32 library tests, 2 smoke integration tests, 0 doctests
- `worldwright-protocol`: 14 library tests, 0 doctests
- Seven real loopback WebSocket acceptance tests run inside the server library:
  - two authenticated clients, attach ordering, movement, Say fanout, stream sequence/epoch checks, and standard WebSocket Ping/Pong;
  - unauthenticated/wrong-Origin upgrade refusal and shared-router logout revocation;
  - authenticated takeover, `session_replaced`, and continued replacement Look execution;
  - graceful shutdown closing an attached socket before the serving task completes.

The server tests use a real `BoundServer` on `127.0.0.1:0`, shared Router login/logout requests, explicit `Origin` and cookie headers, bounded socket send/receive operations, and deterministic fixture cleanup. No external service, database, browser process, or manual accessibility test was used.

## Protocol and server evidence

Protocol tests cover strict JSON DTOs, duplicate/unknown/trailing/non-object rejection, frame and semantic bounds, version negotiation, server envelope round trips, snake_case tags, and the compatibility status constant. Server tests cover strict account registries including empty/oversized/over-capacity/malformed/duplicate-field negatives, Argon2id fixed profile, generic authentication failures, session expiry/capacity/pruning, origin/configuration rules, owner attachment/takeover/revocation/cancellation behavior, slow recipients, Accepted ordering, NotFound exits, absolute handshake deadlines, unsupported-version and binary-frame refusal, browser source constraints, static security headers, and smoke CLI behavior.

Resolved transport versions include:

```text
argon2 0.5.3
axum 0.8.6
futures-util 0.3.31
getrandom 0.3.4
rpassword 7.4.0
tokio 1.48.0
http-body-util 0.1.3
tokio-tungstenite 0.28.0
tower 0.5.2
serde 1.0.228
serde_json 1.0.145
```

This evidence establishes a locally verified development slice only. It does not establish durable sessions, restart recovery, public deployment, built-in TLS termination, proxy trust, stable SDK/protocol compatibility, production capacity, or Linux/hosted-CI acceptance.
