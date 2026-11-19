# Independent Release Acceptance Policy

Document ID: IRA-POL-001  
Version: 1.0 — draft for adoption  
Effective date: To be set upon approval  
Owner: Designated release-governance authority  
Approval: Accountable executive or formally delegated governance body

## 1. Purpose

This policy ensures that a release satisfies all six contractual prerequisite closure requirements, receives separately attributable decisions from five distinct review functions, and is accepted through evidence, accountable judgment, and valid authority. It prevents technical test results from substituting for required decisions and preserves an auditable history.

## 2. Basis and adoption assumptions

**Supplied contract requirement:** Six signed prerequisite closures and distinct project, engineering, security, service, and legal reviewers are required. Passing tests cannot replace those decisions.

The exact six prerequisites and complete contract have not been supplied. They shall be extracted from the governing contract before this process operates.

**Proposed organizational controls:** This policy additionally requires affirmative approval by all five reviewers, prohibits self-acceptance as defined below, and blocks release when evidence or decisions are missing or invalid. These are proposed controls for adoption, not assertions about unsupplied contract language.

## 3. Scope

This policy applies to initial and subsequent releases covered by the acceptance contract, changes affecting an accepted release, participating internal teams and suppliers, and emergency releases unless an expressly authorized contractual process establishes different requirements.

Nonproduction testing may proceed under separate controls where permitted. It shall not be represented as contractual acceptance or production authorization.

## 4. Governing principles

### 4.1 Separate obligations

| Obligation | Establishes | Does not establish |
|---|---|---|
| Technical verification | Observed behavior under specified test conditions | Contractual closure or acceptance |
| Prerequisite closure | Satisfaction of one identified contractual prerequisite | All five functional approvals |
| Functional acceptance | An authorized decision within one function's remit | A decision for another required function |
| Release authorization | Permission to deploy after all gates are valid | Authority to invent or waive missing decisions |
| Deployment verification | Correct deployment of the authorized baseline | Retroactive authorization |

A successful build, test suite, scan, demonstration, or deployment is evidence. It shall not substitute for a required signed decision.

### 4.2 Fail-closed operation

A release shall remain blocked whenever a required condition is missing, invalid, expired, disputed, superseded, or unresolved. Silence, meeting attendance, receipt of email, or expiration of a review deadline shall not constitute approval.

### 4.3 Baseline-specific decisions

Every closure, review decision, and authorization shall identify the exact release baseline. General product approval or approval of an earlier artifact shall not authorize the current release.

## 5. Definitions

| Term | Definition |
|---|---|
| Release baseline | Identified artifacts, configuration, deployment scope, and supporting evidence submitted for acceptance |
| Prerequisite | One of the six obligations identified by the governing contract |
| Signed closure | An authorized person's attributable attestation that a specific prerequisite is satisfied for the baseline |
| Functional reviewer | Named person authorized to act for project, engineering, security, service, or legal |
| Independent acceptance | Decision without prohibited self-review, identity duplication, or undisclosed conflict |
| Acceptance package | Controlled baseline records, closures, evidence, and review decisions |
| Material change | Change capable of affecting a prerequisite, conclusion, risk assessment, or deployment scope |
| Release authorization | Recorded permission to deploy the identified baseline after all gates are satisfied |

## 6. Contract prerequisite register

The contract authority, with legal review, shall establish a controlled register of exactly the six contractual prerequisites before acceptance begins. Use the register in [TEMPLATES.md](TEMPLATES.md).

For each prerequisite, the register shall identify the contract version and clause, exact requirement wording, objective closure criteria, required evidence, prerequisite owner, and authorized closure signatory.

PR-01 through PR-06 are administrative labels only.

Mandatory rules:

- Prerequisites shall not be combined to reduce the required number of closures.
- One closure shall not be counted twice.
- “Not applicable” shall not satisfy a prerequisite unless expressly permitted by the contract or a valid amendment.
- A supplier statement shall not constitute closure unless the supplier is an authorized signatory for that requirement.
- Ambiguities shall be resolved by the contract authority before closure.
- Additional organizational controls shall be tracked separately from the six contractual prerequisites.
- Acceptance shall be blocked if the six prerequisites cannot be identified and mapped to the contract.

## 7. Roles and responsibilities

| Role | Responsibility |
|---|---|
| Contract authority | Confirms obligations, resolves interpretations, controls amendments |
| Policy owner | Maintains policy, arranges periodic review, monitors control effectiveness |
| Release manager | Coordinates acceptance, maintains the package, records blockers, verifies completeness |
| Prerequisite owner | Performs required work and assembles evidence |
| Closure signatory | Reviews and signs the authorized prerequisite closure decision |
| Project reviewer | Assesses scope, commitments, dependencies, and delivery obligations |
| Engineering reviewer | Assesses technical adequacy, verification, defects, and configuration integrity |
| Security reviewer | Assesses exposure, controls, vulnerabilities, and residual security risk |
| Service reviewer | Assesses operational readiness, support, monitoring, recovery, and transition |
| Legal reviewer | Assesses relevant contractual and legal obligations |
| Release authority | Issues authorization only when all required gates are satisfied |
| Deployment operator | Deploys only the authorized baseline and scope |
| Records custodian | Preserves evidence, decisions, access controls, and audit history |

A closure signatory need not be one of the five functional reviewers unless the contract specifies that relationship. Any overlap shall be checked against applicable self-review and conflict rules. Reviewer distinctness does not by itself require six different closure signatories; signatory requirements shall be determined from the contract and authority matrix.

