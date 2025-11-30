# SerialLCD

Serial-to-LCD daemon for HD44780-compatible character LCDs with PCF8574 I2C backpacks.

## Docker cross-build (ARMv6)

See `docker/README.md` for a BuildKit-based flow to produce an `armv6` image targeting Raspberry Pi 1 / BCM2835:

```sh
docker buildx build --platform linux/arm/v6 -f docker/Dockerfile.armv6 -t seriallcd:armv6 .
```

## JSON payload format (v1)

The daemon ingests JSON objects (line-delimited) describing what to render. The raw frame must be <=512 bytes (`MAX_FRAME_BYTES`). Fields:

- `version` (int, optional) — only `1` is accepted when present.
- `line1`, `line2` (strings, required) — text for each row; `{0xNN}` placeholders emit custom glyphs; `mode:"banner"` clears `line2`.
- Bar graph: `bar` (0-100) or `bar_value` + `bar_max` (default `100`, minimum `1`) compute a percent; `bar_label` text prefix; `bar_line1`/`bar_line2` pick target row (dashboard mode forces bottom row).
- Backlight/alert: `backlight` (default `true`), `blink` (default `false`).
- Scrolling/paging: `scroll` (default `true`), `scroll_speed_ms` (default `250`), `page_timeout_ms` (default `4000`).
- Lifetimes: `duration_ms` or legacy `ttl_ms` — auto-expire after ms; `config_reload` (bool) hints the daemon to reload config.
- Actions: `clear` (bool) clears display first; `test` (bool) shows a test pattern.
- Mode/icons: `mode` in `normal|dashboard|banner` (default `normal`); `icons` array of `battery|arrow|heart` (unknown entries are ignored).
- Optional integrity: `checksum` hex string (`crc32` of the payload with `checksum` omitted); mismatches are rejected.

### Example payloads

```json
{"version":1,"line1":"Up 12:34  CPU 42%","line2":"RAM 73%","bar_value":73,"bar_max":100,"bar_label":"RAM","mode":"dashboard","page_timeout_ms":6000}
{"version":1,"line1":"ALERT: Temp","line2":"85C","blink":true,"duration_ms":5000}
{"version":1,"line1":"NET {0x00} 12.3Mbps","line2":"","bar_value":650,"bar_max":1000,"bar_label":"NET","icons":["battery"]}
{"version":1,"line1":"Long banner text that scrolls","line2":"ignored","mode":"banner","scroll_speed_ms":220}
{"version":1,"line1":"Backlight OFF demo","line2":"It should go dark","backlight":false}
{"version":1,"line1":"Clear + Test Pattern","line2":"Ensure wiring is OK","clear":true,"test":true}
{"version":1,"line1":"Scroll disabled","line2":"This line stays put","scroll":false,"page_timeout_ms":4500}
{"version":1,"line1":"Config reload hint","line2":"Reload config now","config_reload":true}
```

More samples live in `samples/payload_examples.json` and `samples/test_payload.json`.

### Feeding payloads

- One-shot from disk (no serial input): `seriallcd run --device /dev/ttyUSB0 --payload-file samples/test_payload.json`.
- Streaming over a PTY for development: create paired PTYs with `socat -d -d pty,raw,echo=0 pty,raw,echo=0`, point `seriallcd` at one end, and run `python3 samples/payload_feeder.py --device /dev/pts/X --baud 115200 --delay 4.0` against the other.
- Real hardware: upstream software should send newline-terminated JSON frames (≤512 bytes) at the configured baud; unknown lines are ignored and checksum failures are rejected.

### Limits and defaults

- Frame size: 512 bytes max (JSON string length). Oversized frames are rejected.
- Defaults: `scroll_speed_ms=250`, `page_timeout_ms=4000`, `bar_max=100`, `backlight=true`, `scroll=true`, `blink=false`, `mode=normal`.
- Dashboard mode always renders the bar on the bottom row even if `bar_line1=true`.
- Unknown `icons` entries are ignored; unknown `mode` falls back to `normal`.
