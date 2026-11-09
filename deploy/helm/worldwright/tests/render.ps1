Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$python = Get-Command python -ErrorAction SilentlyContinue
if (-not $python) { $python = Get-Command python3 -ErrorAction SilentlyContinue }
if (-not $python) { throw 'Python 3 is required for chart acceptance.' }
$chart = Split-Path -Parent $PSScriptRoot
# The shared harness checks each Helm subprocess exit immediately and parses
# actual YAML resources. Do not pipe native output before checking its exit.
& $python.Source (Join-Path $PSScriptRoot 'verify.py') --chart $chart
if ($LASTEXITCODE -ne 0) { throw "Helm acceptance failed with exit code $LASTEXITCODE" }
