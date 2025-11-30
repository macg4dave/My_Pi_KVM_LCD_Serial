# LCD UI patterns (ideas to implement)

Quick recipes inspired by the Python examples:

- Progress bar: precompute custom chars for partial blocks (0%, 20%, 40%, 60%, 80%, 100%) and fill a row with them based on a percent value. Keep a helper that maps 0-100 to blocks and writes a fixed-width bar.
- Scrolling text: maintain an offset into a longer string, slice `cols` characters, and update on a timer. Wrap around to the start. For simple effects, add leading/trailing spaces.
- Tiny dashboard: define a layout (e.g., line 1: label/value, line 2: bar or status). Use `move_to` and `write_line` to refresh only the changing segments.
- Backlight hints: flicker-free toggles by keeping backlight bit sticky between writes (driver already preserves this).
- Custom icons: load 5x8 bitmaps (see `Hd44780::load_custom_bitmap`) at startup; reuse them in text with `{0xNN}` placeholders.

These are documentation-only patterns; wire them into the runtime loop once the data source is defined.
