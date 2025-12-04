# LifelineTTY Roadmap (Dec 2025)

Updated 4 Dec 2025

LifelineTTY is the successor to SerialLCD: a single Rust daemon for Raspberry Pi 1 (ARMv6) that ingests newline-delimited JSON via UART and keeps a HD44780 LCD in sync. All work listed here must respect the charter in `.github/copilot-instructions.md` — single binary, no network sockets, RAM-disk cache only, CLI flags remain stable (`--run`, `--test-lcd`, `--test-serial`, `--device`, `--baud`, `--cols`, `--rows`).

## Context & guardrails (Option A inline reminders)

- **Storage**: only `~/.serial_lcd/config.toml` may be written persistently; all temp/log files live under `/run/serial_lcd_cache`.
- **Interfaces**: UART input defaults to `/dev/ttyUSB0` (9600 8N1) but must accept any TTY path provided via config/CLI overrides; LCD via PCF8574 @ 0x27; CLI only, no new protocols without explicit approval.
- **Quality bar**: `cargo test`, `cargo clippy`, `cargo fmt` clean on x86_64 + armv6; every public API has Rustdoc; README documents every flag/config option.
- **Testing**: add/refresh tests in `tests/` and `src/**` modules for each behavioral change; hidden CI assumes watchdog coverage, so no `#[allow(dead_code)]` escapes.

## Blockers (must fix before new scope)

1. **B1 — Finish rename to LifelineTTY everywhere (✅ 2 Dec 2025)**: README, `docs/architecture.md`, `docs/releasing.md`, `lifelinetty.service`, packaging scripts, Dockerfiles, Makefile, and tests were updated to replace `seriallcd` with `lifelinetty`. SerialLCD was an alpha preview only; backward compatibility is not required. All tooling is now LifelineTTY-branded.
2. **B2 — Charter + instructions alignment (✅ 2 Dec 2025)**: `.github/copilot-instructions.md`, README, and every prompt under `.github/instructions/` now restate the `/dev/ttyUSB0 @ 9600` defaults and call out acceptable overrides (`/dev/ttyAMA0`, `/dev/ttyS*`, USB adapters).
3. **B3 — Config + cache policy audit (✅ 2 Dec 2025)**: the daemon rejects log paths outside `/run/serial_lcd_cache`, and `tests/bin_smoke.rs` enforces the cache-only rule so nothing writes outside the RAM disk (other than `~/.serial_lcd/config.toml`).
4. **B4 — CLI docs/tests parity (✅ 2 Dec 2025)**: README gained a full CLI table plus storage notes, and `tests/bin_smoke.rs` now runs non-ignored smoke tests covering `--version`, `--help`, `--device`, `--cols`, `--rows`, and `--demo` documentation.
5. **B5 — AI prompt + automation refresh (✅ 2 Dec 2025)**: `.github/instructions/*.md` and devtool prompts now explicitly describe LifelineTTY’s mission, storage guardrails, and serial defaults before downstream contributors start feature work.
6. **B6 — Release tooling sanity pass (✅ 2 Dec 2025)**: `scripts/local-release.sh`, Dockerfiles, packaging metadata, and docs ship only `lifelinetty_*` binaries/service units; legacy `seriallcd` symlinks and units have been retired.

Note: A set of minimal roadmap skeleton modules has been added to the repository to help iterate on Milestones quickly. See `docs/roadmap_skeletons.md` for the layout and next steps.

There is also a short frameworks document that describes the set of skeleton modules and next steps: `docs/roadmap_frameworks.md`.

> _Only after B1–B6 are closed should we land anything from the P1–P20 queue or milestone features below._

## Latest progress (4 Dec 2025)

- Milestone A’s command tunnel remains stable, and the render loop still routes `channel:"command"` frames through `CommandBridge`, `CommandExecutor`, and `TunnelController`, streaming stdout/stderr/exit chunks while logging decode failures under `/run/serial_lcd_cache/tunnel/`.
- Milestone B’s auto-negotiation is now live: `src/app/connection.rs` manages the INIT/hello/hello_ack exchange using `Negotiator`, records role decisions in `NegotiationLog`, and falls back to LCD-only mode when a peer does not understand the protocol. `src/app/negotiation.rs` captures the deterministic preference/role logic plus capability bits.
- `src/config/loader.rs` now persists the `[negotiation]` block and a top-level `command_allowlist`, so `~/.serial_lcd/config.toml` round-trips cleanly and CLI-driven reconnects reuse the negotiated defaults.
- Config and tunnel tests were refreshed to reflect the new handshake plumbing and still pass across the suite.

