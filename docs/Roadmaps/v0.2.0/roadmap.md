# LifelineTTY Roadmap (v0.2.0 — Debug, Tests, Field Trials)

Updated 4 Dec 2025

LifelineTTY remains a single Rust daemon that ingests newline-delimited JSON via UART and drives an HD44780 LCD (PCF8574 @ 0x27). All work must obey the charter in `.github/copilot-instructions.md`: single binary, no sockets, no new transports, RAM-disk cache only (`/run/serial_lcd_cache`), persistent config at `~/.serial_lcd/config.toml`, and a **stable CLI defined by** `lifelinetty --help` (implemented in `src/cli.rs`).

## How to read this roadmap

This is an **execution document**, not a wish list.

- **Start here**: Blockers (must stay closed) → Priorities (P1–P4) → Field test matrix → Milestones.
- **“Done” means**: code + tests + docs land together, and the acceptance checks can be verified with the commands listed in this file.
- **Avoid drift**: when docs and code disagree, treat the following as sources of truth:
  - CLI surface: `lifelinetty --help` (implemented in `src/cli.rs`)
  - Cache policy: `.github/copilot-instructions.md` + `CACHE_DIR` constant in code
  - Payload contract: `src/payload/` (especially `src/payload/parser.rs`)

## At a glance (v0.2.0)

- **Ship intent:** debug + harden existing behavior; expand tests; complete real hardware field trials (no new transports/flags).
- **Where we are:** P1 ✅; P2–P4 🔵; Milestone 1 ✅; Milestone 2 ✅; Milestone 3–4 planned; Milestone 5 ✅.
- **Next execution steps (ordered):**
  1. Close **P2 / P2a** by removing LCD/icon fallbacks and updating `README.md`, demo payloads, and tests.
  2. Close **P3** by asserting serial backoff telemetry mappings and cache-log placement/rotation limits.
  3. Close **P4** by adding time-budgeted tests for polling/watchdog overlap during reconnect/backoff.
- **Field trial minimum (to call v0.2.0 “trial-ready”):** Baseline + Alt TTY + LCD stress + Permission denied + Unplug/replug completed on Pi 1, each with a dated cache bundle and either a passing regression test or a filed defect.

### Status legend

- **✅ closed:** blocker eliminated; keep guardrails warm.
- **🟢 Done:** merged; acceptance checks reproducible.
- **🔵 Open:** in scope but not yet completed (may be partially implemented).
- **🔴 Active:** currently being executed; expect follow-up PRs.
- **(--Planned):** documented intent only (no code merged).

### Tracking links (v0.2.0)

- Roadmap: `docs/Roadmaps/v0.2.0/roadmap.md`
- Change log: `docs/Roadmaps/v0.2.0/changelog.md`
- Milestones: `docs/Roadmaps/v0.2.0/milestone_1.md`
- Ops playbooks: `docs/dev_test_real.md`, `docs/demo_playbook.md`, `docs/lcd_patterns.md`

### Hardware assumptions (v0.2.0)

- **Primary LCD**: 16×2 HD44780 (PCF8574 @ 0x27).
- **Configurability**: cols/rows are still configurable via config/CLI, but all tests and docs should assume 16×2 unless explicitly labeled as “alternate geometry”.

### Per-PR validation checklist (v0.2.0)

Every PR that changes runtime behavior should include:

1. Tests: unit + integration coverage for the behavior you changed.
2. Cache safety: any new/changed log or temp path must stay under `/run/serial_lcd_cache` (and be asserted in tests if practical).
3. Docs parity: update README and any referenced docs/playbooks when behavior changes.
4. Local commands (minimum): `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`.
5. Platform note: explicitly state x86_64 + ARMv6 status (ran both, or why ARMv6 was skipped).
6. Perf sanity (when touching buffering/render/polling/backoff): run at least one matrix scenario for a 2h soak and note RSS <5 MB (record platform and command used).

## Context & guardrails

