# Milestone 8 performance evidence

Observed 2026-11-07 on the uncommitted working tree based on `66dc092`, Windows
x86_64, Windows build 26200, Intel Core i7-13850HX, 68,388,900,864 bytes
physical memory, Rust/Cargo 1.85.1, release profile.

Command:

```text
cargo run -p worldwright --release --locked --offline -- benchmark --clients 100 --commands-per-client 100 --json
```

Observed report:

```json
{"clients":100,"commands":10000,"elapsed_ns":23091800,"p50_ns":2100,"p95_ns":2600,"p99_ns":3700,"max_ns":50900,"delivered_messages":20000}
```

This run has no separate warm-up; release compilation completed before timing
began. The dataset is one room, 100 attached actors, and 10,000 sequential
runtime-backed Look commands. Each Look delivered two private messages. The
harness bounds clients to 100 and commands to 10,000.

The result proves repeatability of the selected development objective, not
network or beta capacity. It excludes HTTP/WebSocket framing, idle heartbeats,
owner queues, high fan-out speech, slow consumers, memory/CPU sampling,
PostgreSQL contention, reward retries, outbox recovery, content activation,
and long-running leak detection. There is no before/after optimization claim:
M8 establishes this baseline first rather than inventing an earlier number.
Those workloads remain required before any supported capacity commitment.
