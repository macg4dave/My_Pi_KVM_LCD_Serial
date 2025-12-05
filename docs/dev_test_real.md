# **Improved Dev + Test Workflow Plan (Three-Window Hardware Loop)**

Updated workflow guidance aligned with the rest of the docs.
It keeps things blunt and practical: **three-terminal workflow**, **template-driven configs or `--config-file` overrides**, **repeatable dev/test loop**, and shaped as a **stand-alone text file** exactly as you asked.

---

This plan codifies the actions that keep `lifelinetty` field trials fast, reliable, and fully repeatable. It assumes a **Linux desktop** developer connected to a **Raspberry Pi** target over SSH, using the command-line tools described in `devtest/run-dev.sh` with the current roadmap (`docs/Roadmaps/v0.2.0/roadmap.md`) as the context for goals and constraints.

| Element | Value |
| --- | --- |
| Goal | Restartable loop that builds locally, syncs binaries, and runs local vs remote `lifelinetty` builds without leftover processes. |
| Inputs | `devtest/dev.conf`, `devtest/config/*.toml` scenario configs (template copies or `--config-file`), `samples/` payloads. |
| Outputs | Terminal trio (SSH shell, remote runtime, local runtime), cache logs under `/run/serial_lcd_cache`, annotated scenario records in this file. |
| Constraints | Only `/run/serial_lcd_cache` and `~/.serial_lcd/config.toml` for storage; no new CLI flags beyond roadmap-approved set; scripts respect the charter described in `.github/copilot-instructions.md`. |

---

## Overview

1. **Trigger**: a single invocation of `devtest/run-dev.sh` (or a watch script) that reads `devtest/dev.conf` and orchestrates the entire loop.
2. **Primary actions**:
   * Build the binary (debug or release per config).
   * Sync the binary to the Pi (create directories, set execute bit).
   * Ensure no stale `lifelinetty` process is running on the Pi (stop `lifelinetty.service`, kill stray pids).
   * Launch three terminals with clear titles: SSH shell, remote runtime (using scenario TOML copied to `~/.serial_lcd/config.toml`), and local runtime (same binary + config for comparison). When the `--config-file` flag lands in Milestone 5, the same step will pass the override directly instead of copying.
3. **Validation**: confirm the Pi is reachable, the config files exist, and logs remain under `/run/serial_lcd_cache` (watcher scripts keep tabs).
4. **Failure handling**: closing any terminal stops its process; rerunning the script tears down old sessions for a clean slate.

## Directory and Config Expectations

The `devtest/` directory is the anchor for this workflow:

| File | Purpose |
| --- | --- |
| `devtest/dev.conf` | Machine-specific settings (Pi host, username, build targets, config template paths, extra local/remote args, preferred terminal emulator). |
| `devtest/run-dev.sh` | Orchestrates the loop: build → sync → cleanup → terminal launches. |
| `devtest/watch.sh`, `devtest/watch-remote.sh` | Optional watchers that re-run the orchestration on source saves. |

Config handling is flexible:

* For **v0.2.0 Milestone 1**, scenarios are defined by TOML **templates** under `devtest/config/`.
   * `CONFIG_SOURCE_FILE` points at the default template used for both local and remote runs.
   * `LOCAL_CONFIG_SOURCE_FILE` and `REMOTE_CONFIG_SOURCE_FILE` can optionally override the template per side.
   * `devtest/run-dev.sh` copies these templates into `~/.serial_lcd/config.toml` (local temp HOME and remote Pi) before launching the binaries.
* The scripts never hardcode argument values; they delegate runtime behavior to `dev.conf` and the referenced templates.
* The `--config-file` flag is available. `COMMON_ARGS`/`REMOTE_ARGS`/`LOCAL_ARGS` in `dev.conf` can pass scenario TOMLs directly instead of relying on template copies.

This lets you swap UART devices, baud rates, LCD geometries, payloads, logging tweaks, and demo modes simply by editing `dev.conf` or pointing to another TOML.

## Terminal Layout (Mandatory)

