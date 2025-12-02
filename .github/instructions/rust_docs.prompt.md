---
name: rust_docs
description: "Prompt template for LifelineTTY documentation, README, and migration note tasks."
---

Context
-------
- **Primary docs**: `README.md`, `docs/*.md`, `docs/lcd_patterns.md`, `docs/architecture.md`, `docs/releasing.md`, service unit notes, and Rustdoc comments inside `src/**`.
- **Audience**: Raspberry Pi operators configuring `lifelinetty`, plus contributors maintaining UART/LCD render pipeline.
- **Doc requirements**: every CLI flag and config key must be documented; storage guardrails (`/run/serial_lcd_cache`, `~/.serial_lcd/config.toml`) and LCD constraints must be reiterated when relevant.
- **Serial defaults**: `/dev/ttyUSB0 @ 9600 8N1` baseline with config/CLI overrides allowed for `/dev/ttyAMA0`, `/dev/ttyS*`, and USB adapters—ensure every doc matches this statement.
- **Tooling**: run `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` to ensure doctests/examples compile.

Hard constraints
----------------
1. Keep documentation scoped to the requested change but ensure all affected files (README, docs, Rustdoc) stay consistent.
2. Reconfirm accuracy of CLI flags, config keys, storage rules, and LCD behavior when editing text or samples.
3. Run `cargo test` (or the relevant subset) so doctests/examples continue to compile.
4. Provide migration notes whenever behavior changes or old names are retired (e.g., SerialLCD references).

Prompt template
---------------
Task:
"""
<Brief summary of documentation task>

Details:
- What to change: <doc-comments, README sections, migration notes, examples>
- Files: <list files to update>
- Tests/examples: <describe which examples or doctests to add/update>
"""

Assistant instructions
----------------------
1. Provide a concise 2–3 bullet plan (mention specific docs/sections).
2. Apply the minimal documentation change, ensuring README + docs + Rustdoc stay synchronized.
3. Update or add doctests/examples when snippets could compile.
4. Run `cargo test` (or the specified subset) to verify doctests/examples, then paste the full output.
5. Return:
   - Short summary of changes with file paths.
   - Exact patch(es) in `apply_patch` format.
   - `cargo test` output.
   - Optional next steps (e.g., follow-up doc ideas, screenshots, diagrams).

Example prompts
---------------
- "Task: Document `--demo` flag under CLI table. Details: explain wiring validation and mention RAM-disk writes. Files: `README.md`. Tests: `cargo test` for doctests."
- "Task: Add section to `docs/architecture.md` describing RAM-disk cache policy vs persistent config. Files: `docs/architecture.md`, `README.md`."
- "Task: Write migration note for SerialLCD → LifelineTTY rename in `docs/releasing.md` and `README.md`, clarifying binary/flag compatibility."
