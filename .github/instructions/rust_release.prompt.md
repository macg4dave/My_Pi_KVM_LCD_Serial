---
name: "Rust Release Prep"
description: "Prompt template for preparing a Rust crate or workspace release."
---

Context
-------
-- Project: <project_name> (crate `<crate_name>` or workspace).
-- Source: <src_directory>. Tests: <tests_directory>.
-- Tooling: `cargo build`, `cargo test`, `cargo fmt`, `cargo clippy`, `cargo publish --dry-run`.

Hard constraints
----------------
- Bump versions consistently across `Cargo.toml` and `Cargo.lock` (if tracked).
- Update changelog/release notes when behavior changes.
- Keep API and CLI compatibility unless a breaking change is explicitly requested.
- Run `cargo test` (and `cargo fmt`/`cargo clippy` if required) before finalizing.
- Do not publish crates; only prepare the release artifacts and instructions.

Prompt template
---------------
Task:
"""
<One-line summary of the release task>

Details:
- Version: <new version> (workspace members affected: <list or leave blank>)
- Changelog: <sections to update or leave blank>
- Checks to run: <commands like `cargo test -p <crate_name>`, `cargo fmt --check`, `cargo clippy`>
- Files to touch: <Cargo.toml paths, CHANGELOG.md, release notes>
- Constraints / do not modify: <list any files/behaviors that must remain unchanged>
"""

Assistant instructions
----------------------
1. Provide a compact plan (2–3 bullets).
2. Update versions, changelog, and metadata with minimal edits.
3. Ensure dependencies stay compatible; highlight any breaking changes.
4. Run the requested checks/tests and include full output.
5. Return:
   - Summary of changes with file paths.
   - Exact patch(es) in `apply_patch` format.
   - Test/check outputs.
   - Release notes or manual steps to finish publishing.

Example prompts
---------------
- "Task: Prepare v0.4.0 release. Details: bump crate version, update `CHANGELOG.md` with new features, ensure `Cargo.lock` is refreshed. Checks: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test -p app`."
- "Task: Add release notes for workspace crates `core` and `cli`. Details: versions to 1.2.0, note breaking change in CLI flag rename. Files: `crates/core/Cargo.toml`, `crates/cli/Cargo.toml`, workspace `CHANGELOG.md`. Constraints: do not publish."
