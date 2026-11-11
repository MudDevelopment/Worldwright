# Worldwright — Security Policy and Design Baseline

**Protect people, credentials, player progress, and the authority to change a world.**

| Field | Value |
|---|---|
| Document version | 0.1 |
| Updated | November 9, 2026 — P1 and R0/R1 controls; not a certification |
| Status | M4/M5 controls await live PostgreSQL verification; M6 is implemented awaiting live PostgreSQL/recovery execution and is not production ready |
| Accountable roles | Security owner and service owner, with authorized project-owner oversight |
| Related documents | [README](README.md), [Design](DESIGN.md), [Architecture](ARCHITECTURE.md), [Contributing](CONTRIBUTING.md), [Conduct](CODE_OF_CONDUCT.md) |

## Current M4/M5 controls

Durable writes use writer fencing, receipts, atomic audit/outbox work, release
read guards, maintenance/pending activation state, and stale-writer refusal.
Release artifacts use immutable create-new filesystem output and insert-only
application APIs; they are bounded, canonical, digest-verified, and validated
through the strict content loader. Preview clears inherited
environment state, uses bounded ownership-safe slots, and runs artifact-selected
reference or expedition gameplay with in-memory state only.

These controls do not establish authorship or authorization: an artifact digest
is not a signature. Operator authority requires an auth-v1 token and current
database grant; database credentials alone are insufficient. Preview is not an OS sandbox
or network-egress firewall. Production activation requires auth-v1 request,
separate review, and a current MFA-backed database grant. Backup manifests
provide HMAC integrity only; confidentiality/encryption at rest is external.
RPO/RTO targets and 90-day audit retention are unproven/unautomated. Listener
transport remains loopback without TLS. Security-event storage exists, but
unauthenticated token failures are bounded metrics rather than durable events.
Live PostgreSQL and recovery behavior remain unverified.

> **Important:** No released Worldwright software, supported-version roster, deployed service, private reporting endpoint, or staffed response team has been verified. “Must” below describes a release requirement, not an implemented-control claim. This document does not certify Worldwright as secure or production-ready.
>
> **Reporting channel pending:** Do not post an unmitigated exploit, credential, private player record, or vulnerability report with sensitive details in a public issue. Do not use a guessed security email address. See the reporting process below.

## P1 security boundaries and unresolved approval gates

- The session-resume probe uses the existing cookie; no token/principal data is
  returned. Authentication still occurs at WebSocket handshake and gameplay.
  Browser generation guards reject stale UI responses; this is not a claim
  that every concurrent login/logout Set-Cookie race has been certified.
- `WORLDWRIGHT_TELEMETRY_BIND` is optional, separate and loopback-only with an
  explicit nonzero port. It does not enable operator authority or trust public
  proxy headers. Metrics have six fixed stage dimensions and no raw player
  text, token, account, subject or request labels. Same-host users can access
  loopback telemetry; isolate the host or use an approved local proxy if required.
- Linux preview slots now use a per-process-owner directory and require current
  ownership/mode 0700; new lock files request 0600. Existing non-private paths
  fail instead of being silently chmodded. `/proc/self` supplies Linux owner
  identity; no portability claim is made for non-Linux Unix without that source.
- Preview Unix process groups and Windows taskkill are cleanup mechanisms, not
  containment. Real crash/SIGTERM/descendant/race tests and Windows ACL/key-file
  protection remain open. Console signing authority/key rotation remains an
  explicit security review; it was not removed or certified in this change.
- External outbox dispatch is unsupported. Expired-session hash pruning is
  bounded and preserves durable receipts, business uniqueness, OIDC links and
  audit rows. No 90-day deletion promise or adopted broader retention policy
  is supplied. Keep test reports/logs free of credentials and player content.
- Protected beta-load CI uses only an explicitly reviewed acceptance deployment,
  default branch and required-reviewer environment. Credentials are temporary,
  removed on step exit, and excluded from artifacts. Required reviewers and
  branch protection must actually be configured; YAML does not adopt them.
- Release evidence hashes and Cargo metadata SBOM declarations do not prove
  publisher identity, licensing rights or authorized owner decisions. Review
  exact code/artifact/container tuples, advisories, notices, signing policy,
  private reporting/support owners and service acceptance before distribution.
  `publish = false` remains in force.

See `docs/P1_ACCEPTANCE.md` and `docs/verification/p1-implementation-ledger.md`.

## R0/R1 package, process and evidence boundaries

Private acceptance packaging requires complete same-source Ubuntu R0 logs,
source/lock inventory and matching binaries. It rejects malformed inventory
records, bad hashes/modes, symlinks and unexpected files. Existing output files
are not overwritten. The package contains no operator keys, account registries,
production configuration or license grant. Hash verification does NOT authenticate
a publisher if both manifest and files are replaced; verify external archive/
evidence digests and reviewer authority before trusting a package. Native code
and runtime libraries remain trusted code with user-process privileges.

