#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONF_FILE_DEFAULT="$SCRIPT_DIR/dev.conf"
CONF_FILE_REAL="$SCRIPT_DIR/dev_real.conf"
CONF_FILE="$CONF_FILE_DEFAULT"

print_usage() {
    cat <<'EOF'
Usage: run-dev.sh [--real] [--help]

  --real   Use dev_real.conf (real host + real Pi wiring)
  --help   Show this message
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --real)
            CONF_FILE="$CONF_FILE_REAL"
            ;;
        --help|-h)
            print_usage
            exit 0
            ;;
        *)
            echo "[ERROR] Unknown argument: $1" >&2
            print_usage >&2
            exit 1
            ;;
    esac
    shift
done

if [[ -f "$CONF_FILE" ]]; then
    # shellcheck disable=SC1090
    source "$CONF_FILE"
else
    echo "[ERROR] Missing dev.conf. Copy dev.conf.example and customize it." >&2
    exit 1
fi

command -v ssh >/dev/null 2>&1 || { echo "[ERROR] ssh is required." >&2; exit 1; }
command -v scp >/dev/null 2>&1 || { echo "[ERROR] scp is required." >&2; exit 1; }

CACHE_DIR=${CACHE_DIR:-/run/serial_lcd_cache}
if [[ "$CACHE_DIR" != "/run/serial_lcd_cache" ]]; then
    cat <<'EOF' >&2
[ERROR] CACHE_DIR must be /run/serial_lcd_cache to satisfy the storage policy.
        Please create it once with:
          sudo mkdir -p /run/serial_lcd_cache && sudo chown "$USER":"$USER" /run/serial_lcd_cache
EOF
    exit 1
fi
if [[ ! -d "$CACHE_DIR" ]]; then
    if ! mkdir -p "$CACHE_DIR" 2>/dev/null; then
        if command -v sudo >/dev/null 2>&1; then
            echo "[INFO] Creating $CACHE_DIR with sudo (one-time setup)" >&2
            if ! sudo mkdir -p "$CACHE_DIR" || ! sudo chown "$USER":"$USER" "$CACHE_DIR"; then
                echo "[ERROR] Unable to create $CACHE_DIR. Run 'sudo mkdir -p $CACHE_DIR && sudo chown $USER:$USER $CACHE_DIR' then retry." >&2
                exit 1
            fi
        else
            echo "[ERROR] Create $CACHE_DIR manually (sudo mkdir -p ...) then rerun." >&2
            exit 1
        fi
    fi
fi
SCENARIO_NAME=${SCENARIO_NAME:-baseline}
SCENARIO_DATE=${SCENARIO_DATE:-$(date +%Y%m%d)}
SCENARIO_ROOT=${SCENARIO_ROOT:-"$CACHE_DIR/milestone1"}
SCENARIO_DIR=${SCENARIO_DIR:-"$SCENARIO_ROOT/${SCENARIO_NAME}-${SCENARIO_DATE}"}
LOCAL_SCENARIO_DIR="$SCENARIO_DIR/local"
REMOTE_SCENARIO_DIR="$SCENARIO_DIR/remote"

PI_USER=${PI_USER:-pi}
PI_BIN=${PI_BIN:-/home/"$PI_USER"/lifelinetty/lifelinetty}
REMOTE_ARCH=${REMOTE_ARCH:-}
LOCAL_ARCH=${LOCAL_ARCH:-}

: "${PI_HOST:?PI_HOST must be set in dev.conf (hostname, e.g. 192.168.20.106)}"
: "${PI_BIN:?PI_BIN must be set in dev.conf}"

TERMINAL_CMD=${TERMINAL_CMD:-gnome-terminal}
ENABLE_SSH_SHELL=${ENABLE_SSH_SHELL:-true}

# Headless-friendly defaults when running inside a container (e.g., docker-compose
# milestone1). In those cases there's typically no GUI terminal, so default to
# inline/background execution and skip the SSH pane unless explicitly enabled.
if [[ -f /.dockerenv ]]; then
    if [[ -z ${TERMINAL_CMD_OVERRIDE:-} ]]; then
        TERMINAL_CMD=""
    fi
    if [[ -z ${ENABLE_SSH_SHELL_OVERRIDE:-} ]]; then
        ENABLE_SSH_SHELL=false
    fi
fi


