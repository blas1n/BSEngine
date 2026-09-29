use crate::snapshot::{
    EditorCommand, EditorCommandQueueResource, EditorHistory, EditorHistoryResource,
    EditorSelectionResource, EditorSnapshot, EditorSnapshotResource, EntityInfo,
    PrefabCommandQueueResource, PrefabInstantiateCommand, ReflectCommand,
    ReflectCommandQueueResource, SharedCommandQueue, SharedHistory, SharedPrefabCommandQueue,
    SharedReflectCommandQueue, SharedSelection, SharedSnapshot, Tags,
};
use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::{Commands, Entity, IntoSystemConfigs, ParamSet, Query, ResMut, World};
use bevy_ecs::reflect::ReflectCommandExt;
use bsengine_core::{
    Camera, DirectionalLight, EditorPanelRegistry, GlobalTransform, InspectorCmd,
    InspectorEntityInfo, InspectorState, Material, Parent, PointLight, SpotLight, Transform,
    Visible,
};
use bsengine_ecs::Res;
use bsengine_mcp::{McpRegistryResource, McpTool, McpToolOutput};
use bsengine_render::MeshRenderer;
use bsengine_scene::{EntityDescriptor, Name, PrimitiveMesh, SceneDescriptor};
use serde_json::json;
use std::sync::{Arc, Mutex};

mod camera;
mod commands;
mod history;
mod inspector;
mod prefab;
mod reflect;
mod snapshot_sync;
#[cfg(test)]
mod tests;
mod tools;

use camera::*;
use commands::*;
use history::*;
use inspector::*;
use prefab::*;
use reflect::*;
pub(crate) use snapshot_sync::excluded_from_extra_components;
use snapshot_sync::*;

