//! Milestone modules: higher-order features that are gated behind roadmap milestones and
//! optional features. This file re-exports the per-milestone modules so they can be referenced
//! by the rest of the crate (when enabled).

pub mod command_tunnel;
pub mod negotiation;
pub mod polling;
pub mod schema;
pub mod transfer;

// The milestone modules are intentionally minimal by default and don't alter the core
// runtime unless their respective features become fully implemented. This file is only to
// consolidate namespacing and make the feature work tree compile cleanly.
