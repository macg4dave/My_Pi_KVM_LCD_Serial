# LifelineTTY Roadmap (Dec 2025)

Updated 1 Dec 2025

LifelineTTY is the successor to SerialLCD: a single Rust daemon for Raspberry Pi 1 (ARMv6) that ingests newline-delimited JSON via UART and keeps a HD44780 LCD in sync. All work listed here must respect the charter in `.github/copilot-instructions.md` — single binary, no network sockets, RAM-disk cache only, CLI flags remain stable (`--run`, `--test-lcd`, `--test-serial`, `--device`, `--baud`, `--cols`, `--rows`).

## Context & guardrails (Option A inline reminders)

- **Storage**: only `~/.serial_lcd/config.toml` may be written persistently; all temp/log files live under `/run/serial_lcd_cache`.
- **Interfaces**: UART input defaults to `/dev/ttyUSB0` (9600 8N1) but must accept any TTY path provided via config/CLI overrides; LCD via PCF8574 @ 0x27; CLI only, no new protocols without explicit approval.
- **Quality bar**: `cargo test`, `cargo clippy`, `cargo fmt` clean on x86_64 + armv6; every public API has Rustdoc; README documents every flag/config option.
- **Testing**: add/refresh tests in `tests/` and `src/**` modules for each behavioral change; hidden CI assumes watchdog coverage, so no `#[allow(dead_code)]` escapes.

## Blockers (must fix before new scope)

1. **B1 — Finish rename to LifelineTTY everywhere (Done)**: README, `docs/architecture.md`, `docs/releasing.md`, `lifelinetty.service`, `seriallcd.service`, packaging scripts, Dockerfiles, Makefile, and tests were updated to replace `seriallcd` with `lifelinetty`. Compatibility aliases (`/usr/bin/seriallcd`, `seriallcd.service`) remain in packaging for backwards compatibility. Update further, if needed, via the P1 lint & doc cleanup task.
2. **B2 — Charter + instructions alignment**: keep `.github/copilot-instructions.md`, README, and prompt files synchronized on the `/dev/ttyUSB0 @ 9600` defaults and the expectation that config overrides can point at `/dev/ttyAMA0`, `/dev/ttyS0`, USB adapters, etc.
3. **B3 — Config + cache policy audit**: search for any path writes outside `/run/serial_lcd_cache` or `~/.serial_lcd/config.toml`. Add tests in `tests/bin_smoke.rs` to ensure temporary files respect the cache.
4. **B4 — CLI docs/tests parity**: README and `tests/bin_smoke.rs` must document + test `--demo`, `--cols`, `--rows`, etc. Add missing coverage and doc sections.
5. **B5 — AI prompt + automation refresh**: `.github/instructions/*.md` and any devtool prompts (e.g., `rust_mc_ui.prompt.md`) must mention the new product name and scope before downstream contributors start feature work.
6. **B6 — Release tooling sanity pass**: `scripts/local-release.sh`, Dockerfiles, and packaging metadata must emit `lifelinetty_*` artifacts and service units. Without this, installers will remain branded `seriallcd`.

> _Only after B1–B6 are closed should we land anything from the P1–P20 queue or milestone features below._

## Priority queue (P1–P20)

