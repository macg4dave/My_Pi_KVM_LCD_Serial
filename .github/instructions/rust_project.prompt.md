---
name: "Rust Project Assistant"
scope: "repository"
description: "Repository-aware Copilot prompt template for Rust projects. Use this when asking for code changes, tests, or PR-ready patches."
---

Context
-------
-- Project: <project_name> — a Rust application or library.
-- Main code: <src_directory> (crate name `<crate_name>`).
-- Tests: `cargo test -p <crate_name>` (unit + integration under `<tests_directory>`).
-- Tooling: `cargo build`, `cargo test`, `cargo run`, `rustfmt`, `clippy`.

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
