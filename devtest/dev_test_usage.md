

## Usage docs for run-dev.sh 🛠

This describes the current behavior. Scenarios can be supplied via templates (copied to `~/.serial_lcd/config.toml`) or directly via the `--config-file` flag.

### Prerequisites

On your **developer machine** (Linux desktop):

- Rust toolchain installed (`cargo`).
- `ssh` and `scp` installed.
- A terminal emulator that supports:

  - `--title`
  - `-- bash -lc "..."`

  e.g. `gnome-terminal` (default in dev.conf.example).

On your **Raspberry Pi** **or docker remote container**:

- SSH enabled and reachable (`PI_HOST`). For docker-compose.milestone1.yml this is `lifelinetty-remote`.
- A user account (`PI_USER`) that can:
  - SSH in without interactive prompts (SSH keys recommended; password auth is dev-only).
  - Write to the directory that will hold the `lifelinetty` binary (`PI_BIN`’s parent).
- lifelinetty.service stopped or disabled while using this dev loop, so the daemon doesn’t fight you for the UART (not applicable to clean docker remote images).

Inside **docker containers** (both local runner and remote sshd):

- There is no GUI terminal. `run-dev.sh` automatically falls back to headless mode when it detects `/.dockerenv` unless you override with `TERMINAL_CMD_OVERRIDE`/`ENABLE_SSH_SHELL_OVERRIDE`.
- Install Rust in the local runner: `apt-get update && apt-get install -y curl build-essential pkg-config libssl-dev ca-certificates && curl -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable && . $HOME/.cargo/env`.
- Use the shared cache bind (e.g., `/home/dave/lifelinetty-cache:/run/serial_lcd_cache`) so scenario logs are persisted on the host.

### 1. Configure dev.conf

Start from the example:

```bash
cd /home/dave/github/LifelineTTY
cp devtest/dev.conf.example devtest/dev.conf
```

Edit dev.conf:

- Basic Pi info:

  ```bash
  PI_USER=pi
  PI_HOST=192.168.20.106   # or your Pi’s hostname/IP
  PI_BIN=/home/$PI_USER/lifelinetty/lifelinetty
  ```

- Local binary path (what we build and upload):

  ```bash
  LOCAL_BIN=target/debug/lifelinetty
  ```

- Common CLI arguments for both local and remote runs:

  ```bash
  COMMON_ARGS="--run --device /dev/ttyUSB0 --baud 9600 --cols 16 --rows 2"
  ```

  You can override `REMOTE_ARGS` and `LOCAL_ARGS` separately if needed:

  ```bash
  REMOTE_ARGS=""
  LOCAL_ARGS=""
  ```

- Build command:

  ```bash
  BUILD_CMD="make all"
  ```

  This default runs `make all` so that each architecture’s release binary lands under `releases/debug/<arch>/lifelinetty`. Override with `BUILD_CMD="cargo build"` if you only need the host debug build.

- Terminal emulator:

  ```bash
  TERMINAL_CMD=gnome-terminal
  # Set TERMINAL_CMD="" inside headless containers; run-dev.sh will background the commands.
  # In containers, run-dev.sh defaults to headless. To force GUI-like behavior, set TERMINAL_CMD_OVERRIDE=<your_cmd>.
  ```