| ID  | Title & scope (inline guardrails) |
|-----|----------------------------------|
| **P1** | **Repo-wide rename + lint**: resolve B1 in code/tests/docs; run `cargo fmt`, `cargo clippy` to prove no stale identifiers remain. |
| **P2** | **Baud / CLI defaults audit**: codify `/dev/ttyUSB0 @ 9600` as the baseline, ensure config/CLI overrides cover `/dev/ttyAMA0`/`ttyS*` devices, and add integration tests in `tests/bin_smoke.rs` for precedence + persistence. |
| **P3** | **Config loader hardening**: enhance `src/config/loader.rs` to validate cols/rows ranges, default scroll/page timings, and ensure `~/.serial_lcd/config.toml` schema doc matches real struct. |
| **P4** | **LCD driver regression tests**: expand `src/lcd_driver` tests for flicker-free updates, blinking, and icon resets; ensure no `unsafe`. |
| **P5** | **Serial backoff telemetry**: add structured logging to `src/serial/*` capturing reconnect counts into `/run/serial_lcd_cache/*.log`, respecting RAM-only constraint. |
| **P6** | **State machine metrics**: instrument `src/app/render_loop.rs` with counters for frames accepted/rejected (already defined) and expose via CLI `--test-serial` output without breaking protocol. |
| **P7** | **CLI integration mode groundwork**: design `serialsh` pseudo-shell behaviour within current CLI (`lifelinetty --run --serialsh` placeholder flag) while keeping contract stable; gated behind feature flag until milestone ready. |
| **P8** | **Bi-directional command tunnel core**: base framing library in `src/payload/parser.rs` for command request/response envelopes (no network). Must reuse newline JSON framing. |
| **P9** | **Server/client auto-negotiation**: implement handshake state in `src/app/connection.rs`, ensuring deterministic fallback to current behaviour when remote does not understand negotiation packets. |
| **P10** | **Remote file push/pull transport**: extend payload schema for chunk IDs, checksums, resume markers; add tests covering corruption detection. Respect RAM-only buffering. |
| **P11** | **Live hardware polling agent**: modular polling routines (CPU %, temps, disk) gated via config, pushing frames through existing render loop without blocking serial ingestion. |
| **P12** | **LCD/display output mode**: add `display_mode = "panel"` payload option to mirror state onto auxiliary LCD/LED expansions while keeping HD44780 output primary. |
| **P13** | **JSON-protocol strict mode**: introduce schema validation (Serde enums, length caps) and optional `"schema_version"` header to reject malformed inputs gracefully. |
| **P14** | **Payload compression support**: evaluate LZ4 vs zstd (pure Rust crates allowed?) for UART throughput; ensure streaming decompression fits <5 MB RSS. |
| **P15** | **Heartbeat + watchdog**: implement mutual heartbeat packets and fail-safe hooks to re-run LCD “offline” screen or trigger local script (within charter: no extra daemons). |
| **P16** | **CLI tunneling UX polish**: history, prompt, exit codes surfaced nicely in `--test-serial` mode without breaking automation. |
| **P17** | **Remote file integrity tooling**: CLI helper to verify checksums and list staged chunks in `/run/serial_lcd_cache`. |
| **P18** | **Config-driven polling profiles**: allow `profiles` table in `config.toml` to customize polling intervals per metric, validated via tests. |
| **P19** | **Automatic downgrade/compat layer**: ensure older SerialLCD senders can talk to LifelineTTY by sniffing payload capabilities before enabling new features. |
| **P20** | **Documentation + sample payload refresh**: update `README.md`, `samples/payload_examples*.json`, and `docs/lcd_patterns.md` showing new modes and tunnels. |

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

### Milestone D — Live Hardware Polling + Heartbeat/Watchdog

- **Goal**: gather CPU/mem/temp/disk/network metrics and stream to LCD, plus heartbeat watchdog resets if remote silent.
- **Scope**: polling threads in `src/app/demo.rs` or new module, config toggles, heartbeat state in render loop.
- **Dependencies**: P11, P15, P18.
- **Constraints**: polling intervals must be configurable; watchdog actions limited to LCD overlays or local scripts (no rebooting the Pi without systemd coordination).
- **Workflow**:
  1. Build polling module (e.g., `src/app/polling.rs`) that reads `/proc` and `/sys` metrics with non-blocking IO.
  2. Publish metrics into render queue via channels guarded by `std::sync::mpsc` or `crossbeam` (if later approved) to avoid blocking serial ingestion.
  3. Implement heartbeat packets (serde structs) and integrate into render loop timers; fallback to offline screen if missed.
  4. Add config knobs in `config.toml` and tests verifying defaults + overrides.
- **Crates & tooling**: standard library (`std::fs`, `std::time`), `linux-embedded-hal` (if GPIO-based sensors), `log` for watchdog alerts, optional `tokio` timers if async polling chosen.

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

---

### Tracking & next steps

- Close B1–B6, then tackle P1–P4 in order to stabilize the base.
- Once telemetry + schema groundwork (P5–P13) is stable, schedule milestone A/B builds.
- Maintain this roadmap alongside `docs/architecture.md` and update when priorities shift (always annotate date + reason).
