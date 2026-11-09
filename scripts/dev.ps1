param(
    [ValidateSet('help', 'bootstrap', 'doctor', 'check', 'check-native', 'fmt', 'test', 'docs')]
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
function Show-Doctor {
    Assert-Cargo
    Write-Output "Workspace: $(Get-Location)"
    Write-Output "Cargo executable: $((Get-Command cargo).Source)"
    Write-Output "Rustc executable: $((Get-Command rustc).Source)"
    Get-Content rust-toolchain.toml | Select-String '^channel = '
    Invoke-Checked 'rustc' @('-vV')
    Invoke-Checked 'cargo' @('--version')
    Invoke-Checked 'rustc' @('--print', 'sysroot')
    if (Get-Command rustup -ErrorAction SilentlyContinue) {
        Invoke-Checked 'rustup' @('show', 'active-toolchain')
        Invoke-Checked 'rustup' @('target', 'list', '--installed')
    }
    foreach ($name in @('CARGO_BUILD_TARGET','CARGO_TARGET_DIR','RUSTUP_TOOLCHAIN','RUSTC','RUSTDOC','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','RUSTDOCFLAGS','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP')) {
        $present = Test-Path "Env:$name"
        Write-Output "Override ${name}: $(if ($present) {'SET (value redacted)'} else {'unset'})"
    }
    $directory = (Get-Location).Path
    while ($directory) {
        foreach ($name in @('.cargo/config', '.cargo/config.toml')) {
            $file = Join-Path $directory $name
            if (Test-Path $file -PathType Leaf) { Write-Output "Cargo configuration present: $file" }
        }
        $parent = Split-Path $directory -Parent
        if ($parent -eq $directory) { break }
        $directory = $parent
    }
    $cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $HOME '.cargo' }
    foreach ($name in @('config','config.toml')) {
        $file = Join-Path $cargoHome $name
        if (Test-Path $file -PathType Leaf) { Write-Output "User Cargo configuration present: $file" }
    }
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
            Write-Output 'Usage: ./scripts/dev.ps1 <bootstrap|doctor|check|check-native|fmt|test|docs>'
            Write-Output 'bootstrap may install the pinned toolchain and fetch locked dependencies. fmt changes source formatting.'
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
                throw 'Cargo.lock is missing; restore the committed lockfile or regenerate deliberately with cargo generate-lockfile.'
            }
            Invoke-Checked 'cargo' @('fetch', '--locked')
            Invoke-Checked 'cargo' @('metadata', '--format-version', '1', '--no-deps', '--locked', '--offline') | Out-Null
            Write-Output 'Bootstrap complete. Dependencies fetched from Cargo.lock; check runs offline.'
        }
        'doctor' { Show-Doctor }
        'check-native' {
            Assert-Lock
            Show-Doctor
            $pinMatch = [regex]::Match((Get-Content rust-toolchain.toml -Raw), '(?m)^channel = "([^"]+)"\r?$')
            if (-not $pinMatch.Success) { throw 'Cannot read pinned toolchain.' }
            $pin = $pinMatch.Groups[1].Value
            foreach ($program in @('rustc','cargo')) {
                $version = & $program --version
                if ($LASTEXITCODE -ne 0) { throw "$program version failed" }
                if ($version -notmatch ('^' + $program + ' ' + [regex]::Escape($pin) + '(\s|$)')) {
                    throw "$program must use pinned $pin; run bootstrap/select the approved toolchain."
                }
            }
            $details = & rustc -vV
            if ($LASTEXITCODE -ne 0) { throw 'Cannot determine native Rust host.' }
            $hostLine = $details | Select-String '^host: '
            if (-not $hostLine) { throw 'Cannot determine native Rust host.' }
            $hostTriple = ($hostLine.Line -replace '^host: ', '').Trim()
            $target = Join-Path ([IO.Path]::GetTempPath()) ('worldwright-native-check-' + [guid]::NewGuid().ToString('N'))
            New-Item -ItemType Directory -Path $target | Out-Null
            Write-Output "Fresh native build directory (retained for diagnosis): $target"
            Invoke-Checked 'cargo' @('fmt','--all','--','--check')
            Invoke-Checked 'cargo' @('check','--workspace','--all-targets','--locked','--offline','--target',$hostTriple,'--target-dir',$target)
            Invoke-Checked 'cargo' @('clippy','--workspace','--all-targets','--all-features','--locked','--offline','--target',$hostTriple,'--target-dir',$target,'--','-D','warnings')
            Invoke-Checked 'cargo' @('test','--workspace','--locked','--offline','--target',$hostTriple,'--target-dir',$target)
            Invoke-Checked 'cargo' @('build','--workspace','--locked','--offline','--target',$hostTriple,'--target-dir',$target)
            $previous = $env:RUSTDOCFLAGS
            try {
                $env:RUSTDOCFLAGS = "$previous -D warnings".Trim()
                Invoke-Checked 'cargo' @('doc','--workspace','--no-deps','--locked','--offline','--target',$hostTriple,'--target-dir',$target)
            } finally { $env:RUSTDOCFLAGS = $previous }
            Write-Output "PASS: native checks completed; $target"
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
            Show-Doctor
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
