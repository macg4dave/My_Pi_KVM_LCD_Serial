# Dev + Test loop for LifelineTTY

A **repeatable dev-loop**: build on your PC → auto-ship a binary to the Pi → launch the remote/local runs inside dedicated terminal windows (GNOME Terminal by default) → observe logs → restart cleanly without leaving zombies.
_Roadmap alignment: **v0.2.0 field trial readiness + devtest milestone** (real hardware test workflow guide)._ 

---

## Quick reference

- `devtest/dev.conf.example` — copy to `devtest/dev.conf`, fill in Pi host + binary paths.
- `devtest/run-dev.sh` — build locally, copy to Pi, kill stale processes, and open remote/local (and optional log) terminal windows so you can watch both runs live.
- `devtest/watch.sh` — `cargo watch` wrapper for rapid local-run loops using your config args.
- `devtest/watch-remote.sh` — `cargo watch` that re-runs the full build→copy→terminal-window loop on every change.

Run everything from the repo root so relative paths resolve correctly.

---

## 1. Prerequisites

### Local workstation

- Linux host with Rust toolchain installed (`rustup`, `cargo`, etc.).
- `gnome-terminal` (or another `TERMINAL_CMD`) so each run lives in its own window.
- `cargo-watch` for change detection (`cargo install cargo-watch`).
- SSH key-based access to the Pi (password auth works but slows the loop).
- LifelineTTY repo checked out (`git clone git@github.com:macg4dave/LifelineTTY.git`).

### Raspberry Pi under test

- Running the same commit/build you plan to test.
- `lifelinetty.service` temporarily stopped while you run manual sessions:

  ```bash
  sudo systemctl stop lifelinetty.service
  ```

- `/run/serial_lcd_cache` exists and is writable by the service user (systemd unit and the dev loop both rely on it).
- Pi clock + locale set so file timestamps remain sane when logs get pulled back.

**Matrix reminder (v0.2.0):** exercise at least the baseline (USB0 9600 16×2), alt TTY (AMA0 9600), and higher-baud probe (USB0 19200) scenarios while running this loop. Note outcomes and cache logs for each.

---

## 2. Create `dev.conf`

Copy the template and edit the values to match your setup:

```bash
cp devtest/dev.conf.example devtest/dev.conf
```

Key settings inside `dev.conf`:

```bash
PI_USER=pi                              # SSH user on the Pi (override when it differs)
PI_HOST=192.168.20.106                  # Hostname or IP for your Pi
PI_BIN=/opt/lifelinetty/lifelinetty      # Remote binary path (avoid the login home dir)
LOCAL_BIN=target/debug/lifelinetty      # Built binary to copy up
COMMON_ARGS="--run --device /dev/ttyUSB0 --baud 9600 --cols 16 --rows 2"
REMOTE_ARGS=""                          # Optional remote-only args
LOCAL_ARGS=""                           # Optional local-only args
BUILD_CMD="cargo build"                # Override for release/cross builds
TERMINAL_CMD=gnome-terminal             # Terminal used to surface each pane (must support --title + bash -lc)
ENABLE_LOG_PANE=true                    # Adds live cache watcher
LOG_WATCH_CMD="watch -n 0.5 ls -lh /run/serial_lcd_cache"
PKILL_PATTERN=lifelinetty               # What to kill before relaunch
```

Tips:

- Point `LOCAL_BIN` at `target/debug/lifelinetty` for speed or `target/release/lifelinetty` when testing optimized builds.
- Switch `BUILD_CMD` to `cargo build --release` or `scripts/local-release.sh --target arm-unknown-linux-musleabihf` when cross-compiling for Pi without QEMU.
- Keep the arguments aligned with the CLI charter: only use documented flags and point logs/config to allowed paths.

---

## 3. Build → copy → dual-run (`devtest/run-dev.sh`)

```bash
./devtest/run-dev.sh
```

What happens:

1. Loads `dev.conf`, ensures `ssh`/`scp` are available, and fills in defaults such as `PI_USER`, `TERMINAL_CMD`, and `ENABLE_LOG_PANE`.
2. Runs `BUILD_CMD` (defaults to `cargo build`).
3. Ensures the remote directory exists, copies `LOCAL_BIN` to `PI_BIN`, and marks it executable.
4. Executes `pkill -f $PKILL_PATTERN` on the Pi to clean up stale daemons.
5. Launches `$TERMINAL_CMD` windows titled Remote, Local, and (if enabled) Logs. Each window runs the remote SSH command, the local binary, or the log tail inside `bash -lc`, keeping them open until you close the window.
6. Script ends once the windows are launched—the processes continue inside the terminals until you exit them, which kills the remote/local runs on both the Pi and your workstation.

### Systemd safety

- Before the loop, stop the packaged service (`sudo systemctl stop lifelinetty.service`). When you are done testing, restart it with `sudo systemctl start lifelinetty.service`.
- If the service auto-starts (e.g., after a reboot) while you are mid-session, your manual SSH launch will fail with “device busy.” Stop the unit again and rerun `run-dev.sh`.

### Collecting logs after a run

All runtime logs live under `/run/serial_lcd_cache`. Grab them while still attached:

