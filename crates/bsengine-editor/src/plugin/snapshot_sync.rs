//! Copies the ECS world into `EditorSnapshot` each frame -- what the MCP tools read.

use super::*;

pub(super) fn update_editor_snapshot(
    snapshot_res: Res<EditorSnapshotResource>,
    selection_res: Res<EditorSelectionResource>,
    query: Query<(
        Entity,
        Option<&Name>,
        Option<&Transform>,
        Option<&MeshRenderer>,
        Option<&PointLight>,
        Option<&DirectionalLight>,
        Option<&SpotLight>,
        Option<&Camera>,
        Option<&Parent>,
        Option<&Tags>,
        Option<&Visible>,
        Option<&Material>,
        (
            Option<&PrimitiveMesh>,
            Option<&bsengine_scene::ScriptPath>,
            Option<&bsengine_core::TexturePath>,
            Option<&bsengine_scene::PhysicsBodyDesc>,
            Option<&bsengine_core::PrefabInstance>,
        ),
    )>,
) {
    let selection = selection_res.0.lock().unwrap().clone();
    let mut snapshot = snapshot_res.0.lock().unwrap();
    snapshot.entities = query
        .iter()
        .map(
            |(
                e,
                name,
                transform,
                mesh,
                pt,
                dir,
                spot,
                cam,
                parent,
                tags,
                vis,
                mat,
                (prim, script, texture, physics_body, prefab_instance),
            )| {
                let light_type = if pt.is_some() {
                    Some("point".to_string())
                } else if dir.is_some() {
                    Some("directional".to_string())
                } else if spot.is_some() {
                    Some("spot".to_string())
                } else {
                    None
                };
                let light_color = pt
                    .map(|l| l.color.to_array())
                    .or_else(|| dir.map(|l| l.color.to_array()))
                    .or_else(|| spot.map(|l| l.color.to_array()));
                let light_intensity = pt
                    .map(|l| l.intensity)
                    .or_else(|| spot.map(|l| l.intensity));
                let light_range = pt.map(|l| l.range).or_else(|| spot.map(|l| l.range));
                EntityInfo {
                    id: e.index() as u64,
                    name: name.map(|n| n.0.clone()),
                    position: transform.map(|t| t.position.to_array()),
                    rotation: transform.map(|t| {
                        let (rx, ry, rz) = t.rotation.to_euler(glam::EulerRot::XYZ);
                        [rx.to_degrees(), ry.to_degrees(), rz.to_degrees()]
                    }),
                    scale: transform.map(|t| t.scale.to_array()),
                    mesh_id: mesh.map(|m| m.mesh_id),
                    primitive: prim.map(|p| p.0.clone()),
                    script_path: script.map(|s| s.0.clone()),
                    texture_path: texture.map(|t| t.0.clone()),
                    light_type,
                    light_color,
                    light_intensity,
                    light_range,
                    light_ambient: dir.map(|l| l.ambient.to_array()),
                    spot_inner_angle: spot.map(|l| l.inner_angle_degrees.0),
                    spot_outer_angle: spot.map(|l| l.outer_angle_degrees.0),
                    camera_fov: cam.map(|c| c.fov_y_degrees.0),
                    material_base_color: mat.map(|m| m.base_color.to_array()),
                    material_opacity: mat.map(|m| m.opacity),
                    material_metallic: mat.map(|m| m.metallic),
                    material_roughness: mat.map(|m| m.roughness),
                    material_emissive: mat.map(|m| m.emissive.to_array()),
                    parent_id: parent.map(|p| p.0.index() as u64),
                    tags: tags.map(|t| t.0.clone()).unwrap_or_default(),
                    visible: vis.map(|v| v.is_visible).unwrap_or(true),
                    selected: selection.contains(&(e.index() as u64)),
                    extra_components: Vec::new(),
                    physics_body: physics_body.cloned(),
                    is_prefab_instance: prefab_instance.is_some(),
                }
            },
        )
        .collect();
}

