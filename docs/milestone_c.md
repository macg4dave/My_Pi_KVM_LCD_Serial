# 📌 Milestone C — Remote file push/pull (framed, chunked, resumable)

*Draft specification for Milestone C of the LifelineTTY project. This file documents design intent only—no code here is executable.*

## Outcome

Provide a reliable UART-based file transfer channel so operators can push logs/configs to the remote host or pull diagnostics back without leaving the serial link. Transfers are chunked, CRC-protected, resumable after drops, and respect the RAM-disk storage mandate. This milestone implements roadmap items **P10 (file transfer transport)** and **P17 (integrity tooling)**, building on milestones A (tunnel) and B (negotiation) and enabling future compression work (Milestone E / P14).

## Success criteria

- Both push and pull flows move files of at least 4 MiB within UART limits while keeping RSS <5 MB.
- Each chunk carries CRC32 metadata; receivers reject corrupt data and request retransmission without tearing down the whole session.
- Transfers resume automatically using manifests stored under `/run/serial_lcd_cache/transfers/` when serial links drop mid-file.
- No process ever writes outside the RAM disk unless a user manually promotes a completed transfer into `~/.serial_lcd/`.
- CLI tooling documents experimental `--push`/`--pull` flags (gated) and integration tests cover happy-path, resume, and corruption cases.

## Dependencies & prerequisites

1. Milestone A provides the framed transport and async multiplex loop.
2. Milestone B negotiation ensures only the server role accepts inbound pushes to avoid conflicts.
3. Config loader hardening (P3) validates new `[file_transfer]` options.
4. Serial backoff telemetry (P5) remains active so transfers respect reconnect timers.

## Architecture overview

- **File transfer module (`src/app/file_transfer.rs`)** — optional new module (or `events.rs` extension) housing sender/receiver state machines and manifests.
- **Payload schema (`src/payload/schema.rs`)** — adds `TunnelMsg::File(FileMsg)` plus helper structs (`ChunkMeta`, `TransferManifest`).
- **Cache layout** — all temporary objects live under `/run/serial_lcd_cache/transfers/<transfer_id>/`, including `.part` files, manifests, and trace logs.
- **CLI entry points (`src/cli.rs`)** — gated `--push`/`--pull` commands (behind `file-transfer` Cargo feature or config flag) that initiate sender workflows using the existing serial transport.
- **Tests (`tests/integration_mock.rs`)** — comprehensive suite using `tokio::io::duplex` plus temp directories to mimic RAM-disk constraints.

## Protocol additions

```rust
pub type ChunkId = u32;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FileMsg {
  FileInit { transfer_id: TransferId, path_hint: String, size_bytes: u64,
         compressed: bool, direction: FileDirection },
  FileChunk { transfer_id: TransferId, chunk_id: ChunkId, offset: u64,
         data: Vec<u8>, crc32: u32, is_last: bool },
  FileAck { transfer_id: TransferId, chunk_id: ChunkId, ok: bool, crc32: u32 },
  FileComplete { transfer_id: TransferId, success: bool },
  FileResumeRequest { transfer_id: TransferId },
  FileResumeStatus { transfer_id: TransferId, next_chunk_id: ChunkId, next_offset: u64 },
  FileError { transfer_id: TransferId, message: String },
}

#[derive(Serialize, Deserialize, Copy, Clone)]
pub enum FileDirection {
  PushToRemote,
  PullFromRemote,
}
```

- New outer frames reuse Milestone A’s newline JSON + CRC envelope. Additional `FileMsg` CRCs guard per-chunk payloads.
- Chunk sizes are capped at 4–16 KiB (configurable) to keep buffers small on ARMv6 hardware.

## Sender & receiver workflows

### Sender

1. CLI opens the source path (local machine) read-only; for pull, the sender is the server side reading from RAM disk.
2. Build `FileInit` with `transfer_id` (u64 counter or random), total size, and path hint (basename only).
3. Wait for optional `FileResumeStatus`; seek to requested offset if resuming.
4. Loop:
   - Read up to `chunk_size` bytes, compute CRC32 (`crc32fast`), emit `FileChunk` and move to `AwaitingAck`.
   - Retry chunk (bounded exponential backoff) on timeout or negative ACK; abort after N failures and emit `FileError`.