```bash
scp -r "$PI_HOST:/run/serial_lcd_cache" ./tmp/pi-cache-$(date +%s)
```

---

## 4. Fast rebuild + rerun loops

### Local-only watch (`devtest/watch.sh`)

```bash
./devtest/watch.sh
```

`cargo watch` rebuilds whenever files change and re-invokes `cargo run -- COMMON_ARGS LOCAL_ARGS`. Use this while iterating on CLI features, payload parsing, or render loop logic that you can validate locally.

### Pi-integrated watch (`devtest/watch-remote.sh`)

```bash
./devtest/watch-remote.sh
```

This wraps `cargo watch -s ./devtest/run-dev.sh`, so every file save rebuilds, copies the new binary to the Pi, and relaunches the terminal-window loop. Ideal when debugging serial/LCD behavior on hardware.

Both watch scripts honor `COMMON_ARGS`, so you can toggle modes (e.g., `--demo`, `--test-lcd`, different baud/cols/rows) from a single config file.

---

## 5. One-key IDE trigger (optional)

Add the following to `.vscode/tasks.json` to map **Ctrl+Shift+B** (or any keybinding) to the hardware loop:

```json
{
  "version": "2.0.0",
  "tasks": [
    {
      "label": "Dev Loop (Pi + Local)",
      "type": "shell",
      "command": "./devtest/run-dev.sh",
      "problemMatcher": []
    }
  ]
}
```

VS Code will surface stdout/stderr inline while your terminal program handles the interactive panes.

---

## 6. Observability & troubleshooting checklist

- **Live cache pane:** leave `ENABLE_LOG_PANE=true` to keep `/run/serial_lcd_cache/*.log` visible. Swap in a different `LOG_WATCH_CMD` (e.g., `tail -n 50 -f /run/serial_lcd_cache/protocol_errors.log`) when focusing on a specific subsystem.
- **Pre-flight sanity:** always run `cargo fmt && cargo clippy -- -D warnings && cargo test` locally before pushing binaries to hardware. Catching parser or CLI regressions early saves serial round-trips.
- **Permissions:** if `run-dev.sh` exits while creating or copying to `$PI_BIN` with `Permission denied`, pre-create the directory with `ssh $PI_USER@$PI_HOST sudo mkdir -p /opt/lifelinetty` and `ssh $PI_USER@$PI_HOST sudo chown $PI_USER /opt/lifelinetty`, or point `PI_BIN` at a directory the SSH user already owns. The script now explains this requirement when it cannot `mkdir -p` the remote path.
- **Missing windows:** `run-dev.sh` launches whatever terminal program `TERMINAL_CMD` points at. If a previous set of windows is still open, just reuse or close them before rerunning, or override `TERMINAL_CMD` to point at another emulator (e.g., `xterm`).
- **Pi cache wiped on reboot:** expect `/run/serial_lcd_cache` to disappear after every reboot. The script recreates it implicitly when the daemon starts, but you can pre-create it via `sudo mkdir -p /run/serial_lcd_cache && sudo chown $(whoami) /run/serial_lcd_cache` for manual tests.
- **Serial device busy:** verify no other `lifelinetty` instance or `minicom` process has the UART open; the `pkill` step plus stopping systemd usually resolves it.

---

## 6.5 Matrix logging & reporting

- **Scenario checklist:** run the baseline (`/dev/ttyUSB0` @ 9600, 16×2), alt-TTY (`/dev/ttyAMA0` @ 9600), and higher-baud probe (`/dev/ttyUSB0` @ 19200) using `devtest/run-dev.sh` and the watch helpers. For each run, note the payload file (`samples/payload_examples.json` or the appropriate demo set), the CLI args exercised, and any anomalies (RSS, watchdog resets, icon churn). Record these notes inside `docs/dev_test_real.md`, a linked issue, or a `/run/serial_lcd_cache/milestone1/<scenario>-YYYYMMDD` summary file so teammates can retrace the steps.
- **Log snapshot command:** after a run, pull the cache snapshot with something like:

```bash
remote_tag="baseline-$(date +%Y%m%d_%H%M%S)"
scp -r "$PI_HOST:/run/serial_lcd_cache" "./tmp/pi-cache-$remote_tag"
```

Keep the copy date-stamped and include the scenario name in the directory so you can pair logs with the matrix entry. Archived snapshots belong under `tmp/` (or your local artifacts dir) while the canonical live files stay inside `/run/serial_lcd_cache` on the Pi.

- **Traceability:** capture the `scp` command output and the terminal-window layout (e.g., Remote, Local, Logs) in the same note so reviewers know what they saw. If a defect is triggered, reference the cache snapshot and add a regression test (unit or integration) before closing the issue.

---

## 7. Wrap-up checklist

1. Close the Remote/Local/Logs windows you launched to stop their processes.
2. Restart `lifelinetty.service` so the Pi goes back to production behavior.
3. Archive logs from `/run/serial_lcd_cache` if you need to compare runs.
4. Commit any config/doc/script tweaks under `devtest/` so the team shares the same workflow (v0.2.0 devtest milestone requirement).

That’s it—real hardware testing now takes one command, stays within charter guardrails, and provides a clean way to compare local vs. Pi behavior side-by-side.
