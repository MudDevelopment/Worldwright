param(
    [ValidateSet('help', 'bootstrap', 'doctor', 'check', 'fmt', 'test', 'docs')]
    [string]$Task = 'help'
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if ($args.Count -gt 0) { throw 'Expected one task; use help.' }

function Invoke-Checked {
    param([string]$Program, [string[]]$Arguments)
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Program failed with exit code $LASTEXITCODE"
    }
}
function Assert-Cargo {
    Get-Command cargo -ErrorAction Stop | Out-Null
    Get-Command rustc -ErrorAction Stop | Out-Null
}
function Assert-Lock {
    Assert-Cargo
    if (-not (Test-Path Cargo.lock)) { throw 'Run bootstrap first, then commit Cargo.lock.' }
}
function Build-Docs {
    $previous = $env:RUSTDOCFLAGS
    try {
        $env:RUSTDOCFLAGS = "$previous -D warnings".Trim()
        Invoke-Checked 'cargo' @('doc', '--workspace', '--no-deps', '--locked', '--offline')
    } finally {
        $env:RUSTDOCFLAGS = $previous
    }
}

Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    switch ($Task) {
        'help' {
            Write-Output 'Usage: ./scripts/dev.ps1 <bootstrap|doctor|check|fmt|test|docs>'
            Write-Output 'bootstrap may install the pinned toolchain. fmt changes source formatting.'
        }
        'bootstrap' {
            $match = [regex]::Match((Get-Content rust-toolchain.toml -Raw), '(?m)^channel = "([^"]+)"\r?$')
            if (-not $match.Success) { throw 'Cannot read pinned toolchain.' }
            if (Get-Command rustup -ErrorAction SilentlyContinue) {
                Invoke-Checked 'rustup' @('toolchain', 'install', $match.Groups[1].Value, '--profile', 'minimal', '--component', 'rustfmt', '--component', 'clippy')
            } else {
                Write-Output 'rustup is absent; using installed tools. Verify their versions against rust-toolchain.toml.'
            }
            Assert-Cargo
            Invoke-Checked 'rustc' @('--version')
            Invoke-Checked 'cargo' @('--version')
            if (-not (Test-Path Cargo.lock)) {
                Invoke-Checked 'cargo' @('generate-lockfile', '--offline')
            }
            Invoke-Checked 'cargo' @('metadata', '--format-version', '1', '--no-deps', '--locked', '--offline') | Out-Null
            Write-Output 'Bootstrap complete. Review and commit Cargo.lock.'
        }
        'doctor' {
            Assert-Cargo
            Invoke-Checked 'rustc' @('--version')
            Invoke-Checked 'cargo' @('--version')
            Write-Output "Workspace: $(Get-Location)"
            Get-Content rust-toolchain.toml | Select-String '^channel = '
        }
        'fmt' {
            Get-Command cargo -ErrorAction Stop | Out-Null
            Invoke-Checked 'cargo' @('fmt', '--all')
        }
        'test' {
            Assert-Lock
            Invoke-Checked 'cargo' @('test', '--workspace', '--locked', '--offline')
        }
        'docs' { Assert-Lock; Build-Docs }
        'check' {
            Assert-Lock
            Invoke-Checked 'rustc' @('--version')
            Invoke-Checked 'cargo' @('--version')
            Invoke-Checked 'cargo' @('fmt', '--all', '--', '--check')
            Invoke-Checked 'cargo' @('check', '--workspace', '--all-targets', '--locked', '--offline')
            Invoke-Checked 'cargo' @('clippy', '--workspace', '--all-targets', '--all-features', '--locked', '--offline', '--', '-D', 'warnings')
            Invoke-Checked 'cargo' @('test', '--workspace', '--locked', '--offline')
            Invoke-Checked 'cargo' @('build', '--workspace', '--locked', '--offline')
            Build-Docs
        }
    }
} finally {
    Pop-Location
}
