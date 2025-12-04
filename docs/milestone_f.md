
# 📌 Milestone F — JSON-Protocol Mode + Payload Compression (P13 + P14 + P10)

*Draft specification for Milestone f of the LifelineTTY project. This file documents design intent only—no code here is executable.*

---

> Scope alignment: Milestone F implements roadmap items **P13 (JSON schema validation)**, **P14 (payload compression)**, and reuses the transport scaffolding from **P10 (file push/pull)** per [docs/roadmap.md](./roadmap.md). Work must keep the daemon HD44780-only, respect `/run/serial_lcd_cache` for all temporary data, and hold decompression buffers under 1 MB.

## Goal

Ship a strict, versioned JSON protocol for every serial payload plus an optional compression envelope (LZ4 first, zstd when approved). The parser upgrades land in `src/payload/parser.rs`, leverage existing `serde` models, and remain backward compatible with current newline JSON frames. Compression is opt-in, negotiated via Milestone B handshake bits, and never compromises the <5 MB RSS guardrail on Raspberry Pi 1.

### Success criteria

-- Frames include a `schema_version` and type discriminator; the parser now requires `schema_version` and rejects frames that do not include it.
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
-- Teach `RenderFrame::from_payload_json_with_defaults` to require `schema_version` and enforce version `1` rules: `line1/line2` length caps, icon counts ≤ 4, bar labels bounded to display width, etc.
- Whitelist the semantic icon names currently recognized by `src/payload/icons.rs` (`battery`, `heart`, `arrow`, `wifi`) so the deserializer can reject typos when `icons` is present.
- Introduce manual bounds helpers (max string length, array len, numeric ranges). Violations map to `Error::Parse(String)` with concise wording so CLI logs remain readable.
- Update `samples/payload_examples.json` and `docs/lcd_patterns.md` with schema-versioned fixtures to help operators craft valid frames.
- Extend unit tests in `src/payload/parser.rs` to cover version parsing, default fallback, bounds enforcement, and checksum validation with the new canonical serialization (checksum excludes the compression envelope when present).

### 2. Compression envelope + codec plumbing (P14)

**Files:** `src/payload/parser.rs`, new `src/app/compression.rs`, `src/serial/*.rs` (for buffer sizing), `docs/architecture.md`, `README.md` (protocol section).

- Add an optional outer envelope such as `{ "type": "compressed", "schema_version": N, "codec": "lz4", "original_len": 1234, "data": "<base64>" }` and let the parser drive the codec selection.
- Implement the new `src/app/compression.rs` helpers using `lz4_flex` and `zstd` so compressed frames can be round-tripped before the CLI/config toggles land. Streaming encoders emit standard LZ4 frames or zstd streams while the shared bean uses a 4 KB read buffer to keep allocations predictable.
- Decompression loops rehydrate the payload in chunks, returning `Error::Parse` when the data would exceed 1 MB so `/run/serial_lcd_cache` stays under the 5 MB RSS guardrail; unknown codecs also surface `Error::Parse` today.
- Export `CompressionCodec` so future readers (handshake state, CLI/config flags) can call into these helpers without duplicating logic, and keep the helpers behind the `app` module until the pending toggles arrive.

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

1. Parser requires `schema_version` and emits versioned frames when the field is present.
2. Compression envelopes round-trip through the daemon when LZ4 is enabled; attempting to send compressed frames to a non-compression peer logs an error and drops the frame without crashing.
3. Decompression buffers are capped at 1 MB and live exclusively inside `/run/serial_lcd_cache` when heap allocations are required.
4. CLI/config toggles exist, default to uncompressed mode, and surface clear errors for unsupported codecs or schema versions.
5. Tests and documentation cover schema versions, compression behavior, failure modes, and operator guidance; markdownlint passes across updated docs.

## Sample frames

```json
// Schema v1 display payload
{"schema_version":1,"line1":"HELLO","line2":"WORLD","icons":["battery"]}

// Schema v1 payload showing multiple icons (wifi + battery)
{"schema_version":1,"line1":"Wi-Fi OK","line2":"Signal","icons":["wifi","battery"]}

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

Schema enforcement and baseline functionality stay within the approved crates: `std`, `serde`, `serde_json`, `crc32fast`, `hd44780-driver`, `serialport`, optional `tokio`/`tokio-serial` via the existing feature, `rppal`, `linux-embedded-hal`, `ctrlc`, and the new compression codecs `lz4_flex` and `zstd` that power `src/app/compression.rs` for P14.

## Out of scope

- Adding new transport protocols (still newline JSON over UART only).
- Introducing async runtimes or network sockets for compression helpers; everything stays in the existing sync render loop.
- Introducing codecs beyond LZ4/Zstd remains out of scope until the roadmap explicitly approves additional options.
- Persisting compressed artifacts outside `/run/serial_lcd_cache`; the only persistent file remains `~/.serial_lcd/config.toml`.

Delivering Milestone F with these constraints keeps the daemon debuggable, resource-safe, and ready for the heavier payloads planned in the file-transfer and tunnel milestones.
