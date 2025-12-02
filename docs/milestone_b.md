
Draft specification for the Milestone B of the LifelineTTY project.
NOTE: This is not code to be executed, but rather documentation describing design ideas and architecture of the feature my be implemented
---

# **📌 Milestone B — Auto-Negotiation: Server/Client Role Resolution (Rust-Native, Async, Framed)**

**Goal**
Make both LifelineTTY endpoints boot with zero configuration.
When the serial link comes up, the two units **negotiate roles** and agree on:

* **Server** — runs commands, streams stdout/stderr, drives LCD logic when appropriate
* **Client** — issues command requests
* **Fallback** — behaves in “LCD-only” legacy mode if the remote side doesn’t support negotiation

Everything is built on top of Milestone A’s framed JSON/CBOR + CRC32 transport.

---

# **🎯 Design Principles (Rust-ified ser2net lessons)**

We borrow concepts from ser2net’s architecture — but **never its byte-passthrough design**.

✔ Clear layering (backend → framing → control-plane → data-plane)
✔ A single “connection task" owns the serial port (no sharing)
✔ Deterministic state machine (negotiation → active)
✔ Strict framed communication, not arbitrary byte-stream
✔ Full async, no blocking, no PTYs, no networking
✔ Resilient against partial frames, timeouts, unknown capabilities
✔ Always falls back to safe LCD mode

---

# **📦 Crates Used**

* `tokio` — async runtime, `select!`, timers
* `tokio-serial` — UART backend
* `bitflags` — capability map
* `serde`, `serde_json` or `serde_cbor` — handshake/control message serialization
* `crc32fast` — use the same framing from Milestone A
* `thiserror` — structured negotiation errors
* `tracing` — debug/trace logs
* `bytes` — buffer mgmt for partial frames

All safe, stable Rust crates.

---

# **📚 Codebase Integration**

```
src/
 ├ app/
 │   ├ lifecycle.rs        # NEW: negotiation FSM + role state
 │   ├ connection.rs       # serial open + negotiation driver
 │   ├ render_loop.rs      # delays LCD until role established
 │   ├ events.rs           # command execution (Mile A)
 │   └ heartbeat.rs
 ├ payload/
 │   ├ schema.rs           # NEW: ControlMsg, Capability bits
 │   ├ parser.rs           # framing extended for control msgs
 ├ serial/
 │   └ backend.rs          # tokio-serial wrapper
tests/
 └ fake_serial_loop.rs     # two-endpoint negotiation simulation
```

---

# **🧩 1. Capabilities (bitflags)**

Use `bitflags` for capability negotiation.

```rust
bitflags::bitflags! {
    #[derive(Serialize, Deserialize)]
    pub struct Capabilities: u32 {
        const HANDSHAKE_V1    = 0b00000001;
        const CMD_TUNNEL_V1   = 0b00000010;
        const LCD_V2          = 0b00000100;
        const HEARTBEAT_V1    = 0b00001000;
        // Reserved future bits ignored gracefully
    }
}
```

Unknown bits don’t break decoding.

---

# **🧩 2. Control-Plane Messages**

Transported using Milestone A’s framing (JSON/CBOR + CRC32 wrapper):

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ControlMsg {
    Hello {
        proto_version: u8,
        node_id: u32,
        caps: Capabilities,
        pref: RolePreference,
    },
    HelloAck {
        chosen_role: Role,
        peer_caps: Capabilities,
    },
    LegacyFallback,
}
```

Preferences:

```rust
#[derive(Serialize, Deserialize, Copy, Clone)]
pub enum RolePreference {
    PreferServer,
    PreferClient,
    NoPreference,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq)]
pub enum Role {
    Server,
    Client,
}

pub enum NegotiatedRole {
    Server,
    Client,
    LcdOnlyFallback,
}
```

---

# **🧠 3. Lifecycle FSM (`src/app/lifecycle.rs`)**

```rust
pub enum LifecycleState {
    Negotiating(NegState),
    Active(NegotiatedRole),
}

