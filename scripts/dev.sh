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
doctor() {
    need cargo; need rustc
    echo "Workspace: $ROOT"
    echo "Cargo executable: $(command -v cargo)"
    echo "Rustc executable: $(command -v rustc)"
    echo "Toolchain pin: $(sed -n '/^channel = /p' rust-toolchain.toml)"
    rustc -vV
    cargo --version
    local sysroot
    sysroot="$(rustc --print sysroot)"
    echo "Sysroot: $sysroot"
    if command -v rustup >/dev/null 2>&1; then
        rustup show active-toolchain
        rustup target list --installed
    fi
    # Presence only: flags/wrappers/paths can contain sensitive local values.
    for name in CARGO_BUILD_TARGET CARGO_TARGET_DIR RUSTUP_TOOLCHAIN RUSTC RUSTDOC RUSTFLAGS CARGO_ENCODED_RUSTFLAGS RUSTDOCFLAGS RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER RUSTC_BOOTSTRAP; do
        if [[ -v "$name" ]]; then echo "Override $name: SET (value redacted)"; else echo "Override $name: unset"; fi
    done
    local directory="$ROOT"
    while :; do
        for file in "$directory/.cargo/config" "$directory/.cargo/config.toml"; do
            [[ ! -f "$file" ]] || echo "Cargo configuration present: $file"
        done
        [[ "$directory" != / ]] || break
        directory="$(dirname -- "$directory")"
    done
    for file in "${CARGO_HOME:-${HOME:-}/.cargo}/config" "${CARGO_HOME:-${HOME:-}/.cargo}/config.toml"; do
        [[ ! -f "$file" ]] || echo "User Cargo configuration present: $file"
    done
}
docs() {
    RUSTDOCFLAGS="${RUSTDOCFLAGS:-} -D warnings" cargo doc --workspace --no-deps --locked --offline
}
case "$TASK" in
    help)
        echo "Usage: bash scripts/dev.sh <bootstrap|doctor|check|check-native|fmt|test|docs>"
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
    doctor) doctor ;;
    check-native)
        locked
        doctor
        PIN="$(sed -n 's/^channel = "\([^"]*\)"$/\1/p' rust-toolchain.toml)"
        [[ -n "$PIN" ]] || { echo "Cannot read pinned toolchain." >&2; exit 1; }
        for program in rustc cargo; do
            version="$($program --version)"
            version="${version%%$'\n'*}"
            [[ "$version" == "$program $PIN" || "$version" == "$program $PIN "* ]] || { echo "$program must use pinned $PIN; run bootstrap/select the approved toolchain." >&2; exit 1; }
        done
        HOST="$(rustc -vV | sed -n 's/^host: //p')"
        [[ -n "$HOST" ]] || { echo "Cannot determine native Rust host." >&2; exit 1; }
        umask 077
        TARGET="$(mktemp -d "${TMPDIR:-/tmp}/worldwright-native-check.XXXXXXXX")"
        echo "Fresh native build directory (retained for diagnosis): $TARGET"
        cargo fmt --all -- --check
        cargo check --workspace --all-targets --locked --offline --target "$HOST" --target-dir "$TARGET"
        cargo clippy --workspace --all-targets --all-features --locked --offline --target "$HOST" --target-dir "$TARGET" -- -D warnings
        cargo test --workspace --locked --offline --target "$HOST" --target-dir "$TARGET"
        cargo build --workspace --locked --offline --target "$HOST" --target-dir "$TARGET"
        RUSTDOCFLAGS="${RUSTDOCFLAGS:-} -D warnings" cargo doc --workspace --no-deps --locked --offline --target "$HOST" --target-dir "$TARGET"
        echo "PASS: native checks completed; $TARGET"
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
        doctor
        cargo fmt --all -- --check
        cargo check --workspace --all-targets --locked --offline
        cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
        cargo test --workspace --locked --offline
        cargo build --workspace --locked --offline
        docs
        ;;
    *) echo "Unknown task: $TASK" >&2; exit 2 ;;
esac
