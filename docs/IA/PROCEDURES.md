# Independent Release Acceptance — Operating Procedures

Document ID: IRA-PRO-001  
Version: 1.0 — draft for adoption  
Authority: [POLICY.md](POLICY.md)  
Forms: [TEMPLATES.md](TEMPLATES.md)

## 1. Operating rules

- Perform acceptance against an explicitly identified baseline.
- Keep technical verification, prerequisite closure, functional approval, authorization, and deployment verification separate.
- Record decisions when they occur; do not backdate them.
- Keep a release blocked while any mandatory condition is unresolved.
- Use the controlled register for the actual six contractual prerequisites.
- Establish local review targets and escalation contacts during initiation. Targets do not create automatic approvals.

## 2. Procedure A — Initiate acceptance

**Responsible:** Release manager  
**Participants:** Contract authority, prerequisite owners, closure signatories, five review functions  
**Trigger:** Proposed release subject to the acceptance contract

### Inputs

- Governing contract and version.
- Candidate release scope.
- Proposed artifacts and configuration.
- Organizational authority and delegation rules.

### Steps

1. Create a unique release record and package location.
2. Identify the governing contract and applicable version.
3. Obtain the controlled register of six prerequisites, with legal review of the contract mapping.
4. Resolve ambiguities in requirement wording or closure criteria before accepting closure evidence.
5. Define the baseline: source revision where applicable, build identifiers, artifact digests, configuration version, environments, scope, and relevant dependencies.
6. Assign prerequisite owners and authorized closure signatories.
7. Appoint project, engineering, security, service, and legal reviewers and alternates.
8. Verify that the five reviewers are different natural persons, have valid authority, and satisfy independence requirements.
9. Document conflicts and resolve them before review decisions.
10. Set review dates, response targets, escalation contacts, and any authorization window.
11. Confirm approved signature, access, and retention arrangements.
12. Open the release in Evidence preparation state.

### Exit condition and records

Contract mapping, baseline, ownership, review roster, package location, and operating arrangements are complete. Retain the release intake record, prerequisite register, and authority roster.

**Stop condition:** The contract mapping, authority, or independence cannot be established.

## 3. Procedure B — Assemble and close prerequisites

**Responsible:** Prerequisite owner for preparation; authorized closure signatory for the decision  
**Trigger:** Initiation complete

### Steps for each of PR-01 through PR-06

1. Review the contract wording and objective closure criteria.
2. Assemble evidence that applies to the submitted baseline.
3. Link each criterion to its supporting evidence and assessment result.
4. Identify missing evidence, discrepancies, limitations, and deviations.
5. Resolve discrepancies or obtain a permitted, properly authorized disposition. Do not relabel an unsatisfied mandatory prerequisite as accepted risk.
6. Prepare the closure record using the approved form.
7. Provide the signatory access to the evidence and relevant limitations.
8. Have the signatory independently evaluate the closure decision within assigned authority.
9. Obtain the explicit signed closure statement and timestamp.
10. Store the signed record and evidence versions in the package.

### Release-manager completeness check

Verify that:

- All six records are present and map to different required prerequisites.
- Each record identifies the correct baseline.
- Signatures and authority are valid.
- Evidence is accessible, versioned, and sufficient to show the recorded assessment.
- No unresolved condition is represented as closure.
- Any bundled signature arrangement is permitted by the contract and leaves six identifiable decisions.

### Exit condition and records

All six prerequisites have valid signed closures. Set state to Closure complete and retain all closure records and supporting evidence.

**Stop condition:** Any prerequisite lacks valid closure. Passing tests do not resolve this stop condition by themselves.

## 4. Procedure C — Conduct independent functional reviews

**Responsible:** Five functional reviewers  
**Coordinator:** Release manager

Review preparation may run in parallel with prerequisite work. Final approvals shall reference the completed acceptance package.

### Steps for each reviewer

