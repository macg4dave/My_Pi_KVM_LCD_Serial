#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONF_FILE="$SCRIPT_DIR/dev.conf"

if [[ -f "$CONF_FILE" ]]; then
    # shellcheck disable=SC1090
    source "$CONF_FILE"
fi

command -v cargo >/dev/null 2>&1 || { echo "[ERROR] cargo is required." >&2; exit 1; }
if ! cargo watch --version >/dev/null 2>&1; then
    echo "[ERROR] cargo-watch is required. Install with \"cargo install cargo-watch\"." >&2
    exit 1
fi

COMMON_ARGS=${COMMON_ARGS:---demo}
LOCAL_ARGS=${LOCAL_ARGS:-}

build_arg_string() {
    local args="$1"; shift
    for chunk in "$@"; do
        [[ -n "$chunk" ]] || continue
        args+=" $chunk"
    done
    printf '%s' "$args"
}

RUN_ARGS=$(build_arg_string "$COMMON_ARGS" "$LOCAL_ARGS")

printf '[WATCH] Running cargo watch with args: %s\n' "$RUN_ARGS"
cargo watch -q -s "cargo run -- $RUN_ARGS"