5. After last chunk, send `FileComplete { success }` based on final acknowledgments.
### Receiver

1. On `FileInit`, verify role (only server accepts pushes by default), create `/run/serial_lcd_cache/transfers/<transfer_id>/` and `.part` file, then respond with `FileResumeStatus` derived from any existing manifest.
2. For each `FileChunk`, validate transfer/chunk IDs, offsets, and CRC before writing with `tokio::fs::File` + `seek`/`write_all`.
3. Update and fsync `TransferManifest` after each acknowledged chunk so resume data survives crashes.
4. Emit `FileAck { ok, crc32 }` once the data is durable; send `ok=false` for CRC mismatch or unexpected offsets.
5. On `FileComplete`, flag the manifest as `completed = true` but leave promotion to the operator.

## Resume strategy

- Resume data lives in JSON manifests per transfer, containing `next_chunk_id`, `next_offset`, and checksum of the last good chunk.
- When a sender reconnects, it first issues `FileResumeRequest` to learn the next chunk/offset; if manifest is missing or corrupted, receiver instructs sender to restart from offset 0.
- Optional CLI helper `lifelinetty transfer --list` (future P17) inspects manifests safely before promotion.

## Configuration & CLI

```toml
[file_transfer]
chunk_size = 8192            # bytes, validated within 4096..16384
max_concurrent = 1           # only one transfer at a time for now
allow_push = true            # gated per role
allow_pull = true
```

- CLI flags (behind `--enable-file-transfer` or Cargo feature) follow `lifelinetty --push src:dst_hint` and `lifelinetty --pull remote_hint:local_path`. Documentation must stress RAM-disk constraints and experimental status.
- All local destination paths default to `/run/serial_lcd_cache/transfers/received/` unless an explicit safe override is provided.

## Testing & validation

1. **Push happy path** — compare SHA256 of source vs `.part` file after completion; manifest shows `completed=true`.
2. **Pull happy path** — server reads from RAM disk, client receives into local temp dir.
3. **Resume** — kill sender mid-transfer, restart, and ensure transfer completes without re-sending finished chunks.
4. **CRC failure** — corrupt a chunk intentionally; receiver must NACK and sender retries same chunk only.
5. **Boundary cases** — zero-byte file, exact multiples of chunk size, final partial chunk, >4 MiB file.
6. **Constraint enforcement** — tests assert receiver never escapes the RAM-disk sandbox even if malicious `path_hint` includes `../` sequences.

All tests live in `tests/integration_mock.rs` (new module) plus focused unit tests in `src/payload/schema.rs` for serialization.

## Guardrails & observability

- `tracing` spans annotate transfer IDs, chunk IDs, and offsets; logs live under `/run/serial_lcd_cache/transfers/<id>/transfer.log` and rotate on completion.
- Serial bandwidth throttling uses the same pacing as the command tunnel to avoid starving LCD updates.
- Only one transfer executes at a time until later milestones introduce multiplexing; queue additional requests with clear Busy responses.
- Promotion of completed files into `~/.serial_lcd/` is deliberate and manual; Milestone C does not automate it.
 
### Sender (Push)
*** End Patch

Resume is keyed off **(`transfer_id`, `chunk_id`, checksum)**:

* On startup or on new `FileInit`:

  * Look for manifest in `/run/serial_lcd_cache/transfers/<transfer_id>.json`.
  * If found and `completed == false`, use `next_chunk_id` / `next_offset` to produce `FileResumeStatus`.

* The sender, on seeing `FileResumeStatus`, seeks to `next_offset` and continues sending from `next_chunk_id`.

If CRC mismatch is detected for a chunk during resume, receiver can:

* reset to last known good `offset`;
* set `next_chunk_id` accordingly;
* ask sender to resend from there.

This keeps things robust on unreliable serial.

---

*** End Patch

> “never write outside RAM disk except when user explicitly moves file into `~/.serial_lcd/` config path”

Enforce this **in code**, not just docs:

* Receiver:

  * Only ever opens files under `/run/serial_lcd_cache/transfers/`.
  * If a `FileInit` tries to give you a path pointing elsewhere, just treat it as a *hint*, not a real path.

* Promotion to `~/.serial_lcd/`:

  * Only done via a **separate local operation** (e.g. `lifelinetty promote <transfer_id> <name>`), not part of the serial protocol.
  * That promotion is just a `rename`/`copy` initiated by the local user.

