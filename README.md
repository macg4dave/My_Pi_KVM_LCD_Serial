# LifelineTTY —

---

## Quick Start (Raspberry Pi)

These steps get you running fast.

## 1. Download & Install

### Pi OS (Pi 1/Zero/2/3/4/5) — Easy `.deb` install

```sh
wget https://github.com/macg4dave/LifelineTTY/releases/latest/download/lifelinetty_arm.deb
sudo apt install ./lifelinetty_arm.deb
```

Works on:

- Pi 1  
- Pi Zero / Zero 2  
- Pi 2 / 3 / 4 / 5  
- 32-bit or 64-bit Pi OS  

If you’re not on a Pi, download the correct binary from Releases.

---

## 2. Wire the LCD (I²C)

Works with **all HD44780 character LCDs**:

- 16×2
- 20×4
- 16×4
- 40×2  
And more.

Wire the PCF8574 backpack like this:

| LCD Backpack | Raspberry Pi |
|--------------|--------------|
| GND          | GND          |
| VCC          | 5V           |
| SDA          | GPIO 2 (SDA) |
| SCL          | GPIO 3 (SCL) |

I²C must be enabled:

```sh
sudo raspi-config
# Interface Options → I2C → Enable
```

---

## 3. Test the LCD (No JSON Needed)

Just run:

```sh
lifelinetty --demo
```

You’ll see:

- scrolling text  
- bar graphs  
- icons  
- blinking alerts  
- paging  
- test patterns  

If you see animations, your wiring is perfect.

`--demo` is your best friend. For a shot-by-shot breakdown of every playlist frame plus tips for
building your own sample payloads, see [`docs/demo_playbook.md`](docs/demo_playbook.md).

Note for builders: the included `Makefile` and `scripts/local-release.sh` will prefer native host builds when your machine matches the requested target (for example, building arm64 on an aarch64 host). Set `FORCE_DOCKER=1` to force the Docker cross-build path if needed.

---

## Sending JSON (This is the real magic)

LifelineTTY listens for **one JSON object per line** over a serial port.


### Icons and overlays

LifelineTTY now ships with a curated HD44780 icon registry and a runtime CGRAM
bank manager so you can request meaningful glyphs without hand-crafting custom
bytes. Send an `icons` array in your payload and the render loop hot-swaps the
needed bitmaps into the LCD before each render pass, falling back to ASCII when
the request would exceed the hardware’s custom slot budget.

Current semantic icon names (case/spacing/hyphen normalizations are accepted)
include:

```text
battery, heart, wifi, arrow, bell, note, clockface, duck, check, cross, smile,
open_heart, up_arrow, up_arrow_right, up_arrow_left, down_arrow,
down_arrow_right, down_arrow_left, return_arrow, hourglass, degree_symbol,
degree_c, degree_f
```

Each payload can request up to **four** icons so the daemon keeps the eight-slot
CGRAM bank free for bar/heartbeat overlays. When you send more than four names,
the extras are dropped and the daemon falls back to the ASCII placeholder for
each glyph (e.g., `wifi` becomes `w`, `heart` becomes `h`). Set
`LIFELINETTY_LOG_LEVEL=debug` or `--log-level debug` to see icon saturation
warnings in `/run/serial_lcd_cache` and trim the offending names if needed.
Unknown names are ignored entirely, so typos such as `"batery"` simply omit
that icon rather than crashing the daemon. For the full catalog,
attribution, and row-by-row data, see [`docs/icon_library.md`](docs/icon_library.md);
the `samples/payload_examples.json` file also includes a ready-made icon test
frame you can adapt to validate new combinations.

Strict mode (enabled by including `schema_version`) also rejects payloads that
contain fields the current schema does not define. Keep keys tidy—typos like
`"icon"` instead of `"icons"` or extra fields copied from other dashboards
will trigger a validation error and drop the frame.

```json
{"schema_version":1,"line1":"Wi-Fi","line2":"Icons!","icons":["wifi","battery"]}
{"schema_version":1,"line1":"NAV","line2":"↕","icons":["up_arrow","down_arrow_left","return_arrow","hourglass"],"bar":42}
```


### Schema versioning and strict mode

You must include a `schema_version` field to enable strict validation rules (for future-proofing and compatibility): missing `schema_version` will cause the payload to be rejected.

### Migration note

