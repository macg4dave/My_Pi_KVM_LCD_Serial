# 📌 Milestone D — Live Hardware Polling & Heartbeat

> Roadmap alignment: **Milestone D** corresponds to **P11 (Live hardware polling agent)**, **P15 (Heartbeat + watchdog)**, and **P18 (Config-driven polling profiles)** in [docs/roadmap.md](./roadmap.md). Work here assumes Milestones A–C delivered tunnel framing, negotiation, and file-transfer plumbing.

## Current repo snapshot (Dec 2025)

- `src/app/render_loop.rs` owns the only long-running loop. It handles LCD updates, reconnect banners, and a local blink-based heartbeat when frames stop, but it has no concept of remote heartbeats or metrics events.
- `state::RenderState` and `payload::parser` only understand LCD frames (`Payload`/`RenderFrame`). There is no tunneled schema yet; Milestone A will add it, and Milestone D will extend it with metrics + heartbeat variants.
- `Cargo.toml` lists the currently approved crates (`serialport`, `hd44780-driver`, `serde`, `crc32fast`, `ctrlc`, `rppal`, `linux-embedded-hal`). There is **no** `systemstat`, so polling must parse `/proc` + `/sys` directly or leverage `rppal` for Pi sensors.
- `CACHE_DIR` lives in `src/lib.rs` and is already enforced by `app::Logger` and `serial::telemetry`. Any new logs or caches must live in `/run/serial_lcd_cache`.

## Target outcome

1. Raspberry Pi 1 nodes stream CPU, memory, load, disk, temperature, and optional network counters without exceeding the 5 MB RSS ceiling or starving LCD updates.
2. A background polling worker emits structured metrics frames and dedicated heartbeat packets into the same tunnel router as LCD payloads so command/file-transfer traffic stays coordinated.
3. Heartbeat timeouts drive the LCD offline overlay (`display::overlays::render_offline_message`) and pause tunnel/file-transfer actions until the remote endpoint recovers.
4. Polling cadence, heartbeat intervals, and per-metric enable flags are configured via `~/.serial_lcd/config.toml`, validated in `config::loader`, and surfaced through `AppConfig`.
5. Temporary artifacts (logs, cached samples) stay under `/run/serial_lcd_cache/metrics/` and are automatically rotated when the daemon restarts.

## Implementation plan

### 1. Config + feature gates

- Extend `src/config/mod.rs` and `config/loader.rs` with an optional `[polling]` table:

  ```toml
  [polling]
  interval_ms = 5000          # clamp to 1000..60000
  heartbeat_interval_ms = 500 # clamp to 200..5000
  heartbeat_timeout_ms = 2000 # must be > heartbeat_interval_ms
  enable_cpu = true
  enable_memory = true
  enable_disk = true
  enable_temp = true
  enable_network = false
  ```

- Merge these values into `AppConfig` (similar to cols/rows/backoff) so `render_loop` can spawn or skip the worker without CLI changes (charter forbids new flags unless explicitly approved).
- Validation errors reuse `Error::InvalidArgs` and are covered by new tests in `config::loader`.

### 2. Polling worker module (`src/app/polling.rs`)

- Create a lightweight thread via `std::thread::spawn` that receives a cloned `Logger` and a bounded `std::sync::mpsc::Sender<PollingEvent>`.
- Gather metrics using standard library IO:
  - `/proc/stat` for CPU totals (store the previous sample to compute deltas).
  - `/proc/meminfo` for memory totals/free/available.
  - `/proc/loadavg` for 1/5/15-minute load averages.
  - `libc::statvfs` for root filesystem capacity/free bytes.
  - `/sys/class/thermal/thermal_zone*/temp` through buffered reads (optional when `enable_temp`).
  - `/sys/class/net/<iface>/statistics/{rx,tx}_bytes` when `enable_network` is true; interfaces are sanitized to alphanumeric names.
- Reuse string buffers across iterations to keep allocations low; convert samples into a `MetricsSample` struct that only stores primitive numbers.
- Emit `PollingEvent::Metrics(MetricsSample)` and `PollingEvent::Heartbeat(HeartbeatSample)` each tick. Errors log via `logger.warn(...)` but do not kill the thread.