## Priority queue (P1–P20)

| ID  | Title & scope (inline guardrails) |
|-----|----------------------------------|
| **P1 (✅ 2 Dec 2025)** | **Repo-wide rename + lint**: resolve B1 in code/tests/docs; run `cargo fmt`, `cargo clippy` to prove no stale identifiers remain. |
| **P2 (✅ 2 Dec 2025)** | **Baud / CLI defaults audit**: codify `/dev/ttyUSB0 @ 9600` as the baseline, ensure config/CLI overrides cover `/dev/ttyAMA0`/`ttyS*` devices, and add integration tests in `tests/bin_smoke.rs` for precedence + persistence. |
| **P3 (✅ 2 Dec 2025)** | **Config loader hardening**: enhance `src/config/loader.rs` to validate cols/rows ranges, default scroll/page timings, and ensure `~/.serial_lcd/config.toml` schema doc matches real struct. |
| **P4 (✅ 2 Dec 2025)** | **LCD driver regression tests**: expand `src/lcd_driver` tests for flicker-free updates, blinking, and icon resets; ensure no `unsafe`. |
| **P5 (✅ 2 Dec 2025)** | **Serial backoff telemetry**: add structured logging to `src/serial/*` capturing reconnect counts into `/run/serial_lcd_cache/serial_backoff.log`, respecting RAM-only constraint. |
| **P7 (✅ 3 Dec 2025)** | **CLI integration mode groundwork**: `--serialsh` now ships with the default binary, plumbing the CLI command loop into the tunnel and bolstering the smoke suite around the interactive flow. |
| **P8 (✅ 4 Dec 2025)** | **Bi-directional command tunnel core**: base framing library in `src/payload/parser.rs` for command request/response envelopes (no network). Must reuse newline JSON framing. _(Status: CommandBridge, CommandExecutor, and TunnelController now translate request frames into stdout/stderr/exit responses with Busy/error handling and logging under `/run/serial_lcd_cache/tunnel/`.)_ |
| **P9 (✅ 4 Dec 2025)** | **Server/client auto-negotiation**: implement handshake state in `src/app/connection.rs`, ensuring deterministic fallback to current behaviour when remote does not understand negotiation packets. _(Status: INIT handshakes emit hello/hello_ack frames with capability bits, handshake results are recorded via `NegotiationLog`, and legacy peers trigger LCD-only fallbacks.)_ |
| **P11 (✅ 5 Dec 2025)** | **Live hardware polling agent**: `start_polling()` now spins in the daemon whenever `polling_enabled` is true, snapshots feed `run_render_loop` so CPU/mem/disk/temperature stats appear on the LCD during reconnects/boot, and `/run/serial_lcd_cache/polling/events.log` records every snapshot/error. CLI/config regression tests cover the `--polling`/`--poll-interval-ms` overrides. |
| **P13 (✅ 3 Dec 2025)** | **JSON-protocol strict mode**: `RenderFrame::from_payload_json` now requires `schema_version`, applies strict length caps, and rejects oversized icon lists, so malformed frames fail fast. |
| **P14** | **Payload compression support** _(in progress)_: LZ4/Zstd helpers live in `src/compression.rs`, yet payload ingestion still assumes plain JSON and `compression::CompressionCodec` is only referenced by the CLI parser. Implement the compressed envelope, negotiation bit, and parser/encoder wiring to actually ship compressed frames. |
| **P15** | **Heartbeat + watchdog** _(in progress)_: `src/app/watchdog.rs` remains a stub and no module instantiates it; the blink indicator inside `render_loop` is UI-only. Add real heartbeat frames, watchdog timers, and fail-safe hooks to trigger offline screens or scripts. |
| **P19** | **Documentation + sample payload refresh**: README, `docs/lcd_patterns.md`, and `samples/payload_examples*.json` still predate tunnel/polling guidance—update them with negotiated roles, command tunnel payloads, and any new icon rules once features solidify. |
| **P20 (✅ 4 Dec 2025)** | **Serial transport resilience**: finalize explicit 8N1 + flow-control defaults in code, expose DTR/RTS toggles + timeout knobs via config for upcoming tunnels, and add structured error mapping/logs so reconnect logic can distinguish permission, unplug, and framing failures before Milestones A–C. _(Status: CLI + config cover flow-control/parity/stop-bits/DTR/timeouts, and telemetry now records categorized permission/unplug/framing causes for each reconnect.)_ |
| **P21 (✅ 3 Dec 2025)** | **Adopt hd44780-driver crate for Linux builds where possible**: migrate the internal HD44780 driver to use the external `hd44780-driver` crate (via a small adapter for the platform I²C bus) while preserving our public API for any missing functionality. |
| **P22 (✅ 4 Dec 2025)** | **Custom character support and built-in icons**: the IconBank now hot-swaps CGRAM glyphs per frame, `display::overlay_icons` can render every curated icon, and overflow cases gracefully fall back to ASCII with debug logs. Docs and samples cover the new behavior. |
| **P23 (✅ 4 Dec 2025)** | **Guided first-run setup wizard**: Completed. `src/app/wizard.rs` now auto-runs whenever `~/.serial_lcd/config.toml` is missing, mirrors prompts on the LCD, validates `/dev/tty*` candidates + baud picks, persists the answers, and logs transcripts to `/run/serial_lcd_cache/wizard/summary.log`. Operators can re-run it via `lifelinetty --wizard` or `LIFELINETTY_FORCE_WIZARD=1`, and CI feeds scripted answers through `LIFELINETTY_WIZARD_SCRIPT`. |

