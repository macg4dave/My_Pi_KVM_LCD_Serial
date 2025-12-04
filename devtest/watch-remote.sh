#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUN_DEV="$SCRIPT_DIR/run-dev.sh"

if [[ ! -x "$RUN_DEV" ]]; then
    echo "[ERROR] $RUN_DEV must exist and be executable." >&2
    exit 1
fi

command -v cargo >/dev/null 2>&1 || { echo "[ERROR] cargo is required." >&2; exit 1; }
if ! cargo watch --version >/dev/null 2>&1; then
    echo "[ERROR] cargo-watch is required. Install with \"cargo install cargo-watch\"." >&2
    exit 1
fi

printf '[WATCH] Monitoring sources and running %s on change\n' "$RUN_DEV"
cargo watch -q -s "$RUN_DEV"
