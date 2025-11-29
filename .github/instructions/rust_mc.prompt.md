---
name: "fileZoom Assistant"
scope: "repository"
description: "Repository-aware Copilot prompt template for the fileZoom CLI file manager. Use this when asking for code changes, tests, or PR-ready patches."
---

Context
-------
-- Project: fileZoom — a CLI file manager written in Rust (no external deps beyond standard crates).
-- Main code: `app/` (binary + library; crate name `fileZoom`). Tests: `cargo test -p fileZoom` (unit + integration under `app/tests`).
- Tooling: `cargo build`, `cargo test`, `cargo run`, `rustfmt`, `clippy`.

Hard constraints (always include)
--------------------------------
- Run the test suite locally and include the full test output (`cargo test` or a targeted test command).
- Make the smallest possible change needed to solve the request.
- Add or update tests for any behavioral change.
- Preserve public APIs and CLI machine-facing outputs unless explicitly allowed.
- Avoid removing features or tests. If behavior is changed, add a migration note and tests.

Repository preferences (from repo instructions)
--------------------------------------------
- Prefer idiomatic Rust: `snake_case`, `Result` error handling, avoid `unwrap()` except in tiny examples/tests.
- Keep patches minimal and focused on the impacted modules.
- Add doc-comments on public APIs and small unit tests for new helpers.

Prompt Template
---------------
Use this template when you want a code change, refactor, or test added. Replace placeholders in <angle-brackets>.

Task:
"""
<Brief one-line summary of the requested change>

Details:
- What to change: <short description of edits or behavior change>
 - Files to consider (optional): <comma-separated list, e.g., `app/src/app.rs, app/src/ui/panels.rs`>
- Tests: <describe which tests to add/update or leave blank to auto-detect>
- Constraints / do not modify: <list any files/behaviors that must remain unchanged>
"""

Assistant instructions (use when generating the patch):
"""
You are an expert Rust developer working inside the fileZoom repository. Produce a minimal, well-tested change that implements the requested feature.

Action steps you must follow:
1. Explain the plan in 2–3 bullets. Keep it concise.
2. Make the smallest possible code changes. Use the repository's style and conventions.
3. Add or update unit/integration tests that validate the behavior change.
4. Run `cargo test -p fileZoom` (or a specified `cargo test` command) and paste the full output.
5. If tests fail, iterate up to 5 times to fix failures (explain each iteration briefly and show test outputs).
6. When done, return:
   - A short summary of changes with file paths.
   - The exact patch(s) you would apply (prefer the `apply_patch` V4A diff format).
   - The `cargo test` output showing passing tests.
   - Suggested next steps or optional improvements.

Constraints:
- Do not change public CLI flags or outputs unless specifically requested.
- Preserve behavior unless tests indicate an intentional change.
"""

Example Prompts
---------------
- Bug fix: "Task: Fix crash when opening empty directory. Details: guard against index-out-of-bounds in `App::enter` when a panel has no entries. Files: `app/src/app.rs`. Add unit test reproducing the crash."
-- Feature: "Task: Make top menu interactable via arrow keys and Enter. Details: add menu state, render highlight, and handle input in `main.rs`. Files: `app/src/ui/menu.rs`, `app/src/main.rs`. Add tests for menu helper functions and describe manual test steps for the interactive parts."
-- Refactor: "Task: Extract panel list rendering to `app/src/ui/panels.rs` (if not present). Details: move helper functions, add unit tests for formatting helpers. Files: `app/src/ui.rs` -> `app/src/ui/panels.rs`. Ensure `cargo test -p fileZoom` passes."

Usage Guidance for VS Code Copilot Prompt Files
------------------------------------------------
- Place this file under `.github/instructions/` or use `*.prompt.md` for quick access in the Copilot UI.
- When invoking the prompt in the editor, paste the filled Task/Details sections. The assistant should return an actionable patch and test outputs.

Notes
-----
- Keep critical rules (hard constraints) at the top of prompts where possible — Copilot Code Review reads only the first ~4,000 characters of custom instruction files.
- If a requested change affects shared public API, include a short migration note and tests demonstrating the new behavior.
