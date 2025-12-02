
# 📌 Milestone F — JSON-Protocol Mode + Payload Compression (P13 + P14 + P10)

*Draft specification for Milestone f of the LifelineTTY project. This file documents design intent only—no code here is executable.*

---

> Scope alignment: Milestone F implements roadmap items **P13 (JSON schema validation)**, **P14 (payload compression)**, and reuses the transport scaffolding from **P10 (file push/pull)** per [docs/roadmap.md](./roadmap.md). Work must keep the daemon HD44780-only, respect `/run/serial_lcd_cache` for all temporary data, and hold decompression buffers under 1 MB.

## Goal

Ship a strict, versioned JSON protocol for every serial payload plus an optional compression envelope (LZ4 first, zstd when approved). The parser upgrades land in `src/payload/parser.rs`, leverage existing `serde` models, and remain backward compatible with current newline JSON frames. Compression is opt-in, negotiated via Milestone B handshake bits, and never compromises the <5 MB RSS guardrail on Raspberry Pi 1.

### Success criteria

- Frames include a `schema_version` and type discriminator; legacy payloads without the field still parse via compatibility paths.
- The parser enforces length and enum bounds (bars, icons, file chunk sizes, metrics arrays) and returns structured `Error::Parse` variants when validation fails.
- Optional compression envelopes (initially `codec = "lz4"`) encapsulate inner JSON payloads without exceeding a 1 MB post-decompression limit; malformed envelopes log to `/run/serial_lcd_cache/protocol_errors.log` and do not crash the daemon.
- CLI + config switches allow operators to request compression; when remote peers lack the capability the runtime auto-falls back to plain JSON and emits a warning.
- Unit and integration tests cover uncompressed and compressed paths using the existing fake serial loops and stub LCD backends; cargo fmt/clippy/test continue to pass on x86_64 + ARMv6.

## Current architecture snapshot

- `src/payload/parser.rs` converts newline JSON into `RenderFrame` structs (LCD focus) and performs light validation (bars, icons). No schema versioning or compression support exists today.
- `src/state.rs` deduplicates frames via CRC but assumes each frame fits inside the 512-byte raw limit (MAX_FRAME_BYTES) and is uncompressed.
- `src/app/render_loop.rs` reads raw lines from `SerialPort::read_message_line`, feeds them to `RenderState`, and logs parse errors before showing offline overlays.
- `src/cli.rs` exposes only the core flags (`--device`, `--baud`, etc.); there is no way to request compression from the CLI or config.
- `tests/integration_mock.rs` and `tests/fake_serial_loop.rs` assert LCD rendering behavior using plain JSON fixtures.

Milestone F keeps the current framing (newline-delimited JSON) but wraps it in a formal schema + optional envelope so downstream milestones (file transfer, tunnel, telemetry) can reuse the same parser.

## Workstreams

### 1. Schema + validation upgrades (P13)

**Files:** `src/payload/parser.rs`, `src/payload/mod.rs`, new `src/payload/schema.rs` (if needed), `docs/architecture.md`, `docs/roadmap.md` (status annotations), `samples/*.json`.

- Define a `ProtocolFrame` enum with variants for display frames (current `RenderFrame`), file chunk manifests (P10), control messages, and the new compression envelope. Embed a `schema_version: u8` and `frame_type` string in the serialized form.
- Teach `RenderFrame::from_payload_json_with_defaults` to detect missing `schema_version` and treat it as `0` (legacy). Version `1` enforces new rules: `line1/line2` length caps, icon counts ≤ 4, bar labels bounded to display width, etc.
- Introduce manual bounds helpers (max string length, array len, numeric ranges). Violations map to `Error::Parse(String)` with concise wording so CLI logs remain readable.
- Update `samples/payload_examples.json` and `docs/lcd_patterns.md` with schema-versioned fixtures to help operators craft valid frames.
- Extend unit tests in `src/payload/parser.rs` to cover version parsing, default fallback, bounds enforcement, and checksum validation with the new canonical serialization (checksum excludes the compression envelope when present).

### 2. Compression envelope + codec plumbing (P14)

**Files:** `src/payload/parser.rs`, new `src/payload/compression.rs`, `src/serial/*.rs` (for buffer sizing), `docs/architecture.md`, `README.md` (protocol section).

- Add an optional outer envelope: `{ "type": "compressed", "schema_version": N, "codec": "lz4", "original_len": 1234, "data": "<base64>" }`.
- Prefer a pure Rust codec such as `lz4_flex` for the initial implementation. Adding this crate (or any other codec) requires an explicit charter update before landing; until then the compression path stays behind a feature flag with stubbed hooks. Decompression must enforce:
  - `original_len ≤ 1_048_576` bytes (hard limit).
  - `data.len()` sanity checks to avoid allocating huge buffers for clearly invalid frames.
- Store temporary decompression buffers inside `/run/serial_lcd_cache/payload_cache/` when they cannot fit on the stack. Clean up after each frame to honor the RAM-disk policy.
- Provide helper APIs so callers can ask `ProtocolFrame::into_render_frame()` without re-running decompression when not needed.
- Reject unknown codecs or mismatched `original_len` values with `Error::Parse("unsupported codec")` and log the event (see Workstream 5).

### 3. Capability negotiation hooks (ties into Milestone B)

**Files:** `src/app/connection.rs`, `src/app/lifecycle.rs`, `src/app/events.rs`, `docs/milestone_b.md` (if cross-referenced).