Starting with this release, every JSON payload must include `schema_version`. If you have automation or scripts that previously sent bare objects, add `"schema_version":1` to your payloads. Example:

```json
{"schema_version":1,"line1":"Hello","line2":"World"}
```

Examples:

### Simple text

```json
{"schema_version":1,"line1":"Hello","line2":"World"}
```

### Dashboard

```json
{"schema_version":1,"mode":"dashboard","line1":"CPU 42%","line2":"RAM 73%","bar":73}
```

### Banner marquee

```json
{"schema_version":1,"mode":"banner","line1":"Scrolling across the LCD..."}
```

### Alert with blinking backlight

```json
{"schema_version":1,"line1":"TEMP ALERT","line2":"85C","blink":true}
```

### Turn backlight off

```json
{"schema_version":1,"line1":"Lights out","line2":"","backlight":false}
```

### Negotiation & command tunnel

Before the first render frame reaches the LCD, LifelineTTY writes `INIT` and
exchanges `hello` / `hello_ack` control frames. Peers advertise capability bits
(tunnel, heartbeat, compression), the `Negotiator` compares `node_id` +
`preference` (`prefer_server`, `prefer_client`, `no_preference`), and the two
roles are elected deterministicly. All events are recorded in
`/run/serial_lcd_cache/logs/negotiation.log` so you can audit why a node became a
server or client, and the `[negotiation]` block in
`~/.serial_lcd/config.toml` mirrors what the wizard stored for `node_id`,
`preference`, and `timeout_ms`. The top-level `command_allowlist` array limits
which programs the tunnel server is allowed to spawn, regardless of what the
peer requested.

When your daemon winds up as the command-server, every `command` frame carries a
CRC32 and a `message` array that can be one of the `CommandMessage` variants
(`Request`, `Chunk`, `Exit`, `Busy`, `Error`, `Heartbeat`, `Ack`). `Request`
frames include a `scratch_path` that must live under `/run/serial_lcd_cache` and
stay within 256 bytes so command output is always jailed in RAM. `Chunk` frames
stream stdout/stderr, `Exit` reports the final code, and `Busy` alerts the client
when the previous command is still running. The `--serialsh` shell (or any
custom tunnel client) sends these frames, while the server replies using the same
channel.

Example command request frame:

```json
{
  "channel": "command",
  "schema_version": 1,
  "message": {
    "type": "request",
    "request_id": 42,
    "cmd": "tail -n 20 /run/serial_lcd_cache/polling/events.log",
    "scratch_path": "/run/serial_lcd_cache/tunnel/request-42"
  },
  "crc32": 3864358418
}
```

See `samples/payload_examples.json` for ready-made `hello`, `hello_ack`, and
command-frame snippets, and peek at `samples/payload_examples.old.json` for the
journal-style summary of a tunnel-aware dashboard.

### Compression envelopes (Milestone F / P14)

When `--compressed` (or `[protocol].compression.enabled = true`) is enabled, upstream senders can
wrap each payload in a tiny envelope so the UART only ships compressed bytes:

```json
{
  "type":"compressed",
  "schema_version":1,
  "codec":"lz4",
  "original_len":64,
  "data":"BASE64-LZ4-BYTES"
}
```

- `schema_version` tracks the envelope itself (must match the daemon’s configured value).
- `codec` must be one of `lz4`, `zstd`, or `none`.
- `original_len` protects against truncated base64 blobs; mismatches are rejected.
- `data` is base64-encoded LZ4/Zstd bytes; `serde_bytes` handles the encoding automatically if
  you serialize a `Vec<u8>`/`ByteBuf`.

The render loop now normalizes envelopes before deduplication, so the same logical payload counts
as a duplicate whether it was sent compressed or plain-text. Malformed envelopes never crash the
daemon—instead they are logged to `/run/serial_lcd_cache/protocol_errors.log` (auto-rotated at
256 KB) and the LCD shows the usual parse error overlay.

**Everything** the display can do is driven by JSON.

---

## Hardware polling overlay (P11)

The daemon now ships with an optional hardware polling agent that samples the
local CPU, memory, disk, and temperature every few seconds without breaking the
5 MB RSS ceiling. Enable it via `polling_enabled = true` in
`~/.serial_lcd/config.toml` or pass `--polling` on the CLI (use `--no-polling`
to override the config), and adjust `poll_interval_ms` / `--poll-interval-ms`
to any value between 1000 ms and 60 000 ms (default 5000 ms).

