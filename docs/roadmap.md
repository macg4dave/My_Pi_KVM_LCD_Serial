# LifelineTTY Roadmap (Dec 2025)

Updated 1 Dec 2025

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

## Priority queue (P1–P20)

| ID  | Title & scope (inline guardrails) |
|-----|----------------------------------|
| **P1 (✅ 2 Dec 2025)** | **Repo-wide rename + lint**: resolve B1 in code/tests/docs; run `cargo fmt`, `cargo clippy` to prove no stale identifiers remain. |
| **P2 (✅ 2 Dec 2025)** | **Baud / CLI defaults audit**: codify `/dev/ttyUSB0 @ 9600` as the baseline, ensure config/CLI overrides cover `/dev/ttyAMA0`/`ttyS*` devices, and add integration tests in `tests/bin_smoke.rs` for precedence + persistence. |
| **P3 (✅ 2 Dec 2025)** | **Config loader hardening**: enhance `src/config/loader.rs` to validate cols/rows ranges, default scroll/page timings, and ensure `~/.serial_lcd/config.toml` schema doc matches real struct. |
| **P4 (✅ 2 Dec 2025)** | **LCD driver regression tests**: expand `src/lcd_driver` tests for flicker-free updates, blinking, and icon resets; ensure no `unsafe`. |
| **P5 (✅ 2 Dec 2025)** | **Serial backoff telemetry**: add structured logging to `src/serial/*` capturing reconnect counts into `/run/serial_lcd_cache/serial_backoff.log`, respecting RAM-only constraint. |
| **P7 (✅ 3 Dec 2025)** | **CLI integration mode groundwork**: implemented `serialsh` preview flag (gated behind `serialsh-preview` feature), added `ShellContext` preview run path, and tests reflecting the flow. |
| **P8** | **Bi-directional command tunnel core**: base framing library in `src/payload/parser.rs` for command request/response envelopes (no network). Must reuse newline JSON framing. |
| **P9** | **Server/client auto-negotiation**: implement handshake state in `src/app/connection.rs`, ensuring deterministic fallback to current behaviour when remote does not understand negotiation packets. |
| **P10** | **Remote file push/pull transport**: extend payload schema for chunk IDs, checksums, resume markers; add tests covering corruption detection. Respect RAM-only buffering. |
| **P11** | **Live hardware polling agent**: modular polling routines (CPU %, temps, disk) gated via config, pushing frames through existing render loop without blocking serial ingestion. |
| **P13** | **JSON-protocol strict mode**: introduce schema validation (Serde enums, length caps) and optional `"schema_version"` header to reject malformed inputs gracefully. |
| **P14** | **Payload compression support**: evaluate LZ4 vs zstd (pure Rust crates allowed) for UART throughput; ensure streaming decompression fits <5 MB RSS. |
| **P15** | **Heartbeat + watchdog**: implement mutual heartbeat packets and fail-safe hooks to re-run LCD “offline” screen or trigger local script (within charter: no extra daemons). |
| **P13 (✅ 3 Dec 2025)** | **JSON-protocol strict mode**: introduce schema validation (Serde enums, length caps) and a required "schema_version" header to reject malformed inputs gracefully. |
| **P17** | **Remote file integrity tooling**: CLI helper to verify checksums and list staged chunks in `/run/serial_lcd_cache`. |
| **P19** | **Documentation + sample payload refresh**: update `README.md`, `samples/payload_examples*.json`, and `docs/lcd_patterns.md` showing new modes and tunnels. |
| **P20 (✅ 4 Dec 2025)** | **Serial transport resilience**: finalize explicit 8N1 + flow-control defaults in code, expose DTR/RTS toggles + timeout knobs via config for upcoming tunnels, and add structured error mapping/logs so reconnect logic can distinguish permission, unplug, and framing failures before Milestones A–C. _(Status: CLI + config cover flow-control/parity/stop-bits/DTR/timeouts, and telemetry now records categorized permission/unplug/framing causes for each reconnect.)_ |
| **P21 (✅ 3 Dec 2025)** | **Adopt hd44780-driver crate for Linux builds where possible**: migrate the internal HD44780 driver to use the external `hd44780-driver` crate (via a small adapter for the platform I²C bus) while preserving our public API for any missing functionality. |
| **P22** | **Custom character support and built in icons**: Add full HD44780 custom-character handling, including a built-in icon set and an API to load/swap glyph banks at runtime. _See Milestone H for execution details._ |

