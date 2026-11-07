## Studio publication client

Studio uses the authenticated M6 operator release routes:

- `POST /api/v1/operator/releases` with the exact
  `application/vnd.worldwright.release-submission-v1` envelope;
- `GET /api/v1/operator/releases/<request-id>` for status.

Studio never supplies database credentials to the browser and never bypasses
M6 grants, expected-current checks, review, activation, or audit. A submitted
request remains `Requested`; approval and activation are separate operator
actions.

# Operator Control Runbook

M6 is implemented awaiting live PostgreSQL and recovery execution. Production
readiness is not established.

## Bootstrap and credentials

Bootstrap the first operator only against a database with zero operators:

```powershell
cargo run -p worldwright-server --locked --offline -- operator bootstrap `
  --database-url-env WORLDWRIGHT_DATABASE_URL `
  --subject admin `
  --display-name 'Operations Admin' `
  --kind human `
  --environment staging `
  --capabilities view_status,manage_operators,request_release,review_release,activate_release,initiate_recovery,backup_world,restore_world,drain_world `
  --reason 'first operator' `
  --confirm-first-operator
```

Create local credential material and issue a short-lived token. Human and
break-glass operators receive a TOTP enrollment secret; service operators do
not. The signing key is supplied only through an environment variable.

```powershell
cargo run -p worldwright-console --locked --offline -- credential create `
  --output .\admin.credential.json --subject admin --kind human

cargo run -p worldwright-console --locked --offline -- token issue `
  --credential .\admin.credential.json `
  --signing-key-env WORLDWRIGHT_OPERATOR_SIGNING_KEY `
  --issuer worldwright --audience worldwright-ops
```

Enable the operator routes only on durable loopback serve:

```powershell
cargo run -p worldwright-server --locked --offline -- serve `
  --database-url-env WORLDWRIGHT_DATABASE_URL --acquire-writer `
  --origin http://127.0.0.1:3000 --allow-insecure-http `
  --operator-key-env WORLDWRIGHT_OPERATOR_SIGNING_KEY `
  --operator-issuer worldwright --operator-audience worldwright-ops `
  --environment staging
```

## Console/API operations

The Console requires `--endpoint http://127.0.0.1:PORT --token-env ENV`.
Available operations are status, metrics, moderation, session revocation,
operator create/enable/disable, grant create/revoke, and drain. Mutation
commands require a bounded reason and generate a cryptographic correlation ID.

```powershell
cargo run -p worldwright-console --locked --offline -- status `
  --endpoint http://127.0.0.1:3000 --token-env WORLDWRIGHT_OPERATOR_TOKEN
cargo run -p worldwright-console --locked --offline -- moderate `
  --endpoint http://127.0.0.1:3000 --token-env WORLDWRIGHT_OPERATOR_TOKEN `
  --account 00000000000000000000000000000001 --disabled true `
  --reason 'manual moderation'

cargo run -p worldwright-console --locked --offline -- operator create `
  --endpoint http://127.0.0.1:3000 --token-env WORLDWRIGHT_OPERATOR_TOKEN `
  --subject reviewer --display-name 'Release Reviewer' --kind human `
  --reason 'create independent reviewer'

cargo run -p worldwright-console --locked --offline -- grant create `
  --endpoint http://127.0.0.1:3000 --token-env WORLDWRIGHT_OPERATOR_TOKEN `
  --grant 00000000000000000000000000000010 --subject reviewer `
  --capability review_release --environment staging `
  --world 00000000000000000000000000000001 `
  --reason 'grant staging review'
```

Drain records start and completion audits, closes admission, and requires a
bounded quiescence path. A failed completion audit must not claim success.
Metrics are bounded and contain no token, subject, request, or correlation
labels.

## Release authorization

Release commands use database, operator token/key, issuer, audience,
environment, and reason options. `--requested-by`, `--reviewed-by`, and
`--executor` are obsolete and rejected. Production activation requires a
separate reviewer and current MFA-backed `ActivateRelease` grant.

## Security and evidence

The listener remains loopback without TLS. Backup manifest HMACs prove integrity,
not confidentiality or encryption at rest. PostgreSQL tests compile but have
not executed; no manual Console/browser, Linux, CI, deployment, or production
host evidence is claimed.
