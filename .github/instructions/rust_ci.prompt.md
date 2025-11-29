---
name: rust_ci
description: "Prompt template for CI/CD pipeline tasks in Rust projects."
---

Scope
-----
-- Typical files: `.github/workflows/*`, `Cargo.toml`, `Cargo.lock`, or project-specific CI configs.
-- Applies to build/test automation, linting, formatting, and deployment steps.

Hard constraints
----------------
- Keep CI changes minimal and focused on the requested update.
- Preserve reproducibility and correctness of builds/tests.
- Ensure `cargo build`, `cargo test`, `cargo fmt`, and `cargo clippy` run successfully in CI.
- Do not remove existing jobs unless explicitly requested.
- Document changes in workflow comments or README if behavior changes.

Prompt template
---------------
Task:
"""
<Brief summary of CI/CD task>

Details:
- What to change: <describe workflow edits, e.g., add test matrix, run clippy, cache dependencies>
- Files: <list files to update>
- Constraints / do not modify: <list any files/behaviors that must remain unchanged>
"""

Assistant instructions
----------------------
1. Provide a concise plan (2–3 bullets).
2. Make the smallest possible CI/CD changes consistent with project style.
3. Add or update workflow steps to validate builds/tests.
4. Run `cargo test -p <crate_name>` locally and paste the full output to confirm correctness.
5. Return:
   - Short summary of changes with file paths.
   - Exact patch(s) in `apply_patch` diff format.
   - The `cargo test` output showing passing tests.
   - Suggested next steps (e.g., caching, deployment, coverage reporting).

Example prompts
---------------
- "Task: Add GitHub Actions workflow to run `cargo fmt --check` and `cargo clippy`. Details: update `.github/workflows/ci.yml`. Files: `.github/workflows/ci.yml`. Ensure tests pass."
- "Task: Add test matrix for Rust versions 1.70, 1.71, stable. Details: update workflow. Files: `.github/workflows/test.yml`. Run `cargo test` in each job."
- "Task: Enable dependency caching in CI. Details: add `actions/cache` for cargo registry and target dir. Files: `.github/workflows/ci.yml`."
