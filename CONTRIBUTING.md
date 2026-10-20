# Contributing to Worldwright

**Build worlds. Write the rules. Contribute with evidence.**

| Field | Value |
|---|---|
| Document version | 0.1 |
| Updated | October 20, 2026 |
| Status | Proposed contribution and governance policy; pending owner adoption |
| Applies to | Worldwright code, tools, reference content, documentation, and project infrastructure |
| Related documents | [README](README.md), [Design](DESIGN.md), [Architecture](ARCHITECTURE.md), [Conduct](CODE_OF_CONDUCT.md), [Security](SECURITY.md), [Definitions](DEFINITIONS.md) |

> Worldwright remains a planning-stage toolkit. No repository URL, default branch, maintainer roster, CI service, or working release is established by this document. Procedures below become operational when authorized owners adopt them and publish the corresponding project details.
>
> **License decision remains open.** The proposed outbound software expression is `MIT OR Apache-2.0`. Do not treat submitting a pull request to an unlicensed project as a substitute for explicit contribution terms or copyright-owner approval.

## Contents

- [Ways to Contribute](#ways-to-contribute)
- [Where to Start](#where-to-start)
- [Governance and Decision Rights](#governance-and-decision-rights)
- [Licensing and Contribution Rights](#licensing-and-contribution-rights)
- [Development Workflow](#development-workflow)
- [Engineering and Content Standards](#engineering-and-content-standards)
- [Verification](#verification)
- [Pull Request Checklist](#pull-request-checklist)
- [Review and Merge](#review-and-merge)
- [Releases and Emergency Changes](#releases-and-emergency-changes)
- [Policy Adoption Checklist](#policy-adoption-checklist)

## Ways to Contribute

Contributions are welcome in more forms than engine code:

| Area | Useful contributions |
|---|---|
| Engine and SDK | Small vertical slices, invariant tests, interface proposals, error handling |
| CLI and Studio | Validation diagnostics, safe file handling, accessible authoring workflows |
| Game systems | Reusable examples that separate setting-specific rules from engine mechanisms |
| Content | Original rooms, NPCs, dialogue, tutorial zones, and coherent reference scenarios |
| Testkit | Deterministic fixtures, simulated clients, crash/retry tests, performance workloads |
| Operations | Repeatable deployment, recovery exercises, dashboards, and runbooks |
| Security | Threat-model review, authorized testing, dependency analysis, private vulnerability reports |
| Documentation | Correct examples, terminology, onboarding, diagrams, and source-backed explanations |
| Accessibility | Keyboard workflows, screen-reader checks, non-color cues, and readable output |

A useful contribution solves a defined problem, includes provenance, and provides verification evidence proportionate to its risk. A large code drop is not automatically more valuable than a precise bug report.

## Where to Start

1. Read the [README](README.md) for purpose and current status.
2. Read [DEFINITIONS.md](DEFINITIONS.md) for MU* terminology and Worldwright's compatibility boundaries.
3. Review the affected contracts in [DESIGN.md](DESIGN.md) and component ownership in [ARCHITECTURE.md](ARCHITECTURE.md).
4. Review [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) and [SECURITY.md](SECURITY.md).
5. Once a repository exists, inspect its actual tree, manifests, supported toolchain, tests, and issue history before proposing code.
6. Discuss large changes before implementing them. Offer a small design note with the problem, alternatives, compatibility impact, and test plan.

### Route the issue correctly

| Issue | Intended route |
|---|---|
| Ordinary bug or documentation correction | Project issue tracker once published |
| Feature, architecture, or policy proposal | Public design discussion once available |
| Exploitable vulnerability, leaked secret, or sensitive player data | Private process in SECURITY.md |
| Harassment, retaliation, or conflict involving a maintainer | Private process in CODE_OF_CONDUCT.md |
| Account or gameplay dispute on an independent game | That game's operator; toolkit maintainers do not automatically administer it |

Until private channels exist, do not post sensitive details publicly. Use an independently verified private maintainer contact if available, or request a reporting channel without exposing the issue.

## Governance and Decision Rights

### Responsibility model

Roles express responsibilities, not a claim that named staff have been appointed. Small teams may combine roles, but conflicts and independent approvals must still be handled explicitly.

| Role | Accountable decisions | Limits |
|---|---|---|
| Authorized project owner | Ownership, license approval, contribution terms, appointment of accountable leads | Cannot grant rights the organization does not own |
| Product/project lead | Scope, priorities, milestone outcomes | Cannot waive undocumented security or recovery requirements |
| Engineering lead | Architecture, SDK boundaries, compatibility decisions | Must document rationale and obtain relevant specialist review |
| Tooling maintainer | CLI, Studio, developer workflow | Cannot bypass runtime invariants or production access controls |
| Content/design lead | Reference-game rules, schema meaning, content review | Cannot import unlicensed assets or activate production releases alone |
| Verification owner | Acceptance evidence, regression coverage | Reports gaps rather than manufacturing a green result |
| Security owner | Threat review, vulnerability coordination, security blockers | Sensitive information remains need-to-know |
| Service owner | Deployment, rollback, backup/restore readiness | Cannot infer database authority from an available process |
| Community lead | Conduct reporting, moderation process, community safety | Must recuse from conflicts and provide an independent appeal path |
| Designated release owner | Records release go/no-go and accepted residual risks | Must respect established non-waivable gates |

### Decisions and disagreements

- Seek informed agreement, not unanimity through endless discussion.
- Ordinary changes are decided by the responsible maintainer after review.
- Cross-cutting changes need the engineering lead and affected owners.
- Public SDK, wire format, persistence, security, and content-compatibility changes require a decision record before merge.
- Record the decision, reasons, dissenting technical concerns, and follow-up evidence; redact sensitive details.
- If a dispute remains unresolved, escalate to the designated project lead. Ownership/licensing decisions remain with the authorized owner.
- A person with a material conflict must not be the sole decision-maker for that dispute.
- Conduct complaints use the conduct process, not an architecture vote or popularity contest.

No contributor is entitled to merge access, production credentials, or a particular release date. Sponsorship, seniority, and contribution volume do not exempt anyone from review or conduct standards.

### Maintainer appointment and access

Before public collaboration, publish a maintainer roster with responsibility areas, backup reviewers, and contact routes. Appoint maintainers based on sustained review quality, reliability, and stewardship—not only commit count.

Document access grants, review them periodically, and remove unnecessary permissions when responsibilities change. Maintainer removal should use a documented, conflict-aware process. Emergency credential containment may precede that process when access presents an immediate risk.

## Licensing and Contribution Rights

### Proposed inbound-equals-outbound model

Subject to adoption, original software contributions should be offered under **both alternatives** of the proposed `MIT OR Apache-2.0` expression so recipients can choose either license. An inbound MIT-only contribution does not automatically supply Apache patent rights from its author. Do not relabel third-party work as dual-licensed without the necessary grants.

Contributors retain whatever copyright they hold unless a separate, explicit agreement states otherwise. This proposal does not require copyright assignment. Review actual employment, contractor, and organizational ownership before contributing.

The DCO recommendation below is **not** an alternate software license, copyright transfer, patent guarantee, or automatic permission to relicense everyone's work later.

### Recommended sign-off: DCO 1.1

Recommend adopting [Developer Certificate of Origin 1.1](https://developercertificate.org/) together with published inbound terms. The DCO certifies a contributor's right to submit under the indicated license and acknowledges that the contribution record, including sign-off information, may be retained and redistributed publicly.

Once adopted, each contribution commit should carry a truthful sign-off. For example:

```text
Signed-off-by: Contributor Name <contributor@example.com>
```

A contributor can add their own sign-off while committing:

```bash
git commit -s
```

- Read the actual DCO before signing.
- Use an identity and address permitted by the adopted identity/privacy policy; do not fabricate attribution.
- Do not sign for another author, silently replace their authorship, or imply employer approval you do not have.
- Preserve attribution and required sign-offs through a reviewed squash/rebase workflow.
- A DCO sign-off is different from a cryptographic commit signature.
- A private/no-reply address may reduce exposure where permitted, but sign-off information still becomes part of the contribution record.

Whether a contributor agreement is needed instead of, or in addition to, a DCO remains an owner/legal decision. Do not demand both merely because a template mentions them. External code acceptance is gated on settling this choice.

### Imported code, assets, and generated material

For material not wholly original to the contributor, identify:

- Source location and specific revision or release.
- Original author/copyright notices and applicable license.
- What was copied or modified.
- Whether redistribution and the project's intended licensing are permitted.
- Required attribution, NOTICE material, and other conditions.

Public visibility is not a license. A repository, forum post, old MUD distribution, or downloadable world database is not automatically reusable.

Sample-world prose, images, audio, templates, and documentation can have distinct terms. Keep those terms visible at the material's boundary. Do not put third-party assets under a blanket software-license header.

### AI-assisted contributions

AI assistance does not remove the contributor's responsibility for correctness, security, provenance, or licensing authority.

- Use only tools and data handling approved for the material involved.
- Do not submit confidential source, private player logs, credentials, or employer-restricted information to unapproved services.
- Disclose material generation or automated transformation where it affects review, provenance, or test confidence.
- Inspect the result, verify real APIs, and run applicable tests.
- Do not claim generated code is original, permissively licensed, or safe merely because a model produced it.
- Do not submit fabricated test output, benchmark measurements, citations, or acknowledgments.

A maintainer may request additional provenance evidence or decline a contribution whose rights cannot be established.

## Development Workflow

### Before editing

Inspect the actual repository first. Confirm the default branch, toolchain, edition, package names, applicable feature combinations, database requirements, and relevant contracts. Do not assume a published crate, CLI subcommand, or route exists from the planning documents alone.

### Branches and commits

- Branch from the current default branch after confirming its name.
- Use descriptive branch names such as `feature/content-reference-validation` or `fix/reconnect-generation-check`.
- Keep one logical change per pull request.
- Write imperative commit subjects, preferably no more than 72 characters.
- Explain why the change is needed and identify breaking effects in the body.
- Keep unrelated formatting or generated-file churn out of behavioral changes.
- Include directly relevant documentation changes in the same pull request as the behavior they describe.

### Proposed pull request description

```markdown
## What changed
Describe the concrete change and affected components.

## Why
State the problem and link the approved issue or decision when available.

## How verified
List exact commands, environments, manual scenarios, and observed results.
List checks not run and explain why.

## Compatibility and security
Describe API, protocol, content, config, data, permission, and deployment effects.
State the migration and rollback/recovery implications.

## Documentation
List updated documents or explain why no update is needed.

## Provenance
Identify imported material, license implications, and material AI assistance.
```

## Engineering and Content Standards

### Architecture and Rust

- Application binaries belong under `apps/`; package, directory, and executable names stay aligned.
- Shared libraries belong under `crates/`, introduced only when a real boundary is useful.
- Preserve the dependency direction and authoritative-state rules in ARCHITECTURE.md.
- Network handlers and tools must not mutate simulation state or durable inventories through back doors.
- Keep domain code independent of transport/database details where the current contracts require it.
- Use typed errors and explicit ownership; explain any `unsafe` code and test its safety assumptions.
- Do not hold shared locks across uncontrolled waits or introduce unbounded work queues.
- Game callbacks are trusted code, not a sandbox for untrusted submissions.
- Review dependencies for maintenance, license, attack surface, and feature cost before adding them.

### Persistence, interfaces, and compatibility

- Preserve receipt/idempotency, world-writer fencing, and unknown-commit semantics.
- Resolve test/implementation disagreements against the intended contract; never change expectations solely to make CI green.
- Classify changes to configuration or content as additive or breaking.
- Breaking changes include semantic/default changes, not only renamed fields.
- Ship a tested migration or an actionable rejection when safe migration is impossible.
- Keep version histories, examples, compatibility fixtures, and release notes synchronized.
- Do not promise legacy MU* compatibility without a source-grounded mapping and executable evidence.

### Content and terminology

Content contributions need stable identifiers, original or licensed text, reference validation, navigation checks, and playtest evidence. Declare intentional exceptions rather than hiding warnings.

Use [DEFINITIONS.md](DEFINITIONS.md) to distinguish an engine family from a gameplay style. A MUSH-like roleplay feature does not make Worldwright a PennMUSH-compatible server. Preserve these distinctions in marketing, imports, tests, and documentation.

Avoid real player information in fixtures. Use synthetic accounts, invented conversations, and original sample lore.

## Verification

No commands in this section are claimed to have passed against a Worldwright codebase. Once the workspace exists, follow its actual CI and toolchain configuration.

A proposed Rust baseline, from the workspace root, is:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked
```

If features are mutually exclusive, maintain a documented feature matrix rather than blindly running an invalid `--all-features` combination. SQLx compile-time queries may require approved offline metadata or an isolated development database; never point a test command at production.

Additional evidence depends on the change:

| Change | Minimum relevant evidence |
|---|---|
| Pure rule/parser | Unit tests, invalid input, boundary cases |
| Inventory/reward/ownership | Transaction tests, retries, crash windows, invariants |
| Authentication/authorization | Allowed and denied cases, revocation, origin/CSRF paths |
| Content schema | Positive/negative fixtures, migration, reference validation |
| UI/output | Keyboard and screen-reader-conscious behavior, safe text rendering |
| Protocol | Compatibility fixtures, malformed input, limits, disconnect/reconnect |
| Deployment | Isolated startup/drain/replacement and recovery rehearsal |
| Documentation | Links, code-fence labels, examples, scope/status consistency |

Record actual output and important environment details. Skipped tests are a review item, not an invitation to invent success. No benchmark or supported-capacity claim is valid without a reproducible workload and measurement.

## Pull Request Checklist

- [ ] The problem, owner, and acceptance criteria are clear.
- [ ] Actual source/contracts were checked before editing.
- [ ] The change is scoped and contains no unrelated churn.
- [ ] Ownership and license provenance are established.
- [ ] DCO/contributor requirements are satisfied if adopted.
- [ ] Tests cover the intended behavior and relevant failure paths.
- [ ] Test results and skipped checks are reported honestly.
- [ ] Security, privacy, accessibility, and resource-limit impacts were considered.
- [ ] Compatibility and migration effects are explicit.
- [ ] README/design/architecture/security/contribution guidance is current.
- [ ] No secrets, production data, or private reports are included.
- [ ] Sensitive findings were routed privately rather than exposed through the diff.

## Review and Merge

Recommended default after governance adoption:

1. At least one authorized maintainer other than the author reviews a change.
2. Cross-cutting or high-risk changes also receive the relevant security, service, content, or SDK review.
3. Reviewers distinguish blocking findings from suggestions.
4. Automated review can assist where approved and available, but does not replace accountable human review. Never send private security material to an unapproved reviewer service.
5. Required CI checks and compatibility evidence must pass, or a documented exception must be permitted by the adopted policy.
6. The maintainer confirms target branch, merge method, attribution, and sign-offs before merging.
7. A merged change is not automatically a published package or production deployment.

If the project has only one maintainer, arrange an independent reviewer for high-risk changes rather than claiming independence that does not exist. Publish any temporary limitation and keep production gates closed when required review cannot be obtained.

## Releases and Emergency Changes

Before a release, record code/content/config/schema compatibility, applicable license files and notices, test evidence, security advisories, and operator recovery instructions. The release owner records go/no-go after security and service owners assess their gates.

An emergency change may use a shorter review path only under an approved emergency policy. Record the incident, author, approver, scope, evidence, and follow-up review. Emergency status does not authorize publishing secrets, bypassing ownership rules, or concealing a data-integrity risk.

Conduct restrictions affect participation and project-controlled access. They do not by themselves revoke software rights already granted under an applicable license; license enforcement and contributor-access decisions are separate matters.

## Policy Adoption Checklist

Before opening routine external contributions:

- [ ] Confirm copyright owner, publishing authority, and final license scope.
- [ ] Install actual license texts and package metadata.
- [ ] Approve inbound terms and DCO/CLA choice.
- [ ] Publish repository location, default branch, maintainer roster, and review rules.
- [ ] Establish protected branches, required checks, and release permissions.
- [ ] Publish working conduct, alternate/appeal, and security reporting channels.
- [ ] Approve retention, privacy, and contributor-identity handling.
- [ ] Define emergency-change and maintainer-conflict procedures.
- [ ] Review all pending-policy labels and replace only those actually resolved.

Adoption requires an owner-recorded decision; committing this document alone does not prove those organizational controls exist.
