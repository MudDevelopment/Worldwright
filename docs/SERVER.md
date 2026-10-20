# Worldwright Server Transport v0.1

`worldwright-server` is the local development composition root for the authenticated browser transport slice. It loads validated world content and a separate account registry, owns one in-memory world runtime, and exposes bounded HTTP/WebSocket routes. It is not a production deployment, durable account service, TLS terminator, or database-backed server.

## Bootstrap and run

From the repository root, bootstrap the pinned workspace dependencies first:

```powershell
./scripts/dev.ps1 bootstrap
```

Generate one Argon2id password hash separately for each local user. The password is entered interactively and is never a command-line argument:

```text
cargo run -p worldwright-server --locked --offline -- hash-password
```

Create an untracked `accounts.local.json` beside the repository root. Replace each placeholder with the PHC output generated for that account; do not commit this file, passwords, or hashes:

```json
{
  "schema_version": 1,
  "accounts": [
    {
      "username": "alice",
      "password_hash": "<paste-generated-phc-hash>",
      "account_id": "00000000000000000000000000000001",
      "character_id": "00000000000000000000000000000001",
      "character_name": "Alice"
    },
    {
      "username": "bob",
      "password_hash": "<paste-generated-phc-hash>",
      "account_id": "00000000000000000000000000000002",
      "character_id": "00000000000000000000000000000002",
      "character_name": "Bob"
    }
  ]
}
```

Start the explicitly insecure loopback development server:

```text
cargo run -p worldwright-server --locked --offline -- serve --content examples/two-room/world.json --accounts accounts.local.json --origin http://127.0.0.1:3000 --allow-insecure-http
```

Visit `http://127.0.0.1:3000`. The insecure flag is local development only. Keep `accounts.local.json` untracked and treat its password hashes as sensitive operational inputs.

## CLI and exit behavior

```text
worldwright-server --help|-h
worldwright-server --version|-V
worldwright-server status
worldwright-server hash-password
worldwright-server serve --content <path> --accounts <path> --origin <origin> [--bind <socket-address>] [--allow-insecure-http]
```

The default bind is `127.0.0.1:3000`; every non-loopback bind is rejected. `serve` requires each content, account, and origin option exactly once. Duplicate, missing, malformed, unknown, or extra options are usage failures with exit code 2. Help, version, and status return 0. Password hashing and server startup/runtime failures return 1. `status` reports protocol/browser transport implementation and process-local state; it is not a readiness probe.

The default mode requires an `https://` configured origin and emits Secure cookies while the application listener remains plaintext loopback. The separate secure-edge deployment must terminate TLS and WebSocket transport before forwarding to that listener. `--allow-insecure-http` requires `http://` and a localhost or loopback origin host; it must never be inferred silently.

## HTTP and WebSocket routes

- `GET /` — keyboard-first browser shell.
- `GET /app.js`, `GET /app.css` — same-origin assets loaded through `include_str!`.
- `POST /api/v1/session` — strict login JSON and 204 session creation.
- `DELETE /api/v1/session` — exact-Origin logout, revocation, and cookie expiry.
- `GET /api/v1/play` — exact-Origin authenticated WebSocket upgrade.
- `GET /health/live` — lightweight process liveness.
- `GET /health/ready` — 200 only while the world owner accepts work, otherwise 503.

Responses carry the CSP, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, and `Cache-Control: no-store` headers. Login, logout, and WebSocket upgrade require byte-exact equality with the one configured Origin; there is no wildcard authenticated CORS and no query-string token.

## Account and session boundary

The registry is strict JSON schema v1, limited to a 1 MiB file and 256 accounts. It requires 1–64-byte normalized usernames matching `[a-z][a-z0-9_-]*`, unique parsed account/character IDs, bounded nonblank names without controls, and unknown/duplicate-field rejection. Password hashes must be Argon2id v19 with the fixed profile `m=19456,t=2,p=1` and 32-byte output, with random salts and no keyid/data.

Login bodies are limited to 4 KiB; passwords are 1–1024 bytes without controls. Four concurrent blocking Argon2 verifications are admitted, and unknown users and wrong passwords receive the same short 401 response. Successful sessions use a 32-byte random opaque lowercase-hex token stored only server-side, have an absolute 12-hour expiry, and are limited to 1024 active entries after expired-session pruning. Cookies are `HttpOnly; SameSite=Strict; Path=/; Max-Age=43200`, with `Secure` in default mode.

Logout revokes the exact token and cancels only routes carrying that token. Expiry is rechecked during handshake and while attached. Authenticated takeover and stale attachment generations prevent an old connection from detaching a replacement.

## Bounds and shutdown

The HTTP body limit and protocol frame/message limit are 4 KiB and 16 KiB respectively. Gameplay admission is bounded to 256 entries, control operations to 64 entries, and each connection output queue to 64 entries. Control operations have one-second deadlines. Handshake, idle receive, socket writes, and cleanup have bounded timeouts. Slow consumers are cancelled out of band and detached without blocking the world owner. Ctrl-C performs graceful server shutdown, route cancellation, runtime detachment, and listener shutdown.

## Non-goals and limitations

This increment deliberately does not provide PostgreSQL, durable accounts or sessions, restart recovery, durable receipts, registration, recovery, MFA, roles, moderation, inventory, combat, NPCs, scheduling integration, Studio/Console behavior, public deployment, built-in TLS termination, proxy-header trust, containers, stable SDK/protocol compatibility, rate limiting beyond bounded password verification, or production capacity/availability evidence. Browser assets have automated source/transport tests; no interactive browser accessibility assessment has been performed.
