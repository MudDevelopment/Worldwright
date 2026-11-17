# R7 — supported private beta decision and release packet

Authority: ROADMAP_v2.md R7. The release decision/control tooling is implemented;
**the current project remains NO-GO**. R1–R6 native/live/manual acceptance and
owner adoption have not been supplied. The prior R6 runtime feature pass remains
uncompiled and incompletely verified; R7 does not finish it by changing a label.

## Implemented features and boundaries

| Feature | Implementation |
|---|---|
| Candidate initialization | `r7.py init`: new external directory, unfilled release tuple, pending policies, no identities/keys/signatures invented |
| Current identity | `fingerprint`: bounded Git source/lock/patch inventory matching R0's hash recipe, plus exact current migration SHA-384 list |
| Evidence and identity checks | `r7_core.py`: bounded contained files, exact .wwr envelope/payload hash/game/provenance, R0 host/source/phase/log/binary linkage, fixed beta, separate soak, deployment and SBOM tuple |
| Prerequisite closure | Six signed gate certificates covering all 32 named case groups; no missing/pending/skipped case can be waived |
| Owner adoption | Five independent Ed25519 decisions: project, engineering, security, service, legal; exact candidate/gate-set/trust-store binding |
| Exceptions | At most ten, nonblocking operational limitations only, named owner/evidence, seven-day maximum and no expired exceptions |
| Decision and release | `check` retains GO/NO-GO; `release` additionally copies a private packet and generates release notes only after validation |
| Archive verification | `verify-packet` checks every file, signatures, revocation/expiry, candidate/notes/decision binding and Unix executable bit |
| Tests/CI | 35 synthetic policy/crypto/filesystem/CLI tests and a real draft NO-GO check; Ubuntu/Windows tool-test workflow configured |

This is a **reviewed-evidence decision layer**, not another implementation of the
R1–R6 runners. It does not execute DB/load/restore/deployment tests or manufacture
proof that those tests ran. Reviewers must run the existing track validators,
inspect retained original evidence and sign the appropriate closure certificate.
The engine performs the listed automatic checks, then authenticates those signed
closure attestations. A signature establishes the authorized signer and bound
statement; it cannot establish that a dishonest reviewer actually performed a
manual test. Likewise the Python .wwr check verifies container identity, not the
Rust content graph validator. R5 closure must attest actual content validation.

There is **no signing-key input, automatic signing, force/waive flag, public
publisher or production deployer**. A private-beta GO never sets public-registry
publication or production-rollout authorization to true. `publish=false` and
unapproved licensing metadata are not changed by this tooling.

## Prerequisites

- Python 3.11+, Git on PATH, approved acquisition of `scripts/r7-requirements.txt`
  (cryptography 46.0.6). These are Python tooling dependencies, not Cargo changes.
- Run from the actual source checkout for `fingerprint`, `check` and `release`.
  Their source root is the parent of the installed `scripts/` directory.
- New output directories outside the source checkout and candidate input tree;
  parents must already exist. Never overwrite old evidence.
- Trusted operator-controlled directories, a stable checkout and candidate files
  during evaluation/copy. No guarantee against a hostile same-user process or
  privileged administrator racing filesystem changes.
- On Unix, new directories are 0700, data files 0600 and packaged binary files
  0700, subject to umask. On Windows review inherited ACLs on the chosen parent;
  Python file mode bits do not certify DACL isolation. Do not weaken execution
  policy to run the PowerShell wrapper.

## Run the tests now — no release credentials required

Windows, after approving dependency installation, paste only code, not prompt text:

```powershell
. {
    $ErrorActionPreference = 'Stop'
    Set-Location 'E:\Hold\Worldwright'
    $venv = Join-Path $env:LOCALAPPDATA 'Worldwright-R7-Python'
    python -m venv $venv
    if ($LASTEXITCODE -ne 0) { throw 'venv failed' }
    $python = Join-Path $venv 'Scripts\python.exe'
    & $python -m pip install -r scripts/r7-requirements.txt
    if ($LASTEXITCODE -ne 0) { throw 'dependency installation failed' }
    & 'E:\Hold\Worldwright\scripts\r7-check.ps1' -Python $python
}
```

The wrapper creates a fresh LOCALAPPDATA evidence directory, runs the 35 tests,
checks Python syntax, creates a real unapproved draft and verifies that `check`
returns exit **1** with NO-GO. The wrapper itself returns success only for that
expected rejection plus passing tests. `validation.json` explicitly says this is
tool validation, not supported-beta authorization. No Cargo or browser required.

Ubuntu 24.04, use an external approved venv and a NEW external output directory:

