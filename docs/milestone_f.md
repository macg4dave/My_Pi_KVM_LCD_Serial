
*Draft specification for Milestone e of the LifelineTTY project. This file documents design intent only—no code here is executable.*
---

---

## 📌 Milestone F — JSON Protocol Mode + Payload Compression

**Goal**
Introduce a **strict, versioned JSON protocol** for all tunnel payloads, with **optional LZ4/zstd compression** for high-volume traffic (logs, file chunks, metrics bursts), while staying:

* JSON-first (no opaque binary protocol)
* Backwards compatible with uncompressed endpoints
* Safe with hard memory caps and robust validation

---

## 🎯 High-Level Design

This milestone adds three key things:

1. A **versioned JSON schema** with explicit validation rules.
2. A **compression envelope** that can wrap any existing framed payload.
3. Capability/negotiation bits (from Milestone B) so both sides agree on whether compression is allowed and which codec to use.

All of it is implemented in pure Rust, using `serde`/`serde_json` and pure-Rust compression crates.

---

## 1. JSON Schema Versions & Validation (Parser Layer)

Everything lives conceptually in `src/payload/parser.rs` and neighbouring schema modules.

Design:

* Introduce a `schema_version` concept:

  * For example: “v1 = current single-payload layout, v2 = adds compression envelope, new message families, etc.”
  * Parser must know what version it expects to see and what it is allowed to emit.

* Define a **top-level protocol object** (conceptually) with:

  * A version field (e.g. an integer or string like `"v1"`, `"v2"`).
  * A type-discriminant field (e.g. `"type": "file_chunk"`, `"type": "metrics"`, `"type": "display"`, `"type": "control"`, `"type": "compressed"`).
  * A payload field whose structure depends on the type and version.

* Validation helpers:

  * After `serde` deserialization, run **manual bounds checks**:

    * Deny payloads with absurd sizes (e.g. file chunk bytes length > max chunk, metrics arrays too large, string fields exceeding configured limits).
    * Reject unknown or mismatched enum values in a controlled way.
  * On any violation:

    * Return a structured parser error.
    * Do not crash, do not panic.

* Backwards compatibility:

  * If no `schema_version` field is present, treat it as “legacy v0” and map into the v1 model where possible.
  * This allows older tools to still talk to a newer endpoint in a reduced capability mode.

---

## 2. Compression Envelope Design

This is a **pure JSON wrapper** that sits *around* your existing payload types, not inside each specific message schema.

Concept:

* The “outer frame” (which is already a single JSON object per newline from Milestone A) can either be:

  * A normal payload (uncompressed), or
  * A **compression envelope** that says: *“inside here is another payload, but compressed”*.

* The compression envelope conceptually includes:

  * A boolean or explicit marker indicating compression is in use.
  * The codec name (e.g. `"lz4"`, `"zstd"`).
  * The original schema version of the inner payload.
  * The compressed data, encoded in a JSON-safe way (e.g. base64, or a binary-friendly representation if you later move to CBOR).

* Decode order:

  1. Parse outer JSON.
  2. If not compressed → validate and dispatch as normal.
  3. If compressed:

     * Check codec field against negotiated capabilities.
     * Base64 decode (or similar) to get compressed bytes.
     * Enforce the **<1 MB decompression buffer limit** before allocating.
     * Decompress using the chosen Rust codec.
     * Parse the inner JSON payload from the decompressed bytes.
     * Run standard schema version checks and manual validation.

* Encoding order is the reverse:

  1. Serialize the inner payload to JSON text.
  2. Optionally compress it.
  3. Wrap it in the envelope with codec metadata.
  4. Serialize the outer JSON as the actual frame.

This keeps the core protocol logical and debuggable: you can always see when something is compressed and what’s inside, rather than having random opaque blobs on the wire.

---

## 3. Negotiation Bits for Compression (Integration with Milestone B)

Milestone B introduced capabilities and handshake messages. Milestone F adds new **capability bits and negotiation logic** for compression.

Concept:

* Add capability flags like:

  * “supports compressed envelopes”
  * “supports LZ4”
  * “supports zstd”
  * (and optionally, “supports schema v2+”)

* During handshake:

  * Each endpoint advertises:

    * Supported schema versions.
    * Supported compression codecs.
  * The negotiation logic:

    * Chooses the **highest common schema version**.
    * Chooses a **single codec** for compressed mode (or none if no common codec).
  * Once negotiated:

    * Both sides know whether:

      * Uncompressed-only mode must be used.
      * Optional compression can be used.
      * Which codec to default to if compression is requested.

* Protocol guarantee:

  * No side is allowed to send compressed payloads unless the other side explicitly advertised and accepted that codec.
  * If a compressed envelope appears on a peer that doesn’t support it, it is treated as a protocol violation (soft failure, logged, connection continues if possible).

