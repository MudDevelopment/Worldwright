# Ubuntu / Linux desktop development

## Platform and application contract

Ubuntu 24.04 LTS is the primary development and CI baseline. Windows remains
supported by native Cargo host selection, `scripts/dev.ps1`, Windows CI, and
existing Windows-specific filesystem handling. These are implementation and
configuration decisions, not proof of a successful Ubuntu/Windows run.

- `worldwright`: native developer CLI; interactive local play runs in a terminal.
- `worldwright-studio`: native Rust process serving the existing browser editor.
- `worldwright-server`: separate authoritative game service, not automatically
  started by the Studio launcher.
- Studio's browser desktop surface works through the user's browser under X11
  or Wayland; no GTK, Qt, egui, or Windows WebView runtime is introduced.
- This pass does not provide a native GUI window, `.deb`/AppImage distribution,
  desktop-menu registration, or an installed application independent of source.

## Ubuntu prerequisites

Install development prerequisites through the approved package process:

```bash
sudo apt update
sudo apt install --yes build-essential pkg-config ca-certificates xdg-utils
```

Use an approved rustup installation and the repository's pinned Rust 1.85.1,
rustfmt, and Clippy. Do not assume Ubuntu's distribution Rust package meets the
edition/toolchain requirements. `xdg-utils` is needed only for automatic browser
opening; a browser must also be installed. Current local CLI/Studio use does not
require PostgreSQL or native GUI development libraries.

From the repository root:

```bash
bash scripts/dev.sh bootstrap
bash scripts/dev.sh doctor
bash scripts/dev.sh check
bash scripts/desktop.sh
```

Bootstrap installs/fetches the pinned toolchain and locked dependencies and may
need network access. The check and desktop build use locked, offline resolution.
Run Studio as your ordinary desktop user, not with sudo. Shell scripts are
invoked through Bash, so executable Git mode is not a prerequisite.

## Desktop launcher

```bash
bash scripts/desktop.sh --help
bash scripts/desktop.sh
bash scripts/desktop.sh --no-browser
bash scripts/desktop.sh --no-browser "$HOME/Worldwright Drafts"
```

The optional workspace argument must be an absolute Linux path. The default is
`$XDG_DATA_HOME/worldwright/studio`; when XDG_DATA_HOME is unset or empty, it is
`$HOME/.local/share/worldwright/studio`. Existing explicit Studio CLI paths are
unchanged. The launcher resolves source/build paths relative to its own script,
so it can be invoked from another working directory and handles path spaces.

The launcher:

1. Confirms a native Linux Rust host and builds `worldwright` and
   `worldwright-studio` with `--target <rustc-host>` and an explicit repository
   target directory. Ambient cross-target/output overrides cannot make it run
   a stale Windows executable.
2. Starts Studio on `127.0.0.1:0`, which allocates an available loopback port.
3. Passes the exact absolute native CLI executable as the preview command.
4. Reads the URL printed by its own child and opens it with `xdg-open` when
   DISPLAY or WAYLAND_DISPLAY identifies a desktop session. Otherwise it prints
   the URL for manual opening. `--no-browser` suppresses opening only; it does not
   remove the browser UI or change Studio into a separate GUI mode.
5. Keeps Studio under launcher supervision. Keep the terminal/launcher running;
   closing the browser does not terminate Studio. Ctrl+C, SIGTERM, or terminal
   hangup triggers cleanup. The private temporary startup log is shown at exit
   and removed. Startup is bounded to approximately 30 seconds.

Studio now handles Unix SIGTERM and Ctrl+C via its graceful shutdown and existing
preview-stop path. Windows retains Ctrl+C behavior. If launcher cleanup exceeds
10 seconds it forcibly terminates Studio; graceful cleanup of all descendant
processes cannot be guaranteed in that fallback.

Studio still enforces its exact Host and Origin checks and stays loopback-only.
The desktop launcher does not provide publication credentials, start the game
server, bypass authorization, or change draft/workspace formats.

## Native CLI and explicit Windows support

```bash
cargo run -p worldwright --locked --offline -- --help
cargo run -p worldwright --locked --offline -- play examples/two-room/world.json
```

Cargo defaults to its host; `.cargo/config.toml` does not force a platform.
Explicit cross-compilation requires the target standard library AND a compatible
linker/toolchain; a Windows MSVC target is not made runnable on Ubuntu merely by
installing it with rustup. Prefer the retained native Windows CI/job for Windows
verification. The desktop launcher itself intentionally targets Linux only.

## Acceptance gates still required

```bash
bash -n scripts/dev.sh scripts/desktop.sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked --offline
cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
cargo test --workspace --locked --offline
cargo build --workspace --locked --offline
```

Run the actual Studio editor in Ubuntu, test import/edit/save/package/preview,
stop Studio with both Ctrl+C and SIGTERM, and confirm its preview exits. Test a
path with spaces, an existing workspace, X11 and Wayland as available, and manual
browser operation with `--no-browser`. Verify terminal local play separately.
Run Windows CI to establish preservation, and the Docker build to establish
container behavior. Live PostgreSQL/production deployment are separate gates.

See [audit and verification limits](verification/linux-desktop-audit.md). No
successful real Rust build, Ubuntu browser session, hosted CI, or release
packaging result is claimed by this source/configuration change.
