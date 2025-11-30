# Architecture (skeleton)

Goal: a small daemon that converts local status into LCD lines over a serial link. Keep it single-process, single-binary, no network.

## Components
- CLI (`src/cli.rs`): parses `seriallcd run` flags: `--device`, `--baud`, `--cols`, `--rows`. Defaults: `/dev/ttyAMA0`, `115200`, `20x4`.
- App (`src/app.rs`): owns configuration, opens serial, initializes LCD, and runs the refresh loop (to be implemented).
- Serial (`src/serial.rs`): placeholder for UART transport; will frame and send bytes to the LCD controller or bridge MCU.
- LCD (`src/lcd.rs`): placeholder for line/row writes and boot messages; will translate text into controller commands.
- Config (`src/config.rs`): loads `~/.serial_lcd/config.toml`, merges with CLI overrides, and exposes resolved settings.

## Invariants to keep
- No network IO.
- Only one binary: `seriallcd`.
- CLI and protocol stability: once flags and serial framing are defined, treat them as stable contracts.
- Clear error handling: prefer typed errors; no `unwrap()` in production paths.

## Open design questions (fill before coding further)
- Exact LCD controller/protocol (e.g., HD44780-compatible via UART backpack?).
- Serial framing (raw text with newline? prefixed lengths? checksums?).
- Refresh cadence and data source (what metrics to display and from where).
- Logging policy (where to log, acceptable verbosity).

## Testing strategy
- Unit tests for parsing, framing, and formatting.
- Optional simulator for serial transport to allow CI without hardware.
- Integration tests that drive the CLI with sample config and assert emitted bytes (once protocol is fixed).
