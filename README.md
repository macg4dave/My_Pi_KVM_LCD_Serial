# SerialLCD

SerialLCD is a small Rust daemon that takes newline‑terminated JSON over a serial port and renders it onto an HD44780‑compatible character LCD driven via a PCF8574 I²C backpack (typical Pi / PiKVM LCD setups).

- Runs as a simple foreground process or systemd service.
- Designed for Raspberry Pi (Pi 1 / Zero / PiKVM), but has a stub LCD backend on non‑Linux for development and testing.
- Stable JSON “protocol” with built‑in paging, scrolling, bar graphs, alerts, icons, and config reload hints.

---

## Features

- **Dashboard / normal / banner modes**
  - `normal` (default): two lines of text, optional bar graph on either row.
  - `dashboard`: text plus a bottom‑row bar graph for a key metric (CPU/RAM/disk/etc.).
  - `banner`: line 1 scrolls like a marquee, line 2 is hidden.
- **Bar graphs**
  - Use `bar` (0–100) **or** `bar_value` + `bar_max` to render a horizontal bar with 6 custom fill levels.
  - Optional `bar_label` (“CPU”, “RAM”, “NET”, “DISK”, …) to annotate the metric.
- **Paging and TTL**
  - Each payload can set a `page_timeout_ms`, and the daemon will rotate through a queue of frames.
  - `duration_ms` (or legacy `ttl_ms`) automatically expires entries from the queue.
  - Optional GPIO button (on Linux) can advance pages manually.
- **Scrolling text**
  - Long lines scroll with a gap marker `|` between repeats.
  - `scroll_speed_ms` controls the scroll cadence; `scroll:false` disables scrolling and uses `...` truncation.
- **Alerts and backlight control**
  - `blink:true` makes the backlight toggle; `backlight:false` turns it off entirely.
  - Parse / checksum errors are rendered as `"ERR PARSE"` with a detailed second line and blinking backlight.
- **Heartbeat & offline indicators**
  - When frames stop arriving for a grace period, a small heartbeat icon can appear at the line edge.
  - On serial I/O errors the LCD shows `"SERIAL OFFLINE"`/`"will retry..."` and the daemon retries with backoff.
- **Config reload hints**
  - A payload with `config_reload:true` triggers a reload of `~/.serial_lcd/config.toml` and applies updated defaults
    (baud, device, scroll/page timeouts, backoff tuning) without restarting the daemon.
- **Built‑in demo mode**
  - `--demo` cycles a curated set of example dashboards, alerts, icons and scrolling patterns; useful to verify wiring.

See `docs/architecture.md`, `docs/lcd_features.txt`, and `docs/lcd_patterns.md` for more implementation details.

---

## Requirements

### Runtime (Pi / Linux target)

- Linux system with:
  - I²C bus enabled (typically `/dev/i2c-1` on Raspberry Pi).
  - Serial port exposed to the daemon (e.g. `/dev/ttyAMA0`, `/dev/ttyS0`, or USB serial like `/dev/ttyUSB0`).
- HD44780‑compatible character LCD (e.g. 16x2, 20x4) with a PCF8574 I²C backpack.
- For systemd deployment, a dedicated `seriallcd` user/group is recommended (see `seriallcd.service`).

The binary will **try to auto‑detect the PCF8574 address** by probing common addresses (`0x27`‑`0x20`) when
`pcf8574_addr = "auto"` (default).

### Build from source

- Rust toolchain (edition 2021; recent stable recommended).
- `cargo` for building and testing.
- On Linux target with real hardware: I²C and GPIO access (via `rppal`), and serial port access.

Build a release binary:

```sh
cargo build --release
target/release/seriallcd --help
```

On non‑Linux platforms the LCD backend is a stub that records writes in memory; this is what tests and CI use.

### Docker cross‑build (ARMv6)

For Raspberry Pi 1 / Zero (BCM2835) you can build a static `armv6` binary and container image using Docker BuildKit.
See `docker/README.md` for details; the short version:

```sh
docker buildx build \
  --platform linux/arm/v6 \
  -f docker/Dockerfile.armv6 \
  -t seriallcd:armv6 \
  .
```

---

## CLI usage

The `seriallcd` binary is a small CLI wrapper that wires together config, serial, and the LCD driver.

