//! Editor backend for BSEngine, exposed via MCP (Model Context Protocol).
//!
//! `EditorPlugin` wires up the editor's ECS systems; `EditorCommand` is the
//! command layer an AI agent or UI drives the editor through (spawn,
//! transform, hierarchy, tags, selection, ...), snapshotted each frame via
//! `EditorSnapshot`/`EntityInfo` for the ~700 MCP tools described in the
//! project README.
// Bevy ECS system params (Query<(A, B, C, ...)>, ParamSet<(...)>) routinely
// exceed clippy's type-complexity threshold; that's the idiom, not a real
// complexity problem. Bevy itself disables this lint crate-wide for the
// same reason.
#![allow(clippy::type_complexity)]
#![warn(missing_docs)]

/// Registers the editor's ECS systems, resources, and the `EditorCommand`/
/// `ReflectCommand` processing loop into a Bevy `App`.
pub mod plugin;
/// The MCP-facing data model: `EditorSnapshot`, `EntityInfo`, `EditorCommand`,
/// `ReflectCommand`, and the shared resources the editor bridge reads/writes.
pub mod snapshot;

pub use plugin::EditorPlugin;
pub use snapshot::{EditorCommand, EditorSnapshot, EditorSnapshotResource, EntityInfo};
