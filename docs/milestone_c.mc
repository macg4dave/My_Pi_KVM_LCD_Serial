# 📌 Milestone C — Remote file push/pull (chunked + resumable)

> Roadmap alignment: **P10 Remote file transport** + **P17 Integrity tooling** (see [docs/roadmap.md](./roadmap.md)). Milestone C also depends on Milestone A (command tunnel framing) and Milestone B (role negotiation) so tunneled file packets have a home.

## Snapshot of the current codebase (Dec 2025)

- `src/app/render_loop.rs` only ingests newline JSON LCD frames via `serial::sync::SerialPort` and `state::RenderState`; there is no tunnel message type yet.
- `src/payload/parser.rs` defines `Payload`/`RenderFrame` exclusively for LCD rendering; it must be extended (or wrapped by a future `TunnelMsg`) before file-transfer envelopes can exist.
- `CACHE_DIR` is defined in `src/lib.rs` and already used by `app::Logger` and `serial::telemetry` to keep all runtime writes under `/run/serial_lcd_cache`.
- Tests live in `tests/bin_smoke.rs`, `tests/integration_mock.rs`, and `tests/fake_serial_loop.rs`; none cover transfer behavior yet.

## Target outcome

1. Trusted operators can push OR pull files up to **2 MiB** through the same UART session without exceeding the 5 MB RSS ceiling on Raspberry Pi 1.
2. Transfers stream fixed-size chunks (default 8192 bytes, clamped 4096–16384) and checksum each chunk with `crc32fast` so corruption never touches disk.
3. Resume metadata lives in `/run/serial_lcd_cache/transfers/<transfer_id>/manifest.json`; losing power costs at most one chunk.
4. Receivers never create or mutate files outside `CACHE_DIR` unless a human manually copies the staged artifact to `~/.serial_lcd/`.
5. Retry/backpressure honors the existing `serial::backoff::BackoffController` so LCD rendering (Milestone A traffic, heartbeats, etc.) keeps priority.

## Success criteria

- ✅ Push + pull happy paths validated against fake serial endpoints; SHA256(source) == SHA256(staged `.part`).
- ✅ Resume after a forced drop restarts at the `next_chunk_id` recorded in the manifest.
- ✅ CRC mismatch triggers a localized retry without tearing down the tunnel or LCD loop.
- ✅ Boundary files (0 bytes, integer multiple of `chunk_size`, partial tail, > ceiling) exercise explicit success/failure states.
- ✅ CLI reports actionable status while the feature remains “experimental” in README + docs.

## Implementation plan

### 1. Schema + envelope work (P10, depends on Milestone A)

- Extend `src/payload/parser.rs` (or the future `payload::tunnel` module from Milestone A) with a `TransferCommand` enum that covers `Init`, `Chunk`, `Ack`, `ResumeRequest`, `ResumeStatus`, `Finish`, and `Error` messages.
- Shared fields: `transfer_id` (16-byte lowercase hex generated from the existing PRNG helper or timestamp + `crc32fast` hash—no new crates), `chunk_id: u32`, `offset: u64`, `length: u32`, `crc32: u32`, `path_hint`, flags for push vs pull.
- Add serde tests beside the existing payload tests to enforce upper bounds, string lengths, and checksum serialization.
- Provide example tunneled frames in `samples/payload_examples.json` so contract changes are visible to operators.

### 2. Cache + resume layout (P17 groundwork)

- Stage everything under `/run/serial_lcd_cache/transfers/<transfer_id>/` using the already-defined `CACHE_DIR` constant from `src/lib.rs`.
- Manifest format (one per transfer):
  ```json
  {
    "transfer_id": "550e8400-e29b-41d4-a716-446655440000",
    "path_hint": "logs-2025-12-02.txt",
    "bytes_total": 1048576,
    "next_chunk_id": 8,
    "next_offset": 65536,
    "crc_last_good": "3b9aca00",
    "completed": false
  }
  ```
- Data file: `<transfer_id>/<sanitized_basename>.part` – sanitized via `Path::file_name()` to avoid path traversal.
- Introduce `src/app/file_transfer/manifest.rs` helpers that write manifests atomically (temp file + `rename`) and validate JSON schema on load.
- Clean manifests/logs automatically when `completed == true` and the operator has acknowledged the transfer.

### 3. Transfer engine (new module under `src/app/`)

- Create `src/app/file_transfer/mod.rs` housing a `TransferManager` with sender/receiver state machines.
- Allocate one reusable chunk buffer (`Vec<u8>` sized to config chunk) and borrow `SerialPort` for short bursts so `render_loop` keeps control most of the time.
- Integrate with `serial::backoff::BackoffController` + `serial::telemetry::log_backoff_event` for pacing retries and logging.
- Use a bounded channel between `render_loop` and `TransferManager` so LCD frames and tunnel packets can be multiplexed without deadlocks.
- Encode chunk payloads as base64 within JSON using a tiny in-tree helper (no new crate) so we stay under `state::MAX_FRAME_BYTES` and preserve newline framing.
- Receiver side writes to cache, fsyncs on completion, emits `FileAck { chunk_id }`; sender waits for ACK before advancing.

