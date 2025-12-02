# Milestone G — CLI Integration Mode (`--serialsh`)

> **Roadmap alignment**: Milestone G in [docs/roadmap.md](./roadmap.md) (depends on P7, P8, P16 and the completion of Milestones A & B). This document replaces the AI draft with a scoped plan that matches the current repository layout and charter.

## 🎯 Goal

Provide an opt-in CLI front-end that lets an operator run individual commands through the Milestone A command tunnel, see stdout/stderr as text, and exit with the same status code the remote side returned. The feature must *not* change the default `lifelinetty --run` LCD daemon path and must honor all existing storage, serial, and memory limits.

## 🚦 Prerequisites & Gating

* Milestone A (bi-directional command tunnel) exposes a stable request/response API with stdout/stderr chunking in `src/app/connection.rs`.
* Milestone B (negotiation) guarantees that a CLI peer can learn whether the remote endpoint supports the tunnel; `serialsh` must bail out cleanly if the capability bit is missing.
* Priority items P7 (CLI groundwork), P8 (command tunnel schema), and P16 (CLI UX polish hooks) must be merged before this milestone is scheduled.
* No additional crates beyond the charter are required; history/editing must use `std::io` or reuse the existing optional `ctrlc` crate.

## 📦 Deliverables

1. `--serialsh` flag parsing in `src/cli.rs`, mutually exclusive with `--run`, `--test-lcd`, and `--test-serial`.
2. A CLI execution path in `src/app/mod.rs` (or a focused helper module) that:
   * Builds the serial connection using the same config/CLI overrides as the daemon.
   * Initializes the command tunnel client from Milestone A.
   * Drives an interactive loop on stdin/stdout without touching the LCD driver.
3. Signal handling that routes Ctrl+C through the tunnel (cancel current command) and exits the process if pressed again while idle. The existing `ctrlc` dependency must be reused; no new runtime.
4. Deterministic exit code policy documented in `README.md` + roadmap:
   * 0 when the session runs at least one command and the last command exits 0.
   * Last remote exit code when available.
   * 255 for transport/negotiation failures.
5. CLI smoke/integration tests (using `tests/bin_smoke.rs` + `tests/fake_serial_loop.rs`) that cover happy-path execution, non-zero exits, disconnects, and Ctrl+C behavior.
6. Documentation updates (`README.md`, `docs/roadmap.md`, `docs/milestone_g.md`, and `samples/` payloads if needed) describing usage, caveats, and storage impacts. History/temporary artifacts must live under `/run/serial_lcd_cache`.

## 🛠️ Implementation Plan

### 1. Parser & mode selection

* Extend the existing custom parser in `src/cli.rs` to accept `--serialsh` and optional overrides already supported elsewhere (`--device`, `--baud`, `--cols`, `--rows`).
* Reject invalid flag mixes up front (e.g., `--serialsh --run`).
* Expose a `CliMode::SerialShell` enum branch so downstream modules can branch without string parsing.

### 2. Serial connection + negotiation bootstrap

* Reuse `src/app/connection.rs::SerialConnectionBuilder` (introduced for Milestone A) so serialsh shares retry/backoff policies.
* After open, run the Milestone B capability exchange; abort with a friendly message if the command tunnel bit is absent or times out. Ensure logs go to stderr and `/run/serial_lcd_cache/serialsh.log` if logging to file is required.

### 3. Command loop façade

* Create a lightweight loop (synchronous for now) that blocks on `stdin().read_line`. Async runtimes are unnecessary because serial IO is already offloaded.
* Each submitted line is wrapped in the Milestone A command request struct and sent through the tunnel client handle. Responses surface as an iterator/stream of `CommandChunk::Stdout`/`::Stderr` plus a final status frame.
* Print stdout chunks to `stdout` verbatim; stderr chunks go to `stderr`. No ANSI or PTY negotiation. Buffering must be line-oriented to avoid growing RSS.
* Track the most recent exit code; if no commands run, default to 0.

### 4. History & prompt (optional, RAM-only)

* Minimal viable product: constant prompt `serialsh>` written to stderr so stdout remains reserved for remote data.
* If history is enabled later, cache it in `/run/serial_lcd_cache/serialsh_history`. Provide a `--no-history` knob but keep it off by default until UX polish work (P16) lands.

### 5. Ctrl+C and cancellation

* Register a handler via the existing `ctrlc` crate that notifies the command loop (e.g., an `AtomicBool` or channel).
* When a command is in-flight, send a tunnel “cancel” frame defined by Milestone A and wait for confirmation before re-displaying the prompt.
* When idle, the first Ctrl+C simply exits with the last recorded status; document this behavior in the README and tests.

### 6. Error handling + exit semantics

* Lost serial link, negotiation timeout, or malformed tunnel responses must result in a user-facing error plus exit code 255. Cache any debug artifacts under `/run/serial_lcd_cache/serialsh_*`.
* Non-zero remote exits propagate directly so scripts can check `$?`.
* Ensure `Drop` implementations flush/close the serial port cleanly to avoid leaving the daemon endpoint in an inconsistent state.

## 🧪 Testing Strategy

* **Unit tests**: add focused tests around new CLI parsing branches and tunnel-client helpers.
* **Integration tests**:
  * Extend `tests/bin_smoke.rs` with a serialsh invocation that uses the fake serial backend to execute `echo hi` and verify the captured output + exit code.
  * Add scenarios to `tests/fake_serial_loop.rs` (or a new helper) to simulate non-zero exits, disconnects, and cancel frames.
  * Include a regression ensuring `--serialsh` refuses to run when negotiation reports “LCD-only”.
* **Manual smoke**: run `cargo run -- --serialsh --device /dev/ttyUSB0 --baud 9600` against a dev board once Milestone A is wired up, documenting the steps in `docs/releasing.md`.

## 📝 Documentation & Ops

* Update `README.md` CLI table and usage examples.
* Mention serialsh mode in `docs/roadmap.md` progress notes plus `docs/architecture.md` (interaction diagram showing CLI vs daemon path).
* Provide a short “Try it” snippet referencing the fake serial server in `samples/` once available.
* Ensure systemd units stay untouched—`serialsh` is an interactive user command, not a service.

## 🚫 Out of Scope

* No PTY emulation, SSH-like multiplexing, or network sockets.
* No new config files or persistent state outside `~/.serial_lcd/config.toml`.
* No LCD rendering when `--serialsh` is selected—the daemon path remains the only component driving the panel.
* No speculative protocol changes; all framing/negotiation updates must land in Milestones A/B before this milestone is scheduled.

## Allowed crates & dependencies

Serial shell mode keeps the dependency set unchanged: `std`, `serde`, `serde_json`, `crc32fast`, `hd44780-driver`, `serialport`, optional `tokio`/`tokio-serial` via the existing feature flag, `rppal`, `linux-embedded-hal`, and `ctrlc`. Interactive behavior relies on `std::io` plus the already-whitelisted `ctrlc` crate—no line-editing or PTY crates are added.

---

Keeping Milestone G focused on the CLI façade ensures we can deliver value immediately after the tunnel and negotiation work ship, without bloating the binary or violating the RAM-disk/storage contract.
