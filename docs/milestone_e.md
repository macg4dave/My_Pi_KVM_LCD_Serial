
*Draft specification for Milestone e of the LifelineTTY project. This file documents design intent only—no code here is executable.*
---

# 📌 Milestone E — LCD/Display Output Mode Expansion

*(Multi-panel, overlays, dashboards — fully Rust-NLS, HD44780-first)*

**Goal**
Enable LifelineTTY to automatically render **mission dashboards, overlays, multi-screen views, and alternate display layouts** to one or more HD44780-compatible LCD modules (or compatible I²C backpacks).

The display system becomes a composable, data-driven layer:
**Panels → Layout Model → Render Scheduler → I²C Driver**

---

# 🎯 Architectural Principles

Borrowing high-level concepts from ser2net’s approach to modular transports (not its byte-passthrough):

* **Strict layering**: panels, layouts, orchestration, hardware driver all separated.
* **Declarative configuration**: rendering described by payload/enums, not imperative code.
* **Batch operations**: skip wasteful writes; update only what’s changed (HD44780 is slow).
* **Compatibility first**: HD44780 is the baseline; every new panel must share the same driver model (I²C backpack or compatible bus).
* **Backwards-compatible payload schema**: old single-screen messages still work unchanged.

All of this remains pure safe Rust + crates.

---

# 📂 Scope / File Layout

```
src/
 ├ display/
 │   ├ lcd_driver.rs          # low-level HD44780 I²C driver (unchanged baseline)
 │   ├ model.rs               # NEW: panels, regions, transitions, overlays
 │   ├ overlays.rs            # NEW: predefined overlay layouts
 │   ├ scheduler.rs           # NEW: batches updates, diffing, frame timing
 │   ├ compositor.rs          # NEW: merges data + layout → concrete LCD cells
 ├ payload/
 │   ├ schema.rs              # expand with multi-panel display directives
 ├ docs/
 │   └ lcd_patterns.md        # expanded descriptions & examples
tests/
 └ display_modes.rs           # integration tests for layouts/panels
```

---

# 🧱 1. Display Model (Layout Engine)

*(Workflow 1)*

Add a **layout model** that describes panels and how content maps onto them.

### Panel description

Represented as simple logical surfaces:

* width (chars)
* height (lines)
* I²C address
* coordinate system
* optional orientation flags

Example structural fields (no code, just design):

**PanelKind**

* `Primary` (default)
* `Auxiliary`
* `Diagnostics`
* `Banner`
* `External`

**Placement enums**

* `Top`
* `Bottom`
* `FullWidth`
* `SplitLeft`
* `SplitRight`
* `Overlay`
* `HiddenUntilTriggered`

**Transition enums**

