# Independent Release Acceptance — Implementation Checklist

Document ID: IRA-IMP-001  
Version: 1.0 — draft for adoption

## 1. Purpose

Adopt the document set and verify that the implemented process enforces accountable acceptance decisions rather than substituting technical test success. This is a system/process acceptance checklist, not approval of any particular release.

## 2. Phase 1 — Confirm the contract and governance

**Accountable:** Contract authority and policy owner

- [ ] Identify the governing contract and version.
- [ ] Map all six actual prerequisites to contract clauses.
- [ ] Define objective closure criteria and required evidence for each.
- [ ] Confirm required closure signatories and any contract-specific independence rules.
- [ ] Review the proposed all-five-affirmative-approval requirement and adopt or formally revise it consistently across the document set.
- [ ] Approve the authority matrix and conflict rules.
- [ ] Designate the release authority, release manager, and records custodian.
- [ ] Approve the signature mechanism with legal and records management.
- [ ] Establish retention, legal-hold, access, and disposal requirements.
- [ ] Resolve ambiguities before operational use.
- [ ] Approve the policy and supporting procedures; set the effective date.

**Exit evidence:** Approved documents, contract register, authority matrix, signature approval, retention schedule.

## 3. Phase 2 — Configure the process and records

**Accountable:** Release manager and system owner

- [ ] Establish unique release and baseline identifiers.
- [ ] Create controlled acceptance-package storage.
- [ ] Configure all six prerequisite identifiers separately from additional controls.
- [ ] Configure the five functional reviewer positions.
- [ ] Resolve account aliases to natural-person identity for distinctness checking.
- [ ] Configure prospective delegation and alternate reviewer records.
- [ ] Require explicit signed closure and acceptance actions.
- [ ] Bind decisions to baseline and evidence versions.
- [ ] Implement immutable or tamper-evident decision history.
- [ ] Implement findings, suspension, withdrawal, and supersession handling.
- [ ] Implement execution-time gate validation.
- [ ] Restrict and monitor privileged gate and role changes.
- [ ] Configure backups, retrieval, and audit exports.

**Exit evidence:** Configuration records and a traceable mapping to CTL-01 through CTL-15 in [CONTROLS_AND_ASSURANCE.md](CONTROLS_AND_ASSURANCE.md).

## 4. Phase 3 — Execute control acceptance tests

**Accountable:** Designated process assurance lead  
**Environment:** Controlled nonproduction environment using clearly labeled synthetic records

Record the actual result, evidence reference, tester, date, and corrective action for every test. A planned test is not a passed test. These tests validate the acceptance mechanism; passing them does not replace the decisions required for a real release.

| ID | Scenario | Expected result |
|---|---|---|
| AT-01 | All six valid closures, five distinct authorized approvals, correct baseline, and signed authorization | Deployment eligibility is granted only for the authorized scope |
| AT-02 | Remove one closure; repeat for each of the six prerequisites | Authorization and deployment blocked each time |
| AT-03 | Duplicate one closure under another prerequisite without satisfying that prerequisite | Gate blocked; duplicate does not count twice |
| AT-04 | Remove one functional approval; repeat for all five functions | Authorization and deployment blocked each time |
| AT-05 | Assign two functions to the same person using the same account | Distinctness validation fails |
| AT-06 | Assign two functions to the same person using different account aliases | Distinctness validation fails |
| AT-07 | Use a shared or service account as reviewer | Decision rejected as a required human approval |
| AT-08 | Pass all technical tests but supply no signed closures or approvals | Release remains blocked |
| AT-09 | Record approval conditional on unfinished mandatory work | Recorded as Changes required; gate blocked |
| AT-10 | Record rejection, abstention, or pending review | Gate blocked |
| AT-11 | Use a reviewer outside authority scope or effective dates | Decision rejected or marked invalid |
| AT-12 | Add delegation only after the decision was made | Original decision remains invalid; new authorized decision required |
| AT-13 | Declare an unresolved conflict or prohibited self-review | Affected approval blocked until an authorized independent review occurs |
| AT-14 | Change a signed record's substantive content | Integrity check fails or controlled supersession is required; original preserved |
| AT-15 | Present approvals for a different artifact or configuration baseline | Gate blocked |
| AT-16 | Substitute deployment artifact or configuration after authorization | Execution-time check blocks deployment |
| AT-17 | Withdraw or expire a required approval after authorization but before execution | Deployment blocked |
| AT-18 | Introduce a material change or uncertain impact | Authorization suspended pending reassessment |
| AT-19 | Allow a review deadline to expire without a response | Status remains nonapproved; escalation occurs without auto-approval |
| AT-20 | Declare an emergency without contractual authority to bypass normal gates | Release remains blocked |
| AT-21 | Attempt a privileged technical bypass | Attempt restricted where technically possible, logged, alerted, and investigated; no valid acceptance is created |
| AT-22 | Make required signature or authorization validation unavailable | Deployment remains blocked unless an approved equivalent validation process establishes validity |
| AT-23 | Attempt deployment outside authorized scope or window | Deployment blocked |
| AT-24 | Revoke authorization during rollout | Further rollout stopped and incident assessment initiated |
| AT-25 | Restore the package from backup and reconstruct the decision timeline | All required records, versions, signatures, and ordering are recoverable |
| AT-26 | Supersede a closure or functional decision | Original retained; current record is clearly identified and linked |
| AT-27 | Simulate unauthorized deployment and incident response | Historical violation preserved; containment, notifications assessment, and corrective actions recorded |
| AT-28 | Attempt release with complete approvals but no release-authority signature | Deployment blocked; eligibility is not treated as authorization |

