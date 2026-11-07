## Studio client boundary

Studio is a loopback client for the authenticated release POST/GET routes. It
keeps the operator bearer token server-side, sends exact artifact bytes and
review context, and persists only bounded response records. PostgreSQL remains
the authorization and audit authority. Studio does not bypass current grants
or activate releases locally.

# Operations v1

**Status:** Milestone 6 operational control is implemented awaiting live
PostgreSQL and recovery execution. It is not production readiness evidence.

## Authentication and authority

Operator tokens are compact `worldwright-operator-v1` HMAC-SHA256 tokens issued
by the local MFA-capable Console. Claims bind subject, session, kind,
assurance, issuer, audience, issue/authentication time, expiry, and optional
break-glass reason. Signing keys are supplied through environment variables and
are never command-line arguments or output.

Every mutating request carries a correlation ID and reason. The database
rechecks the current operator, enabled state, token-valid-after timestamp,
revoked session, capability, environment, and fixed world scope in the same
transaction as the mutation and operator audit. Actor strings are not
authentication; possession of database credentials is the staging trust
boundary.

Human MFA and break-glass assurance are required for moderation, review,
activation, drain, recovery, operator management, and restore. Service
operators are limited to status, request, and backup capabilities.

## Capability scope

The v1 capabilities are `view_status`, `moderate_account`,
`request_release`, `review_release`, `activate_release`, `drain_world`,
`initiate_recovery`, `manage_operators`, `backup_world`, and `restore_world`.
Grants bind capability, environment, fixed world, operator subject, reason, and
optional expiry. Production activation requires an authorization-version 1
request, separate review, current `ActivateRelease` authorization, and a
current database grant. Legacy APIs are staging-only and authorization-version
0.

## API surface

When configured, the loopback server mounts:

| Method | Path | Capability |
|---|---|---|
| GET | `/api/v1/operator/status` | `view_status` |
| GET | `/api/v1/operator/metrics` | `view_status` |
| POST | `/api/v1/operator/moderation` | `moderate_account` |
| POST | `/api/v1/operator/drain` | `drain_world` |
| POST | `/api/v1/operator/sessions/revoke` | `manage_operators` |
| POST | `/api/v1/operator/operators` | `manage_operators` |
| POST | `/api/v1/operator/operators/enabled` | `manage_operators` |
| POST | `/api/v1/operator/grants` | `manage_operators` |
| POST | `/api/v1/operator/grants/revoke` | `manage_operators` |

Requests use `Authorization: Bearer <token>`, strict deny-unknown JSON, bounded
fields, and caller-supplied correlation IDs for mutations. Responses do not
leak database or authentication details. Status/metrics expose bounded
lifecycle, connection, release, writer, and outbox data.

## Console

`worldwright-console` is a real loopback API client and local credential/token
issuer. Credential files contain Argon2id PHC hashes and, for human or
break-glass operators, a TOTP secret. The Console prompts locally and prints a
token only when explicitly issuing one. API requests use direct `TcpStream`,
loopback HTTP, bounded framing, fixed timeouts, and no user/request labels.

## Drain and recovery

Drain closes admission before lifecycle transition, records `drain_started`,
waits within the bounded deadline, records `drain_completed`, and only then
reports quiescence. A failure to record completion does not claim completion.

Backup manifests provide HMAC integrity only. Confidentiality and encryption
at rest are external operator-controlled storage concerns. Wrapped PostgreSQL
tools execute directly without a shell, with a 600-second bound and cleared
environment. A PostgreSQL service name is operator configuration and must map
to the confirmed target.

Restore requires an empty confirmed target and verifies migrations, active
artifact integrity/release identity, world compatibility, and durable
references. Restore never acquires authority. `recovery takeover` requires
explicit old-writer isolation confirmation and rotates the authority
incarnation.

## Limits and evidence

RPO/RTO targets are 24 hours/4 hours and are unproven until live recovery
execution. The 90-day audit retention policy has no automated purge. Listener
transport remains loopback without TLS. The security-event table exists, but
unauthenticated token failures are bounded metrics rather than durable events.
PostgreSQL ignored tests currently compile but do not run; no manual Console or
browser, pg_dump/pg_restore, Linux, hosted CI, deployment, or production-host
test is claimed.
