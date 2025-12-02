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

# Quick Start (Raspberry Pi)

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

# Sending JSON (This is the real magic)

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

# Sending the JSON (TODO — Sister Program Coming)

Soon there will be a small companion tool that:

- auto-detects the daemon  
- structures JSON for you  
- sends system metrics  
- gives a GUI + CLI interface  

**(TODO: lifelinetty‑send — placeholder section)**

---

# Config File (Auto-generated)

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

# Systemd (Optional but recommended)

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

# Troubleshooting & Debugging

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

# Developer / Advanced Info

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

# Summary

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

## Legacy name: SerialLCD

This project was originally called **SerialLCD**. The last release under that name is tagged as `seriallcd-v1.0.0`. Behaviour remains compatible; only the binary and project branding have changed.

Migration notes:

- The binary is now `lifelinetty` and packaged releases provide a compatibility copy at `/usr/bin/seriallcd` to keep legacy scripts working.
- The primary systemd unit is `lifelinetty.service`. Installer scripts will create a `seriallcd.service` symlink for compatibility — it's recommended to enable `lifelinetty.service` going forward.
- `~/.serial_lcd/config.toml` remains the configuration file path and is unchanged.
- CLI behavior and flags are compatible with previous releases; for logging, `LIFELINETTY_LOG_*` env variables are preferred, but `SERIALLCD_LOG_*` is still accepted.

If you maintain scripts or deployments that assume `seriallcd`, update them to `lifelinetty` (or keep `seriallcd` as the alias until you can migrate).