```text
seriallcd - Serial-to-LCD daemon

USAGE:
  seriallcd run [--device <path>] [--baud <number>] [--cols <number>] [--rows <number>] [--payload-file <path>]
  seriallcd --help
  seriallcd --version

OPTIONS:
  --device <path>   Serial device path (default: /dev/ttyAMA0)
  --baud <number>   Baud rate (default: 115200)
  --cols <number>   LCD columns (default: 20)
  --rows <number>   LCD rows (default: 4)
  --payload-file <path>  Load a local JSON payload and render it once (testing helper)
  --backoff-initial-ms <number>  Initial reconnect backoff (default: 500)
  --backoff-max-ms <number>      Maximum reconnect backoff (default: 10000)
  --pcf8574-addr <auto|0xNN>     PCF8574 I2C address or 'auto' to probe (default: auto)
  --log-level <error|warn|info|debug|trace>  Log verbosity (default: info)
  --log-file <path>              Append logs to a file (also honors SERIALLCD_LOG_PATH)
  --demo                         Run built-in demo pages on the LCD (no serial input)
  -h, --help        Show this help
  -V, --version     Show version
```

Notes:

- You can omit the explicit `run` subcommand: e.g. `seriallcd --device /dev/ttyUSB0 --cols 16 --rows 2`.
- CLI flags override values from the config file (see below) where applicable.

---

## Configuration file

At startup the daemon loads a TOML config from:

- `~/.serial_lcd/config.toml`

If the file does not exist, it is created with sensible defaults. The schema is:

```toml
# seriallcd config
device = "/dev/ttyAMA0"
baud = 115200
cols = 20
rows = 4
scroll_speed_ms = 250
page_timeout_ms = 4000
button_gpio_pin = null       # or e.g. 17 for a GPIO button (Linux only)
pcf8574_addr = "auto"        # or "0x27" / "39" (hex or decimal)
backoff_initial_ms = 500
backoff_max_ms = 10000
```

Unknown keys cause an error; the daemon prefers **config file values** when a CLI flag is not provided.

You can trigger a live reload of this file by sending a payload with `config_reload:true` (see JSON format below).

---

## Environment variables

- `HOME` — used to locate `~/.serial_lcd/config.toml`.
- `SERIALLCD_LOG_LEVEL` — overrides the log level (`error|warn|info|debug|trace`).
- `SERIALLCD_LOG_PATH` — path to a log file; logs are appended. This can also be set via `--log-file`.

---

## JSON payload format (v1)

The daemon ingests **one JSON object per line** on the serial port. Each raw frame must be **≤ 512 bytes**
(`MAX_FRAME_BYTES`); larger frames are rejected.

Required fields:

- `line1` (`string`) — text for row 0.
- `line2` (`string`) — text for row 1 (may be empty; ignored in `banner` mode).

Common optional fields:

- `version` (`int`) — protocol version. When present, only `1` is accepted.
- `bar` (`0‑100`) — direct percent for the bar graph.
- `bar_value` (`u32`) — current value; used with `bar_max` to compute a percent.
- `bar_max` (`u32`) — maximum for `bar_value` (default `100`, must be `>= 1`).
- `bar_label` (`string`) — short label (e.g. `"CPU"`, `"MEM"`) displayed with the bar.
- `bar_line1` (`bool`) — request bar on line 1 (top).
- `bar_line2` (`bool`) — request bar on line 2 (bottom, default when a bar is present).

Display / behavior flags:

- `backlight` (`bool`) — turn backlight on/off (default `true`).
- `blink` (`bool`) — blink the backlight (default `false`).
- `scroll` (`bool`) — enable or disable scrolling for long lines (default `true`).
- `scroll_speed_ms` (`u64`) — scroll speed in milliseconds (default `250`).
- `duration_ms` (`u64`) — how long this frame stays valid; after this it expires.
- `ttl_ms` (`u64`) — legacy alias for `duration_ms`.
- `page_timeout_ms` (`u64`) — how long to keep this frame on screen before paging to the next (default `4000`; must be > 0).
- `clear` (`bool`) — clear the display before rendering this frame (default `false`).
- `test` (`bool`) — draw a test pattern to verify wiring/backlight (default `false`).
- `mode` (`string`) — one of `"normal"`, `"dashboard"`, `"banner"` (default `"normal"`).
- `icons` (`array<string>`) — subset of `["battery", "arrow", "heart"]`; unknown entries are ignored.

Integrity and control:

- `checksum` (`string`) — hex CRC32 of the canonical payload **with `checksum` omitted**.
  - On mismatch the frame is rejected and the LCD shows a parse error.
- `config_reload` (`bool`) — when `true`, the daemon reloads the config file and updates defaults.

Additional behavior:

- Payloads longer than `MAX_FRAME_BYTES` are rejected and do **not** affect the current screen.
- Identical payloads (by CRC of the raw JSON text) are de‑duplicated to avoid unnecessary flicker.
- In `dashboard` mode, any bar is forced onto the **bottom row** even if `bar_line1:true` is set.
- In `banner` mode, `line2` is cleared and only `line1` scrolls.
- If `scroll:false` and a line is too long, it is truncated with an ellipsis (`...`).
- Unknown `mode` values fall back to `normal`; unknown `icons` are ignored.

