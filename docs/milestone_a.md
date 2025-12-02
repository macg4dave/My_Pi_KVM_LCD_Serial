# 📌 Milestone A — Bi-Directional Command Tunnel (Rust-native, async, framed)

*Draft specification for Milestone A of the LifelineTTY project. This file documents design intent only—no code here is executable.* This plan must stay synchronized with the priority notes in [docs/roadmap.md](./roadmap.md).

## Outcome

Deliver a single command/response tunnel running over the existing UART link so that the “client” LifelineTTY instance can submit short commands and receive stdout/stderr streams without breaking LCD updates. This milestone fulfills roadmap items **P7 (CLI groundwork), P8 (tunnel framing)**, and **P16 (CLI UX polish)**, and prepares the ground for heartbeat enforcement from Milestone D/P15.

## Success criteria

- `lifelinetty --run --serialsh` (or config-gated equivalent) forwards one-line commands to the tunnel and exits with the remote process’ status code.
- Structured frames (`TunnelMsg`) are newline-delimited JSON with CRC32 footers and reject malformed or oversized (>4 KB) payloads.
- Only one active command session exists at a time; additional requests return `Busy` immediately.
- LCD rendering and serial ingest remain responsive (<50 ms jitter per loop) while commands run.
- All writes outside `/run/serial_lcd_cache` and `~/.serial_lcd/config.toml` are rejected.

## Dependencies & prerequisites

1. Blockers B1–B6 and priorities P1–P4 are already complete (see `docs/roadmap.md`).
2. CLI groundwork from **P7** defines the `--serialsh` gate; the flag stays hidden until Milestone A lands.
3. Payload framing work from **P8** supplies the serde definitions, CRC helpers, and newline framing utilities.
4. Heartbeat message schema from Milestone D (**P15**) is reused for session health.

## Architecture & layering

1. **Serial backend (`src/serial/{mod,async,sync}.rs`)** — reuse the current `SerialPort` abstraction and, when the existing `async-serial` feature is enabled, the optional `tokio-serial` adapter. The default Pi 1 build remains synchronous so no new backend files or runtimes are added for this milestone.
2. **Framing (`src/payload/parser.rs`)** — converts newline-delimited byte streams into `{ msg, crc32 }` envelopes, validates CRC via `crc32fast`, and hands decoded `TunnelMsg<'_>` values upstream.
3. **Command session FSM (`src/app/events.rs`)** — tracks `SessionState::{Idle, Running { pid }}` and enforces exclusivity. Busy responses are emitted immediately when `Running`.
4. **Process executor (`std::process::Command`)** — spawns whitelisted binaries, captures stdout/stderr into 256–512 byte chunks, and stores temporary buffers in `/run/serial_lcd_cache/tunnel/` to respect RAM-only writes. Optional async glue simply feeds the same executor through bounded channels.
5. **Multiplex loop (`src/app/render_loop.rs`)** — extend the existing synchronous loop to drain tunnel queues between serial reads so LCD rendering stays first-class. Optional async integrations keep using the repo’s `async-serial` feature; no new background runtimes are introduced.

## Module impact

| File | Change |
| --- | --- |
| `src/app/connection.rs` | Initialize tunnel channels, own the serial backend, and surface CRC/framing errors as structured events. |
| `src/app/events.rs` | Implement session FSM, command validation, and stdout/stderr forwarding. |
| `src/app/render_loop.rs` | Multiplex LCD frames with tunnel traffic and heartbeats. |
| `src/cli.rs` | Gate `--serialsh` (feature flag) and document failure modes without changing default CLI semantics. |
| `src/payload/{parser.rs,schema.rs}` | Add `TunnelMsg` enums with borrowed data plus CRC wrappers and serde tests. |
| `tests/bin_smoke.rs` & `tests/integration_mock.rs` | Cover framing, Busy state, CRC rejection, command success/failure paths, and LCD coexistence using the existing `serial::fake::FakeSerialPort` harness (or the revived `tests/fake_serial_loop.rs`). |

