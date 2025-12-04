# Demo playbook

Need to prove the wiring works before you hook up a real telemetry feed? The bundled `--demo`
playlist keeps the LCD busy without touching the serial port. This guide explains what each demo
frame is trying to show, how to tweak the playlist, and how to craft your own sample payloads when
you need a bespoke rehearsal.

## Quick start

1. Connect the LCD/I²C backpack exactly the same way you would for production.
2. Run `lifelinetty --demo` from your shell. The daemon never opens `/dev/tty*` in this mode, so you
   can leave other serial tooling running in parallel.
3. Adjust geometry on the command line to match your panel if needed:
   - `lifelinetty --demo --cols 16 --rows 2`
   - `lifelinetty --demo --cols 20 --rows 4`
4. Watch the LCD for at least one full rotation (≈90 seconds) to confirm scrolling, blinking,
   backlight toggles, and custom glyph swaps all look steady.

When you see smooth scrolling text, stable bars, and icon overlays that update without garbage
pixels, the hardware path is ready for live data.

## Playlist cheat sheet

| Demo frame (in order) | What to look for | Features covered |
| --- | --- | --- |
| CPU / RAM / MEM bars | Bar rendering, label centering, dashboard mode pinning the bar to the second line. | `bar`, `bar_value`, `bar_label`, `mode:"dashboard"` |
| DISK / NET readouts | Mixed custom glyphs (`{0x00}`) plus bar-on-top mode. | Mixed bar rows, legacy CGRAM placeholders, icon overlays |
| Alert pair | Blink + backlight flash cadence, alert wording, and TTL expiration. | `blink`, `backlight:false`, `duration_ms` |
| Scroll/Banner samples | Line-by-line scroll offsets, marquee banner forcing an empty second row. | `scroll`, `scroll_speed_ms`, `mode:"banner"` |
| Icon showreels | IconBank hot-swapping the curated glyphs (battery, heart, wifi, arrows) without flicker. | `icons` array, IconBank logging |
| Arrow + degrees frames | Overlaying navigation icons and degree glyphs while a bar animates. | Multiple icons, CGRAM + ASCII fallbacks |
| Ping-pong alert | Combined blink + icon overlay + heartbeats. | Icon overlay with alert + blink |

The playlist loops forever; if you miss a frame just wait for the next rotation. Debug logs tagged
`demo icon fallback` tell you when IconBank intentionally falls back to ASCII because the request
would exceed the eight-slot CGRAM budget.

## Building your own demos

Need to showcase a custom payload? Point the daemon at a JSON file instead of the canned playlist:

1. Copy `samples/payload_examples.json` somewhere safe and edit as needed. The file intentionally
   uses one JSON object per line (with blank lines between samples) so it is easy to paste into a
   serial terminal or piping script.
2. Run `lifelinetty --payload-file /path/to/your-demo.json`. The daemon will parse, render, and exit
   without touching the serial port, which makes it perfect for screenshots.
3. Keep `schema_version` at `1` (or whatever the daemon is configured to expect) so strict validation
   stays active even during dry runs.

### Sample payloads

The refreshed `samples/payload_examples.json` now includes:

- **Command tunnel banner** – highlights the Milestone A shell with arrow + return glyphs.
- **Polling snapshot** – shows how to place a temperature bar on the top row while the bottom line
  scrolls CPU/memory text.
- **Icon heartbeat** – demonstrates a minimal frame that only toggles icons and backlight.
- **Compression envelope** – reference for wrapping payloads before they hit a slow UART link.

Feel free to duplicate any of those entries as a starting point for site-specific demos.

## Icon and overlay tips

- IconBank automatically loads every glyph before each frame render. You only have to name the icon
  in the payload (e.g., `"icons":["battery","heart"]`).
- Bars share the CGRAM budget with icons. If you rely on icons heavily, keep the bar on the bottom
  row so it reuses the pre-baked partial-block glyphs and leaves more slots free for overlays.
- Heartbeat overlays always occupy the last cell of whichever row is *not* hosting the bar. When the
  serial link goes quiet you will see the heartbeat blink and can log the timestamp.
- Use `config_reload:true` frames sparingly—they force the daemon to reload TOML and temporarily
  pause the render loop. The demo playlist includes a reminder frame so you remember the feature
  exists without actually toggling it.

For even more internals, read `docs/architecture.md` for the render-loop data flow and
`docs/icon_library.md` for the curated glyph catalog.
