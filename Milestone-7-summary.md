# Milestone 7 summary

## Current status

Milestone 7 Builder Workflow and Studio v0.1 is implemented awaiting live
PostgreSQL/operator publication and manual browser accessibility evidence. It
is not production-ready. Studio is a loopback browser application and never
connects directly to PostgreSQL.

## Delivered scope

- Strict `DraftWorldV1`/`DraftRoomV1` editing with shared content validation.
- Append-only local workspace revisions, immutable artifacts, preview receipts,
  and local publication records.
- Save, import, export, validation, package, preview, and publication-status
  HTTP workflows.
- Exact loopback Host/Origin protection, bounded request bodies, CSP, no-store,
  nosniff, and referrer headers.
- Direct invocation of the existing M5 preview command with bounded output,
  input, stop, stale-state, and receipt behavior.
- Authenticated loopback M6 release submission using the bounded
  `worldwright-release-submission-v1` envelope and `StudioReviewContext`.
- Structured room/exit editor with stable IDs, dirty-state handling,
  validation diagnostics, import/export, responsive layout, and local
  deletion safeguards.

## Evidence

The pinned Windows close-out gate passed formatting, all-target checks, Clippy
with warnings denied, workspace tests, doctests, build, rustdoc, metadata,
JavaScript syntax validation, persistence ignored-test compilation, and diff
hygiene. It recorded **240 test-binary tests passed, 0 failed, 19 ignored**,
plus **2 doctests passed separately**. A Windows local manual smoke exercised
the Studio API: default revision, packaging, real sibling
`worldwright preview`, `look`, clean stop, and persisted receipt.
The browser preview server was launched, but visual or accessibility behavior
was not manually evaluated. Live operator publication and live PostgreSQL were
not exercised.

## Known limits and exit assessment

The current content model supports only schema-v1 rooms, exits, namespace,
entry room, and `allow_unreachable`; it does not provide arbitrary entities or
extensions. Preview is not an OS sandbox or network-egress firewall; the
official watchdog remains the orphan bound. Submission is not approval or
activation. Staging and production require current M6 grants, with typed
production confirmation and expected-current enforcement.

PostgreSQL acceptance tests remain ignored and compile-only. Linux, CI,
deployment, manual screen-reader/keyboard/browser matrices, and live operator
publication remain unverified.