/// Bevy `Plugin` that wires the editor bridge into an `App`: inserts the
/// shared snapshot/command-queue/selection/history resources and registers
/// the systems that process `EditorCommand`s and `ReflectCommand`s each frame.
pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        let snapshot: SharedSnapshot = Arc::new(Mutex::new(EditorSnapshot::default()));
        let cmd_queue: SharedCommandQueue = Arc::new(Mutex::new(Vec::new()));
        let reflect_cmd_queue: SharedReflectCommandQueue = Arc::new(Mutex::new(Vec::new()));
        let prefab_cmd_queue: SharedPrefabCommandQueue = Arc::new(Mutex::new(Vec::new()));
        let prefab_apply_cmd_queue: crate::snapshot::SharedPrefabApplyCommandQueue =
            Arc::new(Mutex::new(Vec::new()));
        let selection: SharedSelection = Arc::new(Mutex::new(std::collections::HashSet::new()));
        let history: SharedHistory = Arc::new(Mutex::new(EditorHistory::default()));

        app.insert_resource(EditorSnapshotResource(snapshot.clone()));
        app.insert_resource(EditorCommandQueueResource(cmd_queue.clone()));
        app.insert_resource(ReflectCommandQueueResource(reflect_cmd_queue.clone()));
        app.insert_resource(PrefabCommandQueueResource(prefab_cmd_queue.clone()));
        app.insert_resource(crate::snapshot::PrefabApplyCommandQueueResource(
            prefab_apply_cmd_queue.clone(),
        ));
        app.insert_resource(EditorSelectionResource(selection.clone()));
        app.insert_resource(EditorHistoryResource(history.clone()));
        app.insert_resource(InspectorState::editor());
        app.insert_resource(EditorPanelRegistry::default());
        // Shared with the headless test runtime (`bsengine-runtime --test`'s
        // `build_test_app`) so reflected `components:` entries (Shield,
        // SaveData, AnimationStateMachine, NavMeshAgent, Bloom, ToneMap, ...)
        // deserialize identically in both -- see its doc comment in
        // bsengine-scene for the headless-mode gap this fixed.
        bsengine_scene::register_gameplay_reflect_types(app);
        app.register_type::<Tags>();
        app.register_type::<bsengine_scene::Primitive>();
        app.register_type::<bsengine_scene::PrimitiveMesh>();
        app.register_type::<bsengine_scene::ScriptPath>();
        // Explicit, defensive registration -- as of this writing these three
        // already get a ReflectDefault transitively (bevy_reflect's
        // register_type_dependencies walks Transform's/ScriptPath's own
        // fields), so this is currently redundant, not a fix for an active
        // bug. Kept anyway so List-append/enum-variant-switch (which need
        // ReflectDefault for these types in the real app registry, not just
        // in reflect_ui.rs's own unit-test-local registries) don't silently
        // regress if a future refactor to Transform/ScriptPath breaks that
        // transitive path.
        app.register_type::<String>();
        app.register_type::<bsengine_core::ReflectVec3>();
        app.register_type::<bsengine_core::ReflectQuat>();
        app.add_systems(Update, update_editor_snapshot);
        app.add_systems(Update, update_editor_camera);
        // No ordering constraint. The Timeline panel publishes during egui and
        // this consumes on the following frame -- one frame of lag, the same
        // the terrain brush already lives with, and imperceptible while
        // dragging a playhead.
        app.add_systems(
            Update,
            crate::timeline_preview::apply_timeline_preview_animation,
        );
        app.add_systems(Update, populate_inspector.after(update_editor_snapshot));
        app.add_systems(
            Update,
            populate_snapshot_particles.after(populate_inspector),
        );
        app.add_systems(Update, populate_reflected_component_snapshot);
        // After the command drain, so a write's "drop the snapshot" is
        // followed by the re-read in the same frame rather than a frame of
        // "Reading import settings..." between Apply and the refreshed fields.
        app.add_systems(
            Update,
            populate_asset_import_snapshot.after(apply_inspector_cmds),
        );
        app.add_systems(Update, populate_asset_references_snapshot);
        // After the command drain, so a scene save's request is answered in
        // the same frame rather than a frame later.
        app.add_systems(
            Update,
            populate_asset_graph_snapshot.after(process_editor_commands),
        );
        app.add_systems(
            Update,
            populate_snapshot_extra_components.after(update_editor_snapshot),
        );
        app.add_systems(Update, apply_inspector_cmds.before(process_editor_commands));
        app.add_systems(Update, process_editor_commands);
        app.add_systems(
            Update,
            process_reflect_commands.after(process_editor_commands),
        );
        app.add_systems(
            Update,
            process_prefab_commands.after(process_editor_commands),
        );
        // Explicit `.before(apply_inspector_cmds)`, not left unordered: both
        // this system and `apply_inspector_cmds` only take `Res<...>` handles
        // to their shared `Mutex`-wrapped queue, so Bevy sees no ECS access
        // conflict between them and is free to run them in either order each
        // frame. Without this constraint, a same-frame
        // `InspectorCmd::ApplyToPrefab` (queued via `apply_inspector_cmds`
        // during this exact `Update`) could be drained by this system before
        // it was ever pushed, or after -- nondeterministically, varying
        // update-to-update. Ordering this system first guarantees a command
        // bridged from the Inspector this frame sits in the queue,
        // unprocessed, until the *next* frame -- consistent with this
        // function's own doc comment above (downstream effects already
        // happen "on a later frame", not synchronously) and with the
        // `apply_to_prefab` MCP tool's description ("Queued for processing;
        // check ... on a subsequent call").
        app.add_systems(
            Update,
            process_prefab_apply_commands.before(apply_inspector_cmds),
        );
        app.add_systems(Update, apply_history_action.after(process_editor_commands));
        // Registration position matters here, and is not incidental: this has
        // to come after every `add_systems` call above, not next to the other
        // cross-crate registration calls near the top of this function (where
        // it originally sat). `PrefabWatcherPlugin` adds its own unordered
        // `Update` systems, and registering them earlier shifted the implicit
        // registration-order tie-break `populate_snapshot_extra_components`'s
        // doc comment already relies on between it and
        // `process_editor_commands`/`process_reflect_commands` -- which broke
        // `mcp_set_transform_moves_entity`. If you add or reorder systems in
        // this function, keep this line last, or re-run the full test suite
        // (not just tests that look related) to confirm that tie-break still
        // resolves the same way.
        app.add_plugins(crate::prefab_watcher::PrefabWatcherPlugin);

        // Captured once here (rather than inside the `if let` below) and cloned
        // per-tool like `snapshot`/`cmd_queue` are -- `app.world_mut()` backs the
        // `mcp` binding for the rest of this function, so `app.world()` can't be
        // called again inside that block without a borrow conflict.
        let type_registry = app
            .world()
            .resource::<bevy_ecs::reflect::AppTypeRegistry>()
            .clone();
        // Captured once here for the same reason as `type_registry` above --
        // `app.world_mut()` backs `mcp` for the rest of this function, so
        // `app.world()` can't be called again inside the `if let` block
        // below (e.g. from within `prefab_write`'s registration).
        let project_dir = app
            .world()
            .get_resource::<bsengine_core::ProjectDir>()
            .cloned();

        if let Some(mcp) = app.world_mut().get_resource_mut::<McpRegistryResource>() {
            tools::register_all(
                &mcp,
                &tools::ToolContext {
                    snapshot: snapshot.clone(),
                    cmd_queue: cmd_queue.clone(),
                    reflect_cmd_queue: reflect_cmd_queue.clone(),
                    prefab_apply_cmd_queue: prefab_apply_cmd_queue.clone(),
                    selection: selection.clone(),
                    type_registry: type_registry.clone(),
                    project_dir: project_dir.clone(),
                },
            );
        }
    }
}
