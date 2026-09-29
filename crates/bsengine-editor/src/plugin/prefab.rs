//! Saving entities as prefabs and applying prefab commands.

use super::*;

/// Builds RON-serializable `EntityDescriptor`s from tracked snapshot entities.
/// Only named entities are included (unnamed entities aren't addressable in
/// scene files). GLTF paths are not tracked by `EntityInfo` and are
/// intentionally left `None` here.
pub(super) fn build_entity_descriptors(entities: &[EntityInfo]) -> Vec<EntityDescriptor> {
    let id_to_name: std::collections::HashMap<u64, &str> = entities
        .iter()
        .filter_map(|e| Some((e.id, e.name.as_deref()?)))
        .collect();

    entities
        .iter()
        .filter_map(|e| {
            e.name.as_ref().map(|name| {
                let quat = e.rotation.map(|[rx, ry, rz]| {
                    glam::Quat::from_euler(
                        glam::EulerRot::XYZ,
                        rx.to_radians(),
                        ry.to_radians(),
                        rz.to_radians(),
                    )
                });
                let transform = if e.position.is_some() || e.rotation.is_some() || e.scale.is_some()
                {
                    let position = e.position.unwrap_or([0.0, 0.0, 0.0]);
                    let scale = e.scale.unwrap_or([1.0, 1.0, 1.0]);
                    let q = quat.unwrap_or(glam::Quat::IDENTITY);
                    Some(bsengine_scene::TransformDescriptor {
                        position,
                        rotation: [q.x, q.y, q.z, q.w],
                        scale,
                    })
                } else {
                    None
                };
                let directional_light = if e.light_type.as_deref() == Some("directional") {
                    let dir = quat.unwrap_or(glam::Quat::IDENTITY) * glam::Vec3::NEG_Z;
                    Some(bsengine_scene::DirectionalLightDescriptor {
                        direction: dir.to_array(),
                        color: e.light_color.unwrap_or([1.0, 1.0, 1.0]),
                        ambient: e.light_ambient.unwrap_or([0.1, 0.1, 0.1]),
                    })
                } else {
                    None
                };
                let point_light = if e.light_type.as_deref() == Some("point") {
                    Some(bsengine_scene::PointLightDescriptor {
                        color: e.light_color.unwrap_or([1.0, 1.0, 1.0]),
                        intensity: e.light_intensity.unwrap_or(1.0),
                        range: e.light_range.unwrap_or(10.0),
                    })
                } else {
                    None
                };
                let spot_light = if e.light_type.as_deref() == Some("spot") {
                    Some(bsengine_scene::SpotLightDescriptor {
                        color: e.light_color.unwrap_or([1.0, 1.0, 1.0]),
                        intensity: e.light_intensity.unwrap_or(1.0),
                        range: e.light_range.unwrap_or(10.0),
                        inner_angle_degrees: e.spot_inner_angle.unwrap_or(22.5),
                        outer_angle_degrees: e.spot_outer_angle.unwrap_or(30.0),
                    })
                } else {
                    None
                };
                EntityDescriptor {
                    name: name.clone(),
                    components: e.extra_components.clone(),
                    transform,
                    gltf: None,
                    camera: e.camera_fov.is_some(),
                    camera_fov: e.camera_fov,
                    directional_light,
                    point_light,
                    spot_light,
                    primitive: e.primitive.clone(),
                    script: e.script_path.clone().map(bsengine_scene::AssetRef::Path),
                    texture: e.texture_path.clone().map(bsengine_scene::AssetRef::Path),
                    emissive: e.material_emissive,
                    color: e.material_base_color,
                    opacity: e.material_opacity,
                    look_at: None,
                    rigidbody: e.physics_body.as_ref().map(|p| p.rigidbody.clone()),
                    collider: e.physics_body.as_ref().map(|p| p.collider.clone()),
                    linear_damping: e.physics_body.as_ref().and_then(|p| p.linear_damping),
                    angular_damping: e.physics_body.as_ref().and_then(|p| p.angular_damping),
                    parent: e.parent_id.and_then(|pid| match id_to_name.get(&pid) {
                        Some(name) => Some(name.to_string()),
                        None => {
                            tracing::warn!(
                                "scene save: entity '{}' is parented to entity {pid}, which has \
                                 no Name component, so this scene file cannot preserve that \
                                 parent link",
                                e.name.as_deref().unwrap_or("<unnamed>")
                            );
                            None
                        }
                    }),
                    // Always None: this build has no way to know whether a
                    // live entity was originally instantiated from a
                    // prefab (EntityInfo tracks no such provenance), and
                    // saving that link is out of scope for prefab
                    // instantiation -- it belongs to a future live-sync
                    // feature that doesn't exist yet, not to any task in
                    // this plan.
                    prefab: None,
                    // Always None: no live ECS component carries LOD data
                    // yet (that wiring is a later task in this plan), so
                    // there is nothing here to round-trip on save.
                    lod: None,
                    // Always None, for the same reason `gltf` above is: the
                    // snapshot this rebuilds from (`EntityInfo`) carries no
                    // joint data, so there is nothing here to write back.
                    // Saving a joint would also need the *name* of the other
                    // body, and a joint's `body_b` is an entity id -- the
                    // same id->name lookup `parent:` does above, which only
                    // this function's own slice can answer.
                    joint: None,
                }
            })
        })
        .collect()
}

