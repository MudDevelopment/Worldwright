$ErrorActionPreference = "Stop"
if (-not (Get-Command helm -ErrorAction SilentlyContinue)) {
  Write-Output "SKIP: helm not installed"
  exit 0
}
$chart = Split-Path -Parent $PSScriptRoot
$out = Join-Path $env:TEMP "worldwright-helm-$PID.yaml"
try {
  helm lint $chart
  helm template worldwright $chart `
    --set-string image.repository=registry.example.invalid/worldwright `
    --set-string image.digest=sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa `
    --set-string publicOrigin=https://world.example.invalid `
    --set-string ingress.host=world.example.invalid `
    --set-string ingress.tlsSecretName=world-tls `
    --set-string postgres.existingSecret=world-postgres | Set-Content -Path $out
  $render = Get-Content $out -Raw
  if ($render -notmatch "Recreate") { throw "missing Recreate strategy" }
  if ($render -notmatch "automountServiceAccountToken: false") { throw "missing token disablement" }
  if ($render -notmatch "--acquire-writer") { throw "missing writer acquisition" }
  if ($render -notmatch "--behind-tls-ingress") { throw "missing ingress bind mode" }
  if ($render -match "--allow-insecure-http|--operator-") { throw "forbidden public flags" }
  if ($render -notmatch "initContainers") { throw "missing migration init container" }
  if ($render -notmatch "resources:") { throw "missing resources" }
  if ($render -notmatch "readOnlyRootFilesystem: true") { throw "missing filesystem hardening" }
  if ($render -notmatch "tlsSecretName") { throw "missing TLS configuration" }
  if ($render -match "--operator-|super-secret|client-secret-value") { throw "forbidden secret/operator output" }
  $bad = helm template worldwright $chart --set-string image.repository=x --set-string image.digest="" --set-string publicOrigin=https://world.example.invalid --set-string ingress.host=world.example.invalid --set-string ingress.tlsSecretName=tls --set-string postgres.existingSecret=db --set image.allowMutableTag=false 2>&1
  if ($LASTEXITCODE -eq 0) { throw "mutable image gate did not fail" }
  $bad = helm template worldwright $chart --set-string image.repository=x --set-string image.digest=sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa --set-string publicOrigin=https://world.example.invalid --set-string ingress.host=world.example.invalid --set-string ingress.tlsSecretName=tls --set-string postgres.existingSecret=db --set oidc.enabled=true 2>&1
  if ($LASTEXITCODE -eq 0) { throw "OIDC partial config gate did not fail" }
  Write-Output "helm render smoke passed"
} finally {
  Remove-Item $out -Force -ErrorAction SilentlyContinue
}