When polling is enabled the LCD automatically shows a live system snapshot while
the serial link is offline or before the first JSON frame arrives, so techs can
see host vitals even if the upstream sender is quiet. Line 1 highlights
`CPU`/`MEM` percentages (prefixed with `RC` when reconnecting) and line 2 shows
disk usage, the most recent temperature probe, and available disk space in MB.

Each snapshot (and any poller error) is appended to
`/run/serial_lcd_cache/polling/events.log` for later inspection; the log lives
entirely inside the RAM disk, so nothing persistent ever touches the rootfs.

---

## Sending the JSON (TODO — Sister Program Coming)

Soon there will be a small companion tool that:

- auto-detects the daemon  
- structures JSON for you  
- sends system metrics  
- gives a GUI + CLI interface  

TODO: lifelinetty‑send — placeholder section.

---

## Config File (Auto-generated)

Stored at:

```text
~/.serial_lcd/config.toml
```

By default the daemon listens on `/dev/ttyUSB0` at 9600 8N1. LifelineTTY always starts at 9600 (the enforced minimum) before any higher-speed tuning happens, and the Milestone I first-run wizard now launches automatically the moment `~/.serial_lcd/config.toml` is missing. The wizard walks you through device selection, baud validation, LCD geometry, and role preference, then writes a summary to `/run/serial_lcd_cache/wizard/summary.log`. Edit the config (or pass CLI flags) to point at `/dev/ttyAMA0`, `/dev/ttyS0`, USB adapters, or any other TTY that exposes your sender, and re-run the wizard any time with `lifelinetty --wizard` if you want to revisit those choices.

Example:

```toml
device = "/dev/ttyUSB0"
baud = 9600
flow_control = "none"
parity = "none"
stop_bits = "1"
dtr_on_open = "auto"
serial_timeout_ms = 500
cols = 20
rows = 4
scroll_speed_ms = 250
page_timeout_ms = 4000
pcf8574_addr = "auto"
display_driver = "auto"
button_gpio_pin = null
backoff_initial_ms = 500
backoff_max_ms = 10000

[protocol]
schema_version = 1
compression = { enabled = false, codec = "lz4" }

[negotiation]
node_id = 1
preference = "no_preference"
timeout_ms = 1000
 
command_allowlist = []
```

The `[protocol]` section locks the schema version (currently `1`) and lets you request
compression by default. Set `compression.enabled = true` when both peers have negotiated the
same codec via CLI/config (`lz4` today, `zstd` when enabled). The daemon still accepts plain
JSON frames even when compression is disabled, but the flag ensures upstream senders only ship
compressed envelopes when you opt in.

Use `display_driver = "auto"` (default) to stick with the in-tree PCF8574 driver until the
hd44780-driver rollout finishes. Set it to `"hd44780-driver"` to force the external crate on
Linux builds or `"in-tree"` to explicitly keep the legacy path for troubleshooting.

Advanced serial knobs — `flow_control`, `parity`, `stop_bits`, `dtr_on_open`, and
`serial_timeout_ms` — mirror the CLI flags below so you can keep everything at
9600 8N1 or match whatever framing your sender expects (e.g., asserting DTR for
modems or honoring XON/XOFF).

Reload config without restarting the daemon:

```json
{"schema_version":1,"config_reload":true}
```

---

## Storage & cache policy

- Persistent settings live at `~/.serial_lcd/config.toml` (auto-created the first time you run the daemon).
- Everything else (logs, payload caches, telemetry snapshots, LCD caches) belongs in the RAM disk mounted at `/run/serial_lcd_cache`. The provided systemd unit already restricts writes to that directory.
- The `--log-file` flag and `LIFELINETTY_LOG_PATH` environment variable only accept paths inside `/run/serial_lcd_cache`. Provide an absolute cache path or a relative name (e.g., `logs/runtime.log`) and the daemon will place it under the cache root.
- Reconnect telemetry is automatically appended to `/run/serial_lcd_cache/serial_backoff.log` as newline-delimited JSON (phase, device, baud, attempt counts).
- Parser/compression failures land in `/run/serial_lcd_cache/protocol_errors.log`, which auto-rotates at 256 KB so repeated envelope mistakes never fill the RAM disk.
- `/run/serial_lcd_cache` is wiped on reboot—treat it as ephemeral scratch space.

