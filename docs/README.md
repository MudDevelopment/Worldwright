# Developer Documentation

**Current status:** M4 and M5 are implemented awaiting live PostgreSQL
verification. M6 Operational Control is implemented awaiting live PostgreSQL and
recovery execution; it is not production ready.

- [Scaffold and integration status](SCAFFOLD.md): package boundaries, entry points, and next steps.
- [Core interfaces](CORE_INTERFACES.md): implemented value types, rule planning, scheduling, and fixture contracts.
- [World Runtime v0.1](RUNTIME.md): local Look/Move/Say execution, attachments, rollback, message routing, and focused tests.
- [Transport Protocol v1](PROTOCOL_V1.md): bounded JSON DTOs, envelopes, sequencing, errors, and compatibility rules.
- [Server Transport v0.1](SERVER.md): local account setup, server command, route/security boundary, and limitations.
- [Release v1](RELEASE_V1.md): `.wwr` contract, publication state, activation, recovery, rollback, and preview boundaries.
- [Release activation runbook](operations/RELEASE_ACTIVATION.md): executable staging procedure and recovery steps.
- [Development workflow](DEVELOPMENT.md): local tasks, pinned toolchain, and CI configuration.
- [Verification record](VERIFICATION.md): current M4/M5 counts, commands, and explicit PostgreSQL limits.
- [Transport verification report](verification/transport-report.md): focused server/protocol/network evidence and dependency versions.
- [Milestone 5 verification report](verification/milestone-5-report.md): current Windows counts and PostgreSQL evidence limits.
- [Operations v1](OPERATIONS_V1.md): authoritative M6 capability/auth/API contract.
- [Backup and restore runbook](operations/BACKUP_RESTORE.md): executable recovery procedure.
- [Operator control runbook](operations/OPERATOR_CONTROL.md): bootstrap, Console, grants, moderation, drain, and metrics.
- [Milestone 6 verification report](verification/milestone-6-report.md): focused current M6 counts and limits.
- [Project README](../README.md): setup and project overview.
- [Design](../DESIGN.md): target behavior and invariants.
- [Architecture](../ARCHITECTURE.md): target dependency and runtime boundaries.

The policy documents at the repository root remain proposed. Transport v0.1 is a locally verified development slice, not a production support or deployment promise. No reporting address, license approval, or maintainer appointment is created by this project.

- [Studio v1](STUDIO_V1.md): local draft workspace, artifact, preview, and publication-request workflow.
- [Milestone 7 verification](verification/milestone-7-report.md): evidence categories and unrun live gates.
- [Toolkit v1](TOOLKIT_V1.md): M8 contracts and selected policy.
- [Milestone 8 summary](../Milestone-8-summary.md): delivered M8 scope and limits.
- [Milestone 8 verification](verification/milestone-8-report.md): M8 evidence boundary.
- [Milestone 8 performance](verification/milestone-8-performance.md): release-profile development workload.
- [Toolkit v1](TOOLKIT_V1.md): M8 contracts and selected policy.
- [Milestone 8 summary](../Milestone-8-summary.md): delivered M8 scope and limits.
- [Milestone 8 verification](verification/milestone-8-report.md): M8 evidence boundary.
- [Milestone 8 performance](verification/milestone-8-performance.md): release-profile development workload.

M7 Studio is implemented awaiting live PostgreSQL/operator publication and
manual browser/accessibility evidence.
