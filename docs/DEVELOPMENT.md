# Local Development and CI

## Prerequisites

- The repository's `rust-toolchain.toml` pins Rust **1.85.1**, rustfmt, and Clippy.
- Use an approved Rust installation, preferably rustup. Package dependencies are currently local workspace paths only.
- Bash is supported for the Unix workflow; the parallel Windows workflow targets PowerShell 7.
- No database, container runtime, credentials, environment file, or running service is required.

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
| `bootstrap` | Install the pin through rustup when available; verify tools and locked metadata; generate a lockfile only if absent |
| `doctor` | Print actual Rust/Cargo versions, workspace path, and declared toolchain pin |
| `check` | Formatting check, all-target compilation, Clippy, tests/doctests, debug build, and API documentation |
| `fmt` | Apply rustfmt changes; review the source diff afterwards |
| `test` | Run workspace unit, integration, and documentation tests |
| `docs` | Build workspace API documentation with warnings denied |

Bootstrap can require network access for toolchain installation. If rustup is absent, bootstrap uses installed native tools and explicitly asks you to verify their versions. Neither script silently upgrades a manually installed toolchain. Overrides such as `RUSTUP_TOOLCHAIN` can also affect selection: check `doctor` output rather than inferring the compiler from a filename.

The project currently has no registry dependencies. When introducing one, update the dependency policy and CI bootstrap to populate the approved cache before offline checks; do not casually remove `--locked` or download arbitrary build scripts.

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

Useful focused commands:

```bash
cargo test -p worldwright-core --locked --offline
cargo test -p worldwright-testkit --test runtime --locked --offline
cargo test -p worldwright-testkit --test runtime --locked --offline two_room_look_move_say_walkthrough -- --nocapture
cargo test -p worldwright-testkit --test rules --locked --offline
cargo run -p worldwright-server --locked --offline -- status
```

The last command reports scaffold status. It does not start a server or verify service readiness. Default application startup still fails explicitly.

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

The generation sandbox used a temporary **Rust 1.85.0 Linux** toolchain obtained from third-party npm repackages. That permitted real local compilation and tests, but is not proof of execution with pinned Rust 1.85.1, Windows, or hosted CI. The temporary toolchain is not a project dependency and is not included in the delivered archive.

See [VERIFICATION.md](VERIFICATION.md) for exact results. Use the approved pinned toolchain and both remote CI jobs for the final acceptance gate.