/// Clones every reflected component attached to `InspectorState.selected_id`
/// into `InspectorState.reflected_components`, so the Inspector UI (which
/// has no `&mut World` access) can render and edit them via
/// `draw_reflect_ui`. Runs every frame; exclusive (`world: &mut World`)
/// because reading a component generically by `TypeRegistry` entry —
/// without knowing its concrete type at compile time — needs
/// `ReflectComponent::reflect`, which takes a `FilteredEntityRef` obtained
/// from `world.entity(...)`, mirroring the same "full world access needed
/// for generic reflection" precedent `process_reflect_commands` already
/// established.
pub(super) fn populate_reflected_component_snapshot(world: &mut World) {
    let Some(selected_id) = world
        .get_resource::<InspectorState>()
        .and_then(|insp| insp.selected_id)
    else {
        if let Some(mut insp) = world.get_resource_mut::<InspectorState>() {
            insp.reflected_components.clear();
        }
        return;
    };

    let Some(entity) = world
        .iter_entities()
        .find(|e| e.id().index() as u64 == selected_id)
        .map(|e| e.id())
    else {
        if let Some(mut insp) = world.get_resource_mut::<InspectorState>() {
            insp.reflected_components.clear();
        }
        return;
    };

    let Some(app_registry) = world
        .get_resource::<bevy_ecs::reflect::AppTypeRegistry>()
        .cloned()
    else {
        return;
    };
    let registry = app_registry.read();

    let mut cloned: Vec<(String, Box<dyn bevy_reflect::Reflect>)> = Vec::new();
    {
        let entity_ref = world.entity(entity);
        for registration in registry.iter() {
            let Some(reflect_component) =
                registration.data::<bevy_ecs::reflect::ReflectComponent>()
            else {
                continue;
            };
            if let Some(value) = reflect_component.reflect(entity_ref) {
                // `Reflect::clone_value()` on a derive-macro struct returns a
                // `Box<DynamicStruct>` proxy (see `bevy_reflect_derive`'s
                // struct impl), not a `Box<Camera>` — it does not downcast
                // back to the concrete type, which the Inspector UI (and the
                // round trip back through `ApplyReflectedComponent`) needs.
                // `ReflectFromReflect` (auto-registered by `derive(Reflect)`
                // via `FromReflect`, unless a type explicitly opts out) does
                // produce a real `Box<Camera>` boxed as `dyn Reflect`, so
                // prefer it; fall back to `clone_value()` only if a type
                // somehow lacks that registration.
                let cloned_value = registration
                    .data::<bevy_reflect::ReflectFromReflect>()
                    .and_then(|rfr| rfr.from_reflect(value))
                    .unwrap_or_else(|| value.clone_value());
                cloned.push((
                    registration.type_info().type_path().to_string(),
                    cloned_value,
                ));
            }
        }
    }

    if let Some(mut insp) = world.get_resource_mut::<InspectorState>() {
        insp.reflected_components = cloned;
    }
}

/// Reads the import settings of the asset selected in the Asset Browser
/// into `InspectorState::asset_import`, for the Inspector to draw.
///
/// Only when the selection names an asset the snapshot does not already
/// describe -- once per selection, and once more after a write drops the
/// snapshot -- rather than every frame: the sidecar is a file, and reading
/// it sixty times a second to show the same four checkboxes is what would
/// make the Inspector the one panel that touches the disk while idle. An
/// error is likewise recorded once and shown until the author clicks the
/// asset again (`InspectorState::select_asset` clears it), since re-reading
/// a broken sidecar every frame would warn every frame.
///
/// The path is used as the browser handed it out, relative to the editor's
/// working directory, which is where the browser's own `assets_root()`
/// resolves too.
pub(super) fn populate_asset_import_snapshot(inspector: Option<ResMut<InspectorState>>) {
    let Some(mut inspector) = inspector else {
        return;
    };
    let Some(path) = inspector.selected_asset.clone() else {
        return;
    };
    let already = inspector
        .asset_import
        .as_ref()
        .is_some_and(|snapshot| snapshot.path == path);
    if already || inspector.asset_import_error.is_some() {
        return;
    }
    match bsengine_asset::identity::read_import_settings(std::path::Path::new(&path)) {
        Ok(report) => {
            inspector.asset_import = Some(bsengine_core::AssetImportSnapshot {
                path,
                settings: report.settings,
                recorded: report.recorded,
                edit: report.settings,
            });
        }
        Err(e) => {
            tracing::warn!("import settings for {path} could not be read: {e}");
            inspector.asset_import_error = Some(e.to_string());
        }
    }
}

