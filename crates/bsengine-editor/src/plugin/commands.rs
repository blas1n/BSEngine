//! Applies queued `EditorCommand`s to the world.

use super::*;

pub(super) const MAX_UNDO_HISTORY: usize = 100;

/// Puts an emitter back at its start: every live particle dropped, the
/// fractional spawn carry cleared, and -- for a burst-only effect -- its
/// burst queued again, since the burst is what that effect's start looks
/// like and a restart that left it empty would read as a delete. A
/// continuous effect simply resumes emitting from nothing, which is what
/// Unity's Restart and Godot's `restart()` both do.
pub(super) fn restart_emitter(emitter: &mut bsengine_core::ParticleEmitter) {
    emitter.live.clear();
    emitter.spawn_debt = 0.0;
    emitter.pending_burst = 0;
    if emitter.rate <= 0.0 {
        emitter.burst();
    }
}

#[allow(clippy::too_many_arguments)] // Bevy system params; splitting into a struct is a larger refactor
pub(super) fn process_editor_commands(
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
                    commands.entity(entity).insert(Visible {
                        is_visible: visible,
                    });
                }
            }
            // Both through a deferred world closure rather than a query in
            // `params`: the `ParamSet` above is at Bevy's eight-slot ceiling,
            // and a ninth query for a component only two commands touch is
            // not worth restructuring the set for. The closure runs when
            // `commands` flush, at the end of this system -- the same frame.
            EditorCommand::ParticleBurst { entity_id } => {
                commands.add(move |world: &mut World| {
                    let mut emitters =
                        world.query::<(Entity, &mut bsengine_core::ParticleEmitter)>();
                    for (entity, mut emitter) in emitters.iter_mut(world) {
                        if entity.index() as u64 == entity_id {
                            emitter.burst();
                        }
                    }
                });
            }
            EditorCommand::ParticleRestart { entity_id } => {
                commands.add(move |world: &mut World| {
                    let mut emitters =
                        world.query::<(Entity, &mut bsengine_core::ParticleEmitter)>();
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
                    commands
                        .entity(entity)
                        .remove::<bsengine_scene::ScriptPath>();
                }
            }
            EditorCommand::AttachPrimitiveMesh {
                entity_id,
                primitive,
            } => {
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
            EditorCommand::AttachCamera {
                entity_id,
                fov_y_degrees,
            } => {
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
                            tracing::warn!("load_scene: unknown reflected type path '{type_path}'");
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
                let scene = SceneDescriptor {
                    entities,
                    skybox: None,
                };
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
