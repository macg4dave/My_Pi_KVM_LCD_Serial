# LCD UI patterns

Every payload the daemon renders is just two lines of 5×8 characters, yet we can squeeze a surprising
amount of information onto the glass. This page collects proven layouts and the payload fields that
enable them so you can script new behaviors without poking through the entire render loop.

## Progress bars + dashboards

- Use the `bar`, `bar_label`, `bar_line1`, and `bar_value` fields (see `payload/schema.rs`). The
  render loop keeps a fixed table of six partial-block glyphs so bars are smooth even while icons are
  active.
- When `mode:"dashboard"` is set, the bar anchors to line 2 automatically; for narrower LCDs use
  `bar_line1:true` to pin it to the top row and print text beneath it.
- Pair the bar with `line1` containing a headline (e.g., `CPU 42%`) and `line2` containing either the
  bar or a secondary value. This mimics the playlist’s CPU/RAM frames.

## Scrollable banners

- Set `mode:"banner"` to reserve the bottom row for a hard border and let `line1` scroll as a marquee.
- Fine-tune `scroll_speed_ms` per payload (minimum 100 ms) when you need to slow the marquee down for
  long alerts.
- Combine with `page_timeout_ms` to control how long the banner stays onscreen before the render loop
  advances to the next payload.

## Alerts + blink cadence

- `blink:true` toggles LCD blink mode on both lines. Layer it with `backlight:false` (or true) to
  flash the cathodes politely instead of spamming uppercase WARN messages.
- Alert payloads usually set `line1` to the condition and `line2` to the corrective action.
- Keep alert frames short-lived (`duration_ms`) so they fall back to regular content quickly.

## Icon-heavy overlays

- Populate the `icons` array with semantic names from `docs/icon_library.md`. IconBank auto-loads
  them each frame and falls back to ASCII when the eight-slot CGRAM budget is exceeded.
- When you need both icons and bars, keep the bar on line 2 so it reuses the cached partial-block
  glyphs and leaves more room for overlay icons. This exact trick powers the “Polling snapshot” demo.
- Navigation cues (arrows, return arrow, hourglass) work nicely as live wizard prompts or tunnel
  status indicators.

## Heartbeat + watchdog indicators

- The heartbeat overlay lives on whichever row the bar isn’t using. Trigger it by setting
  `heartbeat:true` in the payload or letting the watchdog inject it when serial traffic pauses.
- For human-friendly text, prefix `line1` with a short tag (`HB OK`, `HB WAIT`) and let the overlay
  communicate liveness visually.

## Config reminders

- Use `config_reload:true` frames sparingly. They intentionally pause the render loop while the daemon
  reloads TOML, so pair them with unique wording (`CONFIG UPDATED`, `RELOAD COMPLETE`) and a short
  `duration_ms`.

## Tips for crafting new layouts

- Keep `line1` and `line2` trimmed to the device width; the render loop pads automatically but short
  strings leave room for icons on the right edge.
- Interleave plain text payloads between complex ones so the CGRAM allocator has breathing room and
  the heartbeat overlay can reclaim glyph slots.
- When in doubt, prototype with `lifelinetty --payload-file samples/payload_examples.json` and tweak
  entries until the LCD looks right. The new demo playbook walks through concrete examples.