LOCAL_BIN=${LOCAL_BIN:-target/debug/lifelinetty}
LOCAL_BIN_SOURCE=${LOCAL_BIN_SOURCE:-}
REMOTE_BIN_SOURCE=${REMOTE_BIN_SOURCE:-}
COMMON_ARGS=${COMMON_ARGS:-run --baud 9600 --cols 16 --rows 2}
REMOTE_ARGS=${REMOTE_ARGS:-}
LOCAL_ARGS=${LOCAL_ARGS:-}
BUILD_CMD=${BUILD_CMD:-make all}
ENABLE_LOG_PANE=${ENABLE_LOG_PANE:-false}
LOG_WATCH_CMD=${LOG_WATCH_CMD:-"watch -n 0.5 ls -lh $SCENARIO_DIR"}
PKILL_PATTERN=${PKILL_PATTERN:-lifelinetty}

# Scenario-aware config templates. By default, use a single template for both
# local and remote, but allow overrides so dev.conf can point each side at a
# different TOML if needed.
resolve_path() {
    local p="$1"
    if [[ "$p" = /* ]]; then
        printf '%s' "$p"
    else
        printf '%s/%s' "$SCRIPT_DIR" "$p"
    fi
}

LOCAL_BIN=$(resolve_path "$LOCAL_BIN")
if [[ -n "${LOCAL_BIN_SOURCE:-}" ]]; then
    LOCAL_BIN_SOURCE=$(resolve_path "$LOCAL_BIN_SOURCE")
fi
if [[ -n "${REMOTE_BIN_SOURCE:-}" ]]; then
    REMOTE_BIN_SOURCE=$(resolve_path "$REMOTE_BIN_SOURCE")
fi

map_machine_to_release_arch() {
    case "$1" in
        x86_64|amd64|i[3-6]86)
            printf 'x86'
            ;;
        armv6l)
            printf 'armv6'
            ;;
        armv7l)
            printf 'armv7'
            ;;
        aarch64|arm64)
            printf 'arm64'
            ;;
        *)
            return 1
            ;;
    esac
}

if [[ -z "${LOCAL_ARCH:-}" ]]; then
    host_arch=$(map_machine_to_release_arch "$(uname -m)" 2>/dev/null || true)
    if [[ -n "$host_arch" ]]; then
        LOCAL_ARCH="$host_arch"
    fi
fi

CONFIG_SOURCE_FILE=$(resolve_path "${CONFIG_SOURCE_FILE:-config/local/default.toml}")
LOCAL_CONFIG_SOURCE_FILE=$(resolve_path "${LOCAL_CONFIG_SOURCE_FILE:-$CONFIG_SOURCE_FILE}")
REMOTE_CONFIG_SOURCE_FILE=$(resolve_path "${REMOTE_CONFIG_SOURCE_FILE:-config/remote/default.toml}")

if [[ ! -f "$LOCAL_CONFIG_SOURCE_FILE" ]]; then
    echo "[ERROR] Missing local dev config template $LOCAL_CONFIG_SOURCE_FILE" >&2
    exit 1
fi
if [[ ! -f "$REMOTE_CONFIG_SOURCE_FILE" ]]; then
    echo "[ERROR] Missing remote dev config template $REMOTE_CONFIG_SOURCE_FILE" >&2
    exit 1
fi

if [[ ! "$SCENARIO_DIR" =~ ^${CACHE_DIR}/ ]]; then
    echo "[ERROR] SCENARIO_DIR must live inside $CACHE_DIR (got $SCENARIO_DIR)" >&2
    exit 1
fi

if ! mkdir -p "$LOCAL_SCENARIO_DIR"; then
    echo "[ERROR] Unable to create local scenario directory $LOCAL_SCENARIO_DIR" >&2
    exit 1
fi
echo "[CACHE] Scenario bundle will be stored under $SCENARIO_DIR"

if [[ -z "${LOCAL_CONFIG_HOME:-}" ]]; then
    # Preserve the temporary HOME for the lifetime of the spawned daemon to keep
    # the copied config reachable (B4: CLI dev loop fidelity).
    LOCAL_CONFIG_HOME=$(mktemp -d -t lifelinetty-home.XXXXXX)
fi
LOCAL_CONFIG_DIR="$LOCAL_CONFIG_HOME/.serial_lcd"
mkdir -p "$LOCAL_CONFIG_DIR"
cp "$LOCAL_CONFIG_SOURCE_FILE" "$LOCAL_CONFIG_DIR/config.toml"
echo "[CONFIG] Using local template $LOCAL_CONFIG_SOURCE_FILE (local HOME=$LOCAL_CONFIG_HOME)"

printf '[BUILD] Using "%s"\n' "$BUILD_CMD"
bash -c "$BUILD_CMD"

REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
RELEASE_ROOT=${RELEASE_ROOT:-"$REPO_ROOT/releases/debug"}

release_bin_path() {
    local arch="$1"
    if [[ -z "$arch" ]]; then
        return 1
    fi
    printf '%s' "$RELEASE_ROOT/$arch/lifelinetty"
}

if [[ -z "$LOCAL_BIN_SOURCE" ]]; then
    if [[ -n "$LOCAL_ARCH" ]]; then
        LOCAL_BIN_SOURCE=$(release_bin_path "$LOCAL_ARCH")
    else
        LOCAL_BIN_SOURCE="$LOCAL_BIN"
    fi
fi

if [[ -z "$REMOTE_BIN_SOURCE" ]]; then
    if [[ -n "$REMOTE_ARCH" ]]; then
        REMOTE_BIN_SOURCE=$(release_bin_path "$REMOTE_ARCH")
    else
        REMOTE_BIN_SOURCE="$LOCAL_BIN_SOURCE"
    fi
fi

if [[ ! -x "$LOCAL_BIN_SOURCE" ]]; then
    echo "[ERROR] Local binary $LOCAL_BIN_SOURCE is missing or not executable." >&2
    exit 1
fi

if [[ ! -x "$REMOTE_BIN_SOURCE" ]]; then
    echo "[ERROR] Remote binary $REMOTE_BIN_SOURCE is missing or not executable." >&2
    exit 1
fi

remote_dir=$(dirname "$PI_BIN")
remote_target="$PI_USER@$PI_HOST"

SSH_OPTIONS=(-o BatchMode=yes -o ConnectTimeout=5)

run_remote_cmd() {
    local cmd="$1"
    local quoted_cmd
    printf -v quoted_cmd '%q' "$cmd"
    if ! ssh "${SSH_OPTIONS[@]}" "$remote_target" bash -lc "$quoted_cmd"; then
        local rc=$?
        echo "[ERROR] Remote command failed on $remote_target: $cmd" >&2
        echo "[ERROR] See above for the ssh output." >&2
        return $rc
    fi
}

assert_remote_reachable() {
    if ! run_remote_cmd "true"; then
        echo "[ERROR] Unable to reach $remote_target. Verify network connectivity and SSH credentials." >&2
        exit 1
    fi
}

ensure_remote_dir() {
    if ! run_remote_cmd "mkdir -p '$remote_dir'"; then
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

deploy_remote_config() {
    if ! run_remote_cmd "mkdir -p ~/.serial_lcd"; then
        echo "[ERROR] Unable to create remote config directory ~/.serial_lcd" >&2
        exit 1
    fi
    echo "[DEPLOY] Copying remote config $REMOTE_CONFIG_SOURCE_FILE to $remote_target:~/.serial_lcd/config.toml"
    scp "${SSH_OPTIONS[@]}" "$REMOTE_CONFIG_SOURCE_FILE" "$remote_target:~/.serial_lcd/config.toml"
}

assert_remote_reachable
echo "[DEPLOY] Ensuring remote directory $remote_dir"
ensure_remote_dir
echo "[CACHE] Ensuring remote scenario directory $REMOTE_SCENARIO_DIR"
if ! run_remote_cmd "mkdir -p '$REMOTE_SCENARIO_DIR'"; then
        echo "[WARN] Failed to create $REMOTE_SCENARIO_DIR; trying sudo" >&2
        if ! run_remote_cmd "sudo mkdir -p '$REMOTE_SCENARIO_DIR' && sudo chown $PI_USER:$PI_USER '$REMOTE_SCENARIO_DIR'"; then
                cat <<'EOF' >&2
[ERROR] Unable to create the remote scenario directory under /run/serial_lcd_cache.
Run on the Pi once:
    sudo mkdir -p /run/serial_lcd_cache && sudo chown $USER:$USER /run/serial_lcd_cache
Then rerun run-dev.sh.
EOF
                exit 1
        fi
fi
deploy_remote_config

LOCAL_CONFIG_PATH="$LOCAL_CONFIG_DIR/config.toml"
REMOTE_CONFIG_PATH="\$HOME/.serial_lcd/config.toml"
LOCAL_CONFIG_ARG="--config-file $LOCAL_CONFIG_PATH"
REMOTE_CONFIG_ARG="--config-file $REMOTE_CONFIG_PATH"

echo "[DEPLOY] Copying $REMOTE_BIN_SOURCE to $remote_target:$PI_BIN"
scp "${SSH_OPTIONS[@]}" "$REMOTE_BIN_SOURCE" "$remote_target:$PI_BIN"
run_remote_cmd "chmod +x '$PI_BIN'"

echo "[REMOTE] Killing stale processes matching '$PKILL_PATTERN'"
if ! run_remote_cmd "pkill -f '$PKILL_PATTERN' || true"; then
    echo "[WARN] pkill reported an error; collecting remote diagnostics" >&2
    run_remote_cmd "echo '[REMOTE-DEBUG] user: ' \$(whoami); id; ls -ld '$remote_dir'; ls -l '$PI_BIN' || true; ps -ef | grep -i lifelinetty || true" || true
fi

LOCAL_LOG_PATH="$LOCAL_SCENARIO_DIR/lifelinetty-local.log"
REMOTE_LOG_PATH="$REMOTE_SCENARIO_DIR/lifelinetty-remote.log"
echo "[LOG] Local log -> $LOCAL_LOG_PATH"
echo "[LOG] Remote log -> $REMOTE_LOG_PATH"

build_cmd_string() {
    local cmd="$1"; shift
    for chunk in "$@"; do
        [[ -n "$chunk" ]] || continue
        cmd+=" $chunk"
    done
    printf '%s' "$cmd"
}

REMOTE_ENV="LIFELINETTY_LOG_PATH=$REMOTE_LOG_PATH"
LOCAL_ENV="HOME=$LOCAL_CONFIG_HOME LIFELINETTY_LOG_PATH=$LOCAL_LOG_PATH"
REMOTE_CMD=$(build_cmd_string "$REMOTE_ENV $PI_BIN" "$COMMON_ARGS" "$REMOTE_CONFIG_ARG" "$REMOTE_ARGS")
LOCAL_CMD=$(build_cmd_string "$LOCAL_ENV $LOCAL_BIN_SOURCE" "$COMMON_ARGS" "$LOCAL_CONFIG_ARG" "$LOCAL_ARGS")
echo "[REMOTE CMD] $REMOTE_CMD"
echo "[LOCAL CMD]  $LOCAL_CMD"
LOG_CMD="$LOG_WATCH_CMD"

terminal_available=true
if [[ -z "$TERMINAL_CMD" ]] || ! command -v "$TERMINAL_CMD" >/dev/null 2>&1; then
    echo "[WARN] Terminal program $TERMINAL_CMD not found; falling back to current shell" >&2
    terminal_available=false
fi

if ! $terminal_available && [[ "${ENABLE_SSH_SHELL:-true}" == "true" ]]; then
    # Disable the interactive SSH shell in headless mode unless explicitly requested.
    ENABLE_SSH_SHELL=false
fi

declare -A HEADLESS_PIDS=()

launch_window() {
    local title="$1"
    local cmd="$2"
    if $terminal_available; then
        printf '[TERM] Opening %s window\n' "$title"
        "$TERMINAL_CMD" --title "$title" -- bash -lc "$cmd; exec bash" &
    else
        printf '[HEADLESS] %s command: %s\n' "$title" "$cmd"
        bash -lc "$cmd" &
        HEADLESS_PIDS["$title"]=$!
    fi
}

# Terminal #1: persistent SSH shell for manual diagnostics/log inspection.
SSH_SHELL_CMD=${SSH_SHELL_CMD:-"ssh $remote_target"}
if [[ "$ENABLE_SSH_SHELL" == "true" ]]; then
    launch_window "SSH" "$SSH_SHELL_CMD"
fi

REMOTE_LAUNCH_DELAY=${REMOTE_LAUNCH_DELAY:-1.0}
printf -v remote_launch 'ssh %s %q' "$remote_target" "sleep $REMOTE_LAUNCH_DELAY; $REMOTE_CMD"
launch_window "Remote" "$remote_launch"

launch_window "Local" "$LOCAL_CMD"

if [[ "$ENABLE_LOG_PANE" == "true" ]]; then
    printf -v log_launch 'ssh %s %q' "$remote_target" "$LOG_CMD"
    launch_window "Logs" "$log_launch"
fi

if $terminal_available; then
    printf '[TERM] Terminals launched. Watch for windows named SSH/Remote/Local/Logs (if enabled).'
else
    printf '\n[HEADLESS] Background processes:'
    for title in "${!HEADLESS_PIDS[@]}"; do
        printf '\n  - %s (pid %s)' "$title" "${HEADLESS_PIDS[$title]}"
    done
    printf '\n[HEADLESS] Use ps/ssh to monitor the runs.\n'
fi
