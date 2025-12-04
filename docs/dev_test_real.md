# Dev + Test loop for LifelineTTY

Got you. You want a **repeatable dev-loop**: build on your PC → auto-ship a binary to the Pi → launch both Pi and local binaries inside tmux → observe logs → restart cleanly without leaving zombies.
_Roadmap alignment: **P19 — Documentation + sample payload refresh** (real hardware test workflow guide)._
_Roadmap alignment: **P19 — Documentation + sample payload refresh** (real hardware test workflow guide)._ 

---

## Quick reference

- `devtest/dev.conf.example` — copy to `devtest/dev.conf`, fill in Pi host + binary paths.
- `devtest/run-dev.sh` — build locally, copy to Pi, kill stale processes, and open a tmux session with remote + local panes (optional log pane).
- `devtest/watch.sh` — `cargo watch` wrapper for rapid local-run loops using your config args.
- `devtest/watch-remote.sh` — `cargo watch` that re-runs the full build→copy→tmux loop on every change.

Run everything from the repo root so relative paths resolve correctly.

---

## 1. Prerequisites

### Local workstation

- Linux host with Rust toolchain installed (`rustup`, `cargo`, etc.).
- `tmux` for multiplexed terminals.
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

---

## 2. Create `dev.conf`

Copy the template and edit the values to match your setup:

```bash
cp devtest/dev.conf.example devtest/dev.conf
```

Key settings inside `dev.conf`:

```bash
PI_HOST=pi@192.168.20.106              # SSH user@host for your Pi
PI_BIN=/home/pi/lifelinetty/lifelinetty # Remote binary path
LOCAL_BIN=target/debug/lifelinetty      # Built binary to copy up
COMMON_ARGS="--run --device /dev/ttyUSB0 --baud 9600 --cols 16 --rows 2"
REMOTE_ARGS=""                          # Optional remote-only args
LOCAL_ARGS=""                           # Optional local-only args
BUILD_CMD="cargo build"                # Override for release/cross builds
TMUX_SESSION=lifeline-dev               # Session name
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

```
./devtest/run-dev.sh
```

What happens:

1. Loads `dev.conf` and verifies `tmux`, `ssh`, and `scp` are available.
2. Runs `BUILD_CMD` (defaults to `cargo build`).
3. Ensures the remote directory exists, copies `LOCAL_BIN` to `PI_BIN`, and marks it executable.
4. Executes `pkill -f $PKILL_PATTERN` on the Pi to clean up stale daemons.
5. Spins up a tmux session (`TMUX_SESSION`):
   - **Window 0 / REMOTE:** SSHs into the Pi and runs `PI_BIN COMMON_ARGS REMOTE_ARGS`.
   - **Window 0 split (optional):** if `ENABLE_LOG_PANE=true`, tails `/run/serial_lcd_cache` using `LOG_WATCH_CMD` so you can watch protocol/polling logs live.
   - **Window 1 / LOCAL:** runs the local binary with `COMMON_ARGS LOCAL_ARGS` for side-by-side comparison.
6. Attaches to the session so `Ctrl+b 1/2` swaps views; detaching or exiting ends both binaries cleanly.

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

```
./devtest/watch.sh
```

`cargo watch` rebuilds whenever files change and re-invokes `cargo run -- COMMON_ARGS LOCAL_ARGS`. Use this while iterating on CLI features, payload parsing, or render loop logic that you can validate locally.

### Pi-integrated watch (`devtest/watch-remote.sh`)

```
./devtest/watch-remote.sh
```

This wraps `cargo watch -s ./devtest/run-dev.sh`, so every file save rebuilds, copies the new binary to the Pi, and relaunches the tmux session. Ideal when debugging serial/LCD behavior on hardware.

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

VS Code will surface stdout/stderr inline while `tmux` handles the interactive panes.

---

## 6. Observability & troubleshooting checklist

- **Live cache pane:** leave `ENABLE_LOG_PANE=true` to keep `/run/serial_lcd_cache/*.log` visible. Swap in a different `LOG_WATCH_CMD` (e.g., `tail -n 50 -f /run/serial_lcd_cache/protocol_errors.log`) when focusing on a specific subsystem.
- **Pre-flight sanity:** always run `cargo fmt && cargo clippy -- -D warnings && cargo test` locally before pushing binaries to hardware. Catching parser or CLI regressions early saves serial round-trips.
- **Permissions:** if `scp` fails with `Permission denied`, ensure the remote directory owner matches your deployment user or adjust with `sudo chown` once.
- **Missing tmux session:** `run-dev.sh` automatically kills an existing session with the same name. If you want to keep history, change `TMUX_SESSION` or detach with `Ctrl+b d`.
- **Pi cache wiped on reboot:** expect `/run/serial_lcd_cache` to disappear after every reboot. The script recreates it implicitly when the daemon starts, but you can pre-create it via `sudo mkdir -p /run/serial_lcd_cache && sudo chown $(whoami) /run/serial_lcd_cache` for manual tests.
- **Serial device busy:** verify no other `lifelinetty` instance or `minicom` process has the UART open; the `pkill` step plus stopping systemd usually resolves it.

---

## 7. Wrap-up checklist

1. Detach from tmux (`Ctrl+b d`) or exit to kill both binaries.
2. Restart `lifelinetty.service` so the Pi goes back to production behavior.
3. Archive logs from `/run/serial_lcd_cache` if you need to compare runs.
4. Commit any config/doc/script tweaks under `devtest/` so the team shares the same workflow (P19 requirement).

That’s it—real hardware testing now takes one command, stays within charter guardrails, and provides a clean way to compare local vs. Pi behavior side-by-side.

