# LifelineTTY crate inventory

This document summarizes the crates currently listed in `Cargo.toml`, how we use them inside the daemon/client, and which roadmap items they support. Use it as a quick reference when planning future changes or reviewing dependency decisions.

| Crate | Purpose | In-app usage | Roadmap tie-in |
| ----- | ------- | ------------ | -------------- |
| `async-io` | Lightweight async I/O adapters for file/serial handles. | Candidate for non-blocking serial + tunnel readers without adopting a full runtime. | Milestone A (command tunnel) and P8 framing work. |
| `bincode` | Compact binary serialization. | Storing resumable manifests, CLI history, or other RAM-disk records without JSON overhead. | P10 / Milestone C (file push/pull). |
| `calloop` | Callback-driven event loop. | Provides an alternative to async runtimes for orchestrating serial, timers, or subprocess I/O. | P11 / Milestone D (polling + heartbeat). |
| `clap_complete` | Shell completion generator for Clap-style CLIs. | Will expose completions for serialsh or other CLI helpers once the interface stabilizes. | P16 / Milestone G (CLI polish). |
| `crc32fast` | CRC32 checksum implementation. | Validating payload frames, chunk uploads, and tunnel messages. | Existing payload parser + P10 file transfers. |
| `crossbeam` | Concurrency utilities (channels, scoped threads). | Future-ready replacement for `std::sync::mpsc` when we juggle polling + rendering + tunnels. | P11 (telemetry) and Milestone D. |
| `ctrlc` | Signal handling helper. | Clean shutdown hooks in CLI tools or serial shell, forwarding Ctrl+C to tunnels. | Milestone G (serialsh) and overall daemon lifecycle. |
| `directories` | Cross-platform config/cache directory discovery. | Ensures helper tools respect `~/.serial_lcd` and `/run/serial_lcd_cache` paths. | B3/P4 (config policy) and P16 features storing history. |
| `embedded-hal` | Common HAL traits for drivers. | Shared interface for the hd44780-driver adapter+mock bus implementations. | P21 display driver integration. |
| `futures` | Core async primitives/combinators. | Building async-aware tunnel readers/writers without full `tokio`. | P8 / Milestone A (command tunnel). |
| `hd44780-driver` | HD44780 LCD control. | Direct communication with the LCD via I²C backpack. | Core app display updates (all milestones). |
| `humantime` | Parse/format human-friendly durations. | CLI/config parsing for heartbeat, polling intervals, logging. | P15 / Milestone D (heartbeat). |
| `lz4_flex` | Fast, pure-Rust LZ4 framing for streaming compression. | Frame encoder/decoder under `src/app/compression.rs` lets us wrap payloads in a compressed envelope with enforced size limits. | P14 (payload compression support). |
| `indicatif` | Progress bars and CLI indicators. | CLI file transfer UX (`--push/--pull`) without touching the LCD. | P10 / Milestone C (file transfers). |
| `linux-embedded-hal` | Embedded HAL traits for Linux. | Access to GPIO/I²C backing HD44780 operations on Pi hardware. | Core LCD work + P12 display expansions. |
| `os_info` | Host OS/arch details. | Attach environment metadata to telemetry or troubleshooting logs. | P11 / Milestone D diagnostics. |
| `rppal` | Raspberry Pi peripheral access. | GPIO/I²C access for LCD and button input. | Core behavior + P12 display features. |
| `rustix` | Safe wrappers over Unix syscalls. | Fine-grained control over serial termios/ioctl when `serialport` needs help. | P8 command tunnel serial tweaks. |
| `zstd` | Zstandard codec for optional compression envelopes. | `src/app/compression.rs` uses the streaming encoder/decoder to compare zstd with LZ4 while keeping decompressed payloads under the 1 MB limit. | P14 (payload compression). |
| `serde` | Serialization framework. | Deriving payload/config structures. | Everywhere: existing payload parser & future schema work. |
| `serde_bytes` | Efficient byte array handling in Serde. | Binary payload chunks for tunnels/file transfers. | P10 / Milestone C. |
| `serde_json` | JSON serialization/deserialization. | Primary payload transport format. | Core behavior + Milestones A–F. |
| `serialport` | Cross-platform serial port access. | Primary UART I/O (sync mode) for payload ingestion. | Core app + P8/P9 handshake. |
| `sysinfo` | System metrics (CPU, RAM, disk). | Polling module for host telemetry while respecting RAM caps. | P11 / Milestone D. |
| `syslog` | Syslog client for structured logging. | Optional path to forward telemetry/errors to system syslog instead of RAM files. | P5 (serial telemetry) + general ops. |
| `tokio` | Async runtime (gated). | Only enabled for async-serial builds; supports `tokio-serial`. | Optional for P8 experiments needing async tasks. |
| `tokio-serial` | Async serial port wrapper. | Used when the `async-serial` feature is enabled. | P8 / Milestone A, especially for tunnel concurrency. |
| `tokio-util` | Extra utilities for tokio (framed codecs). | Potential helper for framed tunnel payloads or chunk streams. | P8, P10, Milestone A/C. |

_Keep this list in sync with `Cargo.toml`. When adding new crates, append a row describing why they fit the roadmap. When removing crates, update this document only after permission is granted._
