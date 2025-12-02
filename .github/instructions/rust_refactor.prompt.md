---
name: rust_refactor
description: "Prompt template for refactoring tasks in Rust projects."
---

Context
-------
- **Project**: LifelineTTY — ultra-light Rust daemon driving HD44780 LCDs from newline JSON received over `/dev/ttyUSB0 @ 9600 8N1` by default; overrides may target `/dev/ttyAMA0`, `/dev/ttyS*`, or USB adapters with identical framing.
- **Storage guardrails**: only `~/.serial_lcd/config.toml` persists; temp files, logs, and caches live strictly under `/run/serial_lcd_cache`.
- **Constraints**: Keep CLI flags stable (`--run`, `--test-lcd`, `--test-serial`, `--device`, `--baud`, `--cols`, `--rows`, `--demo`).

Scope
-----
-- Typical files: `<src/*>` modules, helpers, and internal structures (crate: `<crate_name>`).
-- Applies to code organization, naming, extraction of helpers, or moving functionality between modules.

Hard constraints
----------------
- Keep changes minimal and focused on the requested refactor.
- Preserve public APIs and CLI outputs unless explicitly allowed.
- Run `cargo test -p <crate_name>` and include the full output.
- Do not remove features or tests; if behavior changes, add migration notes and tests.

Prompt template
---------------
Task:
"""
<Brief summary of refactor>

Details:
- What to change: <describe structural edits, e.g., move helpers, rename functions, extract modules>
- Files: <list files to update>
- Tests: <describe which tests to add/update or leave blank>
- Constraints / do not modify: <list any files/behaviors that must remain unchanged>
"""

Assistant instructions
----------------------
1. Provide a concise plan (2–3 bullets).
2. Make the smallest possible code changes consistent with idiomatic Rust.
3. Add or update unit/integration tests to validate the refactor.
4. Run `cargo test -p <crate_name>` and paste the full output.
5. If tests fail, iterate up to 5 times to fix failures (explain each iteration briefly).
6. Return:
   - Short summary of changes with file paths.
   - Exact patch(s) in `apply_patch` diff format.
   - The `cargo test` output showing passing tests.
   - Suggested next steps or optional improvements.

Example prompts
---------------
- "Task: Extract file listing logic into `src/fs/listing.rs`. Details: move helper functions, add unit tests for formatting. Files: `src/fs.rs` → `src/fs/listing.rs`. Ensure `cargo test -p <crate_name>` passes."
- "Task: Rename `Config::load_default()` to `Config::load()`. Details: update call sites, preserve behavior. Files: `src/config.rs`. Add migration note in README."
