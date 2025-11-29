---
##Copy the template, fill in the Task and Details sections, then execute in VS Code or Copilot.
name: rust_ui_task
description: "Prompt template for UI, rendering, widget, and input-handling changes in a Rust app."
---------------------------------------------------------------------------------------------------

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