### Test result record

| Field | Entry |
|---|---|
| Test identifier | To be completed |
| Environment / configuration version | To be completed |
| Preconditions and synthetic record references | To be completed |
| Execution steps | To be completed |
| Expected result | To be completed |
| Actual result | To be completed |
| Evidence reference | To be completed |
| Outcome | Not run / Pass / Fail |
| Tester / timestamp | To be completed |
| Corrective action and retest reference | To be completed, or None |

**Exit evidence:** Completed results for applicable tests; all gate-enforcement failures corrected and retested before go-live.

## 5. Phase 4 — Train and pilot

**Accountable:** Policy owner and release manager

- [ ] Train prerequisite owners on criterion-to-evidence mapping.
- [ ] Train signatories on explicit closure and authority limits.
- [ ] Train reviewers on independent decisions, conflicts, and residual risk.
- [ ] Train release authorities on completeness versus decision-making boundaries.
- [ ] Train deployment operators on baseline matching and execution-time checks.
- [ ] Train administrators on privileged-access restrictions and bypass reporting.
- [ ] Train records custodians on retention, holds, retrieval, and supersession.
- [ ] Run a supervised end-to-end trial.
- [ ] Include a rejected review, a material change, and a reviewer substitution in the trial.
- [ ] Demonstrate emergency escalation without assuming a waiver.
- [ ] Correct process gaps and update the document set consistently.

**Exit evidence:** Training records, pilot package, findings, corrections, and successful retests.

## 6. Phase 5 — Approve operational use

**Accountable:** Designated governance approving authority

- [ ] Contract mapping is complete.
- [ ] Documents are approved and effective.
- [ ] Roles and alternates are appointed.
- [ ] Signature and retention arrangements are approved.
- [ ] Control tests and pilot are complete.
- [ ] No unresolved mandatory implementation finding remains.
- [ ] Monitoring, audit, and incident arrangements are operating.
- [ ] Formal go-live approval is recorded.

Process go-live approval does not authorize an individual release. Each release must still complete its own six closures, five functional approvals, and release authorization.

## 7. Ongoing operation

- [ ] Perform completeness and chronology checks for every release.
- [ ] Review metrics and control findings quarterly.
- [ ] Test retrieval and restoration at the approved cadence.
- [ ] Review policy annually and after material changes.
- [ ] Re-run affected control tests after identity, signing, gate, deployment, or repository changes.
- [ ] Keep policy, procedures, templates, and technical configuration aligned.

## 8. Implementation sign-off

| Field | Entry |
|---|---|
| Implemented system / process version | To be completed |
| Approved document-set version | To be completed |
| Contract register reference | To be completed |
| Test and pilot evidence | To be completed |
| Remaining nonblocking observations | To be completed, or None |
| Approving authority / authority reference | To be completed |
| Signature reference / timestamp | To be completed |
| Effective operational date | To be completed |