```bash
python3 -m venv "$HOME/worldwright-r7-python"
"$HOME/worldwright-r7-python/bin/python" -m pip install -r scripts/r7-requirements.txt
PYTHONDONTWRITEBYTECODE=1 "$HOME/worldwright-r7-python/bin/python" scripts/r7-check.py --output "$HOME/worldwright-r7-checks-new"
```

The test suite uses real Ed25519 signatures and temporary Git repositories but
**synthetic reports, policies, binaries and reviewers**. Its positive GO cases
are not real project approvals. Keys and fixture repositories are temporary.
Configured hosted CI and the Windows wrapper still require actual execution.

## Create a draft and fingerprint

Use new external paths (the following parent is assumed to exist):

```powershell
& $python scripts/r7.py init --output C:\PrivateEvidence\beta-candidate
& $python scripts/r7.py fingerprint --output C:\PrivateEvidence\beta-source
& $python scripts/r7.py check --candidate C:\PrivateEvidence\beta-candidate\candidate.json --output C:\PrivateEvidence\beta-initial-check
```

The final command is expected to return **1 / NO-GO** while blockers remain.
Do not remove blockers merely to obtain a different label. Complete the actual
work and obtain current evidence. Fingerprinting does not build or test anything.

All JSON formats introduced here use `schema_version: 1`. Existing R0–R6 formats,
migrations, .wwr content format and old report hashes remain unchanged. Future
changes to required R7 fields must version this protocol rather than silently
reinterpret an old signature.

## Candidate contract

`candidate.json` has these exact top-level fields: `schema_version`, `kind`,
`release`, `files`, `primary`, `migration_files`, `policies`, `gate_certificates`,
`exceptions`, `open_blockers`, `notes`. The kind is `r7-private-beta-candidate`.
The initializer provides the editable skeleton; no dummy hash/owner is accepted
as a substitute for real data. `open_blockers` must be empty for GO.

### Same-release tuple

| Field | Required value |
|---|---|
| release_id | Actual lowercase artifact release ID |
| commit | Actual 40/64-character lowercase Git commit |
| source_sha256, lock_sha256 | From current fingerprint / matching R0 reports |
| patch_sha256 | SHA-256 of actual `git diff --binary HEAD`; empty-patch hash when clean |
| worktree_state | `clean` or `documented_patch`, matching actual Git status |
| artifact_sha256, payload_sha256 | Actual supplied .wwr and its payload hashes |
| content_source_revision | Exact .wwr provenance revision; distinct from the runtime code commit |
| game_id, contract_version | `worldwright-reference` or `worldwright-expedition`, integer version 1 |
| image_reference | Registry image reference containing an immutable `@sha256:` digest |
| namespace_uid, deployment_uid | Actual staging identities from the matching collection |
| migrations | Ordered `{name, sha384}` list from current fingerprint |
| soak_seconds | Adopted separate soak duration, 1800–3000 seconds |

Content provenance and runtime source are deliberately separate. Studio emits
`studio-r<revision>-<snapshot_sha256>`, not a Git commit; R7 checks that revision
is within 1–10000 and that the suffix matches the payload digest. CLI-packaged
content retains its explicitly reviewed provenance string. The native runtime's
full Git commit/source fingerprint remains independently bound. A documented
patch is bound by the source/patch inventories, same-source native evidence and
all reviewer signatures; it does not permit omitted untracked files or a stale
build. No source/lock mismatch is waived. Do not mutate source during evaluation.

### Files and primary records

Each `files` entry is exactly `{id, path, sha256, kind}`. IDs are lowercase slugs;
paths use `/`, start with `data/`, are contained and have no links, traversal,
backslashes, alternate-data-stream colons or case-colliding duplicates. Kinds:
`artifact`, `binary`, `report`, `log`, `policy`, `document`, `migration`.
All hashes in this table are **byte-exact SHA-256**; preserve original log bytes.
Source fingerprint normalization is separate and follows the R0 recipe.

Limits: 3000 selected files, 512 MiB per file, 4 GiB total; JSON reads 16 MiB.
Select sanitized acceptance evidence and distribution artifacts, not target/
folders, credentials, test passwords, signing keys or unrelated private files.

`primary` maps these seven names to file IDs: `artifact`, `native-ubuntu`,
`native-windows`, `beta`, `soak`, `deployment`, `sbom`.

- Native inputs are real R0 reports. Include every referenced phase log beside
  its report, plus the actual worldwright, Studio and server binary bytes for
  both hosts. Every required phase must be present/passed with integer exit 0.
- Native source inventories and Cargo.lock must match the current fingerprint.
  Ubuntu must be 24.04; the native pin is 1.85.1.
- Beta dimensions/thresholds/zero-failure inventory are unchanged beta-250-v1.
  A smoke or helper report is rejected, not promoted.
