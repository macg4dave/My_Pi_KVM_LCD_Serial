---
name: rust_api
description: "Prompt template for public API / library-level changes in Rust projects."
---

Scope
-----
-- Typical files: `<src/lib.rs>`, `<src/app.rs>`, public helpers in `<src/*>` (crate name: `<crate_name>`).

Hard constraints
----------------
- Preserve backward-compatible public APIs unless a breaking change is explicitly requested.
- If breaking changes are necessary, include a migration note and tests.
- Run `cargo test -p <crate_name>` and include full output.

Prompt template
---------------
Task:
"""
<Brief summary of API change>

Details:
- What to change: <public API additions/removals/behavior changes>
- Files: <list files>
- Tests/migration notes: <describe changes to tests or migration guidance>
"""

Assistant instructions
----------------------
1. Provide a concise plan (2 bullets).
2. Make minimal changes; prefer additive APIs.
3. Add tests that demonstrate the public contract.
4. If breaking, add a migration note in docs and include tests.
5. Run `cargo test -p <crate_name>` and include output.
6. Suggest optional improvements or dependencies if relevant.