Installed mode uses no Cargo/checkout for supported Studio edit/package/preview
operations. It still requires trusted runtime executables, Python 3.11+, Bash,
Linux setsid/pidfds and compatible libraries. Owned private directories and
no-symlink rules tighten filesystem exposure; no automatic chmod or ownership
repair is performed. Shell path checks are not race-proof protection in an
attacker-controlled directory hierarchy.

Cleanup uses a Studio session and pinned pidfds, includes preview process groups,
and retains failure on forced cleanup. It is not a sandbox against malicious
native code escaping sessions. Browser processes are not terminated with Studio.
Real timeout/pipe/crash and supported-host behavior must pass actual acceptance.

R0 logs/config fingerprints are private evidence, not authorization. Database URLs
are supplied via approved environment/secret delivery, not command arguments;
known test URL logging is redacted. Review compiler/tool output for sensitive
material before sharing: this is not universal log DLP. The reviewed Windows
workflow needs required environment reviewers and an acceptance-only disposable
DB secret; merely committing workflow YAML does not install those protections.

R1 automation cannot approve Firefox/X11/Wayland/accessibility, Windows credentials
or human reviewer identity. The manual template is intentionally unobserved and
must remain pending until authenticated observations exist. Closure checks complete
source/log/host/package identities and rejects missing/invalid manual records;
it does not grant legal rights, operational authority or R2+ readiness.
See `docs/R0_R1_ACCEPTANCE.md` and the R0/R1 ledger for evidence and limitations.

## Contents

