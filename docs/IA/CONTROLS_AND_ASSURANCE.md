# Independent Release Acceptance — Controls and Assurance

Document ID: IRA-CTL-001  
Version: 1.0 — draft for adoption  
Authority: [POLICY.md](POLICY.md)

## 1. Control objective

Enforce and demonstrate six valid signed contractual prerequisite closures, five distinct authorized functional approvals, and valid release authorization for the exact deployed baseline. Technical success shall never substitute for a required decision.

## 2. Workflow states

| State | Meaning | Normal exit condition |
|---|---|---|
| Draft | Baseline and responsibilities are being established | Initiation complete |
| Evidence preparation | Prerequisite evidence is being assembled | Six signed closures validated |
| Closure complete | Six valid signed closures exist | Functional review begins or is finalized |
| Independent review | Required functional decisions are being completed | Five valid affirmative approvals |
| Changes required | Blocking findings need action | Corrective evidence reviewed and required decisions renewed |
| Accepted | All five functional approvals are valid | Release authority signs authorization |
| Authorized | Deployment of the specified baseline is permitted | Deployment executed within authorization |
| Deployed | Authorized deployment is recorded | Ongoing monitoring and archive controls |
| Suspended | Change or new information prevents further authorized action | Impact resolved and gate revalidated |
| Rejected | Current release is not accepted | Corrective action and new applicable decisions |
| Superseded | A newer baseline or record replaces the current one | Terminal for that record; preserve history |

Preliminary review may occur during evidence preparation; final acceptance still requires the complete prerequisite package.

No direct transition from “tests passed” to Accepted or Authorized is permitted. Reopening work shall preserve prior decisions rather than overwriting them.

## 3. Logical release gate

The gate is a conjunction: every applicable condition must be true.

1. Exactly the six required prerequisite identifiers are represented by valid current closures.
2. All required closure signatures and signatory authority are valid.
3. All five functional decisions are affirmative and current.
4. Five distinct natural persons hold those decisions.
5. Authority and independence requirements are satisfied.
6. All records apply to the submitted baseline.
7. No mandatory finding or condition remains unresolved.
8. No relevant withdrawal, expiration, rejection, or suspension is active.
9. Additional applicable organizational controls are satisfied.
10. The release authority has signed an applicable authorization.
11. Deployment matches the authorized artifact, configuration, environment, scope, and window.

The first nine conditions establish eligibility for authorization. All eleven are required for deployment. This avoids treating a machine eligibility check as the authorization decision itself.

## 4. Minimum implementation controls

| ID | Control | Required behavior | Evidence |
|---|---|---|---|
| CTL-01 | Unique identity | Use individual authenticated accounts; resolve multiple accounts to one person | Identity and role records |
| CTL-02 | Distinct reviewers | Reject one person occupying two required functions | Gate evaluation and negative test |
| CTL-03 | Authority | Verify role, scope, dates, and prospective delegation | Authority matrix and decision metadata |
| CTL-04 | Independence | Require declaration and resolution of conflicts | Conflict records |
| CTL-05 | Prerequisite completeness | Require all six separately identified closures | Register-to-closure mapping |
| CTL-06 | Explicit decisions | Require intentional signed approvals; no inferred approval | Signed records |
| CTL-07 | Baseline binding | Bind evidence and decisions to versioned baseline identifiers | Manifest and artifact digests |
| CTL-08 | Tamper evidence | Detect changes to signed content; retain originals | Integrity validation and history |
| CTL-09 | Fail-closed enforcement | Block deployment on missing, invalid, or unverifiable gates | Pipeline or deployment logs |
| CTL-10 | Change invalidation | Suspend affected authorization pending reassessment | Change records and state transitions |
| CTL-11 | Deployment recheck | Verify validity and exact baseline immediately before execution | Execution-time gate record |
| CTL-12 | Privileged access | Restrict and independently monitor gate configuration and override attempts | Access review and alerts |
| CTL-13 | Preservation | Back up and restore complete acceptance records | Restore-test results |
| CTL-14 | Retrieval | Export a complete human-readable release history | Audit package |
| CTL-15 | Separation from testing | Test success cannot populate closure or acceptance decisions | Negative acceptance tests |

### Permission boundaries

- Prerequisite owners prepare evidence but do not gain signing authority merely through ownership.
- Signatories and reviewers can issue decisions only within documented authority.
- Release managers coordinate and check completeness; they cannot fabricate or replace others' decisions.
- Release authorities authorize only after the gate is satisfied.
- Deployment operators cannot change acceptance records to make a deployment eligible.
- Platform administrators cannot obtain acceptance authority through technical privilege alone.

Where administrators can technically bypass controls, apply restricted privileged access, independent monitoring, and investigation. A recorded administrative override does not itself create contractual approval.

