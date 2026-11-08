#!/bin/sh
set -eu
if ! command -v helm >/dev/null 2>&1; then echo "SKIP: helm not installed"; exit 0; fi
chart=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
helm lint "$chart"
out=$(mktemp)
trap 'rm -f "$out"' EXIT
helm template worldwright "$chart" \
  --set-string image.repository=registry.example.invalid/worldwright \
  --set-string image.digest=sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa \
  --set-string publicOrigin=https://world.example.invalid \
  --set-string ingress.host=world.example.invalid \
  --set-string ingress.tlsSecretName=world-tls \
  --set-string postgres.existingSecret=world-postgres >"$out"
grep -q 'strategy: { type: Recreate }' "$out"
grep -q 'automountServiceAccountToken: false' "$out"
grep -q -- '--acquire-writer' "$out"
grep -q -- '--behind-tls-ingress' "$out"
! grep -q -- '--allow-insecure-http' "$out"
grep -q 'initContainers' "$out"
grep -q 'resources:' "$out"
grep -q 'readOnlyRootFilesystem: true' "$out"
grep -q 'tlsSecretName' "$out"
! grep -q -- '--operator-' "$out"
! grep -q 'super-secret\|client-secret-value' "$out"
if helm template worldwright "$chart" --set-string image.repository=x --set-string image.digest= --set-string publicOrigin=https://world.example.invalid --set-string ingress.host=world.example.invalid --set-string ingress.tlsSecretName=tls --set-string postgres.existingSecret=db --set image.allowMutableTag=false >/dev/null 2>&1; then exit 1; fi
if helm template worldwright "$chart" --set-string image.repository=x --set-string image.digest=sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa --set-string publicOrigin=https://world.example.invalid --set-string ingress.host=world.example.invalid --set-string ingress.tlsSecretName=tls --set-string postgres.existingSecret=db --set oidc.enabled=true >/dev/null 2>&1; then exit 1; fi
echo "helm render smoke passed"