### Config validation rules

- `cols` must be between 8 and 40; `rows` must be between 1 and 4 to match HD44780 glass sizes.
- `scroll_speed_ms` must be at least 100 ms and `page_timeout_ms` must be at least 500 ms so watchdog UI remains responsive.
- `baud` must be at least 9600 so the serial link always starts from a reliable baseline before additional tuning takes place.
- Invalid values are rejected on startup with a clear error; use the defaults above if you are unsure.

## CLI reference

`lifelinetty run` is the default command, so you can omit `run` and pass flags directly. Every flag below also works from `~/.serial_lcd/config.toml` unless noted.

| Flag | Purpose | Default / Notes |
| ---- | ------- | ---------------- |
| `--device <path>` | Serial device to read newline-delimited JSON from. | `/dev/ttyUSB0` @ 9600 8N1. Override to `/dev/ttyAMA0`, `/dev/ttyS*`, or USB adapters as needed. |
| `--baud <number>` | Serial baud rate. | `9600` (minimum enforced before you opt into higher speeds via the wizard or config) |
| `--flow-control <none\|software\|hardware>` | Override whether RTS/CTS or XON/XOFF is asserted on the UART. | `none` |
| `--parity <none\|odd\|even>` | Choose parity framing when the remote expects it. | `none` |
| `--stop-bits <1\|2>` | Select one or two stop bits. | `1` |
| `--dtr-on-open <auto\|on\|off>` | Force the DTR line high/low on connect or leave the driver default. | `auto` (preserve driver behavior) |
| `--serial-timeout-ms <number>` | Millisecond timeout for serial reads before reconnect logic kicks in. | `500` ms |
| `--cols <number>` | LCD columns. | `20` |
| `--rows <number>` | LCD rows. | `4` |
| `--payload-file <path>` | Load a local JSON payload and render it once (no serial input). | Disabled by default—handy for CI smoke tests. |
| `--backoff-initial-ms <number>` | Initial reconnect backoff after serial failures. | `500` ms |
| `--backoff-max-ms <number>` | Maximum reconnect backoff. | `10_000` ms |
| `--pcf8574-addr <auto\|0xNN>` | I²C address for the PCF8574 backpack or `auto` to probe the common range. | `auto` (tries `0x27`, `0x26`, … ). |
| `--log-level <error\|warn\|info\|debug\|trace>` | Verbosity for stderr/file logs. | `info` (also configurable via `LIFELINETTY_LOG_LEVEL`). |
| `--log-file <path>` | Append logs to a file inside `/run/serial_lcd_cache` (also honors `LIFELINETTY_LOG_PATH`). | No file logging unless you provide a cache-rooted path. |
| `--polling` | Force-enable the hardware polling overlay even if the config disables it. | Defaults to the config value (`polling_enabled`). |
| `--no-polling` | Disable polling even when the config enables it. | Handy for smoke tests if you want to suppress the overlay/logging. |
| `--poll-interval-ms <number>` | Interval between poll snapshots. | `5000` ms (must stay within 1000–60000 ms). |
| `--compressed` | Request compressed payloads when the peer also advertises the capability. | Defaults to `[protocol].compression.enabled` (false). |
| `--no-compressed` | Force plaintext payloads even if config/negotiation enabled compression. | Use when diagnosing envelope issues or talking to legacy peers. |
| `--codec <lz4\|zstd>` | Choose the codec when compression is active. | `lz4` |
| `--demo` | Run built-in demo pages to validate wiring—no serial input required. | Disabled by default. |
| `--serialsh` | Launch the optional serial shell that sends commands through the tunnel and streams remote stdout/stderr plus exit codes. | Disabled by default so daemons keep running headless unless you explicitly opt into the interactive session. |
| `--wizard` | Run the guided first-run wizard even if a config already exists. | Automatically runs when `~/.serial_lcd/config.toml` is missing; also forceable via `LIFELINETTY_FORCE_WIZARD=1`. |
| `--help` / `--version` | Display usage or the crate version. | Utility flags that never touch hardware. |

### Guided first-run wizard (Milestone I)

