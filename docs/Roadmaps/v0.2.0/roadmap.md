# LifelineTTY Roadmap (v0.2.0 — Debug, Tests, Field Trials)

Updated 4 Dec 2025

LifelineTTY remains a single Rust daemon that ingests newline-delimited JSON via UART and drives an HD44780 LCD (PCF8574 @ 0x27). All work must obey the charter in `.github/copilot-instructions.md`: single binary, no sockets, no new transports, RAM-disk cache only (`/run/serial_lcd_cache`), persistent config at `~/.serial_lcd/config.toml`, stable CLI flags (`--run`, `--test-lcd`, `--test-serial`, `--device`, `--baud`, `--cols`, `--rows`, `--demo`, `--serialsh`, `--wizard`).

## Context & guardrails

- **Storage**: only `~/.serial_lcd/config.toml` is persistent; everything else stays under `/run/serial_lcd_cache`.
- **Interfaces**: UART defaults to `/dev/ttyUSB0` @ 9600 8N1; allow `/dev/ttyAMA0` and `/dev/ttyS*` via config/CLI. LCD via defaults to PCF8574 @ 0x27. No Wi‑Fi/BLE/sockets/HTTP.
- **Quality bar**: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` must pass on x86_64 and ARMv6. No `unsafe` or unchecked `unwrap()` in production paths.
- **Performance**: target <5 MB RSS, no busy loops; respect backoff for serial/LCD errors.
- **Testing expectation**: every behavioral change ships with unit + integration coverage in `tests/` and `src/**`; CLI flags retain regression tests.

### AI execution cues (read first)

- **Storage**: RAM-disk only at `/run/serial_lcd_cache`; only `~/.serial_lcd/config.toml` persists. Never write elsewhere; clean up cache artifacts.
- **CLI surface (stable)**: `--run`, `--test-lcd`, `--test-serial`, `--device`, `--baud`, `--cols`, `--rows`, `--demo`, `--serialsh`, `--wizard`. Do **not** add new flags in v0.2.0.
- **Protocols/transports**: UART newline JSON or `key=value`; HD44780 + PCF8574 @ 0x27 only; no sockets/BLE/HTTP/PTYs.
- **Allowed crates**: std + charter whitelist (hd44780-driver, linux-embedded-hal, rppal, serialport, tokio-serial async, tokio for async serial, serde/serde_json, crc32fast, ctrlc, anyhow/thiserror, log/tracing, calloop, async-io, syslog-rs, os_info, crossbeam, rustix, sysinfo, futures, directories, humantime, serde_bytes, bincode, clap_complete, indicatif, tokio-util). New crates must be added to `docs/lifelinetty_creates.md` and stay within the whitelist.
- **Quality bar**: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` on x86_64 + ARMv6; no `unsafe`, no unchecked `unwrap()` in production paths; keep RSS <5 MB and avoid busy loops.

## Blockers (B1–B6) — closed, keep guardrails tight

| ID | Status | Owner | Action (keep warm) | Acceptance checks |
| --- | --- | --- | --- | --- |
| **B1 — Rename fallout** | ✅ closed (2 Dec 2025) | Release | Re-scan `Makefile`, `packaging/`, `docker/`, `scripts/` for `seriallcd` strings before each release cut. | `ripgrep seriallcd` returns none; package names/units stay `lifelinetty`. |
| **B2 — Charter sync** | ✅ closed | Eng | `.github/copilot-instructions.md` matches UART/LCD/cache rules; roadmap restates the same. | Doc diff vs charter shows no drift; CLI defaults remain `/dev/ttyUSB0@9600` 8N1. |
| **B3 — Cache policy audit** | ✅ closed | Eng | Ensure logs/temp stay under `/run/serial_lcd_cache`; only `~/.serial_lcd/config.toml` persists. | `tests/bin_smoke.rs` + integration mock enforce paths; no CI leaks outside cache/config. |
| **B4 — CLI docs/tests parity** | ✅ closed | Eng | Keep README/help tables aligned; smoke tests cover `--run --test-lcd --test-serial --device --baud --cols --rows --demo --serialsh --wizard`. | `cargo test -p lifelinetty` smoke passes; README/help text match. |
| **B5 — AI prompt refresh** | ✅ closed | Eng | Prompts/instructions describe mission, guardrails, cache policy. | `.github/copilot-instructions.md` and roadmap match charter text. |
| **B6 — Release tooling sanity** | ✅ closed | Release | Packaging/Docker scripts emit only `lifelinetty_*`; no legacy symlinks. | `scripts/local-release.sh` outputs lifelinetty artifacts only; systemd units named `lifelinetty.service`. |

> Keep the B1 sweep active before shipping anything in v0.2.0 to prevent `seriallcd` regressions.

## v0.2.0 scope & goals

Focus: **Debug existing features, expand tests, and run real-world user trials** without adding new protocols or flags.

### Success criteria

- Stability: zero known crashers across serial/link/LCD paths; watchdog/heartbeat never wedges the render loop; no cache writes outside `CACHE_DIR`.
- Test depth: expanded regression coverage for config loading, LCD driver behaviors, serial backoff/telemetry, and negotiation/tunnel happy + failure paths.
- Field readiness: documented, repeatable on-device test matrix with Pi 1 hardware, common UART adapters, and multiple baud rates; issues from trials are reproduced and fixed with tests.
- Resource safety: sustained runs stay under 5 MB RSS; no unbounded buffers during long tunnels or polling.

## v0.2.0 priorities (in scope)

| ID / Theme | Status | Owner | What “done” means (v0.2.0) | Guardrails & acceptance |
| --- | --- | --- | --- | --- |
| **P1 — Config hardening (regression sweep)** | 🟢 Done | Config | Config loader now backfills all defaults into `~/.serial_lcd/config.toml` (even when partial/empty) for user visibility; env overrides supported for device/baud/cols/rows; fixtures plus integration tests cover malformed + partial TOML and env precedence; smoke covers CLI cols/rows/baud precedence. | Persist only to `~/.serial_lcd/config.toml`; no new keys unless already documented. Acceptance: fixtures added, integration mock asserts env overrides, smoke covers cols/rows/baud precedence. |
| **P2 — LCD driver regressions** | 🔵 Open | Display | Increase coverage for flicker-free writes, CGRAM/icon churn, backlight/blink toggles, and demo overlays; add host-mode and (where possible) stub tests plus doc updates (`docs/lcd_patterns.md`), ensuring icon churn respects the 8-slot limit with deterministic fallbacks. | No new driver APIs; keep HD44780 + PCF8574 only. Acceptance: icon churn test proves ≤8 slots with deterministic eviction; docs updated; host-mode stub test added. |
| **P3 — Serial backoff telemetry** | 🔵 Open | Serial | Verify structured error mapping (permission/unplug/framing) and reconnect counters; add assertions around log locations and rotation under `/run/serial_lcd_cache/serial_backoff.log`. | No new transports; respect 9600 baud floor and backoff timing. Acceptance: log path asserted in tests; reconnect counters exposed; backoff timing covered. |
| **P4 — Polling/heartbeat stability** | 🔵 Open | Core | Harden polling watchdog interactions and startup/shutdown ordering so LCD stays responsive during reconnects; add time-budgeted tests. | Keep RSS <5 MB; timers respect existing config fields. Acceptance: watchdog/polling tests show render loop stays responsive during reconnects; timing budgets enforced. |
| **Bug backlog & field fixes** | 🔵 Open | Rotation | Triaged defects from real hardware runs get repro tests and fixes in the same PR; no feature creep. | Must ship with regression tests and doc updates as needed; log paths remain in cache. |

> Execution note for AI/agents: keep changes minimal, land tests alongside fixes, and prefer touching only the files listed under each priority/workstream.

### Parity notes (stay aligned)

- **Dependency parity**: Reconcile `Cargo.toml` with the allowed crate list. If `lz4_flex`, `zstd`, or `embedded-hal` features are present, either gate them per charter or document justification in `docs/lifelinetty_creates.md` in the same PR.
- **CLI parity**: `--serialsh` and `--wizard` are **stable**; no new flags in v0.2.0. Keep README/help tables and `tests/bin_smoke.rs` in lockstep.

### Explicitly out of scope for v0.2.0

- No new CLI flags or protocol changes (tunnel/file transfer/negotiation stay as-is).
- No new dependencies beyond the charter-approved set; do not remove existing ones without approval.
- No new transports (network, BLE, PTY) or storage locations outside cache/config.

## Workstreams

- **Stability sweeps**
  - Files: `src/config/loader.rs`, `src/config/mod.rs`, `src/serial/{mod,backoff,telemetry}.rs`, `src/app/{connection,watchdog,polling}.rs`, `tests/{bin_smoke,integration_mock,fake_serial_loop,latency_sim}.rs`.
  - Actions: tighten validation errors, add fixtures for malformed config, assert log placement/rotation in cache, ensure watchdog + polling timers never starve render loop.
  - Cache audit (P3/B3): assert serial backoff/telemetry/tunnel logs stay under `/run/serial_lcd_cache` with rotation limits, extend `tests/integration_mock.rs`/bin smoke to verify paths.

- **LCD regression coverage**
  - Files: `src/display/{lcd,overlays,icon_bank}.rs`, `src/lcd_driver/{mod,pcf8574}.rs`, `tests/bin_smoke.rs`, `tests/integration_mock.rs`, `docs/lcd_patterns.md`.
  - Actions: add host-mode/stub tests for CGRAM swaps and backlight/blink paths; document demo patterns and expected visuals; ensure icon churn respects 8-slot limit with deterministic fallbacks.

- **Field trial readiness (real hardware + doc updates)**
  - Files: `docs/dev_test_real.md`, `docs/architecture.md`, `README.md`, `docs/demo_playbook.md`, `samples/` payloads.
  - Actions: script repeatable runs using existing `devtest/*.sh`; capture expected outputs; document failure triage steps; ensure cache paths are cleared between runs.

- **Dependency alignment**
  - Files: `Cargo.toml`, `docs/lifelinetty_creates.md`.
  - Actions: reconcile listed crates (e.g., `lz4_flex`, `zstd`, `embedded-hal`) with the charter-approved set; either justify/append to the allowed list or gate/prune unused features, updating docs in the same PR.

- **CLI flag parity & status check**
  - Files: `README.md`, `src/cli.rs`, `tests/bin_smoke.rs`.
  - Actions: reconcile `--serialsh`/`--wizard` status (stable vs milestone-gated), ensure `--help`/README flag tables match charter, and add/refresh smoke assertions for allowed flags.


## Real-world test matrix

| Scenario | Device/adapter | Baud & framing | LCD geometry | Payload set | Expected checks |
| --- | --- | --- | --- | --- | --- |
| Baseline | Pi 1 Model A + `/dev/ttyUSB0` (FTDI) | 9600 8N1 | 16×2 | `samples/payload_examples.json` | Clean render, no flicker; cache logs only under `/run/serial_lcd_cache`; RSS <5 MB for 2h run. |
| Alt TTY | Pi 1 Model A + `/dev/ttyAMA0` | 9600 8N1 | 16×2 | same as above | Wizard and config honor device override; reconnect/backoff logs categorized. |
| Higher baud probe | Pi 1 Model A + `/dev/ttyUSB0` | 19200 8N1 (post-wizard) | 16×2 | demo payloads | No framing errors; heartbeat/watchdog stay green. |
| LCD stress | Pi 1 Model A + I²C backpack | 9600 8N1 | 20×4 (if available) | icon-heavy samples | CGRAM swaps ≤8 icons; no display corruption; overlays render. |
| Tunnel coexists | Pi 1 Model A + `/dev/ttyUSB0` | 9600 8N1 | 16×2 | command tunnel enabled via `--serialsh` | Busy/Exit codes correct; LCD continues rendering; cache logs under `tunnel/` only. |

Record outcomes and defects in RAM-disk logs; reproduce with tests before closing.

**Execution notes:**

- Logs/artifacts live under `/run/serial_lcd_cache/{serial_backoff,watchdog,tunnel,wizard,polling}`; never persist elsewhere.
- Owner: field-ops rotation. Exit when each scenario has a dated log bundle in cache plus a matching regression test (or a filed defect).

## Testing & validation (per change)

1. `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` on x86_64 (and ARMv6 when available).
2. Update/add unit + integration tests alongside fixes (config, LCD, serial, tunnel, polling).
3. Run at least one matrix scenario per PR touching runtime behavior; capture logs under `/run/serial_lcd_cache` for review.
4. Document user-facing changes in README and relevant docs (`docs/dev_test_real.md`, `docs/lcd_patterns.md`, `docs/architecture.md`).

## Out-of-scope reminders

- No network/PTY/HTTP features; UART + LCD only.
- Network for testing only via existing scripts; no runtime network code. e.g. SSH tunnels, client-server devlopment setups.
- No writes outside `/run/serial_lcd_cache` and `~/.serial_lcd/config.toml`.
- No new crates beyond the approved list (see `docs/lifelinetty_creates.md`).

## Rollout & tracking

- Keep this file as the active roadmap for v0.2.0; archive older versions under `docs/Roadmaps/v0.1.0` as needed.
- Field-trial issues must land with a regression test before closing.

### Milestone execution cues

- **Entry criteria**: P1–P4 not blocked; B1 sweep re-checked; cache/log paths enforced in smoke tests.
- **Exit criteria**: milestone doc section updated with date; matching tests/docs merged; cache-log evidence stored under `/run/serial_lcd_cache/<milestone>/` or existing log families.
- **Sign-off**: Eng owner + Release acknowledge in PR summary; note x86_64 + ARMv6 test runs (or why skipped).

---

### Milestone 1 — Real-hardware devtest loop (--In progress)

- **Goal:** Ship a repeatable SSH-based `/devtest` loop for Pi 1 field trials (no new transports/flags) that exercises the v0.2.0 real-world matrix without leaving processes or logs outside `/run/serial_lcd_cache`.
- **Implementation:** `devtest/run-dev.sh`, `devtest/watch.sh`, and `devtest/watch-remote.sh` run end-to-end with `devtest/dev.conf` (USB0 + S0 @ 9600; USB0 @ 19200; 16×2). Scripts keep cache log watching inside `/run/serial_lcd_cache` and rely solely on SSH/scp/tmux.
- **Logging & storage:** `docs/dev_test_real.md` documents cache/log collection and matrix checklist with v0.2.0 wording; `devtest/dev.conf.example` defaults stay within charter (9600 baud, `/dev/ttyUSB0`, cache watcher) and remind testers to stop `lifelinetty.service` before runs.
- **Docs/tests:** Roadmap/workstreams point to the guide; matrix runs record outcomes, and defects uncovered during the loop land with reproducible notes or tests before closing the milestone.

### Milestone 2 — Enhanced first-run wizard (--Planned)

- **Goal:** Make the guided setup noticeably more helpful without adding new flags or transports: auto-guess the likely TTY and baud, suggest client/server roles, and offer optional SSH/scp/tmux snippets to move binaries/configs/logs between hosts—while keeping all persistence to `~/.serial_lcd/config.toml` and transcripts under `/run/serial_lcd_cache`.
- **Implementation:**
  - Extend `src/app/wizard.rs` prompts to (a) scan common serial paths (`/dev/ttyUSB*`, `/dev/ttyAMA*`, `/dev/ttyS*`) and propose a ranked default; (b) ask usage intent (server/client/standalone) and LCD presence; (c) surface opt-in helper text for copying the binary/config or log bundles via SSH/scp/tmux (text only, no network code executed); (d) optionally probe baud safely starting at 9600 with backoff; (e) persist choices to `~/.serial_lcd/config.toml` and record the transcript plus probe outcomes in `CACHE_DIR/wizard.log`.
  - Keep CLI stable (`--wizard` only); headless/scripted mode continues via `LIFELINETTY_WIZARD_SCRIPT` with deterministic defaults when stdin is not a TTY.
- **Logging & storage:** All wizard transcripts and suggested snippet text stay under `/run/serial_lcd_cache`; no writes outside `CACHE_DIR` and `~/.serial_lcd/config.toml`. No automated SSH/scp/tmux execution—only displayed snippets for the user to copy.
- **Docs/tests:** Update README and wizard docs to show new prompts and sample SSH/scp/tmux snippets (e.g., copying `lifelinetty` and `~/.serial_lcd/config.toml`, or tailing cache logs via tmux). Add/extend coverage in `tests/bin_smoke.rs` and `tests/integration_mock.rs` for auto-TTY ranking, role prompts, LCD/log questions, and deterministic scripted answers; keep transcript path assertions.

### Milestone 3 — Wizard link-speed rehearsal (--Planned)

- **Goal:** On the first server/client pairing, automatically run a guided link-speed rehearsal to pick the highest stable serial baud and framing the pair can sustain, then store that as the preferred config without adding new flags or transports.
- **Implementation:**
  - When both ends identify as server/client (via wizard role selection), trigger an opt-in speed test before the first full session: iterate a bounded list of baud candidates (starting at 9600) with framing checksums and retries, settle on the highest reliable pick, and write the result to `~/.serial_lcd/config.toml`.
  - Keep the existing `--wizard` flag and scripted mode; the rehearsal is driven through wizard prompts and uses the current serial stack—no sockets or new protocols.
- **Logging & storage:** Record each probe attempt, errors, and the final chosen baud under `/run/serial_lcd_cache/wizard/` (e.g., `link_rehearsal.log`), alongside the normal wizard transcript. Persist only the chosen settings to `~/.serial_lcd/config.toml`; no other writes outside `CACHE_DIR`.
- **Docs/tests:** Expand README and wizard docs to explain the first-pairing speed test and how to re-run it. Add tests in `tests/bin_smoke.rs` and `tests/integration_mock.rs` for the rehearsal flow (scripted answers, capped candidate list, transcript/log path) and ensure fallback to 9600 on failure. Keep RSS/backoff guardrails and existing CLI behavior unchanged.

### Milestone 4 — Serial shell under systemd (--Planned)

- **Goal:** Document and harden how operators run `lifelinetty --serialsh` on boxes where `lifelinetty.service` is already managing the daemon, without adding new flags or transports. The shell must remain an interactive, manual invocation that never fights systemd for the same TTY or introduces new storage locations.
- **Implementation:**
  - Spell out the expected operator flow: (a) if the service has the target TTY open, stop/disable it temporarily (`systemctl stop lifelinetty.service`) or point `--device` at an idle TTY; (b) open an SSH/tmux session and launch `lifelinetty --serialsh [--device ... --baud ...]`; (c) on exit, restart the service if desired. No new CLI switches beyond the existing `--serialsh`, `--device`, and `--baud` overrides.
  - Keep cache/config handling identical to the daemon path: all transient shell artifacts live under `/run/serial_lcd_cache/serialsh*`, and persistent settings remain in `~/.serial_lcd/config.toml`.
- **Logging & storage:** Reinforce that all shell logs/errors go to stderr (journald when invoked under systemd units) or cache files under `/run/serial_lcd_cache/serialsh*`; do not write anywhere else. Suggest `journalctl -u lifelinetty.service` for prior daemon logs but avoid altering the unit.
- **Docs/tests:** Update README and wizard helper snippets to clarify the systemd flow (stop service, run shell via SSH/tmux, restart). Add a brief checklist in this roadmap and ensure `tests/bin_smoke.rs` keeps covering `--serialsh` help/usage so behavior stays stable. No new tests are required for systemd itself; this milestone is documentation and ops guidance only.

