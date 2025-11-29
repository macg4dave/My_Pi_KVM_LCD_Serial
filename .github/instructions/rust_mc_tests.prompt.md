---
name: rust_tests
description: "Prompt template for creating or updating tests and fixtures in Rust projects."
---

Scope
-----
-- Typical files: `<tests/*>`, unit tests in `<src/*>`, fixtures under `<tests/fixtures>` (crate: `<crate_name>`).

Hard constraints
----------------
- Always run `cargo test -p <crate_name>` locally and include the full output.
- Tests must be deterministic and not depend on external resources.
- Use `assert_fs` or temporary directories for filesystem fixtures.

Prompt template
---------------
Task:
"""
<Brief summary of test task>

Details:
- Which behavior to test: <describe function/feature>
- Files to change/add: <list files>
"""

Assistant instructions
----------------------
1. Provide a short plan (1–3 bullets).
2. Add the smallest test changes required and helper fixtures.
3. If new helper code is needed, add it with unit tests.
4. Run `cargo test -p <crate_name>` and paste the full output.
5. Suggest optional improvements to test coverage or dependencies if relevant.