No remote action can directly write into `~/.serial_lcd/`.

---

## 🧪 Tests — `tests/integration_mock.rs`

Use a fake serial pair (e.g. `tokio::io::duplex`) and real temp dirs styled like `/run/serial_lcd_cache`.

You want tests for:

1. **Simple push**: small file, push from A → B

   * Check `*.part` file content matches
   * Check manifest is marked completed

2. **Simple pull**: B requests file from A

   * Mirror logic of push but reversed roles

3. **Interrupted transfer + resume**:

   * Kill sender mid-file, keep receiver manifest
   * Recreate sender, call `--push` again with same `transfer_id`
   * Confirm we continue from `next_offset` and file ends up correct

4. **CRC failure**:

   * Corrupt one chunk in transit (simulate dirty buffer)
   * Ensure receiver detects mismatch and NACKs, sender resends

5. **Boundary conditions**:

   * 0-byte file

   * file exactly `n * chunk_size`

   * file slightly larger, last partial chunk

6. **Constraint tests**:

   * Prove receiver never writes outside the fake `/run/serial_lcd_cache` root.

---

## 🧨 Crates & Tooling Recap

* `tokio` — async read/write, timers
* `tokio-serial` — serial backend
* `tokio::fs` — async file I/O
* `bytes` — chunk buffers
* `crc32fast` — per-chunk CRC
* `serde` — `FileMsg`, `ChunkMeta`, `TransferManifest`
* `thiserror` / `anyhow` — `FileTransferError` with layered context
* `tracing` — debug logs for chunk IDs, resume points, etc.

---

```

These get wrapped inside your existing framed transport (`TunnelMsg::File(FileMsg)` or similar) with CRC at the outer frame level as in Milestone A.

---

## 🧱 Chunk Metadata Struct (Workflow 1)

Instead of ad-hoc integers everywhere, define a reusable metadata struct to reuse in sender/receiver logic:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMeta {
    pub transfer_id: TransferId,
    pub chunk_id:    ChunkId,
    pub offset:      u64,
    pub size:        u32,
    pub crc32:       u32,
    pub is_last:     bool,
}
```

`FileChunk` then mirrors `ChunkMeta` plus the actual `data`.

Chunk size: pick something sane like **4–16 KiB**. Hard limit it; don’t let the client send arbitrary giant blobs per frame.

---

## 🧠 Sender / Receiver State Machines (Workflow 2)

### Sender (Push)

Lives on the side that has the source file.

State struct:

```rust
pub struct FileSender {
    state: SenderState,
    file: tokio::fs::File,
    meta: TransferMeta,
}

pub struct TransferMeta {
    pub transfer_id: TransferId,
    pub path_hint: String,
    pub total_size: u64,
    pub chunk_size: u32,
}
```

`SenderState`:

```rust
enum SenderState {
    Init,
    Sending { next_chunk_id: ChunkId, next_offset: u64 },
    AwaitingAck { chunk_id: ChunkId, offset: u64 },
    Completed,
    Error(String),
}
```

Behaviour:

1. On `--push`:

   * open file from local FS (but note: **never write to disk outside `/run/serial_lcd_cache` on the receiving side**).
   * send `FileInit`.
   * wait for `FileResumeStatus` (optional; if receiver has manifest).
   * seek to `next_offset` and start sending `FileChunk` frames.

2. For each chunk:

   * read up to `chunk_size` into buffer
   * compute CRC32 (`crc32fast`)
   * send `FileChunk`
   * move to `AwaitingAck`

3. On `FileAck { ok: true }`:

   * advance to next chunk
   * if EOF → send `FileComplete { success: true }`

4. On `FileAck { ok: false }` or timeout:

   * either resend chunk or fail with `FileError` depending on policy.

Backpressure / ser2net-style discipline: don’t fire off chunks faster than the other side acks them. One outstanding chunk at a time is simplest.

### Receiver (Pull / Push Target)

Receiver state:

