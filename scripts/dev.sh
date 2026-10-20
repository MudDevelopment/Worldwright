#!/usr/bin/env bash
# Run from any directory; no production environment or credentials are used.
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
TASK="${1:-help}"
if (( $# > 1 )); then echo "Expected one task; use help." >&2; exit 2; fi
need() { command -v "$1" >/dev/null 2>&1 || { echo "Missing tool: $1" >&2; exit 1; }; }
locked() {
    need cargo
    need rustc
    if [[ ! -f Cargo.lock ]]; then echo "Run bootstrap first, then commit Cargo.lock." >&2; exit 1; fi
}
docs() {
    RUSTDOCFLAGS="${RUSTDOCFLAGS:-} -D warnings" cargo doc --workspace --no-deps --locked --offline
}
case "$TASK" in
    help)
        echo "Usage: bash scripts/dev.sh <bootstrap|doctor|check|fmt|test|docs>"
        echo "bootstrap may install the pinned toolchain and fetch locked dependencies. fmt changes source formatting."
        ;;
    bootstrap)
        CHANNEL="$(sed -n 's/^channel = "\([^"]*\)"$/\1/p' rust-toolchain.toml)"
        [[ -n "$CHANNEL" ]] || { echo "Cannot read pinned toolchain." >&2; exit 1; }
        if command -v rustup >/dev/null 2>&1; then
            rustup toolchain install "$CHANNEL" --profile minimal --component rustfmt --component clippy
        else
            echo "rustup is absent; using installed tools. Verify their versions against rust-toolchain.toml."
        fi
        need cargo
        need rustc
        rustc --version
        cargo --version
        [[ -f Cargo.lock ]] || { echo "Cargo.lock is missing; restore the committed lockfile or regenerate it deliberately with cargo generate-lockfile." >&2; exit 1; }
        cargo fetch --locked
        cargo metadata --format-version 1 --no-deps --locked --offline >/dev/null
        echo "Bootstrap complete. Dependencies fetched from Cargo.lock; check runs offline."
        ;;
    doctor)
        need cargo
        need rustc
        rustc --version
        cargo --version
        echo "Workspace: $ROOT"
        echo "Toolchain pin: $(sed -n '/^channel = /p' rust-toolchain.toml)"
        ;;
    fmt)
        need cargo
        cargo fmt --all
        ;;
    test)
        locked
        cargo test --workspace --locked --offline
        ;;
    docs)
        locked
        docs
        ;;
    check)
        locked
        rustc --version
        cargo --version
        cargo fmt --all -- --check
        cargo check --workspace --all-targets --locked --offline
        cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
        cargo test --workspace --locked --offline
        cargo build --workspace --locked --offline
        docs
        ;;
    *) echo "Unknown task: $TASK" >&2; exit 2 ;;
esac
