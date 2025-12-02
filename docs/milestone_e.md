# 📌 Milestone E — LCD/Display Output Mode Expansion (P12 + P20)

*Draft specification for Milestone e of the LifelineTTY project. This file documents design intent only—no code here is executable.*

---

> Scope alignment: Roadmap Milestone E depends on **P12 (LCD/display output mode)** and the documentation/test refresh tracked as **P20** in [docs/roadmap.md](./roadmap.md). Work delivered here must respect the charter guardrails: HD44780-first, PCF8574 I²C backpacks, <5 MB RSS, RAM-disk caches only, CLI flags unchanged.

## Goal

Extend the existing render pipeline (`payload::RenderFrame → display::overlays → display::lcd`) so LifelineTTY can target **one or more HD44780 panels** without breaking current single-screen deployments. Payload authors gain a `display_mode = "panel"` option (P12) plus explicit panel directives that let them mirror or partition content across multiple 16×2 / 20×4 modules. Documentation, samples, and regression tests (P20) explain how to opt in and how the fallback path works when only the primary panel is connected.

### Success criteria

- Supports **at least two panels** with independent PCF8574 addresses while keeping the primary panel mandatory and defaulting to today’s behavior.
- `display_mode = "panel"` (and related directives) parse via `payload::RenderFrame` without regressing `DisplayMode::Normal|Dashboard|Banner`.
- Render loop drives additional panels without starving serial ingestion or exceeding the RAM-disk policy.
- Tests cover parser changes, panel failover, and multi-panel render logic (host-only mocks are acceptable; hardware-only tests stay `#[ignore]`).
- `docs/lcd_patterns.md`, `samples/*.json`, and this milestone file document the new payload fields and configuration knobs.

## Current architecture snapshot

- `src/payload/parser.rs` converts incoming JSON frames into `RenderFrame` structs with `DisplayMode`, scroll/backlight flags, and optional progress bars.
- `src/display/overlays.rs` performs scroll math, icon overlays, and heartbeat injection before delegating to `display::lcd::Lcd`.
- `src/display/lcd.rs` holds a single HD44780 facade backed by `lcd_driver::Hd44780<RppalBus>` on Linux or an in-memory stub elsewhere.
- `src/app/render_loop.rs` keeps exactly one `Lcd` instance inside the loop; all frames are rendered serially to that device.

Milestone E keeps this flow but lets the render loop fan out frames to additional `Lcd` instances created from config.

## Workstreams

### 1. Payload schema & parser upgrades (P12)

**Files:** `src/payload/parser.rs`, `src/payload/icons.rs`, `src/payload/mod.rs`, `docs/lcd_patterns.md`, `samples/*.json`, `tests/integration_mock.rs` (parser assertions).

- Add `DisplayMode::Panel` (and optional `DisplayMode::PanelMirror`) to describe multi-panel payloads. Unknown strings continue to map to `DisplayMode::Normal`.
- Extend `Payload` with an optional `panels` array:

  ```json
  {
    "mode": "panel",
    "panels": [
      {"id": "primary", "line1": "OPS", "line2": "Nominal"},
      {"id": "diagnostics", "line1": "Temp:42C", "line2": "RSSI:-71"}
    ]
  }
  ```

  - Each entry inherits defaults from the root payload unless overridden (bar, blink, scroll, etc.).
  - Missing `panels` → treat the root payload as the single panel (backward compatibility).
- Update `RenderFrame` to include `Vec<PanelFrame>` plus helper methods so the render loop can ask “does this frame carry per-panel overrides?” without re-parsing JSON.
- Add parser tests that prove:
  - Legacy single-panel payloads still succeed.
  - `display_mode = "panel"` rejects malformed panel arrays with actionable errors.
  - Checksums cover the canonical payload including the new `panels` field.

### 2. Panel runtime & overlay wiring (P12)

**Files:** `src/app/render_loop.rs`, `src/display/overlays.rs`, `src/display/mod.rs`, `src/state.rs`, `src/app/events.rs`.

- Introduce a lightweight `PanelId` enum/struct so the render loop can reference `primary`, `mirror`, `diagnostics`, etc. (string-backed for Serde but strongly typed internally).
- Extend `RenderState` so it stores the normalized panel metadata inside each `RenderFrame`; dedupe logic remains byte-based to avoid accidental re-render loops.
- In `overlays.rs`, add helpers that accept a `(panel_id, RenderFrame)` tuple and compute scroll offsets independently per panel. When a frame contains per-panel overrides:
  - Primary panel stays on the existing code path.
  - Auxiliary panels call the same overlay helpers but skip heartbeat injection if the payload marks them as passive mirrors.
- Update `app::render_loop` to maintain a small list (Vec) of `PanelRuntime` structs, each holding `Lcd`, scroll offsets, and last-render timestamps. The main loop continues to poll serial once per iteration; panel writes happen after parsing so no busy waits are introduced.
- Heartbeat + reconnect UI stays tied to the primary panel. Auxiliary panels fall back to a static “Panel offline” template (rendered via `overlays::render_offline_message`) whenever their I²C bus errors out.

### 3. LCD + config plumbing (P12)

