#requires -Version 5.1
# Native development launcher only; not a test or acceptance-closing collector.
[CmdletBinding()]
param([string]$Workspace)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $repo
if (-not (Test-Path -LiteralPath (Join-Path $repo 'Cargo.toml') -PathType Leaf)) {
    throw 'Run this script from the Worldwright scripts directory'
}

$nativeTarget = 'x86_64-pc-windows-msvc'
$targetDir = Join-Path $env:TEMP 'worldwright-native-check-e73e9337f80540e5a64ef73e18782200'
$env:CARGO_BUILD_TARGET = $nativeTarget
$env:CARGO_TARGET_DIR = $targetDir

$rustDetails = & rustc -vV
if ($LASTEXITCODE -ne 0) { throw 'rustc inspection failed' }
if ($rustDetails -notcontains 'release: 1.85.1' -or
    $rustDetails -notcontains "host: $nativeTarget") {
    throw 'Require Rust 1.85.1 on native Windows MSVC'
}
$cargoVersion = & cargo --version
if ($LASTEXITCODE -ne 0 -or $cargoVersion -notmatch '^cargo 1\.85\.1(\s|$)') {
    throw 'Require Cargo 1.85.1'
}

$reopen = -not [string]::IsNullOrWhiteSpace($Workspace)
if ($reopen) {
    $Workspace = (Resolve-Path -LiteralPath $Workspace).Path
    if (-not (Test-Path -LiteralPath (Join-Path $Workspace '.worldwright-studio') -PathType Leaf)) {
        throw 'Reopen requires an existing Studio workspace; no files were removed'
    }
}

$binDir = Join-Path (Join-Path $targetDir $nativeTarget) 'debug'
$studio = Join-Path $binDir 'worldwright-studio.exe'
$preview = Join-Path $binDir 'worldwright.exe'
Write-Host "Expected Studio: $studio"
Write-Host "Expected preview: $preview"
$buildArgs = @(
    'build', '--locked', '--offline',
    '--target', $nativeTarget, '--target-dir', $targetDir,
    '-p', 'worldwright-studio', '-p', 'worldwright',
    '--bin', 'worldwright-studio', '--bin', 'worldwright'
)
& cargo @buildArgs
if ($LASTEXITCODE -ne 0) { throw 'Targeted build failed; launch stopped' }
foreach ($exe in @($studio, $preview)) {
    if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
        throw "Expected binary is missing after successful build: $exe"
    }
}
Get-FileHash -Algorithm SHA256 -LiteralPath @($studio, $preview)

if (-not $reopen) {
    $evidence = Join-Path $env:LOCALAPPDATA ('Worldwright-R5-Launch-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $evidence | Out-Null
    $Workspace = Join-Path $evidence 'native-studio-workspace'
}
$serveArgs = @('serve', '--workspace', $Workspace, '--preview-command', $preview, '--bind', '127.0.0.1:0')
if (-not $reopen) {
    $content = (Resolve-Path -LiteralPath (Join-Path $repo 'examples/two-room/world.json')).Path
    $serveArgs += @('--content', $content)
}
Write-Host "Keep this workspace path: $Workspace"
Write-Host 'Open the printed loopback URL. Leave this terminal running; Ctrl+C requests shutdown.'
& $studio @serveArgs
if ($LASTEXITCODE -ne 0) { throw 'Studio exited with an error; preserve output and workspace' }