1. Confirm identity, authority, and independence.
2. Confirm the baseline and acceptance-package version.
3. Inspect relevant closures and supporting evidence.
4. Assess minimum review coverage below and any contract-specific criteria.
5. Request additional evidence or clarification where necessary.
6. Record findings, required actions, limitations, and residual risks.
7. Confirm authorized disposition of any residual risk relevant to approval.
8. Issue Approve, Changes required, Reject, or Abstain / conflict using the functional decision form.
9. Sign and timestamp the record. Pending remains the state until a decision is complete.

### Minimum review coverage

| Function | Considerations |
|---|---|
| Project | Agreed scope, commitments, dependencies, stakeholder obligations, unresolved delivery risks |
| Engineering | Requirements coverage, test limitations, defects, configuration integrity, reproducibility where required |
| Security | Threat and exposure changes, vulnerabilities, access controls, security configuration, incident readiness |
| Service | Deployment plan, monitoring, support ownership, runbooks, capacity, backup, rollback readiness |
| Legal | Applicable contract terms, licenses, third-party obligations, privacy or regulatory requirements, required notices |

Record a rationale for any consideration judged not applicable. The function's decision remains required.

### Exit condition and records

Five different authorized reviewers have issued valid affirmative approvals for the same baseline. Set state to Accepted.

**Stop condition:** Any decision is pending, changes required, rejected, conflicted, expired, withdrawn, or otherwise invalid.

## 5. Procedure D — Resolve findings and disagreements

**Responsible:** Finding owner and relevant reviewer  
**Coordinator:** Release manager

### Steps

1. Create a finding with an identifier, description, affected obligation, owner, and due date.
2. Classify it as evidence deficiency, technical defect, contract interpretation, operational readiness, authority/independence, or risk requiring disposition.
3. Set the release to Changes required or Rejected as appropriate. Keep mandatory findings blocking.
4. Perform corrective action and gather evidence.
5. If the baseline or acceptance basis changes, apply Procedure F.
6. Submit the corrected evidence to the relevant decision authority.
7. Obtain the signed closure or superseding functional decision.
8. Preserve the prior record and link the new record as its successor.
9. Update the finding and release status only after the required decision is complete.

### Escalation

- Contract interpretation: contract authority with legal involvement.
- Reviewer unavailability: use a preauthorized alternate or arrange prospective delegation; obtain a new decision.
- Independence concern: policy owner and affected function's authority.
- Schedule or capacity issue: release manager and accountable management; no automatic approval.

Disagreements shall not be resolved by majority vote, removing an inconvenient reviewer, or reclassifying a mandatory requirement as optional risk.

### Exit condition

Required corrective actions are accepted by the relevant authorities and all mandatory blockers are closed.

## 6. Procedure E — Authorize and deploy

**Responsible:** Release authority for authorization; deployment operator for execution  
**Trigger:** Release is Accepted

### Authorization steps

1. Freeze the acceptance-package manifest and record its version or digest.
2. Validate all six closure records and all five functional approvals.
3. Verify the five-person distinctness rule, authority, and independence records.
4. Confirm that no mandatory findings remain and no decision has expired, been withdrawn, or been superseded by rejection.
5. Confirm that no material change occurred after the applicable reviews.
6. Confirm additional release controls, deployment readiness, and rollback readiness.
7. Sign the authorization record, specifying the artifact and configuration, environment, scope, time window if applicable, restrictions, and package reference.
8. Set state to Authorized.

### Deployment steps

1. Immediately before deployment, verify that authorization remains active and applicable.
2. Match artifact digests and configuration to the authorization.
3. Check environment, scope, window, and operator permissions.
4. Stop if any check fails or validity cannot be established.
5. Deploy through the controlled release process.
6. Record actual deployed digests, configuration, operator, environment, and timestamps.
7. Perform post-deployment checks and record results.
8. Set state to Deployed when deployment completion is recorded; record service health separately rather than assuming success from acceptance.
9. If deployment fails, initiate the approved incident or rollback process.

### Exit condition and records

The authorized baseline has been deployed with traceable execution and verification records, or failure and recovery have been recorded. Acceptance does not prove deployment success.

## 7. Procedure F — Manage changes after closure or approval

**Responsible:** Release manager and affected decision authorities  
**Trigger:** Change to an artifact, configuration, prerequisite evidence, deployment scope, or acceptance basis

