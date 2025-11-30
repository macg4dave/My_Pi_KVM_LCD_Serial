# SerialLCD (skeleton)

Serial-to-LCD daemon for Raspberry Pi / PiKVM style targets. It reads local status (to be defined) and sends lines over a serial link to a character LCD. This repository is intended to be driven entirely by Codex/Copilot; scope is locked down so the AI stays on target.

## Scope
- In scope: single binary `seriallcd` that drives a character LCD over a serial/UART link. Configuration is local only (CLI flags or a future config file).
- Out of scope: networking, cloud sync, telemetry, remote control, GUI, databases, authentication systems, or additional binaries unless explicitly requested.
- Target platform: Raspberry Pi OS (ARM); serial device such as `/dev/ttyAMA0` or `/dev/ttyUSB0`.
- Interface: CLI subcommand `run` with flags for serial device, baud, and LCD geometry.

Adjust the above if your hardware differs; keep the list explicit so the AI avoids feature creep.

## Layout
- `src/main.rs` — binary entrypoint.
- `src/lib.rs` — shared types and error handling.
- `src/cli.rs` — minimal CLI parser and defaults.
- `src/app.rs` — daemon wiring (placeholder).
- `src/serial.rs` — stub serial transport.
- `src/lcd.rs` — stub LCD driver.
- `seriallcd.service` — example systemd unit.
- `.github/instructions/` — prompt templates and AI guardrails.

## Usage (skeleton)
```sh
cargo build
cargo run -- run --device /dev/ttyUSB0 --baud 9600 --cols 16 --rows 2
```

## Dependencies (allowed)
- `hd44780-driver` for the LCD controller.
- `linux-embedded-hal` and `rppal` for Raspberry Pi I²C/hal support.
- `serialport` for synchronous UART.
- `tokio-serial` (behind the `async-serial` feature) and `tokio` for optional async serial handling.

## Next steps for Codex/Copilot
1) Confirm the hardware protocol: serial framing, LCD commands, and data source (what text goes to the display).  
2) Fill `.github/copilot-instructions.md` TODOs with concrete values (allowed crates, stable CLI flags, hardware assumptions).  
3) Implement the real serial transport and LCD command set; add integration tests or a simulator.  
4) Wire the daemon loop to pull the desired status metrics and refresh the display.  
5) Update the systemd unit if paths/users differ from your target environment.

## Docker cross-build (ARMv6)
See `docker/README.md` for a BuildKit-based flow to produce an `armv6` image targeting Raspberry Pi 1 / BCM2835:
```sh
docker buildx build --platform linux/arm/v6 -f docker/Dockerfile.armv6 -t seriallcd:armv6 .
```