## 8. Reviewer distinctness and independence

### 8.1 Five different natural persons

The five required functional positions shall be occupied by five different natural persons.

- One person shall not approve under two functions.
- Multiple accounts for one person do not establish distinctness.
- Shared accounts, service accounts, bots, and group mailboxes shall not serve as reviewer identities.
- Committees may advise, but shall not obscure the accountable individual signatory.

### 8.2 Self-review and conflicts

A reviewer shall not be the sole independent acceptance authority for work the reviewer personally performed or substantively authored. Where a conflict exists, an authorized alternate shall perform the affected review.

The authority matrix shall define additional independence requirements, including reporting-line separation where required by contract or policy. Reporting-line separation is not assumed to be a supplied contract term.

Conflicts shall be declared, evaluated, and resolved before the decision. An unresolved conflict blocks the affected approval.

### 8.3 Appointment and delegation

Appointments shall identify the person, function, scope of authority, effective dates, alternate, and applicable restrictions. Delegation shall be recorded before the delegated decision. Retroactive delegation is prohibited.

Substitution requires a new decision from the substitute reviewer. Signatures shall not be transferred or copied between individuals.

### 8.4 Protection of judgment

Reviewers shall have relevant evidence, sufficient review time, and a means to raise concerns without their decisions being overwritten. Commercial urgency, schedule pressure, and management preference shall not convert rejection into approval.

## 9. Signed prerequisite closures

Each prerequisite shall have a separately identifiable record containing:

1. Release and baseline identifiers.
2. Prerequisite identifier and contract reference.
3. Closure criteria and evidence versions.
4. Assessment showing satisfaction of each criterion.
5. Deviations, limitations, and authorized dispositions.
6. Signatory identity and authority.
7. Explicit closure statement, signature, and timestamp.
8. Validity restrictions or expiration, where applicable.
9. Record version and supersession history.

A single signed document may contain multiple closure records only where permitted by the contract and each of the six decisions remains explicit and separately identifiable.

“Tests passed” alone is not sufficient closure rationale. The record shall explain how the evidence satisfies the prerequisite.

## 10. Functional acceptance decisions

| Decision | Meaning | Satisfies gate? |
|---|---|---|
| Approve | Accepts the identified baseline within the assigned remit | Yes |
| Changes required | Actions must be completed and reviewed | No |
| Reject | Release is unacceptable within the remit | No |
| Abstain / conflict | Reviewer cannot make an independent decision | No |
| Pending | Review is incomplete | No |

“Approve subject to completing required work” shall be recorded as Changes required.

Each decision shall identify the person, function, authority, baseline, evidence considered, rationale, independence declaration, signature, and timestamp.

Residual risk may accompany approval only when the appropriate authority has accepted the risk, the contract permits the disposition, no mandatory prerequisite remains unsatisfied, and compensating controls and limitations are recorded.

A reviewer may document that a consideration within the review remit is not applicable, with rationale. This does not eliminate that function's required decision.

## 11. Signature and record integrity

The approved signing method shall authenticate the signer, capture an explicit act of signing, bind the decision to the record and baseline, timestamp the act, make later modification detectable, preserve original and superseding decisions, and support audit retrieval.

Permitted mechanisms may include a controlled approval system, approved electronic-signature service, or controlled wet-signature process. Legal and records-management authorities shall approve the mechanism for the applicable contract and jurisdiction.

A typed name, pasted signature image, chat reaction, or ordinary email reply shall not automatically qualify as a valid signature.

## 12. Release authorization

All of the following are required:

- Six valid signed prerequisite closures.
- Five valid affirmative functional approvals.
- Five distinct reviewer identities.
- Valid authority and required independence.
- Applicability of all decisions to the current baseline.
- No unresolved mandatory condition.
- No relevant withdrawal, expiry, superseding rejection, or suspension.
- Satisfaction of additional applicable organizational release controls.

The release authority shall verify these conditions and sign the authorization. It shall not override a missing closure or substitute its judgment for a required reviewer.

Deployment shall be restricted to the authorized artifacts, configuration, environment, scope, and applicable time window.

## 13. Automation boundaries

Automation may validate completeness, identity distinctness, authority status, evidence versions, artifact digests, and expiration; block deployment; and produce audit manifests.

Automation shall not generate human acceptance from test results, populate signatures on behalf of reviewers, infer closure without the authorized decision, or treat timeout as approval.

Inability to establish validity shall leave the release gate closed. Administrative access shall not confer acceptance authority.

## 14. Changes, emergencies, and violations

Changes affecting the acceptance basis shall undergo the impact and reassessment process in [PROCEDURES.md](PROCEDURES.md). The release manager shall not unilaterally carry another person's decision to a materially changed baseline.

An emergency does not itself waive the contract. Internal risk acceptance shall not amend an external contract. Any different emergency path shall be expressly authorized by the governing contract or a valid contractual instrument.

Unauthorized deployment is a control failure. Records shall be preserved, further rollout stopped, the incident assessed, and corrective action performed. Retrospective signing shall not be represented as predeployment approval.

## 15. Records, oversight, and maintenance

Records shall be retained and access-controlled under [CONTROLS_AND_ASSURANCE.md](CONTROLS_AND_ASSURANCE.md). A retention schedule shall be established before operation; legal holds take precedence over deletion.

The policy owner shall review this policy annually and after material contract or control changes. Policy amendments require the designated approving authority. Policy amendments shall not be used to bypass contractual obligations.
