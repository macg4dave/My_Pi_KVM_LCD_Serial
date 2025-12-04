#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONF_FILE="$SCRIPT_DIR/dev.conf"

if [[ -f "$CONF_FILE" ]]; then
    # shellcheck disable=SC1090
    source "$CONF_FILE"
else
    echo "[ERROR] Missing dev.conf. Copy dev.conf.example and customize it." >&2
    exit 1
fi

command -v tmux >/dev/null 2>&1 || { echo "[ERROR] tmux is required." >&2; exit 1; }
command -v ssh >/dev/null 2>&1 || { echo "[ERROR] ssh is required." >&2; exit 1; }
command -v scp >/dev/null 2>&1 || { echo "[ERROR] scp is required." >&2; exit 1; }

: "${PI_HOST:?PI_HOST must be set in dev.conf}"
: "${PI_BIN:?PI_BIN must be set in dev.conf}"

LOCAL_BIN=${LOCAL_BIN:-target/debug/lifelinetty}
COMMON_ARGS=${COMMON_ARGS:---demo}
REMOTE_ARGS=${REMOTE_ARGS:-}
LOCAL_ARGS=${LOCAL_ARGS:-}
BUILD_CMD=${BUILD_CMD:-cargo build}
TMUX_SESSION=${TMUX_SESSION:-lifeline-dev}
ENABLE_LOG_PANE=${ENABLE_LOG_PANE:-false}
LOG_WATCH_CMD=${LOG_WATCH_CMD:-watch -n 0.5 ls -lh /run/serial_lcd_cache}
PKILL_PATTERN=${PKILL_PATTERN:-lifelinetty}

printf '[BUILD] Using "%s"\n' "$BUILD_CMD"
bash -c "$BUILD_CMD"

if [[ ! -x "$LOCAL_BIN" ]]; then
    echo "[WARN] Local binary $LOCAL_BIN is missing or not executable." >&2
fi

remote_dir=$(dirname "$PI_BIN")

echo "[DEPLOY] Ensuring remote directory $remote_dir"
ssh "$PI_HOST" "mkdir -p '$remote_dir'"

echo "[DEPLOY] Copying $LOCAL_BIN to $PI_HOST:$PI_BIN"
scp "$LOCAL_BIN" "$PI_HOST:$PI_BIN"
ssh "$PI_HOST" "chmod +x '$PI_BIN'"

echo "[REMOTE] Killing stale processes matching '$PKILL_PATTERN'"
ssh "$PI_HOST" "pkill -f '$PKILL_PATTERN' || true"

build_cmd_string() {
    local cmd="$1"; shift
    for chunk in "$@"; do
        [[ -n "$chunk" ]] || continue
        cmd+=" $chunk"
    done
    printf '%s' "$cmd"
}

REMOTE_CMD=$(build_cmd_string "$PI_BIN" "$COMMON_ARGS" "$REMOTE_ARGS")
LOCAL_CMD=$(build_cmd_string "$LOCAL_BIN" "$COMMON_ARGS" "$LOCAL_ARGS")
LOG_CMD="$LOG_WATCH_CMD"

if tmux has-session -t "$TMUX_SESSION" 2>/dev/null; then
    echo "[TMUX] Killing existing session $TMUX_SESSION"
    tmux kill-session -t "$TMUX_SESSION"
fi

echo "[TMUX] Launching new session $TMUX_SESSION"
tmux new-session -d -s "$TMUX_SESSION"
tmux rename-window -t "$TMUX_SESSION:0" 'REMOTE'

printf -v remote_launch 'ssh %s %q' "$PI_HOST" "$REMOTE_CMD"
tmux send-keys -t "$TMUX_SESSION:0" "$remote_launch" C-m

if [[ "$ENABLE_LOG_PANE" == "true" ]]; then
    printf -v log_launch 'ssh %s %q' "$PI_HOST" "$LOG_CMD"
    tmux split-window -t "$TMUX_SESSION:0" -v "$log_launch"
fi

TMUX_LOCAL_WINDOW="$TMUX_SESSION:1"
tmux new-window -t "$TMUX_SESSION" -n 'LOCAL'
tmux send-keys -t "$TMUX_LOCAL_WINDOW" "$LOCAL_CMD" C-m

tmux select-window -t "$TMUX_SESSION:0"
tmux attach-session -t "$TMUX_SESSION"
