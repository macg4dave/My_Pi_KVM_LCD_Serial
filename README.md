# SerialLCD — A Powerful, Drop‑In Display Engine for Raspberry Pi & Linux

SerialLCD turns any **HD44780‑compatible character LCD** (16×2, 20×4, 16×4, 40×20 into a **smart, JSON‑driven dashboard** for your Raspberry Pi or Linux device.

If you want a **clean, easy way to show system stats, alerts, KVM info, network activity, or anything else** on a LCD screen — SerialLCD does all the hard work for you.

**No I²C code. No screen‑handling logic. Just send JSON → the display updates.**

It’s compact, robust, beginner‑friendly, and powerful enough for advanced setups.

---

## What SerialLCD Gives You (Why It's Special)

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
wget https://github.com/macg4dave/My_Pi_KVM_LCD_Serial/releases/latest/download/seriallcd_arm.deb
sudo apt install ./seriallcd_arm.deb
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
seriallcd --demo
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

SerialLCD listens for **one JSON object per line** over a serial port.

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

**(TODO: seriallcd‑send — placeholder section)**

---

# Config File (Auto-generated)

Stored at:

```
~/.serial_lcd/config.toml
```

Example:

```toml
device = "/dev/ttyAMA0"
baud = 115200
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
sudo install -m 0755 /usr/local/bin/seriallcd /usr/local/bin/seriallcd
sudo install -m 0644 seriallcd.service /etc/systemd/system/seriallcd.service
sudo systemctl daemon-reload
sudo systemctl enable --now seriallcd.service
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

```
/dev/ttyAMA0
/dev/ttyS0
/dev/ttyUSB0
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

<https://github.com/macg4dave/My_Pi_KVM_LCD_Serial>

### Architecture docs  

See `docs/architecture.md` and the LCD pattern files.

### Packaging  

See `docs/releasing.md` for `.deb`, `.rpm`, and multi‑arch builds.

---

# Summary

SerialLCD gives you a **professional-quality LCD dashboard** with:

- JSON-driven rendering  
- Powerful display modes  
- Automatic scrolling, paging, alerts, icons, bar graphs  
- Super simple setup  
- Raspberry‑Pi‑first design  
- Rock‑solid daemon mode  

It’s one of the easiest ways to add a live display to a Raspberry Pi project — whether it’s PiKVM, a home server, a cluster, a sensor node, or anything else.

---

Enjoy the project — and watch for the companion **seriallcd‑send** tool coming soon!