```rust
pub struct FileReceiver {
    state: ReceiverState,
    file: tokio::fs::File,
    manifest: TransferManifest,
}

enum ReceiverState {
    Idle,
    Receiving { transfer_id: TransferId },
    Completed,
    Error(String),
}

#[derive(Serialize, Deserialize)]
pub struct TransferManifest {
    pub transfer_id: TransferId,
    pub path_hint:   String,
    pub size_bytes:  u64,
    pub next_chunk_id: ChunkId,
    pub next_offset: u64,
    pub completed:   bool,
}
```

Location:

* All actual file writes go to **`/run/serial_lcd_cache/transfers/<transfer_id>.part`**.
* Manifest goes to **`/run/serial_lcd_cache/transfers/<transfer_id>.json`**.

Nothing gets automatically moved into `~/.serial_lcd/` — user has to explicitly promote it (CLI or command later).

Receiver behaviour:

1. On `FileInit`:

   * create/open part file in `/run/serial_lcd_cache/transfers/`
   * create/update manifest
   * reply with `FileResumeStatus` (use manifest to decide offset)

2. On `FileChunk`:

   * validate chunk CRC
   * check `transfer_id` matches current receiver transfer
   * check `chunk_id` and `offset` match manifest expectations
   * write data at `offset` using `tokio::fs::File` and `seek`/`write_all`
   * update manifest (`next_chunk_id`, `next_offset`)
   * flush manifest to disk
   * send `FileAck { ok: true, crc32 }`

3. On mismatch/CRC fail:

   * send `FileAck { ok: false }` or `FileError`
   * optionally keep manifest for resume later

4. On `FileComplete`:

   * mark `manifest.completed = true`
   * **do not move** file out of cache here; that’s user’s job.

---

*** End Patch

In your CLI layer (whatever you’re using: `clap` or hand-rolled), add **documented but gated** commands:

* `lifelinetty --push /path/to/local.log:/remote/hint.log`
* `lifelinetty --pull /remote/hint.log:/run/serial_lcd_cache/grabbed.log`

At this milestone:

* The commands are **visible but marked experimental** or require `--enable-file-transfer` or Cargo feature flag.
* Under the hood they:

  * connect to your serial backend
  * ensure Milestones A/B negotiation has happened
  * create a `FileSender` or `FileReceiver` instance and drive the state machine until completion.

---

## 🔐 Constraints Enforcement

> “never write outside RAM disk except when user explicitly moves file into `~/.serial_lcd/` config path”

Enforce this **in code**, not just docs:

* Receiver:

  * Only ever opens files under `/run/serial_lcd_cache/transfers/`.
  * If a `FileInit` tries to give you a path pointing elsewhere, just treat it as a *hint*, not a real path.

* Promotion to `~/.serial_lcd/`:

  * Only done via a **separate local operation** (e.g. `lifelinetty promote <transfer_id> <name>`), not part of the serial protocol.
  * That promotion is just a `rename`/`copy` initiated by the local user.

No remote action can directly write into `~/.serial_lcd/`.

---

## 🧪 Tests — `tests/integration_mock.rs`

Use a fake serial pair (e.g. `tokio::io::duplex`) and real temp dirs styled like `/run/serial_lcd_cache`.

You want tests for:

1. **Simple push**: small file, push from A → B

   * Check `*.part` file content matches
   * Check manifest is marked completed

2. **Simple pull**: B requests file from A

   * Mirror logic of push but reversed roles

3. **Interrupted transfer + resume**:

   * Kill sender mid-file, keep receiver manifest
   * Recreate sender, call `--push` again with same `transfer_id`
   * Confirm we continue from `next_offset` and file ends up correct

4. **CRC failure**:

   * Corrupt one chunk in transit (simulate dirty buffer)
   * Ensure receiver detects mismatch and NACKs, sender resends

5. **Boundary conditions**:

   * 0-byte file
   * file exactly `n * chunk_size`
   * file slightly larger, last partial chunk

6. **Constraint tests**:

   * Prove receiver never writes outside the fake `/run/serial_lcd_cache` root.

---

## 🧨 Crates & Tooling Recap

* `tokio` — async read/write, timers
* `tokio-serial` — serial backend
* `tokio::fs` — async file I/O
* `bytes` — chunk buffers
* `crc32fast` — per-chunk CRC
* `serde` — `FileMsg`, `ChunkMeta`, `TransferManifest`
* `thiserror` / `anyhow` — `FileTransferError` with layered context
* `tracing` — debug logs for chunk IDs, resume points, etc.

---