1. **Terminal #1 – SSH Remote Shell**: persistent login shell for diagnostics, log inspection, cache wipes, and manual commands. Opened by `run-dev.sh` as an `SSH`-titled window.
2. **Terminal #2 – Remote LifelineTTY Runtime**: auto-ssh into the Pi and run `lifelinetty` with `COMMON_ARGS` + `REMOTE_ARGS`; this terminal remains open to show live hardware behavior and any errors (titled `Remote`).
3. **Terminal #3 – Local LifelineTTY Runtime**: runs the same binary locally for side-by-side comparison; uses identical configs unless explicitly overridden for local-specific args (titled `Local`).

Terminal names/titles are explicit (SSH, Remote, Local) so you never confuse them. Re-running `run-dev.sh` kills the old trio before spawning the new one.

> Optional Terminal #4 (log watcher): monitor `/run/serial_lcd_cache` or `stdout` logs if your workflow demands a dedicated observability window, but the core loop only needs three terminals.

## Workflow Steps (Conceptual)

1. **Pre-flight checks**
   * Load `dev.conf` and resolve `CONFIG_SOURCE_FILE` / `LOCAL_CONFIG_SOURCE_FILE` / `REMOTE_CONFIG_SOURCE_FILE`.
   * Confirm SSH reachability to the Pi.
   * Ensure all referenced files exist locally (binary, config, payloads).
   * Verify `lifelinetty.service` is stopped on the Pi to free the UART.
2. **Local build**
   * Run `BUILD_CMD` (defaults to `make all`, which produces release binaries under `releases/debug/<arch>/lifelinetty`).
   * Fail fast on compile errors.
3. **Binary sync**
   * Create the remote directory if missing.
   * Copy the new `lifelinetty` binary to the Pi.
   * Set the execute bit and confirm ownership.
4. **Remote cleanup**
   * Stop `lifelinetty.service` if enabled.
   * Kill stray `lifelinetty` processes to guarantee a clean run.
5. **Terminal launches**
   * Terminal 1: SSH shell (auto-opened `SSH` window).
   * Terminal 2: remote runtime (auto-ssh + binary via `REMOTE_CMD`).
   * Terminal 3: local runtime (same binary under a temp HOME with its own `~/.serial_lcd/config.toml`).
   * Titles and commands are derived from `dev.conf` so you can add custom flags without editing the script.
6. **Exit handling**
   * Closing a terminal stops its process cleanly.
   * Re-running the loop kills previous processes and starts again from step 1.

## Scenario Presets

Maintain multiple `.toml` presets for quick experimentation:

* `devtest/config/test-16x2.toml`
* `devtest/config/test-20x4.toml`
* `devtest/config/uart-ama0.toml`
* `devtest/config/stress-9600.toml`
* `devtest/config/stress-19200.toml`

Switching the scenario is as simple as editing `dev.conf` to point at another preset. `run-dev.sh` honors the new path immediately via the template copy step; the same paths can also be passed directly to the binary with `--config-file`.

## Logging, Storage, and Safety

* All logs, cache snapshots, and artifacts must stay under `/run/serial_lcd_cache` or `~/.serial_lcd/config.toml` (per the charter).
* The watcher scripts (`watch*.sh`) monitor the RAM-disk to keep you aware of new files and to ensure nothing leaks onto `/etc` or other persistent locations.
* Everything is recoverable: kill sessions, restart `lifelinetty.service`, and reboot the Pi without leaving traces outside the approved directories.
* Logs are easily retrievable via `scp -r "$PI_HOST:/run/serial_lcd_cache" ./tmp/pi-cache-$(date +%s)` for field-ops reviews, as documented in the roadmap’s milestone section.

## Developer Experience Goals

* **One command**: start the loop, have the windows open, and watch the runtimes in sync.
* **Rapid comparison**: local vs remote outputs side-by-side highlight serial, LCD, and payload differences instantly.
* **Zero friction**: no manual terminal management, no forgotten args, and no zombie processes.
* **Safe discipline**: the loop enforces charter guardrails—no writes outside the cache, no unauthorized CLI flags, and no service conflicts.
