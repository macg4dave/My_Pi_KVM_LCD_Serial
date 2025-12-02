# LifelineTTY (formerly SerialLCD) — A Powerful, Drop‑In Display Engine for Raspberry Pi & Linux

LifelineTTY (previously called SerialLCD) turns any **HD44780‑compatible character LCD** (16×2, 20×4, 16×4, 40×20 into a **smart, JSON‑driven dashboard** for your Raspberry Pi or Linux device.

If you want a **clean, easy way to show system stats, alerts, KVM info, network activity, or anything else** on a LCD screen — LifelineTTY does all the hard work for you.

**No I²C code. No screen‑handling logic. Just send JSON → the display updates.**

It’s compact, robust, beginner‑friendly, and powerful enough for advanced setups.

---

## What LifelineTTY Gives You (Why It's Special)

- **True plug‑and‑play** LCD support  
  Works with *any* HD44780 LCD using a PCF8574 I²C backpack — the most common type used by Pi and Arduino hobbyists.

- **Simple JSON Interface**  
  Send a JSON line and the LCD updates instantly.

- **Three polished display modes**  
  - Normal (default)
  - Dashboard (metrics + bar graph)
  - Banner (scrolling marquee)

- **Real bar graphs** with smooth fill steps  
- **Scrolling text**, **paging**, **alerts**, **backlight control**

- Fully test your setup with **demo mode!** (`--demo`) that:
  - Proves your wiring is correct  
  - Shows every feature (bars, scrolling, icons, alerts)  
  - Helps you understand exactly what kind of JSON to send  

- Extremely resilient:  
  - Detects serial errors  
  - Shows “offline” screens  
  - Recovers automatically  

Everything is designed so that **any Pi beginner** can get results in minutes, while more advanced users can script anything they want.

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

`--demo` is your best friend.

---

## Sending JSON (This is the real magic)

LifelineTTY listens for **one JSON object per line** over a serial port.

Examples:

### Simple text

```json
{"line1":"Hello","line2":"World"}
```

### Dashboard

```json
{"mode":"dashboard","line1":"CPU 42%","line2":"RAM 73%","bar":73}
```

### Banner marquee

```json
{"mode":"banner","line1":"Scrolling across the LCD..."}
```

### Alert with blinking backlight

```json
{"line1":"TEMP ALERT","line2":"85C","blink":true}
```

### Turn backlight off

```json
{"line1":"Lights out","line2":"","backlight":false}
```

**Everything** the display can do is driven by JSON.

---

## Sending the JSON (TODO — Sister Program Coming)

Soon there will be a small companion tool that:

- auto-detects the daemon  
- structures JSON for you  
- sends system metrics  
- gives a GUI + CLI interface  

**(TODO: lifelinetty‑send — placeholder section)**

---

## Config File (Auto-generated)

Stored at:

```
~/.serial_lcd/config.toml
```

By default the daemon listens on `/dev/ttyUSB0` at 9600 8N1. Edit the config (or pass CLI flags) to point at `/dev/ttyAMA0`, `/dev/ttyS0`, USB adapters, or any other TTY that exposes your sender.

Example:

```toml
device = "/dev/ttyUSB0"
baud = 9600
cols = 20
rows = 4
scroll_speed_ms = 250
page_timeout_ms = 4000
pcf8574_addr = "auto"
button_gpio_pin = null
```

Reload config without restarting the daemon:

```json
{"config_reload":true}
```

---

## Storage & cache policy

- Persistent settings live at `~/.serial_lcd/config.toml` (auto-created the first time you run the daemon).
- Everything else (logs, payload caches, telemetry snapshots, LCD caches) belongs in the RAM disk mounted at `/run/serial_lcd_cache`. The provided systemd unit already restricts writes to that directory.
- The `--log-file` flag and `LIFELINETTY_LOG_PATH` environment variable only accept paths inside `/run/serial_lcd_cache`. Provide an absolute cache path or a relative name (e.g., `logs/runtime.log`) and the daemon will place it under the cache root.
- `/run/serial_lcd_cache` is wiped on reboot—treat it as ephemeral scratch space.

## CLI reference

`lifelinetty run` is the default command, so you can omit `run` and pass flags directly. Every flag below also works from `~/.serial_lcd/config.toml` unless noted.

| Flag | Purpose | Default / Notes |
| ---- | ------- | ---------------- |
| `--device <path>` | Serial device to read newline-delimited JSON from. | `/dev/ttyUSB0` @ 9600 8N1. Override to `/dev/ttyAMA0`, `/dev/ttyS*`, or USB adapters as needed. |
| `--baud <number>` | Serial baud rate. | `9600` |
| `--cols <number>` | LCD columns. | `20` |
| `--rows <number>` | LCD rows. | `4` |
| `--payload-file <path>` | Load a local JSON payload and render it once (no serial input). | Disabled by default—handy for CI smoke tests. |
| `--backoff-initial-ms <number>` | Initial reconnect backoff after serial failures. | `500` ms |
| `--backoff-max-ms <number>` | Maximum reconnect backoff. | `10_000` ms |
| `--pcf8574-addr <auto\|0xNN>` | I²C address for the PCF8574 backpack or `auto` to probe the common range. | `auto` (tries `0x27`, `0x26`, … ). |
| `--log-level <error\|warn\|info\|debug\|trace>` | Verbosity for stderr/file logs. | `info` (also configurable via `LIFELINETTY_LOG_LEVEL`). |
| `--log-file <path>` | Append logs to a file inside `/run/serial_lcd_cache` (also honors `LIFELINETTY_LOG_PATH`). | No file logging unless you provide a cache-rooted path. |
| `--demo` | Run built-in demo pages to validate wiring—no serial input required. | Disabled by default. |
| `--help` / `--version` | Display usage or the crate version. | Utility flags that never touch hardware. |

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

## About SerialLCD (Alpha)

**LifelineTTY is the production release.** SerialLCD was an alpha preview and is no longer supported. No backward compatibility is maintained.

If you were using SerialLCD, migrate your setup to LifelineTTY:

- Download and install the latest LifelineTTY release from the GitHub releases page.
- Update your scripts and configs to use `lifelinetty` instead of `seriallcd`.
- Configuration file path remains `~/.serial_lcd/config.toml`; all existing configs are compatible.
- JSON payload format is unchanged — your sender scripts will work as-is.

For details on what's new in LifelineTTY, see the roadmap and architecture docs.