- **Auto-run trigger**: the wizard starts before any run/test mode whenever `~/.serial_lcd/config.toml` is missing. It records the serial device, highest stable baud, LCD geometry, and negotiation role preference, then persists those answers and writes a short transcript to `/run/serial_lcd_cache/wizard/summary.log`.
- **Manual reruns**: invoke `lifelinetty --wizard` or set `LIFELINETTY_FORCE_WIZARD=1` to re-run the interview even when a config already exists. This is helpful after hardware moves or when testing new baud profiles.
- **Headless + CI support**: when stdin is not a TTY (systemd, CI), the wizard auto-accepts safe defaults so the daemon can boot unattended. Provide `LIFELINETTY_WIZARD_SCRIPT=/path/to/answers.txt` with newline-delimited responses to script the prompts during testing. The first line now answers the new LCD-presence question (`y`/`n`).
- **LCD detection**: the wizard now asks up front whether an LCD is connected before touching the hardware; answering `n` saves a 2-row fallback configuration so the daemon keeps running without an attached display.
- **LCD cues + logging**: prompts mirror onto the LCD (when available), and every outcome is appended to `/run/serial_lcd_cache/wizard/summary.log` for auditing alongside serial/log caches.

### Serial shell mode (Milestone G)

Milestone G supplies an official interactive shell for the command tunnel. Run `lifelinetty --serialsh` to drop into the `serialsh>` prompt, send JSON `CmdRequest` frames, and stream the remote stdout/stderr chunks plus their exit code. Busy responses and command failures stay visible so you always know when the remote host is congested. The CLI rejects `--demo` and `--payload-file` when `--serialsh` is enabled so that the tunnel stays dedicated to interactive commands, and the default systemd service still runs the headless `lifelinetty run` path unless you explicitly launch the shell yourself.

### Serial precedence cheatsheet

- If a flag is omitted, the daemon falls back to `~/.serial_lcd/config.toml`.
- When both CLI and config omit a setting, the built-in defaults apply: `/dev/ttyUSB0` @ 9600 8N1, 20×4 LCD.
- Alternate Linux UARTs like `/dev/ttyAMA0`, `/dev/ttyS0`, or USB adapters work equally well—point the CLI flag or config entry at the path you need.

---

## Systemd (Optional but recommended)

Install as a service:

```sh
sudo install -m 0755 /usr/local/bin/lifelinetty /usr/local/bin/lifelinetty
sudo install -m 0644 lifelinetty.service /etc/systemd/system/lifelinetty.service
sudo systemctl daemon-reload
sudo systemctl enable --now lifelinetty.service
```

Gives you:

- automatic restart  
- locked-down service  
- background mode  

---

## Troubleshooting & Debugging

### LCD is blank  

- I²C disabled — run `sudo raspi-config`
- SDA/SCL swapped  
- LCD contrast too low  
- Wrong LCD size (`--cols X --rows Y`)

### Shows garbage characters  

- Columns/rows don’t match the LCD  
- Power brownout (use 5V, not 3.3V)

### `i2cdetect` shows nothing  

- Wrong wiring  
- Faulty backpack  
- Using a Pi Zero with old cable

Check:

```sh
i2cdetect -y 1
```

You should see something like `27` or `3f`.

### JSON ignored  

- Must be **one JSON object per line**
- Max 512 bytes  
- Bad JSON → LCD shows a parse error  

### Serial port wrong  

Try:

```text
/dev/ttyUSB0
/dev/ttyAMA0
/dev/ttyS0
```

---

## Developer / Advanced Info

### Build from source

```sh
cargo build --release
```

### Run tests

```sh
cargo test
```

### ARM cross‑build with Docker

```sh
docker buildx build   --platform linux/arm/v6   -f docker/Dockerfile.armv6 .
```

### Repo  

<https://github.com/macg4dave/LifelineTTY>


### Architecture docs  

See `docs/architecture.md` and the LCD pattern files.

### Packaging  

See `docs/releasing.md` for `.deb`, `.rpm`, and multi‑arch builds.

---

## Summary

LifelineTTY gives you a **professional-quality LCD dashboard** with:

- JSON-driven rendering  
- Powerful display modes  
- Automatic scrolling, paging, alerts, icons, bar graphs  
- Super simple setup  
- Raspberry‑Pi‑first design  
- Rock‑solid daemon mode  

It’s one of the easiest ways to add a live display to a Raspberry Pi project — whether it’s PiKVM, a home server, a cluster, a sensor node, or anything else.

---

Enjoy the project — and watch for the companion **lifelinetty‑send** tool coming soon!

---
