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
    InspectorEntityInfo, InspectorState,
    Material, Parent, PointLight, SpotLight, Transform, Visible,
};
use bsengine_ecs::Res;
use bsengine_mcp::{McpRegistryResource, McpTool, McpToolOutput};
use bsengine_render::MeshRenderer;
use bsengine_scene::{EntityDescriptor, Name, PrimitiveMesh, SceneDescriptor};
use serde_json::json;
use std::sync::{Arc, Mutex};

fn update_editor_snapshot(
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
            |(e, name, transform, mesh, pt, dir, spot, cam, parent, tags, vis, mat, (prim, script, texture, physics_body, prefab_instance))| {
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

const MAX_UNDO_HISTORY: usize = 100;

/// Puts an emitter back at its start: every live particle dropped, the
/// fractional spawn carry cleared, and -- for a burst-only effect -- its
/// burst queued again, since the burst is what that effect's start looks
/// like and a restart that left it empty would read as a delete. A
/// continuous effect simply resumes emitting from nothing, which is what
/// Unity's Restart and Godot's `restart()` both do.
fn restart_emitter(emitter: &mut bsengine_core::ParticleEmitter) {
    emitter.live.clear();
    emitter.spawn_debt = 0.0;
    emitter.pending_burst = 0;
    if emitter.rate <= 0.0 {
        emitter.burst();
    }
}

#[allow(clippy::too_many_arguments)] // Bevy system params; splitting into a struct is a larger refactor
fn process_editor_commands(
    queue_res: Res<EditorCommandQueueResource>,
    snapshot_res: Res<EditorSnapshotResource>,
    history_res: Res<EditorHistoryResource>,
    type_registry_res: Res<bevy_ecs::reflect::AppTypeRegistry>,
    project_dir: Option<Res<bsengine_core::ProjectDir>>,
    mut inspector: Option<ResMut<InspectorState>>,
    mut params: ParamSet<(
        Query<Entity>,
        Query<(Entity, &mut Transform)>,
        Query<(Entity, &mut PointLight)>,
        Query<(Entity, &mut DirectionalLight)>,
        Query<(Entity, &mut SpotLight)>,
        Query<(Entity, &mut Camera)>,
        Query<(Entity, &mut Tags)>,
        Query<(Entity, &mut Material)>,
    )>,
    mut commands: Commands,
) {
    let cmds: Vec<EditorCommand> = {
        let mut queue = queue_res.0.lock().unwrap();
        queue.drain(..).collect()
    };

    if !cmds.is_empty() {
        let checkpoint = snapshot_res.0.lock().unwrap().clone();
        let mut history = history_res.0.lock().unwrap();
        history.undo_stack.push(checkpoint);
        history.redo_stack.clear();
        if history.undo_stack.len() > MAX_UNDO_HISTORY {
            history.undo_stack.remove(0);
        }
    }

    for cmd in cmds {
        match cmd {
            EditorCommand::SpawnNamed(name) => {
                commands.spawn(Name(name));
            }
            EditorCommand::SpawnMeshAsset { name, path } => {
                let resolved = bsengine_core::resolve_project_path(project_dir.as_deref(), &path);
                commands.spawn((Name(name), bsengine_gltf::GltfAsset::new(resolved)));
            }
            EditorCommand::SpawnTerrain {
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
                let resolved =
                    bsengine_core::resolve_project_path(project_dir.as_deref(), &heightmap_path);
                let layer0_resolved = bsengine_core::resolve_project_path(
                    project_dir.as_deref(),
                    &layer0_texture_path,
                );
                let layer1_resolved = bsengine_core::resolve_project_path(
                    project_dir.as_deref(),
                    &layer1_texture_path,
                );
                let layer2_resolved = bsengine_core::resolve_project_path(
                    project_dir.as_deref(),
                    &layer2_texture_path,
                );
                let layer3_resolved = bsengine_core::resolve_project_path(
                    project_dir.as_deref(),
                    &layer3_texture_path,
                );
                let splatmap_resolved = splatmap_path
                    .map(|p| bsengine_core::resolve_project_path(project_dir.as_deref(), &p));
                // `bsengine_scene::Terrain`, not `bsengine_app::terrain::Terrain`
                // (though `bsengine-app`'s module re-exports the same type under
                // that path): `bsengine-app` depends on `bsengine-editor`, so
                // this crate cannot depend on `bsengine-app` without a cycle.
                // See `bsengine_scene::Terrain`'s doc comment.
                commands.spawn((
                    bsengine_scene::Terrain {
                        heightmap_path: resolved,
                        chunk_count,
                        chunk_size,
                        height_scale,
                        layer0_texture_path: layer0_resolved,
                        layer1_texture_path: layer1_resolved,
                        layer2_texture_path: layer2_resolved,
                        layer3_texture_path: layer3_resolved,
                        splatmap_path: splatmap_resolved,
                    },
                    Transform::default(),
                    GlobalTransform::default(),
                ));
            }
            EditorCommand::Despawn { entity_id } => {
                let target = params.p0().iter().find(|e| e.index() as u64 == entity_id);
                if let Some(entity) = target {
                    commands.entity(entity).despawn();
                }
            }
            EditorCommand::SetPosition { entity_id, x, y, z } => {
                for (e, mut t) in params.p1().iter_mut() {
                    if e.index() as u64 == entity_id {
                        t.position = glam::Vec3::new(x, y, z).into();
                        break;
                    }
                }
            }
            EditorCommand::AttachMeshRenderer { entity_id, mesh_id } => {
                let target = params.p0().iter().find(|e| e.index() as u64 == entity_id);
                if let Some(entity) = target {
                    commands
                        .entity(entity)
                        .insert((MeshRenderer { mesh_id }, GlobalTransform::default()));
                }
            }
            EditorCommand::DetachMeshRenderer { entity_id } => {
                let target = params.p0().iter().find(|e| e.index() as u64 == entity_id);
                if let Some(entity) = target {
                    commands.entity(entity).remove::<MeshRenderer>();
                }
            }
            EditorCommand::AttachPhysicsBody {
                entity_id,
                rigidbody,
                collider,
            } => {
                let target = params.p0().iter().find(|e| e.index() as u64 == entity_id);
                if let Some(entity) = target {
                    commands
                        .entity(entity)
                        .insert(bsengine_scene::PhysicsBodyDesc {
                            rigidbody,
                            collider,
                            linear_damping: None,
                            angular_damping: None,
                        });
                }
            }
            EditorCommand::DetachPhysicsBody { entity_id } => {
                let target = params.p0().iter().find(|e| e.index() as u64 == entity_id);
                if let Some(entity) = target {
                    commands
                        .entity(entity)
                        .remove::<bsengine_scene::PhysicsBodyDesc>()
                        .remove::<bsengine_physics::RigidBody>()
                        .remove::<bsengine_physics::Collider>()
                        .remove::<bsengine_physics::PhysicsInput>();
                }
            }
            EditorCommand::SpawnPointLight {
                color,
                intensity,
                range,
                position,
            } => {
                commands.spawn((
                    PointLight {
                        color: glam::Vec3::from(color).into(),
                        intensity,
                        range,
                    },
                    Transform::from_position(glam::Vec3::from(position)),
                    GlobalTransform::default(),
                ));
            }
            EditorCommand::SpawnDirectionalLight {
                direction,
                color,
                ambient,
            } => {
                let dir = glam::Vec3::from(direction).normalize_or(glam::Vec3::NEG_Z);
                let rotation = glam::Quat::from_rotation_arc(glam::Vec3::NEG_Z, dir);
                commands.spawn((
                    DirectionalLight {
                        color: glam::Vec3::from(color).into(),
                        ambient: glam::Vec3::from(ambient).into(),
                    },
                    Transform {
                        rotation: rotation.into(),
                        ..Default::default()
                    },
                    GlobalTransform::default(),
                ));
            }
            EditorCommand::RemoveLight { entity_id } => {
                let target = params.p0().iter().find(|e| e.index() as u64 == entity_id);
                if let Some(entity) = target {
                    commands
                        .entity(entity)
                        .remove::<PointLight>()
                        .remove::<DirectionalLight>()
                        .remove::<SpotLight>();
                }
            }
            EditorCommand::UpdatePointLight {
                entity_id,
                color,
                intensity,
                range,
            } => {
                for (e, mut light) in params.p2().iter_mut() {
                    if e.index() as u64 == entity_id {
                        if let Some(c) = color {
                            light.color = glam::Vec3::from(c).into();
                        }
                        if let Some(i) = intensity {
                            light.intensity = i;
                        }
                        if let Some(r) = range {
                            light.range = r;
                        }
                        break;
                    }
                }
            }
            EditorCommand::UpdateDirectionalLight {
                entity_id,
                direction,
                color,
                ambient,
            } => {
                for (e, mut light) in params.p3().iter_mut() {
                    if e.index() as u64 == entity_id {
                        if let Some(c) = color {
                            light.color = glam::Vec3::from(c).into();
                        }
                        if let Some(a) = ambient {
                            light.ambient = glam::Vec3::from(a).into();
                        }
                        break;
                    }
                }
                if let Some(d) = direction {
                    let dir = glam::Vec3::from(d).normalize_or(glam::Vec3::NEG_Z);
                    let rotation = glam::Quat::from_rotation_arc(glam::Vec3::NEG_Z, dir);
                    for (e, mut t) in params.p1().iter_mut() {
                        if e.index() as u64 == entity_id {
                            t.rotation = rotation.into();
                            break;
                        }
                    }
                }
            }
            EditorCommand::RenameEntity { entity_id, name } => {
                let target = params.p0().iter().find(|e| e.index() as u64 == entity_id);
                if let Some(entity) = target {
                    commands.entity(entity).insert(Name(name));
                }
            }
            EditorCommand::ClearScene => {
                let entities: Vec<_> = params.p0().iter().collect();
                for entity in entities {
                    commands.entity(entity).despawn();
                }
            }
            EditorCommand::SpawnCamera {
                fov_y_degrees,
                position,
            } => {
                commands.spawn((
                    Camera::perspective(fov_y_degrees, 16.0 / 9.0),
                    Transform::from_position(glam::Vec3::from(position)),
                    GlobalTransform::default(),
                ));
            }
            EditorCommand::UpdateCamera {
                entity_id,
                fov_y_degrees,
            } => {
                for (e, mut cam) in params.p5().iter_mut() {
                    if e.index() as u64 == entity_id {
                        if let Some(fov) = fov_y_degrees {
                            cam.fov_y_degrees = fov.into();
                        }
                        break;
                    }
                }
            }
            EditorCommand::DuplicateEntity { entity_id } => {
                let info = {
                    let snapshot = snapshot_res.0.lock().unwrap();
                    snapshot
                        .entities
                        .iter()
                        .find(|e| e.id == entity_id)
                        .cloned()
                };
                if let Some(info) = info {
                    let mut entity = commands.spawn_empty();
                    if let Some(name) = info.name {
                        entity.insert(Name(format!("{name} (copy)")));
                    }
                    if let Some([x, y, z]) = info.position {
                        entity.insert((
                            Transform::from_position(glam::Vec3::new(x, y, z)),
                            GlobalTransform::default(),
                        ));
                    }
                    if let Some(mesh_id) = info.mesh_id {
                        entity.insert(MeshRenderer { mesh_id });
                    }
                    if let Some(fov) = info.camera_fov {
                        entity.insert(Camera::perspective(fov, 16.0 / 9.0));
                    }
                }
            }
            EditorCommand::MoveEntity {
                entity_id,
                dx,
                dy,
                dz,
            } => {
                for (e, mut t) in params.p1().iter_mut() {
                    if e.index() as u64 == entity_id {
                        t.position.0 += glam::Vec3::new(dx, dy, dz);
                        break;
                    }
                }
            }
            EditorCommand::SetVisible { entity_id, visible } => {
                let target = params.p0().iter().find(|e| e.index() as u64 == entity_id);
                if let Some(entity) = target {
                    commands
                        .entity(entity)
                        .insert(Visible { is_visible: visible });
                }
            }
            // Both through a deferred world closure rather than a query in
            // `params`: the `ParamSet` above is at Bevy's eight-slot ceiling,
            // and a ninth query for a component only two commands touch is
            // not worth restructuring the set for. The closure runs when
            // `commands` flush, at the end of this system -- the same frame.
            EditorCommand::ParticleBurst { entity_id } => {
                commands.add(move |world: &mut World| {
                    let mut emitters = world.query::<(Entity, &mut bsengine_core::ParticleEmitter)>();
                    for (entity, mut emitter) in emitters.iter_mut(world) {
                        if entity.index() as u64 == entity_id {
                            emitter.burst();
                        }
                    }
                });
            }
            EditorCommand::ParticleRestart { entity_id } => {
                commands.add(move |world: &mut World| {
                    let mut emitters = world.query::<(Entity, &mut bsengine_core::ParticleEmitter)>();
                    for (entity, mut emitter) in emitters.iter_mut(world) {
                        if entity_id.is_some_and(|id| entity.index() as u64 != id) {
                            continue;
                        }
                        restart_emitter(&mut emitter);
                    }
                });
            }
            EditorCommand::SetParent {
                entity_id,
                parent_id,
            } => {
                let parent_entity = params.p0().iter().find(|e| e.index() as u64 == parent_id);
                let child_entity = params.p0().iter().find(|e| e.index() as u64 == entity_id);
                if let (Some(child), Some(parent)) = (child_entity, parent_entity) {
                    commands.entity(child).insert(Parent(parent));
                }
            }
            EditorCommand::RemoveParent { entity_id } => {
                let target = params.p0().iter().find(|e| e.index() as u64 == entity_id);
                if let Some(entity) = target {
                    commands.entity(entity).remove::<Parent>();
                }
            }
            EditorCommand::TagEntity { entity_id, tag } => {
                let existing = params
                    .p6()
                    .iter_mut()
                    .find(|(e, _)| e.index() as u64 == entity_id)
                    .map(|(_, mut t)| {
                        if !t.0.contains(&tag) {
                            t.0.push(tag.clone());
                        }
                        true
                    });
                if existing.is_none() {
                    let target = params.p0().iter().find(|e| e.index() as u64 == entity_id);
                    if let Some(entity) = target {
                        commands.entity(entity).insert(Tags(vec![tag]));
                    }
                }
            }
            EditorCommand::UntagEntity { entity_id, tag } => {
                for (e, mut t) in params.p6().iter_mut() {
                    if e.index() as u64 == entity_id {
                        t.0.retain(|s| s != &tag);
                        break;
                    }
                }
            }
            EditorCommand::SetRotation {
                entity_id,
                rx,
                ry,
                rz,
            } => {
                for (e, mut t) in params.p1().iter_mut() {
                    if e.index() as u64 == entity_id {
                        t.rotation = glam::Quat::from_euler(
                            glam::EulerRot::XYZ,
                            rx.to_radians(),
                            ry.to_radians(),
                            rz.to_radians(),
                        )
                        .into();
                        break;
                    }
                }
            }
            EditorCommand::SetScale {
                entity_id,
                sx,
                sy,
                sz,
            } => {
                for (e, mut t) in params.p1().iter_mut() {
                    if e.index() as u64 == entity_id {
                        t.scale = glam::Vec3::new(sx, sy, sz).into();
                        break;
                    }
                }
            }
            EditorCommand::SetEntityTransform {
                entity_id,
                position,
                rotation,
                scale,
            } => {
                for (e, mut t) in params.p1().iter_mut() {
                    if e.index() as u64 == entity_id {
                        if let Some([x, y, z]) = position {
                            t.position = glam::Vec3::new(x, y, z).into();
                        }
                        if let Some([rx, ry, rz]) = rotation {
                            t.rotation = glam::Quat::from_euler(
                                glam::EulerRot::XYZ,
                                rx.to_radians(),
                                ry.to_radians(),
                                rz.to_radians(),
                            )
                            .into();
                        }
                        if let Some([sx, sy, sz]) = scale {
                            t.scale = glam::Vec3::new(sx, sy, sz).into();
                        }
                        break;
                    }
                }
            }
            EditorCommand::BatchSpawn { entries } => {
                for (name, pos) in entries {
                    if let Some([x, y, z]) = pos {
                        commands.spawn((
                            Name(name),
                            Transform::from_position(glam::Vec3::new(x, y, z)),
                        ));
                    } else {
                        commands.spawn(Name(name));
                    }
                }
            }
            EditorCommand::SpawnSpotLight {
                color,
                intensity,
                range,
                inner_angle,
                outer_angle,
                position,
            } => {
                commands.spawn((
                    SpotLight {
                        color: glam::Vec3::from(color).into(),
                        intensity,
                        range,
                        inner_angle_degrees: inner_angle.to_degrees().into(),
                        outer_angle_degrees: outer_angle.to_degrees().into(),
                    },
                    Transform::from_position(glam::Vec3::from(position)),
                    GlobalTransform::default(),
                ));
            }
            EditorCommand::UpdateSpotLight {
                entity_id,
                color,
                intensity,
                range,
                inner_angle,
                outer_angle,
            } => {
                for (e, mut light) in params.p4().iter_mut() {
                    if e.index() as u64 == entity_id {
                        if let Some(c) = color {
                            light.color = glam::Vec3::from(c).into();
                        }
                        if let Some(i) = intensity {
                            light.intensity = i;
                        }
                        if let Some(r) = range {
                            light.range = r;
                        }
                        if let Some(a) = inner_angle {
                            light.inner_angle_degrees = a.to_degrees().into();
                        }
                        if let Some(o) = outer_angle {
                            light.outer_angle_degrees = o.to_degrees().into();
                        }
                        break;
                    }
                }
            }
            EditorCommand::AttachScript { entity_id, path } => {
                let target = params.p0().iter().find(|e| e.index() as u64 == entity_id);
                if let Some(entity) = target {
                    commands
                        .entity(entity)
                        .insert(bsengine_scene::ScriptPath(path));
                }
            }
            EditorCommand::DetachScript { entity_id } => {
                let target = params.p0().iter().find(|e| e.index() as u64 == entity_id);
                if let Some(entity) = target {
                    commands.entity(entity).remove::<bsengine_scene::ScriptPath>();
                }
            }
            EditorCommand::AttachPrimitiveMesh { entity_id, primitive } => {
                let target = params.p0().iter().find(|e| e.index() as u64 == entity_id);
                if let Some(entity) = target {
                    commands
                        .entity(entity)
                        .insert(bsengine_scene::PrimitiveMesh(primitive));
                }
            }
            EditorCommand::DetachPrimitiveMesh { entity_id } => {
                let target = params.p0().iter().find(|e| e.index() as u64 == entity_id);
                if let Some(entity) = target {
                    // `resolve_primitives` (bsengine-runtime) reacts to
                    // `Added<PrimitiveMesh>` by inserting a derived
                    // `MeshRenderer` -- remove both here, or the entity keeps
                    // rendering the stale mesh after this "detach".
                    commands
                        .entity(entity)
                        .remove::<bsengine_scene::PrimitiveMesh>()
                        .remove::<MeshRenderer>();
                }
            }
            EditorCommand::AttachPointLight {
                entity_id,
                color,
                intensity,
                range,
            } => {
                let target = params.p0().iter().find(|e| e.index() as u64 == entity_id);
                if let Some(entity) = target {
                    commands.entity(entity).insert(PointLight {
                        color: glam::Vec3::from(color).into(),
                        intensity,
                        range,
                    });
                }
            }
            EditorCommand::AttachCamera { entity_id, fov_y_degrees } => {
                let target = params.p0().iter().find(|e| e.index() as u64 == entity_id);
                if let Some(entity) = target {
                    commands
                        .entity(entity)
                        .insert(Camera::perspective(fov_y_degrees, 16.0 / 9.0));
                }
            }
            EditorCommand::LoadScene(path) => {
                let content = match std::fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!("load_scene: failed to read {path}: {e}");
                        continue;
                    }
                };
                let scene: SceneDescriptor = match ron::from_str(&content) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!("load_scene: failed to parse {path}: {e}");
                        continue;
                    }
                };
                if let Some(insp) = inspector.as_mut() {
                    insp.current_scene_path = Some(path.clone());
                }
                for entity in scene.entities {
                    let mut eb = commands.spawn(Name(entity.name));
                    if let Some(t) = &entity.transform {
                        let rotation = glam::Quat::from_xyzw(
                            t.rotation[0],
                            t.rotation[1],
                            t.rotation[2],
                            t.rotation[3],
                        );
                        eb.insert((
                            Transform {
                                position: glam::Vec3::from(t.position).into(),
                                rotation: rotation.into(),
                                scale: glam::Vec3::from(t.scale).into(),
                            },
                            GlobalTransform::default(),
                        ));
                    }
                    if let Some(prim) = &entity.primitive {
                        eb.insert(PrimitiveMesh(prim.clone()));
                    }
                    if let Some(gltf) = &entity.gltf {
                        let resolved = bsengine_core::resolve_project_path(
                            project_dir.as_deref(),
                            gltf.path(),
                        );
                        eb.insert(bsengine_gltf::GltfAsset::new(resolved));
                    }
                    if entity.camera {
                        match entity.camera_fov {
                            Some(fov) => {
                                eb.insert(Camera::perspective(fov, 16.0 / 9.0));
                            }
                            None => {
                                eb.insert(Camera::default());
                            }
                        }
                    }
                    if let Some(dl) = &entity.directional_light {
                        eb.insert(DirectionalLight {
                            color: glam::Vec3::from(dl.color).into(),
                            ambient: glam::Vec3::from(dl.ambient).into(),
                        });
                        // Direction lives on Transform.rotation (rotation * -Z), same
                        // as SpotLight; reuse translation/scale from the scene file's
                        // own `transform:` block if one was given.
                        let dir = glam::Vec3::from(dl.direction).normalize_or(glam::Vec3::NEG_Z);
                        let rotation = glam::Quat::from_rotation_arc(glam::Vec3::NEG_Z, dir);
                        let (translation, scale) = entity
                            .transform
                            .as_ref()
                            .map(|t| (glam::Vec3::from(t.position), glam::Vec3::from(t.scale)))
                            .unwrap_or((glam::Vec3::ZERO, glam::Vec3::ONE));
                        eb.insert((
                            Transform {
                                position: translation.into(),
                                rotation: rotation.into(),
                                scale: scale.into(),
                            },
                            GlobalTransform::default(),
                        ));
                    }
                    if let Some(pl) = &entity.point_light {
                        eb.insert(PointLight {
                            color: glam::Vec3::from(pl.color).into(),
                            intensity: pl.intensity,
                            range: pl.range,
                        });
                    }
                    if let Some(sl) = &entity.spot_light {
                        eb.insert(SpotLight {
                            color: glam::Vec3::from(sl.color).into(),
                            intensity: sl.intensity,
                            range: sl.range,
                            inner_angle_degrees: sl.inner_angle_degrees.into(),
                            outer_angle_degrees: sl.outer_angle_degrees.into(),
                        });
                    }
                    if let Some(script) = &entity.script {
                        eb.insert(bsengine_scene::ScriptPath(script.path().to_string()));
                    }
                    if entity.emissive.is_some() || entity.color.is_some() {
                        eb.insert(Material {
                            emissive: entity
                                .emissive
                                .map(glam::Vec3::from)
                                .unwrap_or(glam::Vec3::ZERO)
                                .into(),
                            base_color: entity
                                .color
                                .map(glam::Vec3::from)
                                .unwrap_or(glam::Vec3::ONE)
                                .into(),
                            ..Default::default()
                        });
                    }
                    if let (Some(rb), Some(col)) = (&entity.rigidbody, &entity.collider) {
                        eb.insert(bsengine_scene::PhysicsBodyDesc {
                            rigidbody: rb.clone(),
                            collider: col.clone(),
                            linear_damping: entity.linear_damping,
                            angular_damping: entity.angular_damping,
                        });
                    }
                    for (type_path, value_ron) in &entity.components {
                        let registry = type_registry_res.read();
                        let Some(registration) = registry.get_with_type_path(type_path) else {
                            tracing::warn!(
                                "load_scene: unknown reflected type path '{type_path}'"
                            );
                            continue;
                        };
                        let de = bevy_reflect::serde::TypedReflectDeserializer::new(
                            registration,
                            &registry,
                        );
                        match ron::de::Deserializer::from_str(value_ron) {
                            Ok(mut deserializer) => {
                                match serde::de::DeserializeSeed::deserialize(de, &mut deserializer)
                                {
                                    Ok(value) => {
                                        eb.insert_reflect(value);
                                    }
                                    Err(e) => tracing::warn!(
                                        "load_scene: component '{type_path}' RON value doesn't match its shape: {e}"
                                    ),
                                }
                            }
                            Err(e) => tracing::warn!(
                                "load_scene: component '{type_path}' RON parse error: {e}"
                            ),
                        }
                    }
                }
            }
            EditorCommand::SaveScene { path } => {
                let entities = {
                    let s = snapshot_res.0.lock().unwrap();
                    build_entity_descriptors(&s.entities)
                };
                let scene = SceneDescriptor { entities, skybox: None };
                match ron::to_string(&scene) {
                    Ok(ron_str) => match std::fs::write(&path, &ron_str) {
                        Ok(()) => {
                            if let Some(insp) = inspector.as_mut() {
                                insp.current_scene_path = Some(path.clone());
                                // A saved scene is new references on disk;
                                // the References panel's graph is walked
                                // from disk, so ask for it again.
                                insp.asset_graph_refresh = true;
                            }
                        }
                        Err(e) => tracing::warn!("save_scene: write failed to {path}: {e}"),
                    },
                    Err(e) => tracing::warn!("save_scene: serialize failed: {e}"),
                }
            }
        }
    }
}

/// Restores world state to match `target`, diffing against `current` to
/// know which entities to despawn (existed now, not in target) versus spawn
/// fresh (existed in target, not now). Entities present in both are updated
/// in place.
///
/// Only fields fully captured by `EntityInfo` are ever replaced wholesale
/// (Transform, MeshRenderer, PointLight, Name, Visible, Tags, Parent).
/// Camera/DirectionalLight/SpotLight/Material carry fields `EntityInfo`
/// doesn't track (aspect/near/far, direction/ambient, cone angles,
/// texture_id) — for those, only the tracked sub-fields are patched in
/// place on an already-existing component, and a missing component is never
/// fabricated from partial data, to avoid silently losing untracked state.
fn reconcile_to_snapshot(world: &mut World, current: &EditorSnapshot, target: &EditorSnapshot) {
    let mut live_by_id: std::collections::HashMap<u64, Entity> = std::collections::HashMap::new();
    {
        let mut q = world.query::<Entity>();
        for e in q.iter(world) {
            live_by_id.insert(e.index() as u64, e);
        }
    }

    let target_ids: std::collections::HashSet<u64> =
        target.entities.iter().map(|e| e.id).collect();
    for info in &current.entities {
        if !target_ids.contains(&info.id) {
            if let Some(e) = live_by_id.remove(&info.id) {
                world.despawn(e);
            }
        }
    }

    for info in &target.entities {
        if let Some(&e) = live_by_id.get(&info.id) {
            sync_entity_to_info(world, e, info);
        } else {
            let e = spawn_entity_from_info(world, info);
            live_by_id.insert(info.id, e);
        }
    }

    // Parent links are resolved last, once every target entity (updated or
    // freshly respawned) has a live Entity handle.
    for info in &target.entities {
        let Some(&child) = live_by_id.get(&info.id) else {
            continue;
        };
        match info.parent_id.and_then(|pid| live_by_id.get(&pid).copied()) {
            Some(parent) => {
                world.entity_mut(child).insert(Parent(parent));
            }
            None => {
                world.entity_mut(child).remove::<Parent>();
            }
        }
    }
}

fn sync_entity_to_info(world: &mut World, entity: Entity, info: &EntityInfo) {
    let mut e = world.entity_mut(entity);

    match &info.name {
        Some(name) => {
            e.insert(Name(name.clone()));
        }
        None => {
            e.remove::<Name>();
        }
    }

    if let (Some(pos), Some(rot), Some(scale)) = (info.position, info.rotation, info.scale) {
        e.insert((
            Transform {
                position: glam::Vec3::from(pos).into(),
                rotation: glam::Quat::from_euler(
                    glam::EulerRot::XYZ,
                    rot[0].to_radians(),
                    rot[1].to_radians(),
                    rot[2].to_radians(),
                )
                .into(),
                scale: glam::Vec3::from(scale).into(),
            },
            GlobalTransform::default(),
        ));
    } else {
        e.remove::<Transform>();
        e.remove::<GlobalTransform>();
    }

    match info.mesh_id {
        Some(mesh_id) => {
            e.insert(MeshRenderer { mesh_id });
        }
        None => {
            e.remove::<MeshRenderer>();
        }
    }

    e.insert(Visible {
        is_visible: info.visible,
    });

    if info.tags.is_empty() {
        e.remove::<Tags>();
    } else {
        e.insert(Tags(info.tags.clone()));
    }

    match info.light_type.as_deref() {
        Some("point") => {
            e.remove::<DirectionalLight>();
            e.remove::<SpotLight>();
            e.insert(PointLight {
                color: glam::Vec3::from(info.light_color.unwrap_or([1.0; 3])).into(),
                intensity: info.light_intensity.unwrap_or(1.0),
                range: info.light_range.unwrap_or(10.0),
            });
        }
        Some("directional") => {
            e.remove::<PointLight>();
            e.remove::<SpotLight>();
            if let Some(mut dl) = e.get_mut::<DirectionalLight>() {
                if let Some(c) = info.light_color {
                    dl.color = glam::Vec3::from(c).into();
                }
            }
        }
        Some("spot") => {
            e.remove::<PointLight>();
            e.remove::<DirectionalLight>();
            if let Some(mut sl) = e.get_mut::<SpotLight>() {
                if let Some(c) = info.light_color {
                    sl.color = glam::Vec3::from(c).into();
                }
                if let Some(i) = info.light_intensity {
                    sl.intensity = i;
                }
                if let Some(r) = info.light_range {
                    sl.range = r;
                }
            }
        }
        _ => {
            e.remove::<PointLight>();
            e.remove::<DirectionalLight>();
            e.remove::<SpotLight>();
        }
    }

    match info.camera_fov {
        Some(fov_deg) => {
            if let Some(mut cam) = e.get_mut::<Camera>() {
                cam.fov_y_degrees = fov_deg.into();
            }
        }
        None => {
            e.remove::<Camera>();
        }
    }

    match info.material_base_color {
        Some(base_color) => {
            if let Some(mut mat) = e.get_mut::<Material>() {
                mat.base_color = glam::Vec3::from(base_color).into();
                if let Some(m) = info.material_metallic {
                    mat.metallic = m;
                }
                if let Some(r) = info.material_roughness {
                    mat.roughness = r;
                }
                if let Some(em) = info.material_emissive {
                    mat.emissive = glam::Vec3::from(em).into();
                }
            }
        }
        None => {
            e.remove::<Material>();
        }
    }
}

fn spawn_entity_from_info(world: &mut World, info: &EntityInfo) -> Entity {
    let mut e = world.spawn_empty();
    if let Some(name) = &info.name {
        e.insert(Name(name.clone()));
    }
    if let (Some(pos), Some(rot), Some(scale)) = (info.position, info.rotation, info.scale) {
        e.insert((
            Transform {
                position: glam::Vec3::from(pos).into(),
                rotation: glam::Quat::from_euler(
                    glam::EulerRot::XYZ,
                    rot[0].to_radians(),
                    rot[1].to_radians(),
                    rot[2].to_radians(),
                )
                .into(),
                scale: glam::Vec3::from(scale).into(),
            },
            GlobalTransform::default(),
        ));
    }
    if let Some(mesh_id) = info.mesh_id {
        e.insert(MeshRenderer { mesh_id });
    }
    if !info.visible {
        e.insert(Visible::hidden());
    }
    if !info.tags.is_empty() {
        e.insert(Tags(info.tags.clone()));
    }
    match info.light_type.as_deref() {
        Some("point") => {
            e.insert(PointLight {
                color: glam::Vec3::from(info.light_color.unwrap_or([1.0; 3])).into(),
                intensity: info.light_intensity.unwrap_or(1.0),
                range: info.light_range.unwrap_or(10.0),
            });
        }
        Some("directional") => {
            e.insert(DirectionalLight {
                color: glam::Vec3::from(info.light_color.unwrap_or([1.0; 3])).into(),
                ..DirectionalLight::default()
            });
        }
        Some("spot") => {
            e.insert(SpotLight {
                color: glam::Vec3::from(info.light_color.unwrap_or([1.0; 3])).into(),
                intensity: info.light_intensity.unwrap_or(1.0),
                range: info.light_range.unwrap_or(10.0),
                ..SpotLight::default()
            });
        }
        _ => {}
    }
    if let Some(fov_deg) = info.camera_fov {
        e.insert(Camera::perspective(fov_deg, 16.0 / 9.0));
    }
    if let Some(base_color) = info.material_base_color {
        e.insert(Material {
            base_color: glam::Vec3::from(base_color).into(),
            metallic: info.material_metallic.unwrap_or(0.0),
            roughness: info.material_roughness.unwrap_or(0.5),
            emissive: info
                .material_emissive
                .map(glam::Vec3::from)
                .unwrap_or(glam::Vec3::ZERO)
                .into(),
            ..Material::default()
        });
    }
    e.id()
}

/// Builds RON-serializable `EntityDescriptor`s from tracked snapshot entities.
/// Only named entities are included (unnamed entities aren't addressable in
/// scene files). GLTF paths are not tracked by `EntityInfo` and are
/// intentionally left `None` here.
fn build_entity_descriptors(entities: &[EntityInfo]) -> Vec<EntityDescriptor> {
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
fn save_entities_as_prefab(
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
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("could not create directory: {e}"))?;
    }
    std::fs::write(&dest, &ron_str).map_err(|e| format!("write failed: {e}"))?;
    Ok(dest)
}

/// Resolves `assets/prefabs/<name>.ron` against `project_dir`, retrying
/// with `<name>#2.ron`, `<name>#3.ron`, ... until a path that doesn't
/// already exist on disk is found -- mirrors `instantiate_prefab`'s
/// runtime `#N` instance-name suffixing, applied here to filenames.
fn resolve_unique_prefab_path(
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

fn apply_history_action(world: &mut World) {
    let is_undo = {
        let Some(mut insp) = world.get_resource_mut::<InspectorState>() else {
            return;
        };
        if insp.request_undo {
            insp.request_undo = false;
            true
        } else if insp.request_redo {
            insp.request_redo = false;
            false
        } else {
            return;
        }
    };

    let current = {
        let snap_res = world.resource::<EditorSnapshotResource>();
        let s = snap_res.0.lock().unwrap().clone();
        s
    };

    let target = {
        let hist_res = world.resource::<EditorHistoryResource>();
        let mut history = hist_res.0.lock().unwrap();
        let popped = if is_undo {
            history.undo_stack.pop()
        } else {
            history.redo_stack.pop()
        };
        let Some(target) = popped else {
            return;
        };
        if is_undo {
            history.redo_stack.push(current.clone());
        } else {
            history.undo_stack.push(current.clone());
        }
        target
    };

    reconcile_to_snapshot(world, &current, &target);

    if let Some(mut insp) = world.get_resource_mut::<InspectorState>() {
        insp.selected_id = None;
    }
    if let Some(sel_res) = world.get_resource::<EditorSelectionResource>() {
        sel_res.0.lock().unwrap().clear();
    }
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
fn populate_reflected_component_snapshot(world: &mut World) {
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

    let Some(app_registry) = world.get_resource::<bevy_ecs::reflect::AppTypeRegistry>().cloned()
    else {
        return;
    };
    let registry = app_registry.read();

    let mut cloned: Vec<(String, Box<dyn bevy_reflect::Reflect>)> = Vec::new();
    {
        let entity_ref = world.entity(entity);
        for registration in registry.iter() {
            let Some(reflect_component) = registration.data::<bevy_ecs::reflect::ReflectComponent>()
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
                cloned.push((registration.type_info().type_path().to_string(), cloned_value));
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
fn populate_asset_import_snapshot(inspector: Option<ResMut<InspectorState>>) {
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
fn populate_asset_references_snapshot(
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
fn populate_asset_graph_snapshot(
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
fn populate_snapshot_extra_components(world: &mut World) {
    let Some(snapshot_res) = world.get_resource::<EditorSnapshotResource>() else {
        return;
    };
    let Some(app_registry) = world.get_resource::<bevy_ecs::reflect::AppTypeRegistry>().cloned()
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

/// Recursively walks `value`'s reflect tree and, for every leaf field whose
/// declared type is `bevy_ecs::Entity`, replaces it with the live entity
/// that currently has the same `.index()` (if any still does). The
/// Inspector UI (`reflect_ui.rs`) only ever knows a raw `u64` index for an
/// `Entity`-typed field -- it edits a *detached clone*, with no `World`
/// access to look up the entity's real generation -- so it writes
/// `Entity::from_raw(index)` (generation 0) as a placeholder. Applying that
/// as-is could silently target the wrong (or a since-despawned-and-reused)
/// entity slot; this runs once, here, where `World` access is actually
/// available, before the value is applied to a live component.
fn fixup_entity_fields(value: &mut dyn bevy_reflect::Reflect, world: &World) {
    if let Some(entity) = value.downcast_mut::<bevy_ecs::prelude::Entity>() {
        let wanted_index = entity.index();
        if let Some(live) = world.iter_entities().find(|e| e.id().index() == wanted_index) {
            *entity = live.id();
        }
        return;
    }
    match value.reflect_mut() {
        bevy_reflect::ReflectMut::Struct(s) => {
            for i in 0..s.field_len() {
                if let Some(field) = s.field_at_mut(i) {
                    fixup_entity_fields(field, world);
                }
            }
        }
        bevy_reflect::ReflectMut::TupleStruct(ts) => {
            for i in 0..ts.field_len() {
                if let Some(field) = ts.field_mut(i) {
                    fixup_entity_fields(field, world);
                }
            }
        }
        bevy_reflect::ReflectMut::Enum(e) => {
            for i in 0..e.field_len() {
                if let Some(field) = e.field_at_mut(i) {
                    fixup_entity_fields(field, world);
                }
            }
        }
        bevy_reflect::ReflectMut::List(l) => {
            for i in 0..l.len() {
                if let Some(field) = l.get_mut(i) {
                    fixup_entity_fields(field, world);
                }
            }
        }
        bevy_reflect::ReflectMut::Array(a) => {
            for i in 0..a.len() {
                if let Some(field) = a.get_mut(i) {
                    fixup_entity_fields(field, world);
                }
            }
        }
        bevy_reflect::ReflectMut::Map(m) => {
            // `Map` only exposes a mutable reference to the *value* half of
            // each entry (`get_at_mut`) -- keys are immutable through this
            // trait, since mutating one in place would desync the map's
            // internal hash bucket for it. That's fine here: recursing into
            // the value covers the realistic shape (e.g. `HashMap<K,
            // Entity>`); a key itself being an `Entity` is an unusual design
            // this codebase doesn't use.
            for i in 0..m.len() {
                if let Some((_key, value)) = m.get_at_mut(i) {
                    fixup_entity_fields(value, world);
                }
            }
        }
        _ => {}
    }
}

fn process_reflect_commands(world: &mut World) {
    let cmds: Vec<ReflectCommand> = {
        let Some(queue_res) = world.get_resource::<ReflectCommandQueueResource>() else {
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

    let Some(app_registry) = world.get_resource::<bevy_ecs::reflect::AppTypeRegistry>().cloned()
    else {
        return;
    };
    let registry = app_registry.read();

    for cmd in cmds {
        match cmd {
            ReflectCommand::AttachComponentByType { entity_id, type_path } => {
                let Some(registration) = registry.get_with_type_path(&type_path) else {
                    tracing::warn!("reflect: unknown type path '{type_path}'");
                    continue;
                };
                let Some(reflect_component) = registration.data::<bevy_ecs::reflect::ReflectComponent>()
                else {
                    tracing::warn!("reflect: '{type_path}' is not a registered Component");
                    continue;
                };
                let Some(reflect_default) = registration.data::<bevy_reflect::std_traits::ReflectDefault>()
                else {
                    tracing::warn!("reflect: '{type_path}' has no registered Default");
                    continue;
                };
                let default_value = reflect_default.default();
                let target = world.iter_entities().find(|e| e.id().index() as u64 == entity_id);
                if let Some(entity) = target.map(|e| e.id()) {
                    let mut entity_mut = world.entity_mut(entity);
                    reflect_component.insert(&mut entity_mut, default_value.as_ref(), &registry);
                }
            }
            ReflectCommand::RemoveComponentByType { entity_id, type_path } => {
                let Some(registration) = registry.get_with_type_path(&type_path) else {
                    tracing::warn!("reflect: unknown type path '{type_path}'");
                    continue;
                };
                let Some(reflect_component) = registration.data::<bevy_ecs::reflect::ReflectComponent>()
                else {
                    continue;
                };
                let target = world.iter_entities().find(|e| e.id().index() as u64 == entity_id);
                if let Some(entity) = target.map(|e| e.id()) {
                    let mut entity_mut = world.entity_mut(entity);
                    reflect_component.remove(&mut entity_mut);
                }
            }
            ReflectCommand::ApplyComponentValue { entity_id, type_path, mut value } => {
                let Some(registration) = registry.get_with_type_path(&type_path) else {
                    tracing::warn!("reflect: unknown type path '{type_path}'");
                    continue;
                };
                let Some(reflect_component) = registration.data::<bevy_ecs::reflect::ReflectComponent>()
                else {
                    tracing::warn!("reflect: '{type_path}' is not a registered Component");
                    continue;
                };
                fixup_entity_fields(value.as_mut(), world);
                let target = world.iter_entities().find(|e| e.id().index() as u64 == entity_id);
                if let Some(entity) = target.map(|e| e.id()) {
                    let mut entity_mut = world.entity_mut(entity);
                    reflect_component.apply_or_insert(&mut entity_mut, value.as_ref(), &registry);
                }
            }
        }
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
fn process_prefab_commands(world: &mut World) {
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
fn process_prefab_apply_commands(world: &mut World) {
    let cmds: Vec<crate::snapshot::PrefabApplyCommand> = {
        let Some(queue_res) = world.get_resource::<crate::snapshot::PrefabApplyCommandQueueResource>()
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

fn update_editor_camera(
    inspector: Option<ResMut<InspectorState>>,
    mouse: Option<bsengine_ecs::Res<bsengine_input::MouseState>>,
    buttons: Option<bsengine_ecs::Res<bsengine_input::Input<bsengine_input::MouseButton>>>,
) {
    let Some(mut insp) = inspector else { return };
    if !insp.editor_mode {
        return;
    }

    if let (Some(mouse), Some(buttons)) = (mouse, buttons) {
        if insp.viewport_contains_cursor {
            let dx = mouse.delta.0 as f32;
            let dy = mouse.delta.1 as f32;
            let scroll = mouse.scroll_delta as f32;

            if buttons.is_pressed(&bsengine_input::MouseButton::Right) {
                insp.cam_yaw -= dx * 0.005;
                insp.cam_pitch = (insp.cam_pitch - dy * 0.005).clamp(-1.5, 1.5);
            }

            if buttons.is_pressed(&bsengine_input::MouseButton::Middle) {
                let right =
                    glam::Vec3::new(insp.cam_yaw.sin(), 0.0, -insp.cam_yaw.cos());
                let speed = 0.01 * insp.cam_distance;
                let target = glam::Vec3::from(insp.cam_target)
                    - right * dx * speed
                    + glam::Vec3::Y * dy * speed;
                insp.cam_target = target.to_array();
            }

            if scroll != 0.0 {
                insp.cam_distance =
                    (insp.cam_distance - scroll * insp.cam_distance * 0.1).max(0.5);
            }
        }
    }

    let aspect = if insp.viewport_size[1] > 0.0 {
        insp.viewport_size[0] / insp.viewport_size[1]
    } else {
        16.0 / 9.0
    };
    let pitch = insp.cam_pitch;
    let yaw = insp.cam_yaw;
    let dist = insp.cam_distance;
    let target = glam::Vec3::from(insp.cam_target);
    let eye = target
        + glam::Vec3::new(
            dist * yaw.cos() * pitch.cos(),
            dist * pitch.sin(),
            dist * yaw.sin() * pitch.cos(),
        );
    // The Timeline panel's preview replaces the orbit camera's answer here
    // rather than writing `editor_view_proj` itself. This system assigns it
    // unconditionally every frame, so a second writer would simply race it --
    // and the orbit parameters above are deliberately left untouched, which is
    // what makes ending a preview return the user to exactly the viewpoint
    // they had instead of stranding the camera at the cutscene.
    let preview_camera = insp
        .timeline_preview
        .as_ref()
        .and_then(|p| p.camera.clone());

    let (eye, view, proj) = match preview_camera {
        Some(camera) => {
            let eye = glam::Vec3::from(camera.position);
            let rotation = glam::Quat::from_array(camera.rotation);
            let fov = camera
                .fov_y_degrees
                .map_or(std::f32::consts::FRAC_PI_4, |d| d.to_radians());
            // The engine's cameras look down -Z, the convention
            // `CameraPose::rotation` builds against.
            let forward = rotation * glam::Vec3::NEG_Z;
            let up = rotation * glam::Vec3::Y;
            (
                eye,
                glam::Mat4::look_at_rh(eye, eye + forward, up),
                glam::Mat4::perspective_rh(fov, aspect, 0.1, 1000.0),
            )
        }
        None => (
            eye,
            glam::Mat4::look_at_rh(eye, target, glam::Vec3::Y),
            glam::Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, aspect, 0.1, 1000.0),
        ),
    };

    insp.editor_view_proj = Some((proj * view).to_cols_array_2d());
    insp.editor_proj = proj.to_cols_array_2d();
    insp.editor_cam_pos = eye.to_array();
}

fn populate_inspector(
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

/// Fills `InspectorEntityInfo::particles` for every entity that has an
/// emitter, from the live component, so the Particles panel shows this
/// frame's alive count.
///
/// After `populate_inspector`, which rebuilds `inspector.entities` from the
/// cached snapshot every frame and would wipe this. Read here rather than
/// captured into `EntityInfo` by `update_editor_snapshot` because that
/// system's query tuple is already at Bevy's arity ceiling and nested once
/// to get there, and an alive count is not scene state a save would want.
fn populate_snapshot_particles(
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

/// Exhaustive match with no wildcard arm — adding a `Primitive` variant
/// forces a compile error here. If you land here from that error, also
/// update `bsengine_core::PRIMITIVE_KINDS` and `str_to_primitive` below to
/// keep the DTO-boundary string set in sync (see the "primitive kinds test"
/// in this file's test module, which round-trip-checks that sync).
fn primitive_to_str(p: &bsengine_scene::Primitive) -> String {
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
fn str_to_primitive(s: &str) -> Option<bsengine_scene::Primitive> {
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
fn apply_inspector_cmds(
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
                queue.push(EditorCommand::SetPosition { entity_id: id, x, y, z });
            }
            InspectorCmd::SetRotation { id, rx, ry, rz } => {
                queue.push(EditorCommand::SetRotation { entity_id: id, rx, ry, rz });
            }
            InspectorCmd::SetScale { id, sx, sy, sz } => {
                queue.push(EditorCommand::SetScale { entity_id: id, sx, sy, sz });
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
                queue.push(EditorCommand::SetVisible { entity_id: id, visible });
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
                reflect_queue_res.0.lock().unwrap().push(ReflectCommand::AttachComponentByType {
                    entity_id: id,
                    type_path,
                });
            }
            InspectorCmd::RemoveComponentByType { id, type_path } => {
                reflect_queue_res.0.lock().unwrap().push(ReflectCommand::RemoveComponentByType {
                    entity_id: id,
                    type_path,
                });
            }
            InspectorCmd::RenameEntity { id, name } => {
                queue.push(EditorCommand::RenameEntity { entity_id: id, name });
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
                queue.push(EditorCommand::AttachScript { entity_id: id, path });
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
            InspectorCmd::ApplyReflectedComponent { id, type_path, value } => {
                reflect_queue_res.0.lock().unwrap().push(ReflectCommand::ApplyComponentValue {
                    entity_id: id,
                    type_path,
                    value,
                });
            }
            InspectorCmd::InstantiatePrefab { path, name, x, y, z, parent_id } => {
                prefab_queue_res.0.lock().unwrap().push(PrefabInstantiateCommand {
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
        app.add_systems(Update, populate_snapshot_particles.after(populate_inspector));
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
        app.add_systems(Update, process_reflect_commands.after(process_editor_commands));
        app.add_systems(Update, process_prefab_commands.after(process_editor_commands));
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
        app.add_systems(Update, process_prefab_apply_commands.before(apply_inspector_cmds));
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
            // list_entities
            let snap = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "list_entities".to_string(),
                description: "List all entities with their IDs, names, and positions".to_string(),
                input_schema: Some(json!({ "type": "object" })),
                handler: Box::new(move |_input| {
                    let s = snap.lock().unwrap();
                    McpToolOutput::success(json!({
                        "entities": s.entities.iter().map(|e| json!({
                            "id": e.id,
                            "name": e.name,
                            "position": e.position,
                            "mesh_id": e.mesh_id,
                            "rotation": e.rotation,
                            "scale": e.scale,
                            "parent_id": e.parent_id,
                            "tags": e.tags,
                            "visible": e.visible,
                            "selected": e.selected,
                            "light_type": e.light_type,
                            "light_color": e.light_color,
                            "light_intensity": e.light_intensity,
                            "light_range": e.light_range,
                            "camera_fov": e.camera_fov,
                        })).collect::<Vec<_>>()
                    }))
                }),
            });

            // get_entity
            let snap2 = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity".to_string(),
                description: "Get detailed info for a specific entity by ID".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "entity_id": { "type": "integer" } },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(v) => v,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let s = snap2.lock().unwrap();
                    match s.entities.iter().find(|e| e.id == entity_id) {
                        Some(e) => McpToolOutput::success(json!({ "entity": {
                            "id": e.id,
                            "name": e.name,
                            "position": e.position,
                            "rotation": e.rotation,
                            "scale": e.scale,
                            "mesh_id": e.mesh_id,
                            "light_type": e.light_type,
                            "light_color": e.light_color,
                            "light_intensity": e.light_intensity,
                            "light_range": e.light_range,
                            "camera_fov": e.camera_fov,
                            "parent_id": e.parent_id,
                            "tags": e.tags,
                            "visible": e.visible,
                            "selected": e.selected,
                        }})),
                        None => McpToolOutput::error("entity not found"),
                    }
                }),
            });

            // spawn_entity
            let queue = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "spawn_entity".to_string(),
                description: "Spawn a new named entity (applied next frame)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "name": { "type": "string", "description": "Entity name" } },
                    "required": ["name"]
                })),
                handler: Box::new(move |input| {
                    let name = input["name"].as_str().unwrap_or("Entity").to_string();
                    queue
                        .lock()
                        .unwrap()
                        .push(EditorCommand::SpawnNamed(name.clone()));
                    McpToolOutput::success(json!({"status": "queued", "name": name}))
                }),
            });

            // set_transform
            let queue2 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "set_transform".to_string(),
                description: "Set the world position of an entity by ID (applied next frame)"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "number", "description": "Entity ID" },
                        "x": { "type": "number" },
                        "y": { "type": "number" },
                        "z": { "type": "number" }
                    },
                    "required": ["id", "x", "y", "z"]
                })),
                handler: Box::new(move |input| {
                    let id = match input["id"].as_u64() {
                        Some(v) => v,
                        None => return McpToolOutput::error("missing numeric 'id' field"),
                    };
                    let x = input["x"].as_f64().unwrap_or(0.0) as f32;
                    let y = input["y"].as_f64().unwrap_or(0.0) as f32;
                    let z = input["z"].as_f64().unwrap_or(0.0) as f32;
                    queue2.lock().unwrap().push(EditorCommand::SetPosition {
                        entity_id: id,
                        x,
                        y,
                        z,
                    });
                    McpToolOutput::success(
                        json!({"status": "queued", "id": id, "x": x, "y": y, "z": z}),
                    )
                }),
            });

            // despawn_entity
            let queue3 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "despawn_entity".to_string(),
                description: "Despawn an entity by ID (applied next frame)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "id": { "type": "number", "description": "Entity ID" } },
                    "required": ["id"]
                })),
                handler: Box::new(move |input| {
                    let id = match input["id"].as_u64() {
                        Some(v) => v,
                        None => return McpToolOutput::error("missing numeric 'id' field"),
                    };
                    queue3
                        .lock()
                        .unwrap()
                        .push(EditorCommand::Despawn { entity_id: id });
                    McpToolOutput::success(json!({"status": "queued", "id": id}))
                }),
            });

            // save_scene
            let snap3 = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "save_scene".to_string(),
                description: "Serialize current named entities to a RON scene file".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "path": { "type": "string", "description": "Destination file path (.ron)" } },
                    "required": ["path"]
                })),
                handler: Box::new(move |input| {
                    let path = match input["path"].as_str() {
                        Some(p) => p.to_string(),
                        None => return McpToolOutput::error("missing 'path' field"),
                    };
                    let s = snap3.lock().unwrap();
                    let entities = build_entity_descriptors(&s.entities);
                    let count = entities.len();
                    let scene = SceneDescriptor { entities, skybox: None };
                    match ron::to_string(&scene) {
                        Ok(ron_str) => match std::fs::write(&path, &ron_str) {
                            Ok(()) => McpToolOutput::success(json!({
                                "status": "saved",
                                "path": path,
                                "entity_count": count,
                            })),
                            Err(e) => McpToolOutput::error(&format!("write failed: {e}")),
                        },
                        Err(e) => McpToolOutput::error(&format!("serialize failed: {e}")),
                    }
                }),
            });

            // prefab_write
            let snap_pw = snapshot.clone();
            let project_dir_pw = project_dir.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "prefab_write".to_string(),
                description: "Extract an entity and its descendants from the live scene into a \
                    new assets/prefabs/<name>.ron file. Auto-suffixes the filename (#2, #3, ...) \
                    if it already exists."
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer", "description": "Root entity id; itself and all descendants are saved" },
                        "name": { "type": "string", "description": "Prefab file name, no extension or directory (written to assets/prefabs/<name>.ron)" },
                    },
                    "required": ["entity_id", "name"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing 'entity_id' field"),
                    };
                    let name = match input["name"].as_str() {
                        Some(n) => n.to_string(),
                        None => return McpToolOutput::error("missing 'name' field"),
                    };
                    let s = snap_pw.lock().unwrap();
                    match save_entities_as_prefab(&s.entities, entity_id, &name, project_dir_pw.as_ref()) {
                        Ok(path) => McpToolOutput::success(json!({ "status": "saved", "path": path })),
                        Err(e) => McpToolOutput::error(&e),
                    }
                }),
            });

            // apply_to_prefab
            let queue_atp = prefab_apply_cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "apply_to_prefab".to_string(),
                description: "Push this prefab instance's field-level overrides back into its source .ron \
                    file. Structural changes (added/removed entities) are not promoted. Queued for \
                    processing; check the instance's state on a subsequent call to confirm the result."
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer", "description": "The prefab instance's root entity id" },
                    },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing 'entity_id' field"),
                    };
                    queue_atp
                        .lock()
                        .unwrap()
                        .push(crate::snapshot::PrefabApplyCommand { entity_id });
                    McpToolOutput::success(json!({ "status": "queued" }))
                }),
            });

            // terrain_write
            let queue_terrain = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "terrain_write".to_string(),
                description: "Spawn a new terrain entity from a heightmap (applied next frame). \
                    The existing terrain system loads the heightmap and spawns chunk children \
                    (render mesh + heightfield collider) automatically."
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "heightmap_path": { "type": "string", "description": "Path to the heightmap asset (16-bit grayscale PNG)" },
                        "chunk_count": { "type": "array", "items": {"type": "integer"}, "minItems": 2, "maxItems": 2, "description": "[chunks_x, chunks_z]" },
                        "chunk_size": { "type": "number", "description": "World-space size of one chunk along each axis" },
                        "height_scale": { "type": "number", "description": "Multiplier applied to the normalized heightmap sample" },
                        "layer0_texture_path": { "type": "string", "description": "Diffuse texture for the low/flat splat layer (e.g. grass)" },
                        "layer1_texture_path": { "type": "string", "description": "Diffuse texture for the steep-slope splat layer (e.g. rock)" },
                        "layer2_texture_path": { "type": "string", "description": "Diffuse texture for the paint-only splat layer (e.g. dirt)" },
                        "layer3_texture_path": { "type": "string", "description": "Diffuse texture for the high-altitude splat layer (e.g. snow)" },
                        "splatmap_path": { "type": "string", "description": "Optional path to a whole-terrain RGBA8 splatmap image; omit to keep procedural splat generation" }
                    },
                    "required": ["heightmap_path", "chunk_count", "chunk_size", "height_scale", "layer0_texture_path", "layer1_texture_path", "layer2_texture_path", "layer3_texture_path"]
                })),
                handler: Box::new(move |input| {
                    let heightmap_path = match input["heightmap_path"].as_str() {
                        Some(p) => p.to_string(),
                        None => return McpToolOutput::error("missing 'heightmap_path' field"),
                    };
                    let chunk_count = match input["chunk_count"].as_array() {
                        Some(arr) if arr.len() == 2 => {
                            match (arr[0].as_u64(), arr[1].as_u64()) {
                                (Some(x), Some(z)) => (x as u32, z as u32),
                                _ => return McpToolOutput::error(
                                    "'chunk_count' must be an array of 2 integers",
                                ),
                            }
                        }
                        _ => return McpToolOutput::error(
                            "'chunk_count' must be an array of 2 integers",
                        ),
                    };
                    let chunk_size = match input["chunk_size"].as_f64() {
                        Some(v) => v as f32,
                        None => return McpToolOutput::error("missing numeric 'chunk_size' field"),
                    };
                    let height_scale = match input["height_scale"].as_f64() {
                        Some(v) => v as f32,
                        None => {
                            return McpToolOutput::error("missing numeric 'height_scale' field")
                        }
                    };
                    let layer0_texture_path = match input["layer0_texture_path"].as_str() {
                        Some(p) => p.to_string(),
                        None => {
                            return McpToolOutput::error("missing 'layer0_texture_path' field")
                        }
                    };
                    let layer1_texture_path = match input["layer1_texture_path"].as_str() {
                        Some(p) => p.to_string(),
                        None => {
                            return McpToolOutput::error("missing 'layer1_texture_path' field")
                        }
                    };
                    let layer2_texture_path = match input["layer2_texture_path"].as_str() {
                        Some(p) => p.to_string(),
                        None => {
                            return McpToolOutput::error("missing 'layer2_texture_path' field")
                        }
                    };
                    let layer3_texture_path = match input["layer3_texture_path"].as_str() {
                        Some(p) => p.to_string(),
                        None => {
                            return McpToolOutput::error("missing 'layer3_texture_path' field")
                        }
                    };
                    let splatmap_path = input["splatmap_path"].as_str().map(|s| s.to_string());
                    queue_terrain.lock().unwrap().push(EditorCommand::SpawnTerrain {
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
                    McpToolOutput::success(json!({"status": "queued"}))
                }),
            });

            // load_scene
            let queue4 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "load_scene".to_string(),
                description: "Load and spawn entities from a RON scene file (applied next frame)"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "path": { "type": "string", "description": "Source file path (.ron)" } },
                    "required": ["path"]
                })),
                handler: Box::new(move |input| {
                    let path = match input["path"].as_str() {
                        Some(p) => p.to_string(),
                        None => return McpToolOutput::error("missing 'path' field"),
                    };
                    queue4
                        .lock()
                        .unwrap()
                        .push(EditorCommand::LoadScene(path.clone()));
                    McpToolOutput::success(json!({"status": "queued", "path": path}))
                }),
            });

            // attach_mesh
            let queue5 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "attach_mesh".to_string(),
                description: "Attach a MeshRenderer to an entity by ID (applied next frame)"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "number", "description": "Entity ID" },
                        "mesh_id":   { "type": "number", "description": "Registered mesh ID" }
                    },
                    "required": ["entity_id", "mesh_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(v) => v,
                        None => return McpToolOutput::error("missing numeric 'entity_id' field"),
                    };
                    let mesh_id = match input["mesh_id"].as_u64() {
                        Some(v) => v,
                        None => return McpToolOutput::error("missing numeric 'mesh_id' field"),
                    };
                    queue5
                        .lock()
                        .unwrap()
                        .push(EditorCommand::AttachMeshRenderer { entity_id, mesh_id });
                    McpToolOutput::success(
                        json!({"status": "queued", "entity_id": entity_id, "mesh_id": mesh_id}),
                    )
                }),
            });

            // detach_mesh
            let queue6 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "detach_mesh".to_string(),
                description: "Remove MeshRenderer from an entity by ID (applied next frame)"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "number", "description": "Entity ID" }
                    },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(v) => v,
                        None => return McpToolOutput::error("missing numeric 'entity_id' field"),
                    };
                    queue6
                        .lock()
                        .unwrap()
                        .push(EditorCommand::DetachMeshRenderer { entity_id });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // attach_physics_body
            let queue_phys_attach = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "attach_physics_body".to_string(),
                description: "Attach a physics body (rigidbody + collider) to an entity by ID (applied next frame)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id":     { "type": "number", "description": "Entity ID" },
                        "rigidbody":     { "type": "string", "enum": ["Dynamic", "Static", "Kinematic"] },
                        "collider_shape":{ "type": "string", "enum": ["Box", "Sphere", "Capsule"] },
                        "hx":            { "type": "number", "description": "Box half-extent X" },
                        "hy":            { "type": "number", "description": "Box half-extent Y" },
                        "hz":            { "type": "number", "description": "Box half-extent Z" },
                        "radius":        { "type": "number", "description": "Sphere/Capsule radius" },
                        "half_height":   { "type": "number", "description": "Capsule half-height" },
                        "restitution":   { "type": "number", "description": "Bounciness, 0-1 (default 0.0)" },
                        "friction":      { "type": "number", "description": "Surface friction (default 0.5)" },
                        "sensor":        { "type": "boolean", "description": "Overlap-only, no physical collision (default false)" }
                    },
                    "required": ["entity_id", "rigidbody", "collider_shape"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(v) => v,
                        None => return McpToolOutput::error("missing numeric 'entity_id' field"),
                    };
                    let rigidbody = match input["rigidbody"].as_str() {
                        Some("Dynamic") => bsengine_scene::RigidBodyDesc::Dynamic,
                        Some("Static") => bsengine_scene::RigidBodyDesc::Static,
                        Some("Kinematic") => bsengine_scene::RigidBodyDesc::Kinematic,
                        _ => return McpToolOutput::error(
                            "'rigidbody' must be one of \"Dynamic\", \"Static\", \"Kinematic\"",
                        ),
                    };
                    let shape = match input["collider_shape"].as_str() {
                        Some("Box") => {
                            let hx = input["hx"].as_f64().unwrap_or(0.5) as f32;
                            let hy = input["hy"].as_f64().unwrap_or(0.5) as f32;
                            let hz = input["hz"].as_f64().unwrap_or(0.5) as f32;
                            bsengine_scene::ColliderShapeDesc::Box { hx, hy, hz }
                        }
                        Some("Sphere") => {
                            let radius = input["radius"].as_f64().unwrap_or(0.5) as f32;
                            bsengine_scene::ColliderShapeDesc::Sphere { radius }
                        }
                        Some("Capsule") => {
                            let half_height = input["half_height"].as_f64().unwrap_or(0.5) as f32;
                            let radius = input["radius"].as_f64().unwrap_or(0.3) as f32;
                            bsengine_scene::ColliderShapeDesc::Capsule { half_height, radius }
                        }
                        _ => return McpToolOutput::error(
                            "'collider_shape' must be one of \"Box\", \"Sphere\", \"Capsule\"",
                        ),
                    };
                    let collider = bsengine_scene::ColliderDesc {
                        shape,
                        restitution: input["restitution"].as_f64().unwrap_or(0.0) as f32,
                        friction: input["friction"].as_f64().unwrap_or(0.5) as f32,
                        sensor: input["sensor"].as_bool().unwrap_or(false),
                    };
                    queue_phys_attach.lock().unwrap().push(EditorCommand::AttachPhysicsBody {
                        entity_id,
                        rigidbody,
                        collider,
                    });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // detach_physics_body
            let queue_phys_detach = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "detach_physics_body".to_string(),
                description: "Remove an entity's physics body by ID (applied next frame)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "number", "description": "Entity ID" }
                    },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(v) => v,
                        None => return McpToolOutput::error("missing numeric 'entity_id' field"),
                    };
                    queue_phys_detach
                        .lock()
                        .unwrap()
                        .push(EditorCommand::DetachPhysicsBody { entity_id });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // spawn_point_light
            let queue7 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "spawn_point_light".to_string(),
                description: "Spawn a point light entity at a position (applied next frame)"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "color":     { "type": "array", "items": {"type":"number"}, "description": "[r,g,b]" },
                        "intensity": { "type": "number" },
                        "range":     { "type": "number" },
                        "position":  { "type": "array", "items": {"type":"number"}, "description": "[x,y,z]" }
                    },
                    "required": ["color", "intensity", "range", "position"]
                })),
                handler: Box::new(move |input| {
                    let color = parse_vec3_input(&input["color"]).unwrap_or([1.0, 1.0, 1.0]);
                    let intensity = input["intensity"].as_f64().unwrap_or(1.0) as f32;
                    let range = input["range"].as_f64().unwrap_or(10.0) as f32;
                    let position = parse_vec3_input(&input["position"]).unwrap_or([0.0, 0.0, 0.0]);
                    queue7.lock().unwrap().push(EditorCommand::SpawnPointLight {
                        color,
                        intensity,
                        range,
                        position,
                    });
                    McpToolOutput::success(json!({"status": "queued"}))
                }),
            });

            // spawn_directional_light
            let queue8 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "spawn_directional_light".to_string(),
                description: "Spawn a directional light (applied next frame)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "direction": { "type": "array", "items": {"type":"number"}, "description": "[x,y,z]" },
                        "color":     { "type": "array", "items": {"type":"number"}, "description": "[r,g,b]" },
                        "ambient":   { "type": "array", "items": {"type":"number"}, "description": "[r,g,b]" }
                    }
                })),
                handler: Box::new(move |input| {
                    let direction = parse_vec3_input(&input["direction"]).unwrap_or([-0.4, -0.8, -0.4]);
                    let color = parse_vec3_input(&input["color"]).unwrap_or([1.0, 1.0, 1.0]);
                    let ambient = parse_vec3_input(&input["ambient"]).unwrap_or([0.15, 0.15, 0.15]);
                    queue8.lock().unwrap().push(EditorCommand::SpawnDirectionalLight {
                        direction,
                        color,
                        ambient,
                    });
                    McpToolOutput::success(json!({"status": "queued"}))
                }),
            });

            // remove_light
            let queue9 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "remove_light".to_string(),
                description:
                    "Remove all light components from an entity by ID (applied next frame)"
                        .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "number", "description": "Entity ID" }
                    },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(v) => v,
                        None => return McpToolOutput::error("missing numeric 'entity_id' field"),
                    };
                    queue9
                        .lock()
                        .unwrap()
                        .push(EditorCommand::RemoveLight { entity_id });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // update_point_light
            let queue10 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "update_point_light".to_string(),
                description: "Update PointLight properties on an entity (all fields optional, applied next frame)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "number", "description": "Entity ID" },
                        "color":     { "type": "array", "items": {"type":"number"}, "description": "[r,g,b]" },
                        "intensity": { "type": "number" },
                        "range":     { "type": "number" }
                    },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(v) => v,
                        None => return McpToolOutput::error("missing numeric 'entity_id' field"),
                    };
                    let color = parse_vec3_input(&input["color"]);
                    let intensity = input["intensity"].as_f64().map(|v| v as f32);
                    let range = input["range"].as_f64().map(|v| v as f32);
                    queue10.lock().unwrap().push(EditorCommand::UpdatePointLight {
                        entity_id,
                        color,
                        intensity,
                        range,
                    });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // update_directional_light
            let queue11 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "update_directional_light".to_string(),
                description: "Update DirectionalLight properties on an entity (all fields optional, applied next frame)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "number", "description": "Entity ID" },
                        "direction": { "type": "array", "items": {"type":"number"}, "description": "[x,y,z]" },
                        "color":     { "type": "array", "items": {"type":"number"}, "description": "[r,g,b]" },
                        "ambient":   { "type": "array", "items": {"type":"number"}, "description": "[r,g,b]" }
                    },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(v) => v,
                        None => return McpToolOutput::error("missing numeric 'entity_id' field"),
                    };
                    let direction = parse_vec3_input(&input["direction"]);
                    let color = parse_vec3_input(&input["color"]);
                    let ambient = parse_vec3_input(&input["ambient"]);
                    queue11.lock().unwrap().push(EditorCommand::UpdateDirectionalLight {
                        entity_id,
                        direction,
                        color,
                        ambient,
                    });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // rename_entity
            let queue12 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "rename_entity".to_string(),
                description: "Rename an entity by ID (applied next frame)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "number", "description": "Entity ID" },
                        "name":      { "type": "string", "description": "New name" }
                    },
                    "required": ["entity_id", "name"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(v) => v,
                        None => return McpToolOutput::error("missing numeric 'entity_id' field"),
                    };
                    let name = match input["name"].as_str() {
                        Some(n) => n.to_string(),
                        None => return McpToolOutput::error("missing string 'name' field"),
                    };
                    queue12
                        .lock()
                        .unwrap()
                        .push(EditorCommand::RenameEntity { entity_id, name });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // get_scene_stats
            let snap_stats = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_scene_stats".to_string(),
                description: "Return aggregate counts for the current scene".to_string(),
                input_schema: Some(json!({ "type": "object" })),
                handler: Box::new(move |_input| {
                    let s = snap_stats.lock().unwrap();
                    let total = s.entities.len();
                    let light_count = s.entities.iter().filter(|e| e.light_type.is_some()).count();
                    let mesh_count = s.entities.iter().filter(|e| e.mesh_id.is_some()).count();
                    let named_count = s.entities.iter().filter(|e| e.name.is_some()).count();
                    McpToolOutput::success(json!({
                        "total_entities": total,
                        "light_count": light_count,
                        "mesh_count": mesh_count,
                        "named_count": named_count,
                    }))
                }),
            });

            // query_entities -- see `entity_query` for why this is one tool.
            let snap_query = snapshot.clone();
            let sel_query = selection.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "query_entities".to_string(),
                description: "Find, count, select or summarise entities by any combination of \
                    conditions: `where` (all must hold) and `any` (one must hold) take \
                    {field, op, value}; `sort` {by, desc}, `limit`, and `from`/`from_entity` \
                    for the `distance` field. `action`: get (default), count, select, \
                    deselect, select_only, tags (histogram), bounds (min/max/center). \
                    Example: lights above y=3 named *Lamp*: {\"where\": [{\"field\": \
                    \"light.type\", \"op\": \"exists\"}, {\"field\": \"position.y\", \"op\": \
                    \"gt\", \"value\": 3}, {\"field\": \"name\", \"op\": \"contains\", \
                    \"value\": \"Lamp\"}]}. An unknown field, op or key is an error."
                    .to_string(),
                input_schema: Some(crate::entity_query::input_schema()),
                handler: Box::new(move |input| {
                    // Snapshot first, selection second -- the order every
                    // other handler that holds both takes them in.
                    let s = snap_query.lock().unwrap();
                    let mut sel = sel_query.lock().unwrap();
                    match crate::entity_query::execute(&input, &s.entities, &mut sel) {
                        Ok(v) => McpToolOutput::success(v),
                        Err(e) => McpToolOutput::error(&format!("query_entities: {e}")),
                    }
                }),
            });

            // spawn_spot_light
            let queue13 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "spawn_spot_light".to_string(),
                description: "Spawn a spot light entity at a position (applied next frame)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "color":       { "type": "array", "items": {"type":"number"}, "description": "[r,g,b]" },
                        "intensity":   { "type": "number" },
                        "range":       { "type": "number" },
                        "inner_angle": { "type": "number", "description": "Inner cone half-angle in radians" },
                        "outer_angle": { "type": "number", "description": "Outer cone half-angle in radians" },
                        "position":    { "type": "array", "items": {"type":"number"}, "description": "[x,y,z]" }
                    },
                    "required": ["color", "intensity", "range", "inner_angle", "outer_angle", "position"]
                })),
                handler: Box::new(move |input| {
                    let color = parse_vec3_input(&input["color"]).unwrap_or([1.0, 1.0, 1.0]);
                    let intensity = input["intensity"].as_f64().unwrap_or(1.0) as f32;
                    let range = input["range"].as_f64().unwrap_or(10.0) as f32;
                    let inner_angle = input["inner_angle"].as_f64().unwrap_or(std::f64::consts::PI / 8.0) as f32;
                    let outer_angle = input["outer_angle"].as_f64().unwrap_or(std::f64::consts::PI / 6.0) as f32;
                    let position = parse_vec3_input(&input["position"]).unwrap_or([0.0, 0.0, 0.0]);
                    queue13.lock().unwrap().push(EditorCommand::SpawnSpotLight {
                        color,
                        intensity,
                        range,
                        inner_angle,
                        outer_angle,
                        position,
                    });
                    McpToolOutput::success(json!({"status": "queued"}))
                }),
            });

            // update_spot_light
            let queue14 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "update_spot_light".to_string(),
                description: "Update SpotLight properties on an entity (all fields optional, applied next frame)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id":   { "type": "number" },
                        "color":       { "type": "array", "items": {"type":"number"} },
                        "intensity":   { "type": "number" },
                        "range":       { "type": "number" },
                        "inner_angle": { "type": "number" },
                        "outer_angle": { "type": "number" }
                    },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(v) => v,
                        None => return McpToolOutput::error("missing numeric 'entity_id' field"),
                    };
                    let color = parse_vec3_input(&input["color"]);
                    let intensity = input["intensity"].as_f64().map(|v| v as f32);
                    let range = input["range"].as_f64().map(|v| v as f32);
                    let inner_angle = input["inner_angle"].as_f64().map(|v| v as f32);
                    let outer_angle = input["outer_angle"].as_f64().map(|v| v as f32);
                    queue14.lock().unwrap().push(EditorCommand::UpdateSpotLight {
                        entity_id,
                        color,
                        intensity,
                        range,
                        inner_angle,
                        outer_angle,
                    });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // clear_scene
            let queue15 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "clear_scene".to_string(),
                description: "Despawn all entities in the scene (applied next frame)".to_string(),
                input_schema: Some(json!({ "type": "object" })),
                handler: Box::new(move |_input| {
                    queue15.lock().unwrap().push(EditorCommand::ClearScene);
                    McpToolOutput::success(json!({"status": "queued"}))
                }),
            });

            // batch_spawn
            let queue16 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "batch_spawn".to_string(),
                description: "Spawn multiple named entities at once (applied next frame)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entities": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "name":     { "type": "string" },
                                    "position": { "type": "array", "items": {"type":"number"}, "description": "[x,y,z]" }
                                },
                                "required": ["name"]
                            }
                        }
                    },
                    "required": ["entities"]
                })),
                handler: Box::new(move |input| {
                    let items = match input["entities"].as_array() {
                        Some(a) => a,
                        None => return McpToolOutput::error("missing array 'entities' field"),
                    };
                    let entries: Vec<(String, Option<[f32; 3]>)> = items
                        .iter()
                        .filter_map(|item| {
                            let name = item["name"].as_str()?.to_string();
                            let pos = parse_vec3_input(&item["position"]);
                            Some((name, pos))
                        })
                        .collect();
                    let count = entries.len();
                    queue16
                        .lock()
                        .unwrap()
                        .push(EditorCommand::BatchSpawn { entries });
                    McpToolOutput::success(json!({"status": "queued", "count": count}))
                }),
            });

            // spawn_camera
            let queue17 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "spawn_camera".to_string(),
                description: "Spawn a perspective camera entity (applied next frame)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "fov_y_degrees": { "type": "number", "description": "Vertical field of view in degrees (default 60)" },
                        "position": { "type": "array", "items": {"type":"number"}, "description": "[x,y,z]" }
                    }
                })),
                handler: Box::new(move |input| {
                    let fov_y_degrees = input["fov_y_degrees"].as_f64().unwrap_or(60.0) as f32;
                    let position = parse_vec3_input(&input["position"]).unwrap_or([0.0, 0.0, 0.0]);
                    queue17
                        .lock()
                        .unwrap()
                        .push(EditorCommand::SpawnCamera { fov_y_degrees, position });
                    McpToolOutput::success(json!({"status": "queued"}))
                }),
            });

            // update_camera
            let queue18 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "update_camera".to_string(),
                description: "Update a camera entity's field of view (applied next frame)"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer" },
                        "fov_y_degrees": { "type": "number" }
                    },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let fov_y_degrees = input["fov_y_degrees"].as_f64().map(|f| f as f32);
                    queue18.lock().unwrap().push(EditorCommand::UpdateCamera {
                        entity_id,
                        fov_y_degrees,
                    });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // duplicate_entity
            let queue19 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "duplicate_entity".to_string(),
                description: "Duplicate an entity, copying its name, transform, mesh, and camera components (applied next frame)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer" }
                    },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    queue19
                        .lock()
                        .unwrap()
                        .push(EditorCommand::DuplicateEntity { entity_id });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // move_entity
            let queue20 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "move_entity".to_string(),
                description: "Move an entity by a delta offset [dx, dy, dz] relative to its current position (applied next frame)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer" },
                        "dx": { "type": "number", "default": 0 },
                        "dy": { "type": "number", "default": 0 },
                        "dz": { "type": "number", "default": 0 }
                    },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let dx = input["dx"].as_f64().unwrap_or(0.0) as f32;
                    let dy = input["dy"].as_f64().unwrap_or(0.0) as f32;
                    let dz = input["dz"].as_f64().unwrap_or(0.0) as f32;
                    queue20
                        .lock()
                        .unwrap()
                        .push(EditorCommand::MoveEntity { entity_id, dx, dy, dz });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // set_rotation
            let queue21 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "set_rotation".to_string(),
                description:
                    "Set an entity's rotation from Euler angles in degrees XYZ (applied next frame)"
                        .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer" },
                        "rx": { "type": "number", "default": 0 },
                        "ry": { "type": "number", "default": 0 },
                        "rz": { "type": "number", "default": 0 }
                    },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let rx = input["rx"].as_f64().unwrap_or(0.0) as f32;
                    let ry = input["ry"].as_f64().unwrap_or(0.0) as f32;
                    let rz = input["rz"].as_f64().unwrap_or(0.0) as f32;
                    queue21.lock().unwrap().push(EditorCommand::SetRotation {
                        entity_id,
                        rx,
                        ry,
                        rz,
                    });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // set_scale
            let queue22 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "set_scale".to_string(),
                description: "Set an entity's uniform or non-uniform scale (applied next frame)"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer" },
                        "sx": { "type": "number", "default": 1 },
                        "sy": { "type": "number", "default": 1 },
                        "sz": { "type": "number", "default": 1 }
                    },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let sx = input["sx"].as_f64().unwrap_or(1.0) as f32;
                    let sy = input["sy"].as_f64().unwrap_or(1.0) as f32;
                    let sz = input["sz"].as_f64().unwrap_or(1.0) as f32;
                    queue22.lock().unwrap().push(EditorCommand::SetScale {
                        entity_id,
                        sx,
                        sy,
                        sz,
                    });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // get_components
            let snap_comp = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_components".to_string(),
                description: "List which components are attached to a specific entity".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer" }
                    },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let snapshot = snap_comp.lock().unwrap();
                    let info = snapshot.entities.iter().find(|e| e.id == entity_id);
                    match info {
                        None => McpToolOutput::error("entity not found"),
                        Some(e) => {
                            let mut components: Vec<&str> = Vec::new();
                            if e.name.is_some() {
                                components.push("Name");
                            }
                            if e.position.is_some() {
                                components.push("Transform");
                            }
                            if e.mesh_id.is_some() {
                                components.push("MeshRenderer");
                            }
                            if e.light_type.as_deref() == Some("point") {
                                components.push("PointLight");
                            } else if e.light_type.as_deref() == Some("directional") {
                                components.push("DirectionalLight");
                            } else if e.light_type.as_deref() == Some("spot") {
                                components.push("SpotLight");
                            }
                            if e.camera_fov.is_some() {
                                components.push("Camera");
                            }
                            McpToolOutput::success(
                                json!({"entity_id": entity_id, "components": components}),
                            )
                        }
                    }
                }),
            });

            // tag_entity
            let queue25 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "tag_entity".to_string(),
                description: "Add a string tag to an entity (applied next frame)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer" },
                        "tag": { "type": "string" }
                    },
                    "required": ["entity_id", "tag"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let tag = match input["tag"].as_str() {
                        Some(t) => t.to_string(),
                        None => return McpToolOutput::error("missing tag"),
                    };
                    queue25
                        .lock()
                        .unwrap()
                        .push(EditorCommand::TagEntity { entity_id, tag });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // untag_entity
            let queue26 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "untag_entity".to_string(),
                description: "Remove a string tag from an entity (applied next frame)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer" },
                        "tag": { "type": "string" }
                    },
                    "required": ["entity_id", "tag"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let tag = match input["tag"].as_str() {
                        Some(t) => t.to_string(),
                        None => return McpToolOutput::error("missing tag"),
                    };
                    queue26
                        .lock()
                        .unwrap()
                        .push(EditorCommand::UntagEntity { entity_id, tag });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // select_entity
            let sel1 = selection.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "select_entity".to_string(),
                description: "Add an entity to the editor selection set (immediate)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "entity_id": { "type": "integer" } },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    sel1.lock().unwrap().insert(entity_id);
                    McpToolOutput::success(json!({"status": "selected", "entity_id": entity_id}))
                }),
            });

            // deselect_entity
            let sel2 = selection.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "deselect_entity".to_string(),
                description: "Remove an entity from the editor selection set (immediate)"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "entity_id": { "type": "integer" } },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    sel2.lock().unwrap().remove(&entity_id);
                    McpToolOutput::success(json!({"status": "deselected", "entity_id": entity_id}))
                }),
            });

            // get_selection
            let sel3 = selection.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_selection".to_string(),
                description: "Return the list of currently selected entity IDs".to_string(),
                input_schema: Some(json!({ "type": "object" })),
                handler: Box::new(move |_input| {
                    let ids: Vec<u64> = sel3.lock().unwrap().iter().copied().collect();
                    McpToolOutput::success(json!({"selected_ids": ids}))
                }),
            });

            // clear_selection
            let sel4 = selection.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "clear_selection".to_string(),
                description: "Clear all selected entities (immediate)".to_string(),
                input_schema: Some(json!({ "type": "object" })),
                handler: Box::new(move |_input| {
                    sel4.lock().unwrap().clear();
                    McpToolOutput::success(json!({"status": "cleared"}))
                }),
            });

            // set_entity_transform
            let queue29 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "set_entity_transform".to_string(),
                description: "Set position, rotation (degrees), and/or scale for an entity in one call (applied next frame)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer" },
                        "position": {
                            "type": "array",
                            "items": { "type": "number" },
                            "minItems": 3, "maxItems": 3
                        },
                        "rotation": {
                            "type": "array",
                            "items": { "type": "number" },
                            "minItems": 3, "maxItems": 3
                        },
                        "scale": {
                            "type": "array",
                            "items": { "type": "number" },
                            "minItems": 3, "maxItems": 3
                        }
                    },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let parse = |v: &serde_json::Value| -> Option<[f32; 3]> {
                        let a = v.as_array()?;
                        Some([
                            a.first()?.as_f64()? as f32,
                            a.get(1)?.as_f64()? as f32,
                            a.get(2)?.as_f64()? as f32,
                        ])
                    };
                    let position = parse(&input["position"]);
                    let rotation = parse(&input["rotation"]);
                    let scale = parse(&input["scale"]);
                    queue29.lock().unwrap().push(EditorCommand::SetEntityTransform {
                        entity_id,
                        position,
                        rotation,
                        scale,
                    });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // get_selected_entities
            let snap_gse = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_selected_entities".to_string(),
                description: "Return all currently selected entities; returns {entities}"
                    .to_string(),
                input_schema: Some(json!({"type": "object"})),
                handler: Box::new(move |_input| {
                    let s = snap_gse.lock().unwrap();
                    let entities: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| e.selected)
                        .map(|e| json!({"id": e.id, "name": e.name, "selected": e.selected}))
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
                }),
            });

            // select/deselect for no_name

            // toggle_entity_selection
            let sel_tes = selection.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "toggle_entity_selection".to_string(),
                description: "Toggle the selection state of a single entity (select if unselected, deselect if selected); returns {selected}".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "entity_id": { "type": "integer" } },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let target = match input["entity_id"].as_u64() {
                        Some(id) => id, None => return McpToolOutput::error("missing entity_id"),
                    };
                    let mut sel = sel_tes.lock().unwrap();
                    let selected = if sel.contains(&target) {
                        sel.remove(&target); false
                    } else {
                        sel.insert(target); true
                    };
                    McpToolOutput::success(json!({"selected": selected}))
                }),
            });

            // shrink_selection_to_roots
            let snap_sstr = snapshot.clone();
            let sel_sstr = selection.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "shrink_selection_to_roots".to_string(),
                description: "Keep only selected entities that have no selected ancestor (topmost selections); removes descendants from selection; returns {removed_count}".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_sstr.lock().unwrap();
                    let sel_ids: Vec<u64> = sel_sstr.lock().unwrap().iter().copied().collect();
                    let sel_set: std::collections::HashSet<u64> = sel_ids.iter().copied().collect();
                    let mut to_remove: Vec<u64> = Vec::new();
                    for &id in &sel_ids {
                        // walk ancestors; if any ancestor is also selected, remove this entity
                        let mut cur = id;
                        let mut has_selected_ancestor = false;
                        loop {
                            let p = s.entities.iter().find(|e| e.id == cur).and_then(|e| e.parent_id);
                            match p {
                                Some(pid) => {
                                    if sel_set.contains(&pid) { has_selected_ancestor = true; break; }
                                    cur = pid;
                                }
                                None => break,
                            }
                        }
                        if has_selected_ancestor { to_remove.push(id); }
                    }
                    let count = to_remove.len() as u64;
                    let mut sel = sel_sstr.lock().unwrap();
                    for id in to_remove { sel.remove(&id); }
                    McpToolOutput::success(json!({"removed_count": count}))
                }),
            });

            // expand_selection_to_subtrees
            let snap_ests = snapshot.clone();
            let sel_ests = selection.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "expand_selection_to_subtrees".to_string(),
                description: "For each selected entity, add its entire subtree (all descendants) to the selection; returns {added_count}".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_ests.lock().unwrap();
                    let roots: Vec<u64> = sel_ests.lock().unwrap().iter().copied().collect();
                    let mut to_add: Vec<u64> = Vec::new();
                    for root in &roots {
                        let mut queue = vec![*root];
                        while let Some(cur) = queue.pop() {
                            let children: Vec<u64> = s.entities.iter()
                                .filter(|e| e.parent_id == Some(cur))
                                .map(|e| e.id).collect();
                            for child in children {
                                to_add.push(child);
                                queue.push(child);
                            }
                        }
                    }
                    let count = to_add.len() as u64;
                    let mut sel = sel_ests.lock().unwrap();
                    for id in to_add { sel.insert(id); }
                    McpToolOutput::success(json!({"added_count": count}))
                }),
            });

            // select/deselect/count for positive_x, negative_x, positive_y, negative_y, positive_z, negative_z






            // select/deselect/count for same_parent

            // untag_all_selected
            let sel_uas = selection.clone();
            let queue89 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "untag_all_selected".to_string(),
                description:
                    "Remove a tag from every currently selected entity; returns {untagged_count}"
                        .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "tag": { "type": "string" } },
                    "required": ["tag"]
                })),
                handler: Box::new(move |input| {
                    let tag = match input["tag"].as_str() {
                        Some(t) => t.to_string(),
                        None => return McpToolOutput::error("missing tag"),
                    };
                    let selected: Vec<u64> = sel_uas.lock().unwrap().iter().copied().collect();
                    let count = selected.len() as u64;
                    let mut q = queue89.lock().unwrap();
                    for entity_id in selected {
                        q.push(EditorCommand::UntagEntity {
                            entity_id,
                            tag: tag.clone(),
                        });
                    }
                    McpToolOutput::success(json!({"untagged_count": count}))
                }),
            });

            // set_visibility_by_tag
            let snap_svbt = snapshot.clone();
            let queue90 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "set_visibility_by_tag".to_string(),
                description:
                    "Show or hide all entities that have a given tag; returns {affected_count}"
                        .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "tag": { "type": "string" },
                        "visible": { "type": "boolean" }
                    },
                    "required": ["tag", "visible"]
                })),
                handler: Box::new(move |input| {
                    let tag = match input["tag"].as_str() {
                        Some(t) => t.to_string(),
                        None => return McpToolOutput::error("missing tag"),
                    };
                    let visible = match input["visible"].as_bool() {
                        Some(v) => v,
                        None => return McpToolOutput::error("missing visible"),
                    };
                    let s = snap_svbt.lock().unwrap();
                    let ids: Vec<u64> = s
                        .entities
                        .iter()
                        .filter(|e| e.tags.contains(&tag))
                        .map(|e| e.id)
                        .collect();
                    drop(s);
                    let count = ids.len() as u64;
                    let mut q = queue90.lock().unwrap();
                    for entity_id in ids {
                        q.push(EditorCommand::SetVisible { entity_id, visible });
                    }
                    McpToolOutput::success(json!({"affected_count": count}))
                }),
            });

            // tag_all_selected
            let sel_tas = selection.clone();
            let queue88 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "tag_all_selected".to_string(),
                description:
                    "Apply a tag to every currently selected entity; returns {tagged_count}"
                        .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "tag": { "type": "string" } },
                    "required": ["tag"]
                })),
                handler: Box::new(move |input| {
                    let tag = match input["tag"].as_str() {
                        Some(t) => t.to_string(),
                        None => return McpToolOutput::error("missing tag"),
                    };
                    let selected: Vec<u64> = sel_tas.lock().unwrap().iter().copied().collect();
                    let count = selected.len() as u64;
                    let mut q = queue88.lock().unwrap();
                    for entity_id in selected {
                        q.push(EditorCommand::TagEntity {
                            entity_id,
                            tag: tag.clone(),
                        });
                    }
                    McpToolOutput::success(json!({"tagged_count": count}))
                }),
            });

            // offset_selection_rotation
            let snap_osr = snapshot.clone();
            let sel_osr = selection.clone();
            let queue86 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "offset_selection_rotation".to_string(),
                description: "Add rotation offsets (degrees) to all selected entities; returns {rotated_count}".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "rx": { "type": "number" },
                        "ry": { "type": "number" },
                        "rz": { "type": "number" }
                    },
                    "required": ["rx", "ry", "rz"]
                })),
                handler: Box::new(move |input| {
                    let drx = match input["rx"].as_f64() { Some(v) => v as f32, None => return McpToolOutput::error("missing rx") };
                    let dry = match input["ry"].as_f64() { Some(v) => v as f32, None => return McpToolOutput::error("missing ry") };
                    let drz = match input["rz"].as_f64() { Some(v) => v as f32, None => return McpToolOutput::error("missing rz") };
                    let selected = sel_osr.lock().unwrap().clone();
                    let s = snap_osr.lock().unwrap();
                    let updates: Vec<(u64, [f32; 3])> = s.entities.iter()
                        .filter(|e| selected.contains(&e.id))
                        .map(|e| {
                            let cur = e.rotation.unwrap_or([0.0, 0.0, 0.0]);
                            (e.id, [cur[0] + drx, cur[1] + dry, cur[2] + drz])
                        })
                        .collect();
                    drop(s);
                    let count = updates.len() as u64;
                    let mut q = queue86.lock().unwrap();
                    for (entity_id, [rx, ry, rz]) in updates {
                        q.push(EditorCommand::SetRotation { entity_id, rx, ry, rz });
                    }
                    McpToolOutput::success(json!({"rotated_count": count}))
                }),
            });

            // offset_selection_scale
            let snap_oss = snapshot.clone();
            let sel_oss = selection.clone();
            let queue87 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "offset_selection_scale".to_string(),
                description: "Add scale offsets to all selected entities; returns {scaled_count}"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "sx": { "type": "number" },
                        "sy": { "type": "number" },
                        "sz": { "type": "number" }
                    },
                    "required": ["sx", "sy", "sz"]
                })),
                handler: Box::new(move |input| {
                    let dsx = match input["sx"].as_f64() {
                        Some(v) => v as f32,
                        None => return McpToolOutput::error("missing sx"),
                    };
                    let dsy = match input["sy"].as_f64() {
                        Some(v) => v as f32,
                        None => return McpToolOutput::error("missing sy"),
                    };
                    let dsz = match input["sz"].as_f64() {
                        Some(v) => v as f32,
                        None => return McpToolOutput::error("missing sz"),
                    };
                    let selected = sel_oss.lock().unwrap().clone();
                    let s = snap_oss.lock().unwrap();
                    let updates: Vec<(u64, [f32; 3])> = s
                        .entities
                        .iter()
                        .filter(|e| selected.contains(&e.id))
                        .map(|e| {
                            let cur = e.scale.unwrap_or([1.0, 1.0, 1.0]);
                            (e.id, [cur[0] + dsx, cur[1] + dsy, cur[2] + dsz])
                        })
                        .collect();
                    drop(s);
                    let count = updates.len() as u64;
                    let mut q = queue87.lock().unwrap();
                    for (entity_id, [sx, sy, sz]) in updates {
                        q.push(EditorCommand::SetScale {
                            entity_id,
                            sx,
                            sy,
                            sz,
                        });
                    }
                    McpToolOutput::success(json!({"scaled_count": count}))
                }),
            });

            // offset_selection_position
            let snap_osp = snapshot.clone();
            let sel_osp = selection.clone();
            let queue84 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "offset_selection_position".to_string(),
                description: "Add position offsets to all selected entities; returns {moved_count}"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "dx": { "type": "number" },
                        "dy": { "type": "number" },
                        "dz": { "type": "number" }
                    },
                    "required": ["dx", "dy", "dz"]
                })),
                handler: Box::new(move |input| {
                    let dx = match input["dx"].as_f64() {
                        Some(v) => v as f32,
                        None => return McpToolOutput::error("missing dx"),
                    };
                    let dy = match input["dy"].as_f64() {
                        Some(v) => v as f32,
                        None => return McpToolOutput::error("missing dy"),
                    };
                    let dz = match input["dz"].as_f64() {
                        Some(v) => v as f32,
                        None => return McpToolOutput::error("missing dz"),
                    };
                    let selected = sel_osp.lock().unwrap().clone();
                    let s = snap_osp.lock().unwrap();
                    let moves: Vec<(u64, [f32; 3])> = s
                        .entities
                        .iter()
                        .filter(|e| selected.contains(&e.id))
                        .map(|e| {
                            let cur = e.position.unwrap_or([0.0, 0.0, 0.0]);
                            (e.id, [cur[0] + dx, cur[1] + dy, cur[2] + dz])
                        })
                        .collect();
                    drop(s);
                    let count = moves.len() as u64;
                    let mut q = queue84.lock().unwrap();
                    for (entity_id, [x, y, z]) in moves {
                        q.push(EditorCommand::SetPosition { entity_id, x, y, z });
                    }
                    McpToolOutput::success(json!({"moved_count": count}))
                }),
            });

            // scale_selection_uniformly
            let snap_ssu = snapshot.clone();
            let sel_ssu = selection.clone();
            let queue85 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "scale_selection_uniformly".to_string(),
                description:
                    "Apply a uniform scale to all selected entities; returns {scaled_count}"
                        .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "scale": { "type": "number" } },
                    "required": ["scale"]
                })),
                handler: Box::new(move |input| {
                    let s_val = match input["scale"].as_f64() {
                        Some(v) => v as f32,
                        None => return McpToolOutput::error("missing scale"),
                    };
                    let selected = sel_ssu.lock().unwrap().clone();
                    let s = snap_ssu.lock().unwrap();
                    let ids: Vec<u64> = s
                        .entities
                        .iter()
                        .filter(|e| selected.contains(&e.id))
                        .map(|e| e.id)
                        .collect();
                    drop(s);
                    let count = ids.len() as u64;
                    let mut q = queue85.lock().unwrap();
                    for entity_id in ids {
                        q.push(EditorCommand::SetScale {
                            entity_id,
                            sx: s_val,
                            sy: s_val,
                            sz: s_val,
                        });
                    }
                    McpToolOutput::success(json!({"scaled_count": count}))
                }),
            });

            // offset_entity_rotation
            let snap_oer = snapshot.clone();
            let queue82 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "offset_entity_rotation".to_string(),
                description: "Add rotation offsets (degrees) to an entity's current rotation; returns {status}".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer" },
                        "rx": { "type": "number" },
                        "ry": { "type": "number" },
                        "rz": { "type": "number" }
                    },
                    "required": ["entity_id", "rx", "ry", "rz"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let drx = match input["rx"].as_f64() { Some(v) => v as f32, None => return McpToolOutput::error("missing rx") };
                    let dry = match input["ry"].as_f64() { Some(v) => v as f32, None => return McpToolOutput::error("missing ry") };
                    let drz = match input["rz"].as_f64() { Some(v) => v as f32, None => return McpToolOutput::error("missing rz") };
                    let s = snap_oer.lock().unwrap();
                    let cur = s.entities.iter().find(|e| e.id == entity_id)
                        .and_then(|e| e.rotation)
                        .unwrap_or([0.0, 0.0, 0.0]);
                    drop(s);
                    queue82.lock().unwrap().push(EditorCommand::SetRotation {
                        entity_id,
                        rx: cur[0] + drx,
                        ry: cur[1] + dry,
                        rz: cur[2] + drz,
                    });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // offset_entity_scale
            let snap_oes = snapshot.clone();
            let queue83 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "offset_entity_scale".to_string(),
                description: "Add scale offsets to an entity's current scale; returns {status}"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer" },
                        "sx": { "type": "number" },
                        "sy": { "type": "number" },
                        "sz": { "type": "number" }
                    },
                    "required": ["entity_id", "sx", "sy", "sz"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let dsx = match input["sx"].as_f64() {
                        Some(v) => v as f32,
                        None => return McpToolOutput::error("missing sx"),
                    };
                    let dsy = match input["sy"].as_f64() {
                        Some(v) => v as f32,
                        None => return McpToolOutput::error("missing sy"),
                    };
                    let dsz = match input["sz"].as_f64() {
                        Some(v) => v as f32,
                        None => return McpToolOutput::error("missing sz"),
                    };
                    let s = snap_oes.lock().unwrap();
                    let cur = s
                        .entities
                        .iter()
                        .find(|e| e.id == entity_id)
                        .and_then(|e| e.scale)
                        .unwrap_or([1.0, 1.0, 1.0]);
                    drop(s);
                    queue83.lock().unwrap().push(EditorCommand::SetScale {
                        entity_id,
                        sx: cur[0] + dsx,
                        sy: cur[1] + dsy,
                        sz: cur[2] + dsz,
                    });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // scale_entity_uniformly
            let queue81 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "scale_entity_uniformly".to_string(),
                description:
                    "Apply the same scale value on all three axes of an entity; returns {status}"
                        .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer" },
                        "scale": { "type": "number" }
                    },
                    "required": ["entity_id", "scale"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let s = match input["scale"].as_f64() {
                        Some(v) => v as f32,
                        None => return McpToolOutput::error("missing scale"),
                    };
                    queue81.lock().unwrap().push(EditorCommand::SetScale {
                        entity_id,
                        sx: s,
                        sy: s,
                        sz: s,
                    });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // reset_entity_scale
            let queue80 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "reset_entity_scale".to_string(),
                description: "Reset a single entity's scale to [1,1,1]; returns {status}"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "entity_id": { "type": "integer" } },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    queue80.lock().unwrap().push(EditorCommand::SetScale {
                        entity_id,
                        sx: 1.0,
                        sy: 1.0,
                        sz: 1.0,
                    });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // reset_entity_position
            let queue78 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "reset_entity_position".to_string(),
                description: "Reset a single entity's position to [0,0,0]; returns {status}"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "entity_id": { "type": "integer" } },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    queue78.lock().unwrap().push(EditorCommand::SetPosition {
                        entity_id,
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // reset_entity_rotation
            let queue79 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "reset_entity_rotation".to_string(),
                description:
                    "Reset a single entity's rotation to [0,0,0] degrees; returns {status}"
                        .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "entity_id": { "type": "integer" } },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    queue79.lock().unwrap().push(EditorCommand::SetRotation {
                        entity_id,
                        rx: 0.0,
                        ry: 0.0,
                        rz: 0.0,
                    });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // detach_selection_from_parent
            let sel_dsfp = selection.clone();
            let queue76 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "detach_selection_from_parent".to_string(),
                description: "Remove the parent from all selected entities (move to root); returns {affected_count}".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let selected: Vec<u64> = sel_dsfp.lock().unwrap().iter().copied().collect();
                    let count = selected.len() as u64;
                    let mut q = queue76.lock().unwrap();
                    for id in selected {
                        q.push(EditorCommand::RemoveParent { entity_id: id });
                    }
                    McpToolOutput::success(json!({"affected_count": count}))
                }),
            });

            // reparent_selection
            let sel_rs = selection.clone();
            let queue77 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "reparent_selection".to_string(),
                description: "Set a new parent for all selected entities; returns {affected_count}"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "parent_id": { "type": "integer" } },
                    "required": ["parent_id"]
                })),
                handler: Box::new(move |input| {
                    let parent_id = match input["parent_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing parent_id"),
                    };
                    let selected: Vec<u64> = sel_rs.lock().unwrap().iter().copied().collect();
                    let count = selected.len() as u64;
                    let mut q = queue77.lock().unwrap();
                    for id in selected {
                        q.push(EditorCommand::SetParent {
                            entity_id: id,
                            parent_id,
                        });
                    }
                    McpToolOutput::success(json!({"affected_count": count}))
                }),
            });

            // set_light_intensity
            let snap_sli = snapshot.clone();
            let queue74 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "set_light_intensity".to_string(),
                description: "Set the light intensity of a single light entity (point or spot); returns {status}".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer" },
                        "intensity": { "type": "number" }
                    },
                    "required": ["entity_id", "intensity"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let intensity = match input["intensity"].as_f64() {
                        Some(v) => v as f32,
                        None => return McpToolOutput::error("missing intensity"),
                    };
                    let light_type = {
                        let s = snap_sli.lock().unwrap();
                        s.entities.iter().find(|e| e.id == entity_id)
                            .and_then(|e| e.light_type.clone())
                    };
                    let cmd = match light_type.as_deref() {
                        Some("point") => EditorCommand::UpdatePointLight {
                            entity_id, color: None, intensity: Some(intensity), range: None,
                        },
                        Some("spot") => EditorCommand::UpdateSpotLight {
                            entity_id, color: None, intensity: Some(intensity), range: None,
                            inner_angle: None, outer_angle: None,
                        },
                        _ => return McpToolOutput::error("entity has no point or spot light"),
                    };
                    queue74.lock().unwrap().push(cmd);
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // set_all_lights_color
            let snap_salc = snapshot.clone();
            let queue75 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "set_all_lights_color".to_string(),
                description: "Set the color [r,g,b] on every light entity in the scene; returns {affected_count}".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "r": { "type": "number" },
                        "g": { "type": "number" },
                        "b": { "type": "number" }
                    },
                    "required": ["r", "g", "b"]
                })),
                handler: Box::new(move |input| {
                    let r = match input["r"].as_f64() { Some(v) => v as f32, None => return McpToolOutput::error("missing r") };
                    let g = match input["g"].as_f64() { Some(v) => v as f32, None => return McpToolOutput::error("missing g") };
                    let b = match input["b"].as_f64() { Some(v) => v as f32, None => return McpToolOutput::error("missing b") };
                    let color = [r, g, b];
                    let lights: Vec<(u64, String)> = {
                        let s = snap_salc.lock().unwrap();
                        s.entities.iter()
                            .filter_map(|e| e.light_type.as_ref().map(|t| (e.id, t.clone())))
                            .collect()
                    };
                    let count = lights.len() as u64;
                    let mut q = queue75.lock().unwrap();
                    for (entity_id, light_type) in lights {
                        let cmd = match light_type.as_str() {
                            "point" => EditorCommand::UpdatePointLight {
                                entity_id, color: Some(color), intensity: None, range: None,
                            },
                            "spot" => EditorCommand::UpdateSpotLight {
                                entity_id, color: Some(color), intensity: None, range: None,
                                inner_angle: None, outer_angle: None,
                            },
                            "directional" => EditorCommand::UpdateDirectionalLight {
                                entity_id, direction: None, color: Some(color), ambient: None,
                            },
                            _ => continue,
                        };
                        q.push(cmd);
                    }
                    McpToolOutput::success(json!({"affected_count": count}))
                }),
            });

            // hide_all_selected
            let sel_has = selection.clone();
            let queue72 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "hide_all_selected".to_string(),
                description: "Hide all currently selected entities (set visible=false); returns {affected_count}".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let selected: Vec<u64> = sel_has.lock().unwrap().iter().copied().collect();
                    let count = selected.len() as u64;
                    let mut q = queue72.lock().unwrap();
                    for id in selected {
                        q.push(EditorCommand::SetVisible { entity_id: id, visible: false });
                    }
                    McpToolOutput::success(json!({"affected_count": count}))
                }),
            });

            // show_all_selected
            let sel_sas = selection.clone();
            let queue73 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "show_all_selected".to_string(),
                description: "Show all currently selected entities (set visible=true); returns {affected_count}".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let selected: Vec<u64> = sel_sas.lock().unwrap().iter().copied().collect();
                    let count = selected.len() as u64;
                    let mut q = queue73.lock().unwrap();
                    for id in selected {
                        q.push(EditorCommand::SetVisible { entity_id: id, visible: true });
                    }
                    McpToolOutput::success(json!({"affected_count": count}))
                }),
            });

            // move_selection_to_origin
            let sel_msto = selection.clone();
            let queue70 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "move_selection_to_origin".to_string(),
                description:
                    "Move all selected entities to position [0,0,0]; returns {affected_count}"
                        .to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let selected: Vec<u64> = sel_msto.lock().unwrap().iter().copied().collect();
                    let count = selected.len() as u64;
                    let mut q = queue70.lock().unwrap();
                    for id in selected {
                        q.push(EditorCommand::SetPosition {
                            entity_id: id,
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                        });
                    }
                    McpToolOutput::success(json!({"affected_count": count}))
                }),
            });

            // set_selection_uniform_scale
            let sel_ssus = selection.clone();
            let queue71 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "set_selection_uniform_scale".to_string(),
                description: "Set all selected entities to a uniform scale (sx=sy=sz=scale); returns {affected_count}".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "scale": { "type": "number" } },
                    "required": ["scale"]
                })),
                handler: Box::new(move |input| {
                    let scale = match input["scale"].as_f64() {
                        Some(s) => s as f32,
                        None => return McpToolOutput::error("missing scale"),
                    };
                    let selected: Vec<u64> = sel_ssus.lock().unwrap().iter().copied().collect();
                    let count = selected.len() as u64;
                    let mut q = queue71.lock().unwrap();
                    for id in selected {
                        q.push(EditorCommand::SetScale { entity_id: id, sx: scale, sy: scale, sz: scale });
                    }
                    McpToolOutput::success(json!({"affected_count": count}))
                }),
            });

            // are_all_selected_visible
            let snap_aasv = snapshot.clone();
            let sel_aasv = selection.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "are_all_selected_visible".to_string(),
                description: "Return true if all currently selected entities are visible; returns {all_visible}".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let selected = sel_aasv.lock().unwrap().clone();
                    if selected.is_empty() {
                        return McpToolOutput::success(json!({"all_visible": true}));
                    }
                    let s = snap_aasv.lock().unwrap();
                    let all_visible = selected.iter().all(|&id| {
                        s.entities.iter().find(|e| e.id == id).map(|e| e.visible).unwrap_or(false)
                    });
                    McpToolOutput::success(json!({"all_visible": all_visible}))
                }),
            });

            // select/deselect/count for directional_light, spot_light, point_light, light_type




            // align_entities_to_selection_pivot
            let snap_aetsp = snapshot.clone();
            let sel_aetsp = selection.clone();
            let queue68 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "align_entities_to_selection_pivot".to_string(),
                description: "Align all selected entities' position along the given axis to match the pivot entity; pivot_entity_id: entity whose axis value is the reference; axis: 'x'|'y'|'z'; returns {affected_count}".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "pivot_entity_id": { "type": "integer" },
                        "axis": { "type": "string", "enum": ["x", "y", "z"] }
                    },
                    "required": ["pivot_entity_id", "axis"]
                })),
                handler: Box::new(move |input| {
                    let axis = input["axis"].as_str().unwrap_or("x");
                    let pivot_id = match input["pivot_entity_id"].as_u64() {
                        Some(v) => v,
                        None => return McpToolOutput::error("pivot_entity_id required"),
                    };
                    let selected: Vec<u64> = sel_aetsp.lock().unwrap().iter().cloned().collect();
                    if selected.is_empty() {
                        return McpToolOutput::success(json!({"affected_count": 0}));
                    }
                    let s = snap_aetsp.lock().unwrap();
                    let pivot_pos = match s.entities.iter().find(|e| e.id == pivot_id).and_then(|e| e.position) {
                        Some(p) => p,
                        None => return McpToolOutput::error("pivot entity has no position"),
                    };
                    let axis_idx = match axis { "y" => 1, "z" => 2, _ => 0 };
                    let mut affected = 0u64;
                    let mut q = queue68.lock().unwrap();
                    for &id in selected.iter().filter(|&&id| id != pivot_id) {
                        if let Some(e) = s.entities.iter().find(|e| e.id == id) {
                            if let Some(mut pos) = e.position {
                                pos[axis_idx] = pivot_pos[axis_idx];
                                q.push(crate::snapshot::EditorCommand::SetPosition {
                                    entity_id: id,
                                    x: pos[0],
                                    y: pos[1],
                                    z: pos[2],
                                });
                                affected += 1;
                            }
                        }
                    }
                    McpToolOutput::success(json!({"affected_count": affected}))
                }),
            });

            // distribute_entities_evenly
            let snap_dee = snapshot.clone();
            let sel_dee = selection.clone();
            let queue69 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "distribute_entities_evenly".to_string(),
                description: "Distribute selected entities at equal intervals along the given axis (min/max preserved); axis: 'x'|'y'|'z'; returns {affected_count}".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "axis": { "type": "string", "enum": ["x", "y", "z"] } },
                    "required": ["axis"]
                })),
                handler: Box::new(move |input| {
                    let axis = input["axis"].as_str().unwrap_or("x");
                    let axis_idx = match axis { "y" => 1, "z" => 2, _ => 0 };
                    let selected: Vec<u64> = sel_dee.lock().unwrap().iter().cloned().collect();
                    let s = snap_dee.lock().unwrap();
                    let mut items: Vec<(u64, [f32; 3])> = selected.iter()
                        .filter_map(|&id| s.entities.iter().find(|e| e.id == id).and_then(|e| e.position).map(|p| (id, p)))
                        .collect();
                    if items.len() < 2 {
                        return McpToolOutput::success(json!({"affected_count": 0}));
                    }
                    items.sort_by(|a, b| a.1[axis_idx].partial_cmp(&b.1[axis_idx]).unwrap_or(std::cmp::Ordering::Equal));
                    let min_v = items.first().unwrap().1[axis_idx];
                    let max_v = items.last().unwrap().1[axis_idx];
                    let n = (items.len() - 1) as f32;
                    let mut q = queue69.lock().unwrap();
                    for (i, (id, mut pos)) in items.into_iter().enumerate() {
                        pos[axis_idx] = min_v + (max_v - min_v) * (i as f32) / n;
                        q.push(crate::snapshot::EditorCommand::SetPosition {
                            entity_id: id,
                            x: pos[0],
                            y: pos[1],
                            z: pos[2],
                        });
                    }
                    McpToolOutput::success(json!({"affected_count": selected.len() as u64}))
                }),
            });

            // copy_transform_from_entity / paste_transform_to_selection (shared clipboard)
            let transform_clipboard: std::sync::Arc<Mutex<Option<([f32; 3], [f32; 3], [f32; 3])>>> =
                std::sync::Arc::new(Mutex::new(None));

            let snap_ctfe = snapshot.clone();
            let clipboard_copy = transform_clipboard.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "copy_transform_from_entity".to_string(),
                description: "Copy position, rotation, and scale from the given entity into the transform clipboard; returns {position, rotation, scale}".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "entity_id": { "type": "integer" } },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let id = match input["entity_id"].as_u64() {
                        Some(v) => v,
                        None => return McpToolOutput::error("entity_id required"),
                    };
                    let s = snap_ctfe.lock().unwrap();
                    let e = match s.entities.iter().find(|e| e.id == id) {
                        Some(e) => e,
                        None => return McpToolOutput::error("entity not found"),
                    };
                    let pos = e.position.unwrap_or([0.0, 0.0, 0.0]);
                    let rot = e.rotation.unwrap_or([0.0, 0.0, 0.0]);
                    let sc  = e.scale.unwrap_or([1.0, 1.0, 1.0]);
                    *clipboard_copy.lock().unwrap() = Some((pos, rot, sc));
                    McpToolOutput::success(json!({
                        "position": pos,
                        "rotation": rot,
                        "scale": sc,
                    }))
                }),
            });

            let clipboard_paste = transform_clipboard.clone();
            let sel_ptts = selection.clone();
            let queue67 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "paste_transform_to_selection".to_string(),
                description: "Apply the copied transform (position, rotation, scale) to all currently selected entities; returns {affected_count}".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let (pos, rot, sc) = match *clipboard_paste.lock().unwrap() {
                        Some(t) => t,
                        None => return McpToolOutput::error("no transform in clipboard"),
                    };
                    let selected: Vec<u64> = sel_ptts.lock().unwrap().iter().cloned().collect();
                    let count = selected.len() as u64;
                    let mut q = queue67.lock().unwrap();
                    for id in selected {
                        q.push(crate::snapshot::EditorCommand::SetEntityTransform {
                            entity_id: id,
                            position: Some(pos),
                            rotation: Some(rot),
                            scale: Some(sc),
                        });
                    }
                    McpToolOutput::success(json!({"affected_count": count}))
                }),
            });

            // rename_selection_replace
            let snap_rsr2 = snapshot.clone();
            let sel_rsr2 = selection.clone();
            let queue66 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "rename_selection_replace".to_string(),
                description: "For each selected entity, replace all occurrences of 'from' with 'to' in its name; returns renamed_count".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "from": { "type": "string" },
                        "to":   { "type": "string" }
                    },
                    "required": ["from", "to"]
                })),
                handler: Box::new(move |input| {
                    let from = input["from"].as_str().unwrap_or("").to_string();
                    let to   = input["to"].as_str().unwrap_or("").to_string();
                    let s = snap_rsr2.lock().unwrap();
                    let sel = sel_rsr2.lock().unwrap();
                    let mut q = queue66.lock().unwrap();
                    let mut count = 0u64;
                    for &id in sel.iter() {
                        if let Some(e) = s.entities.iter().find(|e| e.id == id) {
                            if let Some(name) = &e.name {
                                let new_name = name.replace(&from[..], &to[..]);
                                if new_name != *name {
                                    q.push(crate::snapshot::EditorCommand::RenameEntity { entity_id: id, name: new_name });
                                    count += 1;
                                }
                            }
                        }
                    }
                    McpToolOutput::success(json!({"renamed_count": count}))
                }),
            });

            // select/deselect/count for duplicate_names, empty_name


            // distribute_selection_along_y
            let snap_dsay = snapshot.clone();
            let sel_dsay = selection.clone();
            let queue65 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "distribute_selection_along_y".to_string(),
                description: "Distribute selected entities evenly along Y axis with given spacing, sorted by current Y; returns distributed_count".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "spacing": { "type": "number" } },
                    "required": ["spacing"]
                })),
                handler: Box::new(move |input| {
                    let spacing = input["spacing"].as_f64().unwrap_or(1.0) as f32;
                    let s = snap_dsay.lock().unwrap();
                    let sel = sel_dsay.lock().unwrap();
                    let mut q = queue65.lock().unwrap();
                    let mut entries: Vec<(u64, [f32; 3])> = sel.iter()
                        .filter_map(|&id| s.entities.iter().find(|e| e.id == id))
                        .filter_map(|e| e.position.map(|p| (e.id, p)))
                        .collect();
                    entries.sort_by(|a, b| a.1[1].partial_cmp(&b.1[1]).unwrap_or(std::cmp::Ordering::Equal));
                    let count = entries.len();
                    for (i, (id, pos)) in entries.into_iter().enumerate() {
                        let new_y = i as f32 * spacing;
                        q.push(crate::snapshot::EditorCommand::SetPosition { entity_id: id, x: pos[0], y: new_y, z: pos[2] });
                    }
                    McpToolOutput::success(json!({"distributed_count": count}))
                }),
            });

            // distribute_selection_along_x
            let snap_dsax = snapshot.clone();
            let sel_dsax = selection.clone();
            let queue63 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "distribute_selection_along_x".to_string(),
                description: "Distribute selected entities evenly along X axis with given spacing, sorted by current X; returns distributed_count".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "spacing": { "type": "number" } },
                    "required": ["spacing"]
                })),
                handler: Box::new(move |input| {
                    let spacing = input["spacing"].as_f64().unwrap_or(1.0) as f32;
                    let s = snap_dsax.lock().unwrap();
                    let sel = sel_dsax.lock().unwrap();
                    let mut q = queue63.lock().unwrap();
                    let mut entries: Vec<(u64, [f32; 3])> = sel.iter()
                        .filter_map(|&id| s.entities.iter().find(|e| e.id == id))
                        .filter_map(|e| e.position.map(|p| (e.id, p)))
                        .collect();
                    entries.sort_by(|a, b| a.1[0].partial_cmp(&b.1[0]).unwrap_or(std::cmp::Ordering::Equal));
                    let count = entries.len();
                    for (i, (id, pos)) in entries.into_iter().enumerate() {
                        let new_x = i as f32 * spacing;
                        q.push(crate::snapshot::EditorCommand::SetPosition { entity_id: id, x: new_x, y: pos[1], z: pos[2] });
                    }
                    McpToolOutput::success(json!({"distributed_count": count}))
                }),
            });

            // distribute_selection_along_z
            let snap_dsaz = snapshot.clone();
            let sel_dsaz = selection.clone();
            let queue64 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "distribute_selection_along_z".to_string(),
                description: "Distribute selected entities evenly along Z axis with given spacing, sorted by current Z; returns distributed_count".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "spacing": { "type": "number" } },
                    "required": ["spacing"]
                })),
                handler: Box::new(move |input| {
                    let spacing = input["spacing"].as_f64().unwrap_or(1.0) as f32;
                    let s = snap_dsaz.lock().unwrap();
                    let sel = sel_dsaz.lock().unwrap();
                    let mut q = queue64.lock().unwrap();
                    let mut entries: Vec<(u64, [f32; 3])> = sel.iter()
                        .filter_map(|&id| s.entities.iter().find(|e| e.id == id))
                        .filter_map(|e| e.position.map(|p| (e.id, p)))
                        .collect();
                    entries.sort_by(|a, b| a.1[2].partial_cmp(&b.1[2]).unwrap_or(std::cmp::Ordering::Equal));
                    let count = entries.len();
                    for (i, (id, pos)) in entries.into_iter().enumerate() {
                        let new_z = i as f32 * spacing;
                        q.push(crate::snapshot::EditorCommand::SetPosition { entity_id: id, x: pos[0], y: pos[1], z: new_z });
                    }
                    McpToolOutput::success(json!({"distributed_count": count}))
                }),
            });

            // align_selection_to_y
            let snap_asty = snapshot.clone();
            let sel_asty = selection.clone();
            let queue61 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "align_selection_to_y".to_string(),
                description: "Set the Y position of all selected entities to y while preserving X and Z; returns aligned_count".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "y": { "type": "number" } },
                    "required": ["y"]
                })),
                handler: Box::new(move |input| {
                    let target_y = input["y"].as_f64().unwrap_or(0.0) as f32;
                    let s = snap_asty.lock().unwrap();
                    let sel = sel_asty.lock().unwrap();
                    let mut q = queue61.lock().unwrap();
                    let mut count = 0u64;
                    for &id in sel.iter() {
                        if let Some(e) = s.entities.iter().find(|e| e.id == id) {
                            if let Some(pos) = e.position {
                                q.push(crate::snapshot::EditorCommand::SetPosition {
                                    entity_id: id, x: pos[0], y: target_y, z: pos[2],
                                });
                                count += 1;
                            }
                        }
                    }
                    McpToolOutput::success(json!({"aligned_count": count}))
                }),
            });

            // align_selection_to_z
            let snap_astz = snapshot.clone();
            let sel_astz = selection.clone();
            let queue62 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "align_selection_to_z".to_string(),
                description: "Set the Z position of all selected entities to z while preserving X and Y; returns aligned_count".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "z": { "type": "number" } },
                    "required": ["z"]
                })),
                handler: Box::new(move |input| {
                    let target_z = input["z"].as_f64().unwrap_or(0.0) as f32;
                    let s = snap_astz.lock().unwrap();
                    let sel = sel_astz.lock().unwrap();
                    let mut q = queue62.lock().unwrap();
                    let mut count = 0u64;
                    for &id in sel.iter() {
                        if let Some(e) = s.entities.iter().find(|e| e.id == id) {
                            if let Some(pos) = e.position {
                                q.push(crate::snapshot::EditorCommand::SetPosition {
                                    entity_id: id, x: pos[0], y: pos[1], z: target_z,
                                });
                                count += 1;
                            }
                        }
                    }
                    McpToolOutput::success(json!({"aligned_count": count}))
                }),
            });

            // align_selection_to_x
            let snap_astx = snapshot.clone();
            let sel_astx = selection.clone();
            let queue60 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "align_selection_to_x".to_string(),
                description: "Set the X position of all selected entities to x while preserving Y and Z; returns aligned_count".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "x": { "type": "number" } },
                    "required": ["x"]
                })),
                handler: Box::new(move |input| {
                    let target_x = input["x"].as_f64().unwrap_or(0.0) as f32;
                    let s = snap_astx.lock().unwrap();
                    let sel = sel_astx.lock().unwrap();
                    let mut q = queue60.lock().unwrap();
                    let mut count = 0u64;
                    for &id in sel.iter() {
                        if let Some(e) = s.entities.iter().find(|e| e.id == id) {
                            if let Some(pos) = e.position {
                                q.push(crate::snapshot::EditorCommand::SetPosition {
                                    entity_id: id,
                                    x: target_x,
                                    y: pos[1],
                                    z: pos[2],
                                });
                                count += 1;
                            }
                        }
                    }
                    McpToolOutput::success(json!({"aligned_count": count}))
                }),
            });

            // number_selection
            let snap_ns = snapshot.clone();
            let sel_ns = selection.clone();
            let queue59 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "number_selection".to_string(),
                description: "Rename selected entities to prefix_1, prefix_2, … in id order; returns renamed_count".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "prefix": { "type": "string" } },
                    "required": ["prefix"]
                })),
                handler: Box::new(move |input| {
                    let prefix = input["prefix"].as_str().unwrap_or("Entity").to_string();
                    let s = snap_ns.lock().unwrap();
                    let sel = sel_ns.lock().unwrap();
                    let mut ids: Vec<u64> = sel.iter().copied().filter(|&id| s.entities.iter().any(|e| e.id == id)).collect();
                    ids.sort();
                    let mut q = queue59.lock().unwrap();
                    let count = ids.len() as u64;
                    for (i, entity_id) in ids.into_iter().enumerate() {
                        q.push(crate::snapshot::EditorCommand::RenameEntity {
                            entity_id,
                            name: format!("{}_{}", prefix, i + 1),
                        });
                    }
                    McpToolOutput::success(json!({"renamed_count": count}))
                }),
            });

            // reset_selection_scale
            let snap_rss = snapshot.clone();
            let sel_rss = selection.clone();
            let queue58 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "reset_selection_scale".to_string(),
                description: "Set scale to (1,1,1) for all selected entities; returns reset_count"
                    .to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_rss.lock().unwrap();
                    let sel = sel_rss.lock().unwrap();
                    let mut q = queue58.lock().unwrap();
                    let mut count = 0u64;
                    for &id in sel.iter() {
                        if s.entities.iter().any(|e| e.id == id && e.scale.is_some()) {
                            q.push(crate::snapshot::EditorCommand::SetScale {
                                entity_id: id,
                                sx: 1.0,
                                sy: 1.0,
                                sz: 1.0,
                            });
                            count += 1;
                        }
                    }
                    McpToolOutput::success(json!({"reset_count": count}))
                }),
            });

            // reset_selection_position
            let snap_rsp = snapshot.clone();
            let sel_rsp = selection.clone();
            let queue56 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "reset_selection_position".to_string(),
                description:
                    "Set position to (0,0,0) for all selected entities; returns reset_count"
                        .to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_rsp.lock().unwrap();
                    let sel = sel_rsp.lock().unwrap();
                    let mut q = queue56.lock().unwrap();
                    let mut count = 0u64;
                    for &id in sel.iter() {
                        if s.entities
                            .iter()
                            .any(|e| e.id == id && e.position.is_some())
                        {
                            q.push(crate::snapshot::EditorCommand::SetPosition {
                                entity_id: id,
                                x: 0.0,
                                y: 0.0,
                                z: 0.0,
                            });
                            count += 1;
                        }
                    }
                    McpToolOutput::success(json!({"reset_count": count}))
                }),
            });

            // reset_selection_rotation
            let snap_rsr = snapshot.clone();
            let sel_rsr = selection.clone();
            let queue57 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "reset_selection_rotation".to_string(),
                description:
                    "Set rotation to (0,0,0) for all selected entities; returns reset_count"
                        .to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_rsr.lock().unwrap();
                    let sel = sel_rsr.lock().unwrap();
                    let mut q = queue57.lock().unwrap();
                    let mut count = 0u64;
                    for &id in sel.iter() {
                        if s.entities
                            .iter()
                            .any(|e| e.id == id && e.rotation.is_some())
                        {
                            q.push(crate::snapshot::EditorCommand::SetRotation {
                                entity_id: id,
                                rx: 0.0,
                                ry: 0.0,
                                rz: 0.0,
                            });
                            count += 1;
                        }
                    }
                    McpToolOutput::success(json!({"reset_count": count}))
                }),
            });

            // toggle_visibility_on_selection
            let snap_tvos = snapshot.clone();
            let sel_tvos = selection.clone();
            let queue55 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "toggle_visibility_on_selection".to_string(),
                description: "Toggle visibility (visible↔hidden) for all selected entities; returns toggled_count".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_tvos.lock().unwrap();
                    let sel = sel_tvos.lock().unwrap();
                    let mut q = queue55.lock().unwrap();
                    let mut count = 0u64;
                    for &id in sel.iter() {
                        if let Some(e) = s.entities.iter().find(|e| e.id == id) {
                            q.push(crate::snapshot::EditorCommand::SetVisible { entity_id: id, visible: !e.visible });
                            count += 1;
                        }
                    }
                    McpToolOutput::success(json!({"toggled_count": count}))
                }),
            });

            // select/deselect/count for child_count

            // expand_selection_to_children
            let snap_estc = snapshot.clone();
            let sel_estc = selection.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "expand_selection_to_children".to_string(),
                description: "Add all direct children of currently selected entities to the selection; returns added_count".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_estc.lock().unwrap();
                    let mut sel = sel_estc.lock().unwrap();
                    let current: Vec<u64> = sel.iter().copied().collect();
                    let current_set: std::collections::HashSet<u64> = current.iter().copied().collect();
                    let children: Vec<u64> = s.entities.iter()
                        .filter(|e| e.parent_id.is_some_and(|p| current_set.contains(&p)) && !current_set.contains(&e.id))
                        .map(|e| e.id)
                        .collect();
                    let count = children.len() as u64;
                    for id in children {
                        sel.insert(id);
                    }
                    McpToolOutput::success(json!({"added_count": count}))
                }),
            });

            // replace_tag_on_selection
            let snap_rtos = snapshot.clone();
            let sel_rtos = selection.clone();
            let queue53 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "replace_tag_on_selection".to_string(),
                description: "On selected entities that have old_tag, remove it and add new_tag; returns replaced_count".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "old_tag": { "type": "string" },
                        "new_tag": { "type": "string" }
                    },
                    "required": ["old_tag", "new_tag"]
                })),
                handler: Box::new(move |input| {
                    let old_tag = input["old_tag"].as_str().unwrap_or("").to_string();
                    let new_tag = input["new_tag"].as_str().unwrap_or("").to_string();
                    let s = snap_rtos.lock().unwrap();
                    let sel = sel_rtos.lock().unwrap();
                    let mut q = queue53.lock().unwrap();
                    let mut count = 0u64;
                    for &id in sel.iter() {
                        if let Some(e) = s.entities.iter().find(|e| e.id == id) {
                            if e.tags.contains(&old_tag) {
                                q.push(crate::snapshot::EditorCommand::UntagEntity { entity_id: id, tag: old_tag.clone() });
                                q.push(crate::snapshot::EditorCommand::TagEntity { entity_id: id, tag: new_tag.clone() });
                                count += 1;
                            }
                        }
                    }
                    McpToolOutput::success(json!({"replaced_count": count}))
                }),
            });

            // clear_tags_on_selection
            let snap_ctos = snapshot.clone();
            let sel_ctos = selection.clone();
            let queue54 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "clear_tags_on_selection".to_string(),
                description: "Remove all tags from all selected entities; returns cleared_count (number of entities whose tags were cleared)".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_ctos.lock().unwrap();
                    let sel = sel_ctos.lock().unwrap();
                    let mut q = queue54.lock().unwrap();
                    let mut count = 0u64;
                    for &id in sel.iter() {
                        if let Some(e) = s.entities.iter().find(|e| e.id == id) {
                            if !e.tags.is_empty() {
                                for tag in &e.tags {
                                    q.push(crate::snapshot::EditorCommand::UntagEntity { entity_id: id, tag: tag.clone() });
                                }
                                count += 1;
                            }
                        }
                    }
                    McpToolOutput::success(json!({"cleared_count": count}))
                }),
            });

            // add_prefix_to_selection
            let snap_apts = snapshot.clone();
            let sel_apts = selection.clone();
            let queue51 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "add_prefix_to_selection".to_string(),
                description:
                    "Prepend prefix to the name of all selected entities; returns renamed_count"
                        .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "prefix": { "type": "string" } },
                    "required": ["prefix"]
                })),
                handler: Box::new(move |input| {
                    let prefix = input["prefix"].as_str().unwrap_or("").to_string();
                    let s = snap_apts.lock().unwrap();
                    let sel = sel_apts.lock().unwrap();
                    let mut q = queue51.lock().unwrap();
                    let mut count = 0u64;
                    for &id in sel.iter() {
                        if let Some(e) = s.entities.iter().find(|e| e.id == id) {
                            let new_name = format!("{}{}", prefix, e.name.as_deref().unwrap_or(""));
                            q.push(crate::snapshot::EditorCommand::RenameEntity {
                                entity_id: id,
                                name: new_name,
                            });
                            count += 1;
                        }
                    }
                    McpToolOutput::success(json!({"renamed_count": count}))
                }),
            });

            // add_suffix_to_selection
            let snap_asts = snapshot.clone();
            let sel_asts = selection.clone();
            let queue52 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "add_suffix_to_selection".to_string(),
                description:
                    "Append suffix to the name of all selected entities; returns renamed_count"
                        .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "suffix": { "type": "string" } },
                    "required": ["suffix"]
                })),
                handler: Box::new(move |input| {
                    let suffix = input["suffix"].as_str().unwrap_or("").to_string();
                    let s = snap_asts.lock().unwrap();
                    let sel = sel_asts.lock().unwrap();
                    let mut q = queue52.lock().unwrap();
                    let mut count = 0u64;
                    for &id in sel.iter() {
                        if let Some(e) = s.entities.iter().find(|e| e.id == id) {
                            let new_name = format!("{}{}", e.name.as_deref().unwrap_or(""), suffix);
                            q.push(crate::snapshot::EditorCommand::RenameEntity {
                                entity_id: id,
                                name: new_name,
                            });
                            count += 1;
                        }
                    }
                    McpToolOutput::success(json!({"renamed_count": count}))
                }),
            });

            // clone_selection
            let sel_csel = selection.clone();
            let queue50 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "clone_selection".to_string(),
                description: "Duplicate all selected entities; returns duplicated_count"
                    .to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let sel = sel_csel.lock().unwrap();
                    let mut q = queue50.lock().unwrap();
                    let count = sel.len() as u64;
                    for &entity_id in sel.iter() {
                        q.push(crate::snapshot::EditorCommand::DuplicateEntity { entity_id });
                    }
                    McpToolOutput::success(json!({"duplicated_count": count}))
                }),
            });

            // center_selection
            let snap_cs = snapshot.clone();
            let sel_cs = selection.clone();
            let queue49 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "center_selection".to_string(),
                description: "Move all selected entities so their collective centroid is at the world origin (0,0,0)".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_cs.lock().unwrap();
                    let sel = sel_cs.lock().unwrap();
                    let selected_entities: Vec<&crate::snapshot::EntityInfo> = s.entities.iter()
                        .filter(|e| sel.contains(&e.id) && e.position.is_some())
                        .collect();
                    if selected_entities.is_empty() {
                        return McpToolOutput::success(json!({"centered_count": 0}));
                    }
                    let count = selected_entities.len() as f32;
                    let cx = selected_entities.iter().map(|e| e.position.unwrap()[0]).sum::<f32>() / count;
                    let cy = selected_entities.iter().map(|e| e.position.unwrap()[1]).sum::<f32>() / count;
                    let cz = selected_entities.iter().map(|e| e.position.unwrap()[2]).sum::<f32>() / count;
                    let mut q = queue49.lock().unwrap();
                    for e in &selected_entities {
                        let pos = e.position.unwrap();
                        q.push(crate::snapshot::EditorCommand::SetPosition {
                            entity_id: e.id,
                            x: pos[0] - cx,
                            y: pos[1] - cy,
                            z: pos[2] - cz,
                        });
                    }
                    McpToolOutput::success(json!({"centered_count": selected_entities.len()}))
                }),
            });

            // select/deselect/count for exactly_one_child

            // align_selection_to_ground
            let snap_astg = snapshot.clone();
            let sel_astg = selection.clone();
            let queue48 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "align_selection_to_ground".to_string(),
                description: "Set Y position to 0 for all selected entities; returns aligned_count"
                    .to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_astg.lock().unwrap();
                    let sel = sel_astg.lock().unwrap();
                    let mut q = queue48.lock().unwrap();
                    let mut count = 0u64;
                    for &id in sel.iter() {
                        if let Some(e) = s.entities.iter().find(|e| e.id == id) {
                            if let Some(pos) = e.position {
                                q.push(crate::snapshot::EditorCommand::SetPosition {
                                    entity_id: id,
                                    x: pos[0],
                                    y: 0.0,
                                    z: pos[2],
                                });
                                count += 1;
                            }
                        }
                    }
                    McpToolOutput::success(json!({"aligned_count": count}))
                }),
            });

            // set_selection_rotation
            let sel_ssr = selection.clone();
            let queue46 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "set_selection_rotation".to_string(),
                description:
                    "Set rotation (degrees) for all selected entities; returns rotated_count"
                        .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "rx": { "type": "number" },
                        "ry": { "type": "number" },
                        "rz": { "type": "number" }
                    },
                    "required": ["rx", "ry", "rz"]
                })),
                handler: Box::new(move |input| {
                    let rx = input["rx"].as_f64().unwrap_or(0.0) as f32;
                    let ry = input["ry"].as_f64().unwrap_or(0.0) as f32;
                    let rz = input["rz"].as_f64().unwrap_or(0.0) as f32;
                    let sel = sel_ssr.lock().unwrap();
                    let mut q = queue46.lock().unwrap();
                    let count = sel.len() as u64;
                    for &entity_id in sel.iter() {
                        q.push(crate::snapshot::EditorCommand::SetRotation {
                            entity_id,
                            rx,
                            ry,
                            rz,
                        });
                    }
                    McpToolOutput::success(json!({"rotated_count": count}))
                }),
            });

            // set_selection_scale
            let sel_sss = selection.clone();
            let queue47 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "set_selection_scale".to_string(),
                description: "Set scale for all selected entities; returns scaled_count"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "sx": { "type": "number" },
                        "sy": { "type": "number" },
                        "sz": { "type": "number" }
                    },
                    "required": ["sx", "sy", "sz"]
                })),
                handler: Box::new(move |input| {
                    let sx = input["sx"].as_f64().unwrap_or(1.0) as f32;
                    let sy = input["sy"].as_f64().unwrap_or(1.0) as f32;
                    let sz = input["sz"].as_f64().unwrap_or(1.0) as f32;
                    let sel = sel_sss.lock().unwrap();
                    let mut q = queue47.lock().unwrap();
                    let count = sel.len() as u64;
                    for &entity_id in sel.iter() {
                        q.push(crate::snapshot::EditorCommand::SetScale {
                            entity_id,
                            sx,
                            sy,
                            sz,
                        });
                    }
                    McpToolOutput::success(json!({"scaled_count": count}))
                }),
            });

            // select/deselect/count for non_default_scale

            // snap_selection_to_grid
            let snap_sstg = snapshot.clone();
            let sel_sstg = selection.clone();
            let queue45 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "snap_selection_to_grid".to_string(),
                description: "Snap the position of all selected entities to the nearest grid cell"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "grid_size": { "type": "number" } },
                    "required": ["grid_size"]
                })),
                handler: Box::new(move |input| {
                    let grid = input["grid_size"].as_f64().unwrap_or(1.0) as f32;
                    if grid <= 0.0 {
                        return McpToolOutput::error("grid_size must be positive");
                    }
                    let s = snap_sstg.lock().unwrap();
                    let sel = sel_sstg.lock().unwrap();
                    let mut q = queue45.lock().unwrap();
                    let mut count = 0u64;
                    for &id in sel.iter() {
                        if let Some(e) = s.entities.iter().find(|e| e.id == id) {
                            if let Some(pos) = e.position {
                                let snap = |v: f32| (v / grid).round() * grid;
                                q.push(crate::snapshot::EditorCommand::SetPosition {
                                    entity_id: id,
                                    x: snap(pos[0]),
                                    y: snap(pos[1]),
                                    z: snap(pos[2]),
                                });
                                count += 1;
                            }
                        }
                    }
                    McpToolOutput::success(json!({"snapped_count": count}))
                }),
            });

            // expand_selection_to_siblings
            let snap_ests = snapshot.clone();
            let sel_ests = selection.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "expand_selection_to_siblings".to_string(),
                description: "Add all entities sharing the same parent as any selected entity to the selection".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_ests.lock().unwrap();
                    let mut sel = sel_ests.lock().unwrap();
                    let selected_ids: Vec<u64> = sel.iter().cloned().collect();
                    let selected_parents: std::collections::HashSet<Option<u64>> = selected_ids.iter()
                        .filter_map(|id| s.entities.iter().find(|e| e.id == *id))
                        .map(|e| e.parent_id)
                        .collect();
                    let mut added = 0u64;
                    for e in &s.entities {
                        if selected_parents.contains(&e.parent_id) && !sel.contains(&e.id) {
                            sel.insert(e.id);
                            added += 1;
                        }
                    }
                    McpToolOutput::success(json!({"added_count": added}))
                }),
            });

            // select/deselect/count for non_default_rotation

            // set_selection_parent
            let sel_ssp = selection.clone();
            let queue43 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "set_selection_parent".to_string(),
                description: "Set the parent of all selected entities to the given parent entity"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "parent_id": { "type": "integer" } },
                    "required": ["parent_id"]
                })),
                handler: Box::new(move |input| {
                    let parent_id = input["parent_id"].as_u64().unwrap_or(u64::MAX);
                    let sel = sel_ssp.lock().unwrap();
                    let mut q = queue43.lock().unwrap();
                    let count = sel.len() as u64;
                    for &entity_id in sel.iter() {
                        q.push(crate::snapshot::EditorCommand::SetParent {
                            entity_id,
                            parent_id,
                        });
                    }
                    McpToolOutput::success(json!({"parented_count": count}))
                }),
            });

            // unparent_selection
            let sel_ups = selection.clone();
            let queue44 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "unparent_selection".to_string(),
                description: "Remove the parent from all selected entities (make them root-level)"
                    .to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let sel = sel_ups.lock().unwrap();
                    let mut q = queue44.lock().unwrap();
                    let count = sel.len() as u64;
                    for &entity_id in sel.iter() {
                        q.push(crate::snapshot::EditorCommand::RemoveParent { entity_id });
                    }
                    McpToolOutput::success(json!({"unparented_count": count}))
                }),
            });

            // select/deselect/count for scale_above, scale_below


            // rotate_selection_by
            let snap_rsb = snapshot.clone();
            let sel_rsb = selection.clone();
            let queue42 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "rotate_selection_by".to_string(),
                description:
                    "Add Euler-angle delta (degrees) to the rotation of all selected entities"
                        .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "drx": { "type": "number" },
                        "dry": { "type": "number" },
                        "drz": { "type": "number" }
                    },
                    "required": ["drx", "dry", "drz"]
                })),
                handler: Box::new(move |input| {
                    let drx = input["drx"].as_f64().unwrap_or(0.0) as f32;
                    let dry = input["dry"].as_f64().unwrap_or(0.0) as f32;
                    let drz = input["drz"].as_f64().unwrap_or(0.0) as f32;
                    let sel = sel_rsb.lock().unwrap();
                    let s = snap_rsb.lock().unwrap();
                    let mut q = queue42.lock().unwrap();
                    let count = sel.len() as u64;
                    for &entity_id in sel.iter() {
                        let current = s
                            .entities
                            .iter()
                            .find(|e| e.id == entity_id)
                            .and_then(|e| e.rotation)
                            .unwrap_or([0.0, 0.0, 0.0]);
                        q.push(crate::snapshot::EditorCommand::SetRotation {
                            entity_id,
                            rx: current[0] + drx,
                            ry: current[1] + dry,
                            rz: current[2] + drz,
                        });
                    }
                    McpToolOutput::success(json!({"rotated_count": count}))
                }),
            });

            // move_selection_by
            let sel_msb = selection.clone();
            let queue40 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "move_selection_by".to_string(),
                description: "Move all selected entities by (dx, dy, dz)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "dx": { "type": "number" },
                        "dy": { "type": "number" },
                        "dz": { "type": "number" }
                    },
                    "required": ["dx", "dy", "dz"]
                })),
                handler: Box::new(move |input| {
                    let dx = input["dx"].as_f64().unwrap_or(0.0) as f32;
                    let dy = input["dy"].as_f64().unwrap_or(0.0) as f32;
                    let dz = input["dz"].as_f64().unwrap_or(0.0) as f32;
                    let sel = sel_msb.lock().unwrap();
                    let mut q = queue40.lock().unwrap();
                    let count = sel.len() as u64;
                    for &entity_id in sel.iter() {
                        q.push(crate::snapshot::EditorCommand::MoveEntity {
                            entity_id,
                            dx,
                            dy,
                            dz,
                        });
                    }
                    McpToolOutput::success(json!({"moved_count": count}))
                }),
            });

            // scale_selection_by
            let snap_ssb = snapshot.clone();
            let sel_ssb = selection.clone();
            let queue41 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "scale_selection_by".to_string(),
                description: "Multiply the scale of all selected entities by (sx, sy, sz)"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "sx": { "type": "number" },
                        "sy": { "type": "number" },
                        "sz": { "type": "number" }
                    },
                    "required": ["sx", "sy", "sz"]
                })),
                handler: Box::new(move |input| {
                    let sx = input["sx"].as_f64().unwrap_or(1.0) as f32;
                    let sy = input["sy"].as_f64().unwrap_or(1.0) as f32;
                    let sz = input["sz"].as_f64().unwrap_or(1.0) as f32;
                    let sel = sel_ssb.lock().unwrap();
                    let s = snap_ssb.lock().unwrap();
                    let mut q = queue41.lock().unwrap();
                    let count = sel.len() as u64;
                    for &entity_id in sel.iter() {
                        let current_scale = s
                            .entities
                            .iter()
                            .find(|e| e.id == entity_id)
                            .and_then(|e| e.scale)
                            .unwrap_or([1.0, 1.0, 1.0]);
                        q.push(crate::snapshot::EditorCommand::SetScale {
                            entity_id,
                            sx: current_scale[0] * sx,
                            sy: current_scale[1] * sy,
                            sz: current_scale[2] * sz,
                        });
                    }
                    McpToolOutput::success(json!({"scaled_count": count}))
                }),
            });

            // add_tag_to_selection
            let sel_atts = selection.clone();
            let queue38 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "add_tag_to_selection".to_string(),
                description: "Add a tag to all currently selected entities".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "tag": { "type": "string" } },
                    "required": ["tag"]
                })),
                handler: Box::new(move |input| {
                    let tag = input["tag"].as_str().unwrap_or("").to_string();
                    let sel = sel_atts.lock().unwrap();
                    let mut q = queue38.lock().unwrap();
                    let count = sel.len() as u64;
                    for &entity_id in sel.iter() {
                        q.push(crate::snapshot::EditorCommand::TagEntity {
                            entity_id,
                            tag: tag.clone(),
                        });
                    }
                    McpToolOutput::success(json!({"tagged_count": count}))
                }),
            });

            // remove_tag_from_selection
            let sel_rtfs = selection.clone();
            let queue39 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "remove_tag_from_selection".to_string(),
                description: "Remove a tag from all currently selected entities".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "tag": { "type": "string" } },
                    "required": ["tag"]
                })),
                handler: Box::new(move |input| {
                    let tag = input["tag"].as_str().unwrap_or("").to_string();
                    let sel = sel_rtfs.lock().unwrap();
                    let mut q = queue39.lock().unwrap();
                    let count = sel.len() as u64;
                    for &entity_id in sel.iter() {
                        q.push(crate::snapshot::EditorCommand::UntagEntity {
                            entity_id,
                            tag: tag.clone(),
                        });
                    }
                    McpToolOutput::success(json!({"untagged_count": count}))
                }),
            });

            // group_entities_by_tag
            let snap_gebt = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "group_entities_by_tag".to_string(),
                description: "Return a map of tag → list of entity IDs that have that tag"
                    .to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_gebt.lock().unwrap();
                    let mut groups: std::collections::HashMap<String, Vec<u64>> =
                        std::collections::HashMap::new();
                    for e in &s.entities {
                        for tag in &e.tags {
                            groups.entry(tag.clone()).or_default().push(e.id);
                        }
                    }
                    let groups_json: serde_json::Map<String, serde_json::Value> = groups
                        .into_iter()
                        .map(|(k, v)| {
                            (
                                k,
                                serde_json::Value::Array(
                                    v.into_iter().map(|id| json!(id)).collect(),
                                ),
                            )
                        })
                        .collect();
                    McpToolOutput::success(
                        json!({"groups": serde_json::Value::Object(groups_json)}),
                    )
                }),
            });

            // select/deselect/count for intensity_above, intensity_below, range_above



            // select/deselect for position; select/deselect/count for position_above, position_below, position_in_box




            // batch_tag_entities
            let queue36 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "batch_tag_entities".to_string(),
                description: "Add a tag to multiple entities at once".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_ids": { "type": "array", "items": { "type": "integer" } },
                        "tag": { "type": "string" }
                    },
                    "required": ["entity_ids", "tag"]
                })),
                handler: Box::new(move |input| {
                    let tag = match input["tag"].as_str() {
                        Some(t) => t.to_string(),
                        None => return McpToolOutput::error("missing tag"),
                    };
                    let ids: Vec<u64> = match input["entity_ids"].as_array() {
                        Some(arr) => arr.iter().filter_map(|v| v.as_u64()).collect(),
                        None => return McpToolOutput::error("missing entity_ids"),
                    };
                    let count = ids.len() as u64;
                    let mut q = queue36.lock().unwrap();
                    for entity_id in ids {
                        q.push(EditorCommand::TagEntity {
                            entity_id,
                            tag: tag.clone(),
                        });
                    }
                    McpToolOutput::success(json!({"tagged_count": count}))
                }),
            });

            // batch_untag_entities
            let queue37 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "batch_untag_entities".to_string(),
                description: "Remove a tag from multiple entities at once".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_ids": { "type": "array", "items": { "type": "integer" } },
                        "tag": { "type": "string" }
                    },
                    "required": ["entity_ids", "tag"]
                })),
                handler: Box::new(move |input| {
                    let tag = match input["tag"].as_str() {
                        Some(t) => t.to_string(),
                        None => return McpToolOutput::error("missing tag"),
                    };
                    let ids: Vec<u64> = match input["entity_ids"].as_array() {
                        Some(arr) => arr.iter().filter_map(|v| v.as_u64()).collect(),
                        None => return McpToolOutput::error("missing entity_ids"),
                    };
                    let count = ids.len() as u64;
                    let mut q = queue37.lock().unwrap();
                    for entity_id in ids {
                        q.push(EditorCommand::UntagEntity {
                            entity_id,
                            tag: tag.clone(),
                        });
                    }
                    McpToolOutput::success(json!({"untagged_count": count}))
                }),
            });

            // clear_all_tags_from_entity
            let snap_catfe = snapshot.clone();
            let queue35 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "clear_all_tags_from_entity".to_string(),
                description: "Remove all tags from an entity".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "entity_id": { "type": "integer" } },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let s = snap_catfe.lock().unwrap();
                    let tags = match s.entities.iter().find(|e| e.id == entity_id) {
                        Some(e) => e.tags.clone(),
                        None => return McpToolOutput::error("entity not found"),
                    };
                    drop(s);
                    let mut q = queue35.lock().unwrap();
                    for tag in tags {
                        q.push(EditorCommand::UntagEntity { entity_id, tag });
                    }
                    McpToolOutput::success(json!({"entity_id": entity_id, "status": "queued"}))
                }),
            });

            // toggle_entity_visibility
            let snap_tev = snapshot.clone();
            let queue34 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "toggle_entity_visibility".to_string(),
                description: "Toggle the visible state of an entity".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "entity_id": { "type": "integer" } },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let s = snap_tev.lock().unwrap();
                    let visible = match s.entities.iter().find(|e| e.id == entity_id) {
                        Some(e) => e.visible,
                        None => return McpToolOutput::error("entity not found"),
                    };
                    drop(s);
                    queue34.lock().unwrap().push(EditorCommand::SetVisible {
                        entity_id,
                        visible: !visible,
                    });
                    McpToolOutput::success(json!({"entity_id": entity_id, "new_visible": !visible}))
                }),
            });

            // move_entity_to_origin
            let queue33 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "move_entity_to_origin".to_string(),
                description: "Move an entity to position [0, 0, 0]".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "entity_id": { "type": "integer" } },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    queue33.lock().unwrap().push(EditorCommand::SetPosition {
                        entity_id,
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    });
                    McpToolOutput::success(json!({"entity_id": entity_id}))
                }),
            });

            // rename_entity_with_suffix
            let queue32 = cmd_queue.clone();
            let snap_rews = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "rename_entity_with_suffix".to_string(),
                description: "Append a suffix to an entity's current name".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer" },
                        "suffix": { "type": "string" }
                    },
                    "required": ["entity_id", "suffix"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let suffix = match input["suffix"].as_str() {
                        Some(s) => s.to_string(),
                        None => return McpToolOutput::error("missing suffix"),
                    };
                    let s = snap_rews.lock().unwrap();
                    let current_name = match s.entities.iter().find(|e| e.id == entity_id) {
                        Some(e) => e.name.clone().unwrap_or_default(),
                        None => return McpToolOutput::error("entity not found"),
                    };
                    drop(s);
                    let new_name = format!("{}{}", current_name, suffix);
                    queue32.lock().unwrap().push(EditorCommand::RenameEntity {
                        entity_id,
                        name: new_name.clone(),
                    });
                    McpToolOutput::success(json!({"entity_id": entity_id, "new_name": new_name}))
                }),
            });

            // set_entity_scale_uniform
            let queue31 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "set_entity_scale_uniform".to_string(),
                description: "Set uniform scale (same value for X, Y, Z) on an entity".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer" },
                        "scale": { "type": "number" }
                    },
                    "required": ["entity_id", "scale"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let s = match input["scale"].as_f64() {
                        Some(v) => v as f32,
                        None => return McpToolOutput::error("missing scale"),
                    };
                    queue31.lock().unwrap().push(EditorCommand::SetScale {
                        entity_id,
                        sx: s,
                        sy: s,
                        sz: s,
                    });
                    McpToolOutput::success(
                        json!({"status": "queued", "entity_id": entity_id, "scale": s}),
                    )
                }),
            });

            // copy_tags_from_entity
            let snap_cte = snapshot.clone();
            let queue30 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "copy_tags_from_entity".to_string(),
                description: "Copy all tags from a source entity to a target entity".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "source_id": { "type": "integer" },
                        "target_id": { "type": "integer" }
                    },
                    "required": ["source_id", "target_id"]
                })),
                handler: Box::new(move |input| {
                    let source_id = match input["source_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing source_id"),
                    };
                    let target_id = match input["target_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing target_id"),
                    };
                    let tags = {
                        let s = snap_cte.lock().unwrap();
                        match s.entities.iter().find(|e| e.id == source_id) {
                            Some(e) => e.tags.clone(),
                            None => return McpToolOutput::error("source entity not found"),
                        }
                    };
                    let count = tags.len() as u64;
                    let mut q = queue30.lock().unwrap();
                    for tag in tags {
                        q.push(EditorCommand::TagEntity {
                            entity_id: target_id,
                            tag,
                        });
                    }
                    McpToolOutput::success(json!({"status": "queued", "tags_copied": count}))
                }),
            });

            // offset_selected_positions
            let snap_osp = snapshot.clone();
            let sel_osp = selection.clone();
            let queue_osp = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "offset_selected_positions".to_string(),
                description: "Move all currently selected entities by (dx, dy, dz)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "dx": {"type": "number"},
                        "dy": {"type": "number"},
                        "dz": {"type": "number"}
                    },
                    "required": ["dx", "dy", "dz"]
                })),
                handler: Box::new(move |input| {
                    let get = |k: &str| input[k].as_f64().map(|v| v as f32);
                    let (dx, dy, dz) = match (get("dx"), get("dy"), get("dz")) {
                        (Some(a), Some(b), Some(c)) => (a, b, c),
                        _ => return McpToolOutput::error("missing dx/dy/dz"),
                    };
                    let ids: Vec<u64> = {
                        let sel = sel_osp.lock().unwrap();
                        sel.iter().copied().collect()
                    };
                    let existing: std::collections::HashSet<u64> = {
                        let s = snap_osp.lock().unwrap();
                        s.entities.iter().map(|e| e.id).collect()
                    };
                    let count = ids.len() as u64;
                    let mut q = queue_osp.lock().unwrap();
                    for id in ids {
                        if existing.contains(&id) {
                            q.push(crate::snapshot::EditorCommand::MoveEntity {
                                entity_id: id,
                                dx,
                                dy,
                                dz,
                            });
                        }
                    }
                    McpToolOutput::success(json!({"moved_count": count}))
                }),
            });

            // copy_entity_transform
            let snap_cet = snapshot.clone();
            let queue_cet = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "copy_entity_transform".to_string(),
                description: "Copy the position, rotation, and scale from source_entity_id to target_entity_id".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "source_entity_id": { "type": "integer" },
                        "target_entity_id": { "type": "integer" }
                    },
                    "required": ["source_entity_id", "target_entity_id"]
                })),
                handler: Box::new(move |input| {
                    let src_id = match input["source_entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing source_entity_id"),
                    };
                    let tgt_id = match input["target_entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing target_entity_id"),
                    };
                    let (pos, rot, scale) = {
                        let s = snap_cet.lock().unwrap();
                        match s.entities.iter().find(|e| e.id == src_id) {
                            Some(e) => (e.position, e.rotation, e.scale),
                            None => return McpToolOutput::error("source entity not found"),
                        }
                    };
                    let mut q = queue_cet.lock().unwrap();
                    q.push(crate::snapshot::EditorCommand::SetEntityTransform {
                        entity_id: tgt_id,
                        position: pos,
                        rotation: rot,
                        scale,
                    });
                    McpToolOutput::success(json!({"source_entity_id": src_id, "target_entity_id": tgt_id}))
                }),
            });

            // clear_all_tags
            let snap_cat = snapshot.clone();
            let queue_cat = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "clear_all_tags".to_string(),
                description: "Remove all tags from every entity in the scene".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let pairs: Vec<(u64, String)> = {
                        let s = snap_cat.lock().unwrap();
                        s.entities
                            .iter()
                            .flat_map(|e| e.tags.iter().map(move |t| (e.id, t.clone())))
                            .collect()
                    };
                    let count = pairs.len() as u64;
                    let mut q = queue_cat.lock().unwrap();
                    for (id, tag) in pairs {
                        q.push(crate::snapshot::EditorCommand::UntagEntity { entity_id: id, tag });
                    }
                    McpToolOutput::success(json!({"removed_tag_count": count}))
                }),
            });

            // rename_entities_with_prefix
            let snap_rewp = snapshot.clone();
            let queue_rewp = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "rename_entities_with_prefix".to_string(),
                description: "Rename all entities whose name starts with old_prefix by replacing it with new_prefix".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "old_prefix": { "type": "string" },
                        "new_prefix": { "type": "string" }
                    },
                    "required": ["old_prefix", "new_prefix"]
                })),
                handler: Box::new(move |input| {
                    let old_p = match input["old_prefix"].as_str() {
                        Some(p) => p.to_string(),
                        None => return McpToolOutput::error("missing old_prefix"),
                    };
                    let new_p = match input["new_prefix"].as_str() {
                        Some(p) => p.to_string(),
                        None => return McpToolOutput::error("missing new_prefix"),
                    };
                    let renames: Vec<(u64, String)> = {
                        let s = snap_rewp.lock().unwrap();
                        s.entities.iter()
                            .filter_map(|e| {
                                let name = e.name.as_deref()?;
                                if name.starts_with(&*old_p) {
                                    let new_name = format!("{}{}", new_p, &name[old_p.len()..]);
                                    Some((e.id, new_name))
                                } else {
                                    None
                                }
                            })
                            .collect()
                    };
                    let count = renames.len() as u64;
                    let mut q = queue_rewp.lock().unwrap();
                    for (id, name) in renames {
                        q.push(crate::snapshot::EditorCommand::RenameEntity { entity_id: id, name });
                    }
                    McpToolOutput::success(json!({"renamed_count": count}))
                }),
            });

            // set_all_visible
            let snap_sav = snapshot.clone();
            let queue_sav = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "set_all_visible".to_string(),
                description: "Set visibility of every entity in the scene".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "visible": { "type": "boolean" } },
                    "required": ["visible"]
                })),
                handler: Box::new(move |input| {
                    let visible = match input["visible"].as_bool() {
                        Some(v) => v,
                        None => return McpToolOutput::error("missing visible"),
                    };
                    let ids: Vec<u64> = {
                        let s = snap_sav.lock().unwrap();
                        s.entities.iter().map(|e| e.id).collect()
                    };
                    let count = ids.len() as u64;
                    let mut q = queue_sav.lock().unwrap();
                    for id in ids {
                        q.push(crate::snapshot::EditorCommand::SetVisible {
                            entity_id: id,
                            visible,
                        });
                    }
                    McpToolOutput::success(json!({"updated_count": count}))
                }),
            });

            // snap_entity_to_grid
            let snap_setg = snapshot.clone();
            let queue_setg = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "snap_entity_to_grid".to_string(),
                description:
                    "Round each axis of the entity's position to the nearest grid_size multiple"
                        .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer" },
                        "grid_size": { "type": "number" }
                    },
                    "required": ["entity_id", "grid_size"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let grid = match input["grid_size"].as_f64() {
                        Some(g) if g > 0.0 => g as f32,
                        _ => return McpToolOutput::error("grid_size must be a positive number"),
                    };
                    let s = snap_setg.lock().unwrap();
                    match s.entities.iter().find(|e| e.id == entity_id) {
                        Some(e) => {
                            let [x, y, z] = e.position.unwrap_or([0.0, 0.0, 0.0]);
                            let snap = |v: f32| (v / grid).round() * grid;
                            let (sx, sy, sz) = (snap(x), snap(y), snap(z));
                            queue_setg.lock().unwrap().push(
                                crate::snapshot::EditorCommand::SetPosition {
                                    entity_id,
                                    x: sx,
                                    y: sy,
                                    z: sz,
                                },
                            );
                            McpToolOutput::success(
                                json!({"entity_id": entity_id, "position": [sx, sy, sz]}),
                            )
                        }
                        None => McpToolOutput::error("entity not found"),
                    }
                }),
            });

            // delete_selected_entities
            let sel_dse = selection.clone();
            let queue_dse = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "delete_selected_entities".to_string(),
                description: "Despawn all currently selected entities and clear the selection"
                    .to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let ids: Vec<u64> = {
                        let s = sel_dse.lock().unwrap();
                        s.iter().copied().collect()
                    };
                    let count = ids.len() as u64;
                    {
                        let mut q = queue_dse.lock().unwrap();
                        for id in &ids {
                            q.push(crate::snapshot::EditorCommand::Despawn { entity_id: *id });
                        }
                    }
                    sel_dse.lock().unwrap().clear();
                    McpToolOutput::success(json!({"deleted_count": count}))
                }),
            });

            // mirror_selected_on_axis
            let snap_msoa = snapshot.clone();
            let sel_msoa = selection.clone();
            let queue_msoa = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "mirror_selected_on_axis".to_string(),
                description: "Negate the specified axis (x/y/z) of all selected entities (applied next frame)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "axis": { "type": "string", "enum": ["x", "y", "z"] } },
                    "required": ["axis"]
                })),
                handler: Box::new(move |input| {
                    let axis = match input["axis"].as_str() {
                        Some(a) => a.to_string(),
                        None => return McpToolOutput::error("missing axis"),
                    };
                    let selected: Vec<u64> = sel_msoa.lock().unwrap().iter().cloned().collect();
                    let s = snap_msoa.lock().unwrap();
                    let mut q = queue_msoa.lock().unwrap();
                    let mut count = 0u64;
                    for &entity_id in &selected {
                        if let Some(e) = s.entities.iter().find(|e| e.id == entity_id) {
                            let [mut x, mut y, mut z] = e.position.unwrap_or([0.0, 0.0, 0.0]);
                            match axis.as_str() {
                                "x" => x = -x,
                                "y" => y = -y,
                                "z" => z = -z,
                                _ => {}
                            }
                            q.push(EditorCommand::SetPosition { entity_id, x, y, z });
                            count += 1;
                        }
                    }
                    McpToolOutput::success(json!({"mirrored_count": count, "axis": axis}))
                }),
            });

            // reset_entity_transform
            let queue_ret = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "reset_entity_transform".to_string(),
                description: "Reset position to (0,0,0), rotation to (0,0,0), and scale to (1,1,1) (applied next frame)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "entity_id": { "type": "integer" } },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let mut q = queue_ret.lock().unwrap();
                    q.push(EditorCommand::SetPosition { entity_id, x: 0.0, y: 0.0, z: 0.0 });
                    q.push(EditorCommand::SetRotation { entity_id, rx: 0.0, ry: 0.0, rz: 0.0 });
                    q.push(EditorCommand::SetScale { entity_id, sx: 1.0, sy: 1.0, sz: 1.0 });
                    McpToolOutput::success(json!({"entity_id": entity_id}))
                }),
            });

            // scale_entity_uniform
            let queue_seu = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "scale_entity_uniform".to_string(),
                description: "Set sx=sy=sz=factor for the entity (applied next frame)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer" },
                        "factor": { "type": "number" }
                    },
                    "required": ["entity_id", "factor"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let factor = input["factor"].as_f64().unwrap_or(1.0) as f32;
                    queue_seu.lock().unwrap().push(EditorCommand::SetScale {
                        entity_id,
                        sx: factor,
                        sy: factor,
                        sz: factor,
                    });
                    McpToolOutput::success(json!({"entity_id": entity_id, "factor": factor}))
                }),
            });

            // detach_all_meshes
            let snap_dam = snapshot.clone();
            let queue_dam = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "detach_all_meshes".to_string(),
                description:
                    "Remove mesh renderers from all entities that have one (applied next frame)"
                        .to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_dam.lock().unwrap();
                    let ids: Vec<u64> = s
                        .entities
                        .iter()
                        .filter(|e| e.mesh_id.is_some())
                        .map(|e| e.id)
                        .collect();
                    let count = ids.len() as u64;
                    let mut q = queue_dam.lock().unwrap();
                    for entity_id in ids {
                        q.push(EditorCommand::DetachMeshRenderer { entity_id });
                    }
                    McpToolOutput::success(json!({"detached_count": count}))
                }),
            });

            // align_selected_on_axis
            let snap_asa = snapshot.clone();
            let sel_asa = selection.clone();
            let queue_asa = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "align_selected_on_axis".to_string(),
                description: "Set the specified axis (x/y/z) of all selected entities to the given value (applied next frame)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "axis":  { "type": "string", "enum": ["x", "y", "z"] },
                        "value": { "type": "number" }
                    },
                    "required": ["axis", "value"]
                })),
                handler: Box::new(move |input| {
                    let axis = match input["axis"].as_str() {
                        Some(a) => a.to_string(),
                        None => return McpToolOutput::error("missing axis"),
                    };
                    let value = input["value"].as_f64().unwrap_or(0.0) as f32;
                    let selected: Vec<u64> = sel_asa.lock().unwrap().iter().cloned().collect();
                    let s = snap_asa.lock().unwrap();
                    let mut count = 0u64;
                    let mut q = queue_asa.lock().unwrap();
                    for &entity_id in &selected {
                        if let Some(e) = s.entities.iter().find(|e| e.id == entity_id) {
                            let [mut x, mut y, mut z] = e.position.unwrap_or([0.0, 0.0, 0.0]);
                            match axis.as_str() {
                                "x" => x = value,
                                "y" => y = value,
                                "z" => z = value,
                                _ => {}
                            }
                            q.push(EditorCommand::SetPosition { entity_id, x, y, z });
                            count += 1;
                        }
                    }
                    McpToolOutput::success(json!({"aligned_count": count, "axis": axis, "value": value}))
                }),
            });

            // search_entities
            let snap_se = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "search_entities".to_string(),
                description: "Search entities by name (substring) or tag (exact match); returns union of matches".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "query": { "type": "string" } },
                    "required": ["query"]
                })),
                handler: Box::new(move |input| {
                    let query = match input["query"].as_str() {
                        Some(q) => q.to_string(),
                        None => return McpToolOutput::error("missing query"),
                    };
                    let s = snap_se.lock().unwrap();
                    let entities: Vec<serde_json::Value> = s.entities.iter()
                        .filter(|e| {
                            let name_match = e.name.as_deref().map(|n| n.contains(&*query)).unwrap_or(false);
                            let tag_match = e.tags.iter().any(|t| t == &query);
                            name_match || tag_match
                        })
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
                }),
            });

            // reset_transform
            let queue_rt = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "reset_transform".to_string(),
                description: "Reset position to [0,0,0], rotation to [0,0,0], and scale to [1,1,1] (applied next frame)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "entity_id": { "type": "integer" } },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    queue_rt.lock().unwrap().push(EditorCommand::SetEntityTransform {
                        entity_id,
                        position: Some([0.0, 0.0, 0.0]),
                        rotation: Some([0.0, 0.0, 0.0]),
                        scale: Some([1.0, 1.0, 1.0]),
                    });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // translate_selected_entities
            let sel_tse = selection.clone();
            let queue_tse = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "translate_selected_entities".to_string(),
                description: "Move all selected entities by a delta offset (applied next frame)"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "dx": { "type": "number" },
                        "dy": { "type": "number" },
                        "dz": { "type": "number" }
                    },
                    "required": ["dx", "dy", "dz"]
                })),
                handler: Box::new(move |input| {
                    let dx = input["dx"].as_f64().unwrap_or(0.0) as f32;
                    let dy = input["dy"].as_f64().unwrap_or(0.0) as f32;
                    let dz = input["dz"].as_f64().unwrap_or(0.0) as f32;
                    let ids: Vec<u64> = sel_tse.lock().unwrap().iter().copied().collect();
                    let count = ids.len();
                    let mut queue = queue_tse.lock().unwrap();
                    for entity_id in ids {
                        queue.push(EditorCommand::MoveEntity {
                            entity_id,
                            dx,
                            dy,
                            dz,
                        });
                    }
                    McpToolOutput::success(json!({"status": "queued", "count": count}))
                }),
            });

            // mirror_entity
            let snap_mir = snapshot.clone();
            let queue_mir = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "mirror_entity".to_string(),
                description: "Negate the entity's position along the given axis (x, y, or z), applied next frame".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer" },
                        "axis": { "type": "string", "enum": ["x", "y", "z"] }
                    },
                    "required": ["entity_id", "axis"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let axis = match input["axis"].as_str() {
                        Some(a) => a.to_string(),
                        None => return McpToolOutput::error("missing axis"),
                    };
                    let s = snap_mir.lock().unwrap();
                    let pos = match s.entities.iter().find(|e| e.id == entity_id) {
                        Some(e) => match e.position {
                            Some(p) => p,
                            None => return McpToolOutput::error("entity has no position"),
                        },
                        None => return McpToolOutput::error("entity not found"),
                    };
                    drop(s);
                    let mirrored = match axis.as_str() {
                        "x" => [-pos[0], pos[1], pos[2]],
                        "y" => [pos[0], -pos[1], pos[2]],
                        "z" => [pos[0], pos[1], -pos[2]],
                        _ => return McpToolOutput::error("axis must be x, y, or z"),
                    };
                    queue_mir.lock().unwrap().push(EditorCommand::SetEntityTransform {
                        entity_id,
                        position: Some(mirrored),
                        rotation: None,
                        scale: None,
                    });
                    McpToolOutput::success(json!({"status": "queued", "mirrored_position": mirrored}))
                }),
            });

            // invert_selection
            let snap_inv = snapshot.clone();
            let sel_inv = selection.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "invert_selection".to_string(),
                description: "Invert the selection: deselect selected entities, select unselected ones (immediate)".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_inv.lock().unwrap();
                    let all_ids: Vec<u64> = s.entities.iter().map(|e| e.id).collect();
                    drop(s);
                    let mut sel = sel_inv.lock().unwrap();
                    let mut new_sel = std::collections::HashSet::new();
                    for id in all_ids {
                        if !sel.contains(&id) { new_sel.insert(id); }
                    }
                    *sel = new_sel;
                    McpToolOutput::success(json!({"status": "ok", "count": sel.len()}))
                }),
            });

            // snap_to_grid
            let snap_sg = snapshot.clone();
            let queue_sg = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "snap_to_grid".to_string(),
                description: "Round entity position to the nearest grid cell (applied next frame)"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer" },
                        "grid_size": { "type": "number" }
                    },
                    "required": ["entity_id", "grid_size"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let grid = input["grid_size"].as_f64().unwrap_or(1.0) as f32;
                    if grid <= 0.0 {
                        return McpToolOutput::error("grid_size must be positive");
                    }
                    let s = snap_sg.lock().unwrap();
                    let pos = match s.entities.iter().find(|e| e.id == entity_id) {
                        Some(e) => match e.position {
                            Some(p) => p,
                            None => return McpToolOutput::error("entity has no position"),
                        },
                        None => return McpToolOutput::error("entity not found"),
                    };
                    drop(s);
                    let snapped = [
                        (pos[0] / grid).round() * grid,
                        (pos[1] / grid).round() * grid,
                        (pos[2] / grid).round() * grid,
                    ];
                    queue_sg
                        .lock()
                        .unwrap()
                        .push(EditorCommand::SetEntityTransform {
                            entity_id,
                            position: Some(snapped),
                            rotation: None,
                            scale: None,
                        });
                    McpToolOutput::success(json!({"status": "queued", "snapped_position": snapped}))
                }),
            });

            // get_scene_hierarchy
            let snap_hier = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_scene_hierarchy".to_string(),
                description:
                    "Return the full scene hierarchy as a nested tree of {id, name, children}"
                        .to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_hier.lock().unwrap();
                    use std::collections::HashMap;
                    let mut children_map: HashMap<u64, Vec<u64>> = HashMap::new();
                    let mut root_ids: Vec<u64> = Vec::new();
                    for e in &s.entities {
                        match e.parent_id {
                            Some(pid) => children_map.entry(pid).or_default().push(e.id),
                            None => root_ids.push(e.id),
                        }
                    }
                    fn build_node(
                        id: u64,
                        entities: &[crate::snapshot::EntityInfo],
                        children_map: &HashMap<u64, Vec<u64>>,
                    ) -> serde_json::Value {
                        let name = entities
                            .iter()
                            .find(|e| e.id == id)
                            .and_then(|e| e.name.clone());
                        let children: Vec<serde_json::Value> = children_map
                            .get(&id)
                            .map(|ids| {
                                ids.iter()
                                    .map(|&cid| build_node(cid, entities, children_map))
                                    .collect()
                            })
                            .unwrap_or_default();
                        json!({"id": id, "name": name, "children": children})
                    }
                    let roots: Vec<serde_json::Value> = root_ids
                        .iter()
                        .map(|&id| build_node(id, &s.entities, &children_map))
                        .collect();
                    McpToolOutput::success(json!({"roots": roots}))
                }),
            });

            // copy_transform
            let snap_ct = snapshot.clone();
            let queue_ct = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "copy_transform".to_string(),
                description: "Copy position, rotation, and scale from one entity to another (applied next frame)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "from_entity_id": { "type": "integer" },
                        "to_entity_id": { "type": "integer" }
                    },
                    "required": ["from_entity_id", "to_entity_id"]
                })),
                handler: Box::new(move |input| {
                    let from_id = match input["from_entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing from_entity_id"),
                    };
                    let to_id = match input["to_entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing to_entity_id"),
                    };
                    let s = snap_ct.lock().unwrap();
                    let src = match s.entities.iter().find(|e| e.id == from_id) {
                        Some(e) => e.clone(),
                        None => return McpToolOutput::error("source entity not found"),
                    };
                    if s.entities.iter().find(|e| e.id == to_id).is_none() {
                        return McpToolOutput::error("target entity not found");
                    }
                    drop(s);
                    queue_ct.lock().unwrap().push(EditorCommand::SetEntityTransform {
                        entity_id: to_id,
                        position: src.position,
                        rotation: src.rotation,
                        scale: src.scale,
                    });
                    McpToolOutput::success(json!({"status": "queued", "from": from_id, "to": to_id}))
                }),
            });

            // has_component
            let snap_hc = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "has_component".to_string(),
                description: "Check whether an entity has a given component type (mesh, camera, point_light, directional_light, spot_light, transform)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer" },
                        "component": { "type": "string" }
                    },
                    "required": ["entity_id", "component"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let component = match input["component"].as_str() {
                        Some(c) => c.to_string(),
                        None => return McpToolOutput::error("missing component"),
                    };
                    let s = snap_hc.lock().unwrap();
                    let e = match s.entities.iter().find(|e| e.id == entity_id) {
                        Some(e) => e,
                        None => return McpToolOutput::error("entity not found"),
                    };
                    let has = match component.as_str() {
                        "mesh" => e.mesh_id.is_some(),
                        "camera" => e.camera_fov.is_some(),
                        "point_light" => e.light_type.as_deref() == Some("point"),
                        "directional_light" => e.light_type.as_deref() == Some("directional"),
                        "spot_light" => e.light_type.as_deref() == Some("spot"),
                        "transform" => e.position.is_some(),
                        _ => return McpToolOutput::error("unknown component type"),
                    };
                    McpToolOutput::success(json!({"has_component": has, "entity_id": entity_id, "component": component}))
                }),
            });

            // rotate_selected_entities
            let sel_rotate = selection.clone();
            let queue_rotate = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "rotate_selected_entities".to_string(),
                description:
                    "Set rotation (Euler degrees) for all selected entities (applied next frame)"
                        .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "rx": { "type": "number" },
                        "ry": { "type": "number" },
                        "rz": { "type": "number" }
                    },
                    "required": ["rx", "ry", "rz"]
                })),
                handler: Box::new(move |input| {
                    let rx = input["rx"].as_f64().unwrap_or(0.0) as f32;
                    let ry = input["ry"].as_f64().unwrap_or(0.0) as f32;
                    let rz = input["rz"].as_f64().unwrap_or(0.0) as f32;
                    let ids: Vec<u64> = sel_rotate.lock().unwrap().iter().copied().collect();
                    let count = ids.len();
                    let mut queue = queue_rotate.lock().unwrap();
                    for entity_id in ids {
                        queue.push(EditorCommand::SetRotation {
                            entity_id,
                            rx,
                            ry,
                            rz,
                        });
                    }
                    McpToolOutput::success(json!({"status": "queued", "count": count}))
                }),
            });

            // scale_selected_entities
            let sel_scale = selection.clone();
            let queue_scale = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "scale_selected_entities".to_string(),
                description: "Set scale for all selected entities (applied next frame)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "sx": { "type": "number" },
                        "sy": { "type": "number" },
                        "sz": { "type": "number" }
                    },
                    "required": ["sx", "sy", "sz"]
                })),
                handler: Box::new(move |input| {
                    let sx = input["sx"].as_f64().unwrap_or(1.0) as f32;
                    let sy = input["sy"].as_f64().unwrap_or(1.0) as f32;
                    let sz = input["sz"].as_f64().unwrap_or(1.0) as f32;
                    let ids: Vec<u64> = sel_scale.lock().unwrap().iter().copied().collect();
                    let count = ids.len();
                    let mut queue = queue_scale.lock().unwrap();
                    for entity_id in ids {
                        queue.push(EditorCommand::SetScale {
                            entity_id,
                            sx,
                            sy,
                            sz,
                        });
                    }
                    McpToolOutput::success(json!({"status": "queued", "count": count}))
                }),
            });

            // toggle_visible
            let snap_toggle = snapshot.clone();
            let queue_toggle = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "toggle_visible".to_string(),
                description: "Toggle the visible state of an entity (applied next frame)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "entity_id": { "type": "integer" } },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let current = snap_toggle.lock().unwrap()
                        .entities.iter()
                        .find(|e| e.id == entity_id)
                        .map(|e| e.visible)
                        .unwrap_or(true);
                    queue_toggle.lock().unwrap()
                        .push(EditorCommand::SetVisible { entity_id, visible: !current });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id, "new_visible": !current}))
                }),
            });

            // despawn_selected
            let sel_despawn = selection.clone();
            let queue_despawn = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "despawn_selected".to_string(),
                description:
                    "Despawn all selected entities and clear selection (applied next frame)"
                        .to_string(),
                input_schema: Some(json!({ "type": "object" })),
                handler: Box::new(move |_input| {
                    let mut sel = sel_despawn.lock().unwrap();
                    let ids: Vec<u64> = sel.iter().copied().collect();
                    let count = ids.len();
                    let mut queue = queue_despawn.lock().unwrap();
                    for entity_id in ids {
                        queue.push(EditorCommand::Despawn { entity_id });
                    }
                    sel.clear();
                    McpToolOutput::success(json!({"status": "queued", "count": count}))
                }),
            });

            // hide_selected
            let sel_hide = selection.clone();
            let queue_hide = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "hide_selected".to_string(),
                description: "Hide all selected entities (visible=false, applied next frame)"
                    .to_string(),
                input_schema: Some(json!({ "type": "object" })),
                handler: Box::new(move |_input| {
                    let ids: Vec<u64> = sel_hide.lock().unwrap().iter().copied().collect();
                    let count = ids.len();
                    let mut queue = queue_hide.lock().unwrap();
                    for entity_id in ids {
                        queue.push(EditorCommand::SetVisible {
                            entity_id,
                            visible: false,
                        });
                    }
                    McpToolOutput::success(json!({"status": "queued", "count": count}))
                }),
            });

            // show_selected
            let sel_show = selection.clone();
            let queue_show = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "show_selected".to_string(),
                description: "Show all selected entities (visible=true, applied next frame)"
                    .to_string(),
                input_schema: Some(json!({ "type": "object" })),
                handler: Box::new(move |_input| {
                    let ids: Vec<u64> = sel_show.lock().unwrap().iter().copied().collect();
                    let count = ids.len();
                    let mut queue = queue_show.lock().unwrap();
                    for entity_id in ids {
                        queue.push(EditorCommand::SetVisible {
                            entity_id,
                            visible: true,
                        });
                    }
                    McpToolOutput::success(json!({"status": "queued", "count": count}))
                }),
            });

            // move_selected_entities
            let sel_move = selection.clone();
            let queue_move = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "move_selected_entities".to_string(),
                description: "Move all selected entities by (dx, dy, dz) (applied next frame)"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "dx": { "type": "number" },
                        "dy": { "type": "number" },
                        "dz": { "type": "number" }
                    },
                    "required": ["dx", "dy", "dz"]
                })),
                handler: Box::new(move |input| {
                    let dx = input["dx"].as_f64().unwrap_or(0.0) as f32;
                    let dy = input["dy"].as_f64().unwrap_or(0.0) as f32;
                    let dz = input["dz"].as_f64().unwrap_or(0.0) as f32;
                    let ids: Vec<u64> = sel_move.lock().unwrap().iter().copied().collect();
                    let count = ids.len();
                    let mut queue = queue_move.lock().unwrap();
                    for entity_id in ids {
                        queue.push(EditorCommand::MoveEntity {
                            entity_id,
                            dx,
                            dy,
                            dz,
                        });
                    }
                    McpToolOutput::success(json!({"status": "queued", "count": count}))
                }),
            });

            // select_all
            let sel5 = selection.clone();
            let snap_sel = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "select_all".to_string(),
                description: "Select all entities in the scene (immediate)".to_string(),
                input_schema: Some(json!({ "type": "object" })),
                handler: Box::new(move |_input| {
                    let ids: Vec<u64> = snap_sel
                        .lock()
                        .unwrap()
                        .entities
                        .iter()
                        .map(|e| e.id)
                        .collect();
                    let count = ids.len();
                    let mut sel = sel5.lock().unwrap();
                    for id in ids {
                        sel.insert(id);
                    }
                    McpToolOutput::success(json!({"status": "selected", "count": count}))
                }),
            });

            // deselect_all
            let sel6 = selection.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "deselect_all".to_string(),
                description: "Deselect all entities (immediate)".to_string(),
                input_schema: Some(json!({ "type": "object" })),
                handler: Box::new(move |_input| {
                    sel6.lock().unwrap().clear();
                    McpToolOutput::success(json!({"status": "cleared"}))
                }),
            });

            // hide_entity
            let queue27 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "hide_entity".to_string(),
                description: "Mark an entity as hidden (visible=false, applied next frame)"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer" }
                    },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    queue27.lock().unwrap().push(EditorCommand::SetVisible {
                        entity_id,
                        visible: false,
                    });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // show_entity
            let queue28 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "show_entity".to_string(),
                description: "Mark an entity as visible (visible=true, applied next frame)"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer" }
                    },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    queue28.lock().unwrap().push(EditorCommand::SetVisible {
                        entity_id,
                        visible: true,
                    });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // set_parent
            let queue23 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "set_parent".to_string(),
                description:
                    "Set the parent of an entity to establish a hierarchy (applied next frame)"
                        .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer" },
                        "parent_id": { "type": "integer" }
                    },
                    "required": ["entity_id", "parent_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let parent_id = match input["parent_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing parent_id"),
                    };
                    queue23.lock().unwrap().push(EditorCommand::SetParent {
                        entity_id,
                        parent_id,
                    });
                    McpToolOutput::success(
                        json!({"status": "queued", "entity_id": entity_id, "parent_id": parent_id}),
                    )
                }),
            });

            // remove_parent
            let queue24 = cmd_queue.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "remove_parent".to_string(),
                description:
                    "Remove the parent of an entity, making it a root entity (applied next frame)"
                        .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer" }
                    },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    queue24
                        .lock()
                        .unwrap()
                        .push(EditorCommand::RemoveParent { entity_id });
                    McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
                }),
            });

            // set_reflected_component — generic attach-or-update for any
            // #[derive(Reflect, Component)] type, from a JSON value matching its
            // field shape. Closes the gap where AnimationStateMachine/NavMeshAgent/
            // Shield/Bloom/ToneMap have no attach path anywhere in the ~700-tool
            // MCP surface (their scripting setters are no-ops on a missing
            // component; only the native GUI's "Add Component" picker could attach
            // them before this). Reuses the exact ApplyComponentValue path the
            // GUI's own reflected field editor already goes through.
            let reflect_queue_for_set = reflect_cmd_queue.clone();
            let type_registry_for_set = type_registry.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "set_reflected_component".to_string(),
                description: "Attach (if missing) or update a reflected component on an \
                    entity by its fully-qualified type path, from a JSON value matching \
                    the component's field shape. Applied next frame via the same path \
                    the Inspector's generic reflected field editor uses. Works for any \
                    type registered with app.register_type -- including \
                    AnimationStateMachine, NavMeshAgent, Shield, Bloom, ToneMap, and any \
                    other component with no dedicated attach tool. Example value_json for \
                    NavMeshAgent: {\"destination\": null, \"speed\": 3.5, \"angular_speed\": \
                    2.0, \"acceleration\": 8.0, \"stopping_distance\": 0.1, \"radius\": 0.3, \
                    \"height\": 1.8, \"state\": \"Idle\", \"enabled\": true}"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer" },
                        "type_path": {
                            "type": "string",
                            "description": "e.g. bsengine_core::nav_mesh_agent::NavMeshAgent"
                        },
                        "value_json": {
                            "type": "string",
                            "description": "JSON object matching the component's fields"
                        },
                    },
                    "required": ["entity_id", "type_path", "value_json"],
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(v) => v,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let type_path = match input["type_path"].as_str() {
                        Some(v) => v.to_string(),
                        None => return McpToolOutput::error("missing type_path"),
                    };
                    let value_json = match input["value_json"].as_str() {
                        Some(v) => v.to_string(),
                        None => return McpToolOutput::error("missing value_json"),
                    };
                    let registry = type_registry_for_set.read();
                    let Some(registration) = registry.get_with_type_path(&type_path) else {
                        return McpToolOutput::error(&format!(
                            "unknown type path '{type_path}'"
                        ));
                    };
                    let de = bevy_reflect::serde::TypedReflectDeserializer::new(
                        registration,
                        &registry,
                    );
                    let mut deserializer = serde_json::Deserializer::from_str(&value_json);
                    let value =
                        match serde::de::DeserializeSeed::deserialize(de, &mut deserializer) {
                            Ok(v) => v,
                            Err(e) => {
                                return McpToolOutput::error(&format!(
                                    "value_json doesn't match '{type_path}': {e}"
                                ))
                            }
                        };
                    reflect_queue_for_set.lock().unwrap().push(
                        ReflectCommand::ApplyComponentValue {
                            entity_id,
                            type_path,
                            value,
                        },
                    );
                    McpToolOutput::success(json!({ "queued": true }))
                }),
            });
        }
    }
}

fn parse_vec3_input(v: &serde_json::Value) -> Option<[f32; 3]> {
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

#[cfg(test)]
mod tests {
    use super::{build_entity_descriptors, save_entities_as_prefab, EditorPlugin};
    use bsengine_app::new_app;
    use bsengine_core::{InspectorCmd, InspectorState, Parent, Transform};
    use bsengine_mcp::{McpPlugin, McpRegistryResource};
    use bsengine_scene::Name;
    use glam::Vec3;
    use serde_json::json;

    use crate::snapshot::{
        EditorCommand, EditorCommandQueueResource, EditorSelectionResource, EditorSnapshotResource,
        EntityInfo, Tags,
    };

    #[test]
    fn editor_plugin_builds_without_panic() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.update();
    }

    /// A preview request must take over the editor's view-projection, and the
    /// orbit state must win when there is none.
    ///
    /// Both halves are needed: "the view-projection changed" on its own is
    /// satisfied by an override that is always on, which would make the
    /// editor camera unusable and still pass.
    #[test]
    fn a_preview_request_overrides_the_orbit_camera() {
        use bsengine_core::{PreviewCamera, TimelinePreview};

        let mut app = new_app();
        app.insert_resource({
            // Field-by-field rather than functional-update syntax:
            // `InspectorState` has a private field, so `..Default::default()`
            // is barred outside `bsengine-core`.
            let mut insp = InspectorState::default();
            insp.editor_mode = true;
            insp.viewport_size = [800.0, 600.0];
            insp
        });
        app.add_systems(bevy_app::Update, super::update_editor_camera);

        app.update();
        let orbit = app
            .world()
            .resource::<InspectorState>()
            .editor_view_proj
            .expect("the orbit camera always publishes one");

        app.world_mut()
            .resource_mut::<InspectorState>()
            .timeline_preview = Some(TimelinePreview {
            camera: Some(PreviewCamera {
                position: [100.0, 50.0, 25.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                fov_y_degrees: None,
            }),
            clips: Vec::new(),
        });
        app.update();
        let previewing = app
            .world()
            .resource::<InspectorState>()
            .editor_view_proj
            .expect("still published while previewing");
        assert_ne!(
            orbit, previewing,
            "a preview request must take over the editor's view-projection"
        );

        app.world_mut()
            .resource_mut::<InspectorState>()
            .timeline_preview = None;
        app.update();
        let restored = app
            .world()
            .resource::<InspectorState>()
            .editor_view_proj
            .expect("published again once preview ends");
        assert_eq!(
            orbit, restored,
            "clearing the preview must return the user to the orbit viewpoint \
             they had, not leave the camera stranded at the cutscene"
        );
    }

    #[test]
    fn editor_plugin_inserts_panel_registry() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.update();

        let registry = app.world().resource::<bsengine_core::EditorPanelRegistry>();
        assert!(registry.0.lock().unwrap().is_empty());
    }

    #[test]
    fn build_entity_descriptors_resolves_parent_id_to_the_parents_name() {
        let entities = vec![
            EntityInfo {
                id: 1,
                name: Some("Body".to_string()),
                parent_id: None,
                ..Default::default()
            },
            EntityInfo {
                id: 2,
                name: Some("Wheel".to_string()),
                parent_id: Some(1),
                ..Default::default()
            },
        ];

        let descriptors = build_entity_descriptors(&entities);
        let wheel = descriptors
            .iter()
            .find(|d| d.name == "Wheel")
            .expect("Wheel should be in the output");
        assert_eq!(wheel.parent.as_deref(), Some("Body"));

        let body = descriptors
            .iter()
            .find(|d| d.name == "Body")
            .expect("Body should be in the output");
        assert_eq!(body.parent, None, "Body has no parent_id, so no parent name");
    }

    #[test]
    fn build_entity_descriptors_warns_and_drops_parent_link_when_parent_has_no_name() {
        // Ensures the tracing subscriber exists even when this test runs in
        // isolation (e.g. via a `cargo test <name>` filter), so the warn!
        // below is actually visible under `--nocapture`.
        bsengine_core::init_logging();

        let entities = vec![
            // e.g. a spawned point light: has an id but no Name component.
            EntityInfo {
                id: 1,
                name: None,
                parent_id: None,
                ..Default::default()
            },
            EntityInfo {
                id: 2,
                name: Some("Wheel".to_string()),
                parent_id: Some(1),
                ..Default::default()
            },
        ];

        let descriptors = build_entity_descriptors(&entities);
        let wheel = descriptors
            .iter()
            .find(|d| d.name == "Wheel")
            .expect("Wheel should be in the output");
        assert_eq!(
            wheel.parent, None,
            "parent entity has no Name, so the link can't be preserved in the scene file"
        );
    }

    #[test]
    fn populate_inspector_copies_parent_tags_script_primitive() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let parent = app.world_mut().spawn(Name("Parent".to_string())).id();
        let child = app
            .world_mut()
            .spawn((
                Name("Child".to_string()),
                bsengine_core::Parent(parent),
                crate::snapshot::Tags(vec!["enemy".to_string(), "boss".to_string()]),
                bsengine_scene::ScriptPath("assets/scripts/child.js".to_string()),
                bsengine_scene::PrimitiveMesh(bsengine_scene::Primitive::Sphere),
            ))
            .id();
        app.update();

        let insp = app.world().resource::<InspectorState>();
        let child_info = insp
            .entities
            .iter()
            .find(|e| e.id == child.index() as u64)
            .expect("child entity should be in inspector snapshot");
        assert_eq!(child_info.parent_id, Some(parent.index() as u64));
        assert_eq!(
            child_info.tags,
            vec!["enemy".to_string(), "boss".to_string()]
        );
        assert_eq!(
            child_info.script_path,
            Some("assets/scripts/child.js".to_string())
        );
        assert_eq!(child_info.primitive, Some("sphere".to_string()));
    }

    #[test]
    fn primitive_kinds_const_round_trips_through_str_conversions() {
        // Every string in the canonical `PRIMITIVE_KINDS` list (the
        // lowercase string form of `bsengine_scene::Primitive` used at the
        // `InspectorEntityInfo`/`InspectorCmd::AttachPrimitiveMesh`
        // DTO boundary, since the Inspector no longer has a dedicated Mesh
        // dropdown -- `PrimitiveMesh` attaches through the generic Add
        // Component menu like any other component) must parse via
        // `str_to_primitive` and the parsed value must map back to the same
        // string via `primitive_to_str`.
        //
        // This used to concede that it could not catch a *new* `Primitive`
        // variant going unlisted. It can now: `every_variant` below holds an
        // exhaustive match, so adding a variant to `Primitive` stops this
        // crate compiling until the variant is also added to the list, and the
        // loop then proves it round-trips. That gap was not hypothetical --
        // adding `Cylinder` left `PRIMITIVE_KINDS` at 4 entries, which the
        // editor dropdown silently omits, and only the hardcoded count caught
        // it.
        fn every_variant() -> Vec<bsengine_scene::Primitive> {
            use bsengine_scene::Primitive as P;
            // Exhaustive on purpose. Do not add a `_` arm: this match failing
            // to compile IS the guard.
            match P::Cube {
                P::Cube | P::Sphere | P::Plane | P::Capsule | P::Cylinder => {}
            }
            vec![P::Cube, P::Sphere, P::Plane, P::Capsule, P::Cylinder]
        }

        let variants = every_variant();
        assert_eq!(
            bsengine_core::PRIMITIVE_KINDS.len(),
            variants.len(),
            "every `Primitive` variant needs an entry in `PRIMITIVE_KINDS`, or              the editor's primitive dropdown silently omits it"
        );
        for p in &variants {
            let name = super::primitive_to_str(p);
            assert!(
                bsengine_core::PRIMITIVE_KINDS.contains(&name.as_str()),
                "`Primitive::{p:?}` maps to {name:?}, which is not in                  PRIMITIVE_KINDS"
            );
        }
        for &kind in &bsengine_core::PRIMITIVE_KINDS {
            let parsed = super::str_to_primitive(kind)
                .unwrap_or_else(|| panic!("PRIMITIVE_KINDS entry {kind:?} did not parse"));
            assert_eq!(
                super::primitive_to_str(&parsed),
                kind,
                "str_to_primitive/primitive_to_str do not round-trip for {kind:?}"
            );
        }
    }

    #[test]
    fn reflect_command_attaches_and_removes_component_by_type_path() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app.world_mut().spawn(Name("Target".to_string())).id();
        app.update();

        {
            let queue = app.world().resource::<crate::snapshot::ReflectCommandQueueResource>();
            queue.0.lock().unwrap().push(crate::snapshot::ReflectCommand::AttachComponentByType {
                entity_id: eid.index() as u64,
                type_path: "bsengine_core::camera::Camera".to_string(),
            });
        }
        app.update();

        assert!(
            app.world().get::<bsengine_core::Camera>(eid).is_some(),
            "Camera was not attached via ReflectCommand"
        );

        {
            let queue = app.world().resource::<crate::snapshot::ReflectCommandQueueResource>();
            queue.0.lock().unwrap().push(crate::snapshot::ReflectCommand::RemoveComponentByType {
                entity_id: eid.index() as u64,
                type_path: "bsengine_core::camera::Camera".to_string(),
            });
        }
        app.update();

        assert!(
            app.world().get::<bsengine_core::Camera>(eid).is_none(),
            "Camera was not removed via ReflectCommand"
        );
    }

    #[test]
    fn reflect_command_apply_component_value_mutates_attached_component() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app
            .world_mut()
            .spawn((Name("Target".to_string()), bsengine_core::Camera::default()))
            .id();
        app.update();

        let edited = bsengine_core::Camera {
            fov_y_degrees: 1.2345.into(),
            ..Default::default()
        };
        {
            let queue = app.world().resource::<crate::snapshot::ReflectCommandQueueResource>();
            queue.0.lock().unwrap().push(crate::snapshot::ReflectCommand::ApplyComponentValue {
                entity_id: eid.index() as u64,
                type_path: "bsengine_core::camera::Camera".to_string(),
                value: Box::new(edited),
            });
        }
        app.update();

        let cam = app
            .world()
            .get::<bsengine_core::Camera>(eid)
            .expect("Camera should still be attached");
        assert!(
            (cam.fov_y_degrees.0 - 1.2345).abs() < f32::EPSILON,
            "Camera.fov_y_degrees should have been updated to the applied value"
        );
    }

    #[test]
    fn reflect_command_apply_component_value_mutates_a_list_shaped_component() {
        // `Tags` (a `Vec<String>`-wrapping tuple struct) is the best
        // available `List`-shaped reflected component. `reflect_ui.rs`'s
        // List tests exercise the UI-only rendering in isolation (no ECS),
        // and the other `ApplyComponentValue` tests in this module only
        // cover `Struct`-shaped components (`Camera`, `Follow`) -- nothing
        // proves a `List`-shaped component round-trips through the full
        // command pipeline. This does: InspectorCmd::ApplyReflectedComponent
        // -> ReflectCommand::ApplyComponentValue ->
        // ReflectComponent::apply_or_insert.
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app
            .world_mut()
            .spawn((Name("Target".to_string()), Tags(vec!["a".to_string()])))
            .id();
        app.update();

        // Simulates what the List-editing UI's "+" button would produce:
        // the existing value plus one appended element.
        let edited = Tags(vec!["a".to_string(), "b".to_string()]);
        {
            let queue = app
                .world()
                .resource::<crate::snapshot::ReflectCommandQueueResource>();
            queue
                .0
                .lock()
                .unwrap()
                .push(crate::snapshot::ReflectCommand::ApplyComponentValue {
                    entity_id: eid.index() as u64,
                    type_path: "bsengine_editor::snapshot::Tags".to_string(),
                    value: Box::new(edited),
                });
        }
        app.update();

        let tags = app
            .world()
            .get::<Tags>(eid)
            .expect("Tags should still be attached");
        assert_eq!(
            tags.0,
            vec!["a".to_string(), "b".to_string()],
            "Tags(Vec<String>) should round-trip through the full reflect command \
             pipeline end-to-end, not just the UI-only or snapshot-only layers"
        );
    }

    #[test]
    fn apply_component_value_fixes_up_a_stale_entity_field_generation() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        // Spawn, despawn, and respawn at the same index so the live entity's
        // generation is now > 0 -- Entity::from_raw(index) (generation 0)
        // would NOT equal this live entity.
        let throwaway = app.world_mut().spawn(Name("Throwaway".to_string())).id();
        app.world_mut().despawn(throwaway);
        let target = app
            .world_mut()
            .spawn(Name("Target".to_string()))
            .id();
        assert_eq!(
            target.index(),
            throwaway.index(),
            "test setup requires the respawn to reuse the same index"
        );
        assert_ne!(
            target,
            bevy_ecs::prelude::Entity::from_raw(target.index()),
            "test setup requires a nonzero generation to actually exercise the fixup"
        );

        let follower = app.world_mut().spawn(Name("Follower".to_string())).id();
        app.update();

        let stale_follow = bsengine_core::Follow::new(bevy_ecs::prelude::Entity::from_raw(target.index()));
        {
            let queue = app
                .world()
                .resource::<crate::snapshot::ReflectCommandQueueResource>();
            queue
                .0
                .lock()
                .unwrap()
                .push(crate::snapshot::ReflectCommand::ApplyComponentValue {
                    entity_id: follower.index() as u64,
                    type_path: "bsengine_core::follow::Follow".to_string(),
                    value: Box::new(stale_follow),
                });
        }
        app.update();

        let applied = app
            .world()
            .get::<bsengine_core::Follow>(follower)
            .expect("Follow should have been applied");
        assert_eq!(
            applied.target, target,
            "the applied Follow.target must be fixed up to the live entity's real generation, \
             not the stale generation-0 placeholder the UI layer constructed"
        );
    }

    #[test]
    fn apply_component_value_fixes_up_stale_entity_generations_inside_array_and_map_fields() {
        use bevy_ecs::prelude::{Component, ReflectComponent};
        use bevy_reflect::Reflect;

        // Test-only component whose reflect tree puts an `Entity` inside
        // both an `Array` (`[Entity; 2]`) and a `Map`
        // (`HashMap<String, Entity>`) field -- proving `fixup_entity_fields`'s
        // `Array`/`Map` arms actually recurse into their elements, rather
        // than silently falling through `_ => {}` like before this fix.
        #[derive(Component, Clone, Debug, Reflect)]
        #[reflect(Component)]
        struct EntityCollections {
            array: [bevy_ecs::prelude::Entity; 2],
            map: std::collections::HashMap<String, bevy_ecs::prelude::Entity>,
        }

        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.register_type::<EntityCollections>();

        // Spawn, despawn, and respawn at the same index so the live entity's
        // generation is now > 0 -- Entity::from_raw(index) (generation 0)
        // would NOT equal this live entity (same technique as
        // `apply_component_value_fixes_up_a_stale_entity_field_generation`).
        let throwaway = app.world_mut().spawn(Name("Throwaway".to_string())).id();
        app.world_mut().despawn(throwaway);
        let target = app.world_mut().spawn(Name("Target".to_string())).id();
        assert_eq!(
            target.index(),
            throwaway.index(),
            "test setup requires the respawn to reuse the same index"
        );
        assert_ne!(
            target,
            bevy_ecs::prelude::Entity::from_raw(target.index()),
            "test setup requires a nonzero generation to actually exercise the fixup"
        );

        let holder = app.world_mut().spawn(Name("Holder".to_string())).id();
        app.update();

        let stale = bevy_ecs::prelude::Entity::from_raw(target.index());
        let mut map = std::collections::HashMap::new();
        map.insert("target".to_string(), stale);
        let value = EntityCollections { array: [stale, stale], map };

        let type_path = <EntityCollections as bevy_reflect::TypePath>::type_path().to_string();
        {
            let queue = app
                .world()
                .resource::<crate::snapshot::ReflectCommandQueueResource>();
            queue
                .0
                .lock()
                .unwrap()
                .push(crate::snapshot::ReflectCommand::ApplyComponentValue {
                    entity_id: holder.index() as u64,
                    type_path,
                    value: Box::new(value),
                });
        }
        app.update();

        let applied = app
            .world()
            .get::<EntityCollections>(holder)
            .expect("EntityCollections should have been applied");
        assert_eq!(
            applied.array[0], target,
            "Array element containing a stale Entity must be fixed up to the live generation"
        );
        assert_eq!(
            applied.array[1], target,
            "Array element containing a stale Entity must be fixed up to the live generation"
        );
        assert_eq!(
            applied.map.get("target"),
            Some(&target),
            "Map value containing a stale Entity must be fixed up to the live generation"
        );
    }

    #[test]
    fn inspector_cmd_apply_reflected_component_reaches_reflect_queue() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app
            .world_mut()
            .spawn((Name("Target".to_string()), bsengine_core::Camera::default()))
            .id();
        app.update();

        let edited = bsengine_core::Camera {
            fov_y_degrees: 0.5.into(),
            ..Default::default()
        };
        {
            let mut insp = app.world_mut().resource_mut::<InspectorState>();
            insp.cmd_queue.push(InspectorCmd::ApplyReflectedComponent {
                id: eid.index() as u64,
                type_path: "bsengine_core::camera::Camera".to_string(),
                value: Box::new(edited),
            });
        }
        app.update();

        let cam = app.world().get::<bsengine_core::Camera>(eid).expect("Camera should exist");
        assert!(
            (cam.fov_y_degrees.0 - 0.5).abs() < f32::EPSILON,
            "Camera should have been updated end-to-end via InspectorCmd"
        );
    }

    #[test]
    fn reflect_command_queue_pushes_undo_checkpoint() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app.world_mut().spawn(Name("Target".to_string())).id();
        app.update();

        let history_len_before = app
            .world()
            .resource::<crate::snapshot::EditorHistoryResource>()
            .0
            .lock()
            .unwrap()
            .undo_stack
            .len();

        {
            let queue = app.world().resource::<crate::snapshot::ReflectCommandQueueResource>();
            queue.0.lock().unwrap().push(crate::snapshot::ReflectCommand::AttachComponentByType {
                entity_id: eid.index() as u64,
                type_path: "bsengine_core::camera::Camera".to_string(),
            });
        }
        app.update();

        let history_len_after = app
            .world()
            .resource::<crate::snapshot::EditorHistoryResource>()
            .0
            .lock()
            .unwrap()
            .undo_stack
            .len();
        assert_eq!(
            history_len_after,
            history_len_before + 1,
            "ReflectCommand processing should push an undo checkpoint, same as EditorCommand does"
        );
    }

    #[test]
    fn populate_reflected_component_snapshot_clones_attached_camera() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app
            .world_mut()
            .spawn((Name("Target".to_string()), bsengine_core::Camera::default()))
            .id();
        {
            let mut insp = app.world_mut().resource_mut::<InspectorState>();
            insp.selected_id = Some(eid.index() as u64);
        }
        app.update();

        let insp = app.world().resource::<InspectorState>();
        let camera_entry = insp
            .reflected_components
            .iter()
            .find(|(type_path, _)| type_path == "bsengine_core::camera::Camera")
            .expect("Camera should be in reflected_components after selecting the entity");
        let cloned_camera = camera_entry
            .1
            .as_ref()
            .downcast_ref::<bsengine_core::Camera>()
            .expect("cloned value should downcast back to Camera");
        assert!(
            (cloned_camera.fov_y_degrees.0 - 60.0).abs() < f32::EPSILON,
            "cloned Camera's fov_y_degrees should be 60.0 (degrees) — proves the field is wired \
             through the real reflection pipeline with the correct unit, not just internally \
             self-consistent"
        );
    }

    #[test]
    fn populate_reflected_component_snapshot_includes_attached_tags() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app
            .world_mut()
            .spawn((Name("Target".to_string()), Tags(vec!["enemy".to_string()])))
            .id();
        {
            let mut insp = app.world_mut().resource_mut::<InspectorState>();
            insp.selected_id = Some(eid.index() as u64);
        }
        app.update();

        let insp = app.world().resource::<InspectorState>();
        let found = insp
            .reflected_components
            .iter()
            .find(|(type_path, _)| type_path == "bsengine_editor::snapshot::Tags");
        assert!(
            found.is_some(),
            "Tags must appear in the Reflected Fields list once selected, now that it's a Reflect component"
        );
    }

    #[test]
    fn populate_reflected_component_snapshot_clears_when_nothing_selected() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app
            .world_mut()
            .spawn((Name("Target".to_string()), bsengine_core::Camera::default()))
            .id();
        {
            let mut insp = app.world_mut().resource_mut::<InspectorState>();
            insp.selected_id = Some(eid.index() as u64);
        }
        app.update();
        assert!(!app.world().resource::<InspectorState>().reflected_components.is_empty());

        {
            let mut insp = app.world_mut().resource_mut::<InspectorState>();
            insp.selected_id = None;
        }
        app.update();
        assert!(
            app.world().resource::<InspectorState>().reflected_components.is_empty(),
            "reflected_components should clear once nothing is selected"
        );
    }

    #[test]
    fn populate_reflected_component_snapshot_includes_attached_primitive_mesh_and_script_path() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app
            .world_mut()
            .spawn((
                Name("Target".to_string()),
                bsengine_scene::PrimitiveMesh(bsengine_scene::Primitive::Capsule),
                bsengine_scene::ScriptPath("assets/scripts/foo.js".to_string()),
            ))
            .id();
        {
            let mut insp = app.world_mut().resource_mut::<InspectorState>();
            insp.selected_id = Some(eid.index() as u64);
        }
        app.update();

        let insp = app.world().resource::<InspectorState>();
        assert!(
            insp.reflected_components
                .iter()
                .any(|(p, _)| p == "bsengine_scene::types::PrimitiveMesh"),
            "PrimitiveMesh must appear in Reflected Fields"
        );
        assert!(
            insp.reflected_components
                .iter()
                .any(|(p, _)| p == "bsengine_scene::types::ScriptPath"),
            "ScriptPath must appear in Reflected Fields"
        );
    }

    #[test]
    fn inspector_cmd_attach_component_by_type_reaches_reflect_queue() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app.world_mut().spawn(Name("Target".to_string())).id();
        app.update();

        {
            let mut insp = app.world_mut().resource_mut::<InspectorState>();
            insp.cmd_queue.push(InspectorCmd::AttachComponentByType {
                id: eid.index() as u64,
                type_path: "bsengine_core::camera::Camera".to_string(),
            });
        }
        app.update();

        assert!(
            app.world().get::<bsengine_core::Camera>(eid).is_some(),
            "Camera was not attached end-to-end via InspectorCmd"
        );
    }

    #[test]
    fn inspector_cmd_attach_component_by_type_inserts_a_real_default_primitive_mesh() {
        // Full verification would confirm `resolve_primitives`
        // (bsengine-runtime's system reacting to `Added<PrimitiveMesh>` by
        // inserting a derived `MeshRenderer`) still fires when the component
        // arrives via this reflected path rather than a typed
        // `commands.insert()`. That's not reachable from this crate's test
        // harness: `resolve_primitives` lives in `bsengine-runtime`, which
        // is a binary-only crate (no `[lib]` target -- see its Cargo.toml,
        // `[[bin]]` only) that itself depends on `bsengine-editor`, so there
        // is no way to add its plugin here without an unresolvable cycle
        // (and nothing to `use` even if there weren't one). See
        // `editor_command_detach_primitive_mesh_also_removes_derived_mesh_renderer`
        // above for the same limitation on the older typed-command path.
        //
        // This narrower test instead confirms the piece that *is* testable
        // here: `InspectorCmd::AttachComponentByType` (the unified Add
        // Component menu's mechanism) inserts a genuine, usable
        // `PrimitiveMesh` component via `ReflectComponent::insert`+
        // `ReflectDefault` -- not just a UI-only stub -- with its
        // `#[default]` variant (`Primitive::Cube`), the same starting point
        // a typed `commands.insert(PrimitiveMesh::default())` would produce.
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app.world_mut().spawn(Name("Target".to_string())).id();
        app.update();

        {
            let mut insp = app.world_mut().resource_mut::<InspectorState>();
            insp.cmd_queue.push(InspectorCmd::AttachComponentByType {
                id: eid.index() as u64,
                type_path: "bsengine_scene::types::PrimitiveMesh".to_string(),
            });
        }
        app.update();

        let mesh = app
            .world()
            .get::<bsengine_scene::PrimitiveMesh>(eid)
            .expect(
                "PrimitiveMesh should have been attached via the reflected \
                 AttachComponentByType path",
            );
        assert_eq!(
            mesh.0,
            bsengine_scene::Primitive::Cube,
            "the default PrimitiveMesh attached via ReflectDefault should be \
             Primitive::Cube per its #[default]"
        );
    }

    #[test]
    fn editor_plugin_registers_reflected_component_types() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.update();

        let registry = app.world().resource::<bevy_ecs::reflect::AppTypeRegistry>();
        let registry = registry.read();
        assert!(
            registry.get(std::any::TypeId::of::<bsengine_core::Camera>()).is_some(),
            "Camera not registered in AppTypeRegistry"
        );
        assert!(
            registry.get(std::any::TypeId::of::<bsengine_core::PointLight>()).is_some(),
            "PointLight not registered in AppTypeRegistry"
        );
    }

    #[test]
    fn editor_plugin_registers_lights_and_material_reflected_types() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.update();

        let registry = app.world().resource::<bevy_ecs::reflect::AppTypeRegistry>();
        let registry = registry.read();
        assert!(
            registry
                .get(std::any::TypeId::of::<bsengine_core::DirectionalLight>())
                .is_some(),
            "DirectionalLight not registered in AppTypeRegistry"
        );
        assert!(
            registry
                .get(std::any::TypeId::of::<bsengine_core::SpotLight>())
                .is_some(),
            "SpotLight not registered in AppTypeRegistry"
        );
        assert!(
            registry
                .get(std::any::TypeId::of::<bsengine_core::Material>())
                .is_some(),
            "Material not registered in AppTypeRegistry"
        );
    }

    #[test]
    fn editor_snapshot_reflects_named_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.world_mut().spawn(Name("Hero".to_string()));
        app.world_mut().spawn(Name("Camera".to_string()));
        app.update();

        let snapshot = app
            .world()
            .resource::<EditorSnapshotResource>()
            .0
            .lock()
            .unwrap();
        let names: Vec<_> = snapshot
            .entities
            .iter()
            .filter_map(|e| e.name.as_deref())
            .collect();
        assert!(names.contains(&"Hero"), "expected Hero in {:?}", names);
        assert!(names.contains(&"Camera"), "expected Camera in {:?}", names);
    }

    #[test]
    fn editor_snapshot_includes_transform_position() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.world_mut().spawn((
            Name("Box".to_string()),
            Transform::from_position(Vec3::new(1.0, 2.0, 3.0)),
        ));
        app.update();

        let snapshot = app
            .world()
            .resource::<EditorSnapshotResource>()
            .0
            .lock()
            .unwrap();
        let entity = snapshot
            .entities
            .iter()
            .find(|e| e.name.as_deref() == Some("Box"))
            .expect("Box not found");
        let pos = entity.position.expect("Box has no position");
        assert!((pos[0] - 1.0).abs() < 1e-5);
        assert!((pos[1] - 2.0).abs() < 1e-5);
        assert!((pos[2] - 3.0).abs() < 1e-5);
    }

    #[test]
    fn inspector_set_selection_updates_selection_resource() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let id_a = app.world_mut().spawn(Name("A".to_string())).id().index() as u64;
        let id_b = app.world_mut().spawn(Name("B".to_string())).id().index() as u64;
        app.update();

        app.world_mut()
            .resource_mut::<InspectorState>()
            .cmd_queue
            .push(InspectorCmd::SetSelection {
                ids: vec![id_a, id_b],
            });
        app.update();

        let selection = app
            .world()
            .resource::<EditorSelectionResource>()
            .0
            .lock()
            .unwrap();
        assert!(selection.contains(&id_a));
        assert!(selection.contains(&id_b));
        assert_eq!(selection.len(), 2);
    }

    #[test]
    fn editor_snapshot_reflects_multi_selection() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let id_a = app.world_mut().spawn(Name("A".to_string())).id().index() as u64;
        let id_b = app.world_mut().spawn(Name("B".to_string())).id().index() as u64;
        let id_c = app.world_mut().spawn(Name("C".to_string())).id().index() as u64;
        app.update();

        app.world_mut()
            .resource_mut::<InspectorState>()
            .cmd_queue
            .push(InspectorCmd::SetSelection {
                ids: vec![id_a, id_b],
            });
        app.update();
        app.update();

        let snapshot = app
            .world()
            .resource::<EditorSnapshotResource>()
            .0
            .lock()
            .unwrap();
        let selected = |id: u64| snapshot.entities.iter().find(|e| e.id == id).unwrap().selected;
        assert!(selected(id_a));
        assert!(selected(id_b));
        assert!(!selected(id_c));
    }

    #[test]
    fn inspector_set_selection_replaces_previous_selection() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let id_a = app.world_mut().spawn(Name("A".to_string())).id().index() as u64;
        let id_b = app.world_mut().spawn(Name("B".to_string())).id().index() as u64;
        app.update();

        app.world_mut()
            .resource_mut::<InspectorState>()
            .cmd_queue
            .push(InspectorCmd::SetSelection { ids: vec![id_a] });
        app.update();
        app.world_mut()
            .resource_mut::<InspectorState>()
            .cmd_queue
            .push(InspectorCmd::SetSelection { ids: vec![id_b] });
        app.update();

        let selection = app
            .world()
            .resource::<EditorSelectionResource>()
            .0
            .lock()
            .unwrap();
        assert!(!selection.contains(&id_a));
        assert!(selection.contains(&id_b));
        assert_eq!(selection.len(), 1);
    }

    #[test]
    fn undo_restores_despawned_entity() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let id = app
            .world_mut()
            .spawn((
                Name("Box".to_string()),
                Transform::from_position(Vec3::new(1.0, 2.0, 3.0)),
            ))
            .id()
            .index() as u64;
        app.update();

        {
            let queue_res = app.world().resource::<EditorCommandQueueResource>();
            queue_res
                .0
                .lock()
                .unwrap()
                .push(EditorCommand::Despawn { entity_id: id });
        }
        app.update();
        app.update();

        {
            let snapshot = app
                .world()
                .resource::<EditorSnapshotResource>()
                .0
                .lock()
                .unwrap();
            assert!(!snapshot.entities.iter().any(|e| e.name.as_deref() == Some("Box")));
        }

        app.world_mut()
            .resource_mut::<InspectorState>()
            .request_undo = true;
        app.update();
        app.update();

        let snapshot = app
            .world()
            .resource::<EditorSnapshotResource>()
            .0
            .lock()
            .unwrap();
        let restored = snapshot
            .entities
            .iter()
            .find(|e| e.name.as_deref() == Some("Box"))
            .expect("Box should be restored by undo");
        let pos = restored.position.expect("restored Box has a transform");
        assert!((pos[0] - 1.0).abs() < 1e-5);
        assert!((pos[1] - 2.0).abs() < 1e-5);
        assert!((pos[2] - 3.0).abs() < 1e-5);
    }

    #[test]
    fn redo_reapplies_despawn_after_undo() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let id = app
            .world_mut()
            .spawn(Name("Box".to_string()))
            .id()
            .index() as u64;
        app.update();

        {
            let queue_res = app.world().resource::<EditorCommandQueueResource>();
            queue_res
                .0
                .lock()
                .unwrap()
                .push(EditorCommand::Despawn { entity_id: id });
        }
        app.update();
        app.update();

        app.world_mut()
            .resource_mut::<InspectorState>()
            .request_undo = true;
        app.update();
        app.update();

        app.world_mut()
            .resource_mut::<InspectorState>()
            .request_redo = true;
        app.update();
        app.update();

        let snapshot = app
            .world()
            .resource::<EditorSnapshotResource>()
            .0
            .lock()
            .unwrap();
        assert!(
            !snapshot.entities.iter().any(|e| e.name.as_deref() == Some("Box")),
            "redo should reapply the despawn"
        );
    }

    #[test]
    fn undo_reverts_position_change() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let id = app
            .world_mut()
            .spawn((
                Name("Box".to_string()),
                Transform::from_position(Vec3::new(1.0, 0.0, 0.0)),
            ))
            .id()
            .index() as u64;
        app.update();

        {
            let queue_res = app.world().resource::<EditorCommandQueueResource>();
            queue_res.0.lock().unwrap().push(EditorCommand::SetPosition {
                entity_id: id,
                x: 9.0,
                y: 9.0,
                z: 9.0,
            });
        }
        app.update();
        app.update();

        {
            let snapshot = app
                .world()
                .resource::<EditorSnapshotResource>()
                .0
                .lock()
                .unwrap();
            let pos = snapshot
                .entities
                .iter()
                .find(|e| e.id == id)
                .and_then(|e| e.position)
                .expect("Box has a transform");
            assert!((pos[0] - 9.0).abs() < 1e-5);
        }

        app.world_mut()
            .resource_mut::<InspectorState>()
            .request_undo = true;
        app.update();
        app.update();

        let snapshot = app
            .world()
            .resource::<EditorSnapshotResource>()
            .0
            .lock()
            .unwrap();
        let pos = snapshot
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| e.position)
            .expect("Box still has a transform after undo");
        assert!((pos[0] - 1.0).abs() < 1e-5, "expected x reverted to 1.0, got {}", pos[0]);
    }

    #[test]
    fn undo_with_empty_history_does_not_panic() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.update();

        app.world_mut()
            .resource_mut::<InspectorState>()
            .request_undo = true;
        app.update();
        app.update();
    }

    #[test]
    fn inspector_duplicate_cmd_spawns_a_copy() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let id = app
            .world_mut()
            .spawn((
                Name("Box".to_string()),
                Transform::from_position(Vec3::new(1.0, 2.0, 3.0)),
            ))
            .id()
            .index() as u64;
        app.update();

        app.world_mut()
            .resource_mut::<InspectorState>()
            .cmd_queue
            .push(InspectorCmd::Duplicate { id });
        app.update();
        app.update();

        let snapshot = app
            .world()
            .resource::<EditorSnapshotResource>()
            .0
            .lock()
            .unwrap();
        assert!(
            snapshot
                .entities
                .iter()
                .any(|e| e.name.as_deref() == Some("Box (copy)")),
            "expected a duplicated entity named 'Box (copy)'"
        );
        assert_eq!(
            snapshot
                .entities
                .iter()
                .filter(|e| e.name.as_deref() == Some("Box"))
                .count(),
            1,
            "original Box should still exist exactly once"
        );
    }

    #[test]
    fn inspector_set_rotation_updates_transform() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let id = app
            .world_mut()
            .spawn((
                Name("Box".to_string()),
                Transform::from_position(Vec3::ZERO),
            ))
            .id()
            .index() as u64;
        app.update();

        app.world_mut()
            .resource_mut::<InspectorState>()
            .cmd_queue
            .push(InspectorCmd::SetRotation {
                id,
                rx: 0.0,
                ry: 90.0,
                rz: 0.0,
            });
        app.update();
        app.update();

        let snapshot = app
            .world()
            .resource::<EditorSnapshotResource>()
            .0
            .lock()
            .unwrap();
        let rot = snapshot
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| e.rotation)
            .expect("Box has a rotation");
        assert!(
            (rot[1] - 90.0).abs() < 1e-3,
            "expected y rotation ~90deg, got {:?}",
            rot
        );
    }

    #[test]
    fn mcp_list_entities_tool_registered() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .expect("list_entities not found");
        assert!(result.is_ok());
        assert!(result.content.get("entities").is_some());
    }

    #[test]
    fn mcp_spawn_entity_queues_spawn() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let result = mcp
                .0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "Sword"}))
                .expect("spawn_entity not found");
            assert!(result.is_ok());
            assert_eq!(result.content["status"], "queued");
        }

        app.update();

        let mut q = app.world_mut().query::<&Name>();
        let names: Vec<_> = q.iter(app.world()).map(|n| n.0.as_str()).collect();
        assert!(names.contains(&"Sword"), "Sword not spawned: {:?}", names);
    }

    #[test]
    fn mcp_get_entity_returns_info() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app
            .world_mut()
            .spawn((
                Name("Shield".to_string()),
                Transform::from_position(Vec3::new(5.0, 0.0, 0.0)),
            ))
            .id();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": eid.index() as u64}))
            .expect("get_entity not found");
        assert!(result.is_ok(), "error: {:?}", result.error);
        assert_eq!(result.content["entity"]["name"], "Shield");
        let pos = &result.content["entity"]["position"];
        assert!((pos[0].as_f64().unwrap() - 5.0).abs() < 1e-4);
    }

    #[test]
    fn mcp_despawn_entity_removes_it() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app.world_mut().spawn(Name("Temp".to_string())).id();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("despawn_entity", json!({"id": eid.index() as u64}))
                .expect("despawn_entity not found");
        }
        app.update();

        let mut q = app.world_mut().query::<&Name>();
        let names: Vec<_> = q.iter(app.world()).map(|n| n.0.as_str()).collect();
        assert!(!names.contains(&"Temp"), "Temp still alive: {:?}", names);
    }

    #[test]
    fn mcp_attach_mesh_adds_mesh_renderer() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app
            .world_mut()
            .spawn((
                Name("Cube".to_string()),
                Transform::from_position(Vec3::ZERO),
            ))
            .id();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let result = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "attach_mesh",
                    json!({"entity_id": eid.index() as u64, "mesh_id": 42u64}),
                )
                .expect("attach_mesh not found");
            assert!(result.is_ok(), "{:?}", result.error);
        }
        app.update();

        let mut q = app
            .world_mut()
            .query::<(&Name, &bsengine_render::MeshRenderer)>();
        let found = q
            .iter(app.world())
            .any(|(n, m)| n.0 == "Cube" && m.mesh_id == 42);
        assert!(found, "MeshRenderer not attached");
    }

    #[test]
    fn mcp_attach_physics_body_adds_physics_body_desc() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app
            .world_mut()
            .spawn((
                Name("Box".to_string()),
                Transform::from_position(Vec3::ZERO),
            ))
            .id();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let result = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "attach_physics_body",
                    json!({
                        "entity_id": eid.index() as u64,
                        "rigidbody": "Dynamic",
                        "collider_shape": "Sphere",
                        "radius": 0.75,
                    }),
                )
                .expect("attach_physics_body not found");
            assert!(result.is_ok(), "{:?}", result.error);
        }
        app.update();

        let desc = app
            .world()
            .get::<bsengine_scene::PhysicsBodyDesc>(eid)
            .expect("PhysicsBodyDesc should be attached");
        assert_eq!(desc.rigidbody, bsengine_scene::RigidBodyDesc::Dynamic);
        match &desc.collider.shape {
            bsengine_scene::ColliderShapeDesc::Sphere { radius } => {
                assert!((radius - 0.75).abs() < 1e-5);
            }
            other => panic!("expected Sphere shape, got {other:?}"),
        }
    }

    #[test]
    fn mcp_terrain_write_spawns_a_terrain_entity_with_the_given_params() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "terrain_write",
                    json!({
                        "heightmap_path": "assets/terrain/test_heightmap.png",
                        "chunk_count": [2, 2],
                        "chunk_size": 32.0,
                        "height_scale": 20.0,
                        "layer0_texture_path": "assets/terrain/grass.png",
                        "layer1_texture_path": "assets/terrain/rock.png",
                        "layer2_texture_path": "assets/terrain/dirt.png",
                        "layer3_texture_path": "assets/terrain/snow.png",
                    }),
                )
                .expect("terrain_write not registered");
            assert!(out.is_ok(), "terrain_write failed: {:?}", out.error);
        }
        app.update();

        // terrain_write spawns a brand-new entity (unlike attach_physics_body,
        // which attaches to a caller-supplied id), and the command-queue
        // pattern every other MCP spawn tool uses (spawn_point_light,
        // attach_physics_body, ...) is fire-and-forget with no synchronous id
        // returned in the response -- so the spawned entity is found by
        // querying for its Terrain component, mirroring
        // `spawn_mesh_asset_command_spawns_entity_with_name_and_gltf_asset`.
        let mut query = app.world_mut().query::<&bsengine_app::terrain::Terrain>();
        let terrain = query
            .iter(app.world())
            .next()
            .expect("expected one entity with a Terrain component");
        assert_eq!(terrain.heightmap_path, "assets/terrain/test_heightmap.png");
        assert_eq!(terrain.chunk_count, (2, 2));
        assert!((terrain.chunk_size - 32.0).abs() < 1e-5);
        assert!((terrain.height_scale - 20.0).abs() < 1e-5);
        assert_eq!(terrain.layer0_texture_path, "assets/terrain/grass.png");
        assert_eq!(terrain.layer1_texture_path, "assets/terrain/rock.png");
        assert_eq!(terrain.layer2_texture_path, "assets/terrain/dirt.png");
        assert_eq!(terrain.layer3_texture_path, "assets/terrain/snow.png");
    }

    /// `splatmap_path` is the one optional `terrain_write` arg (the other 6
    /// are `required`); the test above proves omitting it still works, and
    /// this proves the field round-trips onto the spawned `Terrain` when the
    /// caller does provide it.
    #[test]
    fn mcp_terrain_write_threads_an_optional_splatmap_path_through_when_given() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "terrain_write",
                    json!({
                        "heightmap_path": "assets/terrain/test_heightmap.png",
                        "chunk_count": [2, 2],
                        "chunk_size": 32.0,
                        "height_scale": 20.0,
                        "layer0_texture_path": "assets/terrain/grass.png",
                        "layer1_texture_path": "assets/terrain/rock.png",
                        "layer2_texture_path": "assets/terrain/dirt.png",
                        "layer3_texture_path": "assets/terrain/snow.png",
                        "splatmap_path": "assets/terrain/splatmap.png",
                    }),
                )
                .expect("terrain_write not registered");
            assert!(out.is_ok(), "terrain_write failed: {:?}", out.error);
        }
        app.update();

        let mut query = app.world_mut().query::<&bsengine_app::terrain::Terrain>();
        let terrain = query
            .iter(app.world())
            .next()
            .expect("expected one entity with a Terrain component");
        assert_eq!(
            terrain.splatmap_path.as_deref(),
            Some("assets/terrain/splatmap.png")
        );
    }

    #[test]
    fn mcp_spawn_point_light_creates_entity() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let result = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "spawn_point_light",
                    json!({"color":[1.0,0.5,0.0],"intensity":2.0,"range":8.0,"position":[0.0,3.0,0.0]}),
                )
                .expect("spawn_point_light not found");
            assert!(result.is_ok(), "{:?}", result.error);
        }
        app.update();

        let mut q = app.world_mut().query::<&bsengine_core::PointLight>();
        let lights: Vec<_> = q.iter(app.world()).collect();
        assert_eq!(lights.len(), 1);
        assert!((lights[0].intensity - 2.0).abs() < 1e-4);
        assert!((lights[0].range - 8.0).abs() < 1e-4);
    }

    #[test]
    fn mcp_spawn_directional_light_creates_entity() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let result = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "spawn_directional_light",
                    json!({"direction":[0.0,-1.0,0.0],"color":[1.0,1.0,1.0],"ambient":[0.1,0.1,0.1]}),
                )
                .expect("spawn_directional_light not found");
            assert!(result.is_ok(), "{:?}", result.error);
        }
        app.update();

        let mut q =
            app.world_mut()
                .query::<(&bsengine_core::DirectionalLight, &Transform)>();
        let (_, transform) = q.iter(app.world()).next().expect("no DirectionalLight spawned");
        // direction lives on Transform.rotation (rotation * -Z), same as SpotLight.
        let derived_dir = transform.rotation.0 * Vec3::NEG_Z;
        assert!(
            (derived_dir - Vec3::new(0.0, -1.0, 0.0)).length() < 1e-4,
            "expected direction (0,-1,0), derived {:?}",
            derived_dir
        );
    }

    #[test]
    fn mcp_update_directional_light_direction_rotates_transform() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        let eid = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let result = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "spawn_directional_light",
                    json!({"direction":[0.0,-1.0,0.0],"color":[1.0,1.0,1.0],"ambient":[0.1,0.1,0.1]}),
                )
                .expect("spawn_directional_light not found");
            assert!(result.is_ok(), "{:?}", result.error);
            app.update();
            let mut q = app
                .world_mut()
                .query::<(bevy_ecs::entity::Entity, &bsengine_core::DirectionalLight)>();
            q.iter(app.world()).next().unwrap().0.index() as u64
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let result = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "update_directional_light",
                    json!({"entity_id": eid, "direction": [1.0, 0.0, 0.0]}),
                )
                .expect("update_directional_light not found");
            assert!(result.is_ok(), "{:?}", result.error);
        }
        app.update();
        app.update();

        let mut q = app.world_mut().query::<&Transform>();
        let transform = q
            .iter(app.world())
            .next()
            .expect("entity should still have a Transform");
        let derived_dir = transform.rotation.0 * Vec3::NEG_Z;
        assert!(
            (derived_dir - Vec3::new(1.0, 0.0, 0.0)).length() < 1e-4,
            "expected direction (1,0,0) after update, derived {:?}",
            derived_dir
        );
    }

    #[test]
    fn mcp_remove_light_removes_point_light() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app
            .world_mut()
            .spawn(bsengine_core::PointLight::default())
            .id();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("remove_light", json!({"entity_id": eid.index() as u64}))
                .expect("remove_light not found");
        }
        app.update();

        let mut q = app.world_mut().query::<&bsengine_core::PointLight>();
        assert!(
            q.iter(app.world()).next().is_none(),
            "PointLight still present"
        );
    }

    #[test]
    fn mcp_get_entity_returns_entity_info() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "Queried"}))
                .unwrap();
        }
        app.update();
        app.update();

        let entity_id = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("Queried"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": entity_id}))
            .unwrap();
        assert!(out.is_ok(), "get_entity should succeed");
        assert_eq!(out.content["entity"]["id"], entity_id, "id matches");
        assert_eq!(out.content["entity"]["name"], "Queried", "name matches");
    }

    #[test]
    fn mcp_get_entity_missing_returns_error() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": 9999}))
            .unwrap();
        assert!(!out.is_ok(), "unknown entity should return error");
    }

    #[test]
    fn mcp_select_and_deselect_entity() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "Selected"}))
                .unwrap();
        }
        app.update();
        app.update();

        let entity_id = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("Selected"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        // default: not selected
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let list = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap();
            let entity = list.content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("Selected"))
                .unwrap();
            assert_eq!(entity["selected"], false);
        }

        // select
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": entity_id}))
                .unwrap();
        }
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("get_selection", json!({}))
                .unwrap();
            let ids = out.content["selected_ids"].as_array().unwrap();
            assert!(
                ids.contains(&serde_json::json!(entity_id)),
                "entity should be selected"
            );

            let list = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap();
            let entity = list.content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("Selected"))
                .unwrap();
            assert_eq!(
                entity["selected"], true,
                "list_entities.selected should be true"
            );
        }

        // deselect
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("deselect_entity", json!({"entity_id": entity_id}))
                .unwrap();
        }
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("get_selection", json!({}))
                .unwrap();
            let ids = out.content["selected_ids"].as_array().unwrap();
            assert!(
                !ids.contains(&serde_json::json!(entity_id)),
                "entity should be deselected"
            );
        }
    }

    #[test]
    fn mcp_has_component_detects_mesh_and_camera() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let mcp = mcp.0.lock().unwrap();
            mcp.execute(
                "batch_spawn",
                json!({"entities": [{"name": "Plain", "position": [0.0,0.0,0.0]}]}),
            )
            .unwrap();
            mcp.execute(
                "spawn_camera",
                json!({"fov_y_degrees": 60.0, "position": [0.0, 5.0, 10.0]}),
            )
            .unwrap();
        }
        app.update();
        app.update();

        let (plain_id, camera_id) = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let list = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap();
            let entities = list.content["entities"].as_array().unwrap();
            let p = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("Plain"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let c = entities
                .iter()
                .find(|e| !e["camera_fov"].is_null())
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (p, c)
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let mcp = mcp.0.lock().unwrap();
            mcp.execute("attach_mesh", json!({"entity_id": plain_id, "mesh_id": 1}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let mcp = mcp.0.lock().unwrap();

        let has_mesh = mcp
            .execute(
                "has_component",
                json!({"entity_id": plain_id, "component": "mesh"}),
            )
            .unwrap();
        assert_eq!(has_mesh.content["has_component"], true);

        let no_cam = mcp
            .execute(
                "has_component",
                json!({"entity_id": plain_id, "component": "camera"}),
            )
            .unwrap();
        assert_eq!(no_cam.content["has_component"], false);

        let has_cam = mcp
            .execute(
                "has_component",
                json!({"entity_id": camera_id, "component": "camera"}),
            )
            .unwrap();
        assert_eq!(has_cam.content["has_component"], true);
    }

    #[test]
    fn mcp_toggle_entity_selection_toggles_selection_state() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "Entity"},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let entity_id = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"][0]["id"]
                .as_u64()
                .unwrap()
        };

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();

        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("toggle_entity_selection", json!({"entity_id": entity_id}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(
            out.content["selected"], true,
            "toggled from unselected → selected"
        );

        let out2 = mcp
            .0
            .lock()
            .unwrap()
            .execute("toggle_entity_selection", json!({"entity_id": entity_id}))
            .unwrap();
        assert_eq!(
            out2.content["selected"], false,
            "toggled from selected → unselected"
        );

        let out3 = mcp
            .0
            .lock()
            .unwrap()
            .execute("toggle_entity_selection", json!({"entity_id": entity_id}))
            .unwrap();
        assert_eq!(out3.content["selected"], true, "toggled back to selected");
    }

    #[test]
    fn mcp_shrink_selection_to_roots_keeps_only_topmost_selected() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "Root"},
                        {"name": "Child"},
                        {"name": "Grandchild"},
                        {"name": "IndepRoot"},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (root_id, child_id, gc_id, indep_id) = (
            id_of("Root"),
            id_of("Child"),
            id_of("Grandchild"),
            id_of("IndepRoot"),
        );

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let reg = mcp.0.lock().unwrap();
            reg.execute(
                "set_parent",
                json!({"entity_id": child_id, "parent_id": root_id}),
            )
            .unwrap();
            reg.execute(
                "set_parent",
                json!({"entity_id": gc_id, "parent_id": child_id}),
            )
            .unwrap();
            // select all
            reg.execute("select_entity", json!({"entity_id": root_id}))
                .unwrap();
            reg.execute("select_entity", json!({"entity_id": child_id}))
                .unwrap();
            reg.execute("select_entity", json!({"entity_id": gc_id}))
                .unwrap();
            reg.execute("select_entity", json!({"entity_id": indep_id}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("shrink_selection_to_roots", json!({}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(
            out.content["removed_count"], 2,
            "Child and Grandchild removed"
        );

        let sel = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_selection", json!({}))
            .unwrap();
        let ids: Vec<u64> = sel.content["selected_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        assert!(ids.contains(&root_id), "Root kept");
        assert!(ids.contains(&indep_id), "IndepRoot kept");
        assert!(!ids.contains(&child_id), "Child removed");
        assert!(!ids.contains(&gc_id), "Grandchild removed");
    }

    #[test]
    fn mcp_expand_selection_to_subtrees_adds_all_descendants() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "Root"},
                        {"name": "Child"},
                        {"name": "Grandchild"},
                        {"name": "Unrelated"},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (root_id, child_id, gc_id, unrel_id) = (
            id_of("Root"),
            id_of("Child"),
            id_of("Grandchild"),
            id_of("Unrelated"),
        );

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let reg = mcp.0.lock().unwrap();
            reg.execute(
                "set_parent",
                json!({"entity_id": child_id, "parent_id": root_id}),
            )
            .unwrap();
            reg.execute(
                "set_parent",
                json!({"entity_id": gc_id, "parent_id": child_id}),
            )
            .unwrap();
            reg.execute("select_entity", json!({"entity_id": root_id}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("expand_selection_to_subtrees", json!({}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["added_count"], 2, "Child and Grandchild added");

        let sel = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_selection", json!({}))
            .unwrap();
        let ids: Vec<u64> = sel.content["selected_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        assert!(ids.contains(&root_id));
        assert!(ids.contains(&child_id), "Child added");
        assert!(ids.contains(&gc_id), "Grandchild added");
        assert!(!ids.contains(&unrel_id), "Unrelated not added");
    }

    #[test]
    fn mcp_untag_all_selected_removes_tag_from_all_selected_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "A", "position": [0.0, 0.0, 0.0]},
                        {"name": "B", "position": [1.0, 0.0, 0.0]},
                        {"name": "C", "position": [2.0, 0.0, 0.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (a_id, b_id, c_id) = (id_of("A"), id_of("B"), id_of("C"));

        // Tag all three with "hero"
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let reg = mcp.0.lock().unwrap();
            for id in [a_id, b_id, c_id] {
                reg.execute("tag_entity", json!({"entity_id": id, "tag": "hero"}))
                    .unwrap();
            }
        }
        app.update();
        app.update();

        // Select A and B, then untag them
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let reg = mcp.0.lock().unwrap();
            reg.execute("select_entity", json!({"entity_id": a_id}))
                .unwrap();
            reg.execute("select_entity", json!({"entity_id": b_id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("untag_all_selected", json!({"tag": "hero"}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["untagged_count"], 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let updated = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let a = updated
            .iter()
            .find(|e| e["id"].as_u64().unwrap() == a_id)
            .unwrap();
        let b = updated
            .iter()
            .find(|e| e["id"].as_u64().unwrap() == b_id)
            .unwrap();
        let c = updated
            .iter()
            .find(|e| e["id"].as_u64().unwrap() == c_id)
            .unwrap();
        assert!(
            !a["tags"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t.as_str() == Some("hero")),
            "A hero tag removed"
        );
        assert!(
            !b["tags"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t.as_str() == Some("hero")),
            "B hero tag removed"
        );
        assert!(
            c["tags"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t.as_str() == Some("hero")),
            "C hero tag kept"
        );
    }

    #[test]
    fn mcp_set_visibility_by_tag_hides_all_entities_with_given_tag() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "A", "position": [0.0, 0.0, 0.0]},
                        {"name": "B", "position": [1.0, 0.0, 0.0]},
                        {"name": "C", "position": [2.0, 0.0, 0.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (a_id, b_id, c_id) = (id_of("A"), id_of("B"), id_of("C"));

        // Tag A and B with "prop"
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let reg = mcp.0.lock().unwrap();
            reg.execute("tag_entity", json!({"entity_id": a_id, "tag": "prop"}))
                .unwrap();
            reg.execute("tag_entity", json!({"entity_id": b_id, "tag": "prop"}))
                .unwrap();
        }
        app.update();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "set_visibility_by_tag",
                    json!({"tag": "prop", "visible": false}),
                )
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["affected_count"], 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let updated = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let a = updated
            .iter()
            .find(|e| e["id"].as_u64().unwrap() == a_id)
            .unwrap();
        let b = updated
            .iter()
            .find(|e| e["id"].as_u64().unwrap() == b_id)
            .unwrap();
        let c = updated
            .iter()
            .find(|e| e["id"].as_u64().unwrap() == c_id)
            .unwrap();
        assert!(!a["visible"].as_bool().unwrap(), "A hidden");
        assert!(!b["visible"].as_bool().unwrap(), "B hidden");
        assert!(c["visible"].as_bool().unwrap(), "C still visible");
    }

    #[test]
    fn mcp_tag_all_selected_applies_tag_to_all_selected_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "A", "position": [0.0, 0.0, 0.0]},
                        {"name": "B", "position": [1.0, 0.0, 0.0]},
                        {"name": "C", "position": [2.0, 0.0, 0.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (a_id, b_id, c_id) = (id_of("A"), id_of("B"), id_of("C"));

        // Select A and B
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let reg = mcp.0.lock().unwrap();
            reg.execute("select_entity", json!({"entity_id": a_id}))
                .unwrap();
            reg.execute("select_entity", json!({"entity_id": b_id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("tag_all_selected", json!({"tag": "hero"}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["tagged_count"], 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let updated = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let a = updated
            .iter()
            .find(|e| e["id"].as_u64().unwrap() == a_id)
            .unwrap();
        let b = updated
            .iter()
            .find(|e| e["id"].as_u64().unwrap() == b_id)
            .unwrap();
        let c = updated
            .iter()
            .find(|e| e["id"].as_u64().unwrap() == c_id)
            .unwrap();
        assert!(
            a["tags"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t.as_str() == Some("hero")),
            "A tagged"
        );
        assert!(
            b["tags"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t.as_str() == Some("hero")),
            "B tagged"
        );
        assert!(
            !c["tags"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t.as_str() == Some("hero")),
            "C not tagged"
        );
    }

    #[test]
    fn mcp_offset_selection_rotation_adds_to_rotation_of_all_selected() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "A", "position": [0.0, 0.0, 0.0]},
                        {"name": "B", "position": [1.0, 0.0, 0.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (a_id, b_id) = (id_of("A"), id_of("B"));

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let reg = mcp.0.lock().unwrap();
            reg.execute("select_entity", json!({"entity_id": a_id}))
                .unwrap();
            reg.execute("select_entity", json!({"entity_id": b_id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "offset_selection_rotation",
                    json!({"rx": 10.0, "ry": 20.0, "rz": 30.0}),
                )
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["rotated_count"], 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let updated = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        for id in [a_id, b_id] {
            let e = updated
                .iter()
                .find(|e| e["id"].as_u64().unwrap() == id)
                .unwrap();
            let rot = e["rotation"].as_array().unwrap();
            assert!((rot[0].as_f64().unwrap() - 10.0).abs() < 0.5, "rx=10");
            assert!((rot[1].as_f64().unwrap() - 20.0).abs() < 0.5, "ry=20");
            assert!((rot[2].as_f64().unwrap() - 30.0).abs() < 0.5, "rz=30");
        }
    }

    #[test]
    fn mcp_offset_selection_scale_adds_to_scale_of_all_selected() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "A", "position": [0.0, 0.0, 0.0]},
                        {"name": "B", "position": [1.0, 0.0, 0.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (a_id, b_id) = (id_of("A"), id_of("B"));

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let reg = mcp.0.lock().unwrap();
            reg.execute("select_entity", json!({"entity_id": a_id}))
                .unwrap();
            reg.execute("select_entity", json!({"entity_id": b_id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "offset_selection_scale",
                    json!({"sx": 1.0, "sy": 2.0, "sz": 3.0}),
                )
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["scaled_count"], 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let updated = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        for id in [a_id, b_id] {
            let e = updated
                .iter()
                .find(|e| e["id"].as_u64().unwrap() == id)
                .unwrap();
            let scale = e["scale"].as_array().unwrap();
            // default scale is [1,1,1], so after offset: [2,3,4]
            assert!((scale[0].as_f64().unwrap() - 2.0).abs() < 0.01, "sx=2");
            assert!((scale[1].as_f64().unwrap() - 3.0).abs() < 0.01, "sy=3");
            assert!((scale[2].as_f64().unwrap() - 4.0).abs() < 0.01, "sz=4");
        }
    }

    #[test]
    fn mcp_offset_selection_position_moves_all_selected_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "A", "position": [0.0, 0.0, 0.0]},
                        {"name": "B", "position": [10.0, 0.0, 0.0]},
                        {"name": "C", "position": [20.0, 0.0, 0.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (a_id, b_id, c_id) = (id_of("A"), id_of("B"), id_of("C"));

        // Select A and B only
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let reg = mcp.0.lock().unwrap();
            reg.execute("select_entity", json!({"entity_id": a_id}))
                .unwrap();
            reg.execute("select_entity", json!({"entity_id": b_id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "offset_selection_position",
                    json!({"dx": 5.0, "dy": 3.0, "dz": 1.0}),
                )
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["moved_count"], 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let updated = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let a = updated
            .iter()
            .find(|e| e["id"].as_u64().unwrap() == a_id)
            .unwrap();
        let b = updated
            .iter()
            .find(|e| e["id"].as_u64().unwrap() == b_id)
            .unwrap();
        let c = updated
            .iter()
            .find(|e| e["id"].as_u64().unwrap() == c_id)
            .unwrap();
        assert!(
            (a["position"][0].as_f64().unwrap() - 5.0).abs() < 0.01,
            "A.x=5"
        );
        assert!(
            (b["position"][0].as_f64().unwrap() - 15.0).abs() < 0.01,
            "B.x=15"
        );
        assert!(
            (c["position"][0].as_f64().unwrap() - 20.0).abs() < 0.01,
            "C unchanged"
        );
    }

    #[test]
    fn mcp_scale_selection_uniformly_applies_scale_to_all_selected() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "A", "position": [0.0, 0.0, 0.0]},
                        {"name": "B", "position": [1.0, 0.0, 0.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (a_id, b_id) = (id_of("A"), id_of("B"));

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let reg = mcp.0.lock().unwrap();
            reg.execute("select_entity", json!({"entity_id": a_id}))
                .unwrap();
            reg.execute("select_entity", json!({"entity_id": b_id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("scale_selection_uniformly", json!({"scale": 3.0}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["scaled_count"], 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let updated = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        for id in [a_id, b_id] {
            let e = updated
                .iter()
                .find(|e| e["id"].as_u64().unwrap() == id)
                .unwrap();
            let scale = e["scale"].as_array().unwrap();
            assert!((scale[0].as_f64().unwrap() - 3.0).abs() < 0.01, "sx=3");
            assert!((scale[1].as_f64().unwrap() - 3.0).abs() < 0.01, "sy=3");
            assert!((scale[2].as_f64().unwrap() - 3.0).abs() < 0.01, "sz=3");
        }
    }

    #[test]
    fn mcp_offset_entity_rotation_adds_to_current_rotation() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "A", "position": [0.0, 0.0, 0.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let entity_id = all
            .iter()
            .find(|e| e["name"].as_str() == Some("A"))
            .unwrap()["id"]
            .as_u64()
            .unwrap();

        // Set initial rotation
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "set_rotation",
                    json!({"entity_id": entity_id, "rx": 10.0, "ry": 20.0, "rz": 30.0}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "offset_entity_rotation",
                    json!({"entity_id": entity_id, "rx": 5.0, "ry": 10.0, "rz": 15.0}),
                )
                .unwrap();
            assert!(out.is_ok());
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let updated = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let e = updated
            .iter()
            .find(|e| e["id"].as_u64().unwrap() == entity_id)
            .unwrap();
        let rot = e["rotation"].as_array().unwrap();
        assert!((rot[0].as_f64().unwrap() - 15.0).abs() < 0.5, "rx=15");
        assert!((rot[1].as_f64().unwrap() - 30.0).abs() < 0.5, "ry=30");
        assert!((rot[2].as_f64().unwrap() - 45.0).abs() < 0.5, "rz=45");
    }

    #[test]
    fn mcp_offset_entity_scale_adds_to_current_scale() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "A", "position": [0.0, 0.0, 0.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let entity_id = all
            .iter()
            .find(|e| e["name"].as_str() == Some("A"))
            .unwrap()["id"]
            .as_u64()
            .unwrap();

        // Set initial scale
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "set_scale",
                    json!({"entity_id": entity_id, "sx": 1.0, "sy": 2.0, "sz": 3.0}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "offset_entity_scale",
                    json!({"entity_id": entity_id, "sx": 0.5, "sy": 1.0, "sz": 2.0}),
                )
                .unwrap();
            assert!(out.is_ok());
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let updated = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let e = updated
            .iter()
            .find(|e| e["id"].as_u64().unwrap() == entity_id)
            .unwrap();
        let scale = e["scale"].as_array().unwrap();
        assert!((scale[0].as_f64().unwrap() - 1.5).abs() < 0.01, "sx=1.5");
        assert!((scale[1].as_f64().unwrap() - 3.0).abs() < 0.01, "sy=3.0");
        assert!((scale[2].as_f64().unwrap() - 5.0).abs() < 0.01, "sz=5.0");
    }

    #[test]
    fn mcp_scale_entity_uniformly_applies_same_value_to_all_axes() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "A", "position": [0.0, 0.0, 0.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let entity_id = all
            .iter()
            .find(|e| e["name"].as_str() == Some("A"))
            .unwrap()["id"]
            .as_u64()
            .unwrap();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "scale_entity_uniformly",
                    json!({"entity_id": entity_id, "scale": 4.0}),
                )
                .unwrap();
            assert!(out.is_ok());
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let updated = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let e = updated
            .iter()
            .find(|e| e["id"].as_u64().unwrap() == entity_id)
            .unwrap();
        let scale = e["scale"].as_array().unwrap();
        assert!((scale[0].as_f64().unwrap() - 4.0).abs() < 0.01, "sx=4");
        assert!((scale[1].as_f64().unwrap() - 4.0).abs() < 0.01, "sy=4");
        assert!((scale[2].as_f64().unwrap() - 4.0).abs() < 0.01, "sz=4");
    }

    #[test]
    fn mcp_reset_entity_scale_sets_scale_to_one() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "A", "position": [0.0, 0.0, 0.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let entity_id = all
            .iter()
            .find(|e| e["name"].as_str() == Some("A"))
            .unwrap()["id"]
            .as_u64()
            .unwrap();

        // Apply a non-unit scale
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "set_scale",
                    json!({"entity_id": entity_id, "sx": 3.0, "sy": 5.0, "sz": 7.0}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("reset_entity_scale", json!({"entity_id": entity_id}))
                .unwrap();
            assert!(out.is_ok());
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let updated = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let e = updated
            .iter()
            .find(|e| e["id"].as_u64().unwrap() == entity_id)
            .unwrap();
        let scale = e["scale"].as_array().unwrap();
        assert!((scale[0].as_f64().unwrap() - 1.0).abs() < 0.01, "sx=1");
        assert!((scale[1].as_f64().unwrap() - 1.0).abs() < 0.01, "sy=1");
        assert!((scale[2].as_f64().unwrap() - 1.0).abs() < 0.01, "sz=1");
    }

    #[test]
    fn mcp_reset_entity_position_sets_position_to_origin() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "A", "position": [5.0, 3.0, 2.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let entity_id = all
            .iter()
            .find(|e| e["name"].as_str() == Some("A"))
            .unwrap()["id"]
            .as_u64()
            .unwrap();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("reset_entity_position", json!({"entity_id": entity_id}))
                .unwrap();
            assert!(out.is_ok());
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let updated = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let e = updated
            .iter()
            .find(|e| e["id"].as_u64().unwrap() == entity_id)
            .unwrap();
        let pos = e["position"].as_array().unwrap();
        assert!((pos[0].as_f64().unwrap()).abs() < 0.01, "x=0");
        assert!((pos[1].as_f64().unwrap()).abs() < 0.01, "y=0");
        assert!((pos[2].as_f64().unwrap()).abs() < 0.01, "z=0");
    }

    #[test]
    fn mcp_reset_entity_rotation_sets_rotation_to_zero() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "A", "position": [0.0, 0.0, 0.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let entity_id = all
            .iter()
            .find(|e| e["name"].as_str() == Some("A"))
            .unwrap()["id"]
            .as_u64()
            .unwrap();

        // Set a non-zero rotation first
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "set_rotation",
                    json!({"entity_id": entity_id, "rx": 45.0, "ry": 30.0, "rz": 10.0}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("reset_entity_rotation", json!({"entity_id": entity_id}))
                .unwrap();
            assert!(out.is_ok());
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let updated = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let e = updated
            .iter()
            .find(|e| e["id"].as_u64().unwrap() == entity_id)
            .unwrap();
        let rot = e["rotation"].as_array().unwrap();
        assert!(rot[0].as_f64().unwrap().abs() < 0.1, "rx=0");
        assert!(rot[1].as_f64().unwrap().abs() < 0.1, "ry=0");
        assert!(rot[2].as_f64().unwrap().abs() < 0.1, "rz=0");
    }

    #[test]
    fn mcp_detach_selection_from_parent_removes_parent_from_selected() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "Parent", "position": [0.0, 0.0, 0.0]},
                        {"name": "ChildA", "position": [1.0, 0.0, 0.0]},
                        {"name": "ChildB", "position": [2.0, 0.0, 0.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (parent_id, a_id, b_id) = (id_of("Parent"), id_of("ChildA"), id_of("ChildB"));

        // Attach both children to parent
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute(
                "set_parent",
                json!({"entity_id": a_id, "parent_id": parent_id}),
            )
            .unwrap();
            m.execute(
                "set_parent",
                json!({"entity_id": b_id, "parent_id": parent_id}),
            )
            .unwrap();
        }
        app.update();
        app.update();

        // Select and detach
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("select_entity", json!({"entity_id": a_id}))
                .unwrap();
            m.execute("select_entity", json!({"entity_id": b_id}))
                .unwrap();
        }
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("detach_selection_from_parent", json!({}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["affected_count"].as_u64().unwrap(), 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let updated = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        for e in &updated {
            if e["id"].as_u64().unwrap() == a_id || e["id"].as_u64().unwrap() == b_id {
                assert!(e["parent_id"].is_null(), "parent should be removed");
            }
        }
    }

    #[test]
    fn mcp_reparent_selection_sets_new_parent_for_all_selected() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "NewParent", "position": [0.0, 0.0, 0.0]},
                        {"name": "A", "position": [1.0, 0.0, 0.0]},
                        {"name": "B", "position": [2.0, 0.0, 0.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (new_parent_id, a_id, b_id) = (id_of("NewParent"), id_of("A"), id_of("B"));

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("select_entity", json!({"entity_id": a_id}))
                .unwrap();
            m.execute("select_entity", json!({"entity_id": b_id}))
                .unwrap();
        }
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("reparent_selection", json!({"parent_id": new_parent_id}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["affected_count"].as_u64().unwrap(), 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let updated = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        for e in &updated {
            if e["id"].as_u64().unwrap() == a_id || e["id"].as_u64().unwrap() == b_id {
                assert_eq!(
                    e["parent_id"].as_u64().unwrap(),
                    new_parent_id,
                    "parent should be NewParent"
                );
            }
        }
    }

    #[test]
    fn mcp_set_light_intensity_updates_point_light_intensity() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "spawn_point_light",
                    json!({
                        "color": [1.0, 1.0, 1.0],
                        "intensity": 100.0,
                        "range": 5.0,
                        "position": [0.0, 0.0, 0.0]
                    }),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let light_id = all
            .iter()
            .find(|e| e["light_type"].as_str() == Some("point"))
            .unwrap()["id"]
            .as_u64()
            .unwrap();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "set_light_intensity",
                    json!({"entity_id": light_id, "intensity": 500.0}),
                )
                .unwrap();
            assert!(out.is_ok());
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let updated = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let light = updated
            .iter()
            .find(|e| e["id"].as_u64().unwrap() == light_id)
            .unwrap();
        assert!(
            (light["light_intensity"].as_f64().unwrap() - 500.0).abs() < 0.1,
            "intensity should be 500"
        );
    }

    #[test]
    fn mcp_set_all_lights_color_updates_all_light_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_point_light", json!({
                "color": [1.0, 1.0, 1.0], "intensity": 100.0, "range": 5.0, "position": [0.0, 0.0, 0.0]
            })).unwrap();
            m.execute("spawn_point_light", json!({
                "color": [0.5, 0.5, 0.5], "intensity": 200.0, "range": 8.0, "position": [1.0, 0.0, 0.0]
            })).unwrap();
            m.execute(
                "batch_spawn",
                json!({"entities": [{"name": "NoLight", "position": [2.0, 0.0, 0.0]}]}),
            )
            .unwrap();
        }
        app.update();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "set_all_lights_color",
                    json!({"r": 1.0, "g": 0.0, "b": 0.0}),
                )
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["affected_count"].as_u64().unwrap(), 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let updated = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        for e in &updated {
            if !e["light_type"].is_null() {
                let color = e["light_color"].as_array().unwrap();
                assert!(
                    (color[0].as_f64().unwrap() - 1.0).abs() < 0.01,
                    "r should be 1.0"
                );
                assert!((color[1].as_f64().unwrap()).abs() < 0.01, "g should be 0.0");
                assert!((color[2].as_f64().unwrap()).abs() < 0.01, "b should be 0.0");
            }
        }
    }

    #[test]
    fn mcp_hide_all_selected_hides_all_selected_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "A", "position": [0.0, 0.0, 0.0]},
                        {"name": "B", "position": [1.0, 0.0, 0.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (a_id, b_id) = (id_of("A"), id_of("B"));

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("select_entity", json!({"entity_id": a_id}))
                .unwrap();
            m.execute("select_entity", json!({"entity_id": b_id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("hide_all_selected", json!({}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["affected_count"].as_u64().unwrap(), 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let updated = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        for e in &updated {
            if e["id"].as_u64().unwrap() == a_id || e["id"].as_u64().unwrap() == b_id {
                assert!(!e["visible"].as_bool().unwrap(), "entity should be hidden");
            }
        }
    }

    #[test]
    fn mcp_show_all_selected_shows_all_hidden_selected_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "A", "position": [0.0, 0.0, 0.0]},
                        {"name": "B", "position": [1.0, 0.0, 0.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (a_id, b_id) = (id_of("A"), id_of("B"));

        // First hide them
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("hide_entity", json!({"entity_id": a_id}))
                .unwrap();
            m.execute("hide_entity", json!({"entity_id": b_id}))
                .unwrap();
        }
        app.update();
        app.update();

        // Select and show
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("select_entity", json!({"entity_id": a_id}))
                .unwrap();
            m.execute("select_entity", json!({"entity_id": b_id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("show_all_selected", json!({}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["affected_count"].as_u64().unwrap(), 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let updated = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        for e in &updated {
            if e["id"].as_u64().unwrap() == a_id || e["id"].as_u64().unwrap() == b_id {
                assert!(e["visible"].as_bool().unwrap(), "entity should be visible");
            }
        }
    }

    #[test]
    fn mcp_move_selection_to_origin_moves_all_selected_to_zero() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "A", "position": [5.0, 3.0, 1.0]},
                        {"name": "B", "position": [2.0, 7.0, 4.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (a_id, b_id) = (id_of("A"), id_of("B"));

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("select_entity", json!({"entity_id": a_id}))
                .unwrap();
            m.execute("select_entity", json!({"entity_id": b_id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("move_selection_to_origin", json!({}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["affected_count"].as_u64().unwrap(), 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let updated = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        for e in &updated {
            if e["id"].as_u64().unwrap() == a_id || e["id"].as_u64().unwrap() == b_id {
                let pos = e["position"].as_array().unwrap();
                assert!((pos[0].as_f64().unwrap()).abs() < 0.01, "x should be 0");
                assert!((pos[1].as_f64().unwrap()).abs() < 0.01, "y should be 0");
                assert!((pos[2].as_f64().unwrap()).abs() < 0.01, "z should be 0");
            }
        }
    }

    #[test]
    fn mcp_set_selection_uniform_scale_applies_same_scale_to_all() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "A", "position": [0.0, 0.0, 0.0]},
                        {"name": "B", "position": [1.0, 0.0, 0.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (a_id, b_id) = (id_of("A"), id_of("B"));

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("select_entity", json!({"entity_id": a_id}))
                .unwrap();
            m.execute("select_entity", json!({"entity_id": b_id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("set_selection_uniform_scale", json!({"scale": 3.0}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["affected_count"].as_u64().unwrap(), 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let updated = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        for e in &updated {
            if e["id"].as_u64().unwrap() == a_id || e["id"].as_u64().unwrap() == b_id {
                let scale = e["scale"].as_array().unwrap();
                for component in scale {
                    assert!(
                        (component.as_f64().unwrap() - 3.0).abs() < 0.01,
                        "scale component should be 3.0"
                    );
                }
            }
        }
    }

    #[test]
    fn mcp_are_all_selected_visible_returns_true_only_when_all_visible() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "VisA", "position": [0.0, 0.0, 0.0]},
                        {"name": "VisB", "position": [1.0, 0.0, 0.0]},
                        {"name": "Hid",  "position": [2.0, 0.0, 0.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (vis_a, vis_b, hid_id) = (id_of("VisA"), id_of("VisB"), id_of("Hid"));

        // Hide one entity
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("hide_entity", json!({"entity_id": hid_id}))
                .unwrap();
        }
        app.update();
        app.update();

        // Select only visible — all_visible should be true
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("select_entity", json!({"entity_id": vis_a}))
                .unwrap();
            m.execute("select_entity", json!({"entity_id": vis_b}))
                .unwrap();
        }
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("are_all_selected_visible", json!({}))
                .unwrap();
            assert!(out.is_ok());
            assert!(out.content["all_visible"].as_bool().unwrap());
        }

        // Add hidden entity to selection — all_visible should now be false
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": hid_id}))
                .unwrap();
        }
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("are_all_selected_visible", json!({}))
            .unwrap();
        assert!(out.is_ok());
        assert!(!out.content["all_visible"].as_bool().unwrap());
    }

    #[test]
    fn mcp_align_entities_to_selection_pivot_aligns_x_axis() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "Pivot", "position": [5.0, 1.0, 2.0]},
                        {"name": "Other", "position": [9.0, 3.0, 4.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (pivot_id, other_id) = (id_of("Pivot"), id_of("Other"));

        // Select only "Other"; pivot specified explicitly via pivot_entity_id
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": other_id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "align_entities_to_selection_pivot",
                    json!({"pivot_entity_id": pivot_id, "axis": "x"}),
                )
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["affected_count"].as_u64().unwrap(), 1);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let entities = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let other = entities
            .iter()
            .find(|e| e["id"].as_u64().unwrap() == other_id)
            .unwrap();
        let pos = other["position"].as_array().unwrap();
        // Other's X should be aligned to pivot's X = 5.0; Y and Z unchanged
        assert!(
            (pos[0].as_f64().unwrap() - 5.0).abs() < 0.01,
            "X aligned to pivot"
        );
        assert!((pos[1].as_f64().unwrap() - 3.0).abs() < 0.01, "Y unchanged");
    }

    #[test]
    fn mcp_distribute_entities_evenly_distributes_along_x() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "A", "position": [0.0, 0.0, 0.0]},
                        {"name": "B", "position": [5.0, 0.0, 0.0]},
                        {"name": "C", "position": [10.0, 0.0, 0.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (a_id, b_id, c_id) = (id_of("A"), id_of("B"), id_of("C"));

        // Manually set positions so middle is off-center
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute(
                "set_transform",
                json!({"id": a_id, "x": 0.0, "y": 0.0, "z": 0.0}),
            )
            .unwrap();
            m.execute(
                "set_transform",
                json!({"id": b_id, "x": 2.0, "y": 0.0, "z": 0.0}),
            )
            .unwrap();
            m.execute(
                "set_transform",
                json!({"id": c_id, "x": 6.0, "y": 0.0, "z": 0.0}),
            )
            .unwrap();
        }
        app.update();
        app.update();

        // Select all three
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("select_entity", json!({"entity_id": a_id}))
                .unwrap();
            m.execute("select_entity", json!({"entity_id": b_id}))
                .unwrap();
            m.execute("select_entity", json!({"entity_id": c_id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("distribute_entities_evenly", json!({"axis": "x"}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["affected_count"].as_u64().unwrap(), 3);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let entities = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        // min=0, max=6 → middle entity should be at x=3
        let b = entities
            .iter()
            .find(|e| e["id"].as_u64().unwrap() == b_id)
            .unwrap();
        let bx = b["position"][0].as_f64().unwrap();
        assert!(
            (bx - 3.0).abs() < 0.01,
            "middle entity distributed to x=3, got {bx}"
        );
    }

    #[test]
    fn mcp_copy_transform_then_paste_to_selection_applies_transform() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "Source", "position": [10.0, 20.0, 30.0]},
                        {"name": "TargA",  "position": [0.0,  0.0,  0.0]},
                        {"name": "TargB",  "position": [1.0,  0.0,  0.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (src_id, targ_a, targ_b) = (id_of("Source"), id_of("TargA"), id_of("TargB"));

        // Set rotation and scale on source
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute(
                "set_rotation",
                json!({"entity_id": src_id, "rx": 45.0, "ry": 0.0, "rz": 0.0}),
            )
            .unwrap();
            m.execute(
                "set_scale",
                json!({"entity_id": src_id, "sx": 3.0, "sy": 2.0, "sz": 1.0}),
            )
            .unwrap();
        }
        app.update();
        app.update();

        // Copy transform from source
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("copy_transform_from_entity", json!({"entity_id": src_id}))
                .unwrap();
            assert!(out.is_ok());
            assert!(out.content["position"].is_array());
        }

        // Select targets
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("select_entity", json!({"entity_id": targ_a}))
                .unwrap();
            m.execute("select_entity", json!({"entity_id": targ_b}))
                .unwrap();
        }

        // Paste transform to selection
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("paste_transform_to_selection", json!({}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["affected_count"].as_u64().unwrap(), 2);
        }
        app.update();
        app.update();

        // Verify targets have source transform
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let entities = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        for &tid in &[targ_a, targ_b] {
            let e = entities
                .iter()
                .find(|e| e["id"].as_u64().unwrap() == tid)
                .unwrap();
            let pos = e["position"].as_array().unwrap();
            assert!((pos[0].as_f64().unwrap() - 10.0).abs() < 0.01, "x copied");
            assert!((pos[1].as_f64().unwrap() - 20.0).abs() < 0.01, "y copied");
            assert!((pos[2].as_f64().unwrap() - 30.0).abs() < 0.01, "z copied");
            let sc = e["scale"].as_array().unwrap();
            assert!((sc[0].as_f64().unwrap() - 3.0).abs() < 0.01, "sx copied");
        }
    }

    #[test]
    fn mcp_rename_selection_replace_substitutes_substring_in_name() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "OldHero",   "position": [0.0, 0.0, 0.0]},
                        {"name": "OldVillan", "position": [1.0, 0.0, 0.0]},
                        {"name": "Unrelated", "position": [2.0, 0.0, 0.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (old_hero, old_villan) = (id_of("OldHero"), id_of("OldVillan"));

        // Select OldHero and OldVillan (not Unrelated)
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("select_entity", json!({"entity_id": old_hero}))
                .unwrap();
            m.execute("select_entity", json!({"entity_id": old_villan}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "rename_selection_replace",
                    json!({"from": "Old", "to": "New"}),
                )
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["renamed_count"].as_u64().unwrap(), 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let name_of = |id: u64| {
            ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["name"]
                .as_str()
                .unwrap()
                .to_string()
        };
        assert_eq!(name_of(old_hero), "NewHero", "OldHero → NewHero");
        assert_eq!(name_of(old_villan), "NewVillan", "OldVillan → NewVillan");

        let unrelated_id = id_of("Unrelated");
        assert_eq!(name_of(unrelated_id), "Unrelated", "Unrelated unchanged");
    }

    #[test]
    fn mcp_distribute_selection_along_y_spaces_entities_evenly() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "Low",  "position": [0.0, 1.0, 0.0]},
                        {"name": "Mid",  "position": [0.0, 5.0, 0.0]},
                        {"name": "High", "position": [0.0, 9.0, 0.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let ids_all: Vec<u64> = all.iter().map(|e| e["id"].as_u64().unwrap()).collect();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            for &id in &ids_all {
                m.execute("select_entity", json!({"entity_id": id}))
                    .unwrap();
            }
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("distribute_selection_along_y", json!({"spacing": 3.0}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["distributed_count"].as_u64().unwrap(), 3);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let mut ys: Vec<f32> = ents
            .iter()
            .map(|e| e["position"].as_array().unwrap()[1].as_f64().unwrap() as f32)
            .collect();
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!(
            (ys[1] - ys[0] - 3.0).abs() < 0.01,
            "Y spacing first gap = 3"
        );
        assert!(
            (ys[2] - ys[1] - 3.0).abs() < 0.01,
            "Y spacing second gap = 3"
        );
    }

    #[test]
    fn mcp_distribute_selection_along_x_spaces_entities_evenly() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        // Spawn 3 entities at random X positions; after distribution with spacing=5, they should be at 0, 5, 10
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "First",  "position": [7.0, 0.0, 0.0]},
                        {"name": "Second", "position": [1.0, 0.0, 0.0]},
                        {"name": "Third",  "position": [4.0, 0.0, 0.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let ids_all: Vec<u64> = all.iter().map(|e| e["id"].as_u64().unwrap()).collect();

        // Select all
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            for &id in &ids_all {
                m.execute("select_entity", json!({"entity_id": id}))
                    .unwrap();
            }
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("distribute_selection_along_x", json!({"spacing": 5.0}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["distributed_count"].as_u64().unwrap(), 3);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let mut xs: Vec<f32> = ents
            .iter()
            .map(|e| e["position"].as_array().unwrap()[0].as_f64().unwrap() as f32)
            .collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        // Should be [0, 5, 10] (sorted by original X: Second=1, Third=4, First=7 → spaced at 0, 5, 10)
        assert!(
            (xs[1] - xs[0] - 5.0).abs() < 0.01,
            "spacing between first and second = 5"
        );
        assert!(
            (xs[2] - xs[1] - 5.0).abs() < 0.01,
            "spacing between second and third = 5"
        );
    }

    #[test]
    fn mcp_distribute_selection_along_z_spaces_entities_evenly() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "P1", "position": [0.0, 0.0, 9.0]},
                        {"name": "P2", "position": [0.0, 0.0, 3.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let ids_all: Vec<u64> = all.iter().map(|e| e["id"].as_u64().unwrap()).collect();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            for &id in &ids_all {
                m.execute("select_entity", json!({"entity_id": id}))
                    .unwrap();
            }
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("distribute_selection_along_z", json!({"spacing": 10.0}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["distributed_count"].as_u64().unwrap(), 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let mut zs: Vec<f32> = ents
            .iter()
            .map(|e| e["position"].as_array().unwrap()[2].as_f64().unwrap() as f32)
            .collect();
        zs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!((zs[1] - zs[0] - 10.0).abs() < 0.01, "Z spacing = 10");
    }

    #[test]
    fn mcp_align_selection_to_y_sets_same_y_for_all_selected() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "A", "position": [1.0, 3.0, 5.0]},
                        {"name": "B", "position": [2.0, 7.0, 8.0]},
                        {"name": "C", "position": [4.0, 1.0, 2.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (a_id, b_id, c_id) = (id_of("A"), id_of("B"), id_of("C"));

        // Select A and B (not C)
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("select_entity", json!({"entity_id": a_id}))
                .unwrap();
            m.execute("select_entity", json!({"entity_id": b_id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("align_selection_to_y", json!({"y": 5.0}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["aligned_count"].as_u64().unwrap(), 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let pos_of = |id: u64| {
            let arr = ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["position"]
                .as_array()
                .unwrap()
                .clone();
            [
                arr[0].as_f64().unwrap() as f32,
                arr[1].as_f64().unwrap() as f32,
                arr[2].as_f64().unwrap() as f32,
            ]
        };
        let a_pos = pos_of(a_id);
        let b_pos = pos_of(b_id);
        let c_pos = pos_of(c_id);
        assert!((a_pos[1] - 5.0).abs() < 0.01, "A.y aligned to 5");
        assert!((a_pos[0] - 1.0).abs() < 0.01, "A.x unchanged");
        assert!((b_pos[1] - 5.0).abs() < 0.01, "B.y aligned to 5");
        assert!(
            (c_pos[1] - 1.0).abs() < 0.01,
            "C.y unchanged (not selected)"
        );
    }

    #[test]
    fn mcp_align_selection_to_z_sets_same_z_for_all_selected() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "X", "position": [0.0, 0.0, 1.0]},
                        {"name": "Y", "position": [0.0, 0.0, 9.0]},
                        {"name": "Z", "position": [0.0, 0.0, 4.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (x_id, y_id, z_id) = (id_of("X"), id_of("Y"), id_of("Z"));

        // Select X and Y (not Z)
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("select_entity", json!({"entity_id": x_id}))
                .unwrap();
            m.execute("select_entity", json!({"entity_id": y_id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("align_selection_to_z", json!({"z": 0.0}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["aligned_count"].as_u64().unwrap(), 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let pos_of = |id: u64| {
            let arr = ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["position"]
                .as_array()
                .unwrap()
                .clone();
            [
                arr[0].as_f64().unwrap() as f32,
                arr[1].as_f64().unwrap() as f32,
                arr[2].as_f64().unwrap() as f32,
            ]
        };
        assert!((pos_of(x_id)[2]).abs() < 0.01, "X.z aligned to 0");
        assert!((pos_of(y_id)[2]).abs() < 0.01, "Y.z aligned to 0");
        assert!(
            (pos_of(z_id)[2] - 4.0).abs() < 0.01,
            "Z.z unchanged (not selected)"
        );
    }

    #[test]
    fn mcp_align_selection_to_x_sets_same_x_for_all_selected() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "P", "position": [1.0, 2.0, 3.0]},
                        {"name": "Q", "position": [5.0, 4.0, 6.0]},
                        {"name": "R", "position": [9.0, 0.0, 1.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (p_id, q_id, r_id) = (id_of("P"), id_of("Q"), id_of("R"));

        // Select P and Q (not R)
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("select_entity", json!({"entity_id": p_id}))
                .unwrap();
            m.execute("select_entity", json!({"entity_id": q_id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("align_selection_to_x", json!({"x": 0.0}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["aligned_count"].as_u64().unwrap(), 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let pos_of = |id: u64| {
            let arr = ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["position"]
                .as_array()
                .unwrap()
                .clone();
            [
                arr[0].as_f64().unwrap() as f32,
                arr[1].as_f64().unwrap() as f32,
                arr[2].as_f64().unwrap() as f32,
            ]
        };
        let p_pos = pos_of(p_id);
        let q_pos = pos_of(q_id);
        let r_pos = pos_of(r_id);
        assert!((p_pos[0]).abs() < 0.01, "P.x aligned to 0");
        assert!((p_pos[1] - 2.0).abs() < 0.01, "P.y unchanged");
        assert!((q_pos[0]).abs() < 0.01, "Q.x aligned to 0");
        assert!(
            (r_pos[0] - 9.0).abs() < 0.01,
            "R.x unchanged (not selected)"
        );
    }

    #[test]
    fn mcp_number_selection_renames_with_sequential_index() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "A"})).unwrap();
            m.execute("spawn_entity", json!({"name": "B"})).unwrap();
            m.execute("spawn_entity", json!({"name": "C"})).unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (a_id, b_id) = (id_of("A"), id_of("B"));

        // Select A and B
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("select_entity", json!({"entity_id": a_id}))
                .unwrap();
            m.execute("select_entity", json!({"entity_id": b_id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("number_selection", json!({"prefix": "Item"}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["renamed_count"].as_u64().unwrap(), 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let names: Vec<String> = ents
            .iter()
            .filter(|e| {
                let id = e["id"].as_u64().unwrap();
                id == a_id || id == b_id
            })
            .filter_map(|e| e["name"].as_str().map(|s| s.to_string()))
            .collect();
        assert!(
            names.contains(&"Item_1".to_string()) || names.contains(&"Item_2".to_string()),
            "At least one entity renamed to Item_N pattern"
        );
        assert_eq!(names.len(), 2, "Both selected entities were renamed");
    }

    #[test]
    fn mcp_reset_selection_scale_sets_scale_to_one() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "Big", "position": [0.0, 0.0, 0.0]},
                        {"name": "Small", "position": [5.0, 0.0, 0.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (big_id, small_id) = (id_of("Big"), id_of("Small"));

        // Scale Big to (3,3,3) and Small to (0.5,0.5,0.5)
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute(
                "set_scale",
                json!({"entity_id": big_id, "sx": 3.0, "sy": 3.0, "sz": 3.0}),
            )
            .unwrap();
            m.execute(
                "set_scale",
                json!({"entity_id": small_id, "sx": 0.5, "sy": 0.5, "sz": 0.5}),
            )
            .unwrap();
        }
        app.update();
        app.update();

        // Select only Big
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": big_id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("reset_selection_scale", json!({}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["reset_count"].as_u64().unwrap(), 1);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let scale_of = |id: u64| {
            let arr = ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["scale"]
                .as_array()
                .unwrap()
                .clone();
            [
                arr[0].as_f64().unwrap() as f32,
                arr[1].as_f64().unwrap() as f32,
                arr[2].as_f64().unwrap() as f32,
            ]
        };
        let big_scale = scale_of(big_id);
        assert!(
            (big_scale[0] - 1.0).abs() < 0.01 && (big_scale[1] - 1.0).abs() < 0.01,
            "Big scale reset to 1"
        );
        let small_scale = scale_of(small_id);
        assert!((small_scale[0] - 0.5).abs() < 0.01, "Small scale unchanged");
    }

    #[test]
    fn mcp_reset_selection_position_sets_position_to_origin() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "A", "position": [3.0, 5.0, 7.0]},
                        {"name": "B", "position": [1.0, 2.0, 3.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (a_id, b_id) = (id_of("A"), id_of("B"));

        // Select A only
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": a_id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("reset_selection_position", json!({}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["reset_count"].as_u64().unwrap(), 1);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let pos_of = |id: u64| {
            let arr = ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["position"]
                .as_array()
                .unwrap()
                .clone();
            [
                arr[0].as_f64().unwrap() as f32,
                arr[1].as_f64().unwrap() as f32,
                arr[2].as_f64().unwrap() as f32,
            ]
        };
        let a_pos = pos_of(a_id);
        assert!(
            (a_pos[0]).abs() < 0.01 && (a_pos[1]).abs() < 0.01 && (a_pos[2]).abs() < 0.01,
            "A position reset to origin"
        );
        let b_pos = pos_of(b_id);
        assert!((b_pos[0] - 1.0).abs() < 0.01, "B position unchanged");
    }

    #[test]
    fn mcp_reset_selection_rotation_sets_rotation_to_zero() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "R", "position": [0.0, 0.0, 0.0]},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let r_id = all
            .iter()
            .find(|e| e["name"].as_str() == Some("R"))
            .unwrap()["id"]
            .as_u64()
            .unwrap();

        // Apply a rotation first
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "set_rotation",
                    json!({"entity_id": r_id, "rx": 45.0, "ry": 30.0, "rz": 15.0}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        // Select R
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": r_id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("reset_selection_rotation", json!({}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["reset_count"].as_u64().unwrap(), 1);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let rot = ents
            .iter()
            .find(|e| e["id"].as_u64() == Some(r_id))
            .unwrap()["rotation"]
            .as_array()
            .unwrap();
        assert!((rot[0].as_f64().unwrap()).abs() < 0.1, "rx reset to 0");
        assert!((rot[1].as_f64().unwrap()).abs() < 0.1, "ry reset to 0");
        assert!((rot[2].as_f64().unwrap()).abs() < 0.1, "rz reset to 0");
    }

    #[test]
    fn mcp_toggle_visibility_on_selection_flips_visible_and_hidden() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "Vis"})).unwrap();
            m.execute("spawn_entity", json!({"name": "Hidden"}))
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (vis_id, hid_id) = (id_of("Vis"), id_of("Hidden"));

        // Hide "Hidden"
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("hide_entity", json!({"entity_id": hid_id}))
                .unwrap();
        }
        app.update();
        app.update();

        // Select both
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("select_entity", json!({"entity_id": vis_id}))
                .unwrap();
            m.execute("select_entity", json!({"entity_id": hid_id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("toggle_visibility_on_selection", json!({}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["toggled_count"].as_u64().unwrap(), 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let vis_of = |id: u64| {
            ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["visible"]
                .as_bool()
                .unwrap()
        };
        assert!(!vis_of(vis_id), "Vis was visible, now hidden");
        assert!(vis_of(hid_id), "Hidden was hidden, now visible");
    }

    #[test]
    fn mcp_expand_selection_to_children_adds_direct_children() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "Parent"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "Child1"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "Child2"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "Orphan"}))
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (par_id, c1_id, c2_id) = (id_of("Parent"), id_of("Child1"), id_of("Child2"));

        // Parent Child1 and Child2
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute(
                "set_parent",
                json!({"entity_id": c1_id, "parent_id": par_id}),
            )
            .unwrap();
            m.execute(
                "set_parent",
                json!({"entity_id": c2_id, "parent_id": par_id}),
            )
            .unwrap();
        }
        app.update();
        app.update();

        // Select only Parent
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": par_id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("expand_selection_to_children", json!({}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["added_count"].as_u64().unwrap(), 2);
        }

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let sel: Vec<u64> = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_selection", json!({}))
            .unwrap()
            .content["selected_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        assert!(sel.contains(&par_id));
        assert!(sel.contains(&c1_id));
        assert!(sel.contains(&c2_id));
        assert!(!sel.contains(&id_of("Orphan")));
    }

    #[test]
    fn mcp_replace_tag_on_selection_swaps_tag_on_selected_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "A"})).unwrap();
            m.execute("spawn_entity", json!({"name": "B"})).unwrap();
            m.execute("spawn_entity", json!({"name": "C"})).unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (a_id, b_id, c_id) = (id_of("A"), id_of("B"), id_of("C"));

        // Tag A and B with "old_tag", C with "other"
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("tag_entity", json!({"entity_id": a_id, "tag": "old_tag"}))
                .unwrap();
            m.execute("tag_entity", json!({"entity_id": b_id, "tag": "old_tag"}))
                .unwrap();
            m.execute("tag_entity", json!({"entity_id": c_id, "tag": "other"}))
                .unwrap();
        }
        app.update();
        app.update();

        // Select A and C (not B)
        for id in [a_id, c_id] {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "replace_tag_on_selection",
                    json!({"old_tag": "old_tag", "new_tag": "new_tag"}),
                )
                .unwrap();
            assert!(out.is_ok());
            // Only A is selected AND has "old_tag"
            assert_eq!(out.content["replaced_count"].as_u64().unwrap(), 1);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let tags_of = |id: u64| {
            ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["tags"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect::<Vec<_>>()
        };
        assert!(
            tags_of(a_id).contains(&"new_tag".to_string()),
            "A should have new_tag"
        );
        assert!(
            !tags_of(a_id).contains(&"old_tag".to_string()),
            "A should not have old_tag"
        );
        assert!(
            tags_of(b_id).contains(&"old_tag".to_string()),
            "B (unselected) unchanged"
        );
        assert!(
            tags_of(c_id).contains(&"other".to_string()),
            "C other tag unchanged"
        );
    }

    #[test]
    fn mcp_clear_tags_on_selection_removes_all_tags_from_selected() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "X"})).unwrap();
            m.execute("spawn_entity", json!({"name": "Y"})).unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (x_id, y_id) = (id_of("X"), id_of("Y"));

        // Tag X with two tags
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("tag_entity", json!({"entity_id": x_id, "tag": "alpha"}))
                .unwrap();
            m.execute("tag_entity", json!({"entity_id": x_id, "tag": "beta"}))
                .unwrap();
            m.execute("tag_entity", json!({"entity_id": y_id, "tag": "gamma"}))
                .unwrap();
        }
        app.update();
        app.update();

        // Select only X
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": x_id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("clear_tags_on_selection", json!({}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["cleared_count"].as_u64().unwrap(), 1);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let tags_of = |id: u64| {
            ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["tags"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect::<Vec<_>>()
        };
        assert!(tags_of(x_id).is_empty(), "X tags cleared");
        assert!(tags_of(y_id).contains(&"gamma".to_string()), "Y unchanged");
    }

    #[test]
    fn mcp_add_prefix_to_selection_prepends_prefix_to_names() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "Goblin"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "Orc"})).unwrap();
            m.execute("spawn_entity", json!({"name": "Unselected"}))
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (gob_id, orc_id) = (id_of("Goblin"), id_of("Orc"));

        for id in [gob_id, orc_id] {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": id}))
                .unwrap();
        }
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("add_prefix_to_selection", json!({"prefix": "Enemy_"}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["renamed_count"].as_u64().unwrap(), 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let name_of = |id: u64| {
            ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["name"]
                .as_str()
                .unwrap()
                .to_string()
        };
        assert_eq!(name_of(gob_id), "Enemy_Goblin");
        assert_eq!(name_of(orc_id), "Enemy_Orc");
    }

    #[test]
    fn mcp_add_suffix_to_selection_appends_suffix_to_names() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "Sword"})).unwrap();
            m.execute("spawn_entity", json!({"name": "Shield"}))
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (sword_id, shield_id) = (id_of("Sword"), id_of("Shield"));

        for id in [sword_id, shield_id] {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": id}))
                .unwrap();
        }
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("add_suffix_to_selection", json!({"suffix": "_Prop"}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["renamed_count"].as_u64().unwrap(), 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let name_of = |id: u64| {
            ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["name"]
                .as_str()
                .unwrap()
                .to_string()
        };
        assert_eq!(name_of(sword_id), "Sword_Prop");
        assert_eq!(name_of(shield_id), "Shield_Prop");
    }

    #[test]
    fn mcp_clone_selection_duplicates_all_selected_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "A"})).unwrap();
            m.execute("spawn_entity", json!({"name": "B"})).unwrap();
        }
        app.update();
        app.update();

        let before_count = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .len()
        };

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        for id in [id_of("A"), id_of("B")] {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("clone_selection", json!({}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["duplicated_count"].as_u64().unwrap(), 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let after_count = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .len();
        assert_eq!(
            after_count,
            before_count + 2,
            "2 new entities after cloning 2 selected"
        );
    }

    #[test]
    fn mcp_center_selection_moves_entities_to_origin() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        // Spawn two entities at (2,0,0) and (-2,0,0) — centroid is (0,0,0)
        // After centering, each moves by -(centroid) = (0,0,0), so positions remain
        // Let's use (4,0,0) and (2,0,0) — centroid is (3,0,0)
        // After centering: (4-3,0,0)=(1,0,0) and (2-3,0,0)=(-1,0,0)
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({
                        "entities": [
                            {"name": "A", "position": [4.0, 0.0, 0.0]},
                            {"name": "B", "position": [2.0, 0.0, 0.0]}
                        ]
                    }),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (a_id, b_id) = (id_of("A"), id_of("B"));

        for id in [a_id, b_id] {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": id}))
                .unwrap();
        }
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("center_selection", json!({}))
                .unwrap();
            assert!(out.is_ok());
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let pos_of = |id: u64| {
            ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["position"].clone()
        };
        let ax = pos_of(a_id)[0].as_f64().unwrap();
        let bx = pos_of(b_id)[0].as_f64().unwrap();
        assert!(
            (ax - 1.0).abs() < 0.01,
            "A x should be 1.0 after center, got {}",
            ax
        );
        assert!(
            (bx - (-1.0)).abs() < 0.01,
            "B x should be -1.0 after center, got {}",
            bx
        );
    }

    #[test]
    fn mcp_align_selection_to_ground_sets_y_to_zero() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({
                        "entities": [
                            {"name": "A", "position": [1.0, 5.0, 2.0]},
                            {"name": "B", "position": [3.0, -3.0, 0.0]}
                        ]
                    }),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (a_id, b_id) = (id_of("A"), id_of("B"));

        for id in [a_id, b_id] {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": id}))
                .unwrap();
        }
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("align_selection_to_ground", json!({}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["aligned_count"].as_u64().unwrap(), 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        for id in [a_id, b_id] {
            let pos = &ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["position"];
            assert!(
                (pos[1].as_f64().unwrap()).abs() < 0.01,
                "Y should be 0 after aligning to ground"
            );
        }
    }

    #[test]
    fn mcp_set_selection_rotation_applies_rotation_to_all_selected() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({
                        "entities": [
                            {"name": "A", "position": [0.0, 0.0, 0.0]},
                            {"name": "B", "position": [1.0, 0.0, 0.0]}
                        ]
                    }),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (a_id, b_id) = (id_of("A"), id_of("B"));

        for id in [a_id, b_id] {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "set_selection_rotation",
                    json!({"rx": 0.0, "ry": 90.0, "rz": 0.0}),
                )
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["rotated_count"].as_u64().unwrap(), 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        for id in [a_id, b_id] {
            let rot = &ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["rotation"];
            assert!(
                (rot[1].as_f64().unwrap() - 90.0).abs() < 0.1,
                "ry should be 90°"
            );
        }
    }

    #[test]
    fn mcp_set_selection_scale_applies_scale_to_all_selected() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({
                        "entities": [
                            {"name": "A", "position": [0.0, 0.0, 0.0]},
                            {"name": "B", "position": [1.0, 0.0, 0.0]}
                        ]
                    }),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (a_id, b_id) = (id_of("A"), id_of("B"));

        for id in [a_id, b_id] {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "set_selection_scale",
                    json!({"sx": 3.0, "sy": 3.0, "sz": 3.0}),
                )
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["scaled_count"].as_u64().unwrap(), 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        for id in [a_id, b_id] {
            let sc = &ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["scale"];
            assert!(
                (sc[0].as_f64().unwrap() - 3.0).abs() < 0.01,
                "sx should be 3.0"
            );
        }
    }

    #[test]
    fn mcp_snap_selection_to_grid_rounds_position_to_grid_size() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({
                        "entities": [{"name": "E", "position": [1.3, 2.7, -0.4]}]
                    }),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let e_id = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("E"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": e_id}))
                .unwrap();
        }
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("snap_selection_to_grid", json!({"grid_size": 1.0}))
                .unwrap();
            assert!(out.is_ok());
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let pos = ents
            .iter()
            .find(|e| e["id"].as_u64() == Some(e_id))
            .unwrap()["position"]
            .clone();
        let x = pos[0].as_f64().unwrap();
        let y = pos[1].as_f64().unwrap();
        let z = pos[2].as_f64().unwrap();
        assert!((x - 1.0).abs() < 0.01, "x snapped to 1.0, got {}", x);
        assert!((y - 3.0).abs() < 0.01, "y snapped to 3.0, got {}", y);
        assert!((z - 0.0).abs() < 0.01, "z snapped to 0.0, got {}", z);
    }

    #[test]
    fn mcp_expand_selection_to_siblings_adds_all_same_parent_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        // Root → [A, B, C]; Unrelated (no parent)
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "Root"})).unwrap();
            m.execute("spawn_entity", json!({"name": "A"})).unwrap();
            m.execute("spawn_entity", json!({"name": "B"})).unwrap();
            m.execute("spawn_entity", json!({"name": "C"})).unwrap();
            m.execute("spawn_entity", json!({"name": "Unrelated"}))
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (root_id, a_id, b_id, c_id) = (id_of("Root"), id_of("A"), id_of("B"), id_of("C"));

        // Parent A, B, C under Root
        for child in [a_id, b_id, c_id] {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "set_parent",
                    json!({"entity_id": child, "parent_id": root_id}),
                )
                .unwrap();
            app.update();
            app.update();
        }

        // Select A only; expand_selection_to_siblings should add B and C
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": a_id}))
                .unwrap();
        }
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("expand_selection_to_siblings", json!({}))
                .unwrap();
            assert!(out.is_ok());
        }

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let sel_out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_selection", json!({}))
            .unwrap();
        let sel_ids: Vec<u64> = sel_out.content["selected_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        assert!(sel_ids.contains(&a_id), "A in selection");
        assert!(sel_ids.contains(&b_id), "B added as sibling");
        assert!(sel_ids.contains(&c_id), "C added as sibling");
    }

    #[test]
    fn mcp_set_selection_parent_parents_all_selected_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "Root"})).unwrap();
            m.execute("spawn_entity", json!({"name": "A"})).unwrap();
            m.execute("spawn_entity", json!({"name": "B"})).unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (root_id, a_id, b_id) = (id_of("Root"), id_of("A"), id_of("B"));

        // Select A and B
        for id in [a_id, b_id] {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": id}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("set_selection_parent", json!({"parent_id": root_id}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["parented_count"].as_u64().unwrap(), 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let parent_of = |id: u64| {
            ents.iter()
                .find(|e| e["id"].as_u64() == Some(id))
                .and_then(|e| e["parent_id"].as_u64())
        };
        assert_eq!(parent_of(a_id), Some(root_id), "A parent = Root");
        assert_eq!(parent_of(b_id), Some(root_id), "B parent = Root");
        assert!(parent_of(root_id).is_none(), "Root has no parent");
    }

    #[test]
    fn mcp_unparent_selection_removes_parent_from_selected_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "Root"})).unwrap();
            m.execute("spawn_entity", json!({"name": "A"})).unwrap();
            m.execute("spawn_entity", json!({"name": "B"})).unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (root_id, a_id, b_id) = (id_of("Root"), id_of("A"), id_of("B"));

        // Parent A and B to Root first
        for child in [a_id, b_id] {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "set_parent",
                    json!({"entity_id": child, "parent_id": root_id}),
                )
                .unwrap();
            app.update();
            app.update();
        }

        // Select A only, then unparent
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": a_id}))
                .unwrap();
        }
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("unparent_selection", json!({}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["unparented_count"].as_u64().unwrap(), 1);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let parent_of = |id: u64| {
            ents.iter()
                .find(|e| e["id"].as_u64() == Some(id))
                .and_then(|e| e["parent_id"].as_u64())
        };
        assert!(parent_of(a_id).is_none(), "A unparented");
        assert_eq!(parent_of(b_id), Some(root_id), "B still has Root parent");
    }

    #[test]
    fn mcp_rotate_selection_by_rotates_all_selected_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({
                        "entities": [
                            {"name": "A", "position": [0.0, 0.0, 0.0]},
                            {"name": "B", "position": [1.0, 0.0, 0.0]}
                        ]
                    }),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let find_id = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (id_a, id_b) = (find_id("A"), find_id("B"));

        // Select A only
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": id_a}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "rotate_selection_by",
                    json!({"drx": 0.0, "dry": 45.0, "drz": 0.0}),
                )
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["rotated_count"].as_u64().unwrap(), 1);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let rot_y_of = |id: u64| {
            ents.iter()
                .find(|e| e["id"].as_u64() == Some(id))
                .and_then(|e| e["rotation"].as_array())
                .map(|r| r[1].as_f64().unwrap())
                .unwrap_or(f64::NAN)
        };
        assert!(
            (rot_y_of(id_a) - 45.0).abs() < 0.1,
            "A rotated 45° on Y, got {}",
            rot_y_of(id_a)
        );
        assert!(
            (rot_y_of(id_b) - 0.0).abs() < 0.1,
            "B rotation unchanged, got {}",
            rot_y_of(id_b)
        );
    }

    #[test]
    fn mcp_move_selection_by_moves_all_selected_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({
                        "entities": [
                            {"name": "A", "position": [0.0, 0.0, 0.0]},
                            {"name": "B", "position": [10.0, 0.0, 0.0]},
                            {"name": "C", "position": [20.0, 0.0, 0.0]}
                        ]
                    }),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let find_id = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (id_a, id_b, id_c) = (find_id("A"), find_id("B"), find_id("C"));

        // Select A and B
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": id_a}))
                .unwrap();
        }
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": id_b}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "move_selection_by",
                    json!({"dx": 5.0, "dy": 0.0, "dz": 0.0}),
                )
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["moved_count"].as_u64().unwrap(), 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let pos_of = |id: u64| {
            ents.iter()
                .find(|e| e["id"].as_u64() == Some(id))
                .and_then(|e| e["position"].as_array())
                .map(|p| p[0].as_f64().unwrap())
                .unwrap_or(f64::NAN)
        };
        assert!(
            (pos_of(id_a) - 5.0).abs() < 0.01,
            "A moved to 5, got {}",
            pos_of(id_a)
        );
        assert!(
            (pos_of(id_b) - 15.0).abs() < 0.01,
            "B moved to 15, got {}",
            pos_of(id_b)
        );
        assert!(
            (pos_of(id_c) - 20.0).abs() < 0.01,
            "C unchanged, got {}",
            pos_of(id_c)
        );
    }

    #[test]
    fn mcp_scale_selection_by_scales_all_selected_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({
                        "entities": [
                            {"name": "A", "position": [0.0, 0.0, 0.0]},
                            {"name": "B", "position": [1.0, 0.0, 0.0]}
                        ]
                    }),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let find_id = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (id_a, id_b) = (find_id("A"), find_id("B"));

        // Set initial scale for A
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "set_scale",
                    json!({"entity_id": id_a, "sx": 2.0, "sy": 2.0, "sz": 2.0}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        // Select A only
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": id_a}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "scale_selection_by",
                    json!({"sx": 3.0, "sy": 1.0, "sz": 1.0}),
                )
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["scaled_count"].as_u64().unwrap(), 1);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let scale_x_of = |id: u64| {
            ents.iter()
                .find(|e| e["id"].as_u64() == Some(id))
                .and_then(|e| e["scale"].as_array())
                .map(|s| s[0].as_f64().unwrap())
                .unwrap_or(f64::NAN)
        };
        // A started at (2,2,2), multiplied by (3,1,1) → (6,2,2)
        assert!(
            (scale_x_of(id_a) - 6.0).abs() < 0.01,
            "A scale_x = 6, got {}",
            scale_x_of(id_a)
        );
        // B untouched — batch_spawn gives default scale [1,1,1]
        assert!(
            (scale_x_of(id_b) - 1.0).abs() < 0.01,
            "B scale_x unchanged, got {}",
            scale_x_of(id_b)
        );
    }

    #[test]
    fn mcp_add_tag_to_selection_tags_all_selected_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "A"})).unwrap();
            m.execute("spawn_entity", json!({"name": "B"})).unwrap();
            m.execute("spawn_entity", json!({"name": "C"})).unwrap();
        }
        app.update();
        app.update();

        let ids: Vec<u64> = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e["id"].as_u64().unwrap())
                .collect()
        };
        let (id_a, id_b, id_c) = (ids[0], ids[1], ids[2]);

        // Select A and B
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": id_a}))
                .unwrap();
        }
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": id_b}))
                .unwrap();
        }

        // Add tag to all selected
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("add_tag_to_selection", json!({"tag": "hero"}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["tagged_count"].as_u64().unwrap(), 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let snap = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": id_a}))
            .unwrap();
        assert!(
            snap.content["entity"]["tags"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t.as_str() == Some("hero")),
            "A tagged"
        );
        let snap_b = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": id_b}))
            .unwrap();
        assert!(
            snap_b.content["entity"]["tags"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t.as_str() == Some("hero")),
            "B tagged"
        );
        let snap_c = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": id_c}))
            .unwrap();
        assert!(
            !snap_c.content["entity"]["tags"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t.as_str() == Some("hero")),
            "C not tagged"
        );
    }

    #[test]
    fn mcp_remove_tag_from_selection_untags_all_selected_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "A"})).unwrap();
            m.execute("spawn_entity", json!({"name": "B"})).unwrap();
        }
        app.update();
        app.update();

        let ids: Vec<u64> = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e["id"].as_u64().unwrap())
                .collect()
        };
        let (id_a, id_b) = (ids[0], ids[1]);

        // Tag both entities
        for id in [id_a, id_b] {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("tag_entity", json!({"entity_id": id, "tag": "hero"}))
                .unwrap();
            app.update();
            app.update();
        }
        // Select A only
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": id_a}))
                .unwrap();
        }
        // Remove tag from selection
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("remove_tag_from_selection", json!({"tag": "hero"}))
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["untagged_count"].as_u64().unwrap(), 1);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let snap_a = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": id_a}))
            .unwrap();
        assert!(
            !snap_a.content["entity"]["tags"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t.as_str() == Some("hero")),
            "A untagged"
        );
        let snap_b = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": id_b}))
            .unwrap();
        assert!(
            snap_b.content["entity"]["tags"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t.as_str() == Some("hero")),
            "B still tagged"
        );
    }

    #[test]
    fn mcp_group_entities_by_tag_returns_map_of_tag_to_entity_ids() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "A"})).unwrap();
            m.execute("spawn_entity", json!({"name": "B"})).unwrap();
        }
        app.update();
        app.update();

        let ids: Vec<u64> = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e["id"].as_u64().unwrap())
                .collect()
        };
        let (id_a, id_b) = (ids[0], ids[1]);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("tag_entity", json!({"entity_id": id_a, "tag": "hero"}))
                .unwrap();
        }
        app.update();
        app.update();
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("tag_entity", json!({"entity_id": id_b, "tag": "enemy"}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("group_entities_by_tag", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let groups = &out.content["groups"];
        let hero_ids: Vec<u64> = groups["hero"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e.as_u64().unwrap())
            .collect();
        let enemy_ids: Vec<u64> = groups["enemy"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e.as_u64().unwrap())
            .collect();
        assert!(hero_ids.contains(&id_a));
        assert!(enemy_ids.contains(&id_b));
    }

    #[test]
    fn mcp_batch_tag_entities_adds_tag_to_multiple_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "A"})).unwrap();
            m.execute("spawn_entity", json!({"name": "B"})).unwrap();
            m.execute("spawn_entity", json!({"name": "C"})).unwrap();
        }
        app.update();
        app.update();

        let ids: Vec<u64> = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e["id"].as_u64().unwrap())
                .collect()
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "batch_tag_entities",
                    json!({"entity_ids": [ids[0], ids[1]], "tag": "group"}),
                )
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["tagged_count"].as_u64().unwrap(), 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let tagged: Vec<u64> = ents
            .iter()
            .filter(|e| {
                e["tags"]
                    .as_array()
                    .map(|t| t.iter().any(|v| v.as_str() == Some("group")))
                    .unwrap_or(false)
            })
            .map(|e| e["id"].as_u64().unwrap())
            .collect();
        assert_eq!(tagged.len(), 2, "A and B tagged with 'group'");
    }

    #[test]
    fn mcp_batch_untag_entities_removes_tag_from_multiple_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "X"})).unwrap();
            m.execute("spawn_entity", json!({"name": "Y"})).unwrap();
        }
        app.update();
        app.update();

        let ids: Vec<u64> = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e["id"].as_u64().unwrap())
                .collect()
        };
        for &eid in &ids {
            {
                let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
                mcp.0
                    .lock()
                    .unwrap()
                    .execute("tag_entity", json!({"entity_id": eid, "tag": "remove_me"}))
                    .unwrap();
            }
            app.update();
            app.update();
        }
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "batch_untag_entities",
                    json!({"entity_ids": [ids[0], ids[1]], "tag": "remove_me"}),
                )
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["untagged_count"].as_u64().unwrap(), 2);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        for e in &ents {
            let tags = e["tags"].as_array().unwrap();
            assert!(
                !tags.iter().any(|t| t.as_str() == Some("remove_me")),
                "tag removed from all entities"
            );
        }
    }

    #[test]
    fn mcp_clear_all_tags_from_entity_removes_all_tags() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "MultiTagged"}))
                .unwrap();
        }
        app.update();
        app.update();

        let eid = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("MultiTagged"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        for tag in &["foo", "bar"] {
            {
                let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
                mcp.0
                    .lock()
                    .unwrap()
                    .execute("tag_entity", json!({"entity_id": eid, "tag": tag}))
                    .unwrap();
            }
            app.update();
            app.update();
        }
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("clear_all_tags_from_entity", json!({"entity_id": eid}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let tags = ents.iter().find(|e| e["id"].as_u64() == Some(eid)).unwrap()["tags"]
            .as_array()
            .unwrap();
        assert!(tags.is_empty(), "all tags cleared");
    }

    #[test]
    fn mcp_toggle_entity_visibility_flips_visible_state() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "Toggler"}))
                .unwrap();
        }
        app.update();
        app.update();

        let eid = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("Toggler"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("toggle_entity_visibility", json!({"entity_id": eid}))
                .unwrap();
        }
        app.update();
        app.update();
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let v1 = mcp
                .0
                .lock()
                .unwrap()
                .execute("get_entity", json!({"entity_id": eid}))
                .unwrap();
            assert!(
                !v1.content["entity"]["visible"].as_bool().unwrap(),
                "after first toggle: hidden"
            );
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("toggle_entity_visibility", json!({"entity_id": eid}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let v2 = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": eid}))
            .unwrap();
        assert!(
            v2.content["entity"]["visible"].as_bool().unwrap(),
            "after second toggle: visible again"
        );
    }

    #[test]
    fn mcp_move_entity_to_origin_sets_position_to_zero() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [{"name": "Mover", "position": [5.0, 3.0, 1.0]}]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let eid = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("Mover"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("move_entity_to_origin", json!({"entity_id": eid}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let pos = ents.iter().find(|e| e["id"].as_u64() == Some(eid)).unwrap()["position"]
            .as_array()
            .unwrap()
            .clone();
        assert!((pos[0].as_f64().unwrap()).abs() < 0.01, "x=0");
        assert!((pos[1].as_f64().unwrap()).abs() < 0.01, "y=0");
        assert!((pos[2].as_f64().unwrap()).abs() < 0.01, "z=0");
    }

    #[test]
    fn mcp_rename_entity_with_suffix_appends_suffix_to_name() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "Hero"}))
                .unwrap();
        }
        app.update();
        app.update();

        let eid = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("Hero"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "rename_entity_with_suffix",
                    json!({"entity_id": eid, "suffix": "_01"}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let name = ents.iter().find(|e| e["id"].as_u64() == Some(eid)).unwrap()["name"]
            .as_str()
            .unwrap();
        assert_eq!(name, "Hero_01", "suffix appended to original name");
    }

    #[test]
    fn mcp_set_entity_scale_uniform_applies_same_value_to_all_axes() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "Uniform", "position": [0.0, 0.0, 0.0]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let entity_id = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("Uniform"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "set_entity_scale_uniform",
                    json!({"entity_id": entity_id, "scale": 3.0}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": entity_id}))
            .unwrap();
        assert!(out.is_ok());
        let s = &out.content["entity"]["scale"];
        assert!((s[0].as_f64().unwrap() - 3.0).abs() < 1e-3, "sx=3");
        assert!((s[1].as_f64().unwrap() - 3.0).abs() < 1e-3, "sy=3");
        assert!((s[2].as_f64().unwrap() - 3.0).abs() < 1e-3, "sz=3");
    }

    #[test]
    fn mcp_copy_tags_from_entity_copies_all_tags() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "TagSource"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "TagDest"}))
                .unwrap();
        }
        app.update();
        app.update();

        let (src_id, dst_id) = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let ents = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone();
            let s = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("TagSource"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let d = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("TagDest"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (s, d)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("tag_entity", json!({"entity_id": src_id, "tag": "special"}))
                .unwrap();
        }
        app.update();
        app.update();
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "copy_tags_from_entity",
                    json!({"source_id": src_id, "target_id": dst_id}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": dst_id}))
            .unwrap();
        assert!(out.is_ok());
        let tags: Vec<&str> = out.content["entity"]["tags"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t.as_str())
            .collect();
        assert!(tags.contains(&"special"), "special tag copied to dest");
    }

    #[test]
    fn mcp_scale_selected_entities_sets_scale_on_all_selected() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "ScaleA", "position": [0.0, 0.0, 0.0]},
                        {"name": "ScaleB", "position": [0.0, 0.0, 0.0]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (id_a, id_b) = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let ents = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone();
            let a = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("ScaleA"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let b = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("ScaleB"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (a, b)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("select_entity", json!({"entity_id": id_a}))
                .unwrap();
            m.execute("select_entity", json!({"entity_id": id_b}))
                .unwrap();
        }
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "scale_selected_entities",
                    json!({"sx": 2.0, "sy": 3.0, "sz": 0.5}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out_a = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": id_a}))
            .unwrap();
        let s = &out_a.content["entity"]["scale"];
        assert!((s[0].as_f64().unwrap() - 2.0).abs() < 1e-3, "ScaleA sx=2");
        assert!((s[1].as_f64().unwrap() - 3.0).abs() < 1e-3, "ScaleA sy=3");
        assert!((s[2].as_f64().unwrap() - 0.5).abs() < 1e-3, "ScaleA sz=0.5");
    }

    #[test]
    fn mcp_reset_entity_transform_sets_default_values() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "ResetEnt", "position": [5.0, 10.0, -3.0]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let eid = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("ResetEnt"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("reset_entity_transform", json!({"entity_id": eid}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let pos = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": eid}))
            .unwrap();
        let p = &pos.content["entity"]["position"];
        assert!((p[0].as_f64().unwrap()).abs() < 1e-3, "x reset to 0");
        assert!((p[1].as_f64().unwrap()).abs() < 1e-3, "y reset to 0");
        assert!((p[2].as_f64().unwrap()).abs() < 1e-3, "z reset to 0");
    }

    #[test]
    fn mcp_offset_selected_positions_moves_all_selected() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "OffA", "position": [1.0, 0.0, 0.0]},
                        {"name": "OffB", "position": [2.0, 0.0, 0.0]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (id_a, id_b) = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let ents = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone();
            let a = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("OffA"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let b = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("OffB"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (a, b)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("select_entity", json!({"entity_id": id_a}))
                .unwrap();
            m.execute("select_entity", json!({"entity_id": id_b}))
                .unwrap();
        }
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "offset_selected_positions",
                    json!({"dx": 5.0, "dy": 2.0, "dz": -1.0}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let pa = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": id_a}))
            .unwrap();
        let pb = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": id_b}))
            .unwrap();
        assert!(
            (pa.content["entity"]["position"][0].as_f64().unwrap() - 6.0).abs() < 1e-3,
            "OffA x = 1+5 = 6"
        );
        assert!(
            (pb.content["entity"]["position"][0].as_f64().unwrap() - 7.0).abs() < 1e-3,
            "OffB x = 2+5 = 7"
        );
    }

    #[test]
    fn mcp_copy_entity_transform_copies_position_to_target() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "Src", "position": [7.0, 3.0, -2.0]},
                        {"name": "Dst", "position": [0.0, 0.0, 0.0]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (src_id, dst_id) = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let ents = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone();
            let s = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("Src"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let d = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("Dst"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (s, d)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "copy_entity_transform",
                    json!({"source_entity_id": src_id, "target_entity_id": dst_id}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let pos = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": dst_id}))
            .unwrap();
        let p = &pos.content["entity"]["position"];
        assert!((p[0].as_f64().unwrap() - 7.0).abs() < 1e-3, "x=7 copied");
        assert!((p[1].as_f64().unwrap() - 3.0).abs() < 1e-3, "y=3 copied");
        assert!(
            (p[2].as_f64().unwrap() - (-2.0)).abs() < 1e-3,
            "z=-2 copied"
        );
    }

    #[test]
    fn mcp_clear_all_tags_removes_all_tags_from_all_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "CatA"})).unwrap();
            m.execute("spawn_entity", json!({"name": "CatB"})).unwrap();
        }
        app.update();
        app.update();
        let (id_a, id_b) = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let ents = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone();
            let a = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("CatA"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let b = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("CatB"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (a, b)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("tag_entity", json!({"entity_id": id_a, "tag": "enemy"}))
                .unwrap();
        }
        app.update();
        app.update();
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("tag_entity", json!({"entity_id": id_b, "tag": "ally"}))
                .unwrap();
        }
        app.update();
        app.update();
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("clear_all_tags", json!({}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        for e in &ents {
            let tags = e["tags"].as_array().map(|a| a.len()).unwrap_or(0);
            assert_eq!(
                tags,
                0,
                "all tags cleared on {}",
                e["name"].as_str().unwrap_or("?")
            );
        }
    }

    #[test]
    fn mcp_rename_entities_with_prefix_renames_matching() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "OldA"},
                        {"name": "OldB"},
                        {"name": "Keep"}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "rename_entities_with_prefix",
                    json!({"old_prefix": "Old", "new_prefix": "New"}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let names: Vec<&str> = ents.iter().filter_map(|e| e["name"].as_str()).collect();
        assert!(names.contains(&"NewA"), "OldA renamed to NewA");
        assert!(names.contains(&"NewB"), "OldB renamed to NewB");
        assert!(names.contains(&"Keep"), "Keep unchanged");
        assert!(
            !names.iter().any(|n| n.starts_with("Old")),
            "no Old* names remain"
        );
    }

    #[test]
    fn mcp_set_all_visible_toggles_all_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "VisA"},
                        {"name": "VisB"}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();
        // hide all
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("set_all_visible", json!({"visible": false}))
                .unwrap();
        }
        app.update();
        app.update();
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let ents = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone();
            for e in &ents {
                assert!(
                    !e["visible"].as_bool().unwrap_or(true),
                    "all should be hidden"
                );
            }
        }
        // show all
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("set_all_visible", json!({"visible": true}))
                .unwrap();
        }
        app.update();
        app.update();
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        for e in &ents {
            assert!(
                e["visible"].as_bool().unwrap_or(false),
                "all should be visible"
            );
        }
    }

    #[test]
    fn mcp_snap_entity_to_grid_rounds_position() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "SnapEnt", "position": [1.3, 2.7, -0.6]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let eid = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("SnapEnt"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "snap_entity_to_grid",
                    json!({"entity_id": eid, "grid_size": 1.0}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let pos_out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": eid}))
            .unwrap();
        let p = &pos_out.content["entity"]["position"];
        assert!(
            (p[0].as_f64().unwrap() - 1.0).abs() < 1e-3,
            "x snapped to 1.0"
        );
        assert!(
            (p[1].as_f64().unwrap() - 3.0).abs() < 1e-3,
            "y snapped to 3.0"
        );
        assert!(
            (p[2].as_f64().unwrap() - (-1.0)).abs() < 1e-3,
            "z snapped to -1.0"
        );
    }

    #[test]
    fn mcp_delete_selected_entities_despawns_selection() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "DelSelA"},
                        {"name": "DelSelB"},
                        {"name": "DelSelKeep"}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (id_a, id_b) = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let ents = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone();
            let a = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("DelSelA"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let b = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("DelSelB"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (a, b)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("select_entity", json!({"entity_id": id_a}))
                .unwrap();
            m.execute("select_entity", json!({"entity_id": id_b}))
                .unwrap();
        }
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("delete_selected_entities", json!({}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let ids: Vec<u64> = ents.iter().map(|e| e["id"].as_u64().unwrap()).collect();
        assert!(!ids.contains(&id_a), "DelSelA should be deleted");
        assert!(!ids.contains(&id_b), "DelSelB should be deleted");
        assert!(
            ents.iter()
                .any(|e| e["name"].as_str() == Some("DelSelKeep")),
            "DelSelKeep should remain"
        );
    }

    #[test]
    fn mcp_invert_selection_swaps_selected_and_unselected() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "InvSelA"},
                        {"name": "InvSelB"},
                        {"name": "InvSelC"}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (id_a, id_b, id_c) = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let ents = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone();
            let a = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("InvSelA"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let b = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("InvSelB"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let c = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("InvSelC"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (a, b, c)
        };
        // select only A
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": id_a}))
                .unwrap();
        }
        // invert selection → B and C selected, A deselected
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("invert_selection", json!({}))
                .unwrap();
        }

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let sel = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_selection", json!({}))
            .unwrap();
        let ids: Vec<u64> = sel.content["selected_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e.as_u64().unwrap())
            .collect();
        assert!(
            !ids.contains(&id_a),
            "A was selected before → not selected after invert"
        );
        assert!(
            ids.contains(&id_b),
            "B was not selected → selected after invert"
        );
        assert!(
            ids.contains(&id_c),
            "C was not selected → selected after invert"
        );
    }

    #[test]
    fn mcp_mirror_selected_on_axis_negates_axis_position() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "MirrorA", "position": [5.0, 2.0, 3.0]},
                        {"name": "MirrorB", "position": [10.0, 4.0, 6.0]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (id_a, id_b) = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let ents = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone();
            let a = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("MirrorA"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let b = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("MirrorB"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (a, b)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("select_entity", json!({"entity_id": id_a}))
                .unwrap();
            m.execute("select_entity", json!({"entity_id": id_b}))
                .unwrap();
        }
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("mirror_selected_on_axis", json!({"axis": "x"}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let pa = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": id_a}))
            .unwrap();
        let pb = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": id_b}))
            .unwrap();
        assert!(
            (pa.content["entity"]["position"][0].as_f64().unwrap() - (-5.0)).abs() < 1e-3,
            "MirrorA x should be -5"
        );
        assert!(
            (pa.content["entity"]["position"][1].as_f64().unwrap() - 2.0).abs() < 1e-3,
            "MirrorA y unchanged"
        );
        assert!(
            (pb.content["entity"]["position"][0].as_f64().unwrap() - (-10.0)).abs() < 1e-3,
            "MirrorB x should be -10"
        );
    }

    #[test]
    fn mcp_reset_entity_transform_restores_identity() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [{"name": "ResetTarget", "position": [5.0, 5.0, 5.0]}]}),
                )
                .unwrap();
        }
        app.update();
        app.update();
        let eid = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("ResetTarget"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "set_rotation",
                    json!({"entity_id": eid, "rx": 45.0, "ry": 45.0, "rz": 0.0}),
                )
                .unwrap();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "set_scale",
                    json!({"entity_id": eid, "sx": 2.0, "sy": 2.0, "sz": 2.0}),
                )
                .unwrap();
        }
        app.update();
        app.update();
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("reset_entity_transform", json!({"entity_id": eid}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let pos = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": eid}))
            .unwrap();
        let p = &pos.content["entity"]["position"];
        assert!((p[0].as_f64().unwrap()).abs() < 1e-3, "x=0");
        assert!((p[1].as_f64().unwrap()).abs() < 1e-3, "y=0");
        assert!((p[2].as_f64().unwrap()).abs() < 1e-3, "z=0");
        let sc = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": eid}))
            .unwrap();
        let s = &sc.content["entity"]["scale"];
        assert!((s[0].as_f64().unwrap() - 1.0).abs() < 1e-3, "sx=1");
        assert!((s[1].as_f64().unwrap() - 1.0).abs() < 1e-3, "sy=1");
        assert!((s[2].as_f64().unwrap() - 1.0).abs() < 1e-3, "sz=1");
    }

    #[test]
    fn mcp_scale_entity_uniform_sets_equal_scale() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [{"name": "ScaleTarget", "position": [0.0,0.0,0.0]}]}),
                )
                .unwrap();
        }
        app.update();
        app.update();
        let eid = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("ScaleTarget"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "scale_entity_uniform",
                    json!({"entity_id": eid, "factor": 3.0}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": eid}))
            .unwrap();
        assert!(out.is_ok());
        let s = &out.content["entity"]["scale"];
        assert!((s[0].as_f64().unwrap() - 3.0).abs() < 1e-3, "sx=3");
        assert!((s[1].as_f64().unwrap() - 3.0).abs() < 1e-3, "sy=3");
        assert!((s[2].as_f64().unwrap() - 3.0).abs() < 1e-3, "sz=3");
    }

    #[test]
    fn mcp_move_selected_entities_offsets_positions() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "MoveSelA", "position": [0.0, 0.0, 0.0]},
                        {"name": "MoveSelB", "position": [5.0, 0.0, 0.0]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (id_a, id_b) = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let ents = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone();
            let a = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("MoveSelA"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let b = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("MoveSelB"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (a, b)
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("select_entity", json!({"entity_id": id_a}))
                .unwrap();
            m.execute("select_entity", json!({"entity_id": id_b}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "move_selected_entities",
                    json!({"dx": 0.0, "dy": 3.0, "dz": 0.0}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let pa = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": id_a}))
            .unwrap();
        let pb = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": id_b}))
            .unwrap();
        assert!(
            (pa.content["entity"]["position"][1].as_f64().unwrap() - 3.0).abs() < 1e-3,
            "MoveSelA y should be 3"
        );
        assert!(
            (pb.content["entity"]["position"][1].as_f64().unwrap() - 3.0).abs() < 1e-3,
            "MoveSelB y should be 3"
        );
        assert!(
            (pb.content["entity"]["position"][0].as_f64().unwrap() - 5.0).abs() < 1e-3,
            "MoveSelB x should still be 5"
        );
    }

    #[test]
    fn mcp_detach_all_meshes_removes_mesh_renderers() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "DetachA"},
                        {"name": "DetachB"}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (id_a, id_b) = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let ents = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone();
            let a = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("DetachA"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let b = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("DetachB"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (a, b)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("attach_mesh", json!({"entity_id": id_a, "mesh_id": 1}))
                .unwrap();
            m.execute("attach_mesh", json!({"entity_id": id_b, "mesh_id": 2}))
                .unwrap();
        }
        app.update();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("detach_all_meshes", json!({}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("query_entities", json!({"has_mesh": true}))
            .unwrap();
        let meshed_ids: Vec<u64> = out.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_u64().unwrap())
            .collect();
        assert!(
            !meshed_ids.contains(&id_a),
            "DetachA should have no mesh after detach_all"
        );
        assert!(
            !meshed_ids.contains(&id_b),
            "DetachB should have no mesh after detach_all"
        );
    }

    #[test]
    fn mcp_align_selected_on_axis_aligns_y_axis() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "AlignA", "position": [0.0, 1.0, 0.0]},
                        {"name": "AlignB", "position": [5.0, 3.0, 2.0]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (id_a, id_b) = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let ents = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone();
            let a = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("AlignA"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let b = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("AlignB"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (a, b)
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("select_entity", json!({"entity_id": id_a}))
                .unwrap();
            m.execute("select_entity", json!({"entity_id": id_b}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "align_selected_on_axis",
                    json!({"axis": "y", "value": 10.0}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let pa = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": id_a}))
            .unwrap();
        let pb = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": id_b}))
            .unwrap();
        assert!(
            (pa.content["entity"]["position"][1].as_f64().unwrap() - 10.0).abs() < 1e-3,
            "AlignA y should be 10"
        );
        assert!(
            (pb.content["entity"]["position"][1].as_f64().unwrap() - 10.0).abs() < 1e-3,
            "AlignB y should be 10"
        );
    }

    #[test]
    fn mcp_copy_transform_copies_position_rotation_scale() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "CpySrc", "position": [7.0, 8.0, 9.0]},
                        {"name": "CpyDst", "position": [0.0, 0.0, 0.0]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (src_id, dst_id) = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let entities = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone();
            let s = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("CpySrc"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let d = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("CpyDst"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (s, d)
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "copy_transform",
                    json!({"from_entity_id": src_id, "to_entity_id": dst_id}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let dst = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": dst_id}))
            .unwrap();
        let pos = &dst.content["entity"]["position"];
        assert!(
            (pos[0].as_f64().unwrap() - 7.0).abs() < 1e-3,
            "x should be 7 after copy"
        );
        assert!(
            (pos[1].as_f64().unwrap() - 8.0).abs() < 1e-3,
            "y should be 8 after copy"
        );
        assert!(
            (pos[2].as_f64().unwrap() - 9.0).abs() < 1e-3,
            "z should be 9 after copy"
        );
    }

    #[test]
    fn mcp_search_entities_finds_by_name_and_tag() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute(
                "batch_spawn",
                json!({"entities": [
                    {"name": "SearchByName"},
                    {"name": "TagMatch"},
                    {"name": "Neither"}
                ]}),
            )
            .unwrap();
        }
        app.update();
        app.update();

        let tag_id = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("TagMatch"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "tag_entity",
                    json!({"entity_id": tag_id, "tag": "searchable"}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("search_entities", json!({"query": "searchable"}))
            .unwrap();
        assert!(out.is_ok());
        let names: Vec<&str> = out.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|e| e["name"].as_str())
            .collect();
        assert!(
            names.contains(&"SearchByName") || names.contains(&"TagMatch"),
            "should find by name or tag"
        );
        assert!(!names.contains(&"Neither"), "Neither should not match");
    }

    #[test]
    fn mcp_reset_transform_returns_entity_to_identity() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "ResetMe", "position": [50.0, 30.0, 10.0]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let entity_id = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("ResetMe"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("reset_transform", json!({"entity_id": entity_id}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let e = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": entity_id}))
            .unwrap();
        let pos = &e.content["entity"]["position"];
        assert!(
            (pos[0].as_f64().unwrap()).abs() < 1e-4,
            "x should be 0 after reset"
        );
        assert!(
            (pos[1].as_f64().unwrap()).abs() < 1e-4,
            "y should be 0 after reset"
        );
        assert!(
            (pos[2].as_f64().unwrap()).abs() < 1e-4,
            "z should be 0 after reset"
        );
        let scale = &e.content["entity"]["scale"];
        assert!(
            (scale[0].as_f64().unwrap() - 1.0).abs() < 1e-4,
            "scale.x should be 1 after reset"
        );
    }

    #[test]
    fn mcp_translate_selected_entities_moves_by_delta() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "TSelEntity", "position": [0.0, 0.0, 0.0]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let entity_id = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("TSelEntity"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let mcp = mcp.0.lock().unwrap();
            mcp.execute("select_entity", json!({"entity_id": entity_id}))
                .unwrap();
            mcp.execute(
                "translate_selected_entities",
                json!({"dx": 3.0, "dy": 0.0, "dz": 0.0}),
            )
            .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let e = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": entity_id}))
            .unwrap();
        let pos = &e.content["entity"]["position"];
        assert!(
            (pos[0].as_f64().unwrap() - 3.0).abs() < 1e-4,
            "x should be 0+3=3"
        );
    }

    #[test]
    fn mcp_mirror_entity_flips_position_on_axis() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "MirrorMe", "position": [3.0, 5.0, 7.0]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let entity_id = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("MirrorMe"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "mirror_entity",
                    json!({"entity_id": entity_id, "axis": "x"}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let e = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": entity_id}))
            .unwrap();
        let pos = &e.content["entity"]["position"];
        assert!(
            (pos[0].as_f64().unwrap() - (-3.0)).abs() < 1e-4,
            "x should be negated"
        );
        assert!((pos[1].as_f64().unwrap() - 5.0).abs() < 1e-4, "y unchanged");
        assert!((pos[2].as_f64().unwrap() - 7.0).abs() < 1e-4, "z unchanged");
    }

    #[test]
    fn mcp_invert_selection_flips_all() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "InvA", "position": [0.0,0.0,0.0]},
                        {"name": "InvB", "position": [1.0,0.0,0.0]},
                        {"name": "InvC", "position": [2.0,0.0,0.0]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (id_a, id_b, id_c) = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let list = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap();
            let entities = list.content["entities"].as_array().unwrap();
            let a = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("InvA"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let b = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("InvB"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let c = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("InvC"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (a, b, c)
        };

        // select only A
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entity", json!({"entity_id": id_a}))
                .unwrap();
        }

        // invert → A deselected, B+C selected
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("invert_selection", json!({}))
                .unwrap();
        }

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let sel = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_selection", json!({}))
            .unwrap();
        let selected: Vec<u64> = sel.content["selected_ids"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_u64())
            .collect();
        assert!(
            !selected.contains(&id_a),
            "A should be deselected after invert"
        );
        assert!(
            selected.contains(&id_b),
            "B should be selected after invert"
        );
        assert!(
            selected.contains(&id_c),
            "C should be selected after invert"
        );
    }

    #[test]
    fn mcp_snap_to_grid_rounds_position() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "SnapMe", "position": [1.3, 2.7, 4.4]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let entity_id = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("SnapMe"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "snap_to_grid",
                    json!({"entity_id": entity_id, "grid_size": 1.0}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let e = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": entity_id}))
            .unwrap();
        let pos = &e.content["entity"]["position"];
        assert!(
            (pos[0].as_f64().unwrap() - 1.0).abs() < 1e-4,
            "x should snap to 1.0"
        );
        assert!(
            (pos[1].as_f64().unwrap() - 3.0).abs() < 1e-4,
            "y should snap to 3.0"
        );
        assert!(
            (pos[2].as_f64().unwrap() - 4.0).abs() < 1e-4,
            "z should snap to 4.0"
        );
    }

    #[test]
    fn mcp_get_scene_hierarchy_returns_nested_tree() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "HRoot"}))
                .unwrap();
        }
        app.update();
        app.update();

        let root_id = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("HRoot"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "HChild"}))
                .unwrap();
        }
        app.update();
        app.update();

        let child_id = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("HChild"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "set_parent",
                    json!({"entity_id": child_id, "parent_id": root_id}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_scene_hierarchy", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let roots = out.content["roots"].as_array().unwrap();
        let root_node = roots
            .iter()
            .find(|n| n["name"].as_str() == Some("HRoot"))
            .unwrap();
        let children = root_node["children"].as_array().unwrap();
        assert_eq!(children.len(), 1, "HRoot should have 1 child");
        assert_eq!(children[0]["name"], "HChild");
    }

    #[test]
    fn mcp_copy_transform_copies_position_to_target() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "Source", "position": [5.0, 10.0, 15.0]},
                        {"name": "Target", "position": [0.0, 0.0, 0.0]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (source_id, target_id) = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let list = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap();
            let entities = list.content["entities"].as_array().unwrap();
            let s = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("Source"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let t = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("Target"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (s, t)
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "copy_transform",
                    json!({"from_entity_id": source_id, "to_entity_id": target_id}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let t = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": target_id}))
            .unwrap();
        let pos = &t.content["entity"]["position"];
        assert!(
            (pos[0].as_f64().unwrap() - 5.0).abs() < 1e-4,
            "x should match source"
        );
        assert!(
            (pos[1].as_f64().unwrap() - 10.0).abs() < 1e-4,
            "y should match source"
        );
        assert!(
            (pos[2].as_f64().unwrap() - 15.0).abs() < 1e-4,
            "z should match source"
        );
    }

    #[test]
    fn mcp_rotate_selected_entities_applies_rotation() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [{"name": "RotateMe", "position": [0.0, 0.0, 0.0]}]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let entity_id = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("RotateMe"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let mcp = mcp.0.lock().unwrap();
            mcp.execute("select_entity", json!({"entity_id": entity_id}))
                .unwrap();
            mcp.execute(
                "rotate_selected_entities",
                json!({"rx": 45.0, "ry": 0.0, "rz": 0.0}),
            )
            .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let e = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": entity_id}))
            .unwrap();
        let rot = &e.content["entity"]["rotation"];
        assert!(
            (rot[0].as_f64().unwrap() - 45.0).abs() < 1e-3,
            "rotation.x should be ~45 degrees"
        );
    }

    #[test]
    fn mcp_scale_selected_entities_applies_scale() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "ScaleMe", "position": [0.0, 0.0, 0.0]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let entity_id = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("ScaleMe"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let mcp = mcp.0.lock().unwrap();
            mcp.execute("select_entity", json!({"entity_id": entity_id}))
                .unwrap();
            mcp.execute(
                "scale_selected_entities",
                json!({"sx": 3.0, "sy": 3.0, "sz": 3.0}),
            )
            .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let e = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": entity_id}))
            .unwrap();
        let scale = &e.content["entity"]["scale"];
        assert!(
            (scale[0].as_f64().unwrap() - 3.0).abs() < 1e-4,
            "scale.x should be 3"
        );
    }

    #[test]
    fn mcp_toggle_visible_flips_state() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "Flipper"}))
                .unwrap();
        }
        app.update();
        app.update();

        let entity_id = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("Flipper"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        // toggle once: true → false
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("toggle_visible", json!({"entity_id": entity_id}))
                .unwrap();
        }
        app.update();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let list = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap();
            let e = list.content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("Flipper"))
                .unwrap();
            assert_eq!(e["visible"], false, "should be hidden after first toggle");
        }

        // toggle again: false → true
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("toggle_visible", json!({"entity_id": entity_id}))
                .unwrap();
        }
        app.update();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let list = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap();
            let e = list.content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("Flipper"))
                .unwrap();
            assert_eq!(e["visible"], true, "should be visible after second toggle");
        }
    }

    #[test]
    fn mcp_despawn_selected_removes_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "ToDelete"}))
                .unwrap();
        }
        app.update();
        app.update();

        let entity_id = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("ToDelete"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let mcp = mcp.0.lock().unwrap();
            mcp.execute("select_entity", json!({"entity_id": entity_id}))
                .unwrap();
            mcp.execute("despawn_selected", json!({})).unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let list = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap();
        let found = list.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["name"].as_str() == Some("ToDelete"));
        assert!(!found, "ToDelete should be removed after despawn_selected");
    }

    #[test]
    fn mcp_hide_selected_and_show_selected() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "ToggleVis"}))
                .unwrap();
        }
        app.update();
        app.update();

        let entity_id = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("ToggleVis"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let mcp = mcp.0.lock().unwrap();
            mcp.execute("select_entity", json!({"entity_id": entity_id}))
                .unwrap();
            mcp.execute("hide_selected", json!({})).unwrap();
        }
        app.update();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let list = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap();
            let e = list.content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("ToggleVis"))
                .unwrap();
            assert_eq!(e["visible"], false, "should be hidden after hide_selected");
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("show_selected", json!({}))
                .unwrap();
        }
        app.update();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let list = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap();
            let e = list.content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("ToggleVis"))
                .unwrap();
            assert_eq!(e["visible"], true, "should be visible after show_selected");
        }
    }

    #[test]
    fn mcp_move_selected_entities_moves_all() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "M1", "position": [1.0, 0.0, 0.0]},
                        {"name": "M2", "position": [4.0, 0.0, 0.0]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (id1, id2) = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let list = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap();
            let entities = list.content["entities"].as_array().unwrap();
            let id1 = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("M1"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let id2 = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("M2"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (id1, id2)
        };

        // select both
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let mcp = mcp.0.lock().unwrap();
            mcp.execute("select_entity", json!({"entity_id": id1}))
                .unwrap();
            mcp.execute("select_entity", json!({"entity_id": id2}))
                .unwrap();
        }

        // move selected by dx=10
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "move_selected_entities",
                    json!({"dx": 10.0, "dy": 0.0, "dz": 0.0}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let list = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap();
        let entities = list.content["entities"].as_array().unwrap();

        let m1 = entities
            .iter()
            .find(|e| e["name"].as_str() == Some("M1"))
            .unwrap();
        let m2 = entities
            .iter()
            .find(|e| e["name"].as_str() == Some("M2"))
            .unwrap();
        let x1 = m1["position"][0].as_f64().unwrap();
        let x2 = m2["position"][0].as_f64().unwrap();
        assert!((x1 - 11.0).abs() < 1e-4, "M1 x should be 11, got {}", x1);
        assert!((x2 - 14.0).abs() < 1e-4, "M2 x should be 14, got {}", x2);
    }

    #[test]
    fn mcp_set_entity_transform_applies_all_fields() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [{"name": "Cube", "position": [0.0, 0.0, 0.0]}]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let entity_id = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("Cube"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "set_entity_transform",
                    json!({
                        "entity_id": entity_id,
                        "position": [5.0, 6.0, 7.0],
                        "scale": [2.0, 2.0, 2.0]
                    }),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let entity = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": entity_id}))
            .unwrap();
        let e = &entity.content["entity"];
        let pos = e["position"].as_array().unwrap();
        assert!(
            (pos[0].as_f64().unwrap() - 5.0).abs() < 1e-4,
            "x should be 5"
        );
        assert!(
            (pos[1].as_f64().unwrap() - 6.0).abs() < 1e-4,
            "y should be 6"
        );
        let scale = e["scale"].as_array().unwrap();
        assert!(
            (scale[0].as_f64().unwrap() - 2.0).abs() < 1e-4,
            "scale x should be 2"
        );
    }

    #[test]
    fn mcp_select_all_and_deselect_all() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let mcp = mcp.0.lock().unwrap();
            mcp.execute("spawn_entity", json!({"name": "A"})).unwrap();
            mcp.execute("spawn_entity", json!({"name": "B"})).unwrap();
        }
        app.update();
        app.update();

        // select_all
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("select_all", json!({}))
                .unwrap();
            assert!(out.is_ok());
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("get_selection", json!({}))
                .unwrap();
            let ids = out.content["selected_ids"].as_array().unwrap();
            assert!(
                ids.len() >= 2,
                "select_all should select at least 2 entities"
            );
        }

        // deselect_all
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("deselect_all", json!({}))
                .unwrap();
            assert!(out.is_ok());
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("get_selection", json!({}))
                .unwrap();
            let ids = out.content["selected_ids"].as_array().unwrap();
            assert!(ids.is_empty(), "deselect_all should clear selection");
        }
    }

    #[test]
    fn mcp_hide_and_show_entity() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "Ghost"}))
                .unwrap();
        }
        app.update();
        app.update();

        let entity_id = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("Ghost"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        // default visible = true
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let list = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap();
            let entity = list.content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("Ghost"))
                .unwrap();
            assert_eq!(entity["visible"], true);
        }

        // hide
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("hide_entity", json!({"entity_id": entity_id}))
                .unwrap();
        }
        app.update();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let list = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap();
            let entity = list.content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("Ghost"))
                .unwrap();
            assert_eq!(entity["visible"], false, "entity should be hidden");
        }
    }

    #[test]
    fn mcp_set_parent_records_hierarchy() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "Parent"},
                        {"name": "Child"}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (parent_id, child_id) = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let list = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap();
            let entities = list.content["entities"].as_array().unwrap();
            let pid = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("Parent"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let cid = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("Child"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (pid, cid)
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let result = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "set_parent",
                    json!({"entity_id": child_id, "parent_id": parent_id}),
                )
                .unwrap();
            assert!(result.is_ok());
        }
        app.update();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let list = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap();
            let child = list.content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("Child"))
                .unwrap();
            assert_eq!(
                child["parent_id"].as_u64(),
                Some(parent_id),
                "child.parent_id should be {parent_id}"
            );
        }
    }

    #[test]
    fn mcp_get_components_returns_component_list() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        // spawn camera (has Name, Transform, Camera)
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "spawn_camera",
                    json!({"fov_y_degrees": 60.0, "position": [0.0, 0.0, 0.0]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let entity_id = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["camera_fov"].is_number())
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let result = mcp
                .0
                .lock()
                .unwrap()
                .execute("get_components", json!({"entity_id": entity_id}))
                .unwrap();
            assert!(result.is_ok());
            let comps: Vec<String> = result.content["components"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect();
            assert!(comps.contains(&"Transform".to_string()));
            assert!(comps.contains(&"Camera".to_string()));
        }
    }

    #[test]
    fn mcp_set_rotation_and_scale() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [{"name": "Cube", "position": [0.0, 0.0, 0.0]}]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let entity_id = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("Cube"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let r = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "set_rotation",
                    json!({"entity_id": entity_id, "rx": 0.0, "ry": 90.0, "rz": 0.0}),
                )
                .unwrap();
            assert!(r.is_ok());
            let s = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "set_scale",
                    json!({"entity_id": entity_id, "sx": 2.0, "sy": 3.0, "sz": 0.5}),
                )
                .unwrap();
            assert!(s.is_ok());
        }
        app.update();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let list = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap();
            let entity = list.content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("Cube"))
                .unwrap();
            let rot = entity["rotation"].as_array().unwrap();
            assert!(
                (rot[1].as_f64().unwrap() - 90.0).abs() < 0.5,
                "ry should be 90, got {}",
                rot[1]
            );
            let scale = entity["scale"].as_array().unwrap();
            assert!((scale[0].as_f64().unwrap() - 2.0).abs() < 1e-4);
            assert!((scale[1].as_f64().unwrap() - 3.0).abs() < 1e-4);
            assert!((scale[2].as_f64().unwrap() - 0.5).abs() < 1e-4);
        }
    }

    #[test]
    fn mcp_move_entity_applies_delta() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        // spawn with position [1, 0, 0]
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [{"name": "Mover", "position": [1.0, 0.0, 0.0]}]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let entity_id = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("Mover"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        // move by [3, 2, 1]
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let result = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "move_entity",
                    json!({"entity_id": entity_id, "dx": 3.0, "dy": 2.0, "dz": 1.0}),
                )
                .unwrap();
            assert!(result.is_ok());
        }
        app.update();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let list = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap();
            let entity = list.content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("Mover"))
                .unwrap();
            let pos = entity["position"].as_array().unwrap();
            assert!(
                (pos[0].as_f64().unwrap() - 4.0).abs() < 1e-4,
                "x should be 4"
            );
            assert!(
                (pos[1].as_f64().unwrap() - 2.0).abs() < 1e-4,
                "y should be 2"
            );
            assert!(
                (pos[2].as_f64().unwrap() - 1.0).abs() < 1e-4,
                "z should be 1"
            );
        }
    }

    #[test]
    fn mcp_duplicate_entity_creates_copy() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        // spawn an entity with name+position via batch_spawn
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [{"name": "Original", "position": [2.0, 0.0, 0.0]}]}),
                )
                .unwrap();
        }
        app.update(); // process_editor_commands spawns entity
        app.update(); // update_editor_snapshot captures it

        let entity_id = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let list = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap();
            list.content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("Original"))
                .expect("Original not in snapshot")["id"]
                .as_u64()
                .unwrap()
        };

        // duplicate
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let result = mcp
                .0
                .lock()
                .unwrap()
                .execute("duplicate_entity", json!({"entity_id": entity_id}))
                .unwrap();
            assert!(result.is_ok());
        }
        app.update();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let list = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap();
            let entities = list.content["entities"].as_array().unwrap();
            assert_eq!(entities.len(), 2, "expected 2 entities after duplicate");
            let copy = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("Original (copy)"))
                .expect("copy entity not found");
            let pos = copy["position"].as_array().unwrap();
            assert!((pos[0].as_f64().unwrap() - 2.0).abs() < 1e-4);
        }
    }

    #[test]
    fn update_editor_snapshot_and_populate_inspector_report_is_prefab_instance() {
        let mut app = new_app();
        app.add_plugins(EditorPlugin);

        let with_instance = app
            .world_mut()
            .spawn((
                Name("HasInstance".to_string()),
                bsengine_core::PrefabInstance {
                    source_path: "assets/prefabs/x.ron".to_string(),
                },
            ))
            .id();
        let without_instance = app
            .world_mut()
            .spawn(Name("NoInstance".to_string()))
            .id();

        app.update(); // update_editor_snapshot + populate_inspector capture both

        let inspector = app.world().resource::<InspectorState>();
        let with_info = inspector
            .entities
            .iter()
            .find(|e| e.id == with_instance.index() as u64)
            .unwrap();
        let without_info = inspector
            .entities
            .iter()
            .find(|e| e.id == without_instance.index() as u64)
            .unwrap();
        assert!(
            with_info.is_prefab_instance,
            "entity with PrefabInstance must report true"
        );
        assert!(
            !without_info.is_prefab_instance,
            "entity without PrefabInstance must report false"
        );
    }

    #[test]
    fn mcp_spawn_camera_creates_camera_entity() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let result = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "spawn_camera",
                    json!({"fov_y_degrees": 75.0, "position": [0.0, 5.0, 10.0]}),
                )
                .expect("spawn_camera not found");
            assert!(result.is_ok());
        }
        app.update(); // process_editor_commands spawns Camera
        app.update(); // update_editor_snapshot captures it

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let list = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap();
            let entities = list.content["entities"].as_array().unwrap();
            let cam_entity = entities
                .iter()
                .find(|e| e["camera_fov"].is_number())
                .expect("no camera entity in snapshot");
            let fov = cam_entity["camera_fov"].as_f64().unwrap();
            assert!((fov - 75.0).abs() < 0.5, "expected 75 fov, got {fov}");
            let pos = cam_entity["position"].as_array().unwrap();
            assert!((pos[1].as_f64().unwrap() - 5.0).abs() < 1e-4);
        }
    }

    #[test]
    fn mcp_update_camera_changes_fov() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_camera", json!({"fov_y_degrees": 60.0}))
                .unwrap();
        }
        app.update();
        app.update();

        let entity_id = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let list = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap();
            list.content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["camera_fov"].is_number())
                .expect("no camera")["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let result = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "update_camera",
                    json!({"entity_id": entity_id, "fov_y_degrees": 90.0}),
                )
                .unwrap();
            assert!(result.is_ok());
        }
        app.update();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let list = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap();
            let fov = list.content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["camera_fov"].is_number())
                .unwrap()["camera_fov"]
                .as_f64()
                .unwrap();
            assert!((fov - 90.0).abs() < 0.5, "expected 90 fov, got {fov}");
        }
    }

    #[test]
    fn mcp_batch_spawn_creates_multiple_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let result = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({
                        "entities": [
                            {"name": "Alpha", "position": [1.0, 0.0, 0.0]},
                            {"name": "Beta"},
                            {"name": "Gamma", "position": [3.0, 0.0, 0.0]}
                        ]
                    }),
                )
                .expect("batch_spawn not found");
            assert!(result.is_ok());
            assert_eq!(result.content["count"], 3);
        }
        app.update(); // process_editor_commands spawns entities
        app.update(); // update_editor_snapshot captures them

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let list = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap();
            let names: Vec<_> = list.content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|e| e["name"].as_str())
                .collect();
            assert!(names.contains(&"Alpha"), "Alpha missing: {:?}", names);
            assert!(names.contains(&"Beta"), "Beta missing: {:?}", names);
            assert!(names.contains(&"Gamma"), "Gamma missing: {:?}", names);
            let alpha = list.content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"].as_str() == Some("Alpha"))
                .unwrap();
            let pos = alpha["position"].as_array().unwrap();
            assert!((pos[0].as_f64().unwrap() - 1.0).abs() < 1e-4);
        }
    }

    #[test]
    fn mcp_clear_scene_removes_all_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.world_mut().spawn(bsengine_scene::Name("A".to_string()));
        app.world_mut().spawn(bsengine_scene::Name("B".to_string()));
        app.world_mut().spawn(bsengine_core::PointLight::default());
        app.update(); // snapshot: 3 entities

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let result = mcp
                .0
                .lock()
                .unwrap()
                .execute("clear_scene", json!({}))
                .expect("clear_scene not found");
            assert!(result.is_ok());
        }
        app.update(); // process: all despawned
        app.update(); // snapshot: empty

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let stats = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_scene_stats", json!({}))
            .unwrap();
        assert_eq!(stats.content["total_entities"], 0);
    }

    #[test]
    fn list_entities_includes_light_props_for_point_light() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.world_mut().spawn(bsengine_core::PointLight {
            color: glam::Vec3::new(0.5, 0.5, 0.5).into(),
            intensity: 3.5,
            range: 12.0,
        });
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap();
        let e = &result.content["entities"].as_array().unwrap()[0];
        assert!((e["light_intensity"].as_f64().unwrap() - 3.5).abs() < 1e-3);
        assert!((e["light_range"].as_f64().unwrap() - 12.0).abs() < 1e-3);
        let color = e["light_color"].as_array().unwrap();
        assert!((color[0].as_f64().unwrap() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn get_entity_includes_light_props_for_directional_light() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app
            .world_mut()
            .spawn(bsengine_core::DirectionalLight {
                color: glam::Vec3::new(0.8, 0.8, 0.8).into(),
                ..Default::default()
            })
            .id();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": eid.index() as u64}))
            .unwrap();
        let color = result.content["entity"]["light_color"].as_array().unwrap();
        assert!((color[0].as_f64().unwrap() - 0.8).abs() < 1e-3);
        assert!(
            result.content["entity"]["light_intensity"].is_null(),
            "directional has no intensity"
        );
        assert!(
            result.content["entity"]["light_range"].is_null(),
            "directional has no range"
        );
    }

    #[test]
    fn mcp_spawn_spot_light_creates_entity() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let result = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "spawn_spot_light",
                    json!({
                        "color": [1.0, 1.0, 1.0],
                        "intensity": 3.0,
                        "range": 15.0,
                        "inner_angle": 0.3,
                        "outer_angle": 0.6,
                        "position": [0.0, 5.0, 0.0]
                    }),
                )
                .expect("spawn_spot_light not found");
            assert!(result.is_ok(), "{:?}", result.error);
        }
        app.update();

        let mut q = app.world_mut().query::<&bsengine_core::SpotLight>();
        let lights: Vec<_> = q.iter(app.world()).collect();
        assert_eq!(lights.len(), 1);
        assert!((lights[0].intensity - 3.0).abs() < 1e-4);
        assert!((lights[0].inner_angle_degrees.0 - 0.3_f32.to_degrees()).abs() < 1e-3);
    }

    #[test]
    fn list_entities_spot_light_has_light_type_spot() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.world_mut().spawn(bsengine_core::SpotLight::default());
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap();
        let entities = result.content["entities"].as_array().unwrap();
        let light_types: Vec<_> = entities
            .iter()
            .filter_map(|e| e["light_type"].as_str())
            .collect();
        assert!(light_types.contains(&"spot"), "expected light_type=spot");
    }

    #[test]
    fn mcp_query_entities_filters_by_has_mesh() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.world_mut()
            .spawn(bsengine_render::MeshRenderer { mesh_id: 10 });
        app.world_mut().spawn(bsengine_core::PointLight::default());
        app.world_mut()
            .spawn(bsengine_scene::Name("NoMesh".to_string()));
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute("query_entities", json!({"has_mesh": true}))
            .expect("query_entities not found");
        assert!(result.is_ok());
        let entities = result.content["entities"].as_array().unwrap();
        assert_eq!(entities.len(), 1, "only 1 entity has mesh");
        assert_eq!(entities[0]["mesh_id"], 10);
    }

    /// The MCP path end to end: a condition query through the registry
    /// selects in the editor's real selection, which the next snapshot
    /// reports; and a malformed query comes back as a tool error rather than
    /// as "no matches". The filter logic itself is covered in
    /// `entity_query`'s own tests -- this is the wiring.
    #[test]
    fn mcp_query_entities_selects_through_the_registry_and_reports_errors() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let prop = app
            .world_mut()
            .spawn((
                bsengine_scene::Name("Crate".to_string()),
                crate::snapshot::Tags(vec!["prop".to_string()]),
            ))
            .id()
            .index() as u64;
        let other = app
            .world_mut()
            .spawn((
                bsengine_scene::Name("Wall".to_string()),
                crate::snapshot::Tags(vec!["static".to_string()]),
            ))
            .id()
            .index() as u64;
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            let out = m
                .execute(
                    "query_entities",
                    json!({"where": [{"field": "tags", "op": "contains", "value": "prop"}],
                           "action": "select"}),
                )
                .expect("query_entities not found");
            assert!(out.is_ok(), "{:?}", out.error);
            assert_eq!(out.content["added_count"], 1);

            let bad = m
                .execute(
                    "query_entities",
                    json!({"where": [{"field": "tagz", "op": "contains", "value": "prop"}]}),
                )
                .expect("query_entities not found");
            assert!(!bad.is_ok(), "a typo'd field must fail, not match nothing");
            assert!(
                bad.error.as_deref().unwrap_or("").contains("unknown field `tagz`"),
                "{:?}",
                bad.error
            );
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let sel = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_selected_entities", json!({}))
            .unwrap();
        let ids: Vec<u64> = sel.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_u64().unwrap())
            .collect();
        assert!(ids.contains(&prop), "the tagged entity is selected: {ids:?}");
        assert!(!ids.contains(&other), "premise: the other one is not: {ids:?}");
    }

    #[test]
    fn mcp_query_entities_filters_by_light_type() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.world_mut().spawn(bsengine_core::PointLight::default());
        app.world_mut()
            .spawn(bsengine_core::DirectionalLight::default());
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute("query_entities", json!({"light_type": "point"}))
            .expect("query_entities not found");
        assert!(result.is_ok());
        let entities = result.content["entities"].as_array().unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0]["light_type"], "point");
    }

    #[test]
    fn mcp_get_scene_stats_returns_counts() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.world_mut().spawn(bsengine_scene::Name("A".to_string()));
        app.world_mut().spawn(bsengine_core::PointLight::default());
        app.world_mut()
            .spawn(bsengine_render::MeshRenderer { mesh_id: 1 });
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_scene_stats", json!({}))
            .expect("get_scene_stats not found");
        assert!(result.is_ok(), "{:?}", result.error);
        assert_eq!(result.content["total_entities"], 3);
        assert_eq!(result.content["light_count"], 1);
        assert_eq!(result.content["mesh_count"], 1);
        assert_eq!(result.content["named_count"], 1);
    }

    #[test]
    fn mcp_rename_entity_changes_name() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        let eid = app
            .world_mut()
            .spawn(bsengine_scene::Name("OldName".to_string()))
            .id();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "rename_entity",
                    json!({"entity_id": eid.index() as u64, "name": "NewName"}),
                )
                .expect("rename_entity not found");
        }
        app.update(); // process_editor_commands renames
        app.update(); // update_editor_snapshot captures new name

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let list = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap();
            let names: Vec<_> = list.content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|e| e["name"].as_str())
                .collect();
            assert!(names.contains(&"NewName"), "NewName not found: {:?}", names);
            assert!(!names.contains(&"OldName"), "OldName still present");
        }
    }

    #[test]
    fn mcp_update_point_light_changes_intensity() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app
            .world_mut()
            .spawn(bsengine_core::PointLight::default())
            .id();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let result = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "update_point_light",
                    json!({"entity_id": eid.index() as u64, "intensity": 5.0, "range": 20.0}),
                )
                .expect("update_point_light not found");
            assert!(result.is_ok(), "{:?}", result.error);
        }
        app.update();

        let light = app
            .world_mut()
            .query::<&bsengine_core::PointLight>()
            .iter(app.world())
            .next()
            .unwrap();
        assert!(
            (light.intensity - 5.0).abs() < 1e-4,
            "intensity not updated"
        );
        assert!((light.range - 20.0).abs() < 1e-4, "range not updated");
    }

    #[test]
    fn mcp_update_directional_light_changes_color() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app
            .world_mut()
            .spawn(bsengine_core::DirectionalLight::default())
            .id();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let result = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "update_directional_light",
                    json!({"entity_id": eid.index() as u64, "color": [0.5, 0.5, 0.5]}),
                )
                .expect("update_directional_light not found");
            assert!(result.is_ok(), "{:?}", result.error);
        }
        app.update();

        let light = app
            .world_mut()
            .query::<&bsengine_core::DirectionalLight>()
            .iter(app.world())
            .next()
            .unwrap();
        assert!((light.color.x - 0.5).abs() < 1e-4, "color.r not updated");
    }

    #[test]
    fn list_entities_includes_light_type_point() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.world_mut().spawn(bsengine_core::PointLight::default());
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap();
        let entities = result.content["entities"].as_array().unwrap();
        let light_types: Vec<_> = entities
            .iter()
            .filter_map(|e| e["light_type"].as_str())
            .collect();
        assert!(light_types.contains(&"point"), "expected light_type=point");
    }

    #[test]
    fn list_entities_no_light_type_for_plain_entity() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.world_mut()
            .spawn(bsengine_scene::Name("Cube".to_string()));
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap();
        let entities = result.content["entities"].as_array().unwrap();
        let cube = entities
            .iter()
            .find(|e| e["name"].as_str() == Some("Cube"))
            .unwrap();
        assert!(
            cube["light_type"].is_null(),
            "plain entity should have null light_type"
        );
    }

    #[test]
    fn get_entity_includes_light_type_directional() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app
            .world_mut()
            .spawn(bsengine_core::DirectionalLight::default())
            .id();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": eid.index() as u64}))
            .unwrap();
        assert_eq!(result.content["entity"]["light_type"], "directional");
    }

    #[test]
    fn list_entities_includes_mesh_id() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.world_mut().spawn((
            Name("Renderable".to_string()),
            Transform::from_position(Vec3::ZERO),
            bsengine_render::MeshRenderer { mesh_id: 99 },
            bsengine_core::GlobalTransform::default(),
        ));
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .expect("list_entities not found");
        assert!(result.is_ok());
        let entities = result.content["entities"].as_array().unwrap();
        let entity = entities
            .iter()
            .find(|e| e["name"] == "Renderable")
            .expect("Renderable not found");
        assert_eq!(entity["mesh_id"], 99);
    }

    #[test]
    fn get_entity_includes_mesh_id_when_present() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app
            .world_mut()
            .spawn((
                Name("WithMesh".to_string()),
                Transform::from_position(Vec3::ZERO),
                bsengine_render::MeshRenderer { mesh_id: 55 },
                bsengine_core::GlobalTransform::default(),
            ))
            .id();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": eid.index() as u64}))
            .expect("get_entity not found");
        assert!(result.is_ok());
        assert_eq!(result.content["entity"]["mesh_id"], 55);
    }

    #[test]
    fn mcp_detach_mesh_removes_mesh_renderer() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app
            .world_mut()
            .spawn((
                Name("Sphere".to_string()),
                Transform::from_position(Vec3::ZERO),
                bsengine_render::MeshRenderer { mesh_id: 7 },
                bsengine_core::GlobalTransform::default(),
            ))
            .id();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("detach_mesh", json!({"entity_id": eid.index() as u64}))
                .expect("detach_mesh not found");
        }
        app.update();

        let mut q = app.world_mut().query::<&bsengine_render::MeshRenderer>();
        assert!(
            q.iter(app.world()).next().is_none(),
            "MeshRenderer still present after detach"
        );
    }

    #[test]
    fn mcp_detach_physics_body_removes_physics_body_desc() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app
            .world_mut()
            .spawn((
                Name("Box".to_string()),
                Transform::from_position(Vec3::ZERO),
                bsengine_scene::PhysicsBodyDesc {
                    rigidbody: bsengine_scene::RigidBodyDesc::Static,
                    collider: bsengine_scene::ColliderDesc {
                        shape: bsengine_scene::ColliderShapeDesc::Box { hx: 1.0, hy: 1.0, hz: 1.0 },
                        restitution: 0.0,
                        friction: 0.5,
                        sensor: false,
                    },
                    linear_damping: None,
                    angular_damping: None,
                },
            ))
            .id();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("detach_physics_body", json!({"entity_id": eid.index() as u64}))
                .expect("detach_physics_body not found");
        }
        app.update();

        assert!(app.world().get::<bsengine_scene::PhysicsBodyDesc>(eid).is_none());
    }

    #[test]
    fn mcp_save_scene_writes_ron_file() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.world_mut().spawn((
            Name("Castle".to_string()),
            Transform::from_position(Vec3::new(5.0, 0.0, 0.0)),
        ));
        app.update();

        let path = std::env::temp_dir()
            .join("bsengine_test_save.ron")
            .to_string_lossy()
            .to_string();
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute("save_scene", json!({"path": path}))
            .expect("save_scene not found");
        assert!(result.is_ok(), "save error: {:?}", result.error);
        assert_eq!(result.content["status"], "saved");
        assert_eq!(result.content["entity_count"], 1);
        assert!(std::path::Path::new(&path).exists());
    }

    #[test]
    fn mcp_save_load_scene_round_trip() {
        let path = std::env::temp_dir()
            .join("bsengine_test_roundtrip.ron")
            .to_string_lossy()
            .to_string();

        // Save
        {
            let mut app = new_app();
            app.add_plugins(McpPlugin);
            app.add_plugins(EditorPlugin);
            app.world_mut().spawn((
                Name("Tower".to_string()),
                Transform::from_position(Vec3::new(3.0, 1.0, 0.0)),
            ));
            app.update();
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let r = mcp
                .0
                .lock()
                .unwrap()
                .execute("save_scene", json!({"path": path}))
                .unwrap();
            assert!(r.is_ok(), "{:?}", r.error);
        }

        // Load in new app
        {
            let mut app = new_app();
            app.add_plugins(McpPlugin);
            app.add_plugins(EditorPlugin);
            {
                let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
                mcp.0
                    .lock()
                    .unwrap()
                    .execute("load_scene", json!({"path": path}))
                    .expect("load_scene not found");
            }
            app.update();

            let mut q = app.world_mut().query::<(&Name, &Transform)>();
            let results: Vec<_> = q
                .iter(app.world())
                .map(|(n, t)| (n.0.as_str(), t.position))
                .collect();
            let found = results
                .iter()
                .find(|(name, _)| *name == "Tower")
                .expect("Tower not found after load");
            assert!((found.1.x - 3.0).abs() < 1e-4, "wrong x: {}", found.1.x);
        }
    }

    #[test]
    fn mcp_save_load_scene_round_trip_preserves_reflected_component_attached_via_set_reflected_component()
     {
        let path = std::env::temp_dir()
            .join("bsengine_test_roundtrip_reflected_component.ron")
            .to_string_lossy()
            .to_string();

        // Save: an entity with a NavMeshAgent attached only via
        // set_reflected_component -- no dedicated EntityInfo field carries
        // this data, so it can only survive save/load through
        // EntityInfo.extra_components/EntityDescriptor.components.
        {
            let mut app = new_app();
            app.add_plugins(McpPlugin);
            app.add_plugins(EditorPlugin);
            let eid = app.world_mut().spawn(Name("Enemy".to_string())).id();
            app.update();

            {
                let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
                let registry = mcp.0.lock().unwrap();
                let out = registry
                    .execute(
                        "set_reflected_component",
                        json!({
                            "entity_id": eid.index() as u64,
                            "type_path": "bsengine_core::nav_mesh_agent::NavMeshAgent",
                            "value_json": r#"{"destination": null, "speed": 3.5, "angular_speed": 2.0, "acceleration": 8.0, "stopping_distance": 0.1, "radius": 0.3, "height": 1.8, "state": "Idle", "enabled": true}"#,
                        }),
                    )
                    .expect("tool should be registered");
                assert!(out.is_ok(), "{:?}", out.error);
            }
            // First update() drains the ReflectCommand queue and attaches
            // NavMeshAgent to the world (process_reflect_commands runs last
            // in the frame, after this same frame's snapshot capture). A
            // second update() is needed so populate_snapshot_extra_components
            // captures it into the snapshot save_scene reads from.
            app.update();
            app.update();

            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let r = mcp
                .0
                .lock()
                .unwrap()
                .execute("save_scene", json!({"path": path}))
                .unwrap();
            assert!(r.is_ok(), "{:?}", r.error);
        }

        // Load in a new app; the NavMeshAgent should reappear without any
        // further set_reflected_component call.
        {
            let mut app = new_app();
            app.add_plugins(McpPlugin);
            app.add_plugins(EditorPlugin);
            {
                let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
                mcp.0
                    .lock()
                    .unwrap()
                    .execute("load_scene", json!({"path": path}))
                    .expect("load_scene not found");
            }
            app.update();

            let mut q = app
                .world_mut()
                .query::<(&Name, &bsengine_core::NavMeshAgent)>();
            let results: Vec<_> = q.iter(app.world()).collect();
            assert_eq!(results.len(), 1, "NavMeshAgent missing after load");
            assert_eq!(results[0].0 .0, "Enemy");
            assert!((results[0].1.speed - 3.5).abs() < 1e-4);
            assert!(results[0].1.enabled);
        }
    }

    #[test]
    fn mcp_save_load_scene_round_trip_preserves_physics_body_attached_via_mcp() {
        let path = std::env::temp_dir()
            .join("bsengine_test_roundtrip_physics_body.ron")
            .to_string_lossy()
            .to_string();

        {
            let mut app = new_app();
            app.add_plugins(McpPlugin);
            app.add_plugins(EditorPlugin);
            let eid = app
                .world_mut()
                .spawn((
                    Name("Crate".to_string()),
                    Transform::from_position(Vec3::new(2.0, 0.0, 0.0)),
                ))
                .id();
            app.update();

            {
                let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
                let out = mcp
                    .0
                    .lock()
                    .unwrap()
                    .execute(
                        "attach_physics_body",
                        json!({
                            "entity_id": eid.index() as u64,
                            "rigidbody": "Static",
                            "collider_shape": "Box",
                            "hx": 1.0, "hy": 1.0, "hz": 1.0,
                        }),
                    )
                    .expect("attach_physics_body not found");
                assert!(out.is_ok(), "{:?}", out.error);
            }
            app.update();
            app.update();

            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let r = mcp
                .0
                .lock()
                .unwrap()
                .execute("save_scene", json!({"path": path}))
                .unwrap();
            assert!(r.is_ok(), "{:?}", r.error);
        }

        {
            let mut app = new_app();
            app.add_plugins(McpPlugin);
            app.add_plugins(EditorPlugin);
            {
                let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
                mcp.0
                    .lock()
                    .unwrap()
                    .execute("load_scene", json!({"path": path}))
                    .expect("load_scene not found");
            }
            app.update();

            let mut q = app
                .world_mut()
                .query::<(&Name, &bsengine_scene::PhysicsBodyDesc)>();
            let results: Vec<_> = q.iter(app.world()).collect();
            assert_eq!(results.len(), 1, "PhysicsBodyDesc missing after load");
            assert_eq!(results[0].0 .0, "Crate");
            assert_eq!(results[0].1.rigidbody, bsengine_scene::RigidBodyDesc::Static);
        }
    }

    #[test]
    fn mcp_save_load_scene_round_trip_preserves_animation_state_machine() {
        // AnimationStateMachine::triggers is a HashSet<String>; the generic
        // save-side serializer (TypedReflectSerializer) needs ReflectSerialize
        // registered for HashSet<String> or it fails ("did not register
        // ReflectSerialize") and silently drops the whole component from the
        // saved scene. Regression test for that specific failure mode.
        let path = std::env::temp_dir()
            .join("bsengine_test_roundtrip_animation_state_machine.ron")
            .to_string_lossy()
            .to_string();

        let value_json = r#"{
            "states": {
                "idle": {"clip": "idle_clip", "blend": null, "looping": true, "speed": 1.0, "duration": 1.0},
                "walk": {"clip": "walk_clip", "blend": null, "looping": true, "speed": 1.0, "duration": 1.0}
            },
            "transitions": [
                {"from": "idle", "to": "walk", "condition": {"FloatGreater": {"param": "speed", "threshold": 0.1}}, "blend_duration": 0.2}
            ],
            "current_state": "idle",
            "params_float": {"speed": 0.0},
            "params_bool": {},
            "triggers": [],
            "blend_from": null,
            "blend_weight": 1.0,
            "blend_duration": 0.0,
            "blend_elapsed": 0.0
        }"#;

        {
            let mut app = new_app();
            app.add_plugins(McpPlugin);
            app.add_plugins(EditorPlugin);
            let eid = app.world_mut().spawn(Name("Player".to_string())).id();
            app.update();

            {
                let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
                let registry = mcp.0.lock().unwrap();
                let out = registry
                    .execute(
                        "set_reflected_component",
                        json!({
                            "entity_id": eid.index() as u64,
                            "type_path": "bsengine_core::animation_state_machine::AnimationStateMachine",
                            "value_json": value_json,
                        }),
                    )
                    .expect("tool should be registered");
                assert!(out.is_ok(), "{:?}", out.error);
            }
            app.update();
            app.update();

            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let r = mcp
                .0
                .lock()
                .unwrap()
                .execute("save_scene", json!({"path": path}))
                .unwrap();
            assert!(r.is_ok(), "{:?}", r.error);
        }

        {
            let mut app = new_app();
            app.add_plugins(McpPlugin);
            app.add_plugins(EditorPlugin);
            {
                let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
                mcp.0
                    .lock()
                    .unwrap()
                    .execute("load_scene", json!({"path": path}))
                    .expect("load_scene not found");
            }
            app.update();

            let mut q = app
                .world_mut()
                .query::<(&Name, &bsengine_core::AnimationStateMachine)>();
            let results: Vec<_> = q.iter(app.world()).collect();
            assert_eq!(results.len(), 1, "AnimationStateMachine missing after load");
            assert_eq!(results[0].0 .0, "Player");
            assert_eq!(results[0].1.current_state, "idle");
            assert!(results[0].1.states.contains_key("walk"));
            assert_eq!(results[0].1.transitions.len(), 1);
        }
    }

    #[test]
    fn mcp_save_load_scene_round_trip_preserves_camera_primitive_script_and_lights() {
        let path = std::env::temp_dir()
            .join("bsengine_test_roundtrip_full.ron")
            .to_string_lossy()
            .to_string();

        // Save: a camera, a primitive+scripted cube, a point light, and a spot light.
        {
            let mut app = new_app();
            app.add_plugins(McpPlugin);
            app.add_plugins(EditorPlugin);
            app.world_mut().spawn((
                Name("MainCam".to_string()),
                bsengine_core::Camera::perspective(75.0, 16.0 / 9.0),
                Transform::from_position(Vec3::new(0.0, 2.0, 5.0)),
                bsengine_core::GlobalTransform::default(),
            ));
            app.world_mut().spawn((
                Name("Crate".to_string()),
                bsengine_scene::PrimitiveMesh(bsengine_scene::Primitive::Cube),
                bsengine_scene::ScriptPath("assets/scripts/crate.js".to_string()),
                Transform::from_position(Vec3::new(1.0, 0.0, 0.0)),
                bsengine_core::GlobalTransform::default(),
            ));
            app.world_mut().spawn((
                Name("Lamp".to_string()),
                bsengine_core::PointLight {
                    color: Vec3::new(1.0, 0.5, 0.2).into(),
                    intensity: 2.5,
                    range: 15.0,
                },
                Transform::from_position(Vec3::new(0.0, 3.0, 0.0)),
                bsengine_core::GlobalTransform::default(),
            ));
            app.world_mut().spawn((
                Name("Spot".to_string()),
                bsengine_core::SpotLight {
                    color: Vec3::new(0.2, 0.8, 1.0).into(),
                    intensity: 3.0,
                    range: 20.0,
                    inner_angle_degrees: 15.0.into(),
                    outer_angle_degrees: 25.0.into(),
                },
                Transform::from_position(Vec3::new(2.0, 4.0, 0.0)),
                bsengine_core::GlobalTransform::default(),
            ));
            app.update();

            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let r = mcp
                .0
                .lock()
                .unwrap()
                .execute("save_scene", json!({"path": path}))
                .unwrap();
            assert!(r.is_ok(), "{:?}", r.error);
            assert_eq!(r.content["entity_count"], 4);
        }

        // Load in a fresh app and verify every component round-tripped.
        {
            let mut app = new_app();
            app.add_plugins(McpPlugin);
            app.add_plugins(EditorPlugin);
            {
                let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
                mcp.0
                    .lock()
                    .unwrap()
                    .execute("load_scene", json!({"path": path}))
                    .expect("load_scene not found");
            }
            app.update();

            let mut cam_q = app
                .world_mut()
                .query::<(&Name, &bsengine_core::Camera)>();
            let (_, cam) = cam_q
                .iter(app.world())
                .find(|(n, _)| n.0 == "MainCam")
                .expect("MainCam not found after load");
            assert!(
                (cam.fov_y_degrees.0 - 75.0).abs() < 1e-3,
                "wrong fov: {}",
                cam.fov_y_degrees.0
            );

            let mut prim_q = app.world_mut().query::<(
                &Name,
                &bsengine_scene::PrimitiveMesh,
                &bsengine_scene::ScriptPath,
            )>();
            let (_, prim, script) = prim_q
                .iter(app.world())
                .find(|(n, _, _)| n.0 == "Crate")
                .expect("Crate not found after load");
            assert_eq!(prim.0, bsengine_scene::Primitive::Cube);
            assert_eq!(script.0, "assets/scripts/crate.js");

            let mut pl_q = app
                .world_mut()
                .query::<(&Name, &bsengine_core::PointLight)>();
            let (_, pl) = pl_q
                .iter(app.world())
                .find(|(n, _)| n.0 == "Lamp")
                .expect("Lamp not found after load");
            assert!((pl.intensity - 2.5).abs() < 1e-4);
            assert!((pl.range - 15.0).abs() < 1e-4);
            assert!((pl.color.x - 1.0).abs() < 1e-4);
            assert!((pl.color.y - 0.5).abs() < 1e-4);

            let mut sl_q = app
                .world_mut()
                .query::<(&Name, &bsengine_core::SpotLight)>();
            let (_, sl) = sl_q
                .iter(app.world())
                .find(|(n, _)| n.0 == "Spot")
                .expect("Spot not found after load");
            assert!((sl.intensity - 3.0).abs() < 1e-4);
            assert!((sl.range - 20.0).abs() < 1e-4);
            assert!((sl.inner_angle_degrees.0 - 15.0).abs() < 1e-2);
            assert!((sl.outer_angle_degrees.0 - 25.0).abs() < 1e-2);
        }
    }

    #[test]
    fn mcp_set_transform_moves_entity() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app
            .world_mut()
            .spawn((
                Name("Crate".to_string()),
                Transform::from_position(Vec3::ZERO),
            ))
            .id();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "set_transform",
                    json!({"id": eid.index() as u64, "x": 10.0, "y": 0.0, "z": 0.0}),
                )
                .expect("set_transform not found");
        }
        app.update();

        let snapshot = app
            .world()
            .resource::<EditorSnapshotResource>()
            .0
            .lock()
            .unwrap();
        let crate_entity = snapshot
            .entities
            .iter()
            .find(|e| e.name.as_deref() == Some("Crate"))
            .expect("Crate not found");
        let pos = crate_entity.position.expect("no position");
        assert!((pos[0] - 10.0).abs() < 1e-4, "expected x=10 got {}", pos[0]);
    }

    #[test]
    fn inspector_cmd_rename_entity_renames_via_existing_pipeline() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app.world_mut().spawn(Name("Old Name".to_string())).id();
        app.update();

        {
            let mut insp = app.world_mut().resource_mut::<InspectorState>();
            insp.cmd_queue.push(InspectorCmd::RenameEntity {
                id: eid.index() as u64,
                name: "New Name".to_string(),
            });
        }
        app.update();

        let name = app.world().get::<Name>(eid).expect("Name should exist");
        assert_eq!(name.0, "New Name");
    }

    #[test]
    fn inspector_cmd_set_and_remove_parent_reparents_via_existing_pipeline() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let parent = app.world_mut().spawn(Name("Parent".to_string())).id();
        let child = app.world_mut().spawn(Name("Child".to_string())).id();
        app.update();

        {
            let mut insp = app.world_mut().resource_mut::<InspectorState>();
            insp.cmd_queue.push(InspectorCmd::SetParent {
                id: child.index() as u64,
                parent_id: parent.index() as u64,
            });
        }
        app.update();
        assert_eq!(
            app.world().get::<Parent>(child).map(|p| p.0),
            Some(parent),
            "child should be parented after SetParent"
        );

        {
            let mut insp = app.world_mut().resource_mut::<InspectorState>();
            insp.cmd_queue
                .push(InspectorCmd::RemoveParent { id: child.index() as u64 });
        }
        app.update();
        assert!(
            app.world().get::<Parent>(child).is_none(),
            "child should be unparented after RemoveParent"
        );
    }

    #[test]
    fn inspector_cmd_tag_and_untag_entity_via_existing_pipeline() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app.world_mut().spawn(Name("Target".to_string())).id();
        app.update();

        {
            let mut insp = app.world_mut().resource_mut::<InspectorState>();
            insp.cmd_queue.push(InspectorCmd::TagEntity {
                id: eid.index() as u64,
                tag: "enemy".to_string(),
            });
        }
        app.update();
        assert_eq!(
            app.world().get::<Tags>(eid).map(|t| t.0.clone()),
            Some(vec!["enemy".to_string()]),
        );

        {
            let mut insp = app.world_mut().resource_mut::<InspectorState>();
            insp.cmd_queue.push(InspectorCmd::UntagEntity {
                id: eid.index() as u64,
                tag: "enemy".to_string(),
            });
        }
        app.update();
        assert_eq!(
            app.world().get::<Tags>(eid).map(|t| t.0.clone()),
            Some(vec![]),
        );
    }

    #[test]
    fn inspector_cmd_set_visible_writes_the_component_the_renderer_actually_reads() {
        // Regression test: `EditorCommand::SetVisible` used to insert a
        // separate, unreflected `bsengine_editor::snapshot::Visible(bool)`
        // that bsengine-render's culling and bsengine-scripting's
        // `Bsengine.setVisible`/`getVisible` never read (they only look at
        // `bsengine_core::Visible.is_visible`) -- so toggling "Visible" in
        // the Inspector had no effect on what was actually rendered.
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app.world_mut().spawn(Name("Target".to_string())).id();
        app.update();

        {
            let mut insp = app.world_mut().resource_mut::<InspectorState>();
            insp.cmd_queue.push(InspectorCmd::SetVisible {
                id: eid.index() as u64,
                visible: false,
            });
        }
        app.update();
        assert_eq!(
            app.world().get::<bsengine_core::Visible>(eid),
            Some(&bsengine_core::Visible { is_visible: false }),
            "SetVisible must write bsengine_core::Visible, the type bsengine-render and \
             bsengine-scripting actually read -- not an editor-only phantom component"
        );
    }

    #[test]
    fn editor_command_attach_and_detach_script() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app.world_mut().spawn(Name("Target".to_string())).id();
        app.update();

        {
            let queue = app.world().resource::<EditorCommandQueueResource>();
            queue.0.lock().unwrap().push(EditorCommand::AttachScript {
                entity_id: eid.index() as u64,
                path: "assets/scripts/foo.js".to_string(),
            });
        }
        app.update();
        assert_eq!(
            app.world()
                .get::<bsengine_scene::ScriptPath>(eid)
                .map(|s| s.0.clone()),
            Some("assets/scripts/foo.js".to_string())
        );

        {
            let queue = app.world().resource::<EditorCommandQueueResource>();
            queue
                .0
                .lock()
                .unwrap()
                .push(EditorCommand::DetachScript { entity_id: eid.index() as u64 });
        }
        app.update();
        assert!(app.world().get::<bsengine_scene::ScriptPath>(eid).is_none());
    }

    #[test]
    fn editor_command_attach_and_detach_primitive_mesh() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app.world_mut().spawn(Name("Target".to_string())).id();
        app.update();

        {
            let queue = app.world().resource::<EditorCommandQueueResource>();
            queue
                .0
                .lock()
                .unwrap()
                .push(EditorCommand::AttachPrimitiveMesh {
                    entity_id: eid.index() as u64,
                    primitive: bsengine_scene::Primitive::Capsule,
                });
        }
        app.update();
        assert_eq!(
            app.world()
                .get::<bsengine_scene::PrimitiveMesh>(eid)
                .map(|p| p.0.clone()),
            Some(bsengine_scene::Primitive::Capsule)
        );

        {
            let queue = app.world().resource::<EditorCommandQueueResource>();
            queue.0.lock().unwrap().push(EditorCommand::DetachPrimitiveMesh {
                entity_id: eid.index() as u64,
            });
        }
        app.update();
        assert!(
            app.world()
                .get::<bsengine_scene::PrimitiveMesh>(eid)
                .is_none()
        );
    }

    // Regression test for the Unity/Unreal-style "pressing Play resets the
    // scene" behavior: the toolbar's Play button (bsengine-rhi-wgpu) pushes
    // InspectorCmd::ReloadScene when transitioning Stopped -> Playing, and
    // apply_inspector_cmds must turn that into a PendingSceneLoad so the
    // existing handle_scene_load system (which properly despawns and
    // respawns from the RON file) can pick it up. This can't exercise the
    // full despawn/respawn round trip headlessly here — that system lives
    // in bsengine-runtime, and bsengine-runtime can't combine EditorPlugin
    // with ScriptingPlugin in one process (known V8 handle-scope conflict)
    // — but it does prove the wiring this crate owns: ReloadScene ->
    // PendingSceneLoad with the right path, catching a regression like the
    // match arm being dropped or the path being wrong.
    #[test]
    fn inspector_cmd_reload_scene_inserts_pending_scene_load() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.update();

        {
            let mut inspector = app.world_mut().resource_mut::<InspectorState>();
            inspector.current_scene_path = Some("assets/scenes/level1.ron".to_string());
            inspector.cmd_queue.push(InspectorCmd::ReloadScene);
        }
        app.update();

        let pending = app
            .world()
            .get_resource::<bsengine_scene::PendingSceneLoad>();
        assert_eq!(
            pending.map(|p| p.path.clone()),
            Some("assets/scenes/level1.ron".to_string()),
            "ReloadScene should insert a PendingSceneLoad for the current scene path"
        );
    }

    #[test]
    fn inspector_cmd_reload_scene_without_current_path_does_not_insert_pending_load() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.update();

        {
            let mut inspector = app.world_mut().resource_mut::<InspectorState>();
            assert_eq!(inspector.current_scene_path, None);
            inspector.cmd_queue.push(InspectorCmd::ReloadScene);
        }
        app.update();

        assert!(
            app.world()
                .get_resource::<bsengine_scene::PendingSceneLoad>()
                .is_none(),
            "ReloadScene with no current scene path should not insert PendingSceneLoad"
        );
    }

    #[test]
    fn editor_command_detach_primitive_mesh_also_removes_derived_mesh_renderer() {
        // `resolve_primitives` (bsengine-runtime) reacts to `Added<PrimitiveMesh>`
        // by inserting a derived `MeshRenderer` -- not exercised by this test's
        // plugin set, so it's simulated directly here. `DetachPrimitiveMesh`
        // must remove both, or the entity keeps rendering a stale mesh after
        // the Inspector's "Remove" button supposedly cleared it.
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app.world_mut().spawn(Name("Target".to_string())).id();
        app.update();

        app.world_mut()
            .entity_mut(eid)
            .insert(bsengine_scene::PrimitiveMesh(bsengine_scene::Primitive::Capsule))
            .insert(bsengine_render::MeshRenderer { mesh_id: 1 });

        {
            let queue = app.world().resource::<EditorCommandQueueResource>();
            queue.0.lock().unwrap().push(EditorCommand::DetachPrimitiveMesh {
                entity_id: eid.index() as u64,
            });
        }
        app.update();
        assert!(
            app.world()
                .get::<bsengine_scene::PrimitiveMesh>(eid)
                .is_none(),
            "PrimitiveMesh should be removed"
        );
        assert!(
            app.world().get::<bsengine_render::MeshRenderer>(eid).is_none(),
            "derived MeshRenderer must also be removed, or the entity keeps rendering a stale mesh"
        );
    }

    #[test]
    fn spawn_mesh_asset_command_spawns_entity_with_name_and_gltf_asset() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.update();

        {
            let queue = app.world().resource::<EditorCommandQueueResource>();
            queue.0.lock().unwrap().push(EditorCommand::SpawnMeshAsset {
                name: "Rock".to_string(),
                path: "assets/models/rock.glb".to_string(),
            });
        }
        app.update();

        let mut query = app.world_mut().query::<(&Name, &bsengine_gltf::GltfAsset)>();
        let (name, gltf_asset) = query
            .iter(app.world())
            .next()
            .expect("expected one entity with Name + GltfAsset");
        assert_eq!(name.0, "Rock");
        assert_eq!(gltf_asset.path, "assets/models/rock.glb");
    }

    #[test]
    fn spawn_mesh_asset_command_resolves_path_against_project_dir() {
        let mut app = new_app();
        app.insert_resource(bsengine_core::ProjectDir("games/demo".to_string()));
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.update();

        {
            let queue = app.world().resource::<EditorCommandQueueResource>();
            queue.0.lock().unwrap().push(EditorCommand::SpawnMeshAsset {
                name: "Rock".to_string(),
                path: "assets/models/rock.glb".to_string(),
            });
        }
        app.update();

        let mut query = app.world_mut().query::<(&Name, &bsengine_gltf::GltfAsset)>();
        let (name, gltf_asset) = query
            .iter(app.world())
            .next()
            .expect("expected one entity with Name + GltfAsset");
        assert_eq!(name.0, "Rock");
        assert_eq!(gltf_asset.path, "games/demo/assets/models/rock.glb");
    }

    #[test]
    fn load_scene_spawns_gltf_asset_for_entity_with_gltf_field() {
        let path = std::env::temp_dir()
            .join("bsengine_test_load_scene_gltf.ron")
            .to_string_lossy()
            .to_string();
        std::fs::write(
            &path,
            r#"SceneDescriptor(entities: [
                EntityDescriptor(name: "Player", gltf: Some("models/hero.glb")),
            ])"#,
        )
        .unwrap();

        let mut app = new_app();
        app.insert_resource(bsengine_core::ProjectDir("games/demo".to_string()));
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("load_scene", json!({"path": path}))
                .expect("load_scene not found");
        }
        app.update();

        let mut query = app.world_mut().query::<(&Name, &bsengine_gltf::GltfAsset)>();
        let (name, gltf_asset) = query
            .iter(app.world())
            .find(|(n, _)| n.0 == "Player")
            .expect("expected Player entity with GltfAsset after load_scene");
        assert_eq!(name.0, "Player");
        assert_eq!(gltf_asset.path, "games/demo/models/hero.glb");
    }

    /// Verifies the property List-append/enum-variant-switch actually
    /// depend on -- these three types have a `ReflectDefault` in the real
    /// app's registry, not just in reflect_ui.rs's own unit-test-local
    /// registries. A pass here doesn't uniquely attribute to this file's
    /// explicit `register_type` calls for these three types specifically:
    /// bevy_reflect's `register_type_dependencies` already walks
    /// `Transform`'s/`ScriptPath`'s own fields and would supply the same
    /// `ReflectDefault`s transitively even without them (confirmed by
    /// temporarily removing the explicit calls and re-running this test).
    /// Both paths are acceptable; this test guards the end state either way.
    #[test]
    fn app_type_registry_has_default_for_string_and_glam_wrappers() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.update();

        let registry = app
            .world()
            .resource::<bevy_ecs::reflect::AppTypeRegistry>()
            .read();
        for (type_id, label) in [
            (std::any::TypeId::of::<String>(), "String"),
            (
                std::any::TypeId::of::<bsengine_core::ReflectVec3>(),
                "ReflectVec3",
            ),
            (
                std::any::TypeId::of::<bsengine_core::ReflectQuat>(),
                "ReflectQuat",
            ),
        ] {
            assert!(
                registry
                    .get_type_data::<bevy_reflect::std_traits::ReflectDefault>(type_id)
                    .is_some(),
                "{label} must have a registered ReflectDefault for the List-append and \
                 enum-variant-switch UI features to work in the real app"
            );
        }
    }

    #[test]
    fn set_reflected_component_attaches_nav_mesh_agent_with_given_speed() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app.world_mut().spawn(Name("Enemy".to_string())).id();
        app.update();

        let out = {
            let mcp = app.world().resource::<McpRegistryResource>();
            let registry = mcp.0.lock().unwrap();
            registry.execute("set_reflected_component", json!({
                "entity_id": eid.index() as u64,
                "type_path": "bsengine_core::nav_mesh_agent::NavMeshAgent",
                "value_json": r#"{"destination": null, "speed": 3.5, "angular_speed": 2.0, "acceleration": 8.0, "stopping_distance": 0.1, "radius": 0.3, "height": 1.8, "state": "Idle", "enabled": true}"#,
            })).expect("tool should be registered")
        };
        assert!(out.is_ok(), "{:?}", out.error);

        app.update();

        let agent = app
            .world()
            .get::<bsengine_core::NavMeshAgent>(eid)
            .expect("NavMeshAgent should now be attached");
        assert!((agent.speed - 3.5).abs() < 0.001);
        assert!(agent.enabled);
    }

    #[test]
    fn set_reflected_component_attaches_animation_state_machine_with_states() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app.world_mut().spawn(Name("Player".to_string())).id();
        app.update();

        let value_json = r#"{
            "states": {
                "idle": {"clip": "idle_clip", "blend": null, "looping": true, "speed": 1.0, "duration": 1.0},
                "walk": {"clip": "walk_clip", "blend": null, "looping": true, "speed": 1.0, "duration": 1.0}
            },
            "transitions": [
                {"from": "idle", "to": "walk", "condition": {"FloatGreater": {"param": "speed", "threshold": 0.1}}, "blend_duration": 0.2}
            ],
            "current_state": "idle",
            "params_float": {"speed": 0.0},
            "params_bool": {},
            "triggers": [],
            "blend_from": null,
            "blend_weight": 1.0,
            "blend_duration": 0.0,
            "blend_elapsed": 0.0
        }"#;
        let out = {
            let mcp = app.world().resource::<McpRegistryResource>();
            let registry = mcp.0.lock().unwrap();
            registry.execute("set_reflected_component", json!({
                "entity_id": eid.index() as u64,
                "type_path": "bsengine_core::animation_state_machine::AnimationStateMachine",
                "value_json": value_json,
            })).expect("tool should be registered")
        };
        assert!(out.is_ok(), "{:?}", out.error);

        app.update();

        let asm = app
            .world()
            .get::<bsengine_core::AnimationStateMachine>(eid)
            .expect("AnimationStateMachine should now be attached");
        assert_eq!(asm.current_state, "idle");
        assert!(asm.states.contains_key("walk"));
        assert_eq!(asm.transitions.len(), 1);
    }

    #[test]
    fn set_reflected_component_rejects_unknown_type_path() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app.world_mut().spawn(Name("X".to_string())).id();
        app.update();

        let out = {
            let mcp = app.world().resource::<McpRegistryResource>();
            let registry = mcp.0.lock().unwrap();
            registry.execute("set_reflected_component", json!({
                "entity_id": eid.index() as u64,
                "type_path": "not::a::real::Type",
                "value_json": "{}",
            })).expect("tool should be registered")
        };
        assert!(!out.is_ok());
        assert!(out.error.unwrap().contains("unknown type path"));
    }

    #[test]
    fn set_reflected_component_rejects_json_not_matching_the_type() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app.world_mut().spawn(Name("X".to_string())).id();
        app.update();

        let out = {
            let mcp = app.world().resource::<McpRegistryResource>();
            let registry = mcp.0.lock().unwrap();
            registry.execute("set_reflected_component", json!({
                "entity_id": eid.index() as u64,
                "type_path": "bsengine_core::nav_mesh_agent::NavMeshAgent",
                "value_json": r#"{"speed": "not a number"}"#,
            })).expect("tool should be registered")
        };
        assert!(!out.is_ok());
    }

    #[test]
    fn process_prefab_commands_instantiates_a_queued_prefab() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("assets/prefabs")).unwrap();
        std::fs::write(
            root.join("assets/prefabs/enemy.ron"),
            r#"PrefabDescriptor(entities: [EntityDescriptor(name: "Enemy", primitive: Some(Cube))])"#,
        )
        .unwrap();

        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.insert_resource(bsengine_core::ProjectDir(root.to_string_lossy().to_string()));

        {
            let queue = app.world().resource::<crate::snapshot::PrefabCommandQueueResource>();
            queue.0.lock().unwrap().push(crate::snapshot::PrefabInstantiateCommand {
                path: "assets/prefabs/enemy.ron".to_string(),
                name: None,
                x: 1.0,
                y: 0.0,
                z: 0.0,
                parent_id: None,
            });
        }

        app.update();

        let mut q = app.world_mut().query::<&Name>();
        let names: Vec<String> = q.iter(app.world()).map(|n| n.0.clone()).collect();
        assert!(
            names.iter().any(|n| n.starts_with("Enemy#")),
            "queued prefab command should have been instantiated by the next update, names: {names:?}"
        );
    }

    #[test]
    fn process_prefab_commands_pushes_undo_checkpoint() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("assets/prefabs")).unwrap();
        std::fs::write(
            root.join("assets/prefabs/enemy.ron"),
            r#"PrefabDescriptor(entities: [EntityDescriptor(name: "Enemy", primitive: Some(Cube))])"#,
        )
        .unwrap();

        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.insert_resource(bsengine_core::ProjectDir(root.to_string_lossy().to_string()));
        app.update();

        let history_len_before = app
            .world()
            .resource::<crate::snapshot::EditorHistoryResource>()
            .0
            .lock()
            .unwrap()
            .undo_stack
            .len();

        {
            let queue = app.world().resource::<crate::snapshot::PrefabCommandQueueResource>();
            queue.0.lock().unwrap().push(crate::snapshot::PrefabInstantiateCommand {
                path: "assets/prefabs/enemy.ron".to_string(),
                name: None,
                x: 1.0,
                y: 0.0,
                z: 0.0,
                parent_id: None,
            });
        }
        app.update();

        let history_len_after = app
            .world()
            .resource::<crate::snapshot::EditorHistoryResource>()
            .0
            .lock()
            .unwrap()
            .undo_stack
            .len();
        assert_eq!(
            history_len_after,
            history_len_before + 1,
            "process_prefab_commands should push an undo checkpoint, same as ReflectCommand/EditorCommand do"
        );
    }

    #[test]
    fn process_prefab_apply_commands_applies_a_queued_entity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("assets/prefabs/queued.ron");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let source = r#"PrefabDescriptor(entities: [
            EntityDescriptor(name: "Root", primitive: Some(Cube)),
            EntityDescriptor(
                name: "Lamp",
                parent: Some("Root"),
                point_light: Some((color: (1.0, 1.0, 1.0), intensity: 1.0, range: 10.0)),
            ),
        ])"#;
        std::fs::write(&path, source).unwrap();

        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        app.add_plugins(EditorPlugin);

        let baseline: bsengine_scene::types::PrefabDescriptor = ron::from_str(source).unwrap();
        let root = bsengine_scene::instantiate_prefab(
            app.world_mut(),
            &baseline,
            path.to_str().unwrap(),
            Some("MyQueued"),
            None,
            None,
        )
        .unwrap();
        app.world_mut()
            .entity_mut(root)
            .insert(bsengine_core::PrefabInstanceBaseline {
                synced_ron: source.to_string(),
            });

        let lamp = {
            let mut q = app
                .world_mut()
                .query::<(bevy_ecs::prelude::Entity, &bsengine_scene::Name)>();
            q.iter(app.world())
                .find(|(_, n)| n.0.starts_with("Lamp#"))
                .map(|(e, _)| e)
                .unwrap()
        };
        {
            let mut pl = app
                .world_mut()
                .get_mut::<bsengine_core::PointLight>(lamp)
                .unwrap();
            pl.intensity = 5.0;
        }

        let root_id = root.index() as u64;
        {
            let queue = app
                .world()
                .resource::<crate::snapshot::PrefabApplyCommandQueueResource>();
            queue
                .0
                .lock()
                .unwrap()
                .push(crate::snapshot::PrefabApplyCommand { entity_id: root_id });
        }

        app.update();

        let written = std::fs::read_to_string(&path).unwrap();
        let parsed: bsengine_scene::types::PrefabDescriptor = ron::from_str(&written).unwrap();
        let lamp_out = parsed.entities.iter().find(|e| e.name == "Lamp").unwrap();
        assert_eq!(
            lamp_out.point_light.as_ref().unwrap().intensity,
            5.0,
            "queuing a PrefabApplyCommand and running one app.update() must apply it"
        );
    }

    fn entity_info(id: u64, name: &str, parent_id: Option<u64>) -> EntityInfo {
        EntityInfo {
            id,
            name: Some(name.to_string()),
            parent_id,
            ..Default::default()
        }
    }

    #[test]
    fn save_entities_as_prefab_collects_the_subtree_and_writes_a_ron_file() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = bsengine_core::ProjectDir(dir.path().to_string_lossy().to_string());

        let entities = vec![
            entity_info(1, "Root", None),
            entity_info(2, "Child", Some(1)),
            entity_info(3, "Grandchild", Some(2)),
            entity_info(4, "Unrelated", None),
        ];

        let path = save_entities_as_prefab(&entities, 1, "boss", Some(&project_dir)).unwrap();
        assert!(
            path.ends_with("assets/prefabs/boss.ron"),
            "unexpected path: {path}"
        );

        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: bsengine_scene::types::PrefabDescriptor = ron::from_str(&content).unwrap();
        let names: Vec<&str> = parsed.entities.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names.len(),
            3,
            "expected Root+Child+Grandchild, not Unrelated: {names:?}"
        );
        assert!(names.contains(&"Root"));
        assert!(names.contains(&"Child"));
        assert!(names.contains(&"Grandchild"));

        let root_desc = parsed.entities.iter().find(|e| e.name == "Root").unwrap();
        assert!(
            root_desc.parent.is_none(),
            "prefab root must have no parent:, got {:?}",
            root_desc.parent
        );
    }

    #[test]
    fn save_entities_as_prefab_drops_the_roots_link_to_a_parent_outside_the_subtree() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = bsengine_core::ProjectDir(dir.path().to_string_lossy().to_string());
        let entities = vec![
            entity_info(1, "Outside", None),
            entity_info(2, "SelectedRoot", Some(1)),
            entity_info(3, "Child", Some(2)),
        ];
        let path = save_entities_as_prefab(&entities, 2, "sub", Some(&project_dir)).unwrap();
        let parsed: bsengine_scene::types::PrefabDescriptor =
            ron::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            parsed.entities.len(),
            2,
            "the entity outside the selected subtree must not be included: {:?}",
            parsed.entities.iter().map(|e| &e.name).collect::<Vec<_>>()
        );
        let root_desc = parsed
            .entities
            .iter()
            .find(|e| e.name == "SelectedRoot")
            .unwrap();
        assert!(root_desc.parent.is_none());
    }

    #[test]
    fn save_entities_as_prefab_auto_suffixes_on_a_name_collision() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = bsengine_core::ProjectDir(dir.path().to_string_lossy().to_string());
        let entities = vec![entity_info(1, "Boss", None)];

        let first = save_entities_as_prefab(&entities, 1, "boss", Some(&project_dir)).unwrap();
        let second = save_entities_as_prefab(&entities, 1, "boss", Some(&project_dir)).unwrap();
        assert_ne!(first, second);
        assert!(
            second.ends_with("assets/prefabs/boss#2.ron"),
            "unexpected: {second}"
        );
        assert!(std::path::Path::new(&first).exists());
        assert!(std::path::Path::new(&second).exists());
    }

    #[test]
    fn save_entities_as_prefab_rejects_a_root_with_no_name() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = bsengine_core::ProjectDir(dir.path().to_string_lossy().to_string());
        let entities = vec![EntityInfo {
            id: 1,
            name: None,
            ..Default::default()
        }];
        let err = save_entities_as_prefab(&entities, 1, "boss", Some(&project_dir)).unwrap_err();
        assert!(
            err.contains('1'),
            "error should mention the entity id: {err}"
        );
    }

    #[test]
    fn save_entities_as_prefab_rejects_an_unnamed_descendant() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = bsengine_core::ProjectDir(dir.path().to_string_lossy().to_string());
        let entities = vec![
            entity_info(1, "Root", None),
            EntityInfo {
                id: 2,
                name: None,
                parent_id: Some(1),
                ..Default::default()
            },
        ];
        let err = save_entities_as_prefab(&entities, 1, "boss", Some(&project_dir)).unwrap_err();
        assert!(
            err.contains('2'),
            "error should mention the unnamed descendant's id: {err}"
        );
    }

    #[test]
    fn save_entities_as_prefab_rejects_an_unknown_root_id() {
        let entities = vec![entity_info(1, "Root", None)];
        let err = save_entities_as_prefab(&entities, 999, "boss", None).unwrap_err();
        assert!(err.contains("999"));
    }

    #[test]
    fn save_entities_as_prefab_rejects_a_name_with_path_traversal_characters() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = bsengine_core::ProjectDir(dir.path().to_string_lossy().to_string());
        let entities = vec![entity_info(1, "Root", None)];

        let err =
            save_entities_as_prefab(&entities, 1, "../evil", Some(&project_dir)).unwrap_err();
        assert!(
            err.contains("../evil"),
            "error should mention the rejected name: {err}"
        );

        let err2 =
            save_entities_as_prefab(&entities, 1, "sub/evil", Some(&project_dir)).unwrap_err();
        assert!(
            err2.contains("sub/evil"),
            "error should mention the rejected name: {err2}"
        );

        // Nothing should have been written anywhere, including outside the
        // temp project dir (walk up from the temp dir's parent to make sure
        // no "evil" file leaked out).
        let outside = dir.path().parent().unwrap().join("evil");
        assert!(
            !outside.exists(),
            "path traversal must not escape the project dir"
        );
        assert!(
            std::fs::read_dir(dir.path())
                .unwrap()
                .next()
                .is_none(),
            "nothing should have been written into the project dir either"
        );
    }

    #[test]
    fn save_entities_as_prefab_rejects_an_empty_name() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = bsengine_core::ProjectDir(dir.path().to_string_lossy().to_string());
        let entities = vec![entity_info(1, "Root", None)];

        let err = save_entities_as_prefab(&entities, 1, "", Some(&project_dir)).unwrap_err();
        assert!(err.contains("empty"), "unexpected error: {err}");
        assert!(
            std::fs::read_dir(dir.path()).unwrap().next().is_none(),
            "nothing should have been written for an empty name"
        );
    }

    #[test]
    fn save_entities_as_prefab_terminates_when_the_snapshot_has_a_parent_cycle() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = bsengine_core::ProjectDir(dir.path().to_string_lossy().to_string());
        let entities = vec![
            entity_info(1, "A", Some(2)),
            entity_info(2, "B", Some(1)),
        ];

        // The primary point of this test is that it returns at all: an
        // unguarded BFS over a parent_id cycle would loop forever and hang
        // the test binary. But returning isn't enough on its own to catch a
        // regression cleanly -- a future change that reintroduces the
        // missing guard would hang cargo test itself rather than fail it,
        // which is a bad failure mode to rely on alone. So this also pins
        // down the actual bounded result: both cycle members ended up in
        // the subtree exactly once each (matches HierarchyPanel::push_dfs's
        // same guard -- a visited id is skipped, not re-collected).
        let path = save_entities_as_prefab(&entities, 1, "cyclic", Some(&project_dir))
            .expect("a 2-entity mutual-parent cycle should still resolve, just bounded to visiting each member once");
        let parsed: bsengine_scene::types::PrefabDescriptor =
            ron::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let names: Vec<&str> = parsed.entities.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names.len(),
            2,
            "each cycle member should be collected exactly once, not zero or repeated: {names:?}"
        );
        assert!(names.contains(&"A"));
        assert!(names.contains(&"B"));
    }

    #[test]
    fn a_prefab_saved_from_live_entities_round_trips_through_instantiate_prefab() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = bsengine_core::ProjectDir(dir.path().to_string_lossy().to_string());

        let entities = vec![EntityInfo {
            id: 1,
            name: Some("Turret".to_string()),
            position: Some([2.0, 0.0, 0.0]),
            primitive: Some(bsengine_scene::Primitive::Cube),
            ..Default::default()
        }];

        let path = save_entities_as_prefab(&entities, 1, "turret", Some(&project_dir)).unwrap();

        let mut app = new_app();
        let root = bsengine_scene::instantiate_prefab_from_path(
            app.world_mut(),
            &path,
            None,
            None,
            None,
        )
        .expect("a prefab saved by save_entities_as_prefab must re-instantiate cleanly");

        let name = app.world().get::<Name>(root).unwrap().0.clone();
        assert!(
            name.starts_with("Turret#"),
            "expected an auto-suffixed instance name, got {name}"
        );
        let transform = app.world().get::<Transform>(root).unwrap();
        assert!(
            (transform.position.x - 2.0).abs() < 1e-4,
            "position did not round-trip: {:?}",
            transform.position
        );
    }

    #[test]
    fn mcp_prefab_write_saves_a_subtree_to_a_ron_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = new_app();
        app.insert_resource(bsengine_core::ProjectDir(
            dir.path().to_string_lossy().to_string(),
        ));
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "Turret"},
                        {"name": "Barrel"},
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let all = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"]
                .as_array()
                .unwrap()
                .clone()
        };
        let id_of = |name: &str| {
            all.iter()
                .find(|e| e["name"].as_str() == Some(name))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let (turret_id, barrel_id) = (id_of("Turret"), id_of("Barrel"));

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "set_parent",
                    json!({"entity_id": barrel_id, "parent_id": turret_id}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("prefab_write", json!({"entity_id": turret_id, "name": "turret"}))
            .expect("prefab_write not registered");
        assert!(out.is_ok(), "prefab_write failed: {:?}", out.error);
        let path = out.content["path"].as_str().unwrap().to_string();
        assert!(path.ends_with("assets/prefabs/turret.ron"), "{path}");

        let parsed: bsengine_scene::types::PrefabDescriptor =
            ron::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let names: Vec<&str> = parsed.entities.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names.len(), 2, "{names:?}");
        assert!(names.contains(&"Turret"));
        assert!(names.contains(&"Barrel"));
    }

    #[test]
    fn create_prefab_inspector_cmd_saves_the_subtree() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = new_app();
        app.insert_resource(bsengine_core::ProjectDir(
            dir.path().to_string_lossy().to_string(),
        ));
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("batch_spawn", json!({"entities": [{"name": "Turret"}]}))
                .unwrap();
        }
        app.update();
        app.update();

        // batch_spawn only queues the spawn and reports back a count, not
        // the new entity's id (it's applied on the following frame) -- so
        // the id has to be read back via list_entities, same as
        // mcp_prefab_write_saves_a_subtree_to_a_ron_file above does.
        let entity_id = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"][0]["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mut inspector = app.world_mut().resource_mut::<InspectorState>();
            inspector.cmd_queue.push(InspectorCmd::CreatePrefab {
                entity_id,
                name: "turret".to_string(),
            });
        }
        app.update();

        let path = dir
            .path()
            .join("assets/prefabs/turret.ron")
            .to_string_lossy()
            .to_string();
        assert!(
            std::path::Path::new(&path).exists(),
            "expected {path} to exist"
        );
        let parsed: bsengine_scene::types::PrefabDescriptor =
            ron::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(parsed.entities.len(), 1);
        assert_eq!(parsed.entities[0].name, "Turret");
    }

    #[test]
    fn create_prefab_inspector_cmd_with_an_unknown_entity_id_does_not_panic_and_does_not_block_later_commands(
    ) {
        let dir = tempfile::tempdir().unwrap();
        let mut app = new_app();
        app.insert_resource(bsengine_core::ProjectDir(
            dir.path().to_string_lossy().to_string(),
        ));
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("batch_spawn", json!({"entities": [{"name": "Turret"}]}))
                .unwrap();
        }
        app.update();
        app.update();

        let entity_id = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap()
                .content["entities"][0]["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mut inspector = app.world_mut().resource_mut::<InspectorState>();
            // First command targets an entity that doesn't exist -- must
            // warn-and-continue (matching every other arm in this match),
            // not panic and not stop the queue from draining the rest.
            inspector.cmd_queue.push(InspectorCmd::CreatePrefab {
                entity_id: 999999,
                name: "ghost".to_string(),
            });
            inspector.cmd_queue.push(InspectorCmd::CreatePrefab {
                entity_id,
                name: "turret".to_string(),
            });
        }
        app.update();

        assert!(
            !dir.path().join("assets/prefabs/ghost.ron").exists(),
            "no file should be written for the unknown entity id"
        );
        let path = dir
            .path()
            .join("assets/prefabs/turret.ron")
            .to_string_lossy()
            .to_string();
        assert!(
            std::path::Path::new(&path).exists(),
            "the second, valid command must still have been processed: expected {path} to exist"
        );
    }

    #[test]
    fn mcp_prefab_write_reports_an_error_for_an_unknown_entity_id() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("prefab_write", json!({"entity_id": 999999, "name": "x"}))
            .expect("prefab_write not registered");
        assert!(!out.is_ok());
    }

    #[test]
    fn apply_to_prefab_tool_queues_a_command() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        let out = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let mcp = mcp.0.lock().unwrap();
            mcp.execute("apply_to_prefab", json!({"entity_id": 42}))
        }
        .expect("apply_to_prefab not registered");
        assert!(out.is_ok(), "apply_to_prefab failed: {:?}", out.error);
        assert_eq!(out.content["status"], "queued");

        let queue = app
            .world()
            .resource::<crate::snapshot::PrefabApplyCommandQueueResource>();
        let queued = queue.0.lock().unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].entity_id, 42);
    }

    #[test]
    fn apply_to_prefab_tool_errors_without_entity_id() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        let out = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let mcp = mcp.0.lock().unwrap();
            mcp.execute("apply_to_prefab", json!({}))
        }
        .expect("apply_to_prefab not registered");
        assert!(!out.is_ok());
    }

    /// An editor app with two emitters: a burst-only one and a continuous
    /// one, each holding a few live particles, so a restart has something
    /// to drop and the two kinds of "start" can be told apart.
    fn app_with_two_emitters() -> (bevy_app::App, bevy_ecs::entity::Entity, bevy_ecs::entity::Entity) {
        use bsengine_core::{Particle, ParticleEmitter};
        let mut app = new_app();
        app.add_plugins(EditorPlugin);
        let live = || {
            vec![
                Particle {
                    position: glam::Vec3::ZERO,
                    velocity: glam::Vec3::Y,
                    age: 0.1,
                };
                3
            ]
        };
        let burst_only = app
            .world_mut()
            .spawn((
                Name("Sparks".into()),
                Transform::default(),
                ParticleEmitter {
                    rate: 0.0,
                    burst_count: 7,
                    live: live(),
                    spawn_debt: 0.4,
                    ..Default::default()
                },
            ))
            .id();
        let continuous = app
            .world_mut()
            .spawn((
                Name("Smoke".into()),
                Transform::default(),
                ParticleEmitter {
                    rate: 12.0,
                    burst_count: 5,
                    live: live(),
                    spawn_debt: 0.4,
                    ..Default::default()
                },
            ))
            .id();
        (app, burst_only, continuous)
    }

    fn emitter(app: &bevy_app::App, e: bevy_ecs::entity::Entity) -> &bsengine_core::ParticleEmitter {
        app.world().get::<bsengine_core::ParticleEmitter>(e).unwrap()
    }

    /// The Particles panel's rows come from the live emitters, after the
    /// entity snapshot is rebuilt: an alive count that lagged a frame, or
    /// that was wiped by `populate_inspector`, would show every effect as
    /// empty.
    #[test]
    fn the_inspector_snapshot_carries_each_emitters_live_count() {
        let (mut app, burst_only, continuous) = app_with_two_emitters();
        app.update();
        let insp = app.world().resource::<InspectorState>();
        let find = |e: bevy_ecs::entity::Entity| {
            insp.entities
                .iter()
                .find(|i| i.id == e.index() as u64)
                .and_then(|i| i.particles)
                .expect("an entity with an emitter must carry a particle snapshot")
        };
        assert_eq!(find(burst_only).alive, 3);
        assert_eq!(find(burst_only).rate, 0.0);
        assert_eq!(find(burst_only).burst_count, 7);
        assert_eq!(find(continuous).rate, 12.0);
        assert!(
            insp.entities
                .iter()
                .filter(|i| i.particles.is_some())
                .count()
                == 2,
            "only the two emitters carry one"
        );
    }

    /// Burst reaches the one emitter it names and no other; the queued
    /// count is the emitter's own, as a script's burst would be.
    #[test]
    fn particle_burst_queues_a_burst_on_the_named_emitter_only() {
        let (mut app, burst_only, continuous) = app_with_two_emitters();
        app.world_mut()
            .resource_mut::<InspectorState>()
            .cmd_queue
            .push(InspectorCmd::ParticleBurst {
                id: burst_only.index() as u64,
            });
        app.update();
        assert_eq!(emitter(&app, burst_only).pending_burst, 7);
        assert_eq!(emitter(&app, continuous).pending_burst, 0, "the other emitter is untouched");
    }

    /// Restart's two meanings of "start": a continuous effect comes back
    /// empty and emitting, a burst-only effect comes back with its burst
    /// queued -- and `None` does it to every emitter.
    #[test]
    fn particle_restart_drops_live_particles_and_requeues_a_burst_only_effect() {
        let (mut app, burst_only, continuous) = app_with_two_emitters();
        assert_eq!(emitter(&app, burst_only).live.len(), 3, "premise: live particles to drop");

        app.world_mut()
            .resource_mut::<InspectorState>()
            .cmd_queue
            .push(InspectorCmd::ParticleRestart {
                id: Some(continuous.index() as u64),
            });
        app.update();
        let smoke = emitter(&app, continuous);
        assert!(smoke.live.is_empty(), "restart drops the live particles");
        assert_eq!(smoke.spawn_debt, 0.0, "and the fractional carry");
        assert_eq!(smoke.pending_burst, 0, "a continuous effect gets no burst");
        assert_eq!(emitter(&app, burst_only).live.len(), 3, "the other emitter is untouched");

        app.world_mut()
            .resource_mut::<InspectorState>()
            .cmd_queue
            .push(InspectorCmd::ParticleRestart { id: None });
        app.update();
        let sparks = emitter(&app, burst_only);
        assert!(sparks.live.is_empty(), "Restart All reaches every emitter");
        assert_eq!(
            sparks.pending_burst, 7,
            "a burst-only effect's start is its burst, so it is queued again"
        );
    }

    #[test]
    fn apply_inspector_cmds_bridges_apply_to_prefab_into_the_prefab_apply_queue() {
        let mut app = new_app();
        app.add_plugins(EditorPlugin);

        {
            let mut inspector = app.world_mut().resource_mut::<InspectorState>();
            inspector.cmd_queue.push(InspectorCmd::ApplyToPrefab { entity_id: 42 });
        }

        app.update();

        let queue = app
            .world()
            .resource::<crate::snapshot::PrefabApplyCommandQueueResource>();
        let queued = queue.0.lock().unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].entity_id, 42);
    }

    /// A directory of its own with one fake texture in it, for the import
    /// settings round trip below. The bytes are not a PNG on purpose: nothing
    /// here decodes them, and the sidecar hashes whatever is there.
    struct TextureProbe {
        dir: std::path::PathBuf,
        path: String,
    }

    impl TextureProbe {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "bsengine-editor-import-{tag}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let file = dir.join("wall.png");
            std::fs::write(&file, b"not a png").unwrap();
            Self {
                path: file.to_string_lossy().to_string(),
                dir,
            }
        }
    }

    impl Drop for TextureProbe {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.dir).ok();
        }
    }

    /// A throwaway project with a manifest, so the references walk has an
    /// entry scene to start from: `main.ron` names the model and the
    /// script, the script names a second scene, and `unused.png` is named
    /// by nothing.
    struct ProjectProbe {
        dir: std::path::PathBuf,
    }

    impl ProjectProbe {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "bsengine-editor-refs-{tag}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            let probe = Self { dir };
            probe.write(
                "project.toml",
                "[project]\nname = \"P\"\nentry_scene = \"assets/scenes/main.ron\"\n",
            );
            probe.write(
                "assets/scenes/main.ron",
                r#"(entities: [(name: "Hero", gltf: Some(Path("assets/models/hero.glb")), script: Some(Path("assets/scripts/hero.js")))])"#,
            );
            probe.write(
                "assets/scripts/hero.js",
                "const NEXT = \"assets/scenes/level2.ron\";",
            );
            probe.write(
                "assets/scenes/level2.ron",
                r#"(entities: [(name: "Hero", gltf: Some(Path("assets/models/hero.glb")))])"#,
            );
            probe.write("assets/models/hero.glb", "glb");
            probe.write("assets/textures/unused.png", "png");
            probe
        }

        fn write(&self, relative: &str, contents: &str) {
            let path = self.dir.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }
    }

    impl Drop for ProjectProbe {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.dir).ok();
        }
    }

    /// Selecting an asset walks the project into `asset_references`, through
    /// the real system: a model two scenes name lists both, a leaf lists no
    /// dependencies, and the texture nothing names is `reached: false`. The
    /// walk is the packager's, so the second scene is found through the
    /// script that names it, not because a test listed it.
    #[test]
    fn selecting_an_asset_lists_what_references_it_and_what_it_references() {
        let probe = ProjectProbe::new("walk");
        let mut app = new_app();
        app.add_plugins(EditorPlugin);
        app.insert_resource(bsengine_core::ProjectDir(
            probe.dir.to_string_lossy().to_string(),
        ));

        app.world_mut()
            .resource_mut::<InspectorState>()
            .select_asset("assets/models/hero.glb");
        app.update();
        {
            let insp = app.world().resource::<InspectorState>();
            let refs = insp
                .asset_references
                .as_ref()
                .expect("one update after selecting must walk the project");
            assert_eq!(refs.path, "assets/models/hero.glb");
            assert_eq!(refs.error, None);
            assert!(refs.reached, "the entry scene names the model");
            assert_eq!(
                refs.referencers,
                vec!["assets/scenes/level2.ron", "assets/scenes/main.ron"],
                "both scenes, the second reached only through the script"
            );
            assert!(refs.dependencies.is_empty(), "a model is a leaf");
        }

        app.world_mut()
            .resource_mut::<InspectorState>()
            .select_asset("assets/scripts/hero.js");
        app.update();
        {
            let refs = app
                .world()
                .resource::<InspectorState>()
                .asset_references
                .clone()
                .expect("re-walked for the new selection");
            assert_eq!(refs.path, "assets/scripts/hero.js");
            assert_eq!(refs.referencers, vec!["assets/scenes/main.ron"]);
            assert_eq!(
                refs.dependencies,
                vec!["assets/scenes/level2.ron"],
                "a quoted path in a script is a dependency"
            );
        }

        app.world_mut()
            .resource_mut::<InspectorState>()
            .select_asset("assets/textures/unused.png");
        app.update();
        let refs = app
            .world()
            .resource::<InspectorState>()
            .asset_references
            .clone()
            .expect("walked");
        assert!(!refs.reached, "nothing names the texture");
        assert!(refs.referencers.is_empty());
        assert_eq!(refs.error, None);
    }

    /// The whole-project graph is walked when asked and only then: the
    /// request fills it with the packager's edges, unreached assets and
    /// dangling references, and a scene edited on disk afterwards is not
    /// seen until the next request -- which a scene save makes.
    #[test]
    fn the_asset_graph_is_walked_on_request_and_again_after_a_scene_save() {
        let probe = ProjectProbe::new("graph");
        probe.write(
            "assets/scenes/level2.ron",
            r#"(entities: [(name: "Hero", gltf: Some(Path("assets/models/hero.glb")), texture: Some("assets/textures/gone.png"))])"#,
        );
        let mut app = new_app();
        app.add_plugins(EditorPlugin);
        app.insert_resource(bsengine_core::ProjectDir(
            probe.dir.to_string_lossy().to_string(),
        ));

        app.update();
        assert!(
            app.world()
                .resource::<InspectorState>()
                .asset_graph
                .is_none(),
            "premise: nothing asked, nothing walked"
        );

        app.world_mut()
            .resource_mut::<InspectorState>()
            .asset_graph_refresh = true;
        app.update();
        let insp = app.world().resource::<InspectorState>();
        assert!(!insp.asset_graph_refresh, "the request is consumed");
        let graph = insp.asset_graph.clone().expect("walked on request");
        assert_eq!(graph.error, None);
        let edge = |a: &str, b: &str| (a.to_string(), b.to_string());
        assert!(
            graph
                .edges
                .contains(&edge("assets/scripts/hero.js", "assets/scenes/level2.ron")),
            "the packager's edges, script mentions included; got {:?}",
            graph.edges
        );
        assert_eq!(graph.unreferenced, vec!["assets/textures/unused.png"]);
        assert_eq!(
            graph.missing,
            vec![edge("assets/scenes/level2.ron", "assets/textures/gone.png")]
        );

        // The scene changes on disk; without a request the graph is what
        // it was.
        probe.write(
            "assets/scenes/level2.ron",
            r#"(entities: [(name: "Hero", texture: Some("assets/textures/unused.png"))])"#,
        );
        app.update();
        assert_eq!(
            app.world()
                .resource::<InspectorState>()
                .asset_graph
                .as_ref()
                .map(|g| g.unreferenced.clone()),
            Some(vec!["assets/textures/unused.png".to_string()]),
            "no request, no re-walk"
        );

        // A save of any scene is a request: the saved file here is one the
        // walk does not reach (the editor writes only what it holds), but
        // the request it makes re-reads the whole project.
        app.world()
            .resource::<EditorCommandQueueResource>()
            .0
            .lock()
            .unwrap()
            .push(EditorCommand::SaveScene {
                path: probe
                    .dir
                    .join("assets/scenes/saved.ron")
                    .to_string_lossy()
                    .to_string(),
            });
        app.update();
        let graph = app
            .world()
            .resource::<InspectorState>()
            .asset_graph
            .clone()
            .expect("still there");
        assert_eq!(
            graph.unreferenced,
            vec!["assets/scenes/saved.ron"],
            "after the save the walk sees level2.ron now naming the texture, and the \
             freshly saved scene -- which nothing names -- as the one unreached asset"
        );
        assert!(graph.missing.is_empty(), "and the dangling reference is gone");
    }

    /// A project the walk cannot start in records why, once, instead of
    /// leaving the snapshot empty and walking again every frame.
    #[test]
    fn a_project_without_a_manifest_records_the_error_in_the_snapshot() {
        let probe = ProjectProbe::new("no-manifest");
        std::fs::remove_file(probe.dir.join("project.toml")).unwrap();
        let mut app = new_app();
        app.add_plugins(EditorPlugin);
        app.insert_resource(bsengine_core::ProjectDir(
            probe.dir.to_string_lossy().to_string(),
        ));

        app.world_mut()
            .resource_mut::<InspectorState>()
            .select_asset("assets/models/hero.glb");
        app.update();
        let refs = app
            .world()
            .resource::<InspectorState>()
            .asset_references
            .clone()
            .expect("the failure is a snapshot, not an absence");
        assert!(
            refs.error
                .as_deref()
                .is_some_and(|e| e.contains("project.toml")),
            "the error names the manifest: {:?}",
            refs.error
        );
        assert!(!refs.reached);
    }

    /// The Inspector's whole import-settings loop, through the real
    /// systems: selecting an asset reads its sidecar into the snapshot,
    /// Apply writes the sidecar and the snapshot comes back *from disk* with
    /// `recorded: true`, in the same frame -- `populate_asset_import_snapshot`
    /// is ordered after the command drain for exactly that. The premise
    /// (the first read reports the defaults, unrecorded) is asserted so a
    /// write that silently failed cannot pass as "still the defaults".
    #[test]
    fn selecting_an_asset_reads_its_import_settings_and_apply_writes_them_back() {
        use bsengine_asset::identity::{sidecar_path, Sidecar};
        use bsengine_core::{ImportSettings, TextureFilter, TextureImportSettings};

        let probe = TextureProbe::new("round-trip");
        let mut app = new_app();
        app.add_plugins(EditorPlugin);

        app.world_mut()
            .resource_mut::<InspectorState>()
            .select_asset(probe.path.clone());
        app.update();
        {
            let insp = app.world().resource::<InspectorState>();
            let snapshot = insp
                .asset_import
                .as_ref()
                .expect("one update after selecting must read the sidecar");
            assert_eq!(snapshot.path, probe.path);
            assert!(!snapshot.recorded, "premise: nothing is recorded yet");
            assert_eq!(
                snapshot.settings,
                ImportSettings::Texture(TextureImportSettings::default())
            );
            assert_eq!(snapshot.edit, snapshot.settings);
            assert!(insp.asset_import_error.is_none());
        }

        let tuned = ImportSettings::Texture(TextureImportSettings {
            srgb: false,
            filter: TextureFilter::Nearest,
            ..Default::default()
        });
        app.world_mut()
            .resource_mut::<InspectorState>()
            .cmd_queue
            .push(InspectorCmd::WriteImportSettings {
                path: probe.path.clone(),
                settings: tuned,
            });
        app.update();

        let on_disk = Sidecar::read(sidecar_path(&probe.path))
            .expect("readable")
            .expect("Apply must write a sidecar beside the texture");
        assert_eq!(on_disk.texture_import().filter, TextureFilter::Nearest);
        assert!(!on_disk.texture_import().srgb);

        let insp = app.world().resource::<InspectorState>();
        let snapshot = insp
            .asset_import
            .as_ref()
            .expect("the snapshot must be re-read in the frame the write landed");
        assert!(
            snapshot.recorded,
            "the re-read snapshot must say the settings are now recorded"
        );
        assert_eq!(snapshot.settings, tuned);
        assert_eq!(snapshot.edit, tuned, "nothing is pending after Apply");
        assert!(insp.asset_import_error.is_none());
    }

    /// The scan's rule reaches the Inspector: a sidecar that will not parse
    /// is reported, not overwritten. Both the read and a later Apply leave
    /// the bytes exactly as they were.
    #[test]
    fn a_broken_sidecar_is_reported_to_the_inspector_and_never_overwritten() {
        use bsengine_asset::identity::sidecar_path;
        use bsengine_core::{ImportSettings, TextureImportSettings};

        let probe = TextureProbe::new("broken");
        let meta = sidecar_path(&probe.path);
        std::fs::write(&meta, b"(guid: 7)").unwrap();

        let mut app = new_app();
        app.add_plugins(EditorPlugin);
        app.world_mut()
            .resource_mut::<InspectorState>()
            .select_asset(probe.path.clone());
        app.update();
        {
            let insp = app.world().resource::<InspectorState>();
            assert!(insp.asset_import.is_none());
            let error = insp
                .asset_import_error
                .as_deref()
                .expect("a broken sidecar must be reported");
            assert!(error.contains("could not be read"), "{error}");
        }

        app.world_mut()
            .resource_mut::<InspectorState>()
            .cmd_queue
            .push(InspectorCmd::WriteImportSettings {
                path: probe.path.clone(),
                settings: ImportSettings::Texture(TextureImportSettings::default()),
            });
        app.update();
        assert_eq!(
            std::fs::read(&meta).unwrap(),
            b"(guid: 7)",
            "Apply must not overwrite a sidecar it could not read"
        );
        assert!(app
            .world()
            .resource::<InspectorState>()
            .asset_import_error
            .is_some());
    }
}
