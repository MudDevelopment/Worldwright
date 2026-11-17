#requires -Version 5.1
# Native R6 prerequisite collection; never a release/security approval.
[CmdletBinding()]
param([string]$Python = 'python', [switch]$AllowDirty, [string]$BrowserExecutable)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if ($Python -eq 'python') {
    $Python = Join-Path $repo '.worldwright-local\python\Worldwright-R6-Python\Scripts\python.exe'
}
$oldBytecode = $env:PYTHONDONTWRITEBYTECODE
$oldColor = $env:CARGO_TERM_COLOR
Push-Location -LiteralPath $repo
try {
    $env:PYTHONDONTWRITEBYTECODE = '1'
    $env:CARGO_TERM_COLOR = 'never'
    $arguments = @('-B', (Join-Path $PSScriptRoot 'collect-worldwright-signoff.py'), '--repo', $repo, '--native')
    if ($AllowDirty) { $arguments += '--allow-dirty' }
    if ($BrowserExecutable) { $arguments += @('--browser-executable', $BrowserExecutable) }
    & $Python @arguments
    if ($LASTEXITCODE -ne 0) { throw 'R6 collection stopped; preserve the repository-local evidence directory printed above.' }
    Write-Host 'Collected prerequisites only. R6 security/recovery/retention/independent acceptance remains OPEN.'
} finally {
    $env:PYTHONDONTWRITEBYTECODE = $oldBytecode
    $env:CARGO_TERM_COLOR = $oldColor
    Pop-Location
}
