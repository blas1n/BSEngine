//! The Inspector panel: filling it from the world and applying its edits.

use super::*;

pub(super) fn populate_inspector(
    snapshot_res: Res<EditorSnapshotResource>,
    inspector: Option<ResMut<InspectorState>>,
) {
    let Some(mut inspector) = inspector else {
        return;
    };
    let snapshot = snapshot_res.0.lock().unwrap();
    inspector.entities = snapshot
        .entities
        .iter()
        .map(|e| InspectorEntityInfo {
            id: e.id,
            name: e.name.clone(),
            position: e.position,
            rotation: e.rotation,
            scale: e.scale,
            light_type: e.light_type.clone(),
            light_color: e.light_color,
            light_intensity: e.light_intensity,
            light_range: e.light_range,
            spot_inner_angle: e.spot_inner_angle,
            spot_outer_angle: e.spot_outer_angle,
            camera_fov: e.camera_fov,
            material_base_color: e.material_base_color,
            material_opacity: e.material_opacity,
            material_metallic: e.material_metallic,
            material_roughness: e.material_roughness,
            material_emissive: e.material_emissive,
            parent_id: e.parent_id,
            tags: e.tags.clone(),
            script_path: e.script_path.clone(),
            texture_path: e.texture_path.clone(),
            primitive: e.primitive.as_ref().map(primitive_to_str),
            visible: e.visible,
            selected: e.selected,
            is_prefab_instance: e.is_prefab_instance,
            // Filled in by `populate_snapshot_particles`, which runs after
            // this and reads the live emitter rather than the snapshot.
            particles: None,
        })
        .collect();
}

/// Exhaustive match with no wildcard arm — adding a `Primitive` variant
/// forces a compile error here. If you land here from that error, also
/// update `bsengine_core::PRIMITIVE_KINDS` and `str_to_primitive` below to
/// keep the DTO-boundary string set in sync (see the "primitive kinds test"
/// in this file's test module, which round-trip-checks that sync).
pub(super) fn primitive_to_str(p: &bsengine_scene::Primitive) -> String {
    match p {
        bsengine_scene::Primitive::Cube => "cube",
        bsengine_scene::Primitive::Sphere => "sphere",
        bsengine_scene::Primitive::Plane => "plane",
        bsengine_scene::Primitive::Capsule => "capsule",
        bsengine_scene::Primitive::Cylinder => "cylinder",
    }
    .to_string()
}

/// Inverse of [`primitive_to_str`] — see its doc comment for the
/// keep-in-sync note with `bsengine_core::PRIMITIVE_KINDS`.
pub(super) fn str_to_primitive(s: &str) -> Option<bsengine_scene::Primitive> {
    match s {
        "cube" => Some(bsengine_scene::Primitive::Cube),
        "sphere" => Some(bsengine_scene::Primitive::Sphere),
        "plane" => Some(bsengine_scene::Primitive::Plane),
        "capsule" => Some(bsengine_scene::Primitive::Capsule),
        "cylinder" => Some(bsengine_scene::Primitive::Cylinder),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)] // Bevy system params; splitting into a struct is a larger refactor
