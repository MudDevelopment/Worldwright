# Local Development and CI

## Prerequisites

- The repository's `rust-toolchain.toml` pins Rust **1.85.1**, rustfmt, and Clippy.
- Use an approved Rust installation, preferably rustup. Registry dependencies include Serde/JSON, Tokio, Axum, Argon2, getrandom, futures, rpassword, and their locked transitive dependencies. Bootstrap needs registry access or an approved prepopulated cache.
- Bash is supported for the Unix workflow; the parallel Windows workflow targets PowerShell 7.
- No database, container runtime, or running service is required for checks. Server tests create ephemeral loopback listeners and use private synthetic account fixtures; no external service or database is needed.

The package includes a Cargo-generated `Cargo.lock`. Keep it committed. The normal check workflow uses `--locked --offline`; it does not update dependencies silently.

## Commands

From the workspace root:

```bash
bash scripts/dev.sh bootstrap
bash scripts/dev.sh doctor
bash scripts/dev.sh check
```

On Windows:

```powershell
./scripts/dev.ps1 bootstrap
./scripts/dev.ps1 doctor
./scripts/dev.ps1 check
```

Both scripts resolve the repository relative to their own location, so calling either script by its path from another directory works. They accept exactly one task and stop on failed native commands.

| Task | Behavior |
|---|---|
| `help` | Show supported tasks; also the no-argument default |
| `bootstrap` | Install the pin through rustup when available; require the committed lockfile; fetch dependencies with `cargo fetch --locked`; verify locked metadata |
| `doctor` | Print actual Rust/Cargo versions, workspace path, and declared toolchain pin |
| `check` | Formatting check, all-target compilation, Clippy, tests/doctests, debug build, and API documentation |
| `fmt` | Apply rustfmt changes; review the source diff afterwards |
| `test` | Run workspace unit, integration, and documentation tests |
| `docs` | Build workspace API documentation with warnings denied |

Bootstrap can require network access for toolchain installation. If rustup is absent, bootstrap uses installed native tools and explicitly asks you to verify their versions. Neither script silently upgrades a manually installed toolchain. Overrides such as `RUSTUP_TOOLCHAIN` can also affect selection: check `doctor` output rather than inferring the compiler from a filename.

Content loading now brings registry dependencies. Bootstrap runs `cargo fetch --locked` and fails if the committed lockfile is absent; dependency changes require deliberate `cargo generate-lockfile`, review, and commit. Air-gapped builders need an approved prepopulated cache or vendor source. Checks remain `--locked --offline`.

Cargo now defaults to the native host target; the old unconditional Windows MSVC target has been removed. Cross-compilation is explicit using `--target` and requires that target's approved standard library and linker. Linux CI must not silently target Windows.

## Check Sequence

The full workflow runs:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked --offline
cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
cargo test --workspace --locked --offline
cargo build --workspace --locked --offline
cargo doc --workspace --no-deps --locked --offline
```

For the final command, the scripts add `-D warnings` to `RUSTDOCFLAGS` and preserve the caller's original value. This is a debug/development build; release performance and production deployment are not tested here.

Local play and focused tests (after bootstrap):

```bash
cargo run -p worldwright --locked --offline -- play examples/two-room/world.json
cargo test -p worldwright --test play --locked --offline
```

See [Local Play](LOCAL_PLAY.md) for commands, EOF handling, input limits, and local-only scope. See [SERVER.md](SERVER.md) and [PROTOCOL_V1.md](PROTOCOL_V1.md) for account, server, and wire setup. Other useful focused commands:

```bash
cargo test -p worldwright-core --locked --offline
cargo test -p worldwright-testkit --test runtime --locked --offline
cargo test -p worldwright-testkit --test runtime --locked --offline two_room_look_move_say_walkthrough -- --nocapture
cargo test -p worldwright-testkit --test rules --locked --offline
cargo run -p worldwright-server --locked --offline -- status
cargo test -p worldwright-server --locked --offline
```

The server status command reports protocol/browser implementation and process-local state; it does not start a server or verify service readiness. Use [SERVER.md](SERVER.md) for the explicit local `serve` workflow.

## CI Configuration

[ci.yml](../.github/workflows/ci.yml) runs on pushes, pull requests, and manual dispatch:

- Ubuntu and Windows jobs call the same Bash/PowerShell workflows used locally.
- Rustup installs the declared toolchain on the hosted runners.
- Checkout is pinned to the verified upstream v4 commit `11d5960a326750d5838078e36cf38b85af677262`.
- Workflow token permissions are read-only for repository contents; checkout does not persist credentials.
- Jobs have a 20-minute timeout and per-ref cancellation of superseded runs.
- A final check detects tracked Cargo.lock modifications.
- No publishing, deployment, production credentials, privileged containers, or `pull_request_target` execution is configured.

**Configuration is supplied; CI has not been run on a remote repository.** Add the workflow to the actual repository and inspect both jobs before treating it as an enforced gate. Configure branch protection/rulesets separately; a YAML file does not enforce merge policy by itself.

For GitHub Enterprise or restricted organizational hosting, confirm approved runners, toolchain downloads, and availability/mirroring of the pinned checkout action. Change runner labels through review where required. Never run arbitrary pull-request code on a privileged persistent self-hosted runner with production secrets.

## Local Evidence and Limits

The content increment was checked using temporary **Rust 1.85.1 Linux** tools from third-party npm repackages, with install scripts disabled. crates.io DNS was unavailable; verification used matching upstream GitHub source tags in a temporary source replacement, not registry tarballs. The lockfile retains normal registry identities/checksums. No sandbox-specific configuration, source replacements, toolchain, or dependency snapshots ship in the archive; captured logs retain their original paths. Re-run bootstrap/check with approved Rust and registry artifacts before release acceptance; Windows/hosted CI are not claimed.

See [VERIFICATION.md](VERIFICATION.md) for exact results. Use the approved pinned toolchain and both remote CI jobs for the final acceptance gate.
