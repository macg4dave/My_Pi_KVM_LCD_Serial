
# Roadmap skeletons

Created on 2025-12-02 — roadmap skeleton Rust modules that provide a minimal, compile-safe
surface for the next Milestones and priority items. These files are stubs intended to be
expanded with full implementations and tests in subsequent work.

New modules created under src/app:

- polling.rs — Milestone D (live hardware polling) skeleton (leverages `systemstat` for the lightweight metrics API)
- compression.rs — Milestone F (payload compression) skeleton
- watchdog.rs — Milestone P15 (heartbeat/watchdog) skeleton
- negotiation.rs — Milestone B (auto-negotiation) skeleton shared between the connection logic and config loader
- connection.rs — Milestone B reconnection/handshake driver that wires the new `Negotiator` into the render loop

Each module contains a small API and unit tests so CI can validate the surface area and
developers can build on top of these placeholders safely.

- Next steps

- Flesh out each module with production behaviour (avoid writing outside CACHE_DIR)
- Add integration tests that exercise multi-module flows (tunnel + file transfer)
- Add documentation to milestone files in docs/ and update README.md with new notes