* `Cut`
* `ScrollLeft`
* `ScrollUp`
* `FadeSimulated` (HD44780 can't fade, but you can simulate with patterns)
* `SlideIn`
* `SlideOut`

Everything here is a data model — no code semantics needed yet.

---

# 🧱 2. Expanded Payload Schema

*(Workflow 2)*

Extend your existing `TunnelMsg::Display(…)` family so it can specify:

### A. Panel-level rendering directives

* `RenderTo { panel_id, layout, payload }`
* `Overlay { overlay_id, payload }`
* `Dashboard { mode, fields… }`

### B. Multi-panel instructions:

* `MultiRender { panels: Vec<PanelDirective> }`

### C. Backwards compatibility

* Existing simple “draw-line” or “LCDUpdate” messages remain untouched.
* If a controller receives a multi-panel message but only has one screen:

  * it maps to the primary panel only
  * other panes are silently ignored
  * no protocol failure

### D. Field rendering directives

For dashboards:

* `Field::Cpu`
* `Field::Temp`
* `Field::NetRx`
* `Field::Clock`
* `Field::StatusMessage(String)`
* `Field::FileTransferProgress { percent }`
* etc.

This plugs directly into Milestone D metrics if present.

---

# ⚙️ 3. Updated LCD Driver (Batching)

*(Workflow 3)*

HD44780 is slow. Updating it naïvely wastes cycles.
Milestone E introduces a **render scheduler** to batch writes.

Conceptual behaviour:

* Input: a list of concrete cell-differences (“line 1 col 5 changed from ‘a’ to ‘b’”).
* Group operations per panel:

  * set cursor
  * write run-length segments
  * avoid per-character I²C writes
* Honor I²C bus addressing (other panels = different backpacks).
* Respect user-configured refresh limits (e.g. max writes/sec).

The `lcd_driver.rs` remains 100% Rust, using:

* `hd44780-driver`
* `linux-embedded-hal` or
* `rppal` for Raspberry Pi I²C access

No unsafe. No C bindings.

---

# 🚦 4. Render Scheduler

*(New layer — Rust-only)*

This sits between **layouts** and **lcd_driver**.

Responsibilities:

* watch for layout changes or payload updates
* produce a frame buffer per panel
* diff new frame vs previous
* schedule batched writes using I²C
* maintain refresh timing

Scheduler policies:

* “Always render primary panel first”
* “Treat overlays as highest priority”
* “If any panel fails, fall back to primary-only mode”

---

# 🎭 5. Overlays

*(Workflow 1 extension)*

Predefined overlays stored in `overlays.rs`:

Examples:

### A. Mission Overlay

* CPU/Mem/Temp bar graphs
* Network stats
* File-transfer progress lanes

### B. Alert Overlay

* “REMOTE OFFLINE”
* “LOW CPU”
* “FILE TRANSFER ERROR”
* Flashes on top of current panel without clearing everything

### C. Split-Screen Overlay

* Left: metrics
* Right: logs
* Auto-scroll with jitter correction

### D. Clock & Status Bar

* Always-on top row
* Mission status on bottom row

You expose these via payloads, not local function calls.

---

# 📚 6. Documentation Update

*(Workflow 4)*

Update `docs/lcd_patterns.md`:

* List all panels supported
* Show every overlay mode with a textual mock-up
* Provide example JSON payloads for:

  * multi-panel dashboard
  * file-transfer progress
  * time-series/metrics layout
  * mission alert overlays
  * minimal fallback examples
* Note HD44780 limits (scroll regions, buffer sizes)

---

# 🧪 7. Display Tests

*(Workflow 4)*

Add `tests/display_modes.rs`:

Tests cover:

### ✔ Layout → compositor correctness

* Ensure that each layout maps fields to correct coordinates.
* Verify transitions generate expected intermediate states (mock-only).

### ✔ Multi-panel backward compatibility

* If only 1 panel configured, multi-panel messages should not panic.
* Panels with unsupported layouts produce a safe fallback.

### ✔ Batching logic

* Ensure minimal number of I²C writes for various diff cases.
* Validate that overlays override correctly.

### ✔ Fallback behaviour

* If I²C says “device not present”, system degrades gracefully:

  * primary-only
  * static offline message
  * no fatal errors

---

# 🔒 8. Constraints & Guarantees

### HD44780-first

* All layouts, overlays, dashboards must degrade gracefully to single HD44780 16×2 or 20×4.

### Multi-panel consistency

* All panels must share the **same driver model** (HD44780-compatible backpacks).
* No SPI/TFT/OLED divergence at this milestone — keep unified.

### Never block serial

Display rendering must never interfere with command-tunnel, file-transfer, or heartbeat loops.
The scheduler runs as a **separate async task** consuming messages from a queue.

---

# 🏁 Summary: What Milestone E Delivers

By the end of Milestone E, LifelineTTY will have:

* A **multi-panel, multi-layout display engine**
* A declarative, serde-driven **display protocol**
* A compositor + scheduler that **minimizes I²C writes**
* Support for mission dashboards, overlays, alerts, progress bars
* Safe fallback paths (single panel → single dashboard → simple LCD update)
* Full backwards compatibility with existing single-screen LCD mode
* 100% safe Rust, with crates only (`hd44780-driver`, `linux-embedded-hal`, `rppal`)

This is the right foundation for **dynamic mission UIs**, “smart KVM displays,” and real-time operational dashboards.

---

If you want next, I can build the **full display architecture diagram**, or produce a **ZIP with skeleton modules** ready to drop directly into your repository.