To keep the serial link predictable, the daemon now enforces a 9600-baud floor and always starts there automatically. The new first-run wizard (Milestone I) runs automatically on fresh installs to help operators identify higher stable baud rates before upping the speed, and it records every session under `/run/serial_lcd_cache/wizard/` for auditing.

### Priority implementation plan

Only the still-open priorities are listed below; completed IDs remain documented in the table above.

| Item | Current focus & next steps |
| --- | --- |
| **P14 — Payload compression support** | The parser still assumes plain JSON and no runtime path calls `compression::compress`/`decompress` (only the CLI parser mentions `CompressionCodec`). Define the compressed envelope, plumb negotiation bits, add fixture tests plus watchdog limits for oversized frames, and document the CLI/config knobs so operators know when compression is active. |
| **P15 — Heartbeat + watchdog** | `src/app/watchdog.rs` is a stub and nothing instantiates it; the heartbeat indicator in `render_loop` is cosmetic. Implement mutual heartbeat frames across the LCD + tunnel lanes, add watchdog timers, trigger offline scripts/screens on expiry, and log watchdog transitions under `/run/serial_lcd_cache/watchdog/` with unit + integration coverage. |
| **P19 — Documentation + sample payload refresh** | README and `docs/lcd_patterns.md` still lack negotiated-role, polling, and compression guidance, and `samples/payload_examples*.json` only covers the basic dashboard payloads. Add examples for tunnel commands, strict-schema payloads, icons, and upcoming polling frames, and cross-link storage guardrails (RAM disk vs `~/.serial_lcd`) wherever new docs mention cache paths. |
| **P22 — Custom character support & icon banking** | `overlay_icons` only renders `battery`, `heart`, and `wifi`; other icons fall back to ASCII and there is no CGRAM bank manager. Build runtime glyph scheduling tied to `payload::icons`, persist icon sets per page, add tests covering slot eviction, and update `docs/icon_library.md` plus the README once the runtime is deterministic. |

By keeping this plan visible in the roadmap, every contributor can see the remaining dependency graph: P14 unlocks Milestone F, P15 underpins watchdog safety for tunnels, P19 guards docs UX, and P22 enables Milestone H.

## Crate guidance for roadmap alignment