- Optional log watcher pane (Terminal #4):

  ```bash
  ENABLE_LOG_PANE=true
  LOG_WATCH_CMD="watch -n 0.5 ls -lh /run/serial_lcd_cache"
  ```

- Scenario tagging & log bundles (Milestone 1):

  Each run writes logs to `/run/serial_lcd_cache/milestone1/<scenario>-YYYYMMDD/{local,remote}`. The script automatically sets `LIFELINETTY_LOG_PATH` for both sides; adjust these if you need a specific tag/date:

  ```bash
  # SCENARIO_NAME=baseline
  # SCENARIO_DATE=$(date +%Y%m%d)
  # SCENARIO_DIR=/run/serial_lcd_cache/milestone1/${SCENARIO_NAME}-${SCENARIO_DATE}
  ```

- Remote process cleanup pattern:

  ```bash
  PKILL_PATTERN=lifelinetty
  ```

- **Scenario templates** (still supported alongside `--config-file`):

  These live under `devtest/config` and are copied into `~/.serial_lcd/config.toml` on each side before running:

  ```bash
  CONFIG_SOURCE_FILE=devtest/config/local/default.toml
  # LOCAL_CONFIG_SOURCE_FILE=devtest/config/local/default.toml
  # REMOTE_CONFIG_SOURCE_FILE=devtest/config/remote/default.toml
  ```

  Available templates:

  - `local/default.toml` (baseline local)
  - `remote/default.toml` (baseline remote)
  - `lifelinetty.toml` (shared baseline)
  - `test-16x2.toml` (baseline 16x2)
  - `test-20x4.toml` (20x4 geometry)
  - `uart-ama0.toml` (Pi onboard UART)
  - `stress-9600.toml` (baseline baud + polling enabled)
  - `stress-19200.toml` (higher-baud probe; only after 9600 is stable)

  - If you leave the per-side overrides commented out, local uses `CONFIG_SOURCE_FILE` and remote uses `REMOTE_CONFIG_SOURCE_FILE` by default.
  - To test different scenarios (e.g., local stub vs. real UART on Pi), point `LOCAL_CONFIG_SOURCE_FILE` and `REMOTE_CONFIG_SOURCE_FILE` at different TOMLs.

- **Direct config-file use**:

  The binary accepts `--config-file <path>` as the highest-priority source. To drive scenarios via the flag instead of template copies, add it to your args in `dev.conf`, e.g.:

  ```bash
  COMMON_ARGS="--run --config-file devtest/config/test-16x2.toml --device /dev/ttyUSB0 --baud 9600 --cols 16 --rows 2"
  ```

  Template copying remains supported; the flag simply lets you bypass `~/.serial_lcd/config.toml` if desired.

### 2. What run-dev.sh does

When you run:

```bash
cd /home/dave/github/LifelineTTY
./devtest/run-dev.sh
```

it performs:

1. **Load config**

   - Sources dev.conf.
   - Validates `PI_HOST` and `PI_BIN`.
   - Verifies `ssh` and `scp` are available.
   - Verifies that:
     - `LOCAL_CONFIG_SOURCE_FILE` exists.
     - `REMOTE_CONFIG_SOURCE_FILE` exists.

1. **Prepare scenario bundle + local temp HOME**

   - Validates that `SCENARIO_DIR` lives under `/run/serial_lcd_cache` and creates the per-run bundle at `/run/serial_lcd_cache/milestone1/<scenario>-YYYYMMDD/{local,remote}`.
   - Creates a temp directory: `LOCAL_CONFIG_HOME=$(mktemp -d -t lifelinetty-home.XXXXXX)` and `"$LOCAL_CONFIG_HOME/.serial_lcd"`.
   - Copies `LOCAL_CONFIG_SOURCE_FILE` into:

     ```text
     $LOCAL_CONFIG_HOME/.serial_lcd/config.toml
     ```

   - Prints something like:

     ```text
     [CONFIG] Using local template devtest/config/lifelinetty.toml (local HOME=/tmp/lifelinetty-home.XXXX)
     [CACHE] Scenario bundle will be stored under /run/serial_lcd_cache/milestone1/baseline-YYYYMMDD
     ```

1. **Build locally**

   - Runs `BUILD_CMD` via `bash -c "$BUILD_CMD"`.
     - Default: `make all`, which writes every architecture’s release binary into `releases/debug/<arch>/lifelinetty`.
     - Set `LOCAL_ARCH`/`REMOTE_ARCH` (or `LOCAL_BIN_SOURCE`/`REMOTE_BIN_SOURCE`) to point the loop at the binaries you want to deploy.
     - Override with `BUILD_CMD="cargo build"` if you just need the host debug build to stay lightweight.
   - Warns if `LOCAL_BIN_SOURCE` doesn’t exist or isn’t executable afterwards.

1. **Pre-flight remote**

  Asserts the Pi is reachable via SSH (`ssh -o BatchMode=yes -o ConnectTimeout=5`), ensures `dirname(PI_BIN)` exists (and is writable by `PI_USER`), creates the remote scenario directory under `/run/serial_lcd_cache/milestone1/<scenario>-YYYYMMDD/remote`, and copies `REMOTE_CONFIG_SOURCE_FILE` to:

  ```text
  ~/.serial_lcd/config.toml
  ```

1. **Binary sync**

  Copies `LOCAL_BIN_SOURCE` up to `PI_BIN` via `scp` and then runs `chmod +x "$PI_BIN"` on the Pi to ensure execute permissions.

1. **Remote cleanup**

   - Runs `pkill -f "$PKILL_PATTERN" || true` on the Pi, to kill any stale `lifelinetty` processes.
   - You’re expected to stop lifelinetty.service yourself (e.g. via `systemctl stop lifelinetty.service`) before using this loop.

1. **Build command lines**

   - Remote:

     ```bash
     REMOTE_CMD="LIFELINETTY_LOG_PATH=/run/serial_lcd_cache/milestone1/<scenario>-YYYYMMDD/remote/lifelinetty-remote.log $PI_BIN $COMMON_ARGS $REMOTE_ARGS"
     ```

   - Local (ensuring it uses the temp HOME):

     ```bash
     LOCAL_CMD="HOME=$LOCAL_CONFIG_HOME LIFELINETTY_LOG_PATH=/run/serial_lcd_cache/milestone1/<scenario>-YYYYMMDD/local/lifelinetty-local.log $LOCAL_BIN_SOURCE $COMMON_ARGS $LOCAL_ARGS"
     ```

   - Logs:

     ```bash
     LOG_CMD="$LOG_WATCH_CMD"  # defaults to watching the scenario bundle path
     ```

1. **Launch terminals**

  For each window, if `TERMINAL_CMD` is found, it runs:

  ```bash
  $TERMINAL_CMD --title "<Title>" -- bash -lc "<Command>; exec bash" &
  ```

  Otherwise (or when `TERMINAL_CMD=""`), it backgrounds the commands in the current shell and prints their PIDs. In containers, `ENABLE_SSH_SHELL` defaults to `false`; override with `ENABLE_SSH_SHELL_OVERRIDE=true` if you really want the SSH pane.

  **Terminal #1 – SSH**:

  ```bash
  SSH_SHELL_CMD=${SSH_SHELL_CMD:-"ssh $PI_USER@$PI_HOST"}
  # Title: SSH
  ```

  **Terminal #2 – Remote**:

  ```bash
  remote_launch=$(printf 'ssh %s %q' "$PI_USER@$PI_HOST" "$REMOTE_CMD")
  # Title: Remote
  ```

  **Terminal #3 – Local**:

  ```bash
  LOCAL_CMD   # as built above
  # Title: Local
  ```

  **Optional Terminal #4 – Logs** (if `ENABLE_LOG_PANE=true`):

  ```bash
  log_launch=$(printf 'ssh %s %q' "$PI_USER@$PI_HOST" "$LOG_CMD")
  # Title: Logs
  ```

  You’ll see a summary:

  ```text
  [TERM] Terminals launched. Watch for windows named SSH/Remote/Local/Logs (if enabled).
  ```

1. **Exit behavior**

   - Closing any terminal stops just that process.
   - Re-running run-dev.sh:
     - Rebuilds (per `BUILD_CMD`).
     - Re-syncs binary.
     - Kills any stale remote processes.
     - Opens a fresh set of terminals.

### 3. Optional watchers

You already have:

- watch.sh – runs `cargo watch -q -s "cargo run -- $RUN_ARGS"` for local runs (driven by `COMMON_ARGS`/`LOCAL_ARGS`).
- watch-remote.sh – runs `cargo watch -q -s "./devtest/run-dev.sh"` to automatically rebuild and redeploy to the Pi on source changes.

Examples:

```bash
cd /home/dave/github/LifelineTTY
./devtest/watch.sh          # local-only dev loop
./devtest/watch-remote.sh   # full Milestone 1 hardware loop on changes
```

---

### 4. Docker/compose cheat sheet (Milestone 1)

Assuming `docker-compose.milestone1.yml` is up and the shared cache is bound to `/run/serial_lcd_cache` on both containers:

1) Start the pair (from repo root):

