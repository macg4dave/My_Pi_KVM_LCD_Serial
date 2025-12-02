# 📌 Milestone B — Auto-negotiation: server/client role resolution (Rust-native, async, framed)

*Draft specification for Milestone B of the LifelineTTY project. This file documents design intent only—no code here is executable.*

## Outcome

Enable two LifelineTTY endpoints to connect with zero manual configuration, decide deterministically which node acts as **server** (runs commands and drives LCD policy) and which acts as **client** (issues tunnel requests), and fall back gracefully to legacy LCD-only mode if the peer does not negotiate. This milestone implements roadmap item **P9** and feeds directly into milestones A (command tunnel) and C (file transfer).

## Success criteria

- Each endpoint emits a `Hello` control frame within 250 ms of opening the UART.
- Negotiation converges in ≤2 round-trips (or ≤1 s) under normal conditions; otherwise we enter `LcdOnlyFallback` automatically.
- Both peers agree on the same chosen role using deterministic tie-breakers (node IDs, preferences) and log the decision.
- Unknown capability bits or version mismatches never crash the daemon; they downgrade to the safest compatible behaviour.
- LCD rendering stays paused until a role is finalized, ensuring the display never shows stale or competing content.

## Dependencies & prerequisites

1. Milestone A framing (`TunnelMsg`) supplies CRC-checked transport for control-plane traffic.
2. Roadmap items **P5** (serial backoff telemetry) and **P8** (tunnel framing) remain untouched but continue to log serial health.
3. Node identifiers are persisted in `~/.serial_lcd/config.toml` so both peers have stable IDs; loader validation from **P3** applies here.

## Architecture overview

- **Lifecycle FSM (`src/app/lifecycle.rs`)** — introduces `LifecycleState::{Negotiating, Active(NegotiatedRole)}` with timeout + retry bookkeeping. Negotiation runs once at startup or after serial reconnects.
- **Connection driver (`src/app/connection.rs`)** — owns the serial backend, emits `Hello`, listens for control frames, and invokes the FSM’s decision helpers.
- **Render loop gating (`src/app/render_loop.rs`)** — blocks LCD updates and tunnel traffic until `Active` with either Server/Client role or fallback.
- **Payload schema (`src/payload/schema.rs`)** — defines `ControlMsg`, `Capabilities` bitflags, `RolePreference`, and `NegotiatedRole`, all serialized via serde and validated with unit tests.
- **Testing harness (`tests/fake_serial_loop.rs`)** — uses `tokio::io::duplex` pairs to simulate two devices, race conditions, and error cases without hardware.

## Control-plane protocol

```rust
bitflags::bitflags! {
    #[derive(Serialize, Deserialize)]
    pub struct Capabilities: u32 {
        const HANDSHAKE_V1  = 0b0000_0001;
        const CMD_TUNNEL_V1 = 0b0000_0010;
        const LCD_V2        = 0b0000_0100;
        const HEARTBEAT_V1  = 0b0000_1000;
        const FILE_XFER_V1  = 0b0001_0000; // reserved for Milestone C
    }
}

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

- `proto_version` starts at `1`; newer nodes send both version and caps so older peers can downgrade.
- Frames reuse Milestone A’s newline JSON + CRC envelope; invalid frames are logged and ignored.

## Decision flow

1. **Send HELLO** — after serial open, queue `Hello { node_id, caps, pref }` and start a negotiation deadline (default 1 s, configurable via `[negotiation] timeout_ms`).
2. **Handle peer HELLO** — compare `node_id` and `RolePreference` to choose roles: higher `node_id` wins when both prefer the same role; explicit preferences override when peers disagree.
3. **Emit HELLOACK** — confirm the chosen role with `HelloAck { chosen_role, peer_caps }` and transition to `Active(role)`.
4. **Legacy detection** — if no valid control frames arrive before the deadline, emit `LegacyFallback`, set `Active(LcdOnlyFallback)`, and resume pre-milestone behaviour.
5. **Retries** — failed negotiations retry up to 3 times with exponential backoff that respects existing serial backoff logic.

## Configuration & logging

- New optional table in `~/.serial_lcd/config.toml`:

```toml
[negotiation]
node_id = 123456         # default derived from MAC-like hash
preference = "prefer_server"  # or "prefer_client" / "no_preference"
timeout_ms = 1000
```

- Missing values fall back to safe defaults validated in `src/config/loader.rs` (alignment with P3).
- Decision logs go to stderr and `/run/serial_lcd_cache/logs/negotiation.log`, trimmed on startup.

## Testing & validation

1. **Happy path** — both peers send HELLO concurrently; verify deterministic tie-breaker and matching `NegotiatedRole`.
2. **Legacy peer** — only one side speaks control-plane frames; expect fallback after timeout with LCD-only behaviour.
3. **Version mismatch** — simulate `proto_version = 2` vs `1`; ensure downgrade and capability masking.
4. **Unknown capability bits** — set future bits; serde must ignore them while preserving known flags.
5. **Collision + retries** — drop `HelloAck` on one side to exercise retry loop and ensure state machine recovers.
6. **Broken link** — close serial mid-negotiation and confirm we re-enter `Negotiating` on reconnect.

All tests live in `tests/fake_serial_loop.rs` with helper fixtures for node IDs and capability masks.

## Guardrails & out-of-scope items

- No sockets, PTYs, or extra threads; negotiation happens inside the existing async runtime.
- Negotiation never writes outside `/run/serial_lcd_cache` and config directory; manifests or transcripts stay in RAM.
- Roles only affect which side processes command/file tunnel requests; LCD fallback screens remain identical to today’s behaviour when negotiation fails.
- Milestone B does **not** expose new CLI flags; configuration stays confined to `config.toml` or compile-time defaults.

Delivering Milestone B ensures every subsequent milestone (command tunnel, file transfer, heartbeat) can assume a well-defined server/client split without manual setup.
