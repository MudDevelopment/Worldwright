# Linux desktop audit and change evidence

Date: November 8, 2026
Repository inspected/edited through EWW: `E:\Hold\Worldwright`
Primary requested platform: Ubuntu / Linux, preserving Windows features.

## Scope and limits

This is a targeted build/startup/desktop-workflow portability pass, not a
file-by-file audit of every Rust module or a claim of complete platform support.
The user message ended mid-sentence; this pass implements the explicit
Linux-first requirement without inventing a replacement native GUI framework.

Authoritative artifacts inspected include the root Cargo/toolchain/config files,
application directory tree, CLI startup/parser/command dispatch, CLI release,
SDK and project-generation commands, CLI smoke/play/release/maturity tests,
Studio manifest/startup/parser/web/preview code, the beginning of Studio workspace
storage, Studio smoke tests, server lifecycle and selected application/web
sections, development scripts, CI, Dockerfile, editor/ignore settings, README,
DEVELOPMENT, and the relevant DESIGN sections. Partial server/storage reads are
not full-file coverage.

## Findings and source status

| Unit | Status | Evidence / change |
|---|---|---|
| Native host target | Verified source correction | `.cargo/config.toml` forced Windows MSVC on every OS. Removed that override; retained aliases. Docker's existing Linux output path is no longer contradicted by that setting. Container build not executed. |
| Main application boundary | Verified source | `apps/worldwright/src/main.rs` is a CLI; no desktop window framework exists in that application. Studio serves embedded browser assets through Axum. No native GUI was introduced. |
| Ubuntu desktop development launcher | Verified source + synthetic shell tests | Added `scripts/desktop.sh`: native host build, absolute preview executable, XDG draft path, loopback ephemeral URL, browser opening, supervised bounded cleanup. |
| Studio Unix shutdown | Verified source change; real runtime pending | `apps/worldwright-studio/src/web.rs` now selects Unix SIGTERM or Ctrl+C before taking the existing shutdown/preview-stop path. Windows/non-Unix keeps Ctrl+C. |
| Preview-worker parser tests | Verified source correction; Rust tests pending | `apps/worldwright/src/cli.rs` no longer asserts Unix accepts a Windows drive-letter absolute path. Host-native absolute paths preserve the valid case; uppercase digest/token, relative paths, and lifetime bounds remain rejected. The parser's existing absolute-path contract is authoritative; production validation was not weakened. |
| Cross-target regression coverage | Verified source addition; Rust tests pending | `apps/worldwright/tests/smoke.rs` asserts workspace configuration does not contain an unconditional target assignment. |
| Windows preservation | Verified inspected source/configuration; Windows run pending | Retained `scripts/dev.ps1`, Windows CI, SDK `cfg(windows)` metadata protections, preview `.exe` sibling handling, and non-Unix Ctrl+C. No Windows feature was intentionally removed. |
| Ubuntu CI and shell line endings | Verified source/configuration | `.github/workflows/ci.yml` selects `ubuntu-24.04` alongside `windows-latest`, and syntax-checks both shell scripts. `.gitattributes` requests LF for shell/Rust/TOML/YAML text. Hosted CI has not run. |
| Living documentation | Verified edits | README/DESIGN/DEVELOPMENT updated; added `docs/LINUX_DESKTOP.md`. Corrected the development guide's false statement that the MSVC override was already removed, the Studio `help` example to `--help`, and Linux-facing backslash paths. |
| Complete workspace platform audit | Not yet checked | Other Rust modules, transitive native dependency builds, all tests, database operations and production deployments require further verification. |
| Installed/native desktop packaging | Gap found | No native window, desktop-menu integration, `.deb`/AppImage installer, or relocatable source-independent SDK tooling is provided. The launcher is a source-tree development workflow. |
| Linux multi-user preview slots | Gap found; unchanged | CLI preview slots use the common `temp_dir()/worldwright-preview-v1` root. Ownership/permissions and cross-user interference on a shared `/tmp` require a separate hardening review; this pass does not claim they are solved. |

## Observed execution evidence

EWW exposes filesystem operations but no shell/command-execution operation.
The separate Linux sandbox returned no installed `rustc`, `cargo`, or
`shellcheck` executable when checked. No remote Cargo process or real
Worldwright desktop process was executed.

The candidate desktop launcher was executed in the Linux sandbox under
synthetic Cargo/rustc/Studio/browser fixtures. This verifies shell orchestration,
not Rust compilation, actual Studio signal handling, preview cleanup, or browser
rendering. The remote launcher was re-read after edits to inspect its final
content against that candidate.

Observed successful commands:

```bash
bash -n /tmp/desktop.sh
python /tmp/test_desktop_launcher.py
```

Observed test output:

```text
PASS: native target/output override, spaces, XDG default, ephemeral URL, no-browser, SIGTERM cleanup
PASS: custom absolute workspace and Wayland browser launch
PASS: invalid path/usage, help, startup failure and build failure exit codes
```

The Wayland case supplied WAYLAND_DISPLAY and a synthetic xdg-open; it was not a
real Wayland compositor/browser session. The SIGTERM case verified the launcher
terminates/reaps a synthetic child, not the changed Tokio signal future.
Temporary fixture tools and logs were not copied into the repository. Existing
Cargo.lock, dependency versions and the pinned Rust toolchain were not changed.

## Required real acceptance evidence

Run on Ubuntu 24.04 with approved Rust/dependency artifacts:

```bash
bash scripts/dev.sh bootstrap
bash -n scripts/dev.sh scripts/desktop.sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked --offline
cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
cargo test --workspace --locked --offline
cargo build --workspace --locked --offline
bash scripts/desktop.sh
```

No fmt/check/clippy/test/build success is claimed here. Apply/review any rustfmt
changes required by the real pinned formatter before merging. Confirm the
actual editor's import/save/package/preview flow; stop it using Ctrl+C and
SIGTERM; verify the preview supervisor/worker exit; test source/workspace paths
with spaces, X11/Wayland as available, and manual URL opening. Exercise local
CLI play separately. Re-run retained Windows CI and the Docker build. Database,
recovery, accessibility, hosted CI and production evidence remain open.

No commit, branch, PR, lockfile regeneration, dependency upgrade, persisted
configuration migration, database change or license decision was performed.
