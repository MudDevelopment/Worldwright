#requires -Version 5.1
[CmdletBinding()]
param([string]$Python = 'python')
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$output = Join-Path $env:LOCALAPPDATA ('Worldwright-R7-Checks-' + [guid]::NewGuid().ToString('N'))
$oldBytecode = $env:PYTHONDONTWRITEBYTECODE
Push-Location -LiteralPath $repo
try {
    $env:PYTHONDONTWRITEBYTECODE = '1'
    & $Python -c 'import sys; sys.exit(0 if sys.version_info >= (3,11) else 1)'
    if ($LASTEXITCODE -ne 0) { throw 'Python 3.11+ is required' }
    Write-Host "R7 tool-test evidence: $output"
    & $Python (Join-Path $PSScriptRoot 'r7-check.py') --output $output
    if ($LASTEXITCODE -ne 0) { throw "R7 tool validation failed; preserve $output" }
    Write-Host 'R7 tooling checks finished. No real beta release or earlier gate was approved.'
} finally {
    Pop-Location
    if ($null -eq $oldBytecode) { Remove-Item Env:PYTHONDONTWRITEBYTECODE -ErrorAction SilentlyContinue } else { $env:PYTHONDONTWRITEBYTECODE = $oldBytecode }
}
