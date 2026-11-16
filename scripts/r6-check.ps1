#requires -Version 5.1
# Native R6 prerequisite collection; never a release/security approval.
[CmdletBinding()]
param([string]$Python = 'python', [switch]$AllowDirty, [string]$BrowserExecutable)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$output = Join-Path $env:LOCALAPPDATA ('Worldwright-R6-' + [guid]::NewGuid().ToString('N'))
$target = Join-Path $env:TEMP 'worldwright-native-check-e73e9337f80540e5a64ef73e18782200'
$oldBytecode = $env:PYTHONDONTWRITEBYTECODE
$oldColor = $env:CARGO_TERM_COLOR
Push-Location -LiteralPath $repo
try {
    $env:PYTHONDONTWRITEBYTECODE = '1'
    $env:CARGO_TERM_COLOR = 'never'
    & $Python -c 'import sys; sys.exit(0 if sys.version_info >= (3,11) else 1)'
    if ($LASTEXITCODE -ne 0) { throw 'Python prerequisite failed' }
    & $Python (Join-Path $PSScriptRoot 'test-r6.py')
    if ($LASTEXITCODE -ne 0) { throw 'Collector tests failed; native collection not started' }
    Write-Host "New R6 evidence directory: $output"
    $arguments = @((Join-Path $PSScriptRoot 'r6-native.py'), '--output', $output, '--target-dir', $target)
    if ($AllowDirty) { $arguments += '--allow-dirty' }
    if ($BrowserExecutable) { $arguments += @('--browser-executable', $BrowserExecutable) }
    & $Python @arguments
    if ($LASTEXITCODE -ne 0) { throw "R6 collection stopped; preserve $output" }
    & $Python (Join-Path $PSScriptRoot 'r6-evidence.py') --report (Join-Path $output 'r6-report.json') --check-local
    if ($LASTEXITCODE -ne 0) { throw "R6 evidence validation failed; preserve $output" }
    Write-Host 'Collected prerequisites only. R6 security/recovery/retention/independent acceptance remains OPEN.'
} finally {
    $env:PYTHONDONTWRITEBYTECODE = $oldBytecode
    $env:CARGO_TERM_COLOR = $oldColor
    Pop-Location
}
