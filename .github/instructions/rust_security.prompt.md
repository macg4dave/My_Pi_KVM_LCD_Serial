---
name: rust_security
description: "Prompt template for security reviews and hardening tasks in Rust projects."
---

Context
-------
- **Project**: LifelineTTY — single Rust daemon consuming newline JSON over `/dev/ttyUSB0 @ 9600 8N1` by default, with config/CLI overrides for `/dev/ttyAMA0`, `/dev/ttyS*`, and USB adapters.
- **Storage policy**: persist only `~/.serial_lcd/config.toml`; all other writes (logs, payload caches, telemetry) must live inside `/run/serial_lcd_cache`.
- **Attack surface**: UART payload parsing, LCD rendering, and CLI/config inputs—security changes must preserve the fixed protocol/LCD contracts while keeping RSS < 5 MB.

Scope
-----
-- Typical files: `<src/*>` modules, error handling, input parsing, unsafe code blocks (crate: `<crate_name>`).
-- Applies to security audits, unsafe code reviews, input validation, and error handling improvements.

Hard constraints
----------------
- Preserve correctness and public API behavior unless explicitly requested otherwise.
- Avoid introducing unsafe code unless absolutely necessary; prefer safe abstractions.
- Run `cargo test -p <crate_name>` and include full output to confirm no regressions.
- Add tests for any new validation or error handling logic.
- Document security changes with comments or migration notes if behavior changes.

Prompt template
---------------
Task:
"""
<Brief summary of security task>

Details:
- What to change: <describe audit, validation, unsafe removal, error handling>
- Files: <list files to update>
- Tests: <describe which tests to add/update>
- Constraints / do not modify: <list any files/behaviors that must remain unchanged>
"""

Assistant instructions
----------------------
1. Provide a concise plan (2–3 bullets).
2. Make the smallest possible security‑focused changes consistent with idiomatic Rust.
3. Add or update unit/integration tests to validate the security improvements.
4. Run `cargo test -p <crate_name>` and paste the full output.
5. If tests fail, iterate up to 5 times to fix failures (explain each iteration briefly).
6. Return:
   - Short summary of changes with file paths.
   - Exact patch(s) in `apply_patch` diff format.
   - The `cargo test` output showing passing tests.
   - Suggested next steps (e.g., further audits, fuzz testing, dependency checks).

Example prompts
---------------
- "Task: Add input validation for user‑provided paths. Details: reject relative paths with `..`. Files: `src/fs.rs`. Add unit tests for invalid inputs."
- "Task: Remove unnecessary `unsafe` block in buffer handling. Details: replace with safe slice operations. Files: `src/buffer.rs`. Ensure tests pass."
- "Task: Harden error handling in network parser. Details: return `Result` instead of panicking. Files: `src/net/parser.rs`. Add integration test for malformed input."
