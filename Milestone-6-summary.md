# Milestone 6 Summary

## Status

**Implemented awaiting live PostgreSQL and recovery execution.** M6 is not
production ready.

Implemented slices include typed operational claims and capabilities, local MFA
credential/token issuance, loopback operator API, scoped transactional
authorization/audit, authenticated release request/review/activation,
operator bootstrap, drain lifecycle, backup manifest signing, backup/restore
CLI paths, and explicit authority takeover.

## Boundaries

Backup HMACs provide integrity only. Confidentiality/encryption at rest is
external. RPO/RTO targets are 24 hours/4 hours and unproven. Audit retention
is a 90-day policy without automated purge. Restore never takes authority;
takeover requires old-writer isolation. Loopback HTTP has no TLS.

## Evidence gate

Focused current evidence is Windows-local. PostgreSQL ignored tests compile but
do not run. No manual Console/browser, pg_dump/pg_restore, Linux, hosted CI,
deployment, or production-host test is claimed. M6 exit remains pending live
PostgreSQL and recovery execution.
