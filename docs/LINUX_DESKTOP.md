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
- The private R1 acceptance package provides bundled installed CLI/Studio plus
  runtime launcher/verifier/cleanup, independent of the build checkout for
  supported edit/package/preview use. It is not a native GUI, .deb/AppImage,
  desktop-menu installer, licensed release or approved distribution.

## Ubuntu prerequisites

Install development prerequisites through the approved package process:

```bash
sudo apt update
sudo apt install --yes build-essential pkg-config ca-certificates xdg-utils python3 util-linux
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

1. In development mode, confirms a native Linux Rust host and builds `worldwright` and
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

Studio handles Unix SIGTERM, Ctrl+C and SIGHUP through shared graceful shutdown
and preview-stop. Windows retains Ctrl+C behavior. The launcher starts a new
Studio session; preview groups inherit it. Pidfds pin process lifetimes while
cleaning that owned session, including after a Studio crash. After the 10-second
grace period it forces remaining processes to exit and records cleanup failure,
not a graceful pass. Native code escaping the session is outside this hygiene
mechanism; it is not an OS sandbox. Browser processes are not killed.

Studio still enforces its exact Host and Origin checks and stays loopback-only.
The desktop launcher does not provide publication credentials, start the game
server, bypass authorization, or change draft/workspace formats.

## Installed private runtime

Use the exact package from a passing same-source Ubuntu R0 run; external archive
identity and rights/owner review still matter. See `R0_R1_ACCEPTANCE.md` for build,
extraction, manifest verification and all real/manual gates. From a private owned
package root (0700), outside the checkout:

```bash
python3 "$PACKAGE/scripts/package-desktop.py" verify --directory "$PACKAGE"
"$PACKAGE/bin/worldwright-desktop" --no-browser
```

The package's own verifier works when the checkout is unavailable. The wrapper chooses installed mode and never builds; it requires Python 3.11+,
Bash/coreutils, setsid/pidfd support and the compatible runtime libraries recorded
by packaging. No compiler/Rust cache is needed for supported Studio runtime
operations. Building game source still needs a matching development SDK/toolchain.
Existing workspace parents AND roots must be owned/private; permission failures
require deliberate remediation, not silent chmod. Auto-opening requires xdg-open
and an actual X11/Wayland session; otherwise use the printed URL manually.

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

## Interrupted Studio workspace recovery

Updated Studio creates `.worldwright-studio-session` before binding its listener
and refuses a second updated serve process for the same workspace. Normal clean
return removes its matching sentinel. A crash, panic or uncertain preview cleanup
may retain it. Launcher session cleanup does not automatically reclaim the file.
Confirm all Studio/preview writers have stopped, preserve a private copy including
journals, then follow [R5 recovery](R5_ACCEPTANCE.md) before restarting. Never
remove a sentinel from a possibly live writer or delete `.worldwright-studio`.
The guard is cooperative, not a lock against older binaries or hostile code.

The R1 automation's crash case uses a fresh XDG workspace afterward, so it does
not establish crashed-workspace recovery. The separate native/manual session and
journal matrix in [R5_CHECK_PLAN.md](R5_CHECK_PLAN.md) remains required.

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

## P0 update — November 9, 2026

The default missing-parent defect is corrected in source. The launcher now
prepares the private owned XDG parent with umask077, rejects symlink/dot-dot/
non-directory paths and checks custom-parent access. Existing folders are not
silently chmodded. A non-private managed parent fails with explicit remediation.
Eight synthetic contracts passed in the earlier P0 increment; real fresh-HOME/XDG tests are CI-wired but
unexecuted here. See P0_ACCEPTANCE.md for current commands and policy; historical
statements above about the defect being unresolved are superseded by this note,
not by a claim of complete desktop verification.
