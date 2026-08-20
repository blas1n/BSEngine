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
/// Field-level merge logic for prefab push-sync: snapshots a live instance's
/// current state and decides, per field, which values a resync should
/// preserve (user overrides) vs. adopt from the prefab's new content.
/// Internal wiring for `prefab_watcher.rs`'s resync, not a public API of
/// this crate.
pub(crate) mod prefab_merge;
/// Despawn-and-reinstantiate resync for prefab instances whose source file
/// changed on disk, and the file watcher (`PrefabWatcherPlugin`) that
/// detects those changes.
pub mod prefab_watcher;
/// The MCP-facing data model: `EditorSnapshot`, `EntityInfo`, `EditorCommand`,
/// `ReflectCommand`, and the shared resources the editor bridge reads/writes.
pub mod snapshot;

pub use plugin::EditorPlugin;
pub use prefab_watcher::PrefabWatcherPlugin;
pub use snapshot::{EditorCommand, EditorSnapshot, EditorSnapshotResource, EntityInfo};
