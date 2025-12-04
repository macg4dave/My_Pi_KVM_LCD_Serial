# Architecture

Mission reminder: ship a single Rust daemon that ingests newline-delimited payloads over a 9600 baud
serial link, renders two HD44780 rows via a PCF8574 I²C backpack, and runs for months without
breaking 5 MB RSS. Everything in this document maps the roadmap milestones back to concrete files so
new contributors can find the right module quickly.

## High-level data flow

```text
           CLI/config
                │
      ┌─────────▼─────────┐
      │  App lifecycle    │  src/app/lifecycle.rs
      └─────────┬─────────┘
                │
      ┌─────────▼─────────┐
      │ Serial pipeline   │  src/serial/*
      │ (async + backoff) │
      └─────────┬─────────┘
                │ JSON / key=value
      ┌─────────▼─────────┐
      │ Payload parser    │  src/payload/*
      └─────────┬─────────┘
                │
      ┌─────────▼─────────┐
      │ Render loop       │  src/app/render_loop.rs
      │ + IconBank        │  display/* + payload/icons.rs
      └─────────┬─────────┘
                │
      ┌─────────▼─────────┐
      │ LCD driver + I²C  │  src/display/lcd.rs + lcd_driver/*
      └───────────────────┘
```

Every payload—demo, polling snapshot, or live serial frame—walks the exact same path. The render
loop owns state, debounces identical frames, routes alerts to overlays, and commits rows to the LCD.

## Core modules

- **CLI (`src/cli.rs`)**: Parses `lifelinetty` flags (`--run`, `--demo`, `--wizard`, etc.), merges
  environment overrides, and hands a normalized `AppConfig` to the lifecycle layer.
- **Config loader (`src/config/`)**: Reads `~/.serial_lcd/config.toml`, enforces guardrails (cols
  8–40, rows 1–4, scroll ≥100 ms), survives partial files, and persists wizard answers.
- **Lifecycle (`src/app/lifecycle.rs`)**: Bootstraps logging, Ctrl+C handling, cache directories, and
  whichever operating mode was requested (run, demo, wizard, serial shell, tests).
- **Serial stack (`src/serial/`)**: Provides sync/async transports, reconnect backoff, telemetry, and
  fake transports for tests. Frames are newline-delimited JSON or `key=value` pairs; compression
  envelopes are normalized before parsing.
- **Payload parser (`src/payload/`)**: Validates `schema_version`, decodes envelopes, and converts JSON
  into strongly-typed `Payload` structs. Strict mode enforces known keys and emits duplicates for
  dedupe.
- **Render loop (`src/app/render_loop.rs`)**: Applies dedupe, schedules scrolling/paging timers, tracks
  blinking state, and coordinates overlays (heartbeat, alerts, demo banners, polling snapshots).
- **IconBank + overlays (`display/`, `payload/icons.rs`)**: Curated glyph catalog, CGRAM allocator, and
  overlay helpers (e.g., heartbeat, navigation arrows). Bars reuse the same partial-block table so icon
  usage stays within the 8-slot limit.
- **LCD driver (`src/display/lcd.rs`, `lcd_driver/`)**: Wraps `hd44780-driver` or the legacy driver
  depending on configuration, handles I²C retries, and exposes friendly APIs (`write_line`, `set_cursor`,
  `set_backlight`).
- **Support services**: polling agent (`src/app/polling.rs`), command tunnel (`src/app/command_tunnel/*`),
  compression helpers, telemetry exporters, and watchdogs. Each lives under `src/app/` or
  `src/milestones/` and is gated behind roadmap flags.

## Storage + cache boundaries

- Persistent configuration lives **only** at `~/.serial_lcd/config.toml`.
- Everything transient (logs, protocol errors, wizard transcripts, serial telemetry, tunnel buffers)
  belongs under `/run/serial_lcd_cache` as enforced by the systemd unit and `CACHE_DIR` constant.
- No module may create directories outside the RAM disk or call `mount`. Helpers in `app::logger`
  and `app::watchdog` already constrain paths accordingly.

## Demo + testing surfaces

- `src/app/demo.rs` reuses the full render stack without opening serial ports so installers can verify
  wiring (`lifelinetty --demo`). The playlist mirrors edge cases (scrolling, icon saturation, bar relayouts).
- Integration tests under `tests/` exercise fake serial transports, CLI flag parsing, and filesystem
  boundaries. Use `cargo test -- --test-threads=1` on ARMv6 targets to keep memory usage predictable.
- For manual payload experiments, point `--payload-file` at entries in `samples/payload_examples.json`
  or craft your own using the guidelines in `docs/demo_playbook.md`.

## Error handling + observability

- All errors bubble up as `anyhow::Error` (CLI) or typed errors (library modules). No bare `unwrap()`
  calls remain in production paths; temporary instrumentation must include context or be removed before
  landing.
- Serial reconnects emit machine-readable JSON to
  `/run/serial_lcd_cache/serial_backoff.log`. Parser/compression failures land in
  `/run/serial_lcd_cache/protocol_errors.log`. Polling events append to `polling/events.log`.
- Logging defaults to stderr; opt into cache files with `--log-file` or `LIFELINETTY_LOG_PATH`, both of
  which force the path underneath the RAM disk.

## Testing strategy

- Unit tests cover parsers, payload schema evolution, and CGRAM allocation.
- Integration tests (`tests/*.rs`) drive the CLI, fake serial devices, and the render loop with recorded
  payloads. They run under CI plus `scripts/test_makefile_paths.sh` / `test_localrelease_paths.sh` to
  guarantee packaging correctness.
- Manual smoke tests: `lifelinetty --demo` for LCD validation, `lifelinetty --test-serial` for loopback
  verification, and `lifelinetty --serialsh` for Milestone G tunnel exercises.

Reference docs: `docs/demo_playbook.md` (demo frames), `docs/icon_library.md` (glyph catalog),
`docs/lcd_patterns.md` (UI recipes), and roadmap-specific write-ups in `docs/milestone_*.md`.
