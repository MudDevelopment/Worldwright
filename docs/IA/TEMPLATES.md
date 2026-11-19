# Independent Release Acceptance — Record Templates

Document ID: IRA-FRM-001  
Version: 1.0 — draft for adoption  
Authority: [POLICY.md](POLICY.md)

## Instructions

Copy the appropriate template into the controlled release record. Complete required fields with real values. “To be completed” means the record is not yet valid; it is not an approval. Do not use a typed name in this Markdown file as a substitute for the approved signing mechanism.

Preserve original records and link replacements through explicit supersession references. Use timestamps with a time zone. Bind signed records to controlled evidence versions and the exact baseline.

## Form 01 — Release intake and baseline

| Field | Entry |
|---|---|
| Release identifier / title | To be completed |
| Release manager | To be completed |
| Contract identifier and version | To be completed |
| Contract authority | To be completed |
| Prerequisite register version | To be completed |
| Baseline identifier | To be completed |
| Source revision, where applicable | To be completed |
| Build identifier | To be completed |
| Artifact names and digests | To be completed |
| Configuration identifier / digest | To be completed |
| Dependencies | To be completed |
| Intended environment and scope | To be completed |
| Proposed deployment window | To be completed, or explicitly unrestricted if authorized |
| Acceptance-package repository | To be completed |
| Package manifest version / digest | To be completed |
| Deployment and rollback plan references | To be completed |
| Review targets and escalation contacts | To be completed |
| Signature mechanism | To be completed |
| Retention schedule reference | To be completed |
| Current status | Draft |

## Form 02 — Contract prerequisite register

Populate from the actual governing contract. These labels do not define prerequisite content.

| ID | Contract version / clause | Exact prerequisite wording | Objective closure criteria | Required evidence | Owner | Authorized signatory |
|---|---|---|---|---|---|---|
| PR-01 | To be completed | To be completed | To be completed | To be completed | To be completed | To be completed |
| PR-02 | To be completed | To be completed | To be completed | To be completed | To be completed | To be completed |
| PR-03 | To be completed | To be completed | To be completed | To be completed | To be completed | To be completed |
| PR-04 | To be completed | To be completed | To be completed | To be completed | To be completed | To be completed |
| PR-05 | To be completed | To be completed | To be completed | To be completed | To be completed | To be completed |
| PR-06 | To be completed | To be completed | To be completed | To be completed | To be completed | To be completed |

| Register control | Entry |
|---|---|
| Register identifier / version | To be completed |
| Contract-authority confirmation | To be completed |
| Legal review reference | To be completed |
| Resolved interpretations | To be completed, or None |
| Effective date | To be completed |
| Prior version | To be completed, or None |

## Form 03 — Reviewer appointment and authority roster

| Function | Person / unique identity | Authority reference | Scope | Effective dates | Alternate | Conflict disposition |
|---|---|---|---|---|---|---|
| Project | To be completed | To be completed | To be completed | To be completed | To be completed | To be completed |
| Engineering | To be completed | To be completed | To be completed | To be completed | To be completed | To be completed |
| Security | To be completed | To be completed | To be completed | To be completed | To be completed | To be completed |
| Service | To be completed | To be completed | To be completed | To be completed | To be completed | To be completed |
| Legal | To be completed | To be completed | To be completed | To be completed | To be completed | To be completed |

Validation:

- [ ] Five different natural persons confirmed, including identity aliases.
- [ ] Scope and effective dates verified.
- [ ] Conflicts declared and resolved.
- [ ] Applicable self-review restrictions satisfied.
- [ ] Delegations recorded before decisions.

Validated by / timestamp: To be completed.

## Form 04 — Signed prerequisite closure

Prepare one separately identifiable record per prerequisite.

| Field | Entry |
|---|---|
| Closure record identifier / version | To be completed |
| Release / baseline | To be completed |
| Prerequisite identifier | PR-01, PR-02, PR-03, PR-04, PR-05, or PR-06 |
| Contract version / clause | To be completed |
| Requirement wording | To be completed |
| Closure criteria | To be completed |
| Evidence references and versions | To be completed |
| Deviations / limitations | To be completed, or None |
| Authorized dispositions | To be completed, or None |
| Signatory name and identity | To be completed |
| Signatory role / authority reference | To be completed |
| Validity restrictions / expiration | To be completed, or None where permitted |
| Superseded closure reference | To be completed, or None |