The crates listed here come from `docs/creates_to_check.md`. Each is rated for the part of the roadmap where it will deliver the most value so we avoid scope creep and keep the dependency list grounded in current plans.

| Crate | Roadmap anchor | Why it fits |
| ----- | --------------- | ------------ |
| `calloop` | P11 / Milestone D | Lightweight event loop that can orchestrate `/proc` watchers, heartbeat timers, and serial input without a heavyweight async runtime or extra RAM pressure. |
| `async-io` | P8 / Milestone A | Non-blocking I/O helpers for streaming child stdout/stderr and UART frames; useful when multiplexing tunnel traffic without importing `tokio`. |
| `syslog-rs` | B3 / P5 | Sends warnings and telemetry to syslog while keeping RAM-only cache writes law-abiding, supporting the serial backoff telemetry goal. |
| `serde_json` | P8 / Milestones A, B, C | Already central to payload framing, will continue to anchor command/response envelopes, handshake payloads, and chunk metadata. |
| `os_info` | P11 / Milestone D | Provides host OS/arch metadata for polling and telemetry packets to aid debugging on the ARMv6 targets. |
| `crossbeam` | P11 / Milestone D | Scoped threads and channels that prevent the polling/render/serial lanes from blocking one another as telemetry work grows. |
| `rustix` | P8 / Milestone A | Safe wrappers for low-level ioctls/termios so serial port tweaks or PTY-style helpers stay within the charter. |
| `systemstat` | P11 / Milestone D | Lightweight CPU/memory/disk/temperature snapshots for the polling skeleton so metrics stay under the RSS and cache budgets. |
| `sysinfo` | P11 / Milestone D | CPU/memory/disk snapshots for the polling subsystem while respecting the `<5 MB RSS` constraint. |
| `futures` | P8 / Milestone A | Combinators and stream helpers when we need async-aware state machines for command tunnels or serial shell output streams. |
| `directories` | B3 / P4 | Canonicalizes `~/.serial_lcd` and cache paths so config/history helpers never wander outside the allowed directories. |
| `humantime` | P15 / Milestone D | Human-friendly duration parsing for heartbeat/polling intervals and log messages, aiding CLI/telemetry clarity. |
| `serde_bytes` | P10 / Milestone C | Binary serialization for chunk payloads to avoid base64 bloat when transferring files. |
| `bincode` | P10 / Milestone C | Compact manifests or CLI history caches stored under `CACHE_DIR`, keeping resume data tiny and deterministic. |
| `clap_complete` | P16 / Milestone G | Shell completion generation once `serialsh` options stabilize, aligning docs + CLI UX. |
| `indicatif` | P10 / Milestone C | CLI progress bars/spinners for `--push/--pull` helpers; keep outputs plain so they stay automation friendly. |
| `tokio-util` | P8 / P10 / Milestones A/C | Framed readers/writers and codecs that can simplify tunnel/frame helpers if we adopt `tokio` for higher-level protocols. |

Update this section or `docs/createstocheck.md` whenever priorities shift so the dependency rationale stays tied to the latest roadmap state.

## Milestones (big features & dependencies)

### Milestone A — Bi-Directional Command Tunnel (completed 4 Dec 2025)

- **Goal**: “one-line commands in, stdout/stderr out” over UART — effectively a remote bash shim.
- **Scope**: `src/app/connection.rs`, `src/serial/*`, CLI flag gating (`--serialsh`). Must preserve newline JSON framing by encapsulating command text + stdout chunks in structured payloads.
- **Dependencies**: P7, P8, P16. Requires heartbeat (Milestone D) for session health.
- **Constraints**: no networking, no PTYs; commands must run under same service user with resource caps to keep <5 MB RSS (spawn child processes carefully).
- **Workflow**:
  1. Define command/request schema in `src/payload/parser.rs` with serde enums and checksums.
  2. Extend serial loop (`src/app/render_loop.rs`) to multiplex command traffic alongside LCD updates.
  3. Implement command executor in `src/app/events.rs` that spawns child processes using `std::process::Command` with capped IO buffers in `/run/serial_lcd_cache`.
  4. Add CLI toggles/tests in `tests/bin_smoke.rs` verifying round-trip execution.
