# Repository-local native storage and collection

This guide applies to the E:\Hold\Worldwright working copy. All generated
project files stay under `.worldwright-local/`. Installed system Python,
Windows SDK/MSVC tools, Git and Node may be read/executed from their existing
locations. This is cooperative tool configuration, not an OS filesystem sandbox.
It does not disable endpoint security, alter ACLs, or grant release authority.

## Status

The previous 70-test Windows helper run is preserved at
`.worldwright-local/evidence/signoff-e05fc054b3694bdabadea22f4f8cf8b1/` and is
acknowledged in `validation-acknowledgments.md` in its parent directory.
It proves that earlier source tuple, not the subsequent native-path edits.
Windows preparation completed in `native-storage-prepare-d82f1d3a6dd5415e812e71337e318f69`.
The following check passed the original 12 path/import tests, compiler/tool
checks, offline metadata, and compiled/ran the tiny probe. It then hit a checker
bug: Rust emitted an extended Windows drive path for Cargo and Python path-value
comparison treated it as different from its ordinary spelling. The failed report
`native-storage-check-7a148a6edc414306ab82e9079b2d4493` is preserved unchanged.
The corrected checker adds four regressions (16 native-path tests; 86 total
helper tests) and requires containment plus actual filesystem identity. This
correction and complete preflight still await a fresh Windows run. Preparation
need not be repeated; run only `native-storage.py check` to resume validation.

## What changed

- `native_paths.py` selects real Rust 1.85.1 binaries from the repository-local
  Rustup toolchain directory and prepends that bin directory to child PATH.
  Nested SDK tests therefore compile with a local `env!("CARGO")` value and find
  the local rustc. Global rustup proxy shims are not the selected compiler tools.
- `native-storage.py prepare` copies the existing pinned toolchain and Cargo
  registry/git caches. It does not run rustup, install/download tools, fetch
  dependencies, copy credentials/configuration, or modify source caches.
- Imports are bounded at 250000 files / 32 GiB per tree, reject links/reparse
  points, use new staging directories, compare copied file hashes, and promote
  only a complete copy. Existing destinations are preserved, not overwritten or
  merged. Interrupted staging remains available for diagnosis. Stop other tools
  using the source caches during import; concurrent hostile writers are outside
  this same-user helper's guarantees. Do not delete a remaining storage lock
  until the owning process is confirmed stopped.
- `native-storage.py check` runs 16 path/import regressions; verifies the pinned
  versions, host and sysroot; requires cached locked/offline package metadata;
  builds/runs a tiny nested Cargo probe; checks native temp/Cargo/executable
  locations; checks PowerShell temp placement; and launches/closes Chromium.
  It does not compile Worldwright or contact a database. Existing entry comparison
  permits an unambiguous extended DOS drive spelling only when both forms identify
  the same filesystem entry inside `.worldwright-local`. Device/UNC namespaces,
  ambiguous dot/trailing-space components, streams, outside-root aliases and
  distinct files with identical bytes are not accepted as the expected entry.
  The observed probe values are retained even if a later assertion fails.
- Native build and test commands remain locked/offline. Inherited compiler
  overrides and unreviewed Cargo routing/configuration are rejected. The checked-in
  four aliases are allowed; additional aliases, private registry configuration,
  target/rustc wrappers and ancestor Cargo configs need explicit review rather
  than being silently inherited. No secrets/configuration are automatically copied.
- `r6-native.py` now accepts `.worldwright-local/target`, records selected tool
  identities, and requires a fresh path preflight before application compilation.
  Its existing application test inventories are unchanged. `r6-evidence.py`
  validates the additional preflight binding on repository-local reports;
  historical reports without the local-storage marker retain their old meaning.
- Studio child environments preserve local temp/profile settings. The Windows
  credential ACL PowerShell subprocess now preserves an explicit non-secret
  storage allowlist after `env_clear`, including its module-analysis cache path.
  Preview already forwards TMP/TEMP/TMPDIR. No auth/ACL checks were weakened.
- `r6-check.ps1` delegates to the repository-local signoff wrapper. Other older
  launchers and formal R0 recipes are not migrated by this change; do not use
  their external-directory recipes for this working copy.

## Locations

| Purpose | Under `.worldwright-local/` |
|---|---|
| Cargo registry and Git cache | `cargo-home/` |
| Pinned compiler tools and sysroot | `rustup-home/toolchains/1.85.1-x86_64-pc-windows-msvc/` |
| Application build outputs | `target/` |
| Python/Rust/Node temporary work | `temp/` |
| Chromium binaries | `browsers/` |
| Tool profiles/module caches | `tool-home/` |
| Reports/logs and isolated browser workspaces | `evidence/` |

The old `target/` at the repository root and the former external target cache are
left untouched. No `cargo clean` is run. The new native target starts separately
so reused test executables cannot retain an embedded external Cargo path.
Subsequent development runs reuse `.worldwright-local/target`.

## Prepare, then preflight

Use an ordinary, non-elevated PowerShell window. Close Studio and stop concurrent
Cargo/Rustup work. There is no virtual-environment activation or global environment
change. The subprocess scripts configure their own local paths each time.

```powershell
$python = 'E:\Hold\Worldwright\.worldwright-local\python\Worldwright-R6-Python\Scripts\python.exe'
$storage = 'E:\Hold\Worldwright\scripts\native-storage.py'

& $python -B $storage prepare
if ($LASTEXITCODE -ne 0) { throw 'Preparation failed. Preserve the printed evidence directory and stop here.' }

& $python -B $storage check
if ($LASTEXITCODE -ne 0) { throw 'Preflight failed. Preserve the printed evidence directory; do not start native collection.' }
```

Source caches are discovered from the calling shell's CARGO_HOME/RUSTUP_HOME or
its original home `.cargo`/`.rustup`, BEFORE the child redirects its profiles.
Those old locations are read-only sources. If your cache uses nonstandard paths,
pass `--source-cargo-home` and `--source-rustup-home` with the real locations on
`prepare`. Missing cache/toolchain data causes an explicit failure; no automatic
network fallback occurs. Do not bypass the pin, offline requirement, or path
checks to make preparation pass. Provide the printed report path for diagnosis.

Expected prepare status: `native-storage-prepared-preflight-required`.
Expected check status: `native-storage-preflight-passed-not-acceptance`.
Each command creates a separate new directory under `.worldwright-local/evidence`
and writes `r6-report.json`. Failed reports are useful and must be preserved.

## Native collection after a successful preflight

```powershell
& 'E:\Hold\Worldwright\.worldwright-local\python\Worldwright-R6-Python\Scripts\python.exe' -B `
    'E:\Hold\Worldwright\scripts\collect-worldwright-signoff.py' `
    --repo 'E:\Hold\Worldwright' --native --allow-dirty
```

The native path preflight is repeated inside this collection; an old success
file never bypasses current checks. Helper suites also rerun for the changed
source tuple. Do not edit source, format, or run another build during collection.
Formatting/build/test failures remain failures; this tool does not alter source
or assertions to make them pass. Logs and partial reports identify the exact phase.

Passing native collection remains non-DB prerequisite evidence. Manual Windows
Ctrl+C/same-workspace recovery, live migrations/restore/retention exercises,
independent R1-R6 adoption and the final release decisions are not granted by it.
