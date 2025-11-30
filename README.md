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

## Build requirements & how-to

- Native (x86_64 host): Rust toolchain + build essentials. On Debian/Ubuntu/WSL2: `sudo apt update && sudo apt install -y build-essential pkg-config libudev-dev make`. Run `make x86` (or `cargo build --release`). Output lands in `releases/debug/x86/seriallcd[.exe]`.
- ARMv6 (Pi 1/Zero) via Docker: Docker Desktop/Engine with BuildKit + `buildx` enabled and internet access to pull toolchains. Run `make armv6` to build inside the `docker/Dockerfile.armv6` image and export the runtime filesystem to `releases/debug/armv6` (binary ends up at `releases/debug/armv6/usr/local/bin/seriallcd`). The ARMv6 build now uses an `armv6-linux-musleabihf` toolchain (static musl) to avoid Debian’s ARMv7 baseline that was triggering `Illegal instruction` on real Pi 1/Zero hardware.
- Both at once: `make all` runs the native build and the Docker ARMv6 build.
- Clean artifacts: `make clean` wipes `releases/debug/`.
- If you prefer an image instead of extracted files, use the Dockerfile directly: `docker buildx build --platform linux/arm/v6 -f docker/Dockerfile.armv6 -t seriallcd:armv6 --load .`
- WSL2: ensure Docker Desktop is running and the Docker CLI/socket is available inside WSL2; install the same apt packages above inside WSL2 so `make` and the build toolchain are present.

### Testing / bin smoke check

- `cargo test` (or `make test`) runs unit tests plus a CLI smoke test that executes the built `seriallcd` binary: `seriallcd --version` and `seriallcd run --payload-file samples/test_payload.json`. This keeps the runtime path exercised as part of the build/test flow without needing real serial hardware.

### Config file

Persistent settings live at `~/.serial_lcd/config.toml` and use a simple `key = value` format:

```toml
device = "/dev/ttyAMA0"
baud = 115200
cols = 20
rows = 4
```

CLI flags override config values when provided.

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