- **Crates & tooling**: standard library, `serialport`, `tokio-serial` (if async shim needed), `thiserror` for tunnel-specific errors, `log`/`tracing` for structured stdout/stderr streaming.

### Milestone A plan & status (completed 4 Dec 2025)

Milestone A is now **complete**: the `CommandBridge`/`CommandEvent` pipeline routinely decodes `channel:"command"` frames, the `CommandExecutor` manages session state for whitelisted commands, and the `TunnelController` turns the streamed `Stdout`/`Stderr`/`Exit` chunks into `TunnelMsgOwned` responses while logging decode failures under `/run/serial_lcd_cache/tunnel/errors.log`. The `--serialsh` flag wires straight into this tunnel so that interactive sessions respect Busy responses and report remote exit codes without disturbing the LCD render loop.

The previously cited plan items are now satisfied: the executor handles Busy/Exit transitions, the runnable shell bears out command round-trips (see the new smoke coverage in `tests/bin_smoke.rs`), and CRC tampering is rejected early (now guarded by `tests/integration_mock.rs`). With Milestone A shipped, the focus moves on to Milestone B (auto-negotiation) and the P9+ priorities that build on a stable command tunnel.

### Milestone B — Server/Client Auto-Negotiation (completed 4 Dec 2025)

- **Goal**: Both endpoints boot without config edits; handshake decides who acts as command server vs client. The completed flow now records the negotiated `NegotiationConfig` in `~/.serial_lcd/config.toml`, writes `NegotiationLog` entries to `/run/serial_lcd_cache`, and replays the preferred role/timer settings during reconnects.
- **Scope**: state machine additions in `src/app/lifecycle.rs` + `src/app/connection.rs`, handshake payload definitions in `src/payload/parser.rs`.
- **Dependencies**: P9, P19. Needs fallback path to classic “LCD-only” mode when remote lacks support.
- **Constraints**: handshake occurs after serial open but before LCD writes; any timeout must revert to default LCD display to avoid blank screens.
- **Workflow**:
  1. Introduce negotiation states in `src/app/lifecycle.rs` with deterministic timers.
  2. Encode capabilities bitmap inside JSON payloads; extend parser tests for unknown bits.
  3. Update `src/app/render_loop.rs` to pause rendering until role is resolved, with watchdog fallback.
  4. Document expected behavior and add integration tests with fake serial endpoints (`tests/fake_serial_loop.rs`).
- **Crates & tooling**: `serialport`, `tokio` (for async timeout), `anyhow`/`thiserror` for richer negotiation errors, `log` for trace-level negotiation logging.

### Milestone D — Live Hardware Polling

- **Goal**: gather CPU/mem/temp/disk/network metrics
- **Scope**: gather metrics from hosts system of cpu/mem/temp/disk.
- **Dependencies**: P11, P15, P18.
- **Constraints**: polling intervals must be configurable
- **Workflow**:
  1. Build polling module with `systemstat` to capture CPU/memory/disk/temperature snapshots without inflating the 5 MB RSS cap.
  2. Publish metrics into render queue via channels guarded by `std::sync::mpsc` or `crossbeam` (if later approved) to avoid blocking serial ingestion.
  3. Implement heartbeat packets (serde structs) and integrate into render loop timers; fallback to offline screen if missed.
- **Crates & tooling**: `systemstat` for lightweight CPU/memory/disk snapshots, `os_info` for system information, `sysinfo` for live hardware info, `log` for watchdog alerts, optional `tokio` timers if async polling chosen.

**Status (Dec 2025):** The P11 poller thread now runs whenever `polling_enabled` is set, surfaces CPU/mem/disk/temp snapshots on the LCD during reconnects/boot, and writes newline-delimited summaries to `/run/serial_lcd_cache/polling/events.log`. Remaining Milestone D scope hinges on P15’s watchdog/heartbeat plumbing.

### Milestone F — JSON-Protocol Mode + Payload Compression

**Status (Dec 2025):** Schema validation shipped (`payload::RenderFrame::from_payload_json` now requires `schema_version` and enforces bounds), but compression is still pending because no runtime path calls `compression::compress`/`decompress` and the parser only accepts plain JSON frames. The workflow below remains open until compressed envelopes negotiate successfully and reach the render loop.