- Soak uses the actual r4-supplemental-scenarios report, all seven scenario groups,
  positive observations and a duration at least `soak_seconds`.
- Deployment is the same staging collection image/content/namespace/deployment
  tuple. Its raw collected status is NOT alone a closure: signed R3 is mandatory.
- SBOM is CycloneDX 1.6 with unique components and release metadata properties
  matching `scripts/release-evidence.py`: commit, source_patch_sha256,
  cargo_lock_sha256, artifact_sha256, container_digest. Declarations are not a
  license grant or proof of advisory review; R6/legal review remains mandatory.

`migration_files` maps every migration filename to its corresponding selected
file ID. Those SQL bytes must match the tuple's SHA-384 list. The known earlier
migration syntax failure cannot be repaired by an R7 manifest or exception.

### Adopted policies

`policies` maps each name below to a `policy` file ID. Every policy object has
exact fields `{schema_version, policy, status, scope, owner, reference, details}`:
version 1, matching policy name, status `adopted`, scope `private-beta`, real owner
and adoption reference, and nonempty `details`. Pending templates do not pass.

| Policy | Owner must match | Additional machine-checked details |
|---|---|---|
| license-distribution | legal reviewer | Real adoption reference and substantive details; reviewer verifies terms/notices |
| support | service reviewer | `service_owner` equals reviewer; nonempty `coverage` |
| security-reporting | security reviewer | `private_channel`, `response_coverage` |
| privacy-retention | security reviewer | `preserves_business_uniqueness: true` plus adopted rules |
| recovery | service reviewer | integer rpo_seconds=86400, rto_seconds=14400, audit_days=90 |
| runtime-matrix | engineering reviewer | Matrix described below |

Current recovery numbers are implementation targets requiring actual measured
R6 adoption, not newly issued SLAs. Other targets require a reviewed protocol/
source change, not editing an expected result. Text fields do not authenticate
a policy by themselves: all five final signatures bind every selected policy.

Runtime `details.matrix` must contain exactly chromium-x11, chromium-wayland,
firefox-x11, firefox-wayland and windows-native records. Each has `case`, actual
`version`, `runtime_dependencies` description and an `evidence` file ID. No latest,
unknown or wildcard version. R1 signers verify the actual observations and limits.

## Reviewer trust and signed closures

The trusted public-key store must be supplied independently, **outside the
candidate directory**, with its approved byte-exact SHA-256 pin. Do not trust
a key because it arrived inside an evidence packet. The initializer's unfilled
trust template is NOT an approved trust store; copy/configure it only through
your independent authority process. Private signing keys never enter this tool.

Store shape: `{schema_version: 1, keys: [...]}`. Each key has exact fields
`key_id`, `role`, `subject`, `public_key_hex` (32-byte Ed25519 public key in lowercase
hex), `not_before`, `not_after` (Unix seconds), `revoked` (Boolean). Subjects must
be canonical independently verified identities. The role is one of project,
engineering, security, service, legal. Unknown/revoked/expired keys are refused.

Generate unsigned review requests after assembling the candidate:

```powershell
& $python scripts/r7.py review-templates --candidate C:\PrivateEvidence\beta-candidate\candidate.json --output C:\PrivateEvidence\beta-review-requests
```

Each R1–R6 request contains all required case names from `r7_core.GATES` (32 total).
Reviewers must validate the original track evidence with the existing R0/R1,
R2, R3 and R4 checkers and the R5/R6 required manual/native procedures before
setting a closure case to passed. Each case needs 1–16 selected evidence file IDs.
R1/R2/R5 closures use an engineering key; R3/R4 a service key; R6 a security key.
The signed payload binds the gate, status `closed`, case/evidence lists, complete
release-tuple hash and selected-file-list hash. Each closure lasts at most 30 days.

Put resulting envelopes in `gates/r1.json` through `gates/r6.json`, each as
`{"payload": <completed payload object>, "signature": <base64 detached Ed25519 signature>}`.
Then regenerate final review requests in another NEW output directory so the
final gate-set hash is populated. Final review binds the canonical candidate,
all six gate envelopes and the approved trust-store hash. All five roles must
approve; identities AND public keys must be distinct and different from the
artifact builder identity. This is an intentionally strict separation-of-duties
policy, not an assignment of owners. A denial, missing role or duplicate identity
is NO-GO. Final decisions last at most seven days and no longer than their key.

Canonical signature bytes for both payload kinds are the ASCII prefix
`WORLDWRIGHT-R7-SIGNATURE-V1` followed by one LF, then UTF-8 JSON with sorted keys,
compact separators, ASCII escaping, and no NaN/infinity. Do not sign pretty JSON
or append another newline to the signing input. The payload's purpose separates
`prerequisite-closure` from `private-beta-decision`.

