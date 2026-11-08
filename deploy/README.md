# Deployment

The repository includes an implemented local Worldwright server image and
Helm chart at `deploy/helm/worldwright`. The chart has no bundled PostgreSQL:
provide an existing database secret and current migrations.

## Image

Build the pinned multi-stage image from the repository root:

```text
docker build --tag worldwright-server:local .
```

The runtime image runs as UID/GID 65532, exposes port 8080, uses a read-only
root filesystem in the chart, and contains no operator credentials.

## Helm

Render with nonsecret synthetic values:

```text
helm template ww ./deploy/helm/worldwright \
  --set-string image.repository=registry.example.invalid/worldwright-server \
  --set-string image.digest=sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa \
  --set-string publicOrigin=https://world.example.invalid \
  --set-string ingress.host=world.example.invalid \
  --set-string ingress.tlsSecretName=world-tls \
  --set-string postgres.existingSecret=world-postgres
```

Install with the same immutable digest and a pre-created Secret containing the
database URL under the configured `postgres.key`. The init container applies
and verifies database migrations; it does not seed worlds. Seed the world
first, then submit/review/activate a matching release. Issue invites after
seeding and OIDC provider configuration through the approved maintenance
workflow before expecting readiness. Rotate secrets by updating the external
Secret and explicitly restarting the Deployment so environment references are
reloaded. Helm rollback changes manifests only; database writer/release
recovery remains an operator procedure.

With OIDC enabled, add:

```text
--set-string oidc.enabled=true \
--set-string oidc.issuer=https://id.example.invalid \
--set-string oidc.clientId=worldwright \
--set-string oidc.redirectUri=https://world.example.invalid/api/v1/oidc/callback \
--set-string oidc.existingSecret=worldwright-oidc \
--set-string oidc.key=WORLDWRIGHT_OIDC_CLIENT_SECRET
```

The chart always deploys one replica with Recreate strategy and a single
writer. Mutable tags require explicit `image.allowMutableTag=true`; immutable
digests are preferred. OIDC is disabled by default and, when enabled, requires
an existing Secret for the client secret. The public operator plane is not
configured.

Ingress is required and TLS terminates at the ingress. The server runs with
`--behind-tls-ingress` on the trusted cluster-internal cleartext hop; it does
not trust Forwarded/X-Forwarded-* headers. This mode is not permission for
arbitrary public HTTP. No live Docker build,
Helm cluster install, ingress controller, PostgreSQL
deployment, or production capacity test is claimed by these files.