- **Goal**: strict JSON schema with optional LZ4/zstd compression for log bursts.
- **Scope**: schema definitions in `src/payload/parser.rs`, compression modules (evaluate crate whitelist), CLI flag to enable compressed mode.
- **Dependencies**: P13, P14, P10.
- **Constraints**: ensure decompression buffers stay <1 MB; reject malformed packets gracefully and log to RAM disk.
- **Workflow**:
  1. Define schema versions and validation helpers (Serde + manual bounds checks) in parser module.
  2. Introduce compression envelope (e.g., `{"compressed":true,"codec":"lz4","data":"..."}`) and decode before payload parsing.
  3. Add negotiation bits (Milestone B) to ensure both ends agree on codec.
  4. Extend tests with compressed fixture payloads and fuzz-style boundary cases.
- **Crates & tooling**: `serde`/`serde_json` (existing), candidate pure-Rust codecs such as `lz4_flex` or `zstd-safe` (would require charter update approval), `anyhow` for codec errors.

### Milestone G — CLI Integration Mode (serialsh) (completed 4 Dec 2025)

- **Status**: **complete (4 Dec 2025)** — the optional `--serialsh` flag now ships with `lifelinetty` and can be used whenever you need an interactive terminal over the command tunnel.
- **Summary**: `lifelinetty --serialsh` drops into the `serialsh>` prompt, sends `CmdRequest` frames through the Milestone A tunnel, and prints the streaming stdout/stderr/exit chunks that arrive from the remote host. Busy responses stay evident, the CLI enforces that `--demo`/`--payload-file` cannot be combined with the shell, and systemd units continue to use the headless `run` mode unless manually directed otherwise.
- **Docs & tests**: README now documents how to invoke the shell, and `tests/bin_smoke.rs` guarantees the help output, prompt, and remote responses remain reliable.

### Milestone H — Custom Character Toolkit & Icon Library

- **Goal**: Provide a curated icon registry plus runtime CGRAM bank manager so payload authors can reference semantic icon names instead of hex bitmaps.
- **Scope**: `src/payload/icons.rs`, `src/display/lcd.rs`, `src/app/render_loop.rs`, `src/config/loader.rs`, `docs/icon_library.md`, `samples/payload_examples.json`, and `tests/*`.
- **Dependencies**: P22 (custom characters) with supporting helpers from P21 (hd44780-driver CGRAM plumbing).
- **Constraints**: Respect 8-slot CGRAM limit, <5 MB RSS, keep icon assets embedded in the binary or config; logging/cache writes stay inside `/run/serial_lcd_cache`.
- **Current status:** `icons` payloads can request one or more of `battery`, `heart`, `arrow`, and `wifi`. Unknown names are ignored so legacy dashboards never crash the render loop.
- **Workflow**:
  1. Import the public-domain glyphs from `duinoWitchery/hd44780` into a Rust icon registry plus a Markdown catalog with attribution.
  2. Build a CGRAM bank manager that stages icons ahead of each render pass, reuses existing slots, and falls back predictably when >8 glyphs requested.
  3. Extend payload/config schema so frames can request icons by name (or inline bitmaps) while validation enforces slot limits.
  4. Add tests + demos covering icon churn, slot eviction, and failure cases; refresh docs/samples so operators can opt in confidently.
- **Crates & tooling**: no new crates; reuse `hd44780-driver`, `linux-embedded-hal`, `rppal`, `serde`, existing logging utilities. Detailed plan lives in `docs/milestone_h.md`.

### Milestone I — Guided First-Run Wizard (completed 4 Dec 2025)

