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
No network.
No Wi-Fi, no Bluetooth, no sockets, no cloud.
No USB HID.

Storage/config:
Only local config files under ~/.serial_lcd/
Files: config.toml, optional local log
No database.
No cache.
No state outside this folder.

## Runtime RAM-disk / cache policy (MANDATORY)

The program must never write to the SD card or persistent storage.

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
- assume ~100MB of tmpfs space
- clean up its own temporary files

The application must not:
- write anywhere else
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
* Language: Rust (stable). Edition: TODO (2021/2024).
* Allowed crates: TODO (list permitted crates; default to std + <common choices>).
* Banned crates: TODO (if any).
* Build/test commands: `cargo build`, `cargo test`; optionally `cargo fmt`, `cargo clippy`.
* Runtime environment: TODO (OS/hardware assumptions, serial device paths, etc.).

## Interfaces that must stay stable
* CLI binary name and flags: TODO (enumerate).
* Config/schema/contracts: TODO (file format, env vars, serial protocol framing).
* Outputs: TODO (log formats, exit codes, user-visible text that must not change).

## Quality bar
* Tests: every behavioral change adds/updates tests; no regressions.
* Formatting/lints: `rustfmt`; `clippy` must be clean or documented if skipped.
* Safety: avoid `unsafe`; avoid `unwrap()` outside tests/examples unless justified.
* Performance/reliability: TODO (budgets, retry rules, timeouts if relevant).

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
