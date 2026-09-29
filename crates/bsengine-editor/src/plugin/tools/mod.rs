//! The editor's MCP tools, one file per group. Each group registers its
//! tools from the shared handles in [`ToolContext`] -- the queues, snapshot
//! and selection `EditorPlugin::build` creates -- so a tool's closure
//! captures exactly what it did when every registration sat inline in
//! `build`, which is what these files were until they were split out.

use super::*;

mod components;
mod entities;
mod hierarchy;
mod query;
mod scene;
mod selection;
mod tags;
mod transform;
mod visibility;

/// The handles the tools' closures capture. Clones of `Arc`s: a tool that
/// needs one clones it again into its closure.
pub(super) struct ToolContext {
    pub(super) snapshot: SharedSnapshot,
    pub(super) cmd_queue: SharedCommandQueue,
    pub(super) reflect_cmd_queue: SharedReflectCommandQueue,
    pub(super) prefab_apply_cmd_queue: crate::snapshot::SharedPrefabApplyCommandQueue,
    pub(super) selection: SharedSelection,
    pub(super) type_registry: bevy_ecs::reflect::AppTypeRegistry,
    pub(super) project_dir: Option<bsengine_core::ProjectDir>,
}

/// Registers every group's tools into `mcp`.
pub(super) fn register_all(mcp: &McpRegistryResource, cx: &ToolContext) {
    components::register(mcp, cx);
    entities::register(mcp, cx);
    hierarchy::register(mcp, cx);
    query::register(mcp, cx);
    scene::register(mcp, cx);
    selection::register(mcp, cx);
    tags::register(mcp, cx);
    transform::register(mcp, cx);
    visibility::register(mcp, cx);
}

pub(super) fn parse_vec3_input(v: &serde_json::Value) -> Option<[f32; 3]> {
    let arr = v.as_array()?;
    if arr.len() < 3 {
        return None;
    }
    Some([
        arr[0].as_f64()? as f32,
        arr[1].as_f64()? as f32,
        arr[2].as_f64()? as f32,
    ])
}