/// Turns `root_id` and its descendants (read from `entities`, a live
/// snapshot) into a new `assets/prefabs/<name>.ron` file, auto-suffixing
/// the filename (`<name>#2.ron`, `<name>#3.ron`, ...) if one already
/// exists. Returns the actual path written.
///
/// Reuses `build_entity_descriptors` unchanged: that function only
/// resolves a `parent:` link for a parent id present in the *same slice*
/// it's given, so passing just the collected subtree automatically drops
/// the chosen root's link to whatever it used to be parented to outside
/// that subtree -- no separate re-rooting step is needed. (If the root
/// did have an outside parent, `build_entity_descriptors` logs one
/// pre-existing, slightly-imprecisely-worded warning about a missing
/// Name link; that's accepted, existing behavior from the scene-save
/// path, not something this function introduces.)
pub(super) fn save_entities_as_prefab(
    entities: &[EntityInfo],
    root_id: u64,
    name: &str,
    project_dir: Option<&bsengine_core::ProjectDir>,
) -> Result<String, String> {
    if name.is_empty() {
        return Err("prefab name must not be empty".to_string());
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(format!(
            "prefab name '{name}' must not contain path separators or '..'"
        ));
    }

    let Some(root) = entities.iter().find(|e| e.id == root_id) else {
        return Err(format!("entity {root_id} not found"));
    };
    if root.name.is_none() {
        return Err(format!(
            "entity {root_id} has no Name component and cannot be saved as a prefab"
        ));
    }

    // `visited` is defensive, not merely theoretical: nothing in the
    // command-processing layer (EditorCommand::SetParent, the MCP
    // set_parent tool) checks for cycles when writing parent_id -- only
    // the Hierarchy panel's drag-and-drop UI calls would_create_cycle --
    // so a malformed live snapshot with a real parent_id cycle can reach
    // this BFS. Mirrors HierarchyPanel::push_dfs's guard in
    // crates/bsengine-rhi-wgpu/src/panels/hierarchy.rs.
    let mut subtree: Vec<EntityInfo> = Vec::new();
    let mut visited: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let mut queue = vec![root_id];
    while let Some(cur) = queue.pop() {
        if !visited.insert(cur) {
            continue;
        }
        if let Some(info) = entities.iter().find(|e| e.id == cur) {
            subtree.push(info.clone());
        }
        for child in entities.iter().filter(|e| e.parent_id == Some(cur)) {
            queue.push(child.id);
        }
    }

    if let Some(unnamed) = subtree.iter().find(|e| e.name.is_none()) {
        return Err(format!(
            "entity {} in the selected subtree has no Name component and cannot be saved as a prefab",
            unnamed.id
        ));
    }

    let descriptors = build_entity_descriptors(&subtree);
    let prefab = bsengine_scene::types::PrefabDescriptor {
        entities: descriptors,
    };
    let ron_str = ron::to_string(&prefab).map_err(|e| format!("serialize failed: {e}"))?;

    let dest = resolve_unique_prefab_path(project_dir, name);
    if let Some(parent) = std::path::Path::new(&dest).parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("could not create directory: {e}"))?;
    }
    std::fs::write(&dest, &ron_str).map_err(|e| format!("write failed: {e}"))?;
    Ok(dest)
}

