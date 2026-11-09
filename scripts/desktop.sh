#!/usr/bin/env bash
# Ubuntu desktop development launcher; Studio remains a loopback browser app.
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
OPEN_BROWSER=1
INSTALLED=0
if [[ "${1:-}" == "--installed" ]]; then INSTALLED=1; shift; fi
if [[ "${1:-}" == "--help" ]]; then
    echo "Usage: bash scripts/desktop.sh [--no-browser] [absolute-workspace-directory]"
    if (( INSTALLED )); then echo "Runs verified bundled binaries; no Cargo or source checkout required."; else echo "Builds native Linux CLI + Studio, then opens the loopback Studio URL."; fi
    exit 0
fi
if [[ "${1:-}" == "--no-browser" ]]; then OPEN_BROWSER=0; shift; fi
if (( $# > 1 )); then echo "Too many arguments; use --help." >&2; exit 2; fi
if [[ "$(uname -s)" != "Linux" ]]; then
    echo "This launcher targets Linux. Windows binaries and dev.ps1 remain supported." >&2
    exit 2
fi
if [[ "${EUID}" == 0 ]]; then echo "Run Studio as your desktop user, not root." >&2; exit 2; fi
TOOLS=(mktemp sed stat mkdir setsid python3)
if (( ! INSTALLED )); then TOOLS+=(cargo rustc); fi
for tool in "${TOOLS[@]}"; do
    command -v "$tool" >/dev/null 2>&1 || { echo "Missing tool: $tool; see docs/LINUX_DESKTOP.md." >&2; exit 1; }
done
DATA_HOME="${XDG_DATA_HOME:-${HOME:?HOME is required}/.local/share}"
WORKSPACE="${1:-$DATA_HOME/worldwright/studio}"
if [[ "$WORKSPACE" != /* ]]; then echo "Workspace must be an absolute Linux path." >&2; exit 2; fi
umask 077
# Validate every existing component before/after mkdir. Do not follow a dangling
# link or chmod/remove someone else's existing data. Shell checks are not an OS
# sandbox; run only under a trusted, user-controlled HOME/XDG hierarchy.
real_components() {
    local part current="" remainder="${1#/}"
    while [[ -n "$remainder" ]]; do
        part="${remainder%%/*}"
        if [[ "$remainder" == */* ]]; then remainder="${remainder#*/}"; else remainder=""; fi
        [[ -z "$part" ]] && continue
        [[ "$part" != "." && "$part" != ".." ]] || { echo "Workspace path must not contain . or .. components." >&2; return 1; }
        current="$current/$part"
        [[ ! -L "$current" ]] || { echo "Workspace path contains a symlink: $current" >&2; return 1; }
        if [[ -e "$current" && ! -d "$current" ]]; then echo "Workspace path is not a directory: $current" >&2; return 1; fi
    done
}
PARENT="$(dirname -- "$WORKSPACE")"
real_components "$WORKSPACE"
if (( $# == 0 )); then
    mkdir -p -- "$PARENT"
    real_components "$WORKSPACE"
    [[ "$(stat -c '%u' -- "$PARENT")" == "$EUID" ]] || { echo "Managed workspace parent must belong to this user." >&2; exit 1; }
    MODE="$(stat -c '%a' -- "$PARENT")"
    (( (8#$MODE & 077) == 0 )) || { echo "Managed workspace parent must be private (chmod 700 it deliberately): $PARENT" >&2; exit 1; }
fi
[[ -d "$PARENT" && -w "$PARENT" && -x "$PARENT" ]] || { echo "Workspace parent must exist and be writable/searchable: $PARENT" >&2; exit 1; }
if [[ -d "$WORKSPACE" ]]; then
    [[ "$(stat -c '%u' -- "$WORKSPACE")" == "$EUID" ]] || { echo "Workspace must belong to this user." >&2; exit 1; }
    MODE="$(stat -c '%a' -- "$WORKSPACE")"
    (( (8#$MODE & 077) == 0 )) || { echo "Workspace must be private; repair permissions explicitly: $WORKSPACE" >&2; exit 1; }
fi
# Native target and explicit output directory avoid stale Windows assets and
# CARGO_BUILD_TARGET/CARGO_TARGET_DIR overrides changing the executable path.
if (( INSTALLED )); then
    python3 "$ROOT/scripts/package-desktop.py" verify --directory "$ROOT"
    BIN_DIR="$ROOT/bin"
else
    HOST="$(rustc -vV | sed -n 's/^host: //p')"
    if [[ "$HOST" != *-linux-* ]]; then echo "rustc is not a native Linux toolchain: $HOST" >&2; exit 1; fi
    cd "$ROOT"
    cargo build --locked --offline --target "$HOST" --target-dir "$ROOT/target" \
        -p worldwright -p worldwright-studio
    BIN_DIR="$ROOT/target/$HOST/debug"
fi
for binary in worldwright worldwright-studio; do
    [[ -x "$BIN_DIR/$binary" ]] || { echo "Native executable missing: $BIN_DIR/$binary" >&2; exit 1; }
done
umask 077
LOG="$(mktemp)"
PID=""
cleanup() {
    local result=$?
    trap - EXIT INT TERM HUP
    if [[ -n "$PID" ]]; then
        # Preview has its own process group but inherits Studio's session. pidfd
        # cleanup tracks that session even if Studio crashes before graceful stop.
        if ! python3 "$ROOT/scripts/cleanup-session.py" "$PID"; then
            echo "Studio session required failed/forced cleanup; retain evidence." >&2
            result=1
        fi
        wait "$PID" 2>/dev/null || true
    fi
    cat "$LOG"
    rm -f -- "$LOG"
    exit "$result"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
trap 'exit 129' HUP
setsid -- "$BIN_DIR/worldwright-studio" serve --workspace "$WORKSPACE" \
    --bind 127.0.0.1:0 --preview-command "$BIN_DIR/worldwright" >"$LOG" 2>&1 &
PID=$!
URL=""
# Consume only the URL printed by our child, not a pre-existing fixed-port app.
for ((i=0; i<300; i++)); do
    if ! kill -0 "$PID" 2>/dev/null; then
        wait "$PID" || exit "$?"
        echo "Studio exited before startup." >&2
        exit 1
    fi
    URL="$(sed -n 's@^Worldwright Studio: \(http://127\.0\.0\.1:[0-9][0-9]*/\)$@\1@p' "$LOG" | head -n 1)"
    [[ -n "$URL" ]] && break
    sleep 0.1
done
[[ -n "$URL" ]] || { echo "Studio startup timed out." >&2; exit 1; }
echo "Studio: $URL"
echo "Workspace: $WORKSPACE"
echo "Keep this launcher running. Ctrl+C stops Studio and its managed preview."
if (( OPEN_BROWSER )); then
    if [[ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]] && command -v xdg-open >/dev/null 2>&1; then
        xdg-open "$URL" >/dev/null 2>&1 &
    else
        echo "Browser not opened: install xdg-utils/use a desktop session, or open the URL manually." >&2
    fi
fi
wait "$PID"
