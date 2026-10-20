# Worldwright Transport Protocol v1

This document describes the implemented development protocol in `worldwright-protocol` and the `worldwright-server` WebSocket gateway. It is a bounded JSON text protocol for the local two-player transport slice, not a stable public SDK or production compatibility promise.

## Framing and state

Each WebSocket text message is at most 16 KiB (16,384 UTF-8 bytes). The server rejects oversized, malformed, positional, trailing, duplicate-field, and unknown-field JSON. Binary gameplay frames are rejected. Standard WebSocket Ping/Pong control frames are transport keepalives and are not protocol messages; the server forwards Ping payloads as Pong frames. Application-level `ping`/`pong` messages are JSON messages carrying a `u64` nonce.

The application state machine is:

1. The first non-control message is `hello`.
2. The server negotiates protocol v1 and sends `welcome`.
3. The client sends `attach`; the server admits it through the world owner and sends `accepted`.
4. The server sends `attached`, two initial room `output` messages, `exits`, and `command_result` from an initial runtime-backed Look.
5. The client may send bounded commands and application `ping` messages.

Handshake and attached-idle receive deadlines are bounded. Fatal protocol, authentication, state, takeover, and runtime errors are sent as one fatal envelope when possible and then close the stream. Nonfatal command errors leave the stream usable.

## Envelope and JSON examples

Every server application message is nested in this envelope. `stream_epoch` is a random 32-character lowercase hexadecimal value for one connection. `sequence` starts at 1 and increases by one for every server envelope.

```json
{
  "protocol_version": 1,
  "stream_epoch": "0123456789abcdef0123456789abcdef",
  "sequence": 1,
  "message": {
    "type": "welcome",
    "heartbeat_interval_ms": 20000
  }
}
```

Client messages are unwrapped text JSON objects:

```json
{"type":"hello","supported_versions":[1]}
{"type":"attach","request_id":"attach-1"}
{"type":"command","request_id":"look-1","command":{"type":"look"}}
{"type":"command","request_id":"move-1","command":{"type":"move","exit":"north"}}
{"type":"command","request_id":"say-1","command":{"type":"say","text":"hello"}}
{"type":"ping","nonce":42}
```

Server message examples are nested under the same envelope shape:

```json
{"protocol_version":1,"stream_epoch":"0123456789abcdef0123456789abcdef","sequence":2,"message":{"type":"accepted","request_id":"attach-1"}}
{"protocol_version":1,"stream_epoch":"0123456789abcdef0123456789abcdef","sequence":3,"message":{"type":"attached","request_id":"attach-1","character_name":"Alice"}}
{"protocol_version":1,"stream_epoch":"0123456789abcdef0123456789abcdef","sequence":4,"message":{"type":"output","request_id":"attach-1","kind":"room","speaker":null,"text":"Atrium"}}
{"protocol_version":1,"stream_epoch":"0123456789abcdef0123456789abcdef","sequence":5,"message":{"type":"output","request_id":"attach-1","kind":"room","speaker":null,"text":"Sunlight fills a quiet entry hall."}}
{"protocol_version":1,"stream_epoch":"0123456789abcdef0123456789abcdef","sequence":6,"message":{"type":"exits","request_id":"attach-1","exits":["north"]}}
{"protocol_version":1,"stream_epoch":"0123456789abcdef0123456789abcdef","sequence":7,"message":{"type":"command_result","request_id":"attach-1","revision":2}}
{"protocol_version":1,"stream_epoch":"0123456789abcdef0123456789abcdef","sequence":8,"message":{"type":"error","request_id":"move-1","code":"not_found","message":"exit not found","fatal":false}}
```

## Bounds and validation

- Offered protocol versions contain 1–8 nonzero, unique values. Negotiation selects the highest mutual supported version; v1 is currently the only supported version.
- Request IDs are 1–128 UTF-8 bytes. The first byte is ASCII alphanumeric; later bytes are ASCII alphanumeric or `.`, `_`, `:`, `-`.
- Exit labels are 1–64 bytes and match `[a-z][a-z0-9_-]*`.
- Say text is 1–4096 UTF-8 bytes, is not whitespace-only, and contains no Unicode control characters.
- Character names are 1–64 bytes, nonblank, and contain no controls.
- Server output text is bounded to 4096 bytes; error text is bounded to 512 bytes.
- A server envelope has protocol version exactly 1, a 32-byte lowercase hexadecimal epoch, and a nonzero sequence.
- Visible exits are limited to 64 labels. Welcome heartbeat intervals are 1,000–120,000 ms; the server publishes 20,000 ms.

`accepted` means bounded admission to the owner queue, not successful execution. Runtime revisions and operation IDs are process-local in-memory values. Request IDs are stream-local correlation values, not durable idempotency keys, receipts, or replay handles.

## Output and command semantics

Look emits room output, visible exits, and a command result. Move resolves the label only against the issuing character's current room, executes the runtime Move, emits movement output, and then performs a separate runtime-backed Look. The protocol does not claim those executions are one transaction. Say emits speech to exact current room recipients; the issuer receives its request ID and other recipients receive `request_id: null`. Speech includes the source character name. Before delivery, the owner revalidates the exact attachment and route.

A slow or closed recipient route is detached and cancelled without converting an already committed runtime execution into a command failure for other recipients. An authenticated takeover replaces the old attachment only after the replacement runtime attachment succeeds. The old stream receives a fatal `session_replaced` error and closes; an old disconnect cannot detach the replacement.

## Errors and close behavior

`ErrorCode` values are:

- `invalid_message`, `unsupported_version`, `protocol_state`
- `unauthenticated`, `unauthorized`, `invalid_command`, `not_found`
- `overloaded`, `stale_attachment`, `session_replaced`, `internal_error`

Fatal errors close the stream after the bounded error envelope. Nonfatal errors include the request ID when applicable and leave the stream available for another command. Session expiry, logout, and takeover cancel the exact connection out of band; they cannot be blocked by a full output queue. Session expiry/revocation uses `unauthenticated`; a stale actor uses `stale_attachment`; replacement uses `session_replaced`; slow consumers use `overloaded`.

The epoch and sequence detect a gap or wrong stream, but the server provides no replay buffer or resume operation. A reconnect performs a new authentication, stream epoch, handshake, and attachment.

## Compatibility policy

This is protocol v1 for the current development slice. `STATUS` reports `protocol v1 implemented; transport sessions are process-local`. Stable protocol/SDK compatibility windows, deprecation policy, and a public client promise remain open decisions. Later changes must preserve the explicit bounds and ownership boundaries or update this contract and its negative tests together.
