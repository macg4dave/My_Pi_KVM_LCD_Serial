---
name: "Rust Feature Builder"
description: "Prompt template for adding features or capabilities to a Rust project."
---

Context
-------
-- Project: <project_name> (crate `<crate_name>`).
-- Source: <src_directory>. Tests: <tests_directory>.
-- Tooling: `cargo build`, `cargo test`, `cargo fmt`, `cargo clippy`.

Hard constraints
----------------
- Keep the change minimal and scoped to the requested feature.
- Maintain existing public APIs and CLI outputs unless explicitly allowed to change them.
- Add or update tests that validate the new behavior.
- Run `cargo test -p <crate_name>` (or the specified test command) and share the full output.
- Document user-facing behavior (Rustdoc or README) when it changes.

Prompt template
---------------
Task:
"""
<One-line summary of the feature>

Details:
- What to build: <short description and acceptance criteria>
- Files to touch: <list or leave blank>
- Tests: <which tests to add/update or leave blank>
- Docs: <README/API docs to update or leave blank>
- Constraints / do not modify: <list any files/behaviors that must remain unchanged>
"""

Assistant instructions
----------------------
1. Outline a brief plan (2–3 bullets).
2. Implement the smallest viable slice of the feature.
3. Add focused unit/integration tests that prove the feature works.
4. Update docs or inline Rustdoc for user-facing changes.
5. Run the specified `cargo test` command; include the full output.
6. Deliver:
   - Short summary of changes with file paths.
   - Exact patch(es) in `apply_patch` format.
   - Test output showing passing runs.
   - Optional next steps (e.g., follow-up polish or docs).

Example prompts
---------------
- "Task: Add `--dry-run` flag to the CLI. Details: the flag should log planned actions without modifying files. Files: `src/main.rs`, `src/cli.rs`. Tests: add unit test for `Args::dry_run` and integration test for log output. Docs: README flag table."
- "Task: Support JSON output for the `status` subcommand. Details: add serializer, keep text output default. Files: `src/status.rs`, `src/output.rs`. Tests: integration test for JSON schema. Constraints: do not change existing text output format."
