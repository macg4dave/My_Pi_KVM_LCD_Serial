---
name: "Rust Bugfix Assistant"
description: "Prompt template for reproducing and fixing bugs in Rust projects."
---

Context
-------
-- Project: <project_name> (crate `<crate_name>`).
-- Source: <src_directory>. Tests: <tests_directory> (run with `cargo test -p <crate_name>` or a targeted command).
-- Tooling: `cargo build`, `cargo test`, `cargo fmt`, `cargo clippy`.

Hard constraints
----------------
- Reproduce the bug or failing test before fixing it; capture the error message.
- Make the smallest code change that fixes the issue.
- Add a focused regression test that fails before the fix and passes after.
- Run the relevant `cargo test` command and include the full output.
- Avoid altering public APIs or CLI outputs unless explicitly requested.

Prompt template
---------------
Task:
"""
<One-line summary of the bug>

Details:
- Failure: <panic/backtrace/error output or failing test name>
- Repro steps: <commands or user flows to trigger the bug>
- Suspected files: <list or leave blank>
- Tests to run: <test command or leave default `cargo test -p <crate_name>`>
- Constraints / do not modify: <list any files/behaviors that must remain unchanged>
"""

Assistant instructions
----------------------
1. Summarize a short plan (2–3 bullets) before editing.
2. Reproduce the failure and quote the exact error.
3. Add a minimal test that captures the bug (unit or integration).
4. Implement the fix with the smallest possible diff.
5. Run the specified `cargo test` command; include the full output.
6. Return:
   - Concise summary of changes with file paths.
   - Exact patch(es) in `apply_patch` format.
   - Test output showing the failure (if observed) and the final passing run.
   - Any manual verification steps (if applicable).

Example prompts
---------------
- "Task: Fix panic when parsing empty config file. Details: panic log shows `unwrap()` on empty string. Repro: `cargo run -- --config /tmp/empty.toml`. Suspected files: `src/config.rs`. Tests: `cargo test -p my_crate config`. Keep CLI flags unchanged."
- "Task: Fix failing test `api::handles_timeout`. Details: error `expected Timeout, got Ok`. Repro: `cargo test -p api -- --ignored`. Suspected files: `src/client.rs`. Add regression test to cover timeout handling."