### Unavailable validation services

If identity, signature, evidence, or authorization validity cannot be established, keep deployment blocked. A separately approved manual process may be used only if it establishes the same required validity and is permitted by the contract and policy. Unavailability is not an approval.

### Revocation and execution timing

Recheck decisions and authorization at execution time rather than relying only on an earlier pipeline result. Serialize or otherwise control authorization changes and deployment initiation so a concurrent revocation cannot be silently missed. Record the decision versions used by the deployment gate. If revocation occurs during rollout, stop further rollout and assess containment under the incident process.

## 5. Minimum record relationships

Each release record shall link:

- Governing contract and version.
- Six-prerequisite register version.
- Release baseline and package manifest.
- Six current signed closure records and any superseded versions.
- Five current signed functional decision records and prior decisions.
- Authority, delegation, independence, and conflict records.
- Findings and authorized risk dispositions.
- Release authorization and any suspension or revocation.
- Deployment records and post-deployment verification.
- Changes, exceptions, incidents, and corrective actions.

A reference shall identify a stable controlled record and version, not merely a mutable link to the latest file.

## 6. Records retention and access

The records custodian shall preserve all records necessary to reconstruct acceptance and deployment.

Before operation, establish a retention schedule covering contractual, legal, regulatory, and organizational requirements. Record the retention trigger, duration, repository, owner, disposal authority, and applicable hold rules. Do not invent a universal retention duration.

Legal holds take precedence over routine disposal. Supersession does not authorize deletion of the prior decision.

Evidence may be access-restricted. Required reviewers shall receive authorized access or an authorized evidence-review arrangement adequate to make the decision. Inability to inspect required evidence is a blocker, not a reason to assume compliance.

Control access according to information sensitivity and least privilege. Preserve signature and audit evidence without unnecessarily distributing sensitive source material. Test backup restoration and audit retrieval at the adopted cadence and after material repository changes.

## 7. Per-release assurance

For every release, verify:

- Six valid closures mapped to the actual contract requirements.
- Five approvals from five distinct persons.
- Valid authority and independence.
- Evidence and baseline consistency.
- No unresolved mandatory conditions.
- Correct chronological order: closures and functional approvals before authorization; authorization before deployment.
- No relevant withdrawal, expiry, or invalidation at deployment time.
- Deployment of the authorized artifacts and configuration only.

Record the result and the evaluator, whether the check is automated, manual, or combined. An automated check confirms recorded conditions; it does not make the underlying human decisions.

## 8. Quarterly control review

The policy owner shall arrange quarterly reviews covering:

1. Unauthorized deployments and suspected bypasses.
2. Duplicate-identity or shared-account approvals.
3. Retroactive signatures or delegations.
4. Privileged changes to gates, roles, or signing configurations.
5. Independence and conflict handling.
6. Repeated reopening of closures and underlying evidence-quality issues.
7. Retention, backup restoration, and retrieval failures.
8. Effectiveness of corrective actions from earlier reviews.

Select a risk-based sample for detailed reconstruction in addition to automated completeness checks across releases. Expand the sample when failures indicate a systemic issue. Document the sample rationale, population, findings, owners, and follow-up dates.

## 9. Metrics

| Metric | Definition / purpose |
|---|---|
| Complete predeployment acceptance | Authorized deployments with all required valid prior records divided by deployments assessed |
| Unauthorized releases | Count of deployments lacking valid required authorization |
| Reviewer-distinctness violations | Count of attempts and completed events involving duplicate natural-person identities |
| Decisions invalidated by changes | Count and reasons; measures change-control effectiveness |
| Review duration by function | Time from complete submission to recorded decision, with rework distinguished |
| Reopened prerequisite closures | Count and causes; identifies evidence-quality problems |
| Retrieval failures | Acceptance packages not recoverable or reconstructable when required |
| Privileged bypass events | Attempts, successful technical bypasses, and investigation status |

Report periods and denominators explicitly. Distinguish attempted violations from completed unauthorized releases. Do not use approval speed or approval rate as the sole reviewer-performance measure.

## 10. Findings and corrective action

For each assurance finding:

1. Record the violated control and supporting evidence.
2. Assess whether active release authorizations are affected.
3. Suspend affected activity where validity is uncertain.
4. Assign an owner and due date.
5. Identify root cause and corrective action.
6. Verify effectiveness with evidence, including negative testing where appropriate.
7. Close through the designated assurance authority.

A corrected current record does not erase a historical violation.

## 11. Maintenance

Review this control specification annually and when the contract, signing mechanism, identity system, deployment process, or records repository materially changes. Re-run affected implementation acceptance tests after relevant changes. Keep this file, policy, procedures, and templates consistent in the same document revision.