### Priority implementation plan

The remaining priorities can now be tackled in lockstep with the command-tunnel work that is already underway:

| Item | Current focus & next steps |
| --- | --- |
| **P8 — Bi-directional command tunnel core (in progress)** | **Status:** command-frame schema, CRC helpers, validation, and parse tests are merged (`src/payload/parser.rs`). Bridges/stubs in `src/app/events.rs` and `src/app/render_loop.rs` now detect `channel:"command"` frames without disturbing the LCD loop. Next: build the session FSM/executor, enqueue stdout/stderr chunks while cementing the allow-list and RAM-disk staging policies. Once command handling can produce `CommandEvent`s and send `TunnelMsg` responses, expand `tests/bin_smoke.rs` and `tests/integration_mock.rs` with command-round-trip coverage. |
| **P9 — Server/client auto-negotiation** | Plan: formalize the handshake in `src/app/lifecycle.rs` + `src/app/connection.rs`, serialize capability bitmaps through the command tunnel, and enforce deterministic role fallback to LCD-only if the peer is negotiation-unaware. Add `FakeSerialPort` smoke paths that exercise both sides and log negotiation traces under `/run/serial_lcd_cache/negotiation.log`. |
| **P10 — Remote file push/pull transport** | Plan: define chunk manifests in `src/milestones/transfer`, reuse the new command-frame schema to carry chunk metadata, persist resumable manifests in `/run/serial_lcd_cache`, and stream offloaded stdout into the same RAM-disk buffers. Tests will assert CRC detection and resume behavior against the fake serial harness. |
| **P11 — Live hardware polling agent** | Plan: implement the polling skeleton in `src/app/polling.rs`, feed metrics into the render loop through channels that share the command queue, and expose gating/config options when Milestone D hits. Start by mapping `sysinfo`/`os_info` snapshots into the new render events so dashboards can consume them later. |
| **P13 — JSON-protocol strict mode** | Plan: expand the parser (`src/payload/parser.rs`) to enforce schema_version tracking, length caps, and optional `schema_version` headers for every tunnel frame; tie this into `CommandBridge` so malformed frames trigger clear parse errors. Align tests with the new strict path (we already reject long lines/icons in payloads). |
| **P14 — Payload compression support** | Plan: evaluate `lz4_flex` vs `zstd-safe` in `src/app/compression.rs`, cap streaming buffers at <1 MB, and expose the toggle through a future config table so large file transfers/heartbeat bursts stay efficient without violating the 5 MB RSS budget. |
| **P15 — Heartbeat + watchdog** | Plan: revisit Milestone D’s polling/telemetry timers to emit heartbeat frames for both the LCD render loop and the command tunnel, allowing CLI clients to detect stalls and trigger offline screens. Keep watchdog state machines purely in `src/app/watchdog.rs` to avoid extra threads. |
| **P17 — Remote file integrity tooling** | Plan: add CLI helpers that inspect `/run/serial_lcd_cache` manifests, verify stored checksums, and list incomplete chunk uploads before Milestone C ships. Integrate the existing `crc32fast` helpers, and keep logs inside the RAM disk. |
| **P19 — Documentation + sample payload refresh** | Plan: refresh `README.md`, `samples/payload_examples*.json`, and `docs/lcd_patterns.md` with the tunnel payload formats, heartbeat guidance, and new icon banking rules once the schema/MLA work stabilizes. |
| **P22 — Custom character support and built-in icons** | Plan: expand `src/payload/icons.rs` and `src/display/lcd.rs` to manage CGRAM banks, add icon scheduling tests, and document the behavior in `docs/icon_library.md`; tie the icon manager into the command tunnel so remote greet frames can request glyph swaps. |

By keeping this plan visible in the roadmap, every contributor can spot the dependency graph: P8 feeds Milestone A, which in turn unlocks P9–P11 before the later payload and heartbeat work lands.

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

### Milestone A — Bi-Directional Command Tunnel

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

### Milestone A plan & status

