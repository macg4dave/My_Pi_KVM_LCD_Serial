---
name: rust_perf
description: "Prompt template for performance profiling and optimization tasks in Rust projects."
---

Context
-------
- **Project**: LifelineTTY — HD44780 LCD daemon consuming newline-delimited JSON via `/dev/ttyUSB0 @ 9600 8N1` by default; overrides may point at `/dev/ttyAMA0`, `/dev/ttyS*`, or USB adapters without changing framing.
- **Storage guardrails**: only `~/.serial_lcd/config.toml` persists; all logs, caches, and telemetry must stay inside `/run/serial_lcd_cache`.
- **Performance goals**: keep RSS under 5 MB and avoid busy loops while obeying UART/LCD timing requirements.

Scope
-----
-- Typical files: `<src/*>` modules, hot paths, algorithms, or data structures (crate: `<crate_name>`).
-- Applies to profiling, benchmarking, and micro-optimizations.

Hard constraints
----------------
- Keep changes minimal and focused on measurable performance improvements.
- Preserve correctness and public API behavior.
- Run `cargo test -p <crate_name>` and include full output to confirm no regressions.
- Add benchmarks or performance tests when possible (`cargo bench` or criterion).

Prompt template
---------------
Task:
"""
<Brief summary of performance task>

Details:
- What to change: <describe optimization, e.g., replace data structure, reduce allocations, parallelize>
- Files: <list files to update>
- Tests/benchmarks: <describe which tests/benchmarks to add/update>
"""

Assistant instructions
----------------------
1. Provide a concise plan (2–3 bullets).
2. Make the smallest possible optimization consistent with idiomatic Rust.
3. Add or update benchmarks/tests to validate performance gains.
4. Run `cargo test -p <crate_name>` and paste the full output.
5. Return:
   - Short summary of changes with file paths.
   - Exact patch(s) in `apply_patch` diff format.
   - The `cargo test` output showing passing tests.
   - Suggested next steps (e.g., profiling tools, further optimizations).

Example prompts
---------------
- "Task: Optimize string concatenation in parser. Details: use `String::with_capacity` to reduce reallocations. Files: `src/parser.rs`. Add benchmark comparing old vs new."
- "Task: Parallelize directory scanning. Details: use `rayon` for parallel iteration. Files: `src/fs.rs`. Add integration test for correctness."
