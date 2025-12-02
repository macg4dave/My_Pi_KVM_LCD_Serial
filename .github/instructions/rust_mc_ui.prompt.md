---
name: rust_ui_task
description: "Prompt template for UI, rendering, widget, and input-handling changes in a Rust app."
---

## Copy the template, fill in the Task and Details sections, then execute in VS Code or Copilot.

## Context

- **Project**: LifelineTTY — an ultra-light Rust daemon that renders HD44780 LCD lines from newline-delimited JSON received on `/dev/ttyUSB0 @ 9600 8N1` by default; CLI/config overrides can target `/dev/ttyAMA0`, `/dev/ttyS*`, or USB adapters but must keep the same framing.
- **Storage**: only `~/.serial_lcd/config.toml` persists; all UI caches/logs/overlays must stay within `/run/serial_lcd_cache`.
- **LCD guardrails**: HD44780 + PCF8574 backpack, two 16-character lines today—UI changes must respect those dimensions unless the roadmap explicitly expands them.

## Scope

Typical files (edit as needed):

* `src/ui/*`
* `src/main.rs`
* `src/lib.rs` (for exports or shared state)

## Hard Constraints

* Run `cargo test` and include the full output.
* Keep changes minimal and scoped to UI code unless deeper changes are essential.
* Do not alter public CLI flags or machine-readable output.

## Prompt Template

Task:
""" <One-line summary of the UI change>

Details:

* What to change: <UI behavior / rendering / input changes>
* Files: <e.g., src/ui/panels.rs, src/ui/menu.rs>
* Tests: <unit tests needed for formatting, layout helpers, navigation, etc.>
  """

## Assistant Instructions

1. Outline a brief 2–3 step plan.
2. Provide a minimal compiling patch.
3. Add unit tests for pure helpers; adjust integration tests only if contract changes.
4. Run `cargo test` and include full output.
5. Fix failures and retry up to 5 times.

## Example Prompts

* “Make list navigation wrap around when reaching top/bottom. Files: `src/ui/list.rs`. Add tests for index wrapping helper.”
* “Fix modal centering logic on terminal resize. Files: `src/ui/modal.rs`. Add tests for centering math.”
* “Add keyboard shortcuts for panel switching. Files: `src/ui/panels.rs`, `src/main.rs`.”

## Usage