/// Walks the project for what references the selected asset and what it
/// references, into `InspectorState::asset_references`, for the Inspector
/// to list -- Unreal's Reference Viewer and Godot's View Owners.
///
/// The walk is the packager's (`bsengine_asset::cook::cook_project`): the
/// same static pass over every scene reachable from `project.toml`'s entry
/// scene, following prefabs, reflected components and quoted paths in
/// scripts. Reusing it is what makes "nothing references this" here mean
/// exactly "a packaged build leaves this out" there, rather than two walkers
/// that could disagree.
///
/// Once per selection, like `populate_asset_import_snapshot`, since the
/// walk reads every scene file; `InspectorState::select_asset` drops the
/// snapshot on every click, so re-clicking is the refresh after a save. A
/// walk that cannot run at all -- no manifest, or one without an entry
/// scene -- is recorded in the snapshot's `error` so it is not retried and
/// re-logged sixty times a second.
///
/// The project is `ProjectDir` when the app has one, else the working
/// directory, which is what the Asset Browser's paths are relative to.
pub(super) fn populate_asset_references_snapshot(
    inspector: Option<ResMut<InspectorState>>,
    project_dir: Option<Res<bsengine_core::ProjectDir>>,
) {
    let Some(mut inspector) = inspector else {
        return;
    };
    let Some(path) = inspector.selected_asset.clone() else {
        return;
    };
    let already = inspector
        .asset_references
        .as_ref()
        .is_some_and(|snapshot| snapshot.path == path);
    if already {
        return;
    }
    let dir = project_dir.map_or_else(|| ".".to_string(), |d| d.0.clone());
    let snapshot = match bsengine_asset::cook::cook_project(&dir) {
        Ok(cooked) => bsengine_core::AssetReferencesSnapshot {
            referencers: cooked
                .referencers_of(&path)
                .into_iter()
                .map(str::to_string)
                .collect(),
            dependencies: cooked
                .dependencies_of(&path)
                .into_iter()
                .map(str::to_string)
                .collect(),
            reached: cooked.assets.contains(&path),
            error: None,
            path,
        },
        Err(e) => {
            tracing::warn!("references for {path} could not be walked: {e}");
            bsengine_core::AssetReferencesSnapshot {
                path,
                error: Some(e.to_string()),
                ..Default::default()
            }
        }
    };
    inspector.asset_references = Some(snapshot);
}

/// Walks the whole project into `InspectorState::asset_graph` when
/// `asset_graph_refresh` is set, for the References panel, and clears the
/// flag. Same walk as `populate_asset_references_snapshot`; that one is
/// the selected asset's corner, this is the map. On demand rather than per
/// frame or per selection because the walk reads every scene file, and a
/// flag rather than a dropped snapshot so the panel keeps drawing the old
/// graph until the new one is ready.
pub(super) fn populate_asset_graph_snapshot(
    inspector: Option<ResMut<InspectorState>>,
    project_dir: Option<Res<bsengine_core::ProjectDir>>,
) {
    let Some(mut inspector) = inspector else {
        return;
    };
    if !inspector.asset_graph_refresh {
        return;
    }
    inspector.asset_graph_refresh = false;
    let dir = project_dir.map_or_else(|| ".".to_string(), |d| d.0.clone());
    let snapshot = match bsengine_asset::cook::cook_project(&dir) {
        Ok(cooked) => bsengine_core::AssetGraphSnapshot {
            edges: cooked.edges.into_iter().collect(),
            unreferenced: cooked.unreferenced.into_iter().collect(),
            missing: cooked
                .missing
                .into_iter()
                .map(|m| (m.referrer, m.path))
                .collect(),
            error: None,
        },
        Err(e) => {
            tracing::warn!("the asset graph could not be walked: {e}");
            bsengine_core::AssetGraphSnapshot {
                error: Some(e.to_string()),
                ..Default::default()
            }
        }
    };
    inspector.asset_graph = Some(snapshot);
}

/// Component types excluded from `populate_snapshot_extra_components`'s
/// generic capture: `Transform`/`Camera`/`PointLight`/`DirectionalLight`/
/// `SpotLight`/`Material` already have a dedicated `EntityInfo` field above,
/// and `Parent`/`Follow`/`LookAt` hold a raw `Entity` reference that would
/// point at the wrong entity after a scene reload reassigns indices (unlike
/// `ReflectCommand::ApplyComponentValue`'s `fixup_entity_fields`, there is
/// no live world to re-resolve against during a scene *load*, since the
/// referenced entity may not have spawned yet). `GlobalTransform` is
/// recomputed every frame by `propagate_global_transforms`, not authored
/// state.
pub(crate) fn excluded_from_extra_components() -> std::collections::HashSet<std::any::TypeId> {
    [
        std::any::TypeId::of::<Transform>(),
        std::any::TypeId::of::<GlobalTransform>(),
        std::any::TypeId::of::<Camera>(),
        std::any::TypeId::of::<PointLight>(),
        std::any::TypeId::of::<DirectionalLight>(),
        std::any::TypeId::of::<SpotLight>(),
        std::any::TypeId::of::<Material>(),
        std::any::TypeId::of::<Parent>(),
        std::any::TypeId::of::<bsengine_core::Follow>(),
        std::any::TypeId::of::<bsengine_core::LookAt>(),
        std::any::TypeId::of::<Visible>(),
        std::any::TypeId::of::<Tags>(),
        std::any::TypeId::of::<PrimitiveMesh>(),
        std::any::TypeId::of::<bsengine_scene::ScriptPath>(),
        std::any::TypeId::of::<bsengine_gltf::GltfAsset>(),
        std::any::TypeId::of::<bsengine_core::TexturePath>(),
    ]
    .into_iter()
    .collect()
}

