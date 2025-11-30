# Project — Codex/Copilot Charter
Purpose: keep this AI-only Rust project on-scope. All decisions below are final; do not invent extra features.

## One-line mission
A minimal, ultra-light Rust daemon for a Raspberry Pi 1 that reads structured data over UART and displays formatted status text on a 16×2 I²C LCD with absolute reliability and minimal resource usage.

## In-scope MVP (only these outcomes)
Core user flow 1:
Read incoming serial data from /dev/ttyAMA0 (115200 8N1), parse newline-terminated JSON or key=value messages, and handle malformed input gracefully.

Core user flow 2:
Render two lines of text to a 16×2 HD44780 LCD via PCF8574 I²C backpack, refreshing efficiently without flicker and without CPU spikes.

Core user flow 3:
Run as a systemd service on boot, with stable long-running behaviour, zero crashes, and tiny RAM footprint (target < 5 MB RSS).

Target platform:
Raspberry Pi 1 Model A — ARMv6, Debian minimal install, systemd.

Interface:
CLI only, with these commands:
--test-lcd → write test messages to the LCD
--test-serial → dump raw serial input to stdout
--run → start main daemon loop (reads serial, drives LCD)
No TUI. No REPL. No HTTP server. No GUI.

IO/protocols:
Input: UART serial (/dev/ttyAMA0, 115200 baud)
Output: 16×2 character LCD via I²C (PCF8574, address 0x27)
No Wi-Fi, no Bluetooth, no sockets, no cloud.
No USB HID.

Storage/config:
Only local config files under ~/.serial_lcd/
Files: config.toml (read/write allowed — this is the only persistent write exception), optional local log (ramdisk only)
No database.
No cache.
No state outside this folder.

## Runtime RAM-disk / cache policy (MANDATORY)

The program must never write to the SD card or persistent storage, **except** the single config file at `~/.serial_lcd/config.toml` on the Raspberry Pi.

All temporary files, logs, and caches must be stored in a RAM-disk
created and owned by systemd, not by the application.

The application itself must not:
- call `mount`
- create tmpfs
- require sudo/root
- modify `/etc/fstab`
- perform any disk I/O outside the approved RAM path

Systemd will provision a private tmpfs at:
    /run/serial_lcd_cache

This directory will exist at runtime and will be owned by the service user.
The application may:
- read/write files inside `/run/serial_lcd_cache`
- read/write the config file at `~/.serial_lcd/config.toml` (the only persistent exception)
- assume ~100MB of tmpfs space
- clean up its own temporary files

The application must not:
- write anywhere else (persistent writes are only for the config file)
- attempt to remount or resize the tmpfs
- assume permanence (all data is wiped on reboot)

All path handling must hard-code the RAM-disk root:
    const CACHE_DIR: &str = "/run/serial_lcd_cache";

All logging must go to stderr or files inside this RAM-disk only.

No persistent state must ever be created.

## Out of scope (never do these)
* No new interfaces beyond the one listed above unless explicitly approved.
* No network calls unless explicitly stated.
* No speculative features or refactors without a written request.

## Tech and dependencies
* Language: Rust (stable). Edition: 2021.
* Allowed crates: std, hd44780-driver, linux-embedded-hal, rppal (I2C), serialport, tokio-serial (feature `async-serial`), tokio (only for async serial), anyhow/thiserror/log/tracing once requested explicitly.
* Banned crates: anything that pulls in a runtime or network stack unless explicitly requested (no reqwest, hyper, surf, mqtt, websockets, databases).
* Build/test commands: `cargo build`, `cargo test`; optionally `cargo fmt`, `cargo clippy`.
* Runtime environment: Raspberry Pi OS on ARMv6 (Pi 1), systemd-managed service user, serial at `/dev/ttyAMA0` (115200 8N1), I2C LCD at address `0x27` via PCF8574.

## Interfaces that must stay stable
* CLI binary name and flags: `seriallcd --run`, `--test-lcd`, `--test-serial`, plus `--device`, `--baud`, `--cols`, `--rows`. Do not rename without approval.
* Config/schema/contracts: `~/.serial_lcd/config.toml` (future), serial framing: newline-terminated JSON or `key=value` records; I2C LCD uses HD44780 command set via PCF8574.
* Outputs: stderr logging only; LCD contents are two-line text; exit codes: 0 on success, non-zero on fatal errors.

## Quality bar
* Tests: every behavioral change adds/updates tests; no regressions.
* Formatting/lints: `rustfmt`; `clippy` must be clean or documented if skipped.
* Safety: avoid `unsafe`; avoid `unwrap()` outside tests/examples unless justified.
* Performance/reliability: target <5 MB RSS; avoid busy loops; handle serial/LCD errors with retries and backoff; never crash the daemon loop.

## Task request template (use for every ask)
Task:
"""
<One-line summary of the change>

Details:
- What to change: <short description and acceptance criteria>
- Files to consider: <list or leave blank>
- Tests: <which tests to add/update or leave blank>
- Constraints / do not modify: <files/behaviors to keep unchanged>
"""

## Agent rules (apply to every change)
- If scope is unclear or conflicts with "Out of scope," ask before coding.
- Make the smallest change that meets the acceptance criteria.
- Preserve stable interfaces and outputs unless explicitly allowed to change.
- Add or update tests for behavior changes and include `cargo test` output.
- Document user-facing changes (Rustdoc/README) when behavior shifts.
- No feature creep: do not add capabilities not listed in "In-scope MVP."

## development machine
Target platform is Raspberry Pi 1 Model A (ARMv6) BCM2835. Development can occur on an x86_64 Linux or macOS machine using cross-compilation or QEMU emulation docker. Ensure all builds and tests pass on the target ARMv6 architecture before finalizing changes.