### Steps

1. Record the proposed or discovered change and affected baseline.
2. Suspend authorization when the impact is material or uncertain.
3. Identify affected prerequisites, evidence, risks, reviews, and deployment restrictions.
4. Obtain documented impact assessments from the relevant decision authorities.
5. Establish a new baseline where artifacts or configuration change.
6. Reopen affected closures and functional reviews.
7. Obtain signed replacement decisions or explicit carry-forward confirmations where permitted.
8. Update the acceptance package and preserve superseded records.
9. Revalidate the entire release gate.
10. Obtain new authorization where the authorized baseline or scope changed, or formally restore authorization only when the existing authorization still applies and suspension has been resolved.

### Treatment matrix

| Change | Required treatment |
|---|---|
| Administrative correction, no meaning change | Preserve original and add traceable correction |
| Substantive evidence correction | Reopen affected closures and reviews |
| Artifact or configuration change | New baseline; reassess affected decisions |
| Deployment scope change | Reassess authorization and functional approvals |
| New defect, vulnerability, or legal constraint | Suspend authorization pending impact review |
| Reviewer substitution before required decision | Obtain a new decision from authorized replacement |
| Uncertain impact | Remain suspended until resolved |

Prior decisions may carry forward only if permitted by contract and the affected authorities explicitly confirm applicability. The release manager cannot do this unilaterally.

### Exit condition

The revised package satisfies all gates and valid authorization exists, or the release remains suspended or rejected.

## 8. Procedure G — Emergency releases and exceptions

**Responsible:** Contract authority, release authority, and required reviewers

### Steps

1. Document the emergency, impact, scope, and urgency.
2. Determine whether an authorized contractual emergency process exists.
3. Accelerate evidence collection and reviewer availability without manufacturing decisions.
4. Preserve required closures and independent decisions under the normal process unless an expressly authorized alternative applies.
5. If normal requirements cannot be met, obtain a valid contract amendment or other expressly authorized contractual instrument before release.
6. Record authority, scope, expiration, compensating controls, recovery plan, and follow-up obligations.
7. Implement only the authorized action and retain the full decision history.
8. Perform required follow-up review and corrective action.

### Prohibitions

- Emergency status alone does not waive the contract.
- Internal risk acceptance does not amend an external contract.
- Retrospective signatures shall not be represented as predeployment approvals.
- An internal exception request is not authority to bypass a mandatory gate.

### Exit condition

A compliant authorization path exists. Otherwise the release remains blocked. Separately authorized containment or rollback may be used where applicable.

## 9. Procedure H — Unauthorized release or control failure

**Responsible:** Incident manager and release authority

### Steps

1. Stop further rollout.
2. Preserve logs, evidence, authorization state, and decision history.
3. Notify release governance, security, legal, and service authorities as applicable.
4. Assess containment, rollback, or service restriction.
5. Determine contractual and other notification obligations with the appropriate authorities.
6. Record the event as an unauthorized release, without altering its historical status.
7. Investigate the initiating event and failed preventive or detective controls.
8. Assign corrective actions, owners, and dates.
9. Verify corrective-action effectiveness and restoration of gate enforcement.
10. Obtain valid authorization before further release activity.

### Exit condition

The incident is controlled, obligations addressed, corrective actions verified, and subsequent activity appropriately authorized. Do not rewrite history to make the original release appear compliant.

## 10. Procedure I — Close and archive the release record

**Responsible:** Release manager and records custodian

1. Reconcile the acceptance manifest against retained files and decision records.
2. Confirm deployment records identify what was actually deployed.
3. Link incidents, rollback events, and post-deployment findings.
4. Verify evidence access permissions and retention classification.
5. Apply legal holds where required.
6. Confirm that original and superseding records are retrievable.
7. Close administrative coordination tasks without deleting historical blockers or decisions.
8. Include the release in routine compliance monitoring.

**Exit condition:** An authorized auditor can reconstruct the contract mapping, baseline, six closures, five functional decisions, authorization, and deployment sequence.