Milestone A is now **in progress**: the framing/schema work is merged, the render loop acknowledges `channel:"command"` frames, and the `CommandBridge`/`CommandEvent` router exposes the inlet that the executor will eventually consume. The outstanding steps are:

1. Implement the command FSM in `src/app/events.rs`, capturing `Request`, `Chunk`, `Busy`, and `Exit` transitions with RAM-only stash buffers for stdout/stderr.
2. Extend `src/app/tunnel.rs` (or a dedicated executor module) to launch whitelisted binaries via `std::process::Command`, stream bounded chunks through `TunnelMsg`, and log under `/run/serial_lcd_cache/tunnel/`.
3. Wire `tests/bin_smoke.rs` and `tests/integration_mock.rs` to exercise command round-trips, CRC rejection, and `Busy` races across the fake serial harness.
4. Keep `lifelinetty --serialsh` (feature-gated) as the CLI surface, leaving the default `--run` daemon path focused on LCD rendering.

Once these pieces arrive, Milestone A will own a stable command tunnel that shares the UART with the LCD while honoring the RAM-disk/cache guardrails.

### Milestone B — Server/Client Auto-Negotiation

- **Goal**: Both endpoints boot without config edits; handshake decides who acts as command server vs client.
- **Scope**: state machine additions in `src/app/lifecycle.rs` + `src/app/connection.rs`, handshake payload definitions in `src/payload/parser.rs`.
- **Dependencies**: P9, P19. Needs fallback path to classic “LCD-only” mode when remote lacks support.
- **Constraints**: handshake occurs after serial open but before LCD writes; any timeout must revert to default LCD display to avoid blank screens.
- **Workflow**:
  1. Introduce negotiation states in `src/app/lifecycle.rs` with deterministic timers.
  2. Encode capabilities bitmap inside JSON payloads; extend parser tests for unknown bits.
  3. Update `src/app/render_loop.rs` to pause rendering until role is resolved, with watchdog fallback.
  4. Document expected behavior and add integration tests with fake serial endpoints (`tests/fake_serial_loop.rs`).
- **Crates & tooling**: `serialport`, `tokio` (for async timeout), `anyhow`/`thiserror` for richer negotiation errors, `log` for trace-level negotiation logging.

### Milestone C — Remote File Push/Pull

- **Goal**: send logs/configs/binaries via UART with chunking, checksum, resume.
- **Scope**: new transport module (`src/app/events.rs` extensions), cache utilization under `/run/serial_lcd_cache`, CLI helpers for send/recv.
- **Dependencies**: P10, P17, P13 (schema). Requires compression milestone (E) for large transfers.
- **Constraints**: never write outside RAM disk except when user explicitly moves file into `~/.serial_lcd/` config path.
- **Workflow**:
  1. Design chunk metadata struct (chunk_id, size, crc32) and add serde support.
  2. Implement sender/receiver state machines using buffered file handles pointing to `/run/serial_lcd_cache`.
  3. Integrate resume logic keyed by chunk_id + checksum; store resumable manifests in RAM disk.
  4. Extend CLI with `--push`/`--pull` commands (documented but disabled until stable) and cover with integration tests using fixtures in `tests/integration_mock.rs`.
- **Crates & tooling**: `crc32fast` (already in tree) for chunk verification, `serialport` / `tokio-serial` for transport, consider `anyhow` for layered error propagation.

### Milestone D — Live Hardware Polling

- **Goal**: gather CPU/mem/temp/disk/network metrics
- **Scope**: gather metrics from hosts system of cpu/mem/temp/disk.
- **Dependencies**: P11, P15, P18.
- **Constraints**: polling intervals must be configurable
- **Workflow**:
  1. Build polling module with systemstat crate.
  2. Publish metrics into render queue via channels guarded by `std::sync::mpsc` or `crossbeam` (if later approved) to avoid blocking serial ingestion.
  3. Implement heartbeat packets (serde structs) and integrate into render loop timers; fallback to offline screen if missed.
- **Crates & tooling**: `os_info` for system information, `log` for watchdog alerts, optional `tokio` timers if async polling chosen. systemstat or similar crate for metrics gathering.

### Milestone E — LCD/Display Output Mode Expansion