## Protocol & framing

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TunnelMsg<'a> {
    CmdRequest { cmd: Cow<'a, str> },
    Stdout { chunk: Cow<'a, [u8]> },
    Stderr { chunk: Cow<'a, [u8]> },
    Exit { code: i32 },
    Busy,
    Heartbeat,
}
```

- Frames are newline-delimited JSON objects containing `{ "msg": <TunnelMsg>, "crc32": <u32> }`.
- Frames larger than 4096 bytes or with mismatched CRCs are dropped and logged to `/run/serial_lcd_cache/tunnel/errors.log`.
- Borrowed variants (`Cow<'a, str>` / `Cow<'a, [u8]>`) avoid heap churn while parsing.

## Session flow

1. Client sends `CmdRequest { cmd }` while in `Idle`.
2. Server validates command against a static allow-list (paths, length, UTF-8) and enters `Running { pid }`.
3. Executor streams stdout/stderr through bounded `std::sync::mpsc` (or existing channel utilities) as `Stdout`/`Stderr` frames.
4. On child exit, server emits `Exit { code }`, returns to `Idle`, and acknowledges via heartbeat.
5. If another `CmdRequest` arrives during `Running`, immediately reply with `Busy`.
6. Heartbeat misses (handled via Milestone D) tear down the session and surface an error to the CLI.

## CLI, config, and cache usage

- No new persistent config keys. The tunnel is enabled either via `--serialsh` or a documented `[tunnel] enable = true` config table that defaults to false.
- Temporary buffers, stdout/stderr logs, and executor scratch files live under `/run/serial_lcd_cache/tunnel/`. The directory is cleaned at start/stop.
- Command allow-list resides in `~/.serial_lcd/config.toml` as documented arrays, validated on load (leveraging P3 config hardening).
- The service never mounts filesystems, launches PTYs, or spawns shells; commands are invoked directly with arguments split client-side.

## Testing & validation

1. **Unit tests (`src/payload/parser.rs`)** — CRC happy-path, CRC failure, and 4 KB cap enforcement.
2. **FSM tests (`src/app/events.rs`)** — Idle→Running→Exit transitions, Busy branch, heartbeat-triggered aborts.
3. **Integration tests (`tests/bin_smoke.rs`, `tests/integration_mock.rs`)** — rely on the fake serial helpers already in-tree to emulate UART, covering stdout-only, stderr-only, mixed output, checksum mismatch, partial frame reconstruction, and large file streaming.
4. **CLI smoke (`tests/bin_smoke.rs`)** — `lifelinetty --serialsh --device fake0 --demo` returns expected exit codes and respects config overrides.
5. **Resource budget checks** — ensure RSS stays under 5 MB on Raspberry Pi 1 by bounding chunk buffers and using streaming IO.

## Observability & guardrails

- All tunnel logs go to stderr (systemd journal) or `/run/serial_lcd_cache/tunnel/trace.log`.
- `tracing` spans mark each frame, command, and child PID for later troubleshooting.
- Serial backoff metrics (from P5) continue to apply; tunnel commands never bypass the retry policy.
- No networking, PTYs, or additional daemons are introduced.

## Out of scope

- Multi-command pipelines, shell-style redirection, or background jobs.
- Compression (Milestone E) and negotiation (Milestone B) logic beyond ensuring frames coexist.
- Any writes outside RAM disk or `~/.serial_lcd/config.toml`.

## Allowed crates & dependencies

Milestone A stays within the charter-approved set: `std`, `serde`, `serde_json`, `crc32fast`, `hd44780-driver`, `serialport`, optional `tokio`/`tokio-serial` behind the existing `async-serial` feature, `rppal`, `linux-embedded-hal`, `ctrlc`, plus the built-in logging modules. Any other crate (process managers, PTY helpers, async runtimes) would require a separate roadmap + charter update before landing.

Delivering Milestone A with these guardrails keeps the daemon deterministic, memory-efficient, and ready for the negotiation, file-transfer, and heartbeat features that follow.
