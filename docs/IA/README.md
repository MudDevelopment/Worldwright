# Independent Release Acceptance — Document Set

Status: Draft for organizational adoption  
Prepared: 2026-11-18

## Purpose

Implement the supplied release-contract requirement: **six signed prerequisite closures and distinct project, engineering, security, service, and legal reviewers. Passing tests cannot replace those decisions.**

## Documents

| Document | Purpose |
|---|---|
| [POLICY.md](POLICY.md) | Mandatory governance, authority, independence, signatures, and release gates |
| [PROCEDURES.md](PROCEDURES.md) | End-to-end operating procedures, including changes, emergencies, and unauthorized releases |
| [CONTROLS_AND_ASSURANCE.md](CONTROLS_AND_ASSURANCE.md) | Workflow states, technical enforcement, records, metrics, and audit |
| [TEMPLATES.md](TEMPLATES.md) | Registers and forms for closures, reviews, authorization, changes, and findings |
| [IMPLEMENTATION_CHECKLIST.md](IMPLEMENTATION_CHECKLIST.md) | Adoption plan and control acceptance tests |

## Contract facts versus proposed controls

The supplied contract requirement establishes six signed prerequisite closures and five distinct functional reviewers. The exact six prerequisites and the complete contract text have not been supplied.

This framework proposes additional organizational rules: affirmative approval by all five reviewers, no prohibited self-acceptance, fail-closed release enforcement, and controlled change reassessment. These rules must be approved during adoption; they are not represented as quoted contract terms.

PR-01 through PR-06 are administrative identifiers. They are not definitions of the prerequisites. Populate them from the governing contract before using the process for a release.

## Adoption prerequisites

1. Confirm the governing contract and its six prerequisite requirements.
2. Review this framework with the contract authority and the five functions.
3. Approve the authority matrix, independence requirements, signature method, and records-retention schedule.
4. Appoint the policy owner, release authority, release manager, records custodian, signatories, and reviewers.
5. Implement and test release-gate enforcement.
6. Approve the documents and set an effective date.

## Document control

| Field | Value |
|---|---|
| Policy owner | To be designated before adoption |
| Approving authority | To be designated before adoption |
| Operational custodian | Release manager |
| Effective date | To be set upon approval |
| Review frequency | Annually and after material contract, process, or control changes |
| Governing contract | To be identified before operation |
| Document-set version | 1.0 — draft |

## Interpretation

- “Shall” identifies a mandatory requirement once this policy is adopted.
- “May” identifies a permitted option, subject to the contract and applicable controls.
- Governing law and contractual obligations take precedence over this framework. Conflicts require formal resolution; personnel shall not silently relax either requirement.
- Technical verification, prerequisite closure, functional acceptance, release authorization, and deployment verification are separate obligations.

## Core release rule

Release is permitted only when all six signed prerequisite closures and all five distinct functional approvals are valid for the exact baseline, all mandatory conditions are resolved, applicable additional controls are satisfied, and the release authority has issued a valid authorization.

A successful test run is evidence—not an acceptance decision.
