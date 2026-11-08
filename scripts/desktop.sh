#!/usr/bin/env bash
# Ubuntu desktop development launcher; Studio remains a loopback browser app.
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
OPEN_BROWSER=1
if [[ "${1:-}" == "--help" ]]; then
    echo "Usage: bash scripts/desktop.sh [--no-browser] [absolute-workspace-directory]"
    echo "Builds native Linux CLI + Studio, then opens the loopback Studio URL."
    exit 0
fi
if [[ "${1:-}" == "--no-browser" ]]; then OPEN_BROWSER=0; shift; fi
if (( $# > 1 )); then echo "Too many arguments; use --help." >&2; exit 2; fi
if [[ "$(uname -s)" != "Linux" ]]; then
    echo "This launcher targets Linux. Windows binaries and dev.ps1 remain supported." >&2
    exit 2
fi
if [[ "${EUID}" == 0 ]]; then echo "Run Studio as your desktop user, not root." >&2; exit 2; fi
for tool in cargo rustc mktemp sed; do
    command -v "$tool" >/dev/null 2>&1 || { echo "Missing tool: $tool; see docs/LINUX_DESKTOP.md." >&2; exit 1; }
done
DATA_HOME="${XDG_DATA_HOME:-${HOME:?HOME is required}/.local/share}"
WORKSPACE="${1:-$DATA_HOME/worldwright/studio}"
if [[ "$WORKSPACE" != /* ]]; then echo "Workspace must be an absolute Linux path." >&2; exit 2; fi
# Native target and explicit output directory avoid stale Windows assets and
# CARGO_BUILD_TARGET/CARGO_TARGET_DIR overrides changing the executable path.
HOST="$(rustc -vV | sed -n 's/^host: //p')"
if [[ "$HOST" != *-linux-* ]]; then echo "rustc is not a native Linux toolchain: $HOST" >&2; exit 1; fi
cd "$ROOT"
cargo build --locked --offline --target "$HOST" --target-dir "$ROOT/target" \
    -p worldwright -p worldwright-studio
BIN_DIR="$ROOT/target/$HOST/debug"
for binary in worldwright worldwright-studio; do
    [[ -x "$BIN_DIR/$binary" ]] || { echo "Native executable missing: $BIN_DIR/$binary" >&2; exit 1; }
done
umask 077
LOG="$(mktemp)"
PID=""
cleanup() {
    local result=$?
    trap - EXIT INT TERM HUP
    if [[ -n "$PID" ]] && kill -0 "$PID" 2>/dev/null; then
        kill -TERM "$PID" 2>/dev/null || true
        for ((i=0; i<100; i++)); do
            kill -0 "$PID" 2>/dev/null || break
            sleep 0.1
        done
        if kill -0 "$PID" 2>/dev/null; then
            echo "Studio did not stop within 10 seconds; forcing termination." >&2
            kill -KILL "$PID" 2>/dev/null || true
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
"$BIN_DIR/worldwright-studio" serve --workspace "$WORKSPACE" \
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
