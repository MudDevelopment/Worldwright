# R4 — fixed beta-load and measured optimization

This is the execution guide, not a measured capacity result. Authority:
ROADMAP_v2.md R4 and the unchanged milestone-9-load-contract.md. Full R4 requires
current R2/R3 closure, approved protected staging, 250 beta accounts, two separate
fresh same-room scenario accounts, independent monitoring and retention adoption.

## Implemented source

- Native fixed Rust beta runner retains exactly 250 clients, 30 Look commands,
  2-second periods/offsets, 750 ms p95, 1500 ms p99, maximum below 10 seconds,
  wall time at most 90 seconds and zero failures. No easier public run flags.
- Smoke returns tooling success via additive `tooling_smoke_passed` while its
  beta `passed` remains false. Bounded latency channel/history preserves recipe.
- Eight fixed owner histograms include snapshot/export and idle timer lag.
  Persistence exports fixed pool-begin/commit/replacement durations, attempted
  intent/replacement cardinalities, confirmed commits and pool size/idle gauges.
- Read-only monitor records protected loopback metrics, Linux server PID/start/
  executable hash, CPU/RSS, PG version/role/DB fingerprint, app table statistics,
  runtime role/Pod peer and stable active digest/writer/incarnation/state.
- Seven separate scenarios: speech fanout; welcome reward replay/business
  uniqueness; transfer replay/one owner; bounded slow reader; fresh reconnect;
  idle app heartbeat; and a two-client Look soak of an adopted 1800–3000 seconds.
  None is a substitute for beta-250. No chat/cookie/password bodies are evidence.
- Collection preserves failure reports/logs, reruns real R3 and native integrity
  gates, builds the current release load binary and binds the exact deployment.
  Closure rechecks hashes, actual sample-derived growth and independent adoption.
- Comparison reports p95/p99 deltas for matched environment/artifact/host only.
  No optimization is justified without observed bottleneck evidence. Each
  candidate must rerun integrity suites and the unchanged recipe; no capacity
  increase or complete SQL attribution follows from a comparison file.

## Local checks

```bash
python -m pip install -r scripts/r2-requirements.txt -r deploy/helm/worldwright/tests/requirements.txt
python scripts/test-r4.py
python scripts/test-r3-io.py
python deploy/helm/worldwright/tests/test_verify.py
python deploy/helm/worldwright/tests/verify.py
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked --offline
cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
cargo test --workspace --locked --offline
```

Source/contract fixtures and loopback transport are not current native Rust or
Worldwright/PostgreSQL deployment tests. Missing Cargo/Helm/DB/adoption fails;
no successful skip replaces a required gate. Current results belong in the R4
ledger; historical R3 observations retain their original scope/source hashes.

## Prepare the controlled environment

1. Close same-source R2/R3 first. Never alter immutable SQL/checksums to get past
   the known fresh-migration failure. Provision a separate owned disposable
   `WORLDWRIGHT_TEST_DATABASE_URL` for integrity regressions, not the load DB.
2. Enable chart `telemetry: {enabled: true, port: 9090}`. This is additive within
   chart values format 2; absent/disabled stays disabled. It binds 127.0.0.1 only
   and creates no Service/Ingress. Establish an approved private Pod port-forward
   to a nonzero loopback port; the runner does not adopt access permissions.
3. Provision monitor credentials via the plan's named environment variable.
   PostgreSQL requires verify-full TLS, approved CA/DNS and read/statistics
   privileges; the collector grants none and enables read-only transactions.
4. Identify the actual server's host-visible Linux `/proc` PID. Access to the
   real process/Pod/database must be independently reviewed on the isolated
   runner. A similarly named process or private metrics alone does not prove
   the same image; R3 deployment UID/image/artifact binding is also required.
5. Seed exactly 250 load accounts plus two distinct fresh same-room scenario
   accounts (the current 256-character bound allows 252). Protect plaintext
   credentials. The scenario accounts must not be in the beta credentials file.
   Welcome must be unclaimed. Reset only an approved disposable fixture, never
   receipts/entitlements in an existing operational world to make tests pass.