### 3. Tunnel schema + payload parsing

- When Milestone A adds the `TunnelMsg` serde envelope, extend it with:

  ```rust
  pub enum TunnelMsg {
      // existing variants…
      Metrics(MetricsMsg),
      Heartbeat(HeartbeatMsg),
  }
  ```

- `MetricsMsg` mirrors `MetricsSample` but caps optional arrays/strings to satisfy future P13 strict-mode requirements. `HeartbeatMsg` contains `{ "timestamp_ms": u64, "seq": u64 }`.
- Update `src/payload/parser.rs` tests to cover JSON round-trips, field bounds, and malicious inputs (e.g., giant strings, negative numbers).

### 4. Render loop + lifecycle integration

- Add a `std::sync::mpsc::Receiver<PollingEvent>` to `run_render_loop`. Poll it once per iteration with `try_recv()` so serial reads remain priority.
- Introduce `MetricsState` (likely `src/state.rs`) that caches the latest metrics and exposes formatted LCD lines (e.g., `CPU 42%  Temp 55C`). When configured, enqueue a metrics frame into `RenderState` so it rotates with other payloads.
- Extend `lifecycle.rs` with `HeartbeatState { last_seq, last_seen }`. When heartbeats stop for `heartbeat_timeout_ms`, toggle the offline overlay and pause file-transfer/command operations until the next heartbeat.
- When we are the negotiated “server,” emit outbound heartbeats by passing `TunnelMsg::Heartbeat` through the existing serial writer; when we are the “client,” ingest remote heartbeats and feed them into `HeartbeatState`.

### 5. Storage, logging, and observability

- Add `/run/serial_lcd_cache/metrics/metrics.log` using the existing cache-aware logger helper. Include `polling_tick`, `heartbeat_seq`, and any sensor errors.
- Reuse `tracing` spans (already enabled elsewhere) to annotate polling and heartbeat activity so operators can correlate with `/run/serial_lcd_cache/serial_backoff.log`.
- Provide a `lifelinetty_metrics.json` snapshot (optional) under the same directory for manual inspection; rotate it on each successful poll.

### 6. Testing

- `config::loader` unit tests covering `[polling]` defaults, overrides, and validation failures (bounds + timeout ordering).
- `app::polling` unit tests that feed synthetic `/proc` data via temporary files and assert parsed totals/deltas.
- `tests/integration_mock.rs` scenario where metrics + heartbeat events arrive while LCD frames continue, proving the render loop stays responsive and offline overlay triggers when heartbeats stop.
- `tests/bin_smoke.rs` ensures `lifelinetty --help` references the new config table and that the daemon runs when `[polling]` is omitted.
- Future `tests/fake_serial_loop.rs` (once LCD stubs are enabled per P4) to verify interleaving of metrics, command tunnel, and file-transfer traffic.

### 7. Out of scope for Milestone D

- Adding new dependencies like `systemstat` or async runtimes beyond the optional `tokio-serial` feature.
- Additional CLI subcommands/flags; all control remains in `config.toml`.
- Persisting metrics outside `/run/serial_lcd_cache` or emitting them over non-UART transports.
- Per-metric interval profiles (those belong to P18 once the base polling agent ships).

## Allowed crates & dependencies

Polling and heartbeat work stay within the approved crate list: `std`, `serde`, `serde_json`, `crc32fast`, `hd44780-driver`, `serialport`, optional `tokio`/`tokio-serial` (only through the existing feature flag), `rppal`, `linux-embedded-hal`, and `ctrlc`. Metrics readers parse `/proc` + `/sys` manually so no new crates such as `systemstat` are introduced.

## Result

Milestone D ships a RAM-disk-compliant polling subsystem that feeds structured metrics and heartbeat packets into the existing render/tunnel logic without blocking serial IO. It keeps code localized to `src/app/polling.rs`, `src/app/render_loop.rs`, `src/app/lifecycle.rs`, and the config/payload layers already in this repository, paving the way for stricter watchdog behavior in later priorities.
