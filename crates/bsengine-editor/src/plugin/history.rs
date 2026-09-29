//! Undo/redo: snapshots of the scene and putting the world back to one.

use super::*;

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
pub(super) fn reconcile_to_snapshot(
    world: &mut World,
    current: &EditorSnapshot,
    target: &EditorSnapshot,
) {
    let mut live_by_id: std::collections::HashMap<u64, Entity> = std::collections::HashMap::new();
    {
        let mut q = world.query::<Entity>();
        for e in q.iter(world) {
            live_by_id.insert(e.index() as u64, e);
        }
    }

    let target_ids: std::collections::HashSet<u64> = target.entities.iter().map(|e| e.id).collect();
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

pub(super) fn sync_entity_to_info(world: &mut World, entity: Entity, info: &EntityInfo) {
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

pub(super) fn spawn_entity_from_info(world: &mut World, info: &EntityInfo) -> Entity {
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

pub(super) fn apply_history_action(world: &mut World) {
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
