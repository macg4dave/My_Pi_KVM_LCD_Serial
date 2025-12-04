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

command -v ssh >/dev/null 2>&1 || { echo "[ERROR] ssh is required." >&2; exit 1; }
command -v scp >/dev/null 2>&1 || { echo "[ERROR] scp is required." >&2; exit 1; }

: "${PI_HOST:?PI_HOST must be set in dev.conf (hostname, e.g. 192.168.20.106)}"
: "${PI_BIN:?PI_BIN must be set in dev.conf}"
PI_USER=${PI_USER:-pi}

TERMINAL_CMD=${TERMINAL_CMD:-gnome-terminal}


LOCAL_BIN=${LOCAL_BIN:-target/debug/lifelinetty}
COMMON_ARGS=${COMMON_ARGS:---demo}
REMOTE_ARGS=${REMOTE_ARGS:-}
LOCAL_ARGS=${LOCAL_ARGS:-}
BUILD_CMD=${BUILD_CMD:-cargo build}
ENABLE_LOG_PANE=${ENABLE_LOG_PANE:-false}
LOG_WATCH_CMD=${LOG_WATCH_CMD:-watch -n 0.5 ls -lh /run/serial_lcd_cache}
PKILL_PATTERN=${PKILL_PATTERN:-lifelinetty}

printf '[BUILD] Using "%s"\n' "$BUILD_CMD"
bash -c "$BUILD_CMD"

if [[ ! -x "$LOCAL_BIN" ]]; then
    echo "[WARN] Local binary $LOCAL_BIN is missing or not executable." >&2
fi

remote_dir=$(dirname "$PI_BIN")
remote_target="$PI_USER@$PI_HOST"

ensure_remote_dir() {
    if ! ssh "$remote_target" "mkdir -p '$remote_dir'"; then
        cat <<'EOF' >&2
[ERROR] Could not create $remote_dir on $remote_target.
Make sure the path exists and is writable by $PI_USER (for example,
  ssh $remote_target sudo mkdir -p '$remote_dir' && \
    ssh $remote_target sudo chown $PI_USER '$remote_dir'
)
Alternately, point PI_BIN at a directory the SSH user already owns.
EOF
        exit 1
    fi
}

echo "[DEPLOY] Ensuring remote directory $remote_dir"
ensure_remote_dir

echo "[DEPLOY] Copying $LOCAL_BIN to $remote_target:$PI_BIN"
scp "$LOCAL_BIN" "$remote_target:$PI_BIN"
ssh "$remote_target" "chmod +x '$PI_BIN'"

echo "[REMOTE] Killing stale processes matching '$PKILL_PATTERN'"
ssh "$remote_target" "pkill -f '$PKILL_PATTERN' || true"

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

terminal_available=true
if ! command -v "$TERMINAL_CMD" >/dev/null 2>&1; then
    echo "[WARN] Terminal program $TERMINAL_CMD not found; falling back to current shell" >&2
    terminal_available=false
fi

launch_window() {
    local title="$1"
    local cmd="$2"
    if $terminal_available; then
        printf '[TERM] Opening %s window\n' "$title"
        "$TERMINAL_CMD" --title "$title" -- bash -lc "$cmd; exec bash" &
    else
        printf '[TERM] %s command (fallback): %s\n' "$title" "$cmd"
        bash -lc "$cmd; exec bash"
    fi
}

printf -v remote_launch 'ssh %s %q' "$remote_target" "$REMOTE_CMD"
launch_window "Remote" "$remote_launch"

launch_window "Local" "$LOCAL_CMD"

if [[ "$ENABLE_LOG_PANE" == "true" ]]; then
    printf -v log_launch 'ssh %s %q' "$remote_target" "$LOG_CMD"
    launch_window "Logs" "$log_launch"
fi

printf '[TERM] Terminals launched. Watch for windows named Remote/Local/Logs (if enabled).'
