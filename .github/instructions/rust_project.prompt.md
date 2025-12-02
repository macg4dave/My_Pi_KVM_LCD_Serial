Context
-------
- **Project**: LifelineTTY — single-binary Rust daemon for Raspberry Pi 1 that ingests newline-delimited JSON via `/dev/ttyUSB0 @ 9600 8N1` by default; config/CLI overrides may target `/dev/ttyAMA0`, `/dev/ttyS*`, or USB adapters without changing framing.
- **Main code**: `src/**` in the `lifelinetty` crate; integration tests live under `tests/`.
- **Tests/tooling**: `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` (plus targeted suites) must stay green on x86_64 and ARMv6.
- **Storage guardrails**: only `~/.serial_lcd/config.toml` persists; logs, payload caches, and temp files belong under `/run/serial_lcd_cache`.

Hard constraints (always include)
--------------------------------
- Run the test suite locally and include the full test output (`cargo test` or targeted command).
- Make the smallest possible change needed to solve the request.
- Add or update tests for any behavioral change.
- Preserve public APIs and CLI outputs unless explicitly allowed.
- Avoid removing features or tests. If behavior is changed, add a migration note and tests.

Repository preferences
----------------------
- Prefer idiomatic Rust: `snake_case`, `Result` error handling, avoid `unwrap()` except in tiny examples/tests.
- Keep patches minimal and focused.
- Add doc-comments on public APIs and unit tests for new helpers.

Prompt Template
---------------
Task:
"""
<Brief one-line summary of the requested change>

Details:
- What to change: <short description of edits or behavior change>
- Files to consider (optional): <comma-separated list>
- Tests: <describe which tests to add/update or leave blank>
- Constraints / do not modify: <list any files/behaviors that must remain unchanged>
"""

Assistant instructions
----------------------
1. Explain the plan in 2–3 bullets.
2. Make the smallest possible code changes.
3. Add or update unit/integration tests.
4. Run `cargo test -p <crate_name>` and paste the full output.
5. Iterate up to 5 times if tests fail.
6. Return:
   - Short summary of changes with file paths.
   - Exact patch(s) in `apply_patch` diff format.
   - The `cargo test` output showing passing tests.
   - Suggested next steps or optional improvements.