### Criterion assessment

| Criterion | Evidence reference / version | Assessment and result | Outstanding issue |
|---|---|---|---|
| To be completed | To be completed | Explain satisfaction; do not merely state “tests passed” | To be completed, or None |

### Closure statement

> I attest that the identified prerequisite has been satisfied for the stated release baseline against the cited contractual criteria. This decision is supported by the referenced evidence and is subject only to the explicitly recorded, authorized limitations.

Signature record reference: To be completed.  
Signed timestamp and time zone: To be completed.

Do not sign this closure statement while a mandatory criterion is unsatisfied.

## Form 05 — Functional acceptance decision

Prepare separate records for project, engineering, security, service, and legal.

| Field | Entry |
|---|---|
| Decision record identifier / version | To be completed |
| Release / baseline | To be completed |
| Function | Project / Engineering / Security / Service / Legal |
| Reviewer name and unique identity | To be completed |
| Authority or delegation reference | To be completed |
| Acceptance-package version / digest | To be completed |
| Evidence reviewed | To be completed |
| Independence declaration | To be completed |
| Conflict and disposition references | To be completed, or None |
| Findings and dispositions | To be completed, or None |
| Decision | Pending / Approve / Changes required / Reject / Abstain-conflict |
| Decision rationale | To be completed |
| Residual-risk disposition | To be completed, or None |
| Validity restrictions / expiration | To be completed, or None where permitted |
| Superseded decision | To be completed, or None |

### Approval statement — use only for Approve

> Acting within my documented authority for the identified function, I approve the stated release baseline based on the referenced acceptance package. I confirm that applicable independence requirements are satisfied and that no unresolved mandatory condition remains within my review remit.

For another decision, explicitly state the required changes, rejection reasons, or conflict that prevents acceptance. Do not include the approval statement as if approval had been granted.

Signature record reference: To be completed.  
Signed timestamp and time zone: To be completed.

## Form 06 — Finding and resolution record

| Field | Entry |
|---|---|
| Finding identifier | To be completed |
| Release / baseline | To be completed |
| Originating reviewer or control | To be completed |
| Category | Evidence / Technical / Contract / Operational / Authority-independence / Risk |
| Description and affected obligation | To be completed |
| Blocking status | Blocking / Nonblocking with rationale |
| Owner / due date | To be completed |
| Corrective action | To be completed |
| Evidence of completion | To be completed |
| Baseline impact / change reference | To be completed, or None |
| Required decision authority | To be completed |
| Signed resolution or superseding decision | To be completed |
| Closure date | To be completed only after resolution |

## Form 07 — Residual-risk disposition

This form does not waive contractual prerequisites.

| Field | Entry |
|---|---|
| Risk identifier / release baseline | To be completed |
| Risk description and consequence | To be completed |
| Likelihood / impact assessment | To be completed using organizational method |
| Affected requirements and functions | To be completed |
| Contractual basis permitting disposition | To be completed |
| Compensating controls | To be completed |
| Residual exposure | To be completed |
| Authorized risk decision-maker | To be completed |
| Authority reference | To be completed |
| Decision and rationale | To be completed |
| Scope / expiration / review trigger | To be completed |
| Signature record / timestamp | To be completed |

- [ ] No mandatory prerequisite is left unsatisfied by this disposition.
- [ ] Appropriate functional reviewers have assessed the disposition.

## Form 08 — Release authorization

| Field | Entry |
|---|---|
| Authorization identifier / version | To be completed |
| Release / baseline | To be completed |
| Acceptance manifest version / digest | To be completed |
| PR-01 closure reference | To be completed |
| PR-02 closure reference | To be completed |
| PR-03 closure reference | To be completed |
| PR-04 closure reference | To be completed |
| PR-05 closure reference | To be completed |
| PR-06 closure reference | To be completed |
| Project approval reference | To be completed |
| Engineering approval reference | To be completed |
| Security approval reference | To be completed |
| Service approval reference | To be completed |
| Legal approval reference | To be completed |
| Authorized artifacts and digests | To be completed |
| Authorized configuration | To be completed |
| Environment / scope / window | To be completed |
| Operational restrictions | To be completed, or None |
| Rollback plan | To be completed |
| Additional control results | To be completed |
| Authorizing individual / authority | To be completed |
| Signature record / timestamp | To be completed |
| Revocation / supersession status | To be completed |

