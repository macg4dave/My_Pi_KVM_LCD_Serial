# LifelineTTY Copilot Charter
Purpose: keep every AI-assisted change aligned with the Dec 2025 roadmap in `docs/roadmap.md`. All guidance here is binding—stay within scope, finish the blockers first, and move through priorities only when explicitly scheduled.

## One-line mission
Ship a single, ultra-light Rust daemon for Raspberry Pi 1 (ARMv6) that reads newline-delimited JSON (and key=value fallbacks) from `/dev/ttyUSB0` at 9600 baud, renders two HD44780 LCD lines via a PCF8574 I²C backpack, and runs for months without exceeding 5 MB RSS.

## Roadmap alignment (read before coding)
1. **Blockers (B1–B6)** — rename fallout, charter sync, cache-policy audit, CLI docs/tests, prompt refresh, and release tooling. Nothing else lands until these are closed.
2. **Priority queue (P1–P20)** — once blockers are done, tackle P1–P4 (rename lint, baud audit, config hardening, LCD regression tests) before touching telemetry, tunnels, or protocol work.
3. **Milestones (A–G)** — every large feature (command tunnel, negotiation, file push/pull, polling+heartbeat, display expansion, strict JSON+compression, serial shell) builds on specific priorities. Reference the milestone workflows in `docs/roadmap.md` when planning.
4. Always annotate changes with the roadmap item they advance (e.g., “P3: Config loader hardening”) so we can trace progress.

## Core behavior (never change without approval)
- **IO**: UART input via `/dev/ttyUSB0` (9600 8N1) by default; config/CLI overrides may point to `/dev/ttyAMA0`, `/dev/ttyS*`, or USB adapters as long as they speak the same framing. LCD output via HD44780 + PCF8574 @ 0x27. No Wi-Fi, Bluetooth, sockets, HTTP, USB HID, or other transports.
- **CLI**: binary is now invoked as `lifelinetty` and keeps the old `seriallcd` name as a compatibility alias. Supported flags: `--run`, `--test-lcd`, `--test-serial`, `--device`, `--baud`, `--cols`, `--rows`, `--demo`. Do **not** add flags or modes unless the roadmap explicitly calls for it (e.g., future `--serialsh`).
- **Protocols**: newline-terminated JSON or `key=value` pairs; LCD output is always two 16-character lines. Exit code 0 on success, non-zero on fatal errors.

## Storage + RAM-disk policy (mandatory)
- Persistent writes are limited to `~/.serial_lcd/config.toml`.
- Everything else (logs, temp payloads, LCD caches, tunnel buffers) must live under `/run/serial_lcd_cache`.
- The application must never call `mount`, create tmpfs, require sudo, or write outside the RAM disk.
- Hard-code `const CACHE_DIR: &str = "/run/serial_lcd_cache";` and treat it as ephemeral (clean up after yourself, expect wipe on reboot).
- All logging goes to stderr or files inside the RAM disk.

## Tech + dependencies
- **Language**: Rust 2021, `lifelinetty` crate only.
- **Allowed crates**: std, `hd44780-driver`, `linux-embedded-hal`, `rppal`, `serialport`, `tokio-serial` (feature `async-serial`), `tokio` (only for async serial), `serde`, `serde_json`, `crc32fast`, `ctrlc`, optional `anyhow`, `thiserror`, `log`, `tracing`. New crates require approval.
- **Banned crates**: anything pulling in a network stack, heavyweight runtime, database, or filesystem abstraction that writes outside allowed paths.
- **Build/test commands**: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, `cargo build --release` when needed. All must pass on x86_64 **and** ARMv6.

## Interfaces that must stay stable
- CLI name + flags (see above) until roadmap explicitly authorizes changeover to `lifelinetty` binary.
- Config schema at `~/.serial_lcd/config.toml` and payload contracts in `src/payload/`.
- LCD command set (HD44780) and I²C wiring (PCF8574 @ 0x27).
- Serial framing: newline JSON / key=value. Keep compatibility layers for legacy SerialLCD senders (see roadmap item P19).

## Quality bar & testing
- Every behavioral change gets matching tests (unit + integration under `tests/`). All CLI flags must have regression coverage.
- Run `cargo fmt`, `cargo clippy -- -D warnings`, and the relevant `cargo test` targets before submitting. Include full output in reviews/PRs.
- Avoid `unsafe` and unchecked `unwrap()` in production code.
- Maintain <5 MB RSS, no busy loops. Add backoff/retry handling for serial and LCD errors.
- Document user-facing changes (README, docs/*.md, Rustdoc). Public functions and types require Rustdoc comments.
- Never silence lints globally (`#[allow(dead_code)]`, etc.) without explicit approval and clear justification.

## Task request template (use verbatim)
Task:
"""
<One-line summary of the change>

Details:
- Roadmap link: <B#/P#/Milestone reference>
- What to change: <short description + acceptance criteria>
- Files to consider: <list or leave blank>
- Tests: <which tests to add/update or leave blank>
- Constraints / do not modify: <guardrails>
"""

## Agent rules (apply to every change)
1. If the request conflicts with this charter or the roadmap, clarify before coding.
2. Make the smallest change that satisfies the acceptance criteria and roadmap intent.
3. Preserve stable interfaces unless the roadmap explicitly authorizes modifications (and provide migration notes when it does).
4. Update tests, docs, and roadmap cross-references together in the same PR.
5. Include `cargo test` output and note any platform-specific considerations (x86_64 vs ARMv6).
6. Resist feature creep—no speculative refactors or new capabilities beyond the roadmap milestones.

## Development + review environment
- Target hardware: Raspberry Pi 1 Model A (ARMv6, Debian/systemd). Cross-compile or use QEMU/docker images in `docker/` and `scripts/local-release.sh` for packaging.
- Services: `lifelinetty.service` / `seriallcd.service` must remain systemd-friendly (no extra daemons). When editing, ensure release artifacts (`packaging/`, Dockerfiles) stay consistent with the rename plan (B6).

## Documentation expectations
- Keep README, `docs/architecture.md`, `docs/roadmap.md`, `docs/lcd_patterns.md`, and `samples/` payloads synchronized with functionality.
- When adding protocol/CLI changes, update `spec.txt` (or create it if missing) and annotate roadmap items with the new state.
- Comment non-obvious state machines (render loop, serial backoff, payload parser) so future contributors can reason about them.
- All doc updates must ship with their accompanying code changes.