- **Storage**: only `~/.serial_lcd/config.toml` is persistent; everything else stays under `/run/serial_lcd_cache`.
- **Interfaces**: UART defaults to `/dev/ttyUSB0` @ 9600 8N1; allow `/dev/ttyAMA0` and `/dev/ttyS*` via config/CLI. LCD via defaults to PCF8574 @ 0x27. No Wi‑Fi/BLE/sockets/HTTP.
- **Quality bar**: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` must pass on x86_64 and ARMv6. No `unsafe` or unchecked `unwrap()` in production paths.
- **Performance**: target <5 MB RSS, no busy loops; respect backoff for serial/LCD errors.
- **Testing expectation**: every behavioral change ships with unit + integration coverage in `tests/` and `src/**`; CLI flags retain regression tests.

### AI execution cues (read first)

- **Storage**: RAM-disk only at `/run/serial_lcd_cache`; only `~/.serial_lcd/config.toml` persists. Never write elsewhere; clean up cache artifacts.
- **CLI surface (stable)**: treat `lifelinetty --help` as canonical (see `src/cli.rs`). The binary supports:
  - Commands: `lifelinetty run ...` (or omit `run` and pass flags directly), plus `--help` / `--version`.
  - Core serial/LCD overrides: `--device`, `--baud`, `--flow-control`, `--parity`, `--stop-bits`, `--dtr-on-open`, `--serial-timeout-ms`, `--cols`, `--rows`, `--pcf8574-addr`.
  - Logging + cache policy: `--log-level`, `--log-file` (must be under `/run/serial_lcd_cache`; also honors `LIFELINETTY_LOG_PATH`).
  - Config precedence: `--config-file <path>` loads a specific TOML instead of `~/.serial_lcd/config.toml` (env overrides still apply on top).
  - Modes / toggles: `--demo`, `--serialsh`, `--wizard`, `--payload-file`, `--backoff-initial-ms`, `--backoff-max-ms`, polling flags (`--polling/--no-polling`, `--poll-interval-ms`), and compression flags (`--compressed/--no-compressed`, `--codec`).

  Constraints enforced by CLI parsing today:
  - `--serialsh` cannot be combined with `--demo` or `--payload-file`.
- **Protocols/transports**: UART newline JSON or `key=value`; HD44780 + PCF8574 @ 0x27 only; no sockets/BLE/HTTP/PTYs.
- **Allowed crates**: std + charter whitelist (hd44780-driver, linux-embedded-hal, rppal, serialport, tokio-serial async, tokio for async serial, serde/serde_json, crc32fast, ctrlc, anyhow/thiserror, log/tracing, calloop, async-io, syslog-rs, os_info, crossbeam, rustix, sysinfo, futures, directories, humantime, serde_bytes, bincode, clap_complete, indicatif, tokio-util). New crates must be added to `docs/lifelinetty_creates.md` and stay within the whitelist.
- **Quality bar**: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` on x86_64 + ARMv6; no `unsafe`, no unchecked `unwrap()` in production paths; keep RSS <5 MB and avoid busy loops.

## Blockers (B1–B6) — closed, keep guardrails tight

| ID | Status | Owner | Action (keep warm) | Acceptance checks |
| --- | --- | --- | --- | --- |
| **B1 — Rename fallout** | ✅ closed (2 Dec 2025) | Release | Re-scan `Makefile`, `packaging/`, `docker/`, `scripts/` for `seriallcd` strings before each release cut. | `rg seriallcd` (or `grep -R seriallcd .`) returns none; package names/units stay `lifelinetty`. |
| **B2 — Charter sync** | ✅ closed | Eng | `.github/copilot-instructions.md` matches UART/LCD/cache rules; roadmap restates the same. | Doc diff vs charter shows no drift; CLI defaults remain `/dev/ttyUSB0@9600` 8N1. |
| **B3 — Cache policy audit** | ✅ closed | Eng | Ensure logs/temp stay under `/run/serial_lcd_cache`; only `~/.serial_lcd/config.toml` persists. | `tests/bin_smoke.rs` + integration mock enforce paths; no CI leaks outside cache/config. |
| **B4 — CLI docs/tests parity** | ✅ closed | Eng | Keep README/help tables aligned with `lifelinetty --help` (`src/cli.rs`); smoke tests cover help content and key precedence (e.g., `--config-file` behavior, env overrides) without requiring hardware. | `cargo test -p lifelinetty` smoke passes; smoke asserts core flags appear in help; README/roadmap/help text match. |
| **B5 — AI prompt refresh** | ✅ closed | Eng | Prompts/instructions describe mission, guardrails, cache policy. | `.github/copilot-instructions.md` and roadmap match charter text. |
| **B6 — Release tooling sanity** | ✅ closed | Release | Packaging/Docker scripts emit only `lifelinetty_*`; no legacy symlinks. | `scripts/local-release.sh` outputs lifelinetty artifacts only; systemd units named `lifelinetty.service`. |

> Keep the B1 sweep active before shipping anything in v0.2.0 to prevent `seriallcd` regressions.
>
> If any blocker re-opens, keep the table row short and link to the owning defect/PR from the “Action” column (details belong in the linked ticket or a dedicated subsection).

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
| **P2 — LCD driver regressions** | 🔵 Open | Display | Expand coverage for flicker-free writes, CGRAM/icon churn, backlight/blink toggles, and demo overlays; add host-mode/stub tests where hardware isn’t available; update `docs/lcd_patterns.md` with a 16×2 visual expectation table for the demo playlist and common overlays. | No new driver APIs; keep HD44780 + PCF8574 only. Acceptance: (1) icon churn tests prove ≤8 slots and deterministic eviction behavior, (2) backlight/blink toggles are covered by tests, (3) docs updated for 16×2 expected visuals. |
| **P3 — Serial backoff telemetry** | 🔵 Open | Serial | Verify structured error mapping (permission/unplug/framing) and reconnect counters; assert log placement and rotation limits for serial backoff logs under `/run/serial_lcd_cache/serial_backoff*`. | No new transports; respect 9600 baud floor and backoff timing. Acceptance: (1) tests assert log path is under cache, (2) reconnect counters increase deterministically in fake/unplug scenarios, (3) backoff timing stays within initial ≤500 ms and cap ≤10_000 ms with monotonic non-decreasing steps, (4) rotation limits enforced (e.g., ≤5 files, ≤256 KB each) and covered by tests. |
| **P4 — Polling/heartbeat stability** | 🔵 Open | Core | Harden polling/watchdog interactions and startup/shutdown ordering so the LCD render loop stays responsive during reconnects; add time-budgeted tests that exercise reconnect + polling overlap without flakes. | Keep RSS <5 MB; timers respect existing config fields. Acceptance: (1) watchdog/polling tests prove render loop keeps updating during reconnect/backoff using a measurable liveliness check (e.g., ≥3 renders within any 2s window during overlap), (2) timing budgets enforced (no tests that hang), (3) any new logs remain under cache and are asserted when practical. |
| **Bug backlog & field fixes** | 🔵 Open | Rotation | Triaged defects from real hardware runs get repro tests and fixes in the same PR; no feature creep. | Must ship with regression tests and doc updates as needed; log paths remain in cache; include a log bundle under `/run/serial_lcd_cache/bug-<id>/` and reference the repro test. |

> Execution note for AI/agents: keep changes minimal, land tests alongside fixes, and prefer touching only the files listed under each priority/workstream.

### Parity notes (stay aligned)

- **Dependency parity**: Reconcile `Cargo.toml` with the allowed crate list. If `lz4_flex`, `zstd`, or `embedded-hal` features are present, either gate them per charter or document justification in `docs/lifelinetty_creates.md` in the same PR.
- **CLI parity**: `--serialsh` and `--wizard` are **stable**; do not add new commands/modes in v0.2.0. Keep README/help tables, roadmap, and `tests/bin_smoke.rs` in lockstep with `src/cli.rs`.

  Note: the stable entrypoint is the `run` subcommand (or implicit `run` when you pass flags directly); avoid documenting flags/modes that are not present in `src/cli.rs`.

### Explicitly out of scope for v0.2.0

- No new CLI flags or protocol changes (tunnel/file transfer/negotiation stay as-is).
- No new dependencies beyond the charter-approved set; do not remove existing ones without approval.
- No new transports (network, BLE, PTY) or storage locations outside cache/config.

## Priority ↔ milestone mapping (execution aid)

This avoids “we shipped milestones but didn’t close priorities” drift.

| Priority | What ships (concrete) | Where it lands | Evidence required to mark done |
| --- | --- | --- | --- |
| P2 (LCD driver regressions) | CGRAM churn/eviction tests, backlight/blink coverage, deterministic overlay rendering, updated 16×2 expectations | `src/display/**`, `src/lcd_driver/**`, `tests/**`, `docs/lcd_patterns.md` | `cargo test` + updated docs + at least one LCD-focused matrix bundle |
| P2a (alpha: remove LCD/icon fallbacks) | No implicit ASCII/icon fallbacks; LCD init failure is explicit; docs/samples reflect missing-glyph behavior | `src/display/**`, `src/payload/icons.rs`, `samples/**`, `README.md` | Unit + integration tests prove deterministic missing-glyph behavior |
| P3 (serial backoff telemetry) | Error mapping + counters deterministic; cache log placement + rotation asserted | `src/serial/**`, `tests/**` | Tests that simulate unplug/permission denied and assert cache log paths |
| P4 (polling/heartbeat stability) | Render loop stays responsive during reconnect/backoff overlap; time-budgeted tests | `src/app/**`, `tests/**` | Tests enforce budgets; no flakes in repeated local runs |
| Milestone 2–3 (wizard improvements) | Better first-run UX; optional link-speed rehearsal via prompts (no new flags) | `src/app/wizard.rs`, docs, tests | Scripted wizard tests + transcript/log path assertions |
| Milestone 4 (serialsh under systemd) | Operator docs + checklists; no behavior change required | docs only | Docs updated + smoke still covers `--serialsh` help/usage |

## Risk register (v0.2.0)

| Risk | Impact | Early signal | Mitigation / owner |
| --- | --- | --- | --- |
| Timing-sensitive tests flake (polling/watchdog/backoff) | CI noise blocks merges | intermittent failures, long runtimes | Time-budgeted tests with deterministic clocks/fakes where possible (Core) |
| ARMv6-only regressions | Field trials fail late | x86_64 OK; Pi 1 fails | Require at least one Pi 1 run per runtime PR (Rotation) |
| LCD init variability / bus contention | False “dead device” reports | init fails sporadically | Make init errors explicit + docs for wiring/power triage (Display) |
| UART adapter variability (FTDI/CH340/CP210x) | Missed framing/latency issues | works on one dongle only | Add at least one “alternate adapter” scenario; capture adapter VID/PID in logs (Field ops) |
| Cache growth / unbounded logs | RAM pressure, RSS drift | cache directories balloon | Enforce rotation/limits and assert in tests where practical (Serial/Core) |

## Defect intake & triage loop (field → test → fix)

Field trials only create value if they convert to regressions.

1. **Capture**: create a dated bundle under `/run/serial_lcd_cache/<scenario>-YYYYMMDD/` containing:
   - exact command line(s) run
   - config used (`--config-file` path or default note)
   - payload set(s) used
   - relevant logs (`serial_backoff/`, `watchdog/`, `polling/`, `tunnel/`, `wizard/`)
2. **File**: open a defect with (a) bundle path, (b) hardware notes (Pi model, adapter chipset, LCD backpack address), (c) expected vs observed.
3. **Reproduce**: add a failing unit/integration test (prefer `tests/integration_mock.rs` or the closest existing harness).
4. **Fix**: land the code change + passing regression test in the same PR.
5. **Close**: only close the defect once the regression test is merged and the scenario can be replayed.

## Workstreams

- **Stability sweeps**
  - Files: `src/config/loader.rs`, `src/config/mod.rs`, `src/serial/{mod,backoff,telemetry}.rs`, `src/app/{connection,watchdog,polling}.rs`, `tests/{bin_smoke,integration_mock,fake_serial_loop,latency_sim}.rs`.
  - Actions: tighten validation errors, add fixtures for malformed config, assert log placement/rotation in cache, ensure watchdog + polling timers never starve render loop.
  - Cache audit (P3/B3): assert serial backoff/telemetry/tunnel logs stay under `/run/serial_lcd_cache` with rotation limits, extend `tests/integration_mock.rs`/bin smoke to verify paths.
  - Evidence to capture (per change):
    - Unit/integration tests that reproduce the failure mode (unplug, permission denied, malformed payload, LCD init failure).
    - A representative log bundle under `/run/serial_lcd_cache/<family>/` that matches the test name/scenario.

- **LCD regression coverage**
  - Files: `src/display/{lcd,overlays,icon_bank}.rs`, `src/lcd_driver/{mod,pcf8574}.rs`, `tests/bin_smoke.rs`, `tests/integration_mock.rs`, `docs/lcd_patterns.md`.
  - Actions: add host-mode/stub tests for CGRAM swaps and backlight/blink paths; document demo patterns and expected visuals; ensure icon churn respects the 8-slot CGRAM limit with deterministic eviction and deterministic missing-icon handling (no silent ASCII substitution).
  - Evidence to capture (per change):
    - A 16×2 expected-output table or screenshot notes in `docs/lcd_patterns.md` (frame-by-frame where useful).
    - A deterministic host-mode test that proves behavior without real I²C hardware.

  ### P2a — Remove LCD icon & display-type fallbacks (alpha)

  Status: 🟢 Done (alpha removal) | Owner: Display

  Summary: Implicit ASCII glyph fallbacks and silent stub display fallbacks have been removed. Missing glyphs are now surfaced explicitly and Linux hardware init failures bubble up instead of silently swapping to a stub.

  What “done” means:
  - `Icon::ascii_fallback()` and implicit ASCII substitutions are gone from render paths.
  - `IconPalette` returns optional glyphs only; renderers leave blanks and record `missing_icons` instead of substituting characters.
  - On Linux, `Lcd::new()` returns an error on hardware init failure instead of falling back silently to a stub; host-mode tests use the explicit stub constructor.
  - Docs and tests updated to reflect deterministic behavior (including overlay coverage that asserts no ASCII substitution when glyphs are absent).

  Files to change / review:
  - `src/display/icon_bank.rs`
  - `src/display/overlays.rs`
  - `src/payload/icons.rs`
  - `src/display/lcd.rs`
  - Documentation: `README.md`, `docs/lcd_patterns.md`, `docs/demo_playbook.md`, `samples/*.json`

  Acceptance criteria:
  - Unit coverage asserts missing icons are recorded without ASCII substitution (e.g., `overlay_icons_does_not_substitute_when_missing`).
  - Tests and smoke checks updated for demo payloads; behavior is deterministic and obvious when glyphs are missing or when hardware fails to initialise.
  - Documentation explains the current alpha removal and how operators should handle missing glyphs / hardware errors.


- **Field trial readiness (real hardware + doc updates)**
  - Files: `docs/dev_test_real.md`, `docs/architecture.md`, `README.md`, `docs/demo_playbook.md`, `samples/` payloads.
  - Actions: script repeatable runs using existing `devtest/*.sh`; capture expected outputs; document failure triage steps; ensure cache paths are cleared between runs.
  - Evidence to capture (per run):
    - Scenario directory under `/run/serial_lcd_cache/<scenario>-YYYYMMDD/` with logs and the exact command lines used.
    - A short note in the PR describing whether the run was on x86_64 (Docker) or ARMv6 (Pi 1), plus any observed deviations.
    - RSS observation for at least one 2h soak when the PR touches buffering/render/polling (target <5 MB).

- **Dependency alignment**
  - Files: `Cargo.toml`, `docs/lifelinetty_creates.md`.
  - Actions: reconcile listed crates (e.g., `lz4_flex`, `zstd`, `embedded-hal`) with the charter-approved set; either justify/append to the allowed list or gate/prune unused features, updating `docs/lifelinetty_creates.md` in the same PR.

- **CLI flag parity & status check**
  - Files: `README.md`, `src/cli.rs`, `tests/bin_smoke.rs`.
  - Actions: reconcile `--serialsh`/`--wizard` status (stable vs milestone-gated), ensure `--help`/README flag tables match charter, and add/refresh smoke assertions for allowed flags.
  - Evidence to capture: smoke test names covering help output, `--config-file` precedence, env override precedence, and flag presence.


## Real-world test matrix

| Scenario | Device/adapter | Baud & framing | LCD geometry | Payload set | Expected checks |
| --- | --- | --- | --- | --- | --- |
| Baseline | Pi 1 Model A + `/dev/ttyUSB0` (FTDI) | 9600 8N1 | 16×2 | `samples/payload_examples.json` | Clean render, no flicker; cache logs only under `/run/serial_lcd_cache`; RSS <5 MB for 2h run. |
| Alt TTY | Pi 1 Model A + `/dev/ttyAMA0` | 9600 8N1 | 16×2 | same as above | Wizard and config honor device override; reconnect/backoff logs categorized. |
| Higher baud probe | Pi 1 Model A + `/dev/ttyUSB0` | 19200 8N1 (post-wizard) | 16×2 | demo payloads | No framing errors; heartbeat/watchdog stay green. |
| LCD stress | Pi 1 Model A + I²C backpack | 9600 8N1 | 16×2 | icon-heavy samples | CGRAM swaps ≤8 icons; no display corruption; overlays render. |
| Tunnel coexists | Pi 1 Model A + `/dev/ttyUSB0` | 9600 8N1 | 16×2 | command tunnel enabled via `--serialsh` | Busy/Exit codes correct; LCD continues rendering; cache logs under `tunnel/` only. |
| Permission denied | Pi 1 Model A + `/dev/ttyUSB0` (no group perms) | 9600 8N1 | 16×2 | baseline samples | Error is classified; backoff engages; no crash; cache logs include permission category. |
| Unplug/replug | Pi 1 Model A + `/dev/ttyUSB0` | 9600 8N1 | 16×2 | baseline samples | Clean reconnect; counters/logs deterministic; LCD keeps updating status/heartbeat. |
| Alt adapter | Pi 1 Model A + `/dev/ttyUSB0` (CH340/CP210x) | 9600 8N1 | 16×2 | baseline + demo payloads | No framing regressions; latency within expected; log bundle includes adapter notes. |
| Alt I²C addr (common) | Pi 1 Model A + backpack @ `0x3F` | 9600 8N1 | 16×2 | baseline samples | Either works when configured, or fails with explicit init error (no silent stub). |

Record outcomes and defects in RAM-disk logs; reproduce with tests before closing.

**Execution notes:**

- Logs/artifacts live under `/run/serial_lcd_cache/{serial_backoff,watchdog,tunnel,wizard,polling,negotiation}`; never persist elsewhere. New log families must be added to this list and asserted in tests. Rotation limits for serial_backoff logs apply (see P3).
- Owner: field-ops rotation. Exit when each scenario has a dated log bundle in cache plus a matching regression test (or a filed defect). Note which matrix scenario was run and where its artifacts live (e.g., `/run/serial_lcd_cache/<scenario>-YYYYMMDD`).

## Performance & soak validation (field runs)

The goal is to keep “<5 MB RSS” and “no busy loops” measurable during field trials.

1. Start a run as usual (prefer a matrix scenario name).
2. Create a bundle directory: `/run/serial_lcd_cache/perf/<scenario>-YYYYMMDD/`.
3. Capture RSS periodically (examples):
   - `pidof lifelinetty`
   - `ps -o pid,rss,etime,cmd -p "$(pidof lifelinetty)"`
   - `grep -E 'VmRSS|VmHWM|Threads' "/proc/$(pidof lifelinetty)/status"`
4. Pass criteria for a 2h baseline: RSS stays <5 MB; no monotonic growth trend; no high-frequency log spam suggesting a busy loop.

## Testing & validation (per change)

1. `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` on x86_64 (and ARMv6 when available).
2. Update/add unit + integration tests alongside fixes (config, LCD, serial, tunnel, polling).
3. Run at least one matrix scenario per PR touching runtime behavior; capture logs under `/run/serial_lcd_cache` for review.
4. Document user-facing changes in README and relevant docs (`docs/dev_test_real.md`, `docs/lcd_patterns.md`, `docs/architecture.md`).

## Out-of-scope reminders

- No network/PTY/HTTP features; UART + LCD only.
- Network for testing only via existing scripts; no runtime network code. e.g. SSH tunnels, client-server development setups.
- No writes outside `/run/serial_lcd_cache` and `~/.serial_lcd/config.toml`.
- No new crates beyond the approved list (see `docs/lifelinetty_creates.md`).

## Release readiness gate (v0.2.0)

This is the “can we cut a v0.2.0 release candidate?” checklist.

**Entry criteria (RC can be tagged when all true):**

- Blockers B1–B6 are still ✅ closed (re-run the B1 sweep).
- P1 is 🟢 Done; P2–P4 are 🟢 Done (or explicitly deferred with Eng + Release sign-off in the PR summary).
- `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` pass on x86_64; ARMv6 run is reported (ran or skipped with reason).
- Field trial minimum scenarios (listed in “At a glance”) have a dated cache bundle each.

**Exit criteria (RC becomes “ship”):**

- Any field-trial defects are either fixed with regression tests or explicitly carried as known-issues with mitigations documented.
- README and playbooks match `lifelinetty --help` and current runtime behavior.

## Rollout & tracking

- Keep this file as the active roadmap for v0.2.0; archive older versions under `docs/Roadmaps/v0.1.0` as needed.
- Field-trial issues must land with a regression test before closing.

- Change log: see `docs/Roadmaps/v0.2.0/changelog.md` for details of the fallbacks removal (2025-12-05).

### Milestone execution cues

- **Entry criteria**: P1–P4 not blocked; B1 sweep re-checked; cache/log paths enforced in smoke tests.
- **Exit criteria**: milestone doc section updated with date; matching tests/docs merged; cache-log evidence stored under `/run/serial_lcd_cache/<milestone>/` or existing log families.
- **Sign-off**: Eng owner + Release acknowledge in PR summary; note x86_64 + ARMv6 test runs (or why skipped).

---

### Milestone 1 — Docker-to-docker devtest loop (Completed 5 Dec 2025)

> Detailed playbook: see `docs/Roadmaps/v0.2.0/milestone_1.md` (Docker run + compose recipes, CI/headless flow, test matrix, AI rerun checklist).

- **Goal:** Make the Docker-to-docker auto loop the primary devtest path so CI/hardware-free runs can exercise the full three-terminal workflow with zero drift, confining all artifacts to `/run/serial_lcd_cache` and `~/.serial_lcd` inside each container.
- **Implementation:** Run paired containers: one acts as the “local” runner and one as the “remote” target with an SSH daemon. Mount `/run/serial_lcd_cache` (tmpfs or bind) into both. From the local container (or host), invoke `devtest/run-dev.sh` with `PI_HOST=<remote-container-name>`, `PI_USER` set to the remote SSH user (often `root`), and `PI_BIN` pointing to the remote binary path (e.g., `/opt/lifelinetty/lifelinetty`). Optionally set `LOCAL_BIN_SOURCE`/`REMOTE_BIN_SOURCE` to prebuilt artifacts. The script builds (per `dev.conf`), scps configs and binaries, kills stale processes, and launches the local and remote runs under separate terminals or shells.
- **Required steps:** Pre-flight checks (SSH reachability, config templates present, remote service off), local build, binary sync + chmod, remote cleanup, then window launches for (1) persistent SSH shell, (2) remote run, (3) local comparison run. Config paths remain flexible (`--config-file` honored) so you can vary TTY (`/dev/ttyUSB0`, `/dev/ttyS0`), baud presets, LCD geometries, demos, and logging tweaks without editing the scripts. For hardware trials, the same flow still applies against Pi hosts.
- **Logging & storage:** Keep watcher active on `/run/serial_lcd_cache`. Logs, cache snapshots, and terminal outputs stay under that RAM-disk—never `/etc` or other persistent paths. `docs/dev_test_real.md` remains the workflow reference and checklist; `devtest/dev.conf.example` keeps defaults charter-compliant (9600 on `/dev/ttyUSB0`, cache watcher, service reminder).
- **Docs/tests:** Capture every matrix scenario (baseline, alt-TTY, higher baud, etc.) under `/run/serial_lcd_cache/milestone1/<scenario>-YYYYMMDD`, copy them via `docker cp` or `scp` for field-ops review, and annotate the replay in `docs/dev_test_real.md` along with the payload set used. Any defects unearthed in this loop must land with regression tests (`tests/bin_smoke.rs`, `tests/integration_mock.rs`, etc.) before the milestone can be marked done.
- **Status:** Milestone 1 is documented end-to-end: quickstart recipes, container topology, config templates, log placement, CI recipe, and AI re-run checklist live in `docs/Roadmaps/v0.2.0/milestone_1.md`, proving the build job can be audited entirely within `/run/serial_lcd_cache` and `~/.serial_lcd/config.toml`.

### Milestone 2 — Enhanced first-run wizard (Completed 21 Dec 2025)

- **Goal:** Make the guided setup noticeably more helpful without adding new flags or transports: auto-guess the likely TTY and baud, suggest client/server roles, and offer optional SSH/scp/tmux snippets to move binaries/configs/logs between hosts—while keeping all persistence to `~/.serial_lcd/config.toml` and transcripts under `/run/serial_lcd_cache`.
- **Implementation:**
  - Extend `src/app/wizard.rs` prompts to (a) scan common serial paths (`/dev/ttyUSB*`, `/dev/ttyAMA*`, `/dev/ttyS*`) and propose a ranked default; (b) ask usage intent (server/client/standalone) and LCD presence; (c) surface opt-in helper text for copying the binary/config or log bundles via SSH/scp/tmux (text only, no network code executed); (d) optionally probe baud safely starting at 9600 with backoff; (e) persist choices to `~/.serial_lcd/config.toml` and record the transcript plus probe outcomes in `CACHE_DIR/wizard.log`.
  - Keep CLI stable (`--wizard` only); headless/scripted mode continues via `LIFELINETTY_WIZARD_SCRIPT` with deterministic defaults when stdin is not a TTY.
- **Logging & storage:** All wizard transcripts and suggested snippet text stay under `/run/serial_lcd_cache`; no writes outside `CACHE_DIR` and `~/.serial_lcd/config.toml`. No automated SSH/scp/tmux execution—only displayed snippets for the user to copy.
- **Docs/tests:** README updated with the expanded helper snippets (binary + config + logs), and regression coverage added in `tests/bin_smoke.rs` + `tests/integration_mock.rs` to validate scripted wizard runs (including ranked device lists and persisted answers) without requiring LCD hardware.

### Milestone 3 — Wizard link-speed rehearsal (Completed 21 Dec 2025)

- **Goal:** On the first server/client pairing, automatically run a guided link-speed rehearsal to pick the highest stable serial baud and framing the pair can sustain, then store that as the preferred config without adding new flags or transports.
- **Implementation:**
  - When both ends opt in via the wizard (recommended for server/client setups), iterate a bounded list of baud candidates (starting at 9600) with retries, settle on the highest reliable pick, and write the result to `~/.serial_lcd/config.toml`.
  - The rehearsal validates a control-plane handshake plus a CRC-protected heartbeat frame; it is driven through wizard prompts and uses the current serial stack—no sockets, no new flags.
- **Logging & storage:** Record each probe attempt, errors, and the final chosen baud under `/run/serial_lcd_cache/wizard/` (e.g., `link_rehearsal.log`), alongside the normal wizard transcript. Persist only the chosen settings to `~/.serial_lcd/config.toml`; no other writes outside `CACHE_DIR`.
- **Docs/tests:** Expand README and wizard docs to explain the first-pairing speed test and how to re-run it. Add tests in `tests/bin_smoke.rs` and `tests/integration_mock.rs` for the rehearsal flow (scripted answers, capped candidate list, transcript/log path) and ensure fallback to 9600 on failure. Keep RSS/backoff guardrails and existing CLI behavior unchanged.

### Milestone 4 — Serial shell under systemd (Completed 21 Dec 2025)

- **Goal:** Document and harden how operators run `lifelinetty --serialsh` on boxes where `lifelinetty.service` is already managing the daemon, without adding new flags or transports. The shell must remain an interactive, manual invocation that never fights systemd for the same TTY or introduces new storage locations.
- **Implementation:**
  - Spell out the expected operator flow: (a) if the service has the target TTY open, stop/disable it temporarily (`systemctl stop lifelinetty.service`) or point `--device` at an idle TTY; (b) open an SSH/tmux session and launch `lifelinetty --serialsh [--device ... --baud ...]`; (c) on exit, restart the service if desired. No new CLI switches beyond the existing `--serialsh`, `--device`, and `--baud` overrides.
  - Keep cache/config handling identical to the daemon path: all transient shell artifacts live under `/run/serial_lcd_cache/serialsh*`, and persistent settings remain in `~/.serial_lcd/config.toml`.
- **Logging & storage:** Reinforce that all shell logs/errors go to stderr (journald when invoked under systemd units) or cache files under `/run/serial_lcd_cache/serialsh*`; do not write anywhere else. Suggest `journalctl -u lifelinetty.service` for prior daemon logs but avoid altering the unit.
- **Docs/tests:** Update README and wizard helper snippets to clarify the systemd flow (stop service, run shell via SSH/tmux, restart). Add a brief checklist in this roadmap and ensure `tests/bin_smoke.rs` keeps covering `--serialsh` help/usage so behavior stays stable. No new tests are required for systemd itself; this milestone is documentation and ops guidance only.

### Milestone 5 — Flag improvements & custom config override (**completed 4 Dec 2025**)

- **Goal:** Introduce the deliberate exception to the stable CLI surface by adding `--config-file <path>` so operators can point LifelineTTY at a dedicated TOML that overrides `~/.serial_lcd/config.toml` and becomes the highest-priority configuration source, while still honoring environment overrides and per-flag tweaks layered on top.
- **Status:** Delivered. The CLI accepts `--config-file` across implicit/`run` invocations, bypasses the default config path when supplied, and still applies environment overrides and per-flag tweaks on top. Help/README/devtest docs reference the flag, and regression coverage in `tests/bin_smoke.rs` and `src/app/mod.rs` confirms precedence and default config non-creation.
- **Logging & storage:** The flag only reads the provided file; persistent writes remain limited to `~/.serial_lcd/config.toml` and `/run/serial_lcd_cache`.
- **Docs/tests:** `tests/bin_smoke.rs` exercises env override + default skip; new unit tests in `src/app/mod.rs` assert config-file precedence and CLI override behavior. `docs/dev_test_real.md` and `devtest/dev.conf.example` describe using the flag for scenarios.