6. An independent performance/service reviewer adopts and signs the complete
   plan and later the exact report. Keep signing private keys outside the tool.

## Plan and approval contracts

The new R4 plan is independent schema 1, not chart/SQL/protocol schema evolution.
`r4_common.PLAN_FIELDS` is the strict field inventory; unknown/missing keys fail.
Supply every field, without inserting example approvals:

| Fields | Meaning |
|---|---|
| schema_version, kind, status | 1 / r4-performance-plan / adopted |
| reviewer, expires_at, signature | Actual reviewer, unexpired max-30-day UTC epoch, base64 Ed25519 signature |
| source_sha256 | Current complete source inventory from r0-verify.py; source edits require fresh plan/R2/R3 |
| base_url, artifact_digest, environment_label | Actual canonical HTTPS origin, 64-lowercase-hex active artifact, bounded nonsecret label |
| r3_arguments | Absolute private JSON path to all five current R3 closure inputs |
| credentials, scenario_credentials | Absolute protected beta-250 and separate two-account JSON paths |
| database_url_env, runtime_role, schema, pod_ip | Approved observer env ref and independently checked runtime DB/Pod identities |
| process_pid, telemetry_url | Actual host-visible server PID; approved http loopback /metrics port-forward |
| context, namespace, deployment | Approved isolated Kubernetes identities matching closed R3 deployment |
| soak_seconds | Predeclared 1800–3000 seconds; never lowers beta requirements |
| max_rss_growth_bytes, max_history_growth_bytes | Explicit reviewer-adopted resource bounds, integers 0–1 GiB; not hardcoded capacity promises |
| retention_policy_sha256 | Exact independently adopted policy identity; this tool neither invents policy nor deletes history |

Sign sorted compact JSON excluding `signature`, UTF-8, using an independently
trusted Ed25519 key. Pass the approved public key separately; do not generate a
key and call that service approval. Final `r4-performance-adoption` binds report,
source, active digest, image, deployment UID, retention policy, reviewer/expiry;
it additionally requires `capacity_scope: beta-250-v1-only` and
`retention_adopted: true`, signed by the actual authorized reviewer.

## Execute only on approved infrastructure

```bash
python scripts/r4-verify.py collect --plan /private/r4-plan.json \
  --approval-public-key /private/trusted-performance-public-key.pem \
  --output /private/new-r4-evidence --confirm-load
python scripts/r4-verify.py close --report /private/new-r4-evidence/r4-report.json \
  --adoption /private/r4-adoption.json \
  --approval-public-key /private/trusted-performance-public-key.pem
python scripts/r4-verify.py compare --baseline /private/baseline/beta.json \
  --candidate /private/candidate/beta.json --bottleneck 'reviewed observed finding' \
  --output /private/new-comparison.json
```

A failed collector still creates `r4-report.json` and any actually completed/
interrupted phase logs. Each retry uses a new external directory. Monitor JSONL
and reports remain private. The monitor samples up to 4000 records, rejects PID/
authority/artifact/DB resets, and closure requires gap-bounded coverage of load
and soak. Whole-database transaction counts include observer queries; table
statistics are asynchronous and not a per-command SQL execution trace.
Attempted replacement cardinality can include failures/replays and is not a
confirmed row-count metric. A measured optimization review must interpret these
limits and collect finer independent PG traces where necessary.

R4 CI is paired-host contract coverage plus an explicitly dispatched,
default-branch, reviewer-environment-gated isolated self-hosted measured job.
Configure `worldwright-r4-acceptance` reviewers, runner label and the approved
`WORLDWRIGHT_R4_APPROVAL_PUBLIC_KEY_PATH` before provisioning credentials. YAML
is not protection adoption. Artifacts retain only JSON/JSONL/logs, not native
build output or credential inputs. Hosted runs and independent approval remain
required. Scope is only this tested profile/environment; R7 go/no-go and wider
production capacity require their own evidence.
