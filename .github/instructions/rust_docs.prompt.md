---
name: rust_docs
description: "Prompt template for documentation and migration note tasks in Rust projects."
---

Scope
-----
-- Typical files: `<src/lib.rs>`, `<src/*>` for doc-comments, `README.md`, `CHANGELOG.md`, or other project docs.
-- Applies to adding/updating documentation, migration notes, and developer guidance.

Hard constraints
----------------
- Keep documentation changes minimal and focused on the requested update.
- Preserve accuracy of public APIs and examples.
- Run `cargo test -p <crate_name>` to ensure code examples remain valid.
- If behavior changes, include a migration note and update docs accordingly.

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
1. Provide a concise plan (2–3 bullets).
2. Make the smallest possible documentation changes consistent with project style.
3. Add or update doctests/examples to validate documentation snippets.
4. Run `cargo test -p <crate_name>` and paste the full output.
5. Return:
   - Short summary of changes with file paths.
   - Exact patch(s) in `apply_patch` diff format.
   - The `cargo test` output showing passing tests.
   - Suggested next steps or optional improvements.

Example prompts
---------------
- "Task: Add doc-comments for `Config::load()`. Details: explain parameters and return type. Files: `src/config.rs`. Add doctest showing usage."
- "Task: Update README with installation instructions. Details: add `cargo install <crate_name>` example. Files: `README.md`. Ensure doctests pass."
- "Task: Add migration note for renamed function `App::start()` → `App::run()`. Details: update docs and changelog. Files: `README.md`, `CHANGELOG.md`."
