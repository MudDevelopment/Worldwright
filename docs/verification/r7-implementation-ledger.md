# R7 — supported private beta go/no-go

Authority: ROADMAP_v2.md R7. The defined release-decision/control feature set is
implemented. This is NOT an actual release approval: R1–R6 acceptance and owner
adoption remain OPEN, including unfinished/uncompiled R6 runtime feature work.
No public publication, production rollout, license grant or numerical completion
claim is inferred. Historical report hashes and existing runtime/migration bytes
were not rewritten in this R7 pass.

| Unit | Status | Verified scope / remaining boundary |
|---|---|---|
| R7-1 candidate/source/lock/patch tuple | verified | Bounded actual Git fingerprint, current migration inventory, clean/documented-patch binding; positive/negative synthetic source tests |
| R7-2 artifact/native/runtime/deployment/load evidence | verified | Source-grounded .wwr envelope; R0 tuple/log/binary, immutable deployment, beta/soak/SBOM checks; synthetic fixtures only |
| R7-3 R1–R6 prerequisite closure review | verified | Six signed gate certificates with 32 required case groups; missing/pending gates rejected |
| R7-4 owner/policy adoption | verified | Pinned external Ed25519 trust; five distinct reviewer subjects/keys; policy owners bound; no actual owners assigned |
| R7-5 exception limits/revocation/expiry | verified | Nonblocking-only exceptions; denial, revocation, expiry, role duplication and trust replacement fail closed |
| R7-6 private packet/notes/go-no-go | verified | Create-new output, copy-time hashes, full inventory, notes/decision/signature checks and synthetic packet roundtrip/tamper tests |
| R7-7 tests/docs/CLI handoff | verified | 35 locally executed tests, Python syntax, draft NO-GO, source readbacks; guide and both wrappers installed |
| Windows wrapper / hosted CI | not yet checked | Ubuntu/Windows tool-test workflow configured, not executed here |
| Actual current release decision | gap found | No current complete R1–R6 evidence/certificates or independent final owner decisions; project remains NO-GO |

## Implemented files

- scripts/r7_core.py — bounded contracts, source/evidence checks and signatures.
- scripts/r7.py — init, fingerprint, review-templates, signing-bytes, check,
  release and verify-packet. No signer/private-key input or publisher/deployer.
- scripts/r7-check.py / r7-check.ps1 — logged tool validation only.
- scripts/test-r7.py — 35 synthetic tests using temporary keys/reports/Git repos.
- scripts/r7-requirements.txt — pinned Python cryptography tooling dependency.
- .github/workflows/r7-checks.yml — read-only two-platform test/evidence job.
- docs/R7_ACCEPTANCE.md plus README/DESIGN/SECURITY/roadmap/verification updates.

## Validation observed

Final local runner status: tool-tests-passed-release-not-authorized; 35 tests,
zero skipped, plus actual draft init (exit 0) and draft check (expected exit 1 /
NO-GO for open-prerequisite-blockers). Python 3.12 sandbox, cryptography 46.0.6,
real temporary Git operations and signatures. No Rust build or real release
approval occurred. Positive GO log lines belong ONLY to synthetic test fixtures.

Retained selected outputs: r7-local-tool-checks/validation.json, three phase logs,
and no-go-decision.json (a copy of the local draft decision). Temporary fixture
keys/repos/packets were deleted by test cleanup. This is a selected tool-evidence
record, not a complete native build checkout or full candidate archive.

The R7 workflow YAML, read-only permissions, two-platform matrix and two Bash
blocks (workflow/guide) were checked locally. PowerShell was not available for
execution; hosted approval/runner adoption is not implied. Source/log hash
readback confirmation is recorded below when complete.

## Source-accuracy correction during implementation

Studio artifact provenance is `studio-r<revision>-<snapshot_sha256>`, not the
runtime Git commit (confirmed in apps/worldwright-studio/src/artifact.rs).
R7 now carries a separate content_source_revision, checks Studio revision bounds
and payload binding, and tests that distinction. Native runtime source/lock/patch
identity remains independently mandatory. No existing .wwr schema was changed.

## Trust and scope boundaries

R7 automatically checks selected raw report/content/identity invariants and
AUTHENTICATES independent closure attestations. It does not rerun the R1–R6 live
collectors or prove a dishonest manual statement true. Gate reviewers must run
the existing closure checks and review original native/live/manual evidence.
The Python artifact check does not replace Rust content/graph validation.

Five authorized final roles and six gate certificates, together with the current
same-release inputs, are required. A hash, unsigned name, supplied trust template,
helper-only R6 report or old target-directory build is not approval. Exceptions
cannot waive a required gate. Archive verification checks bytes/current signatures,
not the current checkout/cluster/database. A private-beta GO never authorizes
public registry publication or production deployment.

R6 runtime changes from the preceding session still need their source review,
native tests, documentation and live/OS/recovery validation. R7 leaves that work
visible instead of certifying it by inference.

## Final readback confirmation

Fresh EWW readbacks matched all five source hashes in the retained validation
record, all three phase-log hashes, the validation JSON and the copied draft
NO-GO decision. Thirty-five tests passed after the Studio provenance correction;
no skips were accepted. YAML/read-only CI matrix, documentation fences and two
Bash blocks passed local checks. This closes the scoped local tool-validation
work, not Windows/hosted execution or real R1–R6/owner acceptance. Current real
project release posture remains NO-GO, and unfinished R6 work remains explicit.

## Five remaining workstreams — active follow-through

| Workstream | Status | Current finding / boundary |
|---|---|---|
| 1 Windows/hosted execution | not yet checked | File tools cannot execute the Windows host or dispatch hosted jobs; native runner is being extended to cover server changes |
| 2 R6 source/tests/docs | gap found | PostgreSQL parsing found reserved `grant` aliases and unquoted audit INSERT; restore quarantine violated the existing writer-state constraint; fixes installed, validation underway |
| 3 Migration repair | gap found | Original operational-control migration still contains unquoted authorization; history/owner-approved baseline reconciliation required; no immutable file/checksum silently edited |
| 4 Same-release evidence and independent reviews | not yet checked | No actual live/owner inputs supplied; no signatures or approved identities will be fabricated |
| 5 Real candidate evaluation/package | gap found | Prerequisite gaps remain; only authenticated complete input can produce GO; no public or production release authorized |

New R6 native collection is schema v2: server binary build, server R6 unit tests
and server Clippy are mandatory. Browser reports stay v1. Prior native v1 reports
must be recollected, not upgraded by rewriting a version or inventing missing logs.