You can embed custom LCD glyphs by using `{0xNN}` placeholders inside `line1`/`line2` where `NN` is the CGRAM slot
(0–7). Slots `0–5` are used by the bar graph, `6` by the heartbeat icon, and `7` by the battery icon.

---

## JSON examples

### Dashboard with bottom bar

```json
{"version":1,"line1":"Up 12:34  CPU 42%","line2":"RAM 73%","bar_value":73,"bar_max":100,"bar_label":"RAM","mode":"dashboard","page_timeout_ms":6000}
```

### Simple alert that expires

```json
{"version":1,"line1":"ALERT: Temp","line2":"85C","blink":true,"duration_ms":5000}
```

### Network metric with icon and top bar

```json
{"version":1,"line1":"NET {0x00} 12.3Mbps","line2":"bar on top","bar":65,"bar_line1":true,"icons":["battery"]}
```

### Banner (line 2 ignored)

```json
{"version":1,"line1":"Long banner text that scrolls","line2":"ignored","mode":"banner","scroll_speed_ms":220}
```

### Backlight off

```json
{"version":1,"line1":"Backlight OFF demo","line2":"It should go dark","backlight":false}
```

### Clear + test pattern

```json
{"version":1,"line1":"Clear + Test Pattern","line2":"Ensure wiring is OK","clear":true,"test":true}
```

### Scroll disabled, custom page timeout

```json
{"version":1,"line1":"Scroll disabled","line2":"This line stays put","scroll":false,"page_timeout_ms":4500}
```

### Config reload hint

```json
{"version":1,"line1":"Config reload hint","line2":"Reload config now","config_reload":true}
```

More structured samples live in:

- `samples/payload_examples.json`
- `samples/test_payload.json`

---

## Feeding payloads

### One‑shot from disk (no serial input)

Render a single payload JSON file and exit:

```sh
seriallcd run --device /dev/ttyUSB0 --cols 16 --rows 2 --payload-file samples/test_payload.json
```

### Streaming over a PTY for development

Use a pseudo‑terminal pair so you can feed JSON from a script:

```sh
socat -d -d pty,raw,echo=0 pty,raw,echo=0
```

Take note of both PTY paths (e.g. `/dev/pts/3` and `/dev/pts/4`).

- Point `seriallcd` at one end (e.g. `/dev/pts/3`).
- Write JSON lines to the other end from a script or REPL (Python, Ruby, etc.).

### Real hardware

Your upstream software should send **newline‑terminated JSON objects** (UTF‑8) to the configured serial port.
Frames that cannot be parsed, fail validation, or exceed `MAX_FRAME_BYTES` are rejected and do not corrupt the
render queue.

---

## Systemd service

A sample unit file is included as `seriallcd.service`. Typical installation on a Pi might look like:

```sh
sudo useradd --system --no-create-home --group nogroup seriallcd || true
sudo install -m 0755 target/release/seriallcd /usr/local/bin/seriallcd
sudo install -m 0644 seriallcd.service /etc/systemd/system/seriallcd.service
sudo systemctl daemon-reload
sudo systemctl enable --now seriallcd.service
```

The provided unit is locked down to only allow read/write within `/run/serial_lcd_cache` while keeping the rest of
the system mounted read-only (`ProtectSystem=strict`).

---

## Packaging & releases

- Local Debian/RPM packaging metadata is included in `Cargo.toml`; see `docs/releasing.md` for the full workflow.
- `scripts/local-release.sh` builds the release binary, `.deb`, and `.rpm` into `releases/<version>/` with names like
  `seriallcd_v0.5_armv6.{deb,rpm}` plus the raw binary, and can push them to GitHub when `--upload` (or `--all`)
  is passed (requires the `gh` CLI and an existing git tag for the version). Use `--all-targets` to build host + armv6/armv7/arm64;
  the ARM builds run via Docker Buildx so no local cross toolchain is needed.

---

## Development & testing

Run unit and integration tests:

```sh
cargo test
```

Key areas covered by tests:

- CLI parsing (`src/cli.rs`).
- Config load/save and schema validation (`src/config`).
- JSON payload parsing and validation (`src/payload`).
- Render state and queue behavior (`src/state.rs`).
- Serial + LCD integration using the fake serial port and stub LCD (`tests/fake_serial_loop.rs`).

This makes it possible to evolve the daemon while keeping the JSON/CLI contracts stable.
