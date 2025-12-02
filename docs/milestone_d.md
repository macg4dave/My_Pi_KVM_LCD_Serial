
Draft specification for the Milestone B of the LifelineTTY project.
NOTE: This is not code to be executed, but rather documentation describing design ideas and architecture of the feature my be implemented
---

# 📌 Milestone D — Live Hardware Polling (Rust-Native, Async/Nonblocking, Framed)

**Goal**
Continuously gather **CPU, memory, temperature, disk, and (optional) network metrics** on the host platform and feed them into the tunnel & LCD rendering pipeline. Includes heartbeat generation & offline detection.

**Critical Rule**
This **must not** block the serial loop, file-transfer loop, or command-tunnel loop. Polling must always be asynchronous, throttled, and isolated.

---

# 🎯 Architectural Principles

Taken from ser2net in *concept*, not in implementation:

* **One core I/O reactor owns the serial link** (as in Milestone A).
* **Everything else communicates via message queues**.
* **Never block the main loop**.
* **Treat metrics as a structured protocol, not raw text**.
* **Use timers + state machines**, not sleep loops.

Where ser2net used event loops (gensio), we use **Tokio timers + channels**.

---

# 📂 Scope + File Layout

```
src/
 ├ app/
 │   ├ polling.rs           # NEW: async metrics poller + heartbeat generator
 │   ├ render_loop.rs       # consumes metrics frames
 │   ├ lifecycle.rs         # heartbeat liveness checks
 │   └ connection.rs
 ├ payload/
 │   ├ schema.rs            # add MetricsMsg + HeartbeatMsg
 │   └ parser.rs
 ├ config/
 │   └ config.toml          # add polling interval knobs
tests/
 └ polling_defaults.rs
```

---

# 🧩 1. Polling Backend (`src/app/polling.rs`)

Two implementation strategies:

### ✔ Option A: `systemstat` crate

Clean, safe, async-friendly (just wrap system calls in tasks).

### ✔ Option B: Manual `/proc` + `/sys` readers

Use `tokio::fs` + buffered readers. Good for custom parsing and minimal deps.

Both are 100% Rust, safe, and configurable.

---

# 🧱 Type Definitions (Serde)