- Define compression-related capability bits (e.g., `COMPRESS_LZ4`, `COMPRESS_ZSTD`, `SCHEMA_V1`). These live alongside the handshake scaffolding landing in Milestone B; for now add stubs that default to “compression disabled” so Milestone F can be merged ahead of the full handshake.
- When both peers advertise a common codec, set a runtime flag on `AppConfig` indicating compressed frames are allowed. Otherwise run in legacy mode and keep emitting warnings if the user forced compression via CLI/config.
- Ensure handshake failures never block the LCD render loop: negotiation happens before the render loop starts writing frames, with timeouts falling back to uncompressed mode.

### 4. CLI + config controls

**Files:** `src/cli.rs`, `src/config/{mod.rs,loader.rs}`, `README.md` CLI table, `tests/bin_smoke.rs`.

- Add optional `--compressed` (boolean) and `--codec <lz4|zstd>` flags to `lifelinetty run`. Flags can only be parsed after existing ones; maintain compatibility with implicit `run` mode.
- Mirror these settings in `~/.serial_lcd/config.toml` under a `[protocol]` table:

  ```toml
  [protocol]
  schema_version = 1
  compression = { enabled = false, codec = "lz4" }
  ```

- Config validation ensures the codec value is recognized and refuses to save invalid options.
- Smoke tests (`tests/bin_smoke.rs`) cover `--compressed`/`--codec` combinations, verifying that unsupported codecs trigger user-friendly errors without starting the daemon.

### 5. Logging, limits, and integration tests (P10 alignment)

**Files:** `src/app/logger.rs`, `src/serial/telemetry.rs`, `tests/{integration_mock.rs,fake_serial_loop.rs}`, `docs/releasing.md` (release checklist).

- Create `/run/serial_lcd_cache/protocol_errors.log` (rotate ≤256 KB) for parser/compression failures. The existing `Logger` already writes inside the cache dir; extend it with a helper for protocol errors so field ops can inspect bad payloads.
- For large payload types (file chunks from Milestone C/P10), add guards ensuring chunk metadata + compressed data stay within negotiated limits before writing to disk.
- Integration tests feed compressed and plain fixtures through `tests/fake_serial_loop.rs` to ensure `RenderState` dedupe logic still works when the same logical payload arrives via different envelope forms.
- Document the operational flow in `docs/releasing.md` so release builds capture the negotiated schema/compression state in their QA checklist.

## Acceptance checklist

1. Parser accepts legacy payloads (no `schema_version`) and emits versioned frames when the field is present.
2. Compression envelopes round-trip through the daemon when LZ4 is enabled; attempting to send compressed frames to a non-compression peer logs an error and drops the frame without crashing.
3. Decompression buffers are capped at 1 MB and live exclusively inside `/run/serial_lcd_cache` when heap allocations are required.
4. CLI/config toggles exist, default to uncompressed mode, and surface clear errors for unsupported codecs or schema versions.
5. Tests and documentation cover schema versions, compression behavior, failure modes, and operator guidance; markdownlint passes across updated docs.

## Sample frames

```json
// Legacy display payload (treated as schema_version 0)
{"line1":"HELLO","line2":"WORLD","icons":["battery"]}

// Compressed LZ4 envelope carrying a schema v1 display payload
{
  "type":"compressed",
  "schema_version":1,
  "codec":"lz4",
  "original_len":128,
  "data":"BASE64-LZ4-BYTES"
}
```

## Test & rollout plan

- Unit tests: extend `src/payload/parser.rs` to cover schema versions, bounds, and compression decode errors; add codec-specific tests under a new `payload::compression` module.
- Integration tests: enhance `tests/integration_mock.rs` and `tests/fake_serial_loop.rs` so compressed fixtures reach the LCD stub without panics; verify duplicate suppression works when the same logical payload alternates between compressed/uncompressed forms.
- Fuzz/boundary tests: optionally leverage `proptest` (behind a dev-only feature) to stress the parser with truncated JSON, bogus base64, and oversize `original_len` claims.
- CLI smoke: update `tests/bin_smoke.rs` to exercise `lifelinetty --run --compressed --codec lz4 --demo` and confirm help text documents the new flags.
- Release checklist: document how to toggle compression in `README.md` and `docs/architecture.md`, and capture negotiation logs during QA runs on Raspberry Pi 1 hardware.

## Allowed crates & dependencies

Schema enforcement and baseline functionality stay within the approved crates: `std`, `serde`, `serde_json`, `crc32fast`, `hd44780-driver`, `serialport`, optional `tokio`/`tokio-serial` via the existing feature, `rppal`, `linux-embedded-hal`, and `ctrlc`. Compression codecs such as `lz4_flex` or `zstd` are **not** currently whitelisted; introducing them requires a signed-off charter update plus dependency review before code lands.

## Out of scope

- Adding new transport protocols (still newline JSON over UART only).
- Introducing async runtimes or network sockets for compression helpers; everything stays in the existing sync render loop.
- Implementing non-whitelisted codecs until the charter explicitly allows them (start with LZ4; zstd arrives only after approval).
- Persisting compressed artifacts outside `/run/serial_lcd_cache`; the only persistent file remains `~/.serial_lcd/config.toml`.

Delivering Milestone F with these constraints keeps the daemon debuggable, resource-safe, and ready for the heavier payloads planned in the file-transfer and tunnel milestones.
