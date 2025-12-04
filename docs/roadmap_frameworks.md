# Roadmap frameworks and scaffolding

This file lists the lightweight skeleton modules and supporting framework files that were added to accelerate milestone work. The goal is to have compile-safe, well-tested placeholders that express intent without shipping production complexity.

New app modules

- `src/app/polling.rs` — Poller & PollSnapshot skeleton (Milestone D / P11)
- `src/app/file_transfer.rs` — FileTransferManager skeleton for chunking/resume (Milestone C / P10)
- `src/app/compression.rs` — CompressionCodec plus LZ4/Zstd streaming helpers with a 1 MB decompression cap (Milestone F / P14)
- `src/app/watchdog.rs` — Watchdog for heartbeat expiry checks (P15)
- `src/app/negotiation.rs` — Negotiator, Role and Capabilities for handshake (Milestone B / P9)
- `src/app/telemetry.rs` — Telemetry helper to append small logs into CACHE_DIR (P5)
- `src/app/connection.rs` — Reconnection flow and negotiation handshake driver (Milestone B / P9)
- `src/negotiation.rs` — Shared capability/role definitions consumed by both the connection logic and the config loader.

New config module

- `src/config/profiles.rs` — PollingProfiles skeleton to parse small profile maps (P18)

Purpose

These skeletons are intentionally small but test-covered so downstream work can:

- Add concrete, efficient implementations gradually (e.g., /proc readers, chunk managers).
- Write integration tests once APIs stabilize.
- Avoid churn by following the roadmap guardrails (no writing outside CACHE_DIR, no network sockets).

How to expand

1. Pick a skeleton and implement real behaviour with matching unit + integration tests, starting with Milestone B’s negotiation state machines and fallback wiring.
2. Keep RAM-only cache usage restricted to `CACHE_DIR` (see `src/lib.rs`).
3. Maintain the project's quality bar: `cargo fmt`, `cargo clippy`, `cargo test` on x86_64 and ARMv6.