pub(super) fn apply_inspector_cmds(
    inspector: Option<ResMut<InspectorState>>,
    queue_res: Res<EditorCommandQueueResource>,
    reflect_queue_res: Res<ReflectCommandQueueResource>,
    prefab_queue_res: Res<PrefabCommandQueueResource>,
    prefab_apply_queue_res: Res<crate::snapshot::PrefabApplyCommandQueueResource>,
    selection_res: Res<EditorSelectionResource>,
    snapshot_res: Res<EditorSnapshotResource>,
    project_dir: Option<Res<bsengine_core::ProjectDir>>,
    mut commands: Commands,
) {
    let Some(mut inspector) = inspector else {
        return;
    };
    let cmds: Vec<InspectorCmd> = inspector.cmd_queue.drain(..).collect();
    if cmds.is_empty() {
        return;
    }
    let mut queue = queue_res.0.lock().unwrap();
    for cmd in cmds {
        match cmd {
            InspectorCmd::SetSelection { ids } => {
                let mut sel = selection_res.0.lock().unwrap();
                sel.clear();
                sel.extend(ids);
                continue;
            }
            InspectorCmd::Duplicate { id } => {
                queue.push(EditorCommand::DuplicateEntity { entity_id: id });
            }
            InspectorCmd::SetPosition { id, x, y, z } => {
                queue.push(EditorCommand::SetPosition {
                    entity_id: id,
                    x,
                    y,
                    z,
                });
            }
            InspectorCmd::SetRotation { id, rx, ry, rz } => {
                queue.push(EditorCommand::SetRotation {
                    entity_id: id,
                    rx,
                    ry,
                    rz,
                });
            }
            InspectorCmd::SetScale { id, sx, sy, sz } => {
                queue.push(EditorCommand::SetScale {
                    entity_id: id,
                    sx,
                    sy,
                    sz,
                });
            }
            InspectorCmd::SpawnEntity { name } => {
                queue.push(EditorCommand::SpawnNamed(name));
            }
            InspectorCmd::SpawnTerrain {
                heightmap_path,
                chunk_count,
                chunk_size,
                height_scale,
                layer0_texture_path,
                layer1_texture_path,
                layer2_texture_path,
                layer3_texture_path,
                splatmap_path,
            } => {
                queue.push(EditorCommand::SpawnTerrain {
                    heightmap_path,
                    chunk_count,
                    chunk_size,
                    height_scale,
                    layer0_texture_path,
                    layer1_texture_path,
                    layer2_texture_path,
                    layer3_texture_path,
                    splatmap_path,
                });
            }
            InspectorCmd::Despawn { id } => {
                queue.push(EditorCommand::Despawn { entity_id: id });
            }
            InspectorCmd::SetVisible { id, visible } => {
                queue.push(EditorCommand::SetVisible {
                    entity_id: id,
                    visible,
                });
            }
            InspectorCmd::AddPointLight { id } => {
                queue.push(EditorCommand::AttachPointLight {
                    entity_id: id,
                    color: [1.0, 1.0, 1.0],
                    intensity: 1.0,
                    range: 10.0,
                });
            }
            InspectorCmd::AddCamera { id } => {
                queue.push(EditorCommand::AttachCamera {
                    entity_id: id,
                    fov_y_degrees: 60.0,
                });
            }
            InspectorCmd::SaveScene => {
                if let Some(path) = inspector.current_scene_path.clone() {
                    queue.push(EditorCommand::SaveScene { path });
                } else {
                    tracing::warn!("save requested but no scene file is currently loaded");
                }
            }
            InspectorCmd::ReloadScene => {
                if let Some(path) = inspector.current_scene_path.clone() {
                    // Goes through the same PendingSceneLoad -> handle_scene_load
                    // path Bsengine.loadScene uses (properly despawns existing
                    // named entities, clears HUD, resets the script runtime,
                    // then respawns from the file) -- unlike EditorCommand::
                    // LoadScene above, which only spawns and would duplicate
                    // entities if the scene is already loaded.
                    commands.insert_resource(bsengine_scene::PendingSceneLoad { path });
                } else {
                    tracing::warn!("reload requested but no scene file is currently loaded");
                }
            }
            InspectorCmd::LoadScene { path } => {
                queue.push(EditorCommand::LoadScene(path));
            }
            InspectorCmd::WriteImportSettings { path, settings } => {
                // Through the same function as the MCP tool, so the two
                // cannot differ on minting or keeping the asset's identity.
                // On success the snapshot is dropped, not patched:
                // `populate_asset_import_snapshot` re-reads the sidecar next
                // frame, so what the Inspector shows is what is on disk
                // (`recorded: true` included), never what it hoped it wrote.
                match bsengine_asset::identity::write_import_settings(
                    std::path::Path::new(&path),
                    settings,
                ) {
                    Ok(meta) => {
                        tracing::info!("import settings written to {}", meta.display());
                        inspector.asset_import = None;
                        inspector.asset_import_error = None;
                    }
                    Err(e) => {
                        tracing::warn!("import settings for {path} not written: {e}");
                        inspector.asset_import_error = Some(e.to_string());
                    }
                }
                continue;
            }
            InspectorCmd::ParticleBurst { id } => {
                queue.push(EditorCommand::ParticleBurst { entity_id: id });
            }
            InspectorCmd::ParticleRestart { id } => {
                queue.push(EditorCommand::ParticleRestart { entity_id: id });
            }
            InspectorCmd::SpawnMeshAsset { name, path } => {
                queue.push(EditorCommand::SpawnMeshAsset { name, path });
            }
            InspectorCmd::AttachComponentByType { id, type_path } => {
                reflect_queue_res
                    .0
                    .lock()
                    .unwrap()
                    .push(ReflectCommand::AttachComponentByType {
                        entity_id: id,
                        type_path,
                    });
            }
            InspectorCmd::RemoveComponentByType { id, type_path } => {
                reflect_queue_res
                    .0
                    .lock()
                    .unwrap()
                    .push(ReflectCommand::RemoveComponentByType {
                        entity_id: id,
                        type_path,
                    });
            }
            InspectorCmd::RenameEntity { id, name } => {
                queue.push(EditorCommand::RenameEntity {
                    entity_id: id,
                    name,
                });
            }
            InspectorCmd::SetParent { id, parent_id } => {
                queue.push(EditorCommand::SetParent {
                    entity_id: id,
                    parent_id,
                });
            }
            InspectorCmd::RemoveParent { id } => {
                queue.push(EditorCommand::RemoveParent { entity_id: id });
            }
            InspectorCmd::TagEntity { id, tag } => {
                queue.push(EditorCommand::TagEntity { entity_id: id, tag });
            }
            InspectorCmd::UntagEntity { id, tag } => {
                queue.push(EditorCommand::UntagEntity { entity_id: id, tag });
            }
            InspectorCmd::AttachScript { id, path } => {
                queue.push(EditorCommand::AttachScript {
                    entity_id: id,
                    path,
                });
            }
            InspectorCmd::DetachScript { id } => {
                queue.push(EditorCommand::DetachScript { entity_id: id });
            }
            InspectorCmd::AttachPrimitiveMesh { id, primitive } => {
                if let Some(primitive) = str_to_primitive(&primitive) {
                    queue.push(EditorCommand::AttachPrimitiveMesh {
                        entity_id: id,
                        primitive,
                    });
                } else {
                    tracing::warn!("unknown primitive kind '{primitive}'");
                }
            }
            InspectorCmd::DetachPrimitiveMesh { id } => {
                queue.push(EditorCommand::DetachPrimitiveMesh { entity_id: id });
            }
            InspectorCmd::ApplyReflectedComponent {
                id,
                type_path,
                value,
            } => {
                reflect_queue_res
                    .0
                    .lock()
                    .unwrap()
                    .push(ReflectCommand::ApplyComponentValue {
                        entity_id: id,
                        type_path,
                        value,
                    });
            }
            InspectorCmd::InstantiatePrefab {
                path,
                name,
                x,
                y,
                z,
                parent_id,
            } => {
                prefab_queue_res
                    .0
                    .lock()
                    .unwrap()
                    .push(PrefabInstantiateCommand {
                        path,
                        name,
                        x,
                        y,
                        z,
                        parent_id,
                    });
            }
            InspectorCmd::CreatePrefab { entity_id, name } => {
                let s = snapshot_res.0.lock().unwrap();
                if let Err(e) =
                    save_entities_as_prefab(&s.entities, entity_id, &name, project_dir.as_deref())
                {
                    tracing::warn!("create_prefab: entity {entity_id} -> '{name}' failed: {e}");
                }
            }
            InspectorCmd::ApplyToPrefab { entity_id } => {
                prefab_apply_queue_res
                    .0
                    .lock()
                    .unwrap()
                    .push(crate::snapshot::PrefabApplyCommand { entity_id });
            }
        }
    }
}