```powershell
& $python scripts/r7.py signing-bytes --payload C:\PrivateEvidence\reviewed-project-payload.json --output C:\PrivateEvidence\project-signing-input
```

That command creates `signing-input.bin` and `payload.json`, not a signature.
Use the separately managed signer; combine its detached base64 signature with
the unchanged payload. `pending`, null times/keys and unsigned requests cannot
pass. Reviewer names or hashes alone are never authorization.

## Check, release and verify

Use the approved external trust-store path/pin and five completed decision files.
The following example assumes `$candidate`, `$trust`, `$approvedPin` and the
five paths in `$approvals` have been set from real reviewed inputs:

```powershell
$argsR7 = @('scripts/r7.py','check','--candidate',$candidate,'--output','C:\PrivateEvidence\beta-final-check','--trust-store',$trust,'--trust-store-sha256',$approvedPin)
foreach ($approval in $approvals) { $argsR7 += @('--approval',$approval) }
& $python @argsR7
if ($LASTEXITCODE -ne 0) { throw 'NO-GO: preserve decision.json; do not release' }
```

`check` always retains decision.json and an event journal once its fresh output
is created. Exit 0 means the decision was GO; exit 1 means NO-GO. To create a
packet, use `release` in place of `check` with another new output directory and
the same complete inputs. Evaluation is repeated; it does not trust the prior
GO label. No files are published or deployed. The packet contains selected
artifacts/evidence/policies, candidate, six gate certificates, five decisions,
release notes and an exhaustive hash manifest. Partial-copy failure marks the
packet INCOMPLETE and returns NO-GO. Do not distribute a partial packet.

```powershell
& $python scripts/r7.py verify-packet --packet C:\PrivateEvidence\beta-release\packet --trust-store $trust --trust-store-sha256 $approvedPin
```

Archive verification rejects extra/missing/changed files, changed notes/decision,
revoked/expired reviewers and missing executable permission on Unix binary files.
It **does not** re-check the current source checkout, cluster, DB or browser.
Use fresh `check`/`release` and upstream validation when those change. Signature
expiry and trust-store revocation must be checked at use time; an old GO is not
permanent authority. Updating a trust-store pin requires new bound final reviews.

## Exceptions and release-note scope

Each exception is exactly `{id, category, owner, reason, expires_at, evidence}`.
Only `nonblocking-operational-limitation` is accepted. Owners must be among the
approved reviewers; expiry must be within seven days. Exceptions cannot remove
R1–R6 certificates, change beta thresholds, permit skipped tests, grant a license,
or authorize public/production release. The resulting GO expires no later than
any approval, closure certificate or exception. Notes reproduce the signed
candidate scope and reviewers with escaped text; they do not create broader
capacity, platform, licensing or recovery guarantees.

## Current validation and outstanding acceptance

Local tests cover positive/negative signatures, independence, denial, revocation,
expiry, phase/hash/source/migration drift, fixed beta/soak checks, policy binding,
path attacks, duplicate/nonfinite JSON, packet copy/verification/tampering,
unsigned handoff and actual temporary-Git CLI paths. All fixture GO records are
explicitly synthetic. No project signer keys were created and no real candidate
received GO. PowerShell/hosted Windows execution and independent operational
adoption remain open. The new workflow has read-only repository permissions,
retains failures and contains no signing, publishing or deployment credentials.

The separate R7 implementation ledger records feature tests versus actual release
readiness. R6 code added in the preceding pass is still not certified; finishing
its code review, documentation and native/database validation remains necessary.

## Repository-local Windows runner timeout — November 17, 2026

The retained run at
`.worldwright-local/evidence/signoff-a55ae016739f4302808b00eb841af020/`
reported `interruption: timeout` for the R7 synthetic suite. Its log records 29
completed `ok` cases and an unfinished 30th case, not an assertion failure. The
wrapper recorded unchanged before/after source identities. Later suites were
not reached; the run remains failed and its evidence is preserved unchanged.

The runner's whole-suite deadline was 120 seconds. It is now 600 seconds for
`synthetic-tests` only, with 120 seconds retained for the other phases. Each
phase records `timeout_seconds` and `elapsed_seconds`, and reports interruption
on the console. The fixtures create independent Git checkouts and signed test
packets; changing their runner budget does not change a product latency, beta,
soak, signature, or acceptance requirement. No assertion/count was weakened,
no case was skipped, and no failed evidence was relabeled. Source changes
require a new collection directory and an observed complete rerun.

Use the repository-local `--tools-only` command in README.md. Earlier external
path examples in this document are not valid for the current Windows working
copy. Native storage adaptation and actual native/live acceptance remain open.
