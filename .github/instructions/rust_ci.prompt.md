---
name: rust_ci
description: "Prompt template for CI/CD work specific to the LifelineTTY project."
---

Scope
-----
- Typical files: `.github/workflows/*.yml`, `Makefile`, `scripts/local-release.sh`, Dockerfiles under `docker/`, and packaging metadata.
- Applies to build/test automation, linting, cross-compilation (armv6/armv7/arm64), and release verification for the `lifelinetty` crate.
- CI must run `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, and (when requested) packaging tasks while honoring the RAM-disk + systemd constraints described in `.github/copilot-instructions.md`.
- Serial baseline: `/dev/ttyUSB0 @ 9600 8N1` with config/CLI overrides for `/dev/ttyAMA0`, `/dev/ttyS*`, and USB adapters; keep docs and workflows consistent with these defaults.

Hard constraints
----------------
1. Keep workflow edits minimal and scoped to the requested change.
2. Always run `cargo fmt --check`, `cargo clippy -- -D warnings`, and the appropriate `cargo test` matrix (host + cross targets when relevant).
3. Preserve existing jobs unless the request explicitly removes them; add comments when behavior changes.
4. Ensure CI respects the project charter (no network calls, CLI flags remain stable, packaging still installs `lifelinetty`).
5. When adding uploads (artifacts, releases), ensure they contain only RAM-safe outputs (no secrets, no persistent config files).

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
1. Provide a concise 2–3 bullet plan outlining workflow edits.
2. Make the smallest possible CI/CD changes consistent with repo style (matrix layout, caching, packaging scripts).
3. Ensure workflows run `cargo fmt --check`, `cargo clippy -- -D warnings`, and the requested `cargo test`/cross-build steps.
4. Run the most relevant `cargo test` command locally (typically `cargo test`) and paste the full output to prove correctness.
5. Return:
   - Short summary of changes with file paths.
   - Exact patch(es) in `apply_patch` format.
   - The `cargo test` output showing passing tests.
   - Suggested next steps (e.g., caching improvements, matrix expansion, release automation).

Example prompts
---------------
- "Task: Add `armv6` cross-build job using Dockerfile. Details: extend `.github/workflows/ci.yml` matrix to call `docker/Dockerfile.armv6`. Include `cargo test` on host."
- "Task: Enforce `cargo fmt --check` + `cargo clippy -- -D warnings` before packaging. Details: split lint job out of main workflow. Files: `.github/workflows/ci.yml`."
- "Task: Cache `target/` + crates.io index between jobs. Details: add `actions/cache` keyed by `Cargo.lock`. Files: `.github/workflows/ci.yml`."