Add these to `payload/schema.rs`:

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MetricsMsg {
    pub cpu_usage: f32,
    pub load_avg: (f32, f32, f32),
    pub mem_total: u64,
    pub mem_free: u64,
    pub mem_avail: u64,
    pub disk_total: u64,
    pub disk_free: u64,
    pub temp_celsius: Option<f32>,
    pub net_rx: Option<u64>,
    pub net_tx: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HeartbeatMsg {
    pub timestamp_ms: u64,
    pub seq: u64,
}
```

These will be wrapped inside `TunnelMsg::Metrics(MetricsMsg)` and `TunnelMsg::Heartbeat(HeartbeatMsg)` with the same outer framing used everywhere else (Milestone A).

---

# 🧠 2. Async Polling Task (Tokio)

In `polling.rs`:

```rust
pub async fn start_polling(
    tx: mpsc::Sender<TunnelMsg>,
    interval: Duration,
) {
    let mut ticker = tokio::time::interval(interval);
    let mut seq = 0;

    loop {
        ticker.tick().await;

        if let Ok(metrics) = gather_metrics().await {
            let _ = tx.send(TunnelMsg::Metrics(metrics)).await;
        }

        let hb = HeartbeatMsg {
            timestamp_ms: now_ms(),
            seq,
        };
        seq += 1;

        let _ = tx.send(TunnelMsg::Heartbeat(hb)).await;
    }
}
```

**Key rule:**
The poller **never touches the serial port directly**. It only sends structured frames through the internal queue.

This ensures serial traffic is never blocked, even if `/proc` has a stall or a temperature sensor is missing.

---

# 🖥️ 3. Metric Collection Logic (Nonblocking)

Example using `systemstat`:

```rust
pub async fn gather_metrics() -> anyhow::Result<MetricsMsg> {
    use systemstat::{Platform, System};

    let sys = System::new();
    let cpu = sys.cpu_load_aggregate()?.done()?;
    let mem = sys.memory()?;
    let load = sys.load_average()?;
    let disk = sys.mount_at("/")?;

    let temp = sys.cpu_temp().ok();   // Optional: not all boards expose this

    Ok(MetricsMsg {
        cpu_usage: 1.0 - cpu.idle,
        load_avg: (load.one, load.five, load.fifteen),
        mem_total: mem.total.as_u64(),
        mem_free: mem.free.as_u64(),
        mem_avail: mem.avail.map(|v| v.as_u64()).unwrap_or(0),
        disk_total: disk.total.as_u64(),
        disk_free: disk.free.as_u64(),
        temp_celsius: temp.map(|t| t as f32),
        net_rx: None,  // optional network reading in future
        net_tx: None,
    })
}
```

No blocking. Pure Rust. Always returns quickly.

For missing sensors, return `None`, not an error.

---

# 📡 4. Render Loop Integration

In `render_loop.rs`:

* Add another branch in the serial multiplexer:

```rust
tokio::select! {
    Some(msg) = metrics_rx.recv() => {
        match msg {
            TunnelMsg::Metrics(m) => update_lcd_metrics(m),
            TunnelMsg::Heartbeat(h) => update_heartbeat_state(h),
            _ => {}
        }
    }
    // … existing serial/LCD/command/file-transfer branches …
}
```

* **LCD view should reflect metrics only if in the correct role** (e.g., server or client depending on design).
* **Heartbeat misses** should trigger fallback screens (“remote offline”).

---

# ❤️ 5. Heartbeat System (Critical)

Heartbeat is part of Milestone D because:

* file-transfer (C)
* command tunnel (A)
* negotiation (B)
* metrics (D)

all need a *liveness guarantee*.

### Heartbeat Rules

* Every `interval` subset (e.g. metrics every 5s, heartbeat every 500ms).
* Heartbeat messages include a monotonic sequence number.
* On receive side, if N heartbeats are missed:

  * mark remote as “offline”
  * flip LCD into offline/fallback mode
  * block file transfers + commands

### Detection Logic:

In `lifecycle.rs`:

```rust
struct HeartbeatState {
    last_seq: u64,
    last_seen: Instant,
}

impl HeartbeatState {
    fn seen(&mut self, seq: u64) {
        self.last_seq = seq;
        self.last_seen = Instant::now();
    }

    fn is_alive(&self, timeout: Duration) -> bool {
        self.last_seen.elapsed() < timeout
    }
}
```

Render loop checks this periodically.

---

# ⚙️ 6. Configuration: `config.toml`

Add keys:

```toml
[polling]
interval_ms = 5000
heartbeat_interval_ms = 500
heartbeat_timeout_ms = 2000
```

Test coverage:

* defaults load correctly
* overrides apply correctly
* invalid values fail gracefully

---

# 🧪 7. Tests

### ✔ `tests/polling_defaults.rs`

* Ensure default polling period is correct
* Ensure heartbeat is emitted regularly
* Mock a stalled poll (inject artificial failure) and ensure the system recovers
* Ensure one serial failure doesn’t block metrics queue

### ✔ Integration Test w/ Fake Serial

Using `tokio::io::duplex`:

* Metrics transmitted as framed messages
* Heartbeats interleaved with file-transfer and command-tunnel traffic
* Heartbeat timeout → system switches to offline-mode

---

# 🔒 8. Safety & Constraints

* **No blocking I/O anywhere in polling** — use `tokio::fs`, `tokio::task`, `systemstat`, or pure async `/proc` parsing.
* **Never read sensors more frequently than configured interval** — prevent load spikes.
* **No direct hardware writes** unless you later add sensors via embedded-hal.
* **Heartbeat is fully typed, CRC’d, and framed** — consistent with Milestones A/B/C.

---

# 🏁 Summary: What Milestone D Delivers

You now have:

* A full async hardware-polling module
* Metrics surfaced cleanly into the LCD renderer & serial tunnel
* Heartbeat-based liveness detection for remote endpoints
* Configurable intervals and safe defaults
* Nonblocking, deterministic behaviour
* 100% Rust, built on crates, layered, testable, robust

-