- **Goal (shipped)**: deliver an interactive first-boot experience that interviews operators about desired role (server/client), preferred TTY device, LCD geometry, and permissible baud rates, then writes the validated answers into `~/.serial_lcd/config.toml` while keeping the LCD online with helpful prompts.
- **Implementation**: `src/app/wizard.rs` owns the onboarding state machine, `App::from_options` and the serial shell invoke it before `Config::load_or_default()`, and the wizard mirrors each prompt on the LCD while probing safe `/dev/tty*` candidates at 9600 baud before accepting a higher target rate.
- **Controls & automation**: `lifelinetty --wizard`, `LIFELINETTY_FORCE_WIZARD=1`, and `LIFELINETTY_WIZARD_SCRIPT=/path/to/answers.txt` cover reruns and CI scripting; headless boots auto-accept safe defaults if stdin is not a TTY so systemd launches never hang.
- **Logging & storage**: every run appends a transcript (choices + baud probe results) to `/run/serial_lcd_cache/wizard/summary.log`, respecting the RAM-disk policy while keeping all other writes confined to `~/.serial_lcd/config.toml`.
- **Docs/tests**: README + CLI help now describe the wizard flag/env knobs, the roadmap frameworks list includes the module, and unit tests cover the scripted wizard path so CI can validate the flow without hardware input.

---

### Tracking & next steps

- Close B1–B6, then tackle P1–P4 in order to stabilize the base.
- With Milestones A and B shipped, begin scheduling Milestone C/D work alongside the remaining P10–P15 priorities (heartbeats, compression, chunked transfers).
- Maintain this roadmap alongside `docs/architecture.md` and update when priorities shift (always annotate date + reason).

## Implementation details (P21 — hd44780-driver migration)

This section collects the small, concrete edits and tests for the hd44780-driver adoption; it also serves as a checklist for the developer and reviewer teams.

_Status (3 Dec 2025): The shared adapter now supports both rppal and linux-embedded-hal buses, `display.driver` is user-configurable, and `Lcd::new_with_bus` exists for hardware smoke harnesses._

1. RppalI2cAdapter / I2cdevAdapter

    - File: `src/lcd_driver/pcf8574.rs`
    - Behavior: implement a small adapter to convert `rppal::i2c::I2c` (or `linux-embedded-hal::I2cdev`) into an `embedded-hal::blocking::i2c::Write` implementation. The adapter must be backlight-aware and preserve the PCF8574 write semantics (set `E`, `RS`/`DATA`, backlight bit).

1. Add Linux-only `Hd44780::new_external`

    - File: `src/lcd_driver/mod.rs`
    - Behavior: add `Hd44780::new_external(i2c_adapter, addr, cols, rows)` which constructs `hd44780_driver::HD44780` using `new_i2c` and return a compatibility wrapper with `load_custom_bitmaps`, `write_line`, `clear`, `backlight_on/off`, `blink_cursor_on/off` that delegates to either the external crate or the internal implementation.

1. Add `Lcd::new_with_bus(...)`

    - File: `src/display/lcd.rs`
    - Behavior: add a deterministic constructor that accepts a pre-initialized bus + address and uses the external driver on Linux when selected.

1. Add `display.driver` config option and CLI enablement

    - File: `src/config/loader.rs`, `README.md`.
    - Values: `auto` (default), `hd44780-driver`, `in-tree`.

1. Preserve CGRAM helpers

    - File: `src/display/lcd.rs`, `src/lcd_driver/mod.rs`.
    - Behavior: Implement CGRAM/custom char helpers for the external driver by writing the CGRAM write command and the following pattern bytes via the adapter bus.

1. Tests to add

    - unit tests for adapter & mock bus behaviors in `src/lcd_driver/mod.rs` and `src/lcd_driver/pcf8574.rs`.
    - CGRAM parity tests for both in-tree and external drivers.
    - Linux-only integration hardware tests for `--test-lcd` (init, write_line, clear, backlight, blink, custom_char, `load_custom_bitmaps`).
    - `tests/bin_smoke.rs` `display.driver` toggles and verification.

1. Review & acceptance

    - CI passes (tests + clippy + fmt) for both driver paths.
    - Hardware smoke runs: verify `hd44780-driver` parity for glyphs and backlight.
    - Multi-panel hardware test verifying graceful degradation.

1. Rollout Plan

    - Start opt-in via `display.driver` config + `--test-lcd`.
    - Run extended smoke tests for two weeks on hardware before making `auto` default choose the external driver on Linux.

1. Owner & timeline

    - Owner: hardware/driver engineer (TBD).
    - Estimate: 1–2 weeks split across development, testing, and smoke runs.