```bash
docker compose -f docker-compose.milestone1.yml up -d
```

1) Enter the local runner and install Rust (once):

```bash
docker exec -it lifelinetty-local bash
apt-get update && apt-get install -y curl build-essential pkg-config libssl-dev ca-certificates
curl -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
. $HOME/.cargo/env
```

1) Prepare dev.conf (headless defaults apply in-container):

```bash
cd /workspace
cp devtest/dev.conf.example devtest/dev.conf
cat >> devtest/dev.conf <<'EOF'
PI_HOST=lifelinetty-remote
PI_USER=root
PI_BIN=/opt/lifelinetty/lifelinetty
COMMON_ARGS="--demo --cols 16 --rows 2"
BUILD_CMD="make all"
TERMINAL_CMD=""               # headless
ENABLE_SSH_SHELL=false         # skip SSH pane
EOF
```

1) Run headless (logs land under `/run/serial_lcd_cache/milestone1/<scenario>-YYYYMMDD/{local,remote}` shared to the host):

```bash
. $HOME/.cargo/env
SCENARIO_NAME=baseline ./devtest/run-dev.sh
```

1) Inspect logs from the host (cache bind, e.g., `/home/dave/lifelinetty-cache`):

```bash
ls /home/dave/lifelinetty-cache/milestone1/baseline-*/{local,remote}
```

Override knobs:

- `TERMINAL_CMD_OVERRIDE=<cmd>`: force a terminal even inside containers.
- `ENABLE_SSH_SHELL_OVERRIDE=true`: re-enable SSH pane in headless/container runs.
- `LOCAL_BIN_SOURCE` / `REMOTE_BIN_SOURCE`: point to prebuilt binaries (e.g., `releases/debug/armv6/lifelinetty`).
- `LOCAL_ARCH` / `REMOTE_ARCH`: tell the script which release artifacts under `releases/debug/<arch>/lifelinetty` to deploy. `LOCAL_ARCH` defaults to your host if unset, and the loop mirrors that binary to the Pi unless `REMOTE_ARCH` is set.
- `SCENARIO_NAME` / `SCENARIO_DATE`: tag multiple runs in the shared cache.

All other settings from the desktop flow still apply (config templates, `REMOTE_ARGS`/`LOCAL_ARGS`, etc.).
