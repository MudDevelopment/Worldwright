#!/bin/sh
# Required gate: a missing Helm/Python/dependency is a failure, never a PASS.
set -eu
command -v python3 >/dev/null 2>&1 || { echo 'ERROR: Python 3 is required for chart acceptance.' >&2; exit 1; }
chart=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
exec python3 "$chart/tests/verify.py" --chart "$chart"