pub struct NegState {
    pub sent_hello: bool,
    pub retries: u8,
    pub deadline: tokio::time::Instant,
    pub caps: Capabilities,
    pub pref: RolePreference,
    pub node_id: u32,
}
```

### State Transition Summary

* On open: → `Negotiating`
* Send HELLO immediately
* Wait for peer HELLO or HELLOACK
* Resolve collisions with deterministic tie-breaker:

  ```
  if local_node_id > remote_node_id → local=Server
  else → local=Client
  ```

* Send HelloAck
* Transition to `Active(Server|Client)`
* If timeout → `Active(LcdOnlyFallback)`
* If unknown/garbage frames → ignore until timeout

---

# **⚙️ 4. Negotiation Driver (`connection.rs`)**

Sketch:

```rust
pub async fn negotiate_role(
    serial: &mut SerialPortBackend,
    local_caps: Capabilities,
    local_pref: RolePreference,
    node_id: u32,
) -> Result<NegotiatedRole, NegotiationError> {

    send_hello(...);

    let deadline = Instant::now() + NEG_TIMEOUT;

    loop {
        tokio::select! {
            frame = serial.recv_frame() => {
                let msg = decode_control_or_ignore(frame)?;
                match msg {
                    ControlMsg::Hello { node_id: theirs, caps, pref, .. } => {
                        // collision or one-sided
                        let role = decide_role(node_id, theirs, local_pref, pref);
                        send_hello_ack(role, caps)?;
                        return Ok(map_role(role));
                    }
                    ControlMsg::HelloAck { chosen_role, .. } => {
                        return Ok(map_role(chosen_role));
                    }
                    _ => {}
                }
            }

            _ = tokio::time::sleep_until(deadline) => {
                return Ok(NegotiatedRole::LcdOnlyFallback);
            }
        }
    }
}
```

All error types use `thiserror`.

---

# **📉 5. Render Loop Integration (`render_loop.rs`)**

Before role is established:

* Do **not** send smart LCD updates or command-tunnel frames
* Optional: show “connecting…” static image or just hold last-known LCD state
* Once role is resolved → enable Milestone A’s logic

If fallback:

* Operate **exactly as old LCD mode**
* Ignore all control messages after fallback

---

# **📜 6. Tests (`tests/fake_serial_loop.rs`)**

Simulate two endpoints using `tokio::io::duplex`.

### Tests you must include

#### ✔ Basic Successful Negotiation

* Both send HELLO
* Tie-breaker picks consistent Server/Client roles
* Both transition to `Active`

#### ✔ Legacy Partner (no handshake)

* Only one sends HELLO
* Other only sends legacy LCD messages / garbage
* Timeout → `LcdOnlyFallback`

#### ✔ Version mismatch

* If proto_version incompatible → fallback
* Test logs reflect mismatch

#### ✔ Unknown capability bits

* peer caps have upper bits set
* decoder ignores them
* negotiation still succeeds

#### ✔ Hello collision race

* Both send HELLO at the same instant
* Deterministic node_id tie-break

#### ✔ Broken peer (drops halfway)

* Peer sends HELLO then disconnects
* Local times out → fallback

---

# **🧨 7. Safety Guarantees**

* No PTY
* No shell
* No networking
* Deterministic FSM
* One task owns serial port
* Strict framing + CRC + serde ensures correctness
* Timeouts guarantee nothing hangs
* Fallback mode guarantees LCD is never left blank
* Fully async, safe Rust only

---

# **🏁 Summary: What This Milestone Gives LifelineTTY**

* Fully automatic role negotiation
* Safe, robust fallback
* Zero configuration
* Clean separation of control-plane vs data-plane
* Async FSM integrated with your serial reactor
* Properly documented + testable behaviour
* A future-proof, extensible basis for heartbeat, command tunneling, and LCD ops

---

If you want, I can now generate:

* **the exact Rust module skeletons**,
* **the negotiation FSM code**,
* or a **ZIP with all stubs pre-created** matching your repo layout.

Say the word.