### 4. Config + CLI gating

- Update `src/config/mod.rs` + `config/loader.rs` with an optional `[file_transfer]` table:
  ```toml
  [file_transfer]
  chunk_size = 8192        # clamp 4096..16384 at load time
  allow_push = false       # defaults to deny until role negotiation says otherwise
  allow_pull = false
  max_concurrent = 1
  ```
- Surface these fields through `AppConfig` so runtime toggles do not require recompilation.
- CLI (`src/cli.rs`): keep `run` as the only public subcommand for now, but add hidden/feature-gated flags (`--push src:hint`, `--pull hint:dest`) for manual testing. Docs must label them **experimental** until Milestone C ships.
- Any local path provided on CLI is resolved to `/run/serial_lcd_cache/transfers/received/<id>/` unless the user opts into a safe override.

### 5. Integrity + tooling hooks (ties to P17)

- Provide read-only helpers (future `lifelinetty transfer --list/--inspect`) that dump manifest state without modifying files.
- Document manual promotion workflow: operators move staged `.part` files into `~/.serial_lcd/` or elsewhere themselves; the daemon never writes there.
- Record SHA256 (optional) in the manifest so P17 tooling can verify without rereading the chunks.

## Storage + resume rules

| Item            | Location                                                      | Notes |
|-----------------|---------------------------------------------------------------|-------|
| Manifest        | `/run/serial_lcd_cache/transfers/<id>/manifest.json`          | Atomically updated JSON, includes role + direction. |
| Chunk data      | `/run/serial_lcd_cache/transfers/<id>/<basename>.part`        | Only grows up to `bytes_total`; fsync + rename when complete. |
| Transfer log    | `/run/serial_lcd_cache/transfers/<id>/transfer.log`           | Mirrors tracing spans (`chunk_id`, retries, CRC failures). |
| Local pull temp | `/run/serial_lcd_cache/transfers/received/<id>/` (Pi)         | On developer machines use OS temp dir (never `/tmp` on Pi). |

Resume handshake:
1. Sender issues `FileResumeRequest { transfer_id }` after reconnect.
2. Receiver reads manifest:
   - Missing/corrupt → respond `FileResumeStatus { restart: true }` and wipe cache dir.
   - Valid → respond with `next_chunk_id`, `next_offset`, `crc_last_good`.
3. Sender seeks to `next_offset`, recomputes CRC for the previous chunk, and continues.

CRC mismatch during resume resets the manifest to the last acknowledged chunk and sets `next_chunk_id` accordingly; no global reconnect needed.

## Testing strategy

- `src/payload/parser.rs`: serde/validation tests covering every `TransferCommand` variant, range checks, and checksum behavior.
- `src/app/file_transfer/manifest.rs`: unit tests ensuring manifests stay under `CACHE_DIR`, sanitize hints, and survive process restarts.
- `src/app/file_transfer/manager.rs`: state-machine tests using temp dirs and stubbed serial traits (fake `Read`/`Write`).
- `tests/integration_mock.rs`: add push/pull happy path, resume, CRC failure, malicious path hints, and >2 MiB rejection using fake serial loops.
- `tests/fake_serial_loop.rs`: once LCD stubs come back online, verify render loop keeps scrolling while transfers run in background.
- `tests/bin_smoke.rs`: CLI mentions experimental flags; ensure `--help` stays accurate.

## Observability, limits, and non-goals

- Reuse `Logger` (writes to stderr or `/run/serial_lcd_cache/*.log`) plus `tracing` spans tagged with `transfer_id` + `chunk_id`.
- Retry pacing goes through the already-reviewed `BackoffController`; no busy loops or uncontrolled sleeps.
- Memory budget: one chunk buffer (<16 KiB) + manifest (<4 KiB) + log writer ensures RSS << 5 MB on Pi1 even during transfers.
- Only one transfer at a time (configurable later). Additional requests yield `FileError::Busy` frames.
- Out of scope for Milestone C: compression (Milestone F / P14), automatic promotion to persistent storage, multi-transfer queues, or any network transport.
- Keep every doc/README mention labeled “experimental” until Milestone C is declared GA, and always reference P10/P17 when committing or documenting related work.

All planned work must respect the charter guardrails: UART-only IO, RAM-disk writes, no new CLI flags unless explicitly gated, and no regressions to LCD rendering throughput.

## Allowed crates & dependencies

No additional dependencies are introduced for Milestone C. Implementation relies on the existing approved set: `std`, `serde`, `serde_json`, `crc32fast`, `hd44780-driver`, `serialport`, optional `tokio`/`tokio-serial` (feature-gated), `rppal`, `linux-embedded-hal`, `ctrlc`, and the built-in logging modules. Base64 helpers, manifest writers, and transfer managers stay in-tree to avoid adding crates such as `uuid`, `bitflags`, or compression libraries.