- **Goal**: push mission dashboards or multi-screen overlays to attached LCD/LED panels automatically.
- **Scope**: extend `src/display/*` and `docs/lcd_patterns.md`; add payload options for multi-panel layouts.
- **Dependencies**: P12, P20.
- **Constraints**: keep HD44780 compatibility first; additional panels must use the same I²C backpack or compatible driver.
- **Workflow**:
  1. Model additional panel layouts in `src/display/overlays.rs`, using enums for placement and transitions.
  2. Expand payload schema for multi-panel directives, ensuring backward compatibility.
  3. Update `lcd_driver` to batch updates per panel, minimizing I²C writes.
  4. Provide demo payloads + tests covering new display modes.
- **Crates & tooling**: `hd44780-driver`, `linux-embedded-hal`, `rppal` for I²C, `log` for refresh diagnostics.

#### Milestone E.1 — hd44780-driver migration (subtask of Milestone E)

- **Goal**: Adopt the `hd44780-driver` crate for Linux builds in place of our in-tree implementation where appropriate, preserving the public `Lcd` and `lcd_driver::Hd44780` APIs and behavior.
- **Scope**: `src/lcd_driver/mod.rs`, `src/lcd_driver/pcf8574.rs`, `src/display/lcd.rs`, `src/config/loader.rs`, `README.md`, `docs/lcd_patterns.md`, and tests.
- **Dependencies**: P4, P12, `rppal`, `linux-embedded-hal`.
- **Workflow**:
  1. Add a small I2C adapter that presents a `rppal::i2c::I2c` (or `linux-embedded-hal::I2cdev`) as an `embedded-hal::blocking::i2c::Write` implementation for the `hd44780-driver` crate.
  2. Add new `Hd44780::new_external(i2c_hal, addr, cols, rows)` and `Lcd::new_with_bus(...)` constructors. Keep old constructors as fallback.
  3. Implement CGRAM/custom char helpers for the external driver by writing the CGRAM command and pattern bytes through the bus adapter (ensure alignment with the external crate's nibble/4-bit conventions).
  4. Preserve the `I2cBus` trait for tests and the internal code path; share integration tests that validate parity in output and glyphs.
  5. Add `--test-lcd` smoke tests and an optional `display.driver: "hd44780-driver" | "in-tree" | "auto"` config value that keeps the default experience unchanged but allows CI/experimental testing.
  6. Incrementally default to `hd44780-driver` on Linux if all tests pass and hardware scans show consistency for 2 weeks.

- **Acceptance**:
  - Public `Lcd` facade and CLI remain unchanged.
  - Feature parity validated (write_line, flicker-free writes, backlight toggle, blink, custom chars) by tests and hardware smoke runs.
  - Memory and runtime constraints remain within the charter (<5 MB RSS) when the external driver is engaged.


### Milestone F — JSON-Protocol Mode + Payload Compression

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

### Milestone G — CLI Integration Mode (serialsh)

- **Goal**: Program behaves like a pseudo-shell when invoked with a special flag, piping commands to remote tunnel and showing immediate output.
- **Scope**: CLI UX in `src/cli.rs`, command loop integration in `src/app/mod.rs`, docs/README updates.
- **Dependencies**: Milestone A completion, P16.
- **Constraints**: maintain compatibility with existing `--run` default; serialsh must remain optional and disabled on boot services unless configured.
- **Workflow**:
  1. Extend CLI parser to accept `--serialsh` and interactive options (history file stored in RAM disk if needed).
  2. Wire CLI loop to command tunnel channel, handling Ctrl+C via `ctrlc` crate (if approved) to send termination packets.
  3. Ensure output formatting mirrors POSIX shells while staying pure text (no ANSI requirements by default).
  4. Add doc section + integration tests verifying exit codes and error handling.
- **Crates & tooling**: `clap`-style parsing currently in tree (custom), consider `rustyline` alternative only if allowed; otherwise use `std::io` for line editing; `ctrlc` (already dependency) for signal handling.

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

---

### Tracking & next steps

- Close B1–B6, then tackle P1–P4 in order to stabilize the base.
- Once telemetry + schema groundwork (P5–P13) is stable, schedule milestone A/B builds.
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