/// Serializes every other registered reflected component attached to each
/// entity (skipping `excluded_from_extra_components`) into
/// `EntityInfo.extra_components`, so both the `save_scene` MCP tool (which
/// reads the cached snapshot directly) and `EditorCommand::SaveScene` can
/// round-trip components with no dedicated field -- e.g. `NavMeshAgent`,
/// `Shield`, `Bloom` -- through `EntityDescriptor.components`. Only ordered
/// `.after(update_editor_snapshot)`, which rebuilds `snapshot.entities` from
/// scratch every frame (this system would otherwise be wiped by that
/// overwrite) -- deliberately left unordered against
/// `process_editor_commands`/`process_reflect_commands` so it doesn't
/// invert this codebase's existing (implicit, tie-break-order) expectation
/// that a queued command's effect on `Transform` et al. is visible in the
/// snapshot after a single `app.update()`; a component attached this frame
/// via `ReflectCommand` similarly needs one more `app.update()` before it
/// shows up in `extra_components`, exactly like `set_reflected_component`
/// already needs a second `app.update()` before the component is visible on
/// the entity at all. Exclusive (`world: &mut World`) for the same reason
/// `populate_reflected_component_snapshot` is: reading a component
/// generically by `TypeRegistry` entry needs `ReflectComponent::reflect`,
/// which takes an entity reference obtained from the world.
///
/// This system's implicit tie-break against `process_editor_commands` is
/// fragile enough that `EditorPlugin::build`'s `PrefabWatcherPlugin`
/// registration has to stay positioned after it (see the comment at that
/// call site) -- moving either one can silently flip which side of the
/// tie-break wins.
pub(super) fn populate_snapshot_extra_components(world: &mut World) {
    let Some(snapshot_res) = world.get_resource::<EditorSnapshotResource>() else {
        return;
    };
    let Some(app_registry) = world
        .get_resource::<bevy_ecs::reflect::AppTypeRegistry>()
        .cloned()
    else {
        return;
    };
    let excluded = excluded_from_extra_components();
    let registry = app_registry.read();
    let mut snapshot = snapshot_res.0.lock().unwrap();

    let index_by_id: std::collections::HashMap<u64, usize> = snapshot
        .entities
        .iter()
        .enumerate()
        .map(|(idx, e)| (e.id, idx))
        .collect();

    for entity_ref in world.iter_entities() {
        let id = entity_ref.id().index() as u64;
        let Some(&idx) = index_by_id.get(&id) else {
            continue;
        };
        let mut extra = Vec::new();
        for registration in registry.iter() {
            if excluded.contains(&registration.type_id()) {
                continue;
            }
            let Some(reflect_component) =
                registration.data::<bevy_ecs::reflect::ReflectComponent>()
            else {
                continue;
            };
            let Some(value) = reflect_component.reflect(entity_ref) else {
                continue;
            };
            let serializer = bevy_reflect::serde::TypedReflectSerializer::new(value, &registry);
            match ron::ser::to_string(&serializer) {
                Ok(ron_str) => {
                    extra.push((registration.type_info().type_path().to_string(), ron_str));
                }
                Err(e) => tracing::warn!(
                    "editor snapshot: failed to serialize component '{}' on entity {id}: {e}",
                    registration.type_info().type_path()
                ),
            }
        }
        snapshot.entities[idx].extra_components = extra;
    }
}

/// Fills `InspectorEntityInfo::particles` for every entity that has an
/// emitter, from the live component, so the Particles panel shows this
/// frame's alive count.
///
/// After `populate_inspector`, which rebuilds `inspector.entities` from the
/// cached snapshot every frame and would wipe this. Read here rather than
/// captured into `EntityInfo` by `update_editor_snapshot` because that
/// system's query tuple is already at Bevy's arity ceiling and nested once
/// to get there, and an alive count is not scene state a save would want.
pub(super) fn populate_snapshot_particles(
    inspector: Option<ResMut<InspectorState>>,
    emitters: Query<(Entity, &bsengine_core::ParticleEmitter)>,
) {
    let Some(mut inspector) = inspector else {
        return;
    };
    for (entity, emitter) in emitters.iter() {
        let id = entity.index() as u64;
        if let Some(info) = inspector.entities.iter_mut().find(|e| e.id == id) {
            info.particles = Some(bsengine_core::ParticleSnapshot {
                alive: emitter.live.len(),
                rate: emitter.rate,
                burst_count: emitter.burst_count,
                enabled: emitter.enabled,
            });
        }
    }
}
