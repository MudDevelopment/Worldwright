# Restricted Worldwright deployment — R3

The current deployment contract is chart **0.2.0**, values **`configVersion: 2`**,
application **`0.1.0-dev.0`**, Linux/amd64 image build, and one `Recreate` writer.
This is private staging tooling, not an accepted public/production service.
Current R0/R2 prerequisites remain open. See [R3 acceptance](../docs/R3_ACCEPTANCE.md)
and the [R3 ledger](../docs/verification/r3-acceptance-ledger.md) for observed checks
and blocked gates. Historical milestone reports do not verify current source.

## Image: required immutable base inputs

`Dockerfile` no longer has mutable builder/runtime defaults. An unqualified
`docker build .` is intentionally insufficient. Choose approved digest-pinned
builder and Debian-12-compatible runtime references through the service/security
owner; no example digest here is an approval. The builder checks Rust 1.85.1 and
builds `worldwright-server` for `x86_64-unknown-linux-gnu`. The runtime needs glibc,
libgcc and CA roots, uses UID/GID 65532, and declares SIGTERM. There is no mutable
apt-index install in the runtime stage.

For an actual reviewed build/push with captured provenance:

```bash
python scripts/r3-image.py \
  --output "$R3_IMAGE_EVIDENCE" \
  --builder-image "$R3_RUST_IMAGE" \
  --runtime-image "$R3_RUNTIME_IMAGE" \
  --tag "$R3_REGISTRY_TAG" \
  --confirm-registry-push
```

The variables are operator-supplied real inputs; the output directory must be
new, private and outside the checkout. The script records source identity,
builder/runtime references, buildx metadata, registry manifest digest, allowed
platform image IDs and non-root read-only `--version` linkage. The image config
ID is separate and must not substitute for the deployable manifest digest.
This check is not a running-service, CNI, provider, license or signing approval.
P1's local container smoke now also requires reviewed repository variables
`WORLDWRIGHT_RUST_IMAGE` and `WORLDWRIGHT_RUNTIME_IMAGE` containing pinned refs;
its local tag/linkage result is not R3 registry/staging acceptance.

## Chart values format 2: breaking security tightening

Old format-1 configurations must be explicitly migrated or rebuilt. There is no
implicit opt-in to mutable tags, shared DB identities or unrestricted networking.

| Input | Current meaning |
|---|---|
| `image.repository`, `image.digest` | Immutable deployable image; `tag` must be empty and `allowMutableTag` false |
| `postgres.existingSecret`, `postgres.key` | Runtime login URL reference |
| `postgres.migrationSecret`, `postgres.migrationKey` | Separate migration login URL reference; Secret name must differ |
| Recovery login | Independently provisioned/reviewed; never mounted in the public runtime pod |
| `publicOrigin`, ingress host/TLS Secret | Exact HTTPS origin and matching TLS ingress host |
| `networkPolicy.namespaceSelector`, `podSelector` | Combined ingress-controller namespace AND pod restriction |
| DNS namespace/pod selectors | Combined scoped DNS peer; verify labels against the actual cluster |
| `postgresCidr` | Canonical private non-loopback database network; no world-open CIDR |
| `httpsEgressCidrs` | Explicit bounded HTTPS destinations, required when OIDC is enabled |
| OIDC fields | Complete issuer/client/callback/Secret refs only when enabled; callback matches public origin |

Selectors are scoped to this release. CIDR policies do not implement dynamic
FQDN allowlisting: review actual provider endpoints/IP changes. The standard DNS
label defaults are a configuration starting point, not proof of a cluster's CNI
or CoreDNS topology. Distinct Secret names do not prove distinct DB logins or
least privilege; run the live role audit described in the acceptance guide.

To migrate a known format-1 file, supply every newly security-relevant input:

```bash
python deploy/helm/worldwright/tests/migrate-values.py \
  --input "$R3_OLD_VALUES" --output "$R3_NEW_VALUES" \
  --migration-secret "$R3_MIGRATION_SECRET_NAME" \
  --digest "$R3_MANIFEST_DIGEST" --postgres-cidr "$R3_POSTGRES_CIDR" \
  --ingress-namespace "$R3_INGRESS_NAMESPACE" --ingress-label "$R3_INGRESS_LABEL" \
  --dns-namespace "$R3_DNS_NAMESPACE" --dns-label "$R3_DNS_LABEL"
```

Labels use `key=value`. Add a repeatable `--https-egress-cidr` for each reviewed
CIDR when OIDC is enabled. The tool writes a new private file exclusively, leaves
the old file untouched and validates the resulting complete format-2 values.
It does not create Secrets, users, certificates, grants or approval.

## Render and verify, without deploying

The repository's acceptance values use synthetic `.invalid` hosts, references
and digests. They are valid checker fixtures, not usable/approved staging inputs.

```bash
python -m pip install -r deploy/helm/worldwright/tests/requirements.txt
python -m pip install -r scripts/r2-requirements.txt
python deploy/helm/worldwright/tests/test_verify.py
python scripts/test-r3.py
python deploy/helm/worldwright/tests/verify.py
helm template worldwright deploy/helm/worldwright \
  --values deploy/helm/worldwright/tests/acceptance-values.yaml
```

The shared verifier checks both games with OIDC off/on, schema/semantic refusals,
actual Helm lint/render exit codes and parsed security/identity/policy resources.
Missing Helm is failure, not a passing skip. CI selects Helm v3.19.0 on Ubuntu
and Windows. Supplemental execution with another Helm 3 version is recorded
separately and cannot be relabeled as that hosted/pinned run.

## Actual deployment and recovery boundary

The chart supplies neither PostgreSQL nor seed/release data. An approved
maintenance workflow must verify migrations, seed the test world, package and
activate a matching artifact before a coherent public runtime can become ready.
The init container only applies/verifies migrations with the migration identity;
it does not seed or grant runtime privileges. The current operational migration
has an observed fresh-apply syntax defect; do not edit applied SQL/checksums or
silently bypass the failed R2 prerequisite.

TLS terminates at the reviewed ingress. The server uses `--behind-tls-ingress`
only for its trusted internal HTTP hop and does not trust arbitrary forwarded
headers. The public operator listener/plane is not enabled. Unix SIGTERM/SIGHUP
and Windows Ctrl+C enter the existing shutdown path; controlled exit and database
reconstruction still need actual native/staging evidence.

`r3-staging.py` requires an explicit private plan, passed current R0/R2 and image
inputs, pre-existing marked isolated namespace, credential environment references,
and `--confirm-staging-mutations`. It performs real readiness, image/role peer,
WSS/idle/origin, restart/rotation and failed-rollout/Helm-rollback checks. It never
creates an approved cluster, provider, service owner or credential. Its result is
`collected_not_closed`; four independently witnessed gates remain required.

Helm rollback changes manifests, not committed SQL, active content, writer
incarnation, receipt history or game-state encodings. Keep a recovery plan and
retain failures; an interrupted exercise may leave staging not ready. Recover
through the reviewed maintenance workflow, not by deleting evidence or assuming
an old binary can read new schema/content. Follow the detailed R3 guide.