---

## 4. Buffer Limits, Safety, and Malformed Packets

Critical safety constraint: **no decompression buffer may exceed 1 MB**.

Design:

* Before decompressing:

  * Look at the compressed size and the envelope metadata.
  * If the compressed size already suggests an unreasonably large decompressed payload (e.g. multiple MB for a simple log chunk), reject it outright.
  * Maintain a hard upper bound (e.g. 1 MB) on the decompressed payload size:

    * If decompressor indicates the output would exceed that, terminate decompression and flag an error.

* Malformed packets:

  * Any failure in:

    * JSON parsing
    * Envelope semantics (missing fields, unknown codec)
    * Base64 decoding
    * Decompression
    * Inner JSON parsing
    * Manual validation
  * Must result in:

    * A structured error that includes:

      * Type of failure (parse, codec, bounds)
      * Possibly a short context string (not the full payload, to avoid log spam).
    * Logging to RAM disk (e.g. under `/run/serial_lcd_cache/protocol_errors.log` or similar).
  * The connection stays alive unless failures are continuous and you choose to implement a maximum-error threshold.

* Importantly:

  * The parser must not panic on junk.
  * It should treat untrusted input as untrusted, especially if the serial link might be exposed or bridged.

---

## 5. CLI Mode & Configuration

CLI gets a **compression-aware** mode:

* New CLI flags:

  * A global flag like `--compressed` to request compressed mode.
  * An optional codec selector like `--codec lz4` or `--codec zstd`.
* Behaviour:

  * On startup, if compression is requested:

    * The local endpoint sets its negotiation preferences to include the chosen codec.
  * If the remote peer does not agree on compression:

    * Local side logs that compression is disabled.
    * Falls back to pure JSON mode.
* Config file integration:

  * `config.toml` can include:

    * Default schema version (for sending).
    * Default compression preference.
    * Per-payload-type compression policies (e.g. compress logs and file chunks, but never compress small control messages).
* Default behaviour:

  * Compression is **off by default** unless explicitly enabled via config or CLI, to keep things predictable and simplify debugging.

---

## 6. Test Strategy

You want both **unit tests** and **integration-style tests** around the parser and compression layer.

Test coverage:

1. Plain JSON round-trip:

   * Serialize typical payloads (metrics, file chunks, display commands).
   * Deserialize and verify they match.
   * Confirm they conform to size and bounds limits.

2. Compressed JSON round-trip:

   * Build fixtures where the inner payload is logs or file chunks.
   * Encode using the compression envelope (for each codec).
   * Ensure the parser:

     * Recognizes compression.
     * Decompresses.
     * Validates inner payload correctly.

3. Boundary & fuzz-like tests:

   * Payloads right at size limits.
   * Payloads that claim to be compressed but are not.
   * Truncated base64 data.
   * Random garbage where JSON should be.
   * Very deeply nested JSON (if you permit it at all) → verify it is rejected if it exceeds configured structural depth.

4. Negotiation behaviour:

   * One side supports LZ4 only, other supports zstd only → expect no compression usage.
   * Both support LZ4 → envelope with LZ4 is accepted.
   * Compression requested on CLI but peer doesn’t support it → documented fallback to uncompressed mode.

5. Logging:

   * Verify that malformed packets:

     * Do not crash parser.
     * Are logged to RAM disk.
     * Do not exceed log file quotas (you may choose to cap error log size or rotate).

---

## 7. Crates & Tooling (All-Rust)

* `serde` / `serde_json`

  * Already in use for the core message schemas.

* Candidate compression crates (pure Rust, or safe wrappers):

  * `lz4_flex` (pure Rust LZ4 implementation).
  * `zstd-safe` or similar (safe wrappers; depending on your “no native deps” stance you may or may not allow this).
  * You can start with one codec (LZ4) to keep it simple.

* `anyhow` or your existing error type stack:

  * Useful for wrapping codec/framing/parsing errors with context.

* Testing tools:

  * Use existing Rust test framework (`#[test]`), maybe `proptest` or similar if you later want true fuzz-style coverage.

Everything stays in the Rust ecosystem. No C libs, no external tools required.

---

## 🏁 What Milestone F Actually Delivers

By the end of this milestone, LifelineTTY will have:

* A **versioned JSON protocol** with strict validation.
* An **optional, negotiated compression layer** that can wrap any existing payload.
* Hard limits on decompression buffers and clean handling of malformed/hostile input.
* CLI and configuration knobs to control whether compression is used and which codec is preferred.
* A test suite that proves both compressed and uncompressed paths are safe, bounded, and backward compatible.

All of it is **100% Rust**, schema-driven, and matches the architecture you’ve already laid down in A–E.