**Files:** `src/config/mod.rs`, `src/config/loader.rs`, `src/lcd_driver/*`, `src/display/lcd.rs`, `docs/architecture.md`.

- Add optional `[[panels]]` sections to `~/.serial_lcd/config.toml`:

  ```toml
  [[panels]]
  id = "primary"
  cols = 20
  rows = 4
  pcf8574_addr = "auto"

  [[panels]]
  id = "diagnostics"
  cols = 16
  rows = 2
  pcf8574_addr = "0x26"
  mode = "mirror"
  ```

  - Default: a single implicit `primary` panel created from the existing top-level `cols`, `rows`, and `pcf8574_addr` fields. Config validation ensures `primary` exists exactly once and any auxiliary panels share the same driver type.
- Teach `display::lcd::Lcd` how to expose the PCF8574 address it latched (for logging) and add a helper constructor that accepts a pre-initialized `RppalBus` when we need deterministic ordering across multiple devices.
- In the render loop, instantiate all configured panels at startup. If a panel fails to initialize, log the error, keep running, and mark that panel as `degraded` so payloads know it cannot be targeted this session.
- No new crates are introduced; reuse `hd44780-driver`, `linux-embedded-hal`, and `rppal` only.

### 4. Documentation, samples, and regression tests (P20)

**Files:** `docs/milestone_e.md` (this file), `docs/lcd_patterns.md`, `README.md` (display section), `samples/payload_examples.json`, `tests/integration_mock.rs`, `tests/bin_smoke.rs` (if we expose CLI demos).

- Expand `docs/lcd_patterns.md` with ASCII diagrams for multi-panel dashboards, alert overlays, and mirrored strips. Each diagram must mention how it degrades when only the primary panel is connected.
- Refresh `samples/payload_examples.json` with at least two payloads using `display_mode = "panel"`:
  1. Mirror mode — same status text on both panels.
  2. Split mode — metrics on auxiliary panel, mission copy on primary.
- Add parser + render tests:
  - Host-only unit tests that instantiate multiple stub `Lcd` instances (non-Linux path in `display::lcd`) and assert that `render_frame_with_scroll` can run twice without panicking.
  - Integration test feeding a fake serial loop that emits a multi-panel payload and verifying we write to both panels (via stub history).
- Document operational guidance in `README.md` + `docs/architecture.md`: wiring diagram, config snippet, and warnings about current draw per panel.

## Acceptance checklist

1. Parser understands `display_mode = "panel"` and `panels[]` without regressing legacy payloads (unit tests in `payload::parser` prove it).
2. Multiple `Lcd` devices can be created from config; failures surface as warnings but do not crash the daemon.
3. Render loop updates each active panel at the cadence dictated by the frame while keeping the serial ingest/backoff behavior unchanged (profiling on Pi 1 stays under 5 MB RSS and no busy loops are introduced).
4. Auxiliary panels fall back to a safe template (e.g., “PANEL offline”) or mirror primary output when a payload references an unknown `panel_id`.
5. Docs + samples explain how to opt in, and new tests cover both parser and render logic.

## Sample payloads

```json
// Mirror both panels with the same payload (default when mode="panel" and no panels[] overrides)
{"line1":"OPS NOMINAL","line2":"UTC 22:15","mode":"panel"}

// Split content between primary + diagnostics
{
  "line1":"OPS STAT",
  "line2":"All green",
  "mode":"panel",
  "panels": [
    {"id":"primary","line1":"OPS STAT","line2":"All green"},
    {"id":"diagnostics","line1":"Temp 42C","line2":"Vbat 4.9V","scroll":false}
  ]
}
```

## Test & rollout plan

- Unit tests: `cargo test payload::parser overlays::tests state::tests` (x86_64) with the new panel cases un-ignored.
- Integration tests: extend `tests/integration_mock.rs::renders_payload_frames` (or add `tests/display_panels.rs`) to assert both panels receive updates using the stub `Lcd` backend.
- Hardware smoke: run `lifelinetty --test-lcd --cols <cols> --rows <rows>` on Pi 1 with two PCF8574 backpacks connected at addresses 0x27 and 0x26. Capture logs proving graceful degradation when the auxiliary bus is unplugged mid-run.
- Release notes: mention the new config format and advise users to regenerate `~/.serial_lcd/config.toml` if they want to define auxiliary panels.

## Allowed crates & dependencies

The display expansion sticks to the same approved crates: `std`, `serde`, `serde_json`, `crc32fast`, `hd44780-driver`, `serialport`, optional `tokio`/`tokio-serial` via the existing feature flag, `rppal`, `linux-embedded-hal`, and `ctrlc`. Multi-panel support reuses the current LCD driver abstractions without pulling in GUI/layout libraries or additional HAL crates.

## Out of scope

- SPI/OLED/TFT displays, fancy compositors, or new CLI flags — stay HD44780-only per charter.
- Async runtimes or additional threads beyond what the render loop already uses; panel writes piggyback on the existing loop.
- Command-tunnel UI changes (Milestone A) or polling overlays (Milestone D); those consume the panel API later.

By grounding Milestone E in today’s modules and tests we ensure the LCD expansion lands incrementally, keeps the daemon stable on Raspberry Pi 1 hardware, and sets up downstream milestones without speculative architecture.
