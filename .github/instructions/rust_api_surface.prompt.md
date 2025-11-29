---
name: rust_api_surface
description: "Prompt template for managing and documenting the public API surface in Rust projects."
---

Scope
-----
-- Typical files: `<src/lib.rs>`, `<src/*>` modules, public exports (crate: `<crate_name>`).
-- Applies to adding/removing public items, re-exports, and API visibility changes.

Hard constraints
----------------
- Preserve backward-compatible public APIs unless explicitly requested otherwise.
- If breaking changes are necessary, include migration notes and update docs/tests.
- Run `cargo test -p <crate_name>` and include full output.

Prompt template
---------------
Task:
"""
<Brief summary of API surface change>

Details:
- What to change: <public exports, visibility, re-exports, module structure>
- Files: <list files>
- Tests/migration notes: <describe changes to tests or migration guidance>
"""

Assistant instructions
----------------------
1. Provide a concise plan (2–3 bullets).
2. Make minimal changes; prefer additive APIs over breaking ones.
3. Add or update tests that demonstrate the public contract.
4. If breaking, add migration notes in docs and include tests.
5. Run `cargo test -p <crate_name>` and paste the full output.
6. Return:
   - Short summary of changes with file paths.
   - Exact patch(s) in `apply_patch` diff format.
   - The `cargo test` output showing passing tests.
   - Suggested next steps or optional improvements.

Example prompts
---------------
- "Task: Export `ErrorKind` enum from crate root. Details: add `pub use error::ErrorKind;` in `src/lib.rs`. Files: `src/lib.rs`. Add unit test verifying visibility."
- "Task: Hide internal helper `parse_line()`. Details: make function private. Files: `src/parser.rs`. Add migration note in README."