/// Resolves `assets/prefabs/<name>.ron` against `project_dir`, retrying
/// with `<name>#2.ron`, `<name>#3.ron`, ... until a path that doesn't
/// already exist on disk is found -- mirrors `instantiate_prefab`'s
/// runtime `#N` instance-name suffixing, applied here to filenames.
pub(super) fn resolve_unique_prefab_path(
    project_dir: Option<&bsengine_core::ProjectDir>,
    name: &str,
) -> String {
    let candidate =
        bsengine_core::resolve_project_path(project_dir, &format!("assets/prefabs/{name}.ron"));
    if !std::path::Path::new(&candidate).exists() {
        return candidate;
    }
    let mut n = 2;
    loop {
        let candidate = bsengine_core::resolve_project_path(
            project_dir,
            &format!("assets/prefabs/{name}#{n}.ron"),
        );
        if !std::path::Path::new(&candidate).exists() {
            return candidate;
        }
        n += 1;
    }
}

/// Drains `PrefabCommandQueueResource` each frame and instantiates each
/// queued prefab via `bsengine_scene::instantiate_prefab_from_path`, which
/// needs `&mut World` directly -- see `PrefabInstantiateCommand`'s doc
/// comment. Going through `instantiate_prefab_from_path` (rather than
/// reading/parsing the file here and calling
/// `bsengine_scene::instantiate_prefab` directly, as this used to) is what
/// registers this top-level call into the crate's cycle-detection set
/// before a nested `prefab:` reference inside the file can recurse --
/// skipping it meant a prefab whose own child referenced its containing
/// file silently double-spawned via drag-and-drop, since the guard only
/// ever caught such a reference on its second encounter, not its first.
pub(super) fn process_prefab_commands(world: &mut World) {
    let cmds: Vec<PrefabInstantiateCommand> = {
        let Some(queue_res) = world.get_resource::<PrefabCommandQueueResource>() else {
            return;
        };
        let mut queue = queue_res.0.lock().unwrap();
        queue.drain(..).collect()
    };
    if cmds.is_empty() {
        return;
    }

    if let (Some(snapshot_res), Some(history_res)) = (
        world.get_resource::<EditorSnapshotResource>(),
        world.get_resource::<EditorHistoryResource>(),
    ) {
        let checkpoint = snapshot_res.0.lock().unwrap().clone();
        let mut history = history_res.0.lock().unwrap();
        history.undo_stack.push(checkpoint);
        history.redo_stack.clear();
        if history.undo_stack.len() > MAX_UNDO_HISTORY {
            history.undo_stack.remove(0);
        }
    }

    let project_dir = world.get_resource::<bsengine_core::ProjectDir>().cloned();
    for cmd in cmds {
        let resolved_path = bsengine_core::resolve_project_path(project_dir.as_ref(), &cmd.path);
        let parent_entity = cmd
            .parent_id
            .and_then(|id| world.iter_entities().find(|e| e.id().index() as u64 == id))
            .map(|e| e.id());
        let transform = bsengine_scene::TransformDescriptor {
            position: [cmd.x, cmd.y, cmd.z],
            ..Default::default()
        };
        if let Err(e) = bsengine_scene::instantiate_prefab_from_path(
            world,
            &resolved_path,
            cmd.name.as_deref(),
            Some(transform),
            parent_entity,
        ) {
            tracing::warn!("prefab: '{resolved_path}' failed to instantiate: {e}");
        }
    }
}

/// Drains `PrefabApplyCommandQueueResource` each frame and pushes each
/// queued entity's overrides back into its source prefab file via
/// `crate::prefab_merge::apply_instance_to_prefab`, which needs `&mut World`
/// directly -- same reason `process_prefab_commands` is exclusive. Unlike
/// that system, this one does not push an undo checkpoint: it writes a file
/// on disk, not a live-world mutation the undo/redo history tracks, and its
/// downstream effects (every instance resyncing) happen on a later frame via
/// `PrefabWatcherPlugin`'s own file-watch, not synchronously here.
pub(super) fn process_prefab_apply_commands(world: &mut World) {
    let cmds: Vec<crate::snapshot::PrefabApplyCommand> = {
        let Some(queue_res) =
            world.get_resource::<crate::snapshot::PrefabApplyCommandQueueResource>()
        else {
            return;
        };
        let mut queue = queue_res.0.lock().unwrap();
        queue.drain(..).collect()
    };
    for cmd in cmds {
        let Some(entity) = world
            .iter_entities()
            .find(|e| e.id().index() as u64 == cmd.entity_id)
            .map(|e| e.id())
        else {
            tracing::warn!("apply-to-prefab: entity {} no longer exists", cmd.entity_id);
            continue;
        };
        if let Err(e) = crate::prefab_merge::apply_instance_to_prefab(world, entity) {
            tracing::warn!("apply-to-prefab: failed for entity {}: {e}", cmd.entity_id);
        }
    }
}