- [Reporting a Vulnerability](#reporting-a-vulnerability)
- [Supported Versions](#supported-versions)
- [Response and Disclosure](#response-and-disclosure)
- [Authorized Research](#authorized-research)
- [Scope and Shared Responsibility](#scope-and-shared-responsibility)
- [Threat Model](#threat-model)
- [Authentication and Session Security](#authentication-and-session-security)
- [Authorization and Administrative Safety](#authorization-and-administrative-safety)
- [Browser and Transport Security](#browser-and-transport-security)
- [World State and Economic Integrity](#world-state-and-economic-integrity)
- [Content and Extension Security](#content-and-extension-security)
- [Build and Supply Chain](#build-and-supply-chain)
- [Deployment and Secrets](#deployment-and-secrets)
- [Privacy and Observability](#privacy-and-observability)
- [Incident Handling and Recovery](#incident-handling-and-recovery)
- [Verification and Release Gates](#verification-and-release-gates)
- [Open Security Decisions](#open-security-decisions)

## Reporting a Vulnerability

### Contact readiness

| Required facility | Current state |
|---|---|
| Verified primary private reporting route | Not established |
| Independently verified fallback contact | Not established |
| Named security owner and backup | Roles proposed; appointments not supplied |
| Encrypted evidence-transfer method, if needed | Not selected |
| Staffed response coverage and target times | Not established |
| Advisory and affected-version publication location | Not established |

Before external beta, publish and test the primary route and a fallback. Suitable mechanisms might include a monitored security mailbox, a private form, or repository-host private vulnerability reporting **if the actual repository supports and enables it**. Do not advertise a platform feature merely because another repository uses it.

### While the channel is pending

1. If you already know an independently verified private project-maintainer contact, ask for a secure reporting route before sending sensitive evidence.
2. Otherwise, make only a nonsensitive request for a private security contact through the official project channel once one exists. Do not include exploit details, affected private accounts, or secrets.
3. For an incident on an independently operated game, contact that operator through its published private route. A toolkit maintainer cannot revoke credentials or inspect accounts on an unrelated deployment.
4. If there is no safe verified route, retain the details securely rather than publishing them to force a response. This is a reporting-readiness gap the project must resolve, not a promise of monitored service.

For harassment or retaliation, use [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). A report may involve both security and conduct; share only the information each response team needs, and avoid conflicted recipients.

### Useful report contents

Include what you can establish safely:

- Affected component, build/commit, content release, and deployment context.
- Expected behavior versus observed behavior.
- Preconditions, required privileges, and whether the issue crosses a trust boundary.
- Minimal reproduction using an isolated test world and synthetic accounts.
- Potential effect on confidentiality, integrity, availability, or player assets.
- Sanitized logs/screenshots and a description of any exposed data—not bulk data copies.
- Whether exploitation is known or only hypothesized.
- Workarounds or an optional patch, if available.
- A safe reply method and whether you want public credit.

Do not include live tokens, passwords, private keys, full production databases, or unnecessary chat logs. If a secret is already exposed, tell the authorized operator what kind of secret and where it leaked so it can be rotated; do not repeatedly distribute the value.

## Supported Versions

There are **no verified supported security releases**. Current `0.1.0-dev.0` source includes local transport, durable PostgreSQL identity/gameplay and restricted staging tooling. Their native/live/adoption evidence is incomplete; there is no accepted public deployment or production security-support commitment. See [server guide](docs/SERVER.md) and [verification limits](docs/VERIFICATION.md).

| Artifact | Security support status |
|---|---|
| This documentation package | Planning material; corrections can be reviewed through established project channels once available |
| Unreleased `0.1.0-dev.0` development snapshot | Local ownership/attachment, authenticated loopback transport, and bounded protocol validation tested; no durable persistence or production support commitment |
| Future tagged releases | Support policy must be announced with actual versions and end-of-support dates |
| Independent games and modified deployments | Operator responsibility; upstream applicability assessed case by case |

The local `attach` API is for a trusted host and is not safe to expose directly to clients. Context IDs are not cryptographic capabilities. Reconnect generations are instance-local; the host must discard contexts on runtime teardown until restart-safe session epochs exist. Native rule effects outside owned world state, including RNG consumption, are not rolled back. See [RUNTIME.md](docs/RUNTIME.md) for the precise atomicity and security boundary.

Before a release, publish the supported engine/SDK ranges, platform/dependency assumptions, update path, advisory location, and support period. A branch name alone is not a maintenance commitment. “Latest” is not an adequate supported-version table.

The project may investigate reports affecting unsupported code, but must not promise a fix, backport, or response time it cannot provide.

## Response and Disclosure

### Proposed process after activation

1. Acknowledge through the verified channel and assign a private tracking identifier.
2. Triage impact, affected versions/configurations, prerequisites, and evidence quality.
3. Reproduce in isolation without expanding exposure.
4. Coordinate with affected maintainers, dependency projects, and operators on a need-to-know basis.
5. Implement containment, a fix, and relevant regression tests.
6. Verify release and recovery implications, including data repair where necessary.
7. Agree on practical disclosure coordination with the reporter, considering ongoing exploitation and operator risk.
8. Publish affected/fixed versions, impact, mitigations, upgrade steps, and credit if requested.
9. Record lessons and follow-up controls without exposing unnecessary sensitive material.

Response targets and update cadence must be selected after staffing exists. There is no active SLA, guaranteed patch deadline, guaranteed bounty, or promise of indefinite nondisclosure in this draft.

### Severity considerations

Use a documented scoring method plus game-specific impact. Account takeover, unauthorized staff access, private-message exposure, persistent item duplication, arbitrary code execution, and loss of world-writer integrity can warrant high urgency even when a generic availability metric looks healthy.

A missing best-practice header and a demonstrated cross-account compromise are not equivalent. Neither should be dismissed without understanding the actual exposure.

### Disclosure quality

Advisories should distinguish proven exploitation, confirmed vulnerability, and precautionary hardening. Do not label every repair a “security fix,” hide affected versions, or claim all deployments are vulnerable when only a specific configuration is affected.

Restrict exploit details while a coordinated fix is being prepared, but do not use the conduct policy or license discussion to retaliate against a good-faith reporter.

## Authorized Research

This document does **not** grant permission to test any third-party deployment, promise legal safe harbor, or waive an operator's rules. Any safe-harbor program must be approved by the owner for systems they control and published separately with clear scope.

For testing:

- Use your own isolated instance or obtain explicit authorization with scope and limits.
- Use synthetic data and accounts you control.
- Stop when a meaningful impact is demonstrated; do not exploit unrelated users to strengthen a report.
- Avoid denial-of-service tests, mass scanning, phishing, credential guessing, persistence, or data extraction outside explicit authorization.
- Do not alter or destroy another person's assets, logs, or progress.
- Preserve minimal evidence and report promptly through a safe channel.

A permissive software license allows use under its terms; it is not permission to attack a public service running that software.

## Scope and Shared Responsibility

| Party/component | Security responsibility |
|---|---|
| Engine maintainers | Correctness of runtime boundaries, supported interfaces, persistence, and defaults |
| Tooling maintainers | Safe file/project handling, validation, preview isolation, and publishing controls |
| Game developers | Trusted game rules, extension dependencies, game-specific abuse and integrity constraints |
| Operators | Configuration, patching, credentials, network exposure, backups, monitoring, and incident response |
| Community staff | Scoped investigation, conduct handling, evidence minimization, and access discipline |
| Release maintainers | Artifact integrity, dependency/license review, provenance, and publication credentials |
| Players/builders | Follow service rules and report issues; they are not responsible for compensating for broken server authorization |

Trusted Rust game modules execute in the server's trust domain. Rust's safety features reduce some memory hazards but do not prevent privilege mistakes, malicious dependencies, logic flaws, secret disclosure, or resource exhaustion.

A defect in the promised SDK boundary can still be a toolkit issue even if a game triggers it. Conversely, a deliberately malicious native module with server process privileges is not made safe by calling it an “extension.”

## Threat Model

### Assets and actors

Protect credentials, account/character associations, private communications, player progress, item/currency ownership, content releases, operator authority, recovery material, and release infrastructure.

Consider unauthenticated attackers, abusive authenticated players, malicious or compromised builders, overprivileged staff, compromised dependencies, stolen deployment credentials, and accidental operational mistakes.

### Important boundaries

| Boundary | Representative threat | Required control/evidence |
|---|---|---|
| Internet → edge/server | Flooding, malformed frames, password attacks | Layered limits, timeouts, parser tests, bounded work |
| Browser → session/control services | CSRF, cross-site WebSocket hijacking, session theft | Origin/CSRF checks, secure cookies, revocation tests |
| Player identity → character/action | Cross-account selection or queued stale privileges | Server-bound identity and execution-time authorization |
| Tool → content publication | Unreviewed or unauthorized activation | Scoped permissions, immutable approved artifacts |
| Runtime → PostgreSQL | Injection, stale writer, duplicate valuable operations | Typed/parameterized access, fencing, transactions, receipts |
| Staff → sensitive data/repair | Excessive access or untraceable changes | Capabilities, scope, independent approval, audit |
| Dependency/build → release | Malicious package, credential theft, artifact substitution | Isolated CI, reviewed dependencies, pinned immutable artifacts |
| Production → backups/logs | Secondary data leak or unsafe restoration | Encryption/access policy, redaction, retention, restore testing |

No required boundary is satisfied merely by hiding a UI control or placing a tool on a staff member's laptop.

## Authentication and Session Security

The baseline follows [DESIGN.md](DESIGN.md#identity-and-authorization): browser form login and revocable server-side sessions.

### Password accounts

- Use Argon2id with independent random salts and full encoded parameter/hash storage.
- Select memory/time costs through security review and load tests; do not copy arbitrary values into production without measurement.
- Bound concurrent hash work, password input size, login retries, and recovery requests.
- Use cryptographically secure randomness for session, recovery, and other authentication secrets. Gameplay PRNGs are not suitable.
- Do not store plaintext or reversibly encrypted passwords as the account verifier.
- Avoid account enumeration through recovery/login messages while retaining safe operational diagnostics.
- Define rehash and legacy-credential migration behavior explicitly; do not silently reinterpret stored values.

### Sessions and account recovery

- Cookies require TLS, HttpOnly, an appropriate SameSite policy, deliberate path/domain scope, and protection from accidental logging.
- Rotate identifiers on authentication and privilege changes.
- Define idle/absolute expiry and revocation behavior.
- Logout, suspension, and relevant credential changes invalidate sessions and attached gameplay connections.
- Recheck queued-command authorization so a revoked session cannot finish an unrelated newly executed action solely because admission happened earlier.
- Recovery tokens must be short-lived, single-use, securely generated, and protected at rest according to the selected design.
- Recovery must not bypass stronger authentication or silently restore privileges without review.

The recovery delivery mechanism and identity-provider integration remain open. Basic Auth is **not** an implicit fallback. Automation requires an approved, scoped credential model rather than copying a person's permanent browser session.

### Privileged access

Require approved stronger authentication—normally MFA through an identity provider or a reviewed equivalent—before privileged production use in operational beta. Establish audited recovery and break-glass procedures without shared permanent administrator passwords.

## Authorization and Administrative Safety

Roles bundle explicit capabilities. Every action must check its target world, project, environment, account, or character scope.

- A builder cannot publish simply because they can edit a draft.
- A publisher cannot freely repair inventories or read private player conversations.
- A moderator does not automatically receive database, key, or deployment access.
- An operator's ability to restart the service does not imply authority to inspect every private record.
- The CLI has no production authority merely because a trusted developer runs it.
- Console commands follow the same domain invariants as player/system actions.

Sensitive repair, destructive changes, and exceptional privilege grants need a reason and independent approval where required by the adopted policy. Record the actor, target, decision, operation identity, and outcome without recording secrets.

Approvals must bind to the actual action/release/target and expire or invalidate when those details change. An approval for staging does not authorize production.

## Browser and Transport Security

- Use HTTPS and secure WebSockets for public connections.
- Keep one approved browser origin initially; do not use wildcard authenticated CORS.
- Validate allowed origins on browser WebSocket upgrades. Decide explicitly how missing origins are handled; future non-browser clients need their own approved authentication path.
- Apply CSRF defenses to cookie-authenticated state-changing HTTP operations, including authentication flows where applicable.
- CORS is not authentication, authorization, or a complete CSRF defense.
- Validate proxy headers only from configured trusted proxies; do not let arbitrary forwarded headers decide identity, origin, or rate-limit keys.
- Validate redirects so login or recovery does not create an open redirect.
- Escape player/content text and deploy an appropriate Content Security Policy. Rich text must use a defined safe representation.
- Bound frame/body size, decompressed size, nesting, parsing effort, queues, and output bytes.
- Prevent slow clients from exhausting memory or stalling unrelated connections.
- Protect private output at recipient selection and delivery/revocation boundaries; a guessed stream ID is not authorization.

Transport between hosts and the database must meet the selected deployment's authentication/encryption requirements. Do not assume a private network is sufficient for every environment. Mutual TLS, certificate issuance, and rotation are deployment decisions—not preexisting Worldwright features.

No specific application port, TLS implementation version, certificate provider, or proxy product is established by this policy.

## World State and Economic Integrity

State correctness is a security property where players can benefit from duplication, unauthorized ownership changes, or replay.

Maintain the [persistence/recovery contract](DESIGN.md#persistence-and-recovery):

- One authoritative simulation owner per world.
- PostgreSQL authority for durable state and an explicitly reconstructed runtime projection.
- Transactional valuable-state changes with uniqueness, ownership, and constraint checks.
- Stable operation identities and durable receipts for retry reconciliation.
- Request-key scoping and intent matching; an idempotency key is not a credential.
- Writer-epoch fencing enforced on durable mutations, not just in a process variable.
- No durable success acknowledged before a committed result is established.
- Unknown commit outcomes are reconciled or force safe recovery, never assumed to be rollback.
- Rewards/spawns use stable identities so restart or duplicate delivery does not mint value twice.
- Database outages do not permit unsafely acknowledged in-memory economic changes.

Receipt, audit, and outbox retention must be coordinated with replay protection and privacy. Deleting a record must not silently reopen an old valuable operation for execution.

Administrative repair requires the same integrity rules. Do not quietly fix inconsistencies with unrestricted SQL from a web console.

## Content and Extension Security

### Implemented local content-input boundary

The JSON loader enforces a 32 MiB input ceiling, 32 container levels, 200,000 values, bounded collections, and at most 100 semantic diagnostics. Duplicate object keys are rejected before map conversion; unknown fields, unsupported versions, unresolved references, invalid text, and invalid directed graphs cannot yield validated content. Core room/exit/text limits remain authoritative. Record objects cannot be supplied as positional arrays. See [schema v1](docs/content/SCHEMA_V1.md).

Diagnostics escape control and non-ASCII characters and bound each displayed component to 512 input characters. Raw diagnostics remain data: future HTML or network adapters must apply context-specific escaping. Loading executes no scripts, includes no other files, contacts no URLs, and grants no account/attachment authority. Supplied local file paths should be trusted regular files; the synchronous reader has no timeout or sandbox for device/FIFO paths. Validation is not immutable release approval, live activation, authentication, or a security certification.

### Implemented local terminal play

`worldwright play <path>` is a trusted single-user development host, not authentication. Local world/account/character/connection IDs are ephemeral and never derived from user commands. Gameplay uses the existing runtime authorization/movement checks; host lookup metadata is not authority. Output is filtered by exact current attachment and current-room labels; administrative snapshots are never rendered.

The input framer retains at most 4101 bytes plus bounded stdin buffering, drains oversized lines, and rejects invalid UTF-8/control-bearing input. This is memory bounding, not a deadline on local stdin. The renderer preserves full valid text and ordinary Unicode while escaping control, bidi-direction, and Unicode line/paragraph characters. Runtime actions are not rolled back when terminal delivery fails; Move and its automatic Look are separate executions. Cleanup attempts detach on quit, EOF, or session error. Ctrl-C retains ordinary OS termination behavior.

No network listener, credential handling, durable session, persistent state, or random generator is introduced by Local Play. See [Local Play](docs/LOCAL_PLAY.md).

### Implemented authenticated local browser transport

Transport v0.1 adds a loopback-only, exact-Origin browser service. Registry passwords use Argon2id v19 with `m=19456,t=2,p=1`, random salts, 32-byte output, 1 KiB password bounds, and four concurrent blocking verifications. Unknown and wrong passwords share a generic 401. Sessions use 32-byte random opaque server-side tokens, 12-hour absolute expiry, a 1024-entry cap with pruning, and `HttpOnly; SameSite=Strict` cookies that are Secure by default.

The service rejects wildcard/query-token authentication, requires exact Origin values, and exposes insecure HTTP only through an explicit local-development flag. Body, frame, gameplay/control/output queues, handshake, idle, control, socket-write, and cleanup bounds are enforced. Out-of-band cancellation handles logout, expiry, takeover, shutdown, and slow consumers; attachment generations and recipient revalidation prevent stale execution/delivery. CSP and `textContent` rendering keep browser output data-only.

That original local mode remains a static registry. Current PostgreSQL mode
adds durable sessions/revocation and identity linking; operator assurance is
separate from player login. Live provider/recovery acceptance remains open,
and neither local success nor configured TLS ingress establishes a production
security assessment. The browser assets and real loopback WebSocket cases are automated-tested; interactive accessibility testing has not been performed. See [SERVER.md](docs/SERVER.md) and [PROTOCOL_V1.md](docs/PROTOCOL_V1.md).

### Content and publication

Content is untrusted input until validated, even when submitted by a familiar builder.

- Enforce schema, size, reference, depth, and resource limits.
- Prevent path traversal, unsafe symlink following, archive escapes, and accidental overwrite in CLI/Studio import or scaffolding.
- Keep preview isolated from production accounts, secrets, and persistence.
- Treat embedded legacy expressions, markup, URLs, and scripts as potentially active material.
- Bind validation/review to an immutable content digest and release identity.
- Separate upload, validation, approval, and activation permissions.
- Do not activate a modified artifact under an earlier approval.
- Track provenance and applicable content licenses independently of runtime security checks.

A valid digest establishes integrity against an expected value; it does not by itself prove an artifact's author, approval, or trustworthiness.

### Executable extensions

The initial SDK permits **trusted** Rust game modules. It does not offer a security sandbox for arbitrary player or builder code.

## M8 toolkit controls

Generic GameAction IDs, actor counters, and delayed adjustments are bounded and
validated by the owner. Expedition gameplay randomness is injected gameplay
randomness, not authentication or security randomness. Native game modules are
trusted process code and are not sandboxed. Generated path dependencies and
templates carry no license grant. Expedition durable mode fails closed before
database connection until counters, schedules, receipts, and reconstruction
are implemented.

Any future scripting/plugin system requires an approved design for capabilities, host access, memory/instruction/time budgets, recursion, I/O, cancellation, scheduling, upgrades, and persistence. A language interpreter or WebAssembly runtime alone does not establish a complete sandbox.

Do not execute imported MUSHcode, MUF, MPI, LPC, or MOO behavior in production simply because the surrounding content file parsed successfully. See [DEFINITIONS.md](DEFINITIONS.md#compatibility-and-migration).

## Build and Supply Chain

- Review new crates, features, build scripts, procedural macros, and frontend dependencies; builds can execute third-party code.
- Pin reproducible application dependencies and toolchains according to the adopted release policy.
- Maintain dependency vulnerability and license checks with a reviewed, time-bounded exception process.
- Keep untrusted pull-request jobs separate from signing, publishing, deployment, and production-data access.
- Restrict workflow-change approval and CI token permissions.
- Avoid running untrusted builds on privileged persistent self-hosted runners.
- Protect package registries, source control, artifact stores, and release keys with strong authentication and scoped access.
- Produce a dependency inventory/SBOM and provenance evidence for releases where the selected tooling supports them.
- Verify artifacts against the approved source/build identity before deployment.
- Keep generated test/review claims reproducible; automated or AI-generated findings require human triage.

The project has not yet selected scanning, signing, or attestation tooling. No badge, security audit, or compliance certification is claimed.

## R3 restricted staging security boundary

Chart values format 2 intentionally rejects mutable images, shared migration/
runtime Secret names and disabled/unscoped NetworkPolicy. The private runtime
uses non-root UID/GID 65532, read-only root, dropped capabilities, no privilege
escalation or service-account token, scoped ingress/DNS/DB/HTTPS peers and no
public operator/recovery identity. Different Secret names do not prove different
logins: live ownership/grants/current ledger and actual Pod DB peer observations
are separate required checks. Runtime-grant setup is optional, explicit and
limited to a reviewed disposable worldwright_test database; it is not automatic
migration, account creation or recovery-authority adoption.

TLS roots/hostname are verified in live HTTPS/WSS checks; behind-tls-ingress
only permits the reviewed internal hop and does not grant arbitrary public HTTP
or forwarded-header trust. CIDRs are not dynamic FQDN policy or proof a CNI
actually enforces them. Positive controls must distinguish denial from broken
routing/dead targets. Rotation must separately prove old credentials revoked;
changing a reference or replacing a Pod is insufficient. Native SIGTERM/drain
and normal exit remain independent of seeing a replacement Pod become ready.

Build refs are approved immutable digests, not bundled default trust decisions.
Image config identity cannot substitute for the pushed OCI manifest. Staging
plans hold credential environment names/private proof paths, never credential
values or signing keys. Retained logs omit private API bodies and must not expose
cookies, DB URLs, passwords or tokens. Four independent real operator cases and
an independently installed Ed25519 service-adoption trust root are mandatory for
closure; synthetic helper keys grant no authority.

r3-acceptance.yml separates untrusted Ubuntu/Windows helper jobs from explicit
reviewed default-branch staging. Adopt required environment reviewers, protected
branch and an isolated ephemeral nonproduction runner before provisioning private
inputs. The workflow does not configure/adopt those protections or authorize
third-party testing. Current R0/R2 failures block actual staging; no source change
here rewrites applied migrations, supplies a service owner, establishes legal
rights or closes production gates. See docs/R3_ACCEPTANCE.md and its ledger.

## Deployment and Secrets

- Run with the least required operating-system privileges.
- Restrict the application listener behind the approved edge topology.
- Keep PostgreSQL and detailed monitoring endpoints private and authorized.
- Separate runtime, migration, backup, restore, and publication identities.
- Do not bake secrets into images, content packages, source files, examples, or default configuration.
- Resolve credentials through the chosen deployment secret mechanism and define rotation/revocation tests.
- Use controlled writable locations and a read-only application filesystem where practical.
- Treat takeover/replacement as an explicit fenced workflow, not an autoscaler contest.
- Keep staging and production data, credentials, and identities separate.
- Require deliberate production targeting and approval for destructive commands.

Version configuration and reject unsafe or unsupported settings with actionable errors. Do not silently fall back to plaintext transport, default administrative passwords, wildcard origins, or a development database.

## Privacy and Observability

Collect information for stated operational or community purposes, not because it is available.

### Data handling

| Data | Proposed handling requirement |
|---|---|
| Passwords, session tokens, recovery secrets, private keys | Never log plaintext values; tightly restrict storage and access |
| Account details and network identifiers | Minimize collection and retention; restrict investigative use |
| Character/world state | Protect integrity and define access/export/deletion semantics |
| Private messages and scene logs | Publish collection/retention rules; avoid blanket capture for debugging |
| Moderation and vulnerability reports | Separate confidential case storage with controlled access |
| Audit records | Record accountable actions while minimizing unrelated private content |
| Backups | Apply privacy and access controls, including retention and restoration consequences |

No numerical retention period is adopted here. The security, community, legal/business, and service owners must approve appropriate schedules before external beta, including deletion requests, legal holds, and backup expiration.

Normal logs should not contain authentication headers, cookies, request bodies with secrets, full chat transcripts, or dumpable private world state. Escape control characters and structured-log fields to prevent forged log entries.

Use metrics for rates and sizes rather than recording sensitive payloads. Avoid high-cardinality player identifiers or private content in labels. Any temporary diagnostic expansion needs authorization, an expiry, and secure handling.

## Incident Handling and Recovery

### Operational sequence

1. Assign an incident lead, record the time and known facts, and restrict sensitive communications.
2. Contain the affected surface: revoke credentials, restrict actions, withdraw readiness, or isolate the service as warranted.
3. Preserve minimal necessary evidence; do not wipe useful records solely to make symptoms disappear.
4. Determine affected builds, accounts, content releases, credentials, and durable operations.
5. Fix the cause and verify it in isolation.
6. Recover from trusted data/artifacts, reconciling receipts, inventory, privileges, and writer authority.
7. Rotate affected secrets and invalidate exposed sessions as appropriate.
8. Reopen only after an authorized go/no-go review and relevant invariant checks.
9. Communicate accurately with affected operators/participants and satisfy applicable notification duties with the appropriate legal owner.
10. Document lessons, ownership, and completion evidence.

An incident involving harassment or insider abuse also needs the conflict-aware conduct process. The reported person must not control their own investigation evidence.

### Backup and restoration

Backups need access controls, encryption appropriate to the environment, retention rules, and regular isolated restore exercises. Preserve the code/content/schema/config identities required to interpret restored state.

A restored database can revive previously revoked credentials or erase later bans/receipts. The recovery plan must reconcile security state and replay protection before reopening. Restoring progress from an earlier point can create external inconsistencies; communicate the chosen recovery boundary.

Do not claim zero data loss or a recovery time until measured. RPO and RTO are operator-approved objectives backed by drills, not guarantees provided by a backup file's timestamp.

## Verification and Release Gates

The following are required evidence categories, not completed tests:

| Gate | Required demonstration |
|---|---|
| Identity | Login/recovery abuse handling, expiry, logout, credential-change revocation |
| Scope | Cross-account/character/world denials; privilege changes affect queued operations |
| Browser | CSRF and origin rejection; safe rendering; proxy trust configuration |
| Resource limits | Oversized input, slow clients, queue pressure, bounded hash/parser work |
| Durability | Duplicate requests, injected crashes, unknown commits, projection recovery |
| Writer authority | Competing/stale writers cannot mutate durable world state |
| Content | Malformed/imported inputs, invalid references, changed-after-approval artifacts |
| Staff controls | Scoped actions, approval binding, audit integrity, separation of duties |
| Supply chain | Dependency review, isolated CI, controlled release credentials/artifacts |
| Operations | Secret rotation, tested restore, security-state reconciliation, fenced replacement |
| Reporting | Working private and alternate channels, staffed ownership, disclosure procedures |
| Privacy | Approved retention/access rules and evidence of redaction |

Before operational beta, security and service owners review these gates and document accepted residual risks. Do not waive absent ownership, unsafe credential handling, known critical exposure, or an unestablished durable-state invariant merely to meet a date.

A targeted exception must identify an owner, scope, compensating controls, expiry, and verification plan, and must be allowed by the adopted release policy. A draft document or checklist tick is not evidence a control works.

## Open Security Decisions

| Decision | Accountable owner | Required by |
|---|---|---|
| Primary/fallback reporting routes and response coverage | Security/project owners | Accepting external security reports and external beta |
| Supported versions, patch/backport policy, advisory process | Maintainers/security owner | First supported release |
| Identity provider/MFA and account recovery mechanism | Security owner | Privileged production access / external beta |
| Automation credentials and revocation model | Security/service owners | Any production automation |
| Password costs, session timeouts, rate/resource budgets | Security/engineering owners | External beta, after measurement |
| Chat, audit, receipt, backup, and report retention | Security/community/service/legal owners | Operational beta |
| Secrets, TLS trust, rotation, and artifact verification | Service/security owners | Production deployment |
| Incident roster, recovery objectives, emergency approvals | Service/project owners | Operational beta |
| Research authorization or safe-harbor program, if any | Authorized owner/legal reviewer | Before advertising such a program |

Keep this file synchronized with actual source and deployed defaults as implementation develops. Do not convert a proposed requirement into a statement of protection until there is verification evidence.

The proposed `MIT OR Apache-2.0` software licensing remains subject to owner approval. Licensing terms do not establish a warranty, security support commitment, bug bounty, or authorization to test independent deployments.

## M7 Studio controls

Studio applies exact loopback Host and same-Origin mutation gates, exact media
types, bounded route bodies, CSP/no-store/nosniff/referrer headers, and fixed
workspace symlink/path confinement. Bearer publication tokens remain
server-side and are never persisted or sent to the browser. Preview is direct,
no-shell, environment-cleared, and bounded, but is not an OS sandbox or network
egress firewall. Production confirmation is a destructive-action safeguard,
not authorization; M6 grants and PostgreSQL audit remain authoritative.

## P0 filesystem and acceptance-gate update — November 9, 2026

The Linux launcher now prepares a private owned default workspace parent with
umask077, rejects symlink/dangling/non-directory/dot-dot path components and
fails explicitly for an existing non-private/unowned managed path. It does not
silently chmod existing data or run as root. These shell checks require trusted
HOME/XDG ancestry; they are not an OS sandbox or a concurrent-race guarantee.
Shared preview-slot multi-user hardening remains a separate open task.

Diagnostics report override/config presence while redacting values. The fresh
native check task does not clear arbitrary flags, erase outputs or rewrite user
settings. OIDC disabled/absent validation is now conditional, while enabled
canonical inputs, HTTPS origin/ingress and external secret references remain
required. The chart test harness fails for missing tools/native errors and
rejects plaintext rendered credentials/public operator flags. Its CI database
password is a disposable synthetic test value, not a deployment credential.

Schema/checker/mock tests are not live authentication, database or cluster
security evidence. New jobs must be required by maintainers' branch protection;
workflow configuration does not itself adopt that policy. See docs/P0_ACCEPTANCE.md
and docs/verification/p0-implementation-ledger.md for unclosed execution gates.

## R2 test isolation and provider adoption

R2 fixtures may run only against a loopback disposable `worldwright_test` database.
The fault proxy deliberately uses an unencrypted local test hop to inspect
PostgreSQL COMMIT frames; this is not a deployment TLS exception. Only uniquely
created current-run/current-role schemas are eligible for fallback cleanup, and
a leaked schema fails acceptance even when cleanup succeeds. Existing schemas,
identity audit, business uniqueness and receipts are not erased to obtain a pass.

The controlled HTTPS provider uses ephemeral keys and a test-only CA; Rust TLS
verification stays enabled with that explicit root. Fixture-browser self-signed
certificate acceptance is confined to the controlled harness. The cfg(test) CA
hook cannot affect release binaries. The approved-provider observer keeps normal
TLS validation and never captures credentials, tokens, authorization URLs or
browser storage. Provider login/MFA occurs in the operator's browser, not chat.
Do not upload private raw child logs until they have undergone the organization's
sensitivity review; preserve evidence in an approved private location.

Approved-provider adoption is not inferred from a nonempty reviewer string.
Final R2 closure verifies an Ed25519 signature covering the complete provider
report, source/lock/artifact/log digests, case matrix and adoption policy,
deployment and expiry. The public key must be independently authorized; a key
bundled by the report submitter is not a trust decision. This tooling neither
creates owner keys nor signs/grants approvals. Credential retention, actual
provider ownership and clock health remain owner-reviewed requirements.

The gameplay xorshift PRNG is not a cryptographic random generator or serialized
restart stream. Its seed uses system entropy, entropy failure is explicit, and
rejected/replayed candidates rewind the checkpoint. Durable receipts/counters
preserve committed outcomes. No speculative success/cookie may be returned from
an uncertain COMMIT; reconstruct and inspect before retrying.

### Local R3 audit-regression boundary

The role audit fails on absent schema/read/audit/column/invite/sequence rights,
not just shared-role membership or missing broad DML. Grant setup remains explicit
and transactional; real PostgreSQL fixtures test rollback and forbidden inherited
password/offboarding writes. Those fixtures require an owned disposable cluster
and explicit `WORLDWRIGHT_R3_ROLE_FIXTURES=1`, and remove their unique DBs/roles.
They use no actual runtime passwords or approved migration ledger and cannot
produce an accepted staging role/Secret proof. Do not run them against a shared
production cluster merely because a URL resolves through loopback.

Collector exit evidence now rejects booleans, floats, strings and signal-killed
processes consistently with closure. Plan version and reviewer metadata are
strictly validated; synthetic Ed25519 unit keys still adopt no service. Current
migration failure, native/image/staging and actual owner/CNI/termination/rotation
requirements are not waived by this local coverage.

### R3 collector safeguards

R3 phase output is capped at 16 MiB and captured without a shell or interactive
stdin. Timeout/overflow/cleanup interruption cannot satisfy an expected rollout
failure. Logs use exclusive 0600 creation and mask configured credential values,
JSON passwords and URL passwords; private Kubernetes API bodies remain omitted.
Exact-value masking is not comprehensive data classification: do not provision
arbitrary secrets into logs/evidence or treat masking as confidentiality approval.
Staging API redirects are refused; TLS verification is not disabled. Process
cancellation of test tools is not proof of graceful Worldwright termination.
Real Windows ACL/tree cleanup, CNI, revocation and service adoption remain open.
