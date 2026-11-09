# Milestone 9 beta-load contract

This file fixes the measurement recipe before implementation or execution. It
is not evidence that the target passed.

## Fixed profile

The `beta-250-v1` profile has no tunable acceptance parameters:

- 250 distinct local-credential accounts backed by the target PostgreSQL
  world.
- Four concurrent login/handshake workers. A `503` login response is retried
  at most eight times after delays of 250, 500, 1,000, 2,000, 4,000, 4,000,
  4,000, and 4,000 milliseconds. Other non-success responses are failures.
- Every client must authenticate, open one WebSocket, negotiate protocol v1,
  and attach before measurement starts. Login plus attachment has a
  120-second deadline.
- Once all 250 clients reach a common barrier, each sends 30 `look` commands,
  for exactly 7,500 measured commands.
- The command period is 2,000 milliseconds. Client `i` has a deterministic
  phase offset of `floor(i * 2000 / 250)` milliseconds and sends command `k`
  at `barrier + offset + k * 2000ms`.
- Request IDs are unique for the run and command. No reconnect or retry of a
  gameplay request is allowed in the measured phase.
- Command latency begins immediately before writing the complete WebSocket
  command and ends when the matching `command_result` is decoded. Accepted,
  output, and exits messages are drained but do not end the measurement.
- Each command has a 10-second deadline. Any protocol error, server error
  envelope, timeout, unexpected close, decode failure, or unmatched terminal
  result fails that client and the profile.
- `/health/ready` is requested once per second throughout the measured phase.
  Every response must be HTTP 200. Readiness polling uses the same trusted TLS
  roots as login and has a 5-second request timeout.
- The whole measured phase has a 90-second deadline.

Latency percentiles use nearest rank over all successful command durations in
integer microseconds: sort ascending and select
`max(ceil(p * N), 1) - 1` for `p = 0.50, 0.95, 0.99`.
P2 consolidates that existing arithmetic into
`worldwright_core::measurement::nearest_rank_index`, named
`nearest-rank-integer-v1`. Its quotient/remainder form avoids integer overflow.
This is NOT a beta-profile/report-schema/threshold change; unlike the sequential
runtime v1-to-v2 correction, beta statistics already used nearest rank. Throughput is successful
commands divided by wall-clock seconds from the barrier to the final matching
result. Failed runs still retain all observed counts and latencies.

`beta-250-v1` passes only when all of these are true:

- 250 clients attached before the barrier.
- Exactly 7,500 commands completed.
- Login, attachment, command, protocol, disconnect, and readiness failures are
  all zero.
- p95 command latency is at most 750 milliseconds.
- p99 command latency is at most 1,500 milliseconds.
- Maximum command latency is below the 10-second command deadline.
- Measured wall time is at most 90 seconds.

## Evidence inputs

The runner requires a bounded server revision, exact 64-lowercase-hex active
artifact digest, and a non-secret environment label. It records those values,
profile version, operating system/architecture, start/end Unix timestamps,
counts, integer latency statistics, throughput, and pass/fail reasons in a
create-new JSON report. It never writes passwords, cookies, session tokens,
response bodies, OIDC values, or database URLs.

The network client cannot prove which storage adapter the server selected.
Acceptable Milestone 9 evidence therefore also requires operator-captured
deployment configuration and PostgreSQL monitoring for the same run. A local
or in-memory run is a tooling smoke test, not `beta-250-v1` evidence.

## Fixture generation

The fixture generator creates exactly 250 unique `load001` through `load250`
accounts and matching characters with deterministic nonzero IDs. A password
is read from a named environment variable, validated by the server's password
bounds, hashed once with the repository Argon2id profile and a CSPRNG salt,
then used for all generated seed records. The separate plaintext credentials
file is create-new, explicitly sensitive test material, and must be deleted
after the isolated load environment is destroyed.
