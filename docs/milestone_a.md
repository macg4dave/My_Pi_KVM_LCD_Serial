Draft specification for the Milestone A of the LifelineTTY project.
NOTE: This is not code to be executed, but rather documentation describing design ideas and architecture of the feature my be implemented.

# **📌 Milestone A — Bi-Directional Command Tunnel (Rust-Native, Async, Framed)**

**Goal:**
Implement a **structured command/response tunnel over UART**, capable of:

* receiving JSON-framed command requests
* executing allowed one-line commands locally
* streaming structured stdout/stderr chunks back
* maintaining session health with heartbeats
* running concurrently with existing LCD serial traffic

Zero networking. Zero PTYs. Zero C. Pure Rust + crates.

---

# **🎯 High-Level Architecture (inspired by ser2net’s layering but Rustified)**

1. **Serial Backend Layer**

   * Owns UART fully.
   * Provides `AsyncRead + AsyncWrite` abstraction over `tokio-serial`.
   * Enforces chunked, buffered writes to avoid flooding.
   * Single task owns the serial port, no sharing.

2. **Framing Layer (JSON or CBOR + CRC32)**

   * Serial bytes → newline-delimited frames → CRC check → serde decode.
   * Structured messages only; no raw passthrough.
   * Zero-copy where possible via borrowed slices.
   * Upstream of serial, downstream of command executor.

3. **Command Session Layer (FSM)**

   * Simple finite-state machine: `Idle` → `Running(pid)` → `Exit` → `Idle`.
   * Only one active command at a time.
   * Rejects additional requests when busy.
   * Handles timeouts, exit codes, cleanup.

4. **Process Execution Layer**

   * Uses `tokio::process::Command`.
   * Streams stdout/stderr back in bounded 256–512 byte chunks.
   * Resource controlled: tiny working dir, capped buffers, no stdin, no PTY.

5. **Multiplex Loop**

   * One `tokio::select!` loop handles:

     * serial RX
     * stdout chunks
     * stderr chunks
     * LCD updates
     * heartbeat timer

Everything runs under the same user with minimal RSS.

---

# **📦 Required Rust Crates**

* `tokio` — async runtime, tasks, timers, select loop
* `tokio-serial` — async serial I/O backend
* `serde` + `serde_json` or `serde_cbor` — framing layer
* `crc32fast` — fast checksums
* `thiserror` — error types
* `tracing` — structured logs
* `tokio::process` — streaming child processes
* `bytes` — buffer mgmt for chunked IO
* `tokio::sync::mpsc` — stdout/stderr channels

100% safe Rust. No FFI.

---

# **📂 File/Layout Impact (updated)**

```
src/
 ├ app/
 │   ├ connection.rs        # manages serial port backend + async reactor
 │   ├ render_loop.rs       # now multiplexes LCD + tunnel traffic
 │   ├ events.rs            # FSM + command executor + session control
 │   └ heartbeat.rs         # heartbeat tracker (Milestone D)
 ├ payload/
 │   ├ parser.rs            # framing encode/decode + CRC
 │   ├ schema.rs            # serde enums using Cow<'a>
 │   └ mod.rs
 ├ serial/
 │   └ backend.rs           # wrapper struct over tokio-serial
```

---

# **📜 Protocol / Schema (Rust-native)**

### Enum for requests/responses

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TunnelMsg<'a> {
    CmdRequest { cmd: Cow<'a, str> },
    Stdout     { chunk: Cow<'a, [u8]> },
    Stderr     { chunk: Cow<'a, [u8]> },
    Exit       { code: i32 },
    Busy,
    Heartbeat,
}
```

### Framing

* Line-delimited JSON or CBOR
* Outer wrapper contains CRC32 of raw `msg` bytes:

```json
{ "msg": { ... }, "crc32": 123456789 }
```

* Reject on mismatch
* Hard size cap: max 4 KB frame input

---

# **🧠 Session Logic (FSM)**

State lives in `events.rs`:

```rust
enum SessionState {
    Idle,
    Running { pid: u32 },
}
```

Rules:

* If `Idle` + `CmdRequest` → spawn → Running
* If `Running` + `CmdRequest` → send `Busy`
* Heartbeat resets timeout
* Exit code = drop to Idle

No interleaving. No multi-command batching.

---

# **⚙️ Command Execution Workflow**

1. Parse command request
2. Validate:

   * must be one line
   * must refer only to whitelisted binaries or paths
3. Spawn child via `tokio::process::Command`
4. Create two tasks:

   * stdout reader → sends `Stdout` chunks
   * stderr reader → sends `Stderr` chunks
5. On exit:

   * send `Exit { code }`
   * FSM → Idle

All output is streamed, never buffered.

---

# **🔄 Multiplexer Loop (main engine)**

Inside `render_loop.rs`:

```rust
tokio::select! {
    // incoming serial frames
    Ok(frame) = serial_rx.recv() => handle_frame(frame),

    // outgoing chunks from processes
    Some(out) = stdout_rx.recv() => serial_tx.send(out),
    Some(err) = stderr_rx.recv() => serial_tx.send(err),

    // LCD updates
    _ = lcd_tick.tick() => render_lcd(),

    // heartbeat
    _ = heartbeat_tick.tick() => check_session_health(),
}
```

The entire program behaves like a deterministic I/O reactor — tight, predictable, safe.

---

# **🧪 Tests (improved)**

`tests/bin_smoke.rs` now verifies:

* command request → stdout stream → exit code round-trip
* stderr-only cases
* mixed stdout/stderr
* checksum mismatch
* partial-frame reconstruction
* Busy-state correctness
* Large output (file dump) yields chunked frames
* Heartbeat timeout aborts child process

Use `tokio::io::duplex` to simulate fake serial.

---

# **📉 Memory + Safety Guarantees**

* No PTY → no TTY line-discipline, eliminating whole class of bugs
* No shell → no injection
* Max frame size → bounded parsing
* Chunked IO → high control and tiny RSS
* Only one active command → deterministic state
* One owner of serial port → no races
* Pure async Rust → zero unsafe blocks

This is **far cleaner and safer than ser2net**, while still using some of its architectural lessons.

---

If you want, I can now write:

* the **full protocol spec**
* the **exact trait definition for the serial backend**
* or a **complete Rust prototype** you can paste in your repo.