### Authorization statement

> I have verified that all six required signed prerequisite closures and all five distinct functional approvals are valid and applicable to the identified baseline. I authorize deployment only of the specified artifacts and configuration, within the recorded scope and restrictions. This authorization does not waive any contractual requirement.

## Form 09 — Deployment and verification record

| Field | Entry |
|---|---|
| Deployment identifier | To be completed |
| Release / baseline / authorization | To be completed |
| Execution-time gate result and timestamp | To be completed |
| Closure and decision versions checked | To be completed |
| Actual artifact digests | To be completed |
| Actual configuration | To be completed |
| Environment and deployed scope | To be completed |
| Operator identity | To be completed |
| Start / completion timestamps | To be completed |
| Execution outcome | To be completed |
| Post-deployment checks and results | To be completed |
| Incident / rollback references | To be completed, or None |
| Deviations and response | To be completed, or None |

## Form 10 — Change-impact and reacceptance record

| Field | Entry |
|---|---|
| Change identifier | To be completed |
| Original release / baseline | To be completed |
| New baseline, if applicable | To be completed |
| Change description / reason | To be completed |
| Affected evidence and prerequisites | To be completed |
| Affected functional decisions | To be completed |
| Authorization suspension reference | To be completed, or rationale for no suspension |
| Impact assessments and authorities | To be completed |
| Replacement closures / decisions | To be completed |
| Explicit permitted carry-forward confirmations | To be completed, or None |
| Updated package manifest | To be completed |
| Gate revalidation result | To be completed |
| New or restored authorization reference | To be completed |

## Form 11 — Emergency or contractual exception assessment

An assessment or request is not permission to release.

| Field | Entry |
|---|---|
| Record identifier / release | To be completed |
| Emergency and impact | To be completed |
| Requested action and urgency | To be completed |
| Normal requirements affected | To be completed |
| Contractual emergency provision or amendment | To be completed; no inferred authority |
| Contract authority and legal review | To be completed |
| Required decisions and actual records | To be completed |
| Authorized scope and expiration | To be completed |
| Compensating controls | To be completed |
| Recovery and follow-up plan | To be completed |
| Formal authorization reference | To be completed before action |
| Outcome and follow-up evidence | To be completed |

## Form 12 — Acceptance-package manifest

| Record type | Record identifier / version | Controlled location | Digest or integrity reference | Access classification |
|---|---|---|---|---|
| Contract and prerequisite register | To be completed | To be completed | To be completed | To be completed |
| Baseline inventory | To be completed | To be completed | To be completed | To be completed |
| Six closure records | List each record separately | To be completed | To be completed | To be completed |
| Five functional decisions | List each record separately | To be completed | To be completed | To be completed |
| Supporting evidence | List each evidence item or controlled index | To be completed | To be completed | To be completed |
| Authority and conflict records | To be completed | To be completed | To be completed | To be completed |
| Findings and risk dispositions | To be completed, or None | To be completed | To be completed | To be completed |
| Authorization and deployment records | To be completed | To be completed | To be completed | To be completed |
| Change and incident history | To be completed, or None | To be completed | To be completed | To be completed |

Manifest owner, version, and timestamp: To be completed.

## Form 13 — Retention and retrieval control

| Field | Entry |
|---|---|
| Record class | To be completed |
| Governing retention basis | Contract / Legal / Regulatory / Organizational references |
| Retention trigger and duration | To be established before operation |
| Repository and custodian | To be completed |
| Access restrictions | To be completed |
| Backup and restore arrangements | To be completed |
| Hold status and authority | To be completed |
| Disposal authority and method | To be completed |
| Latest retrieval / restoration test | To be completed |
| Result and corrective actions | To be completed |
