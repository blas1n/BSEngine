use crate::snapshot::{
    EditorCommand, EditorCommandQueueResource, EditorSelectionResource, EditorSnapshot,
    EditorSnapshotResource, EntityInfo, SharedCommandQueue, SharedSelection, SharedSnapshot, Tags,
    Visible,
};
use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::{Commands, Entity, ParamSet, Query};
use bsengine_core::{
    Camera, DirectionalLight, GlobalTransform, Parent, PointLight, SpotLight, Transform,
};
use bsengine_ecs::Res;
use bsengine_mcp::{McpRegistryResource, McpTool, McpToolOutput};
use bsengine_render::MeshRenderer;
use bsengine_scene::{EntityDescriptor, Name, SceneDescriptor};
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
    )>,
) {
    let selection = selection_res.0.lock().unwrap().clone();
    let mut snapshot = snapshot_res.0.lock().unwrap();
    snapshot.entities = query
        .iter()
        .map(
            |(e, name, transform, mesh, pt, dir, spot, cam, parent, tags, vis)| {
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
                    position: transform.map(|t| t.translation.to_array()),
                    rotation: transform.map(|t| {
                        let (rx, ry, rz) = t.rotation.to_euler(glam::EulerRot::XYZ);
                        [rx.to_degrees(), ry.to_degrees(), rz.to_degrees()]
                    }),
                    scale: transform.map(|t| t.scale.to_array()),
                    mesh_id: mesh.map(|m| m.mesh_id),
                    light_type,
                    light_color,
                    light_intensity,
                    light_range,
                    camera_fov: cam.map(|c| c.fov_y_radians.to_degrees()),
                    parent_id: parent.map(|p| p.0.index() as u64),
                    tags: tags.map(|t| t.0.clone()).unwrap_or_default(),
                    visible: vis.map(|v| v.0).unwrap_or(true),
                    selected: selection.contains(&(e.index() as u64)),
                }
            },
        )
        .collect();
}

fn process_editor_commands(
    queue_res: Res<EditorCommandQueueResource>,
    snapshot_res: Res<EditorSnapshotResource>,
    mut params: ParamSet<(
        Query<Entity>,
        Query<(Entity, &mut Transform)>,
        Query<(Entity, &mut PointLight)>,
        Query<(Entity, &mut DirectionalLight)>,
        Query<(Entity, &mut SpotLight)>,
        Query<(Entity, &mut Camera)>,
        Query<(Entity, &mut Tags)>,
    )>,
    mut commands: Commands,
) {
    let cmds: Vec<EditorCommand> = {
        let mut queue = queue_res.0.lock().unwrap();
        queue.drain(..).collect()
    };

    for cmd in cmds {
        match cmd {
            EditorCommand::SpawnNamed(name) => {
                commands.spawn(Name(name));
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
                        t.translation = glam::Vec3::new(x, y, z);
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
            EditorCommand::SpawnPointLight {
                color,
                intensity,
                range,
                position,
            } => {
                commands.spawn((
                    PointLight {
                        color: glam::Vec3::from(color),
                        intensity,
                        range,
                    },
                    Transform::from_translation(glam::Vec3::from(position)),
                    GlobalTransform::default(),
                ));
            }
            EditorCommand::SpawnDirectionalLight {
                direction,
                color,
                ambient,
            } => {
                commands.spawn(DirectionalLight {
                    direction: glam::Vec3::from(direction),
                    color: glam::Vec3::from(color),
                    ambient: glam::Vec3::from(ambient),
                });
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
                            light.color = glam::Vec3::from(c);
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
                        if let Some(d) = direction {
                            light.direction = glam::Vec3::from(d);
                        }
                        if let Some(c) = color {
                            light.color = glam::Vec3::from(c);
                        }
                        if let Some(a) = ambient {
                            light.ambient = glam::Vec3::from(a);
                        }
                        break;
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
                    Transform::from_translation(glam::Vec3::from(position)),
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
                            cam.fov_y_radians = fov.to_radians();
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
                            Transform::from_translation(glam::Vec3::new(x, y, z)),
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
                        t.translation += glam::Vec3::new(dx, dy, dz);
                        break;
                    }
                }
            }
            EditorCommand::SetVisible { entity_id, visible } => {
                let target = params.p0().iter().find(|e| e.index() as u64 == entity_id);
                if let Some(entity) = target {
                    commands.entity(entity).insert(Visible(visible));
                }
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
                        );
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
                        t.scale = glam::Vec3::new(sx, sy, sz);
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
                            t.translation = glam::Vec3::new(x, y, z);
                        }
                        if let Some([rx, ry, rz]) = rotation {
                            t.rotation = glam::Quat::from_euler(
                                glam::EulerRot::XYZ,
                                rx.to_radians(),
                                ry.to_radians(),
                                rz.to_radians(),
                            );
                        }
                        if let Some([sx, sy, sz]) = scale {
                            t.scale = glam::Vec3::new(sx, sy, sz);
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
                            Transform::from_translation(glam::Vec3::new(x, y, z)),
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
                        color: glam::Vec3::from(color),
                        intensity,
                        range,
                        inner_angle,
                        outer_angle,
                    },
                    Transform::from_translation(glam::Vec3::from(position)),
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
                            light.color = glam::Vec3::from(c);
                        }
                        if let Some(i) = intensity {
                            light.intensity = i;
                        }
                        if let Some(r) = range {
                            light.range = r;
                        }
                        if let Some(a) = inner_angle {
                            light.inner_angle = a;
                        }
                        if let Some(o) = outer_angle {
                            light.outer_angle = o;
                        }
                        break;
                    }
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
                for entity in scene.entities {
                    let pos = entity.components.iter().find_map(|(k, v)| {
                        if k == "transform_position" {
                            parse_position(v)
                        } else {
                            None
                        }
                    });
                    if let Some([x, y, z]) = pos {
                        commands.spawn((
                            Name(entity.name),
                            Transform::from_translation(glam::Vec3::new(x, y, z)),
                        ));
                    } else {
                        commands.spawn(Name(entity.name));
                    }
                }
            }
        }
    }
}

fn parse_position(s: &str) -> Option<[f32; 3]> {
    let parts: Vec<f32> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
    if parts.len() == 3 {
        Some([parts[0], parts[1], parts[2]])
    } else {
        None
    }
}

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        let snapshot: SharedSnapshot = Arc::new(Mutex::new(EditorSnapshot::default()));
        let cmd_queue: SharedCommandQueue = Arc::new(Mutex::new(Vec::new()));
        let selection: SharedSelection = Arc::new(Mutex::new(std::collections::HashSet::new()));

        app.insert_resource(EditorSnapshotResource(snapshot.clone()));
        app.insert_resource(EditorCommandQueueResource(cmd_queue.clone()));
        app.insert_resource(EditorSelectionResource(selection.clone()));
        app.add_systems(Update, update_editor_snapshot);
        app.add_systems(Update, process_editor_commands);

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
                    let entities: Vec<EntityDescriptor> = s
                        .entities
                        .iter()
                        .filter_map(|e| {
                            e.name.as_ref().map(|name| {
                                let mut components = Vec::new();
                                if let Some([x, y, z]) = e.position {
                                    components.push((
                                        "transform_position".to_string(),
                                        format!("{x},{y},{z}"),
                                    ));
                                }
                                EntityDescriptor {
                                    name: name.clone(),
                                    components,
                                }
                            })
                        })
                        .collect();
                    let count = entities.len();
                    let scene = SceneDescriptor { entities };
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

            // find_entities_by_name
            let snap_find = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "find_entities_by_name".to_string(),
                description: "Return entities whose name contains the given query string (case-insensitive)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Substring to search for in entity names" }
                    },
                    "required": ["query"]
                })),
                handler: Box::new(move |input| {
                    let query = match input["query"].as_str() {
                        Some(q) => q.to_lowercase(),
                        None => return McpToolOutput::error("missing string 'query' field"),
                    };
                    let s = snap_find.lock().unwrap();
                    let matches: Vec<_> = s
                        .entities
                        .iter()
                        .filter(|e| {
                            e.name
                                .as_deref()
                                .map(|n| n.to_lowercase().contains(&query))
                                .unwrap_or(false)
                        })
                        .map(|e| json!({
                            "id": e.id,
                            "name": e.name,
                            "position": e.position,
                            "mesh_id": e.mesh_id,
                            "light_type": e.light_type,
                        }))
                        .collect();
                    McpToolOutput::success(json!({ "entities": matches }))
                }),
            });

            // query_entities
            let snap_query = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "query_entities".to_string(),
                description: "Filter entities by component presence or light type (all filters optional, AND-combined)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "has_mesh":     { "type": "boolean" },
                        "has_light":    { "type": "boolean" },
                        "has_position": { "type": "boolean" },
                        "light_type":   { "type": "string", "enum": ["point", "directional", "spot"] }
                    }
                })),
                handler: Box::new(move |input| {
                    let s = snap_query.lock().unwrap();
                    let results: Vec<_> = s
                        .entities
                        .iter()
                        .filter(|e| {
                            if let Some(v) = input["has_mesh"].as_bool() {
                                if e.mesh_id.is_some() != v { return false; }
                            }
                            if let Some(v) = input["has_light"].as_bool() {
                                if e.light_type.is_some() != v { return false; }
                            }
                            if let Some(v) = input["has_position"].as_bool() {
                                if e.position.is_some() != v { return false; }
                            }
                            if let Some(lt) = input["light_type"].as_str() {
                                if e.light_type.as_deref() != Some(lt) { return false; }
                            }
                            true
                        })
                        .map(|e| json!({
                            "id": e.id,
                            "name": e.name,
                            "position": e.position,
                            "mesh_id": e.mesh_id,
                            "light_type": e.light_type,
                        }))
                        .collect();
                    McpToolOutput::success(json!({ "entities": results }))
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

            // find_entities_by_tag
            let snap_tag = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "find_entities_by_tag".to_string(),
                description: "Find all entities that have a specific tag".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "tag": { "type": "string" }
                    },
                    "required": ["tag"]
                })),
                handler: Box::new(move |input| {
                    let tag = match input["tag"].as_str() {
                        Some(t) => t.to_string(),
                        None => return McpToolOutput::error("missing tag"),
                    };
                    let s = snap_tag.lock().unwrap();
                    let ids: Vec<u64> = s
                        .entities
                        .iter()
                        .filter(|e| e.tags.iter().any(|t| t == &tag))
                        .map(|e| e.id)
                        .collect();
                    McpToolOutput::success(
                        json!({"tag": tag, "entity_ids": ids, "count": ids.len()}),
                    )
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
                            a.get(0)?.as_f64()? as f32,
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

            // select_untagged_entities
            let snap_sue = snapshot.clone();
            let sel_sue = selection.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "select_untagged_entities".to_string(),
                description: "Add all entities that have no tags to the selection".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_sue.lock().unwrap();
                    let mut sel = sel_sue.lock().unwrap();
                    let mut count = 0u64;
                    for e in s.entities.iter().filter(|e| e.tags.is_empty()) {
                        sel.insert(e.id);
                        count += 1;
                    }
                    McpToolOutput::success(json!({"selected_count": count}))
                }),
            });

            // get_entities_sorted_by_y
            let snap_gesby = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_sorted_by_y".to_string(),
                description: "Return all entities sorted by Y position (ascending), entities without position come last".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_gesby.lock().unwrap();
                    let mut with_pos: Vec<(&crate::snapshot::EntityInfo, f32)> = s.entities.iter()
                        .filter_map(|e| e.position.map(|[_, ey, _]| (e, ey)))
                        .collect();
                    with_pos.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                    let entities: Vec<serde_json::Value> = with_pos.iter()
                        .map(|(e, ey)| json!({"id": e.id, "name": e.name, "position": [e.position.map(|p| p[0]).unwrap_or(0.0), ey, e.position.map(|p| p[2]).unwrap_or(0.0)]}))
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
                }),
            });

            // get_entities_below_y
            let snap_geby = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_below_y".to_string(),
                description:
                    "Return all entities whose Y position is strictly less than the given value"
                        .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "y": { "type": "number" } },
                    "required": ["y"]
                })),
                handler: Box::new(move |input| {
                    let threshold = match input["y"].as_f64() {
                        Some(v) => v as f32,
                        None => return McpToolOutput::error("missing y"),
                    };
                    let s = snap_geby.lock().unwrap();
                    let entities: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| e.position.map(|[_, ey, _]| ey < threshold).unwrap_or(false))
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
                }),
            });

            // get_entities_in_y_range
            let snap_geiyr = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_in_y_range".to_string(),
                description:
                    "Return all entities whose Y position is in [y_min, y_max] (inclusive)"
                        .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "y_min": { "type": "number" },
                        "y_max": { "type": "number" }
                    },
                    "required": ["y_min", "y_max"]
                })),
                handler: Box::new(move |input| {
                    let y_min = match input["y_min"].as_f64() {
                        Some(v) => v as f32,
                        None => return McpToolOutput::error("missing y_min"),
                    };
                    let y_max = match input["y_max"].as_f64() {
                        Some(v) => v as f32,
                        None => return McpToolOutput::error("missing y_max"),
                    };
                    let s = snap_geiyr.lock().unwrap();
                    let entities: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| {
                            e.position
                                .map(|[_, ey, _]| ey >= y_min && ey <= y_max)
                                .unwrap_or(false)
                        })
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
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

            // get_entities_above_y
            let snap_geay = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_above_y".to_string(),
                description:
                    "Return all entities whose Y position is strictly greater than the given value"
                        .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "y": { "type": "number" } },
                    "required": ["y"]
                })),
                handler: Box::new(move |input| {
                    let threshold = match input["y"].as_f64() {
                        Some(v) => v as f32,
                        None => return McpToolOutput::error("missing y"),
                    };
                    let s = snap_geay.lock().unwrap();
                    let entities: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| e.position.map(|[_, ey, _]| ey > threshold).unwrap_or(false))
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
                }),
            });

            // get_entities_with_name_containing
            let snap_gewnc = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_with_name_containing".to_string(),
                description:
                    "Return all entities whose name contains the given substring (case-sensitive)"
                        .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "substring": { "type": "string" } },
                    "required": ["substring"]
                })),
                handler: Box::new(move |input| {
                    let sub = match input["substring"].as_str() {
                        Some(s) => s.to_string(),
                        None => return McpToolOutput::error("missing substring"),
                    };
                    let s = snap_gewnc.lock().unwrap();
                    let entities: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| {
                            e.name
                                .as_deref()
                                .map(|n| n.contains(&*sub))
                                .unwrap_or(false)
                        })
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
                }),
            });

            // get_entity_count_in_radius
            let snap_gecir = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_count_in_radius".to_string(),
                description:
                    "Return the count of entities within a given radius of a world position"
                        .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "x": { "type": "number" },
                        "y": { "type": "number" },
                        "z": { "type": "number" },
                        "radius": { "type": "number" }
                    },
                    "required": ["x", "y", "z", "radius"]
                })),
                handler: Box::new(move |input| {
                    let x = input["x"].as_f64().unwrap_or(0.0) as f32;
                    let y = input["y"].as_f64().unwrap_or(0.0) as f32;
                    let z = input["z"].as_f64().unwrap_or(0.0) as f32;
                    let radius = match input["radius"].as_f64() {
                        Some(r) => r as f32,
                        None => return McpToolOutput::error("missing radius"),
                    };
                    let r2 = radius * radius;
                    let s = snap_gecir.lock().unwrap();
                    let count = s
                        .entities
                        .iter()
                        .filter(|e| {
                            if let Some([ex, ey, ez]) = e.position {
                                let dx = ex - x;
                                let dy = ey - y;
                                let dz = ez - z;
                                dx * dx + dy * dy + dz * dz <= r2
                            } else {
                                false
                            }
                        })
                        .count() as u64;
                    McpToolOutput::success(json!({"count": count}))
                }),
            });

            // get_entities_near_position
            let snap_genp = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_near_position".to_string(),
                description: "Return all entities within a given radius of a world position"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "x": { "type": "number" },
                        "y": { "type": "number" },
                        "z": { "type": "number" },
                        "radius": { "type": "number" }
                    },
                    "required": ["x", "y", "z", "radius"]
                })),
                handler: Box::new(move |input| {
                    let x = input["x"].as_f64().unwrap_or(0.0) as f32;
                    let y = input["y"].as_f64().unwrap_or(0.0) as f32;
                    let z = input["z"].as_f64().unwrap_or(0.0) as f32;
                    let radius = match input["radius"].as_f64() {
                        Some(r) => r as f32,
                        None => return McpToolOutput::error("missing radius"),
                    };
                    let r2 = radius * radius;
                    let s = snap_genp.lock().unwrap();
                    let entities: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter_map(|e| {
                            let [ex, ey, ez] = e.position?;
                            let dx = ex - x;
                            let dy = ey - y;
                            let dz = ez - z;
                            if dx * dx + dy * dy + dz * dz <= r2 {
                                Some(json!({"id": e.id, "name": e.name}))
                            } else {
                                None
                            }
                        })
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
                }),
            });

            // find_entity_by_name
            let snap_febn = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "find_entity_by_name".to_string(),
                description: "Find the first entity whose name exactly matches the given string"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "name": { "type": "string" } },
                    "required": ["name"]
                })),
                handler: Box::new(move |input| {
                    let name = match input["name"].as_str() {
                        Some(n) => n.to_string(),
                        None => return McpToolOutput::error("missing name"),
                    };
                    let s = snap_febn.lock().unwrap();
                    match s.entities.iter().find(|e| e.name.as_deref() == Some(&name)) {
                        Some(e) => McpToolOutput::success(json!({
                            "found": true,
                            "entity": {"id": e.id, "name": e.name}
                        })),
                        None => McpToolOutput::success(json!({"found": false, "entity": null})),
                    }
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

            // get_entity_position_distance
            let snap_gepd = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_position_distance".to_string(),
                description: "Return the Euclidean distance between two entities' positions"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id_a": { "type": "integer" },
                        "entity_id_b": { "type": "integer" }
                    },
                    "required": ["entity_id_a", "entity_id_b"]
                })),
                handler: Box::new(move |input| {
                    let id_a = match input["entity_id_a"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id_a"),
                    };
                    let id_b = match input["entity_id_b"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id_b"),
                    };
                    let s = snap_gepd.lock().unwrap();
                    let pos_a = match s
                        .entities
                        .iter()
                        .find(|e| e.id == id_a)
                        .and_then(|e| e.position)
                    {
                        Some(p) => p,
                        None => return McpToolOutput::error("entity A has no position"),
                    };
                    let pos_b = match s
                        .entities
                        .iter()
                        .find(|e| e.id == id_b)
                        .and_then(|e| e.position)
                    {
                        Some(p) => p,
                        None => return McpToolOutput::error("entity B has no position"),
                    };
                    let dx = pos_b[0] - pos_a[0];
                    let dy = pos_b[1] - pos_a[1];
                    let dz = pos_b[2] - pos_a[2];
                    let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                    McpToolOutput::success(json!({"distance": dist}))
                }),
            });

            // select_entities_with_tag
            let snap_sewt = snapshot.clone();
            let sel_sewt = selection.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "select_entities_with_tag".to_string(),
                description: "Add all entities that have a specific tag to the selection"
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
                    let s = snap_sewt.lock().unwrap();
                    let mut sel = sel_sewt.lock().unwrap();
                    let mut count = 0u64;
                    for e in s.entities.iter().filter(|e| e.tags.contains(&tag)) {
                        sel.insert(e.id);
                        count += 1;
                    }
                    McpToolOutput::success(json!({"tag": tag, "selected_count": count}))
                }),
            });

            // get_all_unique_tags
            let snap_gaut = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_all_unique_tags".to_string(),
                description: "Return the sorted list of all distinct tags used across all entities"
                    .to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_gaut.lock().unwrap();
                    let mut seen = std::collections::HashSet::new();
                    let mut tags: Vec<String> = Vec::new();
                    for e in &s.entities {
                        for t in &e.tags {
                            if seen.insert(t.clone()) {
                                tags.push(t.clone());
                            }
                        }
                    }
                    tags.sort();
                    let tags_json: Vec<serde_json::Value> =
                        tags.into_iter().map(|t| json!(t)).collect();
                    McpToolOutput::success(json!({"tags": tags_json}))
                }),
            });

            // get_entities_without_tag
            let snap_gewt = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_without_tag".to_string(),
                description: "Return all entities that do NOT have a specific tag".to_string(),
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
                    let s = snap_gewt.lock().unwrap();
                    let entities: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| !e.tags.contains(&tag))
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
                }),
            });

            // get_entity_ancestors
            let snap_gea = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_ancestors".to_string(),
                description: "Return all ancestors (parent, grandparent, etc.) of an entity, from immediate parent up to root".to_string(),
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
                    let s = snap_gea.lock().unwrap();
                    if !s.entities.iter().any(|e| e.id == entity_id) {
                        return McpToolOutput::error("entity not found");
                    }
                    let mut ancestors = Vec::new();
                    let mut current_id = entity_id;
                    for _ in 0..32 {
                        if let Some(e) = s.entities.iter().find(|e| e.id == current_id) {
                            match e.parent_id {
                                Some(pid) => {
                                    if let Some(parent) = s.entities.iter().find(|p| p.id == pid) {
                                        ancestors.push(json!({"id": parent.id, "name": parent.name}));
                                        current_id = pid;
                                    } else { break; }
                                }
                                None => break,
                            }
                        } else { break; }
                    }
                    McpToolOutput::success(json!({"entity_id": entity_id, "entities": ancestors}))
                }),
            });

            // get_entity_count_by_tag
            let snap_gectag = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_count_by_tag".to_string(),
                description: "Return the number of entities that have a specific tag".to_string(),
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
                    let s = snap_gectag.lock().unwrap();
                    let count = s.entities.iter().filter(|e| e.tags.contains(&tag)).count() as u64;
                    McpToolOutput::success(json!({"tag": tag, "count": count}))
                }),
            });

            // get_selected_entity_names
            let snap_gsen = snapshot.clone();
            let sel_gsen = selection.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_selected_entity_names".to_string(),
                description: "Return the names of all currently selected entities".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_gsen.lock().unwrap();
                    let sel = sel_gsen.lock().unwrap();
                    let names: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| sel.contains(&e.id))
                        .map(|e| json!(e.name.as_deref().unwrap_or("")))
                        .collect();
                    McpToolOutput::success(json!({"names": names}))
                }),
            });

            // get_visible_entity_count
            let snap_gvec = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_visible_entity_count".to_string(),
                description: "Return the number of entities with visible=true".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_gvec.lock().unwrap();
                    let count = s.entities.iter().filter(|e| e.visible).count() as u64;
                    McpToolOutput::success(json!({"count": count}))
                }),
            });

            // get_hidden_entity_count
            let snap_ghec = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_hidden_entity_count".to_string(),
                description: "Return the number of entities with visible=false".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_ghec.lock().unwrap();
                    let count = s.entities.iter().filter(|e| !e.visible).count() as u64;
                    McpToolOutput::success(json!({"count": count}))
                }),
            });

            // select_entities_with_camera
            let snap_sewcam = snapshot.clone();
            let sel_sewcam = selection.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "select_entities_with_camera".to_string(),
                description: "Add all entities that have a camera component to the selection"
                    .to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_sewcam.lock().unwrap();
                    let mut sel = sel_sewcam.lock().unwrap();
                    let mut count = 0u64;
                    for e in &s.entities {
                        if e.camera_fov.is_some() {
                            sel.insert(e.id);
                            count += 1;
                        }
                    }
                    McpToolOutput::success(json!({"selected_count": count}))
                }),
            });

            // get_entity_descendants
            let snap_ged = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_descendants".to_string(),
                description:
                    "Return all descendants (children, grandchildren, etc.) of an entity via BFS"
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
                    let s = snap_ged.lock().unwrap();
                    if !s.entities.iter().any(|e| e.id == entity_id) {
                        return McpToolOutput::error("entity not found");
                    }
                    let mut result = Vec::new();
                    let mut queue = std::collections::VecDeque::new();
                    queue.push_back(entity_id);
                    while let Some(current) = queue.pop_front() {
                        for e in s.entities.iter().filter(|e| e.parent_id == Some(current)) {
                            result.push(json!({"id": e.id, "name": e.name}));
                            queue.push_back(e.id);
                            if result.len() >= 10_000 {
                                break;
                            }
                        }
                        if result.len() >= 10_000 {
                            break;
                        }
                    }
                    McpToolOutput::success(json!({"entity_id": entity_id, "entities": result}))
                }),
            });

            // get_entities_with_children
            let snap_gewc = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_with_children".to_string(),
                description: "Return all entities that have at least one direct child".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_gewc.lock().unwrap();
                    let parent_ids: std::collections::HashSet<u64> =
                        s.entities.iter().filter_map(|e| e.parent_id).collect();
                    let ents: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| parent_ids.contains(&e.id))
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"entities": ents}))
                }),
            });

            // get_entities_near_entity
            let snap_gene = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_near_entity".to_string(),
                description:
                    "Return all entities within radius of the given entity (excluding itself)"
                        .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "integer" },
                        "radius": { "type": "number" }
                    },
                    "required": ["entity_id", "radius"]
                })),
                handler: Box::new(move |input| {
                    let entity_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let radius = match input["radius"].as_f64() {
                        Some(r) if r >= 0.0 => r as f32,
                        _ => return McpToolOutput::error("radius must be a non-negative number"),
                    };
                    let s = snap_gene.lock().unwrap();
                    let center = match s.entities.iter().find(|e| e.id == entity_id) {
                        Some(e) => e.position.unwrap_or([0.0, 0.0, 0.0]),
                        None => return McpToolOutput::error("entity not found"),
                    };
                    let ents: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| {
                            if e.id == entity_id {
                                return false;
                            }
                            let [x, y, z] = e.position.unwrap_or([0.0, 0.0, 0.0]);
                            let dx = x - center[0];
                            let dy = y - center[1];
                            let dz = z - center[2];
                            (dx * dx + dy * dy + dz * dz).sqrt() <= radius
                        })
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"entities": ents}))
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

            // get_scene_entity_count_by_type
            let snap_gsecbt = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_scene_entity_count_by_type".to_string(),
                description: "Return entity counts broken down by type: cameras, lights, mesh_entities, plain".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_gsecbt.lock().unwrap();
                    let mut cameras = 0u64;
                    let mut lights = 0u64;
                    let mut mesh_entities = 0u64;
                    let mut plain = 0u64;
                    for e in &s.entities {
                        if e.camera_fov.is_some() { cameras += 1; }
                        else if e.light_type.is_some() { lights += 1; }
                        else if e.mesh_id.is_some() { mesh_entities += 1; }
                        else { plain += 1; }
                    }
                    McpToolOutput::success(json!({
                        "total": cameras + lights + mesh_entities + plain,
                        "cameras": cameras,
                        "lights": lights,
                        "mesh_entities": mesh_entities,
                        "plain": plain,
                    }))
                }),
            });

            // get_entities_with_all_tags
            let snap_gewat = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_with_all_tags".to_string(),
                description: "Return entities that have ALL of the specified tags (AND filter)"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "tags": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["tags"]
                })),
                handler: Box::new(move |input| {
                    let tags: Vec<String> = match input["tags"].as_array() {
                        Some(arr) => arr
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect(),
                        None => return McpToolOutput::error("missing tags"),
                    };
                    let s = snap_gewat.lock().unwrap();
                    let ents: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| tags.iter().all(|t| e.tags.contains(t)))
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"entities": ents}))
                }),
            });

            // get_entities_with_any_tag
            let snap_gewanyt = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_with_any_tag".to_string(),
                description:
                    "Return entities that have AT LEAST ONE of the specified tags (OR filter)"
                        .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "tags": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["tags"]
                })),
                handler: Box::new(move |input| {
                    let tags: Vec<String> = match input["tags"].as_array() {
                        Some(arr) => arr
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect(),
                        None => return McpToolOutput::error("missing tags"),
                    };
                    let s = snap_gewanyt.lock().unwrap();
                    let ents: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| tags.iter().any(|t| e.tags.contains(t)))
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"entities": ents}))
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

            // get_all_cameras
            let snap_gac = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_all_cameras".to_string(),
                description: "Return all entities that have a camera component (camera_fov is set)"
                    .to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_gac.lock().unwrap();
                    let cams: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| e.camera_fov.is_some())
                        .map(|e| json!({"id": e.id, "name": e.name, "camera_fov": e.camera_fov}))
                        .collect();
                    McpToolOutput::success(json!({"cameras": cams}))
                }),
            });

            // get_total_light_count
            let snap_gtlc = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_total_light_count".to_string(),
                description:
                    "Return total light count broken down by type (point, directional, spot)"
                        .to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_gtlc.lock().unwrap();
                    let mut point = 0u64;
                    let mut directional = 0u64;
                    let mut spot = 0u64;
                    for e in &s.entities {
                        match e.light_type.as_deref() {
                            Some("point") => point += 1,
                            Some("directional") => directional += 1,
                            Some("spot") => spot += 1,
                            _ => {}
                        }
                    }
                    McpToolOutput::success(json!({
                        "total": point + directional + spot,
                        "point": point,
                        "directional": directional,
                        "spot": spot,
                    }))
                }),
            });

            // get_entity_child_count
            let snap_gecc = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_child_count".to_string(),
                description: "Return the number of direct children of the given entity".to_string(),
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
                    let s = snap_gecc.lock().unwrap();
                    if !s.entities.iter().any(|e| e.id == entity_id) {
                        return McpToolOutput::error("entity not found");
                    }
                    let count = s
                        .entities
                        .iter()
                        .filter(|e| e.parent_id == Some(entity_id))
                        .count() as u64;
                    McpToolOutput::success(json!({"entity_id": entity_id, "child_count": count}))
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

            // find_entities_by_name_prefix
            let snap_febnp = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "find_entities_by_name_prefix".to_string(),
                description: "Return all entities whose name starts with the given prefix"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "prefix": { "type": "string" } },
                    "required": ["prefix"]
                })),
                handler: Box::new(move |input| {
                    let prefix = match input["prefix"].as_str() {
                        Some(p) => p.to_string(),
                        None => return McpToolOutput::error("missing prefix"),
                    };
                    let s = snap_febnp.lock().unwrap();
                    let ents: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| {
                            e.name
                                .as_deref()
                                .map(|n| n.starts_with(&*prefix))
                                .unwrap_or(false)
                        })
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"entities": ents}))
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

            // get_entities_at_depth
            let snap_gead = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_at_depth".to_string(),
                description: "Return entities at a specific parent-chain depth (0 = root, 1 = first-level children, etc.)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "depth": { "type": "integer", "minimum": 0 } },
                    "required": ["depth"]
                })),
                handler: Box::new(move |input| {
                    let target_depth = match input["depth"].as_u64() {
                        Some(d) => d,
                        None => return McpToolOutput::error("missing depth"),
                    };
                    let s = snap_gead.lock().unwrap();
                    let entity_depth = |id: u64| -> u64 {
                        let mut current = id;
                        let mut depth = 0u64;
                        for _ in 0..64 {
                            match s.entities.iter().find(|e| e.id == current).and_then(|e| e.parent_id) {
                                Some(pid) => { current = pid; depth += 1; }
                                None => break,
                            }
                        }
                        depth
                    };
                    let ents: Vec<serde_json::Value> = s.entities.iter()
                        .filter(|e| entity_depth(e.id) == target_depth)
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"depth": target_depth, "entities": ents}))
                }),
            });

            // get_bounding_box
            let snap_gbb = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_bounding_box".to_string(),
                description:
                    "Return the axis-aligned bounding box (min/max) of the given entity IDs"
                        .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_ids": { "type": "array", "items": { "type": "integer" } }
                    },
                    "required": ["entity_ids"]
                })),
                handler: Box::new(move |input| {
                    let ids: Vec<u64> = match input["entity_ids"].as_array() {
                        Some(arr) => arr.iter().filter_map(|v| v.as_u64()).collect(),
                        None => return McpToolOutput::error("missing entity_ids"),
                    };
                    let s = snap_gbb.lock().unwrap();
                    let mut min = [f32::MAX; 3];
                    let mut max = [f32::MIN; 3];
                    let mut found = false;
                    for &id in &ids {
                        if let Some(e) = s.entities.iter().find(|e| e.id == id) {
                            if let Some([x, y, z]) = e.position {
                                min[0] = min[0].min(x);
                                min[1] = min[1].min(y);
                                min[2] = min[2].min(z);
                                max[0] = max[0].max(x);
                                max[1] = max[1].max(y);
                                max[2] = max[2].max(z);
                                found = true;
                            }
                        }
                    }
                    if !found {
                        return McpToolOutput::error("no entities with positions found");
                    }
                    McpToolOutput::success(json!({"min": min, "max": max}))
                }),
            });

            // select_entities_in_bounding_box
            let snap_seibb = snapshot.clone();
            let sel_seibb = selection.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "select_entities_in_bounding_box".to_string(),
                description: "Add all entities whose position is within [min_x..max_x, min_y..max_y, min_z..max_z] to selection".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "min_x": {"type": "number"}, "min_y": {"type": "number"}, "min_z": {"type": "number"},
                        "max_x": {"type": "number"}, "max_y": {"type": "number"}, "max_z": {"type": "number"}
                    },
                    "required": ["min_x","min_y","min_z","max_x","max_y","max_z"]
                })),
                handler: Box::new(move |input| {
                    let get = |key: &str| input[key].as_f64().map(|v| v as f32);
                    let (mn_x, mn_y, mn_z, mx_x, mx_y, mx_z) = match (
                        get("min_x"), get("min_y"), get("min_z"),
                        get("max_x"), get("max_y"), get("max_z"),
                    ) {
                        (Some(a), Some(b), Some(c), Some(d), Some(e), Some(f)) => (a,b,c,d,e,f),
                        _ => return McpToolOutput::error("missing bounding box parameters"),
                    };
                    let s = snap_seibb.lock().unwrap();
                    let mut sel = sel_seibb.lock().unwrap();
                    let mut count = 0u64;
                    for e in &s.entities {
                        if let Some([x, y, z]) = e.position {
                            if x >= mn_x && x <= mx_x && y >= mn_y && y <= mx_y && z >= mn_z && z <= mx_z {
                                sel.insert(e.id);
                                count += 1;
                            }
                        }
                    }
                    McpToolOutput::success(json!({"selected_count": count}))
                }),
            });

            // get_entities_by_light_type
            let snap_geblt = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_by_light_type".to_string(),
                description: "Return all entities whose light_type matches the given value (point, directional, spot)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "light_type": { "type": "string" } },
                    "required": ["light_type"]
                })),
                handler: Box::new(move |input| {
                    let lt = match input["light_type"].as_str() {
                        Some(s) => s.to_string(),
                        None => return McpToolOutput::error("missing light_type"),
                    };
                    let s = snap_geblt.lock().unwrap();
                    let ents: Vec<serde_json::Value> = s.entities.iter()
                        .filter(|e| e.light_type.as_deref() == Some(lt.as_str()))
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"entities": ents}))
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

            // get_entity_component_list
            let snap_gecl = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_component_list".to_string(),
                description: "Return which logical components an entity has (mesh_renderer, point_light, etc.)".to_string(),
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
                    let s = snap_gecl.lock().unwrap();
                    match s.entities.iter().find(|e| e.id == entity_id) {
                        Some(e) => {
                            let mut comps: Vec<&str> = vec!["entity"];
                            if e.position.is_some() { comps.push("transform"); }
                            if e.mesh_id.is_some() { comps.push("mesh_renderer"); }
                            if e.camera_fov.is_some() { comps.push("camera"); }
                            if let Some(ref lt) = e.light_type {
                                match lt.as_str() {
                                    "point" => comps.push("point_light"),
                                    "directional" => comps.push("directional_light"),
                                    "spot" => comps.push("spot_light"),
                                    _ => comps.push("light"),
                                }
                            }
                            if !e.tags.is_empty() { comps.push("tags"); }
                            McpToolOutput::success(json!({"entity_id": entity_id, "components": comps}))
                        }
                        None => McpToolOutput::error("entity not found"),
                    }
                }),
            });

            // get_entity_average_position
            let snap_geap = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_average_position".to_string(),
                description: "Return the centroid (average position) of the specified entity IDs"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_ids": { "type": "array", "items": { "type": "integer" } }
                    },
                    "required": ["entity_ids"]
                })),
                handler: Box::new(move |input| {
                    let ids: Vec<u64> = match input["entity_ids"].as_array() {
                        Some(arr) => arr.iter().filter_map(|v| v.as_u64()).collect(),
                        None => return McpToolOutput::error("missing entity_ids"),
                    };
                    if ids.is_empty() {
                        return McpToolOutput::error("entity_ids is empty");
                    }
                    let s = snap_geap.lock().unwrap();
                    let mut sum = [0.0f32; 3];
                    let mut count = 0u32;
                    for &id in &ids {
                        if let Some(e) = s.entities.iter().find(|e| e.id == id) {
                            if let Some([x, y, z]) = e.position {
                                sum[0] += x;
                                sum[1] += y;
                                sum[2] += z;
                                count += 1;
                            }
                        }
                    }
                    if count == 0 {
                        return McpToolOutput::error("no entities with positions found");
                    }
                    let avg = [
                        sum[0] / count as f32,
                        sum[1] / count as f32,
                        sum[2] / count as f32,
                    ];
                    McpToolOutput::success(json!({"position": avg}))
                }),
            });

            // select_entities_with_mesh
            let snap_sewm = snapshot.clone();
            let sel_sewm = selection.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "select_entities_with_mesh".to_string(),
                description: "Add all entities that have a mesh_id to the selection".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_sewm.lock().unwrap();
                    let mut sel = sel_sewm.lock().unwrap();
                    let mut count = 0u64;
                    for e in &s.entities {
                        if e.mesh_id.is_some() {
                            sel.insert(e.id);
                            count += 1;
                        }
                    }
                    McpToolOutput::success(json!({"selected_count": count}))
                }),
            });

            // get_entity_distance_to_origin
            let snap_gedto = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_distance_to_origin".to_string(),
                description:
                    "Return the Euclidean distance from the entity's position to the world origin"
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
                    let s = snap_gedto.lock().unwrap();
                    match s.entities.iter().find(|e| e.id == entity_id) {
                        Some(e) => {
                            let [x, y, z] = e.position.unwrap_or([0.0, 0.0, 0.0]);
                            let dist = (x * x + y * y + z * z).sqrt();
                            McpToolOutput::success(
                                json!({"entity_id": entity_id, "distance": dist}),
                            )
                        }
                        None => McpToolOutput::error("entity not found"),
                    }
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

            // get_entities_sorted_by_name
            let snap_gesbn = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_sorted_by_name".to_string(),
                description: "Return all entities sorted alphabetically by name".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_gesbn.lock().unwrap();
                    let mut entities: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    entities.sort_by(|a, b| {
                        let na = a["name"].as_str().unwrap_or("");
                        let nb = b["name"].as_str().unwrap_or("");
                        na.cmp(nb)
                    });
                    McpToolOutput::success(json!({"entities": entities}))
                }),
            });

            // get_entity_subtree_size
            let snap_gests = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_subtree_size".to_string(),
                description: "Count entity + all its descendants (BFS through parent_id links)"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "entity_id": { "type": "integer" } },
                    "required": ["entity_id"]
                })),
                handler: Box::new(move |input| {
                    let root_id = match input["entity_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id"),
                    };
                    let s = snap_gests.lock().unwrap();
                    let mut queue = vec![root_id];
                    let mut count = 0u64;
                    while let Some(current) = queue.pop() {
                        count += 1;
                        for e in &s.entities {
                            if e.parent_id == Some(current) {
                                queue.push(e.id);
                            }
                        }
                        if count > 10_000 {
                            break;
                        }
                    }
                    McpToolOutput::success(json!({"entity_id": root_id, "size": count}))
                }),
            });

            // get_entity_sibling_count
            let snap_gesc = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_sibling_count".to_string(),
                description: "Return the number of siblings (same parent, excluding self)"
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
                    let s = snap_gesc.lock().unwrap();
                    let parent_id = match s.entities.iter().find(|e| e.id == entity_id) {
                        Some(e) => e.parent_id,
                        None => return McpToolOutput::error("entity not found"),
                    };
                    let count = s
                        .entities
                        .iter()
                        .filter(|e| {
                            e.id != entity_id && e.parent_id == parent_id && parent_id.is_some()
                        })
                        .count() as u64;
                    McpToolOutput::success(json!({"entity_id": entity_id, "sibling_count": count}))
                }),
            });

            // get_entity_parent_chain
            let snap_gepc = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_parent_chain".to_string(),
                description:
                    "Return the ancestry chain [self, parent, grandparent, ...] up to root"
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
                    let s = snap_gepc.lock().unwrap();
                    let mut chain: Vec<serde_json::Value> = Vec::new();
                    let mut current_id = Some(entity_id);
                    while let Some(cid) = current_id {
                        match s.entities.iter().find(|e| e.id == cid) {
                            Some(e) => {
                                chain.push(json!({"id": e.id, "name": e.name}));
                                current_id = e.parent_id;
                            }
                            None => break,
                        }
                        if chain.len() > 64 {
                            break;
                        }
                    }
                    McpToolOutput::success(json!({"chain": chain}))
                }),
            });

            // get_entity_tags
            let snap_get = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_tags".to_string(),
                description: "Return the list of tags attached to the specified entity".to_string(),
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
                    let s = snap_get.lock().unwrap();
                    match s.entities.iter().find(|e| e.id == entity_id) {
                        Some(e) => {
                            let tags: Vec<serde_json::Value> = e
                                .tags
                                .iter()
                                .map(|t| serde_json::Value::String(t.clone()))
                                .collect();
                            McpToolOutput::success(json!({"entity_id": entity_id, "tags": tags}))
                        }
                        None => McpToolOutput::error("entity not found"),
                    }
                }),
            });

            // get_entities_with_no_tags
            let snap_gewnt = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_with_no_tags".to_string(),
                description: "Return all entities that have zero tags".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_gewnt.lock().unwrap();
                    let entities: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| e.tags.is_empty())
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
                }),
            });

            // select_entities_in_radius
            let snap_seir = snapshot.clone();
            let sel_seir = selection.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "select_entities_in_radius".to_string(),
                description: "Add all entities within radius of (cx,cy,cz) to the selection"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "cx": { "type": "number" },
                        "cy": { "type": "number" },
                        "cz": { "type": "number" },
                        "radius": { "type": "number" }
                    },
                    "required": ["cx", "cy", "cz", "radius"]
                })),
                handler: Box::new(move |input| {
                    let cx = input["cx"].as_f64().unwrap_or(0.0) as f32;
                    let cy = input["cy"].as_f64().unwrap_or(0.0) as f32;
                    let cz = input["cz"].as_f64().unwrap_or(0.0) as f32;
                    let radius = input["radius"].as_f64().unwrap_or(0.0) as f32;
                    let s = snap_seir.lock().unwrap();
                    let mut sel = sel_seir.lock().unwrap();
                    let mut count = 0u64;
                    for e in &s.entities {
                        if let Some([x, y, z]) = e.position {
                            let dx = x - cx;
                            let dy = y - cy;
                            let dz = z - cz;
                            if (dx * dx + dy * dy + dz * dz).sqrt() <= radius {
                                sel.insert(e.id);
                                count += 1;
                            }
                        }
                    }
                    McpToolOutput::success(json!({"selected_count": count}))
                }),
            });

            // is_entity_selected
            let sel_ies = selection.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "is_entity_selected".to_string(),
                description: "Return whether the specified entity is currently selected"
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
                    let selected = sel_ies.lock().unwrap().contains(&entity_id);
                    McpToolOutput::success(json!({"entity_id": entity_id, "selected": selected}))
                }),
            });

            // get_entities_in_radius
            let snap_geir = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_in_radius".to_string(),
                description:
                    "Return entities within Euclidean distance 'radius' of point (cx,cy,cz)"
                        .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "cx": { "type": "number" },
                        "cy": { "type": "number" },
                        "cz": { "type": "number" },
                        "radius": { "type": "number" }
                    },
                    "required": ["cx", "cy", "cz", "radius"]
                })),
                handler: Box::new(move |input| {
                    let cx = input["cx"].as_f64().unwrap_or(0.0) as f32;
                    let cy = input["cy"].as_f64().unwrap_or(0.0) as f32;
                    let cz = input["cz"].as_f64().unwrap_or(0.0) as f32;
                    let radius = input["radius"].as_f64().unwrap_or(0.0) as f32;
                    let s = snap_geir.lock().unwrap();
                    let entities: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter_map(|e| {
                            let [x, y, z] = e.position?;
                            let dx = x - cx;
                            let dy = y - cy;
                            let dz = z - cz;
                            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                            if dist <= radius {
                                Some(json!({"id": e.id, "name": e.name, "distance": dist}))
                            } else {
                                None
                            }
                        })
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
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

            // get_entities_not_visible
            let snap_genv = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_not_visible".to_string(),
                description: "Return all entities whose visible flag is false".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_genv.lock().unwrap();
                    let entities: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| !e.visible)
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
                }),
            });

            // get_entity_camera_info
            let snap_geci = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_camera_info".to_string(),
                description: "Return camera details (fov_y_degrees) for the specified entity"
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
                    let s = snap_geci.lock().unwrap();
                    match s.entities.iter().find(|e| e.id == entity_id) {
                        Some(e) => McpToolOutput::success(json!({
                            "entity_id": entity_id,
                            "fov_y_degrees": e.camera_fov,
                        })),
                        None => McpToolOutput::error("entity not found"),
                    }
                }),
            });

            // get_entity_light_info
            let snap_geli = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_light_info".to_string(),
                description:
                    "Return light details (type, color, intensity, range) for the specified entity"
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
                    let s = snap_geli.lock().unwrap();
                    match s.entities.iter().find(|e| e.id == entity_id) {
                        Some(e) => McpToolOutput::success(json!({
                            "entity_id": entity_id,
                            "light_type": e.light_type,
                            "light_color": e.light_color,
                            "intensity": e.light_intensity,
                            "range": e.light_range,
                        })),
                        None => McpToolOutput::error("entity not found"),
                    }
                }),
            });

            // get_entity_children
            let snap_gec = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_children".to_string(),
                description: "Return direct children of the specified entity".to_string(),
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
                    let s = snap_gec.lock().unwrap();
                    let children: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| e.parent_id == Some(entity_id))
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"children": children}))
                }),
            });

            // get_entities_named
            let snap_gen = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_named".to_string(),
                description: "Return all entities whose name exactly matches the given string"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "name": { "type": "string" } },
                    "required": ["name"]
                })),
                handler: Box::new(move |input| {
                    let target = match input["name"].as_str() {
                        Some(n) => n.to_string(),
                        None => return McpToolOutput::error("missing name"),
                    };
                    let s = snap_gen.lock().unwrap();
                    let entities: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| e.name.as_deref() == Some(&target))
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
                }),
            });

            // get_scene_entity_names
            let snap_gsen = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_scene_entity_names".to_string(),
                description: "Return a flat list of all entity names in the scene".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_gsen.lock().unwrap();
                    let names: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter_map(|e| {
                            e.name
                                .as_ref()
                                .map(|n| serde_json::Value::String(n.clone()))
                        })
                        .collect();
                    McpToolOutput::success(json!({"names": names}))
                }),
            });

            // get_entity_mesh_id
            let snap_gemi = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_mesh_id".to_string(),
                description: "Return the mesh_id of the specified entity, or null if none"
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
                    let s = snap_gemi.lock().unwrap();
                    match s.entities.iter().find(|e| e.id == entity_id) {
                        Some(e) => McpToolOutput::success(
                            json!({"mesh_id": e.mesh_id, "entity_id": entity_id}),
                        ),
                        None => McpToolOutput::error("entity not found"),
                    }
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

            // get_entities_by_mesh_id
            let snap_gebm = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_by_mesh_id".to_string(),
                description: "Return all entities that have the specified mesh_id".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "mesh_id": { "type": "integer" } },
                    "required": ["mesh_id"]
                })),
                handler: Box::new(move |input| {
                    let mesh_id = match input["mesh_id"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing mesh_id"),
                    };
                    let s = snap_gebm.lock().unwrap();
                    let entities: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| e.mesh_id == Some(mesh_id))
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
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

            // get_entity_position
            let snap_gep = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_position".to_string(),
                description: "Return only the position [x,y,z] of the specified entity".to_string(),
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
                    let s = snap_gep.lock().unwrap();
                    match s.entities.iter().find(|e| e.id == entity_id) {
                        Some(e) => McpToolOutput::success(
                            json!({"position": e.position, "entity_id": entity_id}),
                        ),
                        None => McpToolOutput::error("entity not found"),
                    }
                }),
            });

            // get_entities_with_mesh
            let snap_gewm = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_with_mesh".to_string(),
                description: "Return all entities that have a mesh renderer attached".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_gewm.lock().unwrap();
                    let entities: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| e.mesh_id.is_some())
                        .map(|e| json!({"id": e.id, "name": e.name, "mesh_id": e.mesh_id}))
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
                }),
            });

            // get_entity_rotation
            let snap_ger = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_rotation".to_string(),
                description:
                    "Return the rotation [rx, ry, rz] (Euler degrees) of the specified entity"
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
                    let s = snap_ger.lock().unwrap();
                    match s.entities.iter().find(|e| e.id == entity_id) {
                        Some(e) => McpToolOutput::success(
                            json!({"rotation": e.rotation, "entity_id": entity_id}),
                        ),
                        None => McpToolOutput::error("entity not found"),
                    }
                }),
            });

            // get_entity_scale
            let snap_ges = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_scale".to_string(),
                description: "Return the scale [sx, sy, sz] of the specified entity".to_string(),
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
                    let s = snap_ges.lock().unwrap();
                    match s.entities.iter().find(|e| e.id == entity_id) {
                        Some(e) => McpToolOutput::success(
                            json!({"scale": e.scale, "entity_id": entity_id}),
                        ),
                        None => McpToolOutput::error("entity not found"),
                    }
                }),
            });

            // get_selected_entity_count
            let sel_gsec = selection.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_selected_entity_count".to_string(),
                description: "Return the number of currently selected entities".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let count = sel_gsec.lock().unwrap().len();
                    McpToolOutput::success(json!({"count": count}))
                }),
            });

            // get_entities_sorted_by_distance
            let snap_gesd = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_sorted_by_distance".to_string(),
                description: "Return all entities with a transform sorted by ascending distance from the given point".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "x": { "type": "number" },
                        "y": { "type": "number" },
                        "z": { "type": "number" }
                    },
                    "required": ["x", "y", "z"]
                })),
                handler: Box::new(move |input| {
                    let px = input["x"].as_f64().unwrap_or(0.0) as f32;
                    let py = input["y"].as_f64().unwrap_or(0.0) as f32;
                    let pz = input["z"].as_f64().unwrap_or(0.0) as f32;
                    let s = snap_gesd.lock().unwrap();
                    let mut items: Vec<(f32, &crate::snapshot::EntityInfo)> = s.entities.iter()
                        .filter_map(|e| {
                            let [ex, ey, ez] = e.position?;
                            let dx = ex-px; let dy = ey-py; let dz = ez-pz;
                            Some((dx*dx + dy*dy + dz*dz, e))
                        })
                        .collect();
                    items.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                    let entities: Vec<serde_json::Value> = items.iter()
                        .map(|(dist_sq, e)| json!({"id": e.id, "name": e.name, "distance": (*dist_sq as f64).sqrt()}))
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
                }),
            });

            // get_farthest_entity
            let snap_gfe = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_farthest_entity".to_string(),
                description: "Return the entity farthest from the given world position (only considers entities with a transform)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "x": { "type": "number" },
                        "y": { "type": "number" },
                        "z": { "type": "number" }
                    },
                    "required": ["x", "y", "z"]
                })),
                handler: Box::new(move |input| {
                    let px = input["x"].as_f64().unwrap_or(0.0) as f32;
                    let py = input["y"].as_f64().unwrap_or(0.0) as f32;
                    let pz = input["z"].as_f64().unwrap_or(0.0) as f32;
                    let s = snap_gfe.lock().unwrap();
                    let farthest = s.entities.iter()
                        .filter_map(|e| {
                            let [ex, ey, ez] = e.position?;
                            let dx = ex - px; let dy = ey - py; let dz = ez - pz;
                            Some((e, dx*dx + dy*dy + dz*dz))
                        })
                        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                    match farthest {
                        Some((e, dist_sq)) => McpToolOutput::success(json!({
                            "entity": {"id": e.id, "name": e.name, "position": e.position},
                            "distance": (dist_sq as f64).sqrt()
                        })),
                        None => McpToolOutput::error("no entities with position found"),
                    }
                }),
            });

            // get_nearest_entity
            let snap_gne = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_nearest_entity".to_string(),
                description: "Return the entity closest to the given world position (only considers entities with a transform)".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "x": { "type": "number" },
                        "y": { "type": "number" },
                        "z": { "type": "number" }
                    },
                    "required": ["x", "y", "z"]
                })),
                handler: Box::new(move |input| {
                    let px = input["x"].as_f64().unwrap_or(0.0) as f32;
                    let py = input["y"].as_f64().unwrap_or(0.0) as f32;
                    let pz = input["z"].as_f64().unwrap_or(0.0) as f32;
                    let s = snap_gne.lock().unwrap();
                    let nearest = s.entities.iter()
                        .filter_map(|e| {
                            let [ex, ey, ez] = e.position?;
                            let dx = ex - px; let dy = ey - py; let dz = ez - pz;
                            Some((e, dx*dx + dy*dy + dz*dz))
                        })
                        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                    match nearest {
                        Some((e, dist_sq)) => McpToolOutput::success(json!({
                            "entity": {"id": e.id, "name": e.name, "position": e.position},
                            "distance": (dist_sq as f64).sqrt()
                        })),
                        None => McpToolOutput::error("no entities with position found"),
                    }
                }),
            });

            // get_entity_count_by_type
            let snap_gect = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_count_by_type".to_string(),
                description: "Return entity counts broken down by type: mesh, camera, light, generic, and total".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_gect.lock().unwrap();
                    let mut mesh = 0u64;
                    let mut camera = 0u64;
                    let mut light = 0u64;
                    let mut generic = 0u64;
                    for e in &s.entities {
                        if e.camera_fov.is_some() { camera += 1; }
                        else if e.light_type.is_some() { light += 1; }
                        else if e.mesh_id.is_some() { mesh += 1; }
                        else { generic += 1; }
                    }
                    McpToolOutput::success(json!({"mesh": mesh, "camera": camera, "light": light, "generic": generic, "total": s.entities.len()}))
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

            // get_entities_with_tag
            let snap_gewt = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_with_tag".to_string(),
                description: "Return all entities that have the specified tag".to_string(),
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
                    let s = snap_gewt.lock().unwrap();
                    let entities: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| e.tags.iter().any(|t| t == &tag))
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
                }),
            });

            // count_entities
            let snap_ce = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "count_entities".to_string(),
                description: "Return the total number of entities in the scene".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_ce.lock().unwrap();
                    McpToolOutput::success(json!({"count": s.entities.len()}))
                }),
            });

            // get_hidden_entities
            let snap_ghe = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_hidden_entities".to_string(),
                description: "Return all entities whose visible flag is false".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_ghe.lock().unwrap();
                    let entities: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| !e.visible)
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
                }),
            });

            // get_entities_without_parent
            let snap_gnp = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_without_parent".to_string(),
                description: "Return all root-level entities (those with no parent)".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_gnp.lock().unwrap();
                    let entities: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| e.parent_id.is_none())
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
                }),
            });

            // get_visible_entities
            let snap_gve = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_visible_entities".to_string(),
                description: "Return all entities whose visible flag is true".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_gve.lock().unwrap();
                    let entities: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| e.visible)
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
                }),
            });

            // select_entities_by_name_pattern
            let snap_sep = snapshot.clone();
            let sel_sep = selection.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "select_entities_by_name_pattern".to_string(),
                description: "Select all entities whose name contains the given pattern (case-sensitive substring match); replaces current selection".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "pattern": { "type": "string" } },
                    "required": ["pattern"]
                })),
                handler: Box::new(move |input| {
                    let pattern = match input["pattern"].as_str() {
                        Some(p) => p.to_string(),
                        None => return McpToolOutput::error("missing pattern"),
                    };
                    let s = snap_sep.lock().unwrap();
                    let matched_ids: Vec<u64> = s.entities.iter()
                        .filter(|e| e.name.as_deref().map(|n| n.contains(&*pattern)).unwrap_or(false))
                        .map(|e| e.id)
                        .collect();
                    let matched_count = matched_ids.len();
                    let mut sel = sel_sep.lock().unwrap();
                    sel.clear();
                    for id in matched_ids { sel.insert(id); }
                    McpToolOutput::success(json!({"matched_count": matched_count}))
                }),
            });

            // get_scene_center
            let snap_gsc = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_scene_center".to_string(),
                description: "Return the average position of all entities that have a transform"
                    .to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_gsc.lock().unwrap();
                    let positions: Vec<[f32; 3]> =
                        s.entities.iter().filter_map(|e| e.position).collect();
                    if positions.is_empty() {
                        return McpToolOutput::success(
                            json!({"center": [0.0, 0.0, 0.0], "count": 0}),
                        );
                    }
                    let n = positions.len() as f32;
                    let sum = positions.iter().fold([0.0f32; 3], |acc, &p| {
                        [acc[0] + p[0], acc[1] + p[1], acc[2] + p[2]]
                    });
                    let center = [sum[0] / n, sum[1] / n, sum[2] / n];
                    McpToolOutput::success(json!({"center": center, "count": positions.len()}))
                }),
            });

            // get_entities_in_aabb
            let snap_aabb = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_in_aabb".to_string(),
                description: "Return entities whose position falls within the given axis-aligned bounding box".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "min_x": { "type": "number" }, "min_y": { "type": "number" }, "min_z": { "type": "number" },
                        "max_x": { "type": "number" }, "max_y": { "type": "number" }, "max_z": { "type": "number" }
                    },
                    "required": ["min_x", "min_y", "min_z", "max_x", "max_y", "max_z"]
                })),
                handler: Box::new(move |input| {
                    let min_x = input["min_x"].as_f64().unwrap_or(f64::NEG_INFINITY) as f32;
                    let min_y = input["min_y"].as_f64().unwrap_or(f64::NEG_INFINITY) as f32;
                    let min_z = input["min_z"].as_f64().unwrap_or(f64::NEG_INFINITY) as f32;
                    let max_x = input["max_x"].as_f64().unwrap_or(f64::INFINITY) as f32;
                    let max_y = input["max_y"].as_f64().unwrap_or(f64::INFINITY) as f32;
                    let max_z = input["max_z"].as_f64().unwrap_or(f64::INFINITY) as f32;
                    let s = snap_aabb.lock().unwrap();
                    let entities: Vec<serde_json::Value> = s.entities.iter()
                        .filter_map(|e| {
                            let [px, py, pz] = e.position?;
                            if px >= min_x && px <= max_x && py >= min_y && py <= max_y && pz >= min_z && pz <= max_z {
                                Some(json!({"id": e.id, "name": e.name, "position": e.position}))
                            } else { None }
                        })
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

            // get_entity_full_name
            let snap_gfn = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_full_name".to_string(),
                description: "Return the full path name from root to entity joined by '/'"
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
                    let s = snap_gfn.lock().unwrap();
                    if s.entities.iter().find(|e| e.id == entity_id).is_none() {
                        return McpToolOutput::error("entity not found");
                    }
                    let mut parts: Vec<String> = Vec::new();
                    let mut current_id = entity_id;
                    for _ in 0..32 {
                        if let Some(e) = s.entities.iter().find(|e| e.id == current_id) {
                            parts.push(e.name.clone().unwrap_or_else(|| format!("#{}", e.id)));
                            match e.parent_id {
                                Some(pid) => current_id = pid,
                                None => break,
                            }
                        } else {
                            break;
                        }
                    }
                    parts.reverse();
                    let full_name = parts.join("/");
                    McpToolOutput::success(json!({"full_name": full_name, "entity_id": entity_id}))
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

            // get_entity_is_leaf
            let snap_gil = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_is_leaf".to_string(),
                description: "Return whether the entity has no children".to_string(),
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
                    let s = snap_gil.lock().unwrap();
                    if s.entities.iter().find(|e| e.id == entity_id).is_none() {
                        return McpToolOutput::error("entity not found");
                    }
                    let has_children = s.entities.iter().any(|e| e.parent_id == Some(entity_id));
                    McpToolOutput::success(
                        json!({"is_leaf": !has_children, "entity_id": entity_id}),
                    )
                }),
            });

            // get_entities_with_camera
            let snap_gwc = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_with_camera".to_string(),
                description: "Return all entities that have a Camera component".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_gwc.lock().unwrap();
                    let entities: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| e.camera_fov.is_some())
                        .map(|e| json!({"id": e.id, "name": e.name, "camera_fov": e.camera_fov}))
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
                }),
            });

            // get_entities_with_light
            let snap_gwl = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_with_light".to_string(),
                description: "Return all entities that have any light component (point, directional, or spot)".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_gwl.lock().unwrap();
                    let entities: Vec<serde_json::Value> = s.entities.iter()
                        .filter(|e| e.light_type.is_some())
                        .map(|e| json!({
                            "id": e.id,
                            "name": e.name,
                            "light_type": e.light_type,
                            "light_color": e.light_color,
                            "light_intensity": e.light_intensity,
                            "light_range": e.light_range,
                        }))
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
                }),
            });

            // get_entity_tag_count
            let snap_gtc = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_tag_count".to_string(),
                description: "Return the number of tags attached to the given entity".to_string(),
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
                    let s = snap_gtc.lock().unwrap();
                    match s.entities.iter().find(|e| e.id == entity_id) {
                        Some(e) => McpToolOutput::success(
                            json!({"count": e.tags.len(), "entity_id": entity_id}),
                        ),
                        None => McpToolOutput::error("entity not found"),
                    }
                }),
            });

            // get_entities_without_mesh
            let snap_gwm = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_without_mesh".to_string(),
                description: "Return entities that have no MeshRenderer attached".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_gwm.lock().unwrap();
                    let entities: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| e.mesh_id.is_none())
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
                }),
            });

            // get_entity_world_position
            let snap_gwp = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_world_position".to_string(),
                description: "Return the accumulated world position by summing local positions up the parent chain (translation-only approximation)".to_string(),
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
                    let s = snap_gwp.lock().unwrap();
                    if s.entities.iter().find(|e| e.id == entity_id).is_none() {
                        return McpToolOutput::error("entity not found");
                    }
                    let mut world = [0.0f32; 3];
                    let mut current_id = entity_id;
                    for _ in 0..32 {
                        if let Some(e) = s.entities.iter().find(|e| e.id == current_id) {
                            if let Some([lx, ly, lz]) = e.position {
                                world[0] += lx; world[1] += ly; world[2] += lz;
                            }
                            match e.parent_id {
                                Some(pid) => current_id = pid,
                                None => break,
                            }
                        } else { break; }
                    }
                    McpToolOutput::success(json!({"world_position": world, "entity_id": entity_id}))
                }),
            });

            // list_tag_names
            let snap_ltn = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "list_tag_names".to_string(),
                description: "Return all unique tag names used across all entities in the scene"
                    .to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_ltn.lock().unwrap();
                    let mut seen = std::collections::HashSet::new();
                    let tags: Vec<&str> = s
                        .entities
                        .iter()
                        .flat_map(|e| e.tags.iter().map(|t| t.as_str()))
                        .filter(|&t| seen.insert(t))
                        .collect();
                    let tags_owned: Vec<String> = tags.iter().map(|&t| t.to_string()).collect();
                    McpToolOutput::success(json!({"tags": tags_owned}))
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

            // get_entity_children_count
            let snap_gcc = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_children_count".to_string(),
                description: "Return the number of direct children of an entity".to_string(),
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
                    let s = snap_gcc.lock().unwrap();
                    if s.entities.iter().find(|e| e.id == entity_id).is_none() {
                        return McpToolOutput::error("entity not found");
                    }
                    let count = s
                        .entities
                        .iter()
                        .filter(|e| e.parent_id == Some(entity_id))
                        .count();
                    McpToolOutput::success(json!({"count": count, "entity_id": entity_id}))
                }),
            });

            // select_entities_in_range
            let snap_sir = snapshot.clone();
            let sel_sir = selection.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "select_entities_in_range".to_string(),
                description:
                    "Add all entities within radius of a point to the selection (immediate)"
                        .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "cx": { "type": "number" },
                        "cy": { "type": "number" },
                        "cz": { "type": "number" },
                        "radius": { "type": "number" }
                    },
                    "required": ["cx", "cy", "cz", "radius"]
                })),
                handler: Box::new(move |input| {
                    let cx = input["cx"].as_f64().unwrap_or(0.0) as f32;
                    let cy = input["cy"].as_f64().unwrap_or(0.0) as f32;
                    let cz = input["cz"].as_f64().unwrap_or(0.0) as f32;
                    let radius = input["radius"].as_f64().unwrap_or(0.0) as f32;
                    let r2 = radius * radius;
                    let s = snap_sir.lock().unwrap();
                    let ids: Vec<u64> = s
                        .entities
                        .iter()
                        .filter_map(|e| {
                            let [px, py, pz] = e.position?;
                            let dx = px - cx;
                            let dy = py - cy;
                            let dz = pz - cz;
                            if dx * dx + dy * dy + dz * dz <= r2 {
                                Some(e.id)
                            } else {
                                None
                            }
                        })
                        .collect();
                    drop(s);
                    let count = ids.len();
                    let mut sel = sel_sir.lock().unwrap();
                    for id in ids {
                        sel.insert(id);
                    }
                    McpToolOutput::success(json!({"status": "ok", "added": count}))
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

            // get_entities_in_range
            let snap_range = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_in_range".to_string(),
                description: "Return entities whose position is within radius of the given point"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "cx": { "type": "number" },
                        "cy": { "type": "number" },
                        "cz": { "type": "number" },
                        "radius": { "type": "number" }
                    },
                    "required": ["cx", "cy", "cz", "radius"]
                })),
                handler: Box::new(move |input| {
                    let cx = input["cx"].as_f64().unwrap_or(0.0) as f32;
                    let cy = input["cy"].as_f64().unwrap_or(0.0) as f32;
                    let cz = input["cz"].as_f64().unwrap_or(0.0) as f32;
                    let radius = input["radius"].as_f64().unwrap_or(0.0) as f32;
                    let s = snap_range.lock().unwrap();
                    let r2 = radius * radius;
                    let entities: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter_map(|e| {
                            let [px, py, pz] = e.position?;
                            let dx = px - cx;
                            let dy = py - cy;
                            let dz = pz - cz;
                            if dx * dx + dy * dy + dz * dz <= r2 {
                                Some(json!({"id": e.id, "name": e.name, "position": e.position}))
                            } else {
                                None
                            }
                        })
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
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

            // get_leaf_entities
            let snap_leaf = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_leaf_entities".to_string(),
                description: "Return entities that have no children".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let s = snap_leaf.lock().unwrap();
                    let parent_ids: std::collections::HashSet<u64> =
                        s.entities.iter().filter_map(|e| e.parent_id).collect();
                    let leaves: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| !parent_ids.contains(&e.id))
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"entities": leaves}))
                }),
            });

            // get_entity_depth
            let snap_depth = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_depth".to_string(),
                description: "Return the depth of an entity in the hierarchy (0 = root)"
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
                    let s = snap_depth.lock().unwrap();
                    if s.entities.iter().find(|e| e.id == entity_id).is_none() {
                        return McpToolOutput::error("entity not found");
                    }
                    let mut depth: u32 = 0;
                    let mut current_id = entity_id;
                    for _ in 0..32 {
                        if let Some(e) = s.entities.iter().find(|e| e.id == current_id) {
                            match e.parent_id {
                                Some(pid) => {
                                    depth += 1;
                                    current_id = pid;
                                }
                                None => break,
                            }
                        } else {
                            break;
                        }
                    }
                    McpToolOutput::success(json!({"depth": depth, "entity_id": entity_id}))
                }),
            });

            // count_selected
            let sel_count = selection.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "count_selected".to_string(),
                description: "Return the number of currently selected entities".to_string(),
                input_schema: Some(json!({"type": "object", "properties": {}})),
                handler: Box::new(move |_input| {
                    let count = sel_count.lock().unwrap().len();
                    McpToolOutput::success(json!({"count": count}))
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

            // get_sibling_entities
            let snap_sib = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_sibling_entities".to_string(),
                description:
                    "Return entities that share the same parent as the given entity (excludes self)"
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
                    let s = snap_sib.lock().unwrap();
                    let parent_id = match s.entities.iter().find(|e| e.id == entity_id) {
                        Some(e) => e.parent_id,
                        None => return McpToolOutput::error("entity not found"),
                    };
                    let siblings: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| e.id != entity_id && e.parent_id == parent_id)
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"entities": siblings}))
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

            // get_entity_path
            let snap_path = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_path".to_string(),
                description: "Return the ancestor chain from root to the given entity (inclusive)"
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
                    let s = snap_path.lock().unwrap();
                    if s.entities.iter().find(|e| e.id == entity_id).is_none() {
                        return McpToolOutput::error("entity not found");
                    }
                    let mut path: Vec<serde_json::Value> = Vec::new();
                    let mut current_id = entity_id;
                    for _ in 0..32 {
                        if let Some(e) = s.entities.iter().find(|e| e.id == current_id) {
                            path.push(json!({"id": e.id, "name": e.name}));
                            match e.parent_id {
                                Some(pid) => current_id = pid,
                                None => break,
                            }
                        } else {
                            break;
                        }
                    }
                    path.reverse();
                    McpToolOutput::success(json!({"path": path}))
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

            // find_nearest_entity
            let snap_nearest = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "find_nearest_entity".to_string(),
                description: "Return the entity closest to a given (x, y, z) position".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "x": { "type": "number" },
                        "y": { "type": "number" },
                        "z": { "type": "number" }
                    },
                    "required": ["x", "y", "z"]
                })),
                handler: Box::new(move |input| {
                    let px = input["x"].as_f64().unwrap_or(0.0) as f32;
                    let py = input["y"].as_f64().unwrap_or(0.0) as f32;
                    let pz = input["z"].as_f64().unwrap_or(0.0) as f32;
                    let s = snap_nearest.lock().unwrap();
                    let nearest = s
                        .entities
                        .iter()
                        .filter_map(|e| {
                            let [ex, ey, ez] = e.position?;
                            let dx = ex - px;
                            let dy = ey - py;
                            let dz = ez - pz;
                            Some((e, (dx * dx + dy * dy + dz * dz).sqrt()))
                        })
                        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                    match nearest {
                        Some((e, dist)) => McpToolOutput::success(json!({
                            "entity": {"id": e.id, "name": e.name},
                            "distance": dist
                        })),
                        None => McpToolOutput::error("no entities with positions in scene"),
                    }
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

            // get_entity_distance
            let snap_dist = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entity_distance".to_string(),
                description: "Return the Euclidean distance between two entities' positions"
                    .to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_id_a": { "type": "integer" },
                        "entity_id_b": { "type": "integer" }
                    },
                    "required": ["entity_id_a", "entity_id_b"]
                })),
                handler: Box::new(move |input| {
                    let id_a = match input["entity_id_a"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id_a"),
                    };
                    let id_b = match input["entity_id_b"].as_u64() {
                        Some(id) => id,
                        None => return McpToolOutput::error("missing entity_id_b"),
                    };
                    let s = snap_dist.lock().unwrap();
                    let pos_a = match s
                        .entities
                        .iter()
                        .find(|e| e.id == id_a)
                        .and_then(|e| e.position)
                    {
                        Some(p) => p,
                        None => return McpToolOutput::error("entity_a has no position"),
                    };
                    let pos_b = match s
                        .entities
                        .iter()
                        .find(|e| e.id == id_b)
                        .and_then(|e| e.position)
                    {
                        Some(p) => p,
                        None => return McpToolOutput::error("entity_b has no position"),
                    };
                    let dx = pos_a[0] - pos_b[0];
                    let dy = pos_a[1] - pos_b[1];
                    let dz = pos_a[2] - pos_b[2];
                    let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                    McpToolOutput::success(json!({"distance": dist}))
                }),
            });

            // get_scene_bounds
            let snap_bounds = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_scene_bounds".to_string(),
                description: "Return the AABB (min/max xyz) of all positioned entities".to_string(),
                input_schema: Some(json!({ "type": "object" })),
                handler: Box::new(move |_input| {
                    let s = snap_bounds.lock().unwrap();
                    let positions: Vec<[f32; 3]> =
                        s.entities.iter().filter_map(|e| e.position).collect();
                    if positions.is_empty() {
                        return McpToolOutput::success(json!({"min": null, "max": null}));
                    }
                    let mut min = positions[0];
                    let mut max = positions[0];
                    for p in &positions[1..] {
                        for i in 0..3 {
                            if p[i] < min[i] {
                                min[i] = p[i];
                            }
                            if p[i] > max[i] {
                                max[i] = p[i];
                            }
                        }
                    }
                    McpToolOutput::success(json!({"min": min, "max": max}))
                }),
            });

            // get_entities_by_type
            let snap_type = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_entities_by_type".to_string(),
                description: "Filter entities by type: light, mesh, camera, or plain".to_string(),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "entity_type": {
                            "type": "string",
                            "enum": ["light", "mesh", "camera", "plain"]
                        }
                    },
                    "required": ["entity_type"]
                })),
                handler: Box::new(move |input| {
                    let entity_type = match input["entity_type"].as_str() {
                        Some(t) => t.to_string(),
                        None => return McpToolOutput::error("missing entity_type"),
                    };
                    let s = snap_type.lock().unwrap();
                    let entities: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| match entity_type.as_str() {
                            "light" => e.light_type.is_some(),
                            "mesh" => e.mesh_id.is_some(),
                            "camera" => e.camera_fov.is_some(),
                            "plain" => {
                                e.light_type.is_none()
                                    && e.mesh_id.is_none()
                                    && e.camera_fov.is_none()
                            }
                            _ => false,
                        })
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
                }),
            });

            // get_root_entities
            let snap_roots = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_root_entities".to_string(),
                description: "Return all top-level entities (no parent)".to_string(),
                input_schema: Some(json!({ "type": "object" })),
                handler: Box::new(move |_input| {
                    let s = snap_roots.lock().unwrap();
                    let entities: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| e.parent_id.is_none())
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
                }),
            });

            // get_children
            let snap_children = snapshot.clone();
            mcp.0.lock().unwrap().register(McpTool {
                name: "get_children".to_string(),
                description: "Return all direct children of an entity".to_string(),
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
                    let s = snap_children.lock().unwrap();
                    let entities: Vec<serde_json::Value> = s
                        .entities
                        .iter()
                        .filter(|e| e.parent_id == Some(entity_id))
                        .map(|e| json!({"id": e.id, "name": e.name}))
                        .collect();
                    McpToolOutput::success(json!({"entities": entities}))
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
    use super::EditorPlugin;
    use bsengine_app::new_app;
    use bsengine_core::Transform;
    use bsengine_mcp::McpPlugin;
    use bsengine_scene::Name;
    use glam::Vec3;
    use serde_json::json;

    use crate::snapshot::EditorSnapshotResource;

    #[test]
    fn editor_plugin_builds_without_panic() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.update();
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
            Transform::from_translation(Vec3::new(1.0, 2.0, 3.0)),
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
                Transform::from_translation(Vec3::new(5.0, 0.0, 0.0)),
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
                Transform::from_translation(Vec3::ZERO),
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

        let mut q = app.world_mut().query::<&bsengine_core::DirectionalLight>();
        assert!(
            q.iter(app.world()).next().is_some(),
            "no DirectionalLight spawned"
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
    fn mcp_get_entity_path_returns_slash_separated_names() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "PathRoot"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "PathMid"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "PathLeaf"}))
                .unwrap();
        }
        app.update();
        app.update();

        let (root_id, mid_id, leaf_id) = {
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
            let r = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("PathRoot"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let m = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("PathMid"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let l = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("PathLeaf"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (r, m, l)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "set_parent",
                    json!({"entity_id": mid_id, "parent_id": root_id}),
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
                    "set_parent",
                    json!({"entity_id": leaf_id, "parent_id": mid_id}),
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
            .execute("get_entity_path", json!({"entity_id": leaf_id}))
            .unwrap();
        assert!(out.is_ok());
        let path = out.content["path"].as_array().unwrap();
        let names: Vec<&str> = path.iter().filter_map(|e| e["name"].as_str()).collect();
        assert!(names.contains(&"PathRoot"), "path includes root");
        assert!(names.contains(&"PathMid"), "path includes mid");
        assert!(names.contains(&"PathLeaf"), "path includes leaf");
        // verify order: root → mid → leaf
        let root_pos = names.iter().position(|&n| n == "PathRoot").unwrap();
        let leaf_pos = names.iter().position(|&n| n == "PathLeaf").unwrap();
        assert!(root_pos < leaf_pos, "root comes before leaf");
    }

    #[test]
    fn mcp_select_entities_with_camera_selects_only_cameras() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute(
                "spawn_camera",
                json!({"fov_y_degrees": 60.0, "position": [0.0, 5.0, 0.0]}),
            )
            .unwrap();
            m.execute("spawn_entity", json!({"name": "NotCam"}))
                .unwrap();
        }
        app.update();
        app.update();

        let (cam_id, plain_id) = {
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
            let c = ents
                .iter()
                .find(|e| e["camera_fov"].as_f64().is_some())
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let p = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("NotCam"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (c, p)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entities_with_camera", json!({}))
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
        assert!(ids.contains(&cam_id), "camera selected");
        assert!(!ids.contains(&plain_id), "non-camera not selected");
    }

    #[test]
    fn mcp_select_untagged_entities_selects_only_untagged() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "Untagged1"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "Untagged2"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "Tagged"}))
                .unwrap();
        }
        app.update();
        app.update();

        let tagged_id = {
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
                .find(|e| e["name"].as_str() == Some("Tagged"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("tag_entity", json!({"entity_id": tagged_id, "tag": "role"}))
                .unwrap();
        }
        app.update();
        app.update();
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_untagged_entities", json!({}))
                .unwrap();
        }

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let sel = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_selection", json!({}))
            .unwrap();
        let selected_ids: Vec<u64> = sel.content["selected_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e.as_u64().unwrap())
            .collect();
        assert!(
            !selected_ids.contains(&tagged_id),
            "tagged entity not selected"
        );
        assert!(
            selected_ids.len() >= 2,
            "at least 2 untagged entities selected"
        );
    }

    #[test]
    fn mcp_get_entities_sorted_by_y_returns_ascending_order() {
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
                        {"name": "SortC", "position": [0.0, 10.0, 0.0]},
                        {"name": "SortA", "position": [0.0, 1.0, 0.0]},
                        {"name": "SortB", "position": [0.0, 5.0, 0.0]}
                    ]}),
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
            .execute("get_entities_sorted_by_y", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let entities = out.content["entities"].as_array().unwrap();
        let positions: Vec<f64> = entities
            .iter()
            .filter_map(|e| e["position"][1].as_f64())
            .collect();
        // verify ascending order
        for i in 1..positions.len() {
            assert!(
                positions[i] >= positions[i - 1],
                "entities sorted ascending by Y"
            );
        }
        // SortA (1) should come before SortC (10)
        let names: Vec<&str> = entities.iter().filter_map(|e| e["name"].as_str()).collect();
        let a_pos = names
            .iter()
            .position(|&n| n == "SortA")
            .unwrap_or(usize::MAX);
        let c_pos = names
            .iter()
            .position(|&n| n == "SortC")
            .unwrap_or(usize::MAX);
        assert!(a_pos < c_pos, "SortA (y=1) before SortC (y=10)");
    }

    #[test]
    fn mcp_get_entities_below_y_returns_entities_with_lower_position() {
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
                        {"name": "UnderA", "position": [0.0, -5.0, 0.0]},
                        {"name": "UnderB", "position": [0.0, -10.0, 0.0]},
                        {"name": "Above",  "position": [0.0, 10.0, 0.0]}
                    ]}),
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
            .execute("get_entities_below_y", json!({"y": 0.0}))
            .unwrap();
        assert!(out.is_ok());
        let entities = out.content["entities"].as_array().unwrap();
        let names: Vec<&str> = entities.iter().filter_map(|e| e["name"].as_str()).collect();
        assert!(names.contains(&"UnderA"), "UnderA below y=0");
        assert!(names.contains(&"UnderB"), "UnderB below y=0");
        assert!(!names.contains(&"Above"), "Above excluded");
    }

    #[test]
    fn mcp_get_entities_in_y_range_returns_entities_between_min_and_max() {
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
                        {"name": "InRange1", "position": [0.0, 3.0, 0.0]},
                        {"name": "InRange2", "position": [0.0, 7.0, 0.0]},
                        {"name": "TooHigh",  "position": [0.0, 15.0, 0.0]},
                        {"name": "TooLow",   "position": [0.0, -5.0, 0.0]}
                    ]}),
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
            .execute(
                "get_entities_in_y_range",
                json!({"y_min": 0.0, "y_max": 10.0}),
            )
            .unwrap();
        assert!(out.is_ok());
        let entities = out.content["entities"].as_array().unwrap();
        let names: Vec<&str> = entities.iter().filter_map(|e| e["name"].as_str()).collect();
        assert!(names.contains(&"InRange1"), "InRange1 at y=3");
        assert!(names.contains(&"InRange2"), "InRange2 at y=7");
        assert!(!names.contains(&"TooHigh"), "TooHigh at y=15 excluded");
        assert!(!names.contains(&"TooLow"), "TooLow at y=-5 excluded");
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
            .execute("get_entity_scale", json!({"entity_id": entity_id}))
            .unwrap();
        assert!(out.is_ok());
        let s = &out.content["scale"];
        assert!((s[0].as_f64().unwrap() - 3.0).abs() < 1e-3, "sx=3");
        assert!((s[1].as_f64().unwrap() - 3.0).abs() < 1e-3, "sy=3");
        assert!((s[2].as_f64().unwrap() - 3.0).abs() < 1e-3, "sz=3");
    }

    #[test]
    fn mcp_get_entities_above_y_returns_entities_with_higher_position() {
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
                        {"name": "High", "position": [0.0, 10.0, 0.0]},
                        {"name": "Low",  "position": [0.0,  1.0, 0.0]},
                        {"name": "Zero", "position": [0.0,  0.0, 0.0]}
                    ]}),
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
            .execute("get_entities_above_y", json!({"y": 5.0}))
            .unwrap();
        assert!(out.is_ok());
        let entities = out.content["entities"].as_array().unwrap();
        let names: Vec<&str> = entities.iter().filter_map(|e| e["name"].as_str()).collect();
        assert!(names.contains(&"High"), "High above y=5");
        assert!(!names.contains(&"Low"), "Low below y=5");
        assert!(!names.contains(&"Zero"), "Zero below y=5");
    }

    #[test]
    fn mcp_get_entities_with_name_containing_returns_matches() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "EnemySoldier"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "EnemyArcher"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "FriendlyUnit"}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "get_entities_with_name_containing",
                json!({"substring": "Enemy"}),
            )
            .unwrap();
        assert!(out.is_ok());
        let entities = out.content["entities"].as_array().unwrap();
        let names: Vec<&str> = entities.iter().filter_map(|e| e["name"].as_str()).collect();
        assert!(names.contains(&"EnemySoldier"), "EnemySoldier matched");
        assert!(names.contains(&"EnemyArcher"), "EnemyArcher matched");
        assert!(!names.contains(&"FriendlyUnit"), "FriendlyUnit excluded");
    }

    #[test]
    fn mcp_get_entity_count_in_radius_returns_count() {
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
                        {"name": "RadA", "position": [1.0, 0.0, 0.0]},
                        {"name": "RadB", "position": [2.0, 0.0, 0.0]},
                        {"name": "RadC", "position": [50.0, 0.0, 0.0]}
                    ]}),
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
            .execute(
                "get_entity_count_in_radius",
                json!({"x": 0.0, "y": 0.0, "z": 0.0, "radius": 5.0}),
            )
            .unwrap();
        assert!(out.is_ok());
        let count = out.content["count"].as_u64().unwrap();
        assert_eq!(count, 2, "2 entities within radius 5");
    }

    #[test]
    fn mcp_get_entities_near_position_returns_within_radius() {
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
                        {"name": "NearA", "position": [1.0, 0.0, 0.0]},
                        {"name": "NearB", "position": [2.0, 0.0, 0.0]},
                        {"name": "FarC",  "position": [100.0, 0.0, 0.0]}
                    ]}),
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
            .execute(
                "get_entities_near_position",
                json!({"x": 0.0, "y": 0.0, "z": 0.0, "radius": 5.0}),
            )
            .unwrap();
        assert!(out.is_ok());
        let entities = out.content["entities"].as_array().unwrap();
        let names: Vec<&str> = entities.iter().filter_map(|e| e["name"].as_str()).collect();
        assert!(names.contains(&"NearA"), "NearA within radius");
        assert!(names.contains(&"NearB"), "NearB within radius");
        assert!(!names.contains(&"FarC"), "FarC outside radius excluded");
    }

    #[test]
    fn mcp_find_entity_by_name_returns_matching_entity() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "FindMe"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "NotMe"})).unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("find_entity_by_name", json!({"name": "FindMe"}))
            .unwrap();
        assert!(out.is_ok());
        assert!(out.content["found"].as_bool().unwrap(), "entity found");
        assert_eq!(out.content["entity"]["name"].as_str().unwrap(), "FindMe");

        let out_miss = mcp
            .0
            .lock()
            .unwrap()
            .execute("find_entity_by_name", json!({"name": "NoSuchEntity"}))
            .unwrap();
        assert!(out_miss.is_ok());
        assert!(
            !out_miss.content["found"].as_bool().unwrap(),
            "missing entity not found"
        );
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
            .execute("get_entity_tags", json!({"entity_id": dst_id}))
            .unwrap();
        assert!(out.is_ok());
        let tags: Vec<&str> = out.content["tags"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t.as_str())
            .collect();
        assert!(tags.contains(&"special"), "special tag copied to dest");
    }

    #[test]
    fn mcp_get_entity_position_distance_returns_euclidean() {
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
                        {"name": "DistA", "position": [0.0, 0.0, 0.0]},
                        {"name": "DistB", "position": [3.0, 4.0, 0.0]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (da_id, db_id) = {
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
                .find(|e| e["name"].as_str() == Some("DistA"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let b = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("DistB"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (a, b)
        };

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "get_entity_position_distance",
                json!({"entity_id_a": da_id, "entity_id_b": db_id}),
            )
            .unwrap();
        assert!(out.is_ok());
        let dist = out.content["distance"].as_f64().unwrap();
        assert!(
            (dist - 5.0).abs() < 1e-3,
            "distance should be 5.0 (3-4-5 triangle), got {dist}"
        );
    }

    #[test]
    fn mcp_select_entities_with_tag_selects_matching() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "EnemyA"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "EnemyB"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "Ally"})).unwrap();
        }
        app.update();
        app.update();

        let (ea, eb) = {
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
            let e1 = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("EnemyA"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let e2 = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("EnemyB"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (e1, e2)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("tag_entity", json!({"entity_id": ea, "tag": "enemy"}))
                .unwrap();
        }
        app.update();
        app.update();
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("tag_entity", json!({"entity_id": eb, "tag": "enemy"}))
                .unwrap();
        }
        app.update();
        app.update();
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entities_with_tag", json!({"tag": "enemy"}))
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
        assert!(ids.contains(&ea), "EnemyA selected");
        assert!(ids.contains(&eb), "EnemyB selected");
    }

    #[test]
    fn mcp_get_all_unique_tags_returns_distinct_tags() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "UTA"})).unwrap();
            m.execute("spawn_entity", json!({"name": "UTB"})).unwrap();
        }
        app.update();
        app.update();

        let (uta_id, utb_id) = {
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
                .find(|e| e["name"].as_str() == Some("UTA"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let b = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("UTB"))
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
                .execute("tag_entity", json!({"entity_id": uta_id, "tag": "alpha"}))
                .unwrap();
        }
        app.update();
        app.update();
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("tag_entity", json!({"entity_id": utb_id, "tag": "beta"}))
                .unwrap();
        }
        app.update();
        app.update();
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("tag_entity", json!({"entity_id": utb_id, "tag": "alpha"}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_all_unique_tags", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let tags: Vec<&str> = out.content["tags"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t.as_str())
            .collect();
        assert!(tags.contains(&"alpha"), "alpha in tags");
        assert!(tags.contains(&"beta"), "beta in tags");
        // tags should be deduplicated
        let alpha_count = tags.iter().filter(|&&t| t == "alpha").count();
        assert_eq!(alpha_count, 1, "alpha appears once (deduplicated)");
    }

    #[test]
    fn mcp_get_entities_without_tag_excludes_tagged() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "WithTag"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "WithoutTag"}))
                .unwrap();
        }
        app.update();
        app.update();

        let tagged_id = {
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
                .find(|e| e["name"].as_str() == Some("WithTag"))
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
                    json!({"entity_id": tagged_id, "tag": "special"}),
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
            .execute("get_entities_without_tag", json!({"tag": "special"}))
            .unwrap();
        assert!(out.is_ok());
        let entities = out.content["entities"].as_array().unwrap();
        let names: Vec<&str> = entities.iter().filter_map(|e| e["name"].as_str()).collect();
        assert!(names.contains(&"WithoutTag"), "untagged entity in result");
        assert!(!names.contains(&"WithTag"), "tagged entity excluded");
    }

    #[test]
    fn mcp_get_entity_ancestors_returns_parent_chain() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "AncRoot"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "AncMid"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "AncLeaf"}))
                .unwrap();
        }
        app.update();
        app.update();

        let (root_id, mid_id, leaf_id) = {
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
            let r = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("AncRoot"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let m = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("AncMid"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let l = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("AncLeaf"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (r, m, l)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "set_parent",
                    json!({"entity_id": mid_id, "parent_id": root_id}),
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
                    "set_parent",
                    json!({"entity_id": leaf_id, "parent_id": mid_id}),
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
            .execute("get_entity_ancestors", json!({"entity_id": leaf_id}))
            .unwrap();
        assert!(out.is_ok());
        let ancestors = out.content["entities"].as_array().unwrap();
        let ids: Vec<u64> = ancestors.iter().filter_map(|e| e["id"].as_u64()).collect();
        assert!(ids.contains(&root_id), "root is ancestor of leaf");
        assert!(ids.contains(&mid_id), "mid is ancestor of leaf");
        assert!(!ids.contains(&leaf_id), "leaf is not its own ancestor");
    }

    #[test]
    fn mcp_get_entity_count_by_tag_returns_matching_count() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "TagA1"})).unwrap();
            m.execute("spawn_entity", json!({"name": "TagA2"})).unwrap();
            m.execute("spawn_entity", json!({"name": "NoTag"})).unwrap();
        }
        app.update();
        app.update();

        let (a1, a2) = {
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
            let e1 = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("TagA1"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let e2 = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("TagA2"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (e1, e2)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("tag_entity", json!({"entity_id": a1, "tag": "enemy"}))
                .unwrap();
        }
        app.update();
        app.update();
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("tag_entity", json!({"entity_id": a2, "tag": "enemy"}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity_count_by_tag", json!({"tag": "enemy"}))
            .unwrap();
        assert!(out.is_ok());
        let count = out.content["count"].as_u64().unwrap();
        assert_eq!(count, 2, "2 entities tagged enemy");

        let out_none = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity_count_by_tag", json!({"tag": "boss"}))
            .unwrap();
        assert_eq!(
            out_none.content["count"].as_u64().unwrap(),
            0,
            "no boss-tagged entities"
        );
    }

    #[test]
    fn mcp_get_selected_entity_names_returns_names_of_selected() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "SelName1"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "SelName2"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "Unselected"}))
                .unwrap();
        }
        app.update();
        app.update();

        let (sn1, sn2) = {
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
            let e1 = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("SelName1"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let e2 = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("SelName2"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (e1, e2)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("select_entity", json!({"entity_id": sn1}))
                .unwrap();
            m.execute("select_entity", json!({"entity_id": sn2}))
                .unwrap();
        }

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_selected_entity_names", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let names: Vec<&str> = out.content["names"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|n| n.as_str())
            .collect();
        assert!(names.contains(&"SelName1"), "SelName1 in selection names");
        assert!(names.contains(&"SelName2"), "SelName2 in selection names");
        assert!(!names.contains(&"Unselected"), "Unselected not in names");
    }

    #[test]
    fn mcp_get_visible_entity_count_returns_only_visible() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "VisA"})).unwrap();
            m.execute("spawn_entity", json!({"name": "VisB"})).unwrap();
            m.execute("spawn_entity", json!({"name": "Hidden"}))
                .unwrap();
        }
        app.update();
        app.update();

        let hidden_id = {
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
                .find(|e| e["name"].as_str() == Some("Hidden"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("hide_entity", json!({"entity_id": hidden_id}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_visible_entity_count", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let count = out.content["count"].as_u64().unwrap();
        assert_eq!(count, 2, "only 2 entities are visible");
    }

    #[test]
    fn mcp_get_hidden_entity_count_returns_only_hidden() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "HideMe1"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "HideMe2"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "StayVisible"}))
                .unwrap();
        }
        app.update();
        app.update();

        let (hid1, hid2) = {
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
            let h1 = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("HideMe1"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let h2 = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("HideMe2"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (h1, h2)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("hide_entity", json!({"entity_id": hid1}))
                .unwrap();
            m.execute("hide_entity", json!({"entity_id": hid2}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_hidden_entity_count", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let count = out.content["count"].as_u64().unwrap();
        assert_eq!(count, 2, "2 entities are hidden");
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
            .execute("get_entity_scale", json!({"entity_id": id_a}))
            .unwrap();
        let s = &out_a.content["scale"];
        assert!((s[0].as_f64().unwrap() - 2.0).abs() < 1e-3, "ScaleA sx=2");
        assert!((s[1].as_f64().unwrap() - 3.0).abs() < 1e-3, "ScaleA sy=3");
        assert!((s[2].as_f64().unwrap() - 0.5).abs() < 1e-3, "ScaleA sz=0.5");
    }

    #[test]
    fn mcp_get_entity_descendants_returns_all_levels() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "Grandparent"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "Parent"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "Child"})).unwrap();
        }
        app.update();
        app.update();

        let (gp_id, p_id, c_id) = {
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
            let gp = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("Grandparent"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let p = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("Parent"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let c = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("Child"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (gp, p, c)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("set_parent", json!({"entity_id": p_id, "parent_id": gp_id}))
                .unwrap();
        }
        app.update();
        app.update();
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("set_parent", json!({"entity_id": c_id, "parent_id": p_id}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity_descendants", json!({"entity_id": gp_id}))
            .unwrap();
        assert!(out.is_ok());
        let ids: Vec<u64> = out.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_u64().unwrap())
            .collect();
        assert!(ids.contains(&p_id), "Parent is a descendant");
        assert!(ids.contains(&c_id), "Child is a descendant");
        assert!(
            !ids.contains(&gp_id),
            "Grandparent itself not in descendants"
        );
    }

    #[test]
    fn mcp_get_entities_with_children_finds_parents() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "HasChild"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "ChildOf"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "Orphan"}))
                .unwrap();
        }
        app.update();
        app.update();

        let (parent_id, child_id) = {
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
            let p = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("HasChild"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let c = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("ChildOf"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (p, c)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "set_parent",
                    json!({"entity_id": child_id, "parent_id": parent_id}),
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
            .execute("get_entities_with_children", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let ids: Vec<u64> = out.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_u64().unwrap())
            .collect();
        assert!(ids.contains(&parent_id), "HasChild is a parent");
        assert!(!ids.contains(&child_id), "ChildOf is not a parent");
    }

    #[test]
    fn mcp_get_root_entities_returns_only_parentless() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "RootA"})).unwrap();
            m.execute("spawn_entity", json!({"name": "NonRootB"}))
                .unwrap();
        }
        app.update();
        app.update();

        let (root_id, child_id) = {
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
            let r = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("RootA"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let c = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("NonRootB"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (r, c)
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
            .execute("get_root_entities", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let ids: Vec<u64> = out.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_u64().unwrap())
            .collect();
        assert!(ids.contains(&root_id), "RootA has no parent → root");
        assert!(!ids.contains(&child_id), "NonRootB has parent → not root");
    }

    #[test]
    fn mcp_get_entities_near_entity_finds_neighbors() {
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
                        {"name": "Center", "position": [0.0, 0.0, 0.0]},
                        {"name": "Near",   "position": [2.0, 0.0, 0.0]},
                        {"name": "Far",    "position": [100.0, 0.0, 0.0]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let center_id = {
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
                .find(|e| e["name"].as_str() == Some("Center"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "get_entities_near_entity",
                json!({"entity_id": center_id, "radius": 5.0}),
            )
            .unwrap();
        assert!(out.is_ok());
        let names: Vec<&str> = out.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|e| e["name"].as_str())
            .collect();
        assert!(names.contains(&"Near"), "Near is within radius 5");
        assert!(!names.contains(&"Far"), "Far is outside radius 5");
        assert!(!names.contains(&"Center"), "Center itself excluded");
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
            .execute("get_entity_position", json!({"entity_id": eid}))
            .unwrap();
        let p = &pos.content["position"];
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
            .execute("get_entity_position", json!({"entity_id": id_a}))
            .unwrap();
        let pb = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity_position", json!({"entity_id": id_b}))
            .unwrap();
        assert!(
            (pa.content["position"][0].as_f64().unwrap() - 6.0).abs() < 1e-3,
            "OffA x = 1+5 = 6"
        );
        assert!(
            (pb.content["position"][0].as_f64().unwrap() - 7.0).abs() < 1e-3,
            "OffB x = 2+5 = 7"
        );
    }

    #[test]
    fn mcp_get_scene_entity_count_by_type_breaks_down_correctly() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "Plain"})).unwrap();
            m.execute(
                "spawn_camera",
                json!({"fov_y_degrees": 60.0, "position": [0.0,5.0,0.0]}),
            )
            .unwrap();
            m.execute("spawn_point_light", json!({"color":[1.0,1.0,1.0],"intensity":100.0,"range":10.0,"position":[0.0,0.0,0.0]})).unwrap();
        }
        app.update();
        app.update();
        let plain_id = {
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
                .find(|e| e["name"].as_str() == Some("Plain"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("attach_mesh", json!({"entity_id": plain_id, "mesh_id": 1}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_scene_entity_count_by_type", json!({}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["cameras"].as_u64().unwrap(), 1);
        assert_eq!(out.content["lights"].as_u64().unwrap(), 1);
        assert_eq!(out.content["mesh_entities"].as_u64().unwrap(), 1);
    }

    #[test]
    fn mcp_get_entities_with_all_tags_requires_every_tag() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "BothTags"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "OneTag"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "NoTags"}))
                .unwrap();
        }
        app.update();
        app.update();

        let (both_id, one_id) = {
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
            let b = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("BothTags"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let o = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("OneTag"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (b, o)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("tag_entity", json!({"entity_id": both_id, "tag": "alpha"}))
                .unwrap();
        }
        app.update();
        app.update();
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("tag_entity", json!({"entity_id": both_id, "tag": "beta"}))
                .unwrap();
        }
        app.update();
        app.update();
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("tag_entity", json!({"entity_id": one_id, "tag": "alpha"}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "get_entities_with_all_tags",
                json!({"tags": ["alpha", "beta"]}),
            )
            .unwrap();
        assert!(out.is_ok());
        let ids: Vec<u64> = out.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_u64().unwrap())
            .collect();
        assert!(ids.contains(&both_id), "BothTags has alpha+beta");
        assert!(!ids.contains(&one_id), "OneTag has only alpha");
    }

    #[test]
    fn mcp_get_entities_with_any_tag_matches_at_least_one() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "HasAlpha"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "HasBeta"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "HasNeither"}))
                .unwrap();
        }
        app.update();
        app.update();

        let (alpha_id, beta_id) = {
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
                .find(|e| e["name"].as_str() == Some("HasAlpha"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let b = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("HasBeta"))
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
                .execute("tag_entity", json!({"entity_id": alpha_id, "tag": "alpha"}))
                .unwrap();
        }
        app.update();
        app.update();
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("tag_entity", json!({"entity_id": beta_id, "tag": "beta"}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "get_entities_with_any_tag",
                json!({"tags": ["alpha", "beta"]}),
            )
            .unwrap();
        assert!(out.is_ok());
        let ids: Vec<u64> = out.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_u64().unwrap())
            .collect();
        assert!(ids.contains(&alpha_id), "HasAlpha matched");
        assert!(ids.contains(&beta_id), "HasBeta matched");
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
            .execute("get_entity_position", json!({"entity_id": dst_id}))
            .unwrap();
        let p = &pos.content["position"];
        assert!((p[0].as_f64().unwrap() - 7.0).abs() < 1e-3, "x=7 copied");
        assert!((p[1].as_f64().unwrap() - 3.0).abs() < 1e-3, "y=3 copied");
        assert!(
            (p[2].as_f64().unwrap() - (-2.0)).abs() < 1e-3,
            "z=-2 copied"
        );
    }

    #[test]
    fn mcp_get_entities_without_mesh_excludes_meshed() {
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
                        {"name": "HasMesh"},
                        {"name": "NoMesh1"},
                        {"name": "NoMesh2"}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let meshed_id = {
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
                .find(|e| e["name"].as_str() == Some("HasMesh"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("attach_mesh", json!({"entity_id": meshed_id, "mesh_id": 3}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entities_without_mesh", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let ents = out.content["entities"].as_array().unwrap();
        let names: Vec<&str> = ents.iter().filter_map(|e| e["name"].as_str()).collect();
        assert!(names.contains(&"NoMesh1"), "NoMesh1 in result");
        assert!(names.contains(&"NoMesh2"), "NoMesh2 in result");
        assert!(!names.contains(&"HasMesh"), "HasMesh excluded");
    }

    #[test]
    fn mcp_get_all_cameras_returns_camera_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute(
                "spawn_camera",
                json!({"fov_y_degrees": 60.0, "position": [0.0, 5.0, 0.0]}),
            )
            .unwrap();
            m.execute(
                "spawn_camera",
                json!({"fov_y_degrees": 45.0, "position": [10.0, 5.0, 0.0]}),
            )
            .unwrap();
            m.execute("spawn_entity", json!({"name": "NotCamera"}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_all_cameras", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let cams = out.content["cameras"].as_array().unwrap();
        assert_eq!(cams.len(), 2, "two cameras spawned");
        for c in cams {
            assert!(
                c["camera_fov"].as_f64().is_some(),
                "camera entry has camera_fov"
            );
        }
    }

    #[test]
    fn mcp_get_total_light_count_sums_all_light_types() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_point_light", json!({"color":[1.0,1.0,1.0],"intensity":100.0,"range":10.0,"position":[0.0,0.0,0.0]})).unwrap();
            m.execute(
                "spawn_directional_light",
                json!({"direction":[0.0,-1.0,0.0],"color":[1.0,1.0,1.0],"ambient":[0.1,0.1,0.1]}),
            )
            .unwrap();
            m.execute("spawn_spot_light", json!({"color":[1.0,1.0,1.0],"intensity":200.0,"range":15.0,"inner_angle":10.0,"outer_angle":30.0,"position":[0.0,3.0,0.0]})).unwrap();
            m.execute("spawn_entity", json!({"name": "NoLight"}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_total_light_count", json!({}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["total"].as_u64().unwrap(), 3, "3 lights total");
        assert_eq!(out.content["point"].as_u64().unwrap(), 1);
        assert_eq!(out.content["directional"].as_u64().unwrap(), 1);
        assert_eq!(out.content["spot"].as_u64().unwrap(), 1);
    }

    #[test]
    fn mcp_get_entity_child_count_returns_direct_children() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "ParentCC"}))
                .unwrap();
        }
        app.update();
        app.update();
        let parent_id = {
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
                .find(|e| e["name"].as_str() == Some("ParentCC"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "ChildCC1"}))
                .unwrap();
            m.execute("spawn_entity", json!({"name": "ChildCC2"}))
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
            for name in &["ChildCC1", "ChildCC2"] {
                let cid = ents
                    .iter()
                    .find(|e| e["name"].as_str() == Some(name))
                    .unwrap()["id"]
                    .as_u64()
                    .unwrap();
                mcp.0
                    .lock()
                    .unwrap()
                    .execute(
                        "set_parent",
                        json!({"entity_id": cid, "parent_id": parent_id}),
                    )
                    .unwrap();
            }
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity_child_count", json!({"entity_id": parent_id}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(
            out.content["child_count"].as_u64().unwrap(),
            2,
            "ParentCC has 2 direct children"
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
    fn mcp_find_entities_by_name_prefix_filters_names() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute(
                "batch_spawn",
                json!({"entities": [
                    {"name": "Enemy_01"},
                    {"name": "Enemy_02"},
                    {"name": "Player"},
                    {"name": "Enemy_Boss"}
                ]}),
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
            .execute("find_entities_by_name_prefix", json!({"prefix": "Enemy"}))
            .unwrap();
        assert!(out.is_ok());
        let ents = out.content["entities"].as_array().unwrap();
        assert_eq!(ents.len(), 3, "3 entities start with 'Enemy'");
        assert!(
            !ents.iter().any(|e| e["name"].as_str() == Some("Player")),
            "Player excluded"
        );
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
                assert_eq!(
                    e["visible"].as_bool().unwrap_or(true),
                    false,
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
            assert_eq!(
                e["visible"].as_bool().unwrap_or(false),
                true,
                "all should be visible"
            );
        }
    }

    #[test]
    fn mcp_get_entities_at_depth_returns_correct_level() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "Root"})).unwrap();
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
                .find(|e| e["name"].as_str() == Some("Root"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "Child"})).unwrap();
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
                .find(|e| e["name"].as_str() == Some("Child"))
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
        let depth0 = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entities_at_depth", json!({"depth": 0}))
            .unwrap();
        let ids0: Vec<u64> = depth0.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_u64().unwrap())
            .collect();
        assert!(ids0.contains(&root_id), "Root at depth 0");
        assert!(!ids0.contains(&child_id), "Child not at depth 0");

        let depth1 = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entities_at_depth", json!({"depth": 1}))
            .unwrap();
        let ids1: Vec<u64> = depth1.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_u64().unwrap())
            .collect();
        assert!(ids1.contains(&child_id), "Child at depth 1");
        assert!(!ids1.contains(&root_id), "Root not at depth 1");
    }

    #[test]
    fn mcp_get_bounding_box_returns_min_max() {
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
                        {"name": "BBoxA", "position": [-1.0, 2.0, 0.0]},
                        {"name": "BBoxB", "position": [3.0, -1.0, 5.0]},
                        {"name": "BBoxC", "position": [0.0, 0.0, -2.0]}
                    ]}),
                )
                .unwrap();
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

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_bounding_box", json!({"entity_ids": ids}))
            .unwrap();
        assert!(out.is_ok());
        let mn = &out.content["min"];
        let mx = &out.content["max"];
        assert!(
            (mn[0].as_f64().unwrap() - (-1.0)).abs() < 1e-3,
            "min_x = -1"
        );
        assert!(
            (mn[1].as_f64().unwrap() - (-1.0)).abs() < 1e-3,
            "min_y = -1"
        );
        assert!(
            (mn[2].as_f64().unwrap() - (-2.0)).abs() < 1e-3,
            "min_z = -2"
        );
        assert!((mx[0].as_f64().unwrap() - 3.0).abs() < 1e-3, "max_x = 3");
        assert!((mx[1].as_f64().unwrap() - 2.0).abs() < 1e-3, "max_y = 2");
        assert!((mx[2].as_f64().unwrap() - 5.0).abs() < 1e-3, "max_z = 5");
    }

    #[test]
    fn mcp_select_entities_in_bounding_box_picks_inside() {
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
                        {"name": "Inside", "position": [1.0, 1.0, 1.0]},
                        {"name": "Outside", "position": [10.0, 10.0, 10.0]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (id_in, id_out) = {
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
            let i = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("Inside"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let o = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("Outside"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (i, o)
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "select_entities_in_bounding_box",
                    json!({
                        "min_x": 0.0, "min_y": 0.0, "min_z": 0.0,
                        "max_x": 5.0, "max_y": 5.0, "max_z": 5.0
                    }),
                )
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
        assert!(ids.contains(&id_in), "Inside should be selected");
        assert!(!ids.contains(&id_out), "Outside should not be selected");
    }

    #[test]
    fn mcp_get_entities_by_light_type_filters_correctly() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_point_light", json!({"color": [1.0,1.0,1.0], "intensity": 100.0, "range": 10.0, "position": [0.0,0.0,0.0]})).unwrap();
            m.execute("spawn_point_light", json!({"color": [1.0,1.0,1.0], "intensity": 200.0, "range": 20.0, "position": [1.0,0.0,0.0]})).unwrap();
            m.execute("spawn_directional_light", json!({"direction": [0.0,-1.0,0.0], "color": [1.0,1.0,1.0], "ambient": [0.1,0.1,0.1]})).unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entities_by_light_type", json!({"light_type": "point"}))
            .unwrap();
        assert!(out.is_ok());
        let ents = out.content["entities"].as_array().unwrap();
        assert_eq!(ents.len(), 2, "two point lights");

        let out_dir = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "get_entities_by_light_type",
                json!({"light_type": "directional"}),
            )
            .unwrap();
        let dir_ents = out_dir.content["entities"].as_array().unwrap();
        assert_eq!(dir_ents.len(), 1, "one directional light");
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
            .execute("get_entity_position", json!({"entity_id": eid}))
            .unwrap();
        let p = &pos_out.content["position"];
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
    fn mcp_get_entity_component_list_reflects_attached_components() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "CompListEnt"}))
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
                .find(|e| e["name"].as_str() == Some("CompListEnt"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("attach_mesh", json!({"entity_id": eid, "mesh_id": 5}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity_component_list", json!({"entity_id": eid}))
            .unwrap();
        assert!(out.is_ok());
        let comps: Vec<&str> = out.content["components"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|c| c.as_str())
            .collect();
        assert!(
            comps.contains(&"mesh_renderer"),
            "should list mesh_renderer after attach_mesh"
        );
        assert!(!comps.contains(&"camera"), "should not list camera");
    }

    #[test]
    fn mcp_get_entity_average_position_returns_centroid() {
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
                        {"name": "AvgA", "position": [0.0, 0.0, 0.0]},
                        {"name": "AvgB", "position": [4.0, 0.0, 0.0]},
                        {"name": "AvgC", "position": [2.0, 6.0, 0.0]}
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
                .find(|e| e["name"].as_str() == Some("AvgA"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let b = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("AvgB"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let c = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("AvgC"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (a, b, c)
        };

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "get_entity_average_position",
                json!({"entity_ids": [id_a, id_b, id_c]}),
            )
            .unwrap();
        assert!(out.is_ok());
        let p = &out.content["position"];
        assert!(
            (p[0].as_f64().unwrap() - 2.0).abs() < 1e-3,
            "avg x = (0+4+2)/3 = 2"
        );
        assert!(
            (p[1].as_f64().unwrap() - 2.0).abs() < 1e-3,
            "avg y = (0+0+6)/3 = 2"
        );
        assert!((p[2].as_f64().unwrap()).abs() < 1e-3, "avg z = 0");
    }

    #[test]
    fn mcp_select_entities_with_mesh_selects_only_meshed() {
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
                        {"name": "WithMesh"},
                        {"name": "NoMesh"}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (meshed_id, unmeshed_id) = {
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
            let m = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("WithMesh"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let n = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("NoMesh"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (m, n)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("attach_mesh", json!({"entity_id": meshed_id, "mesh_id": 1}))
                .unwrap();
        }
        app.update();
        app.update();
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("select_entities_with_mesh", json!({}))
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
        assert!(ids.contains(&meshed_id), "WithMesh should be selected");
        assert!(!ids.contains(&unmeshed_id), "NoMesh should not be selected");
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
    fn mcp_get_entity_distance_to_origin_returns_distance() {
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
                        {"name": "DistEnt", "position": [3.0, 4.0, 0.0]}
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
                .find(|e| e["name"].as_str() == Some("DistEnt"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity_distance_to_origin", json!({"entity_id": eid}))
            .unwrap();
        assert!(out.is_ok());
        let dist = out.content["distance"].as_f64().unwrap();
        assert!((dist - 5.0).abs() < 1e-3, "sqrt(3^2+4^2)=5, got {}", dist);
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
            .execute("get_entity_position", json!({"entity_id": id_a}))
            .unwrap();
        let pb = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity_position", json!({"entity_id": id_b}))
            .unwrap();
        assert!(
            (pa.content["position"][0].as_f64().unwrap() - (-5.0)).abs() < 1e-3,
            "MirrorA x should be -5"
        );
        assert!(
            (pa.content["position"][1].as_f64().unwrap() - 2.0).abs() < 1e-3,
            "MirrorA y unchanged"
        );
        assert!(
            (pb.content["position"][0].as_f64().unwrap() - (-10.0)).abs() < 1e-3,
            "MirrorB x should be -10"
        );
    }

    #[test]
    fn mcp_get_entities_sorted_by_name_returns_alpha_order() {
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
                        {"name": "Zebra"},
                        {"name": "Apple"},
                        {"name": "Mango"}
                    ]}),
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
            .execute("get_entities_sorted_by_name", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let names: Vec<&str> = out.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|e| e["name"].as_str())
            .collect();
        let apple_pos = names.iter().position(|&n| n == "Apple").unwrap();
        let mango_pos = names.iter().position(|&n| n == "Mango").unwrap();
        let zebra_pos = names.iter().position(|&n| n == "Zebra").unwrap();
        assert!(apple_pos < mango_pos, "Apple before Mango");
        assert!(mango_pos < zebra_pos, "Mango before Zebra");
    }

    #[test]
    fn mcp_get_leaf_entities_returns_entities_with_no_children() {
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
                        {"name": "LeafParent"},
                        {"name": "LeafChild"},
                        {"name": "LeafOrphan"}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (parent_id, child_id, orphan_id) = {
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
            let p = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("LeafParent"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let c = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("LeafChild"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let o = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("LeafOrphan"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (p, c, o)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "set_parent",
                    json!({"entity_id": child_id, "parent_id": parent_id}),
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
            .execute("get_leaf_entities", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let ids: Vec<u64> = out.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_u64().unwrap())
            .collect();
        assert!(
            !ids.contains(&parent_id),
            "LeafParent has a child → not a leaf"
        );
        assert!(ids.contains(&child_id), "LeafChild has no children → leaf");
        assert!(
            ids.contains(&orphan_id),
            "LeafOrphan has no children → leaf"
        );
    }

    #[test]
    fn mcp_get_entity_subtree_size_counts_self_and_descendants() {
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
                        {"name": "SubRoot"},
                        {"name": "SubChildA"},
                        {"name": "SubChildB"},
                        {"name": "SubGrandChild"}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (root_id, ca_id, cb_id, gc_id) = {
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
            let r = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("SubRoot"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let ca = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("SubChildA"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let cb = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("SubChildB"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let gc = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("SubGrandChild"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (r, ca, cb, gc)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "set_parent",
                    json!({"entity_id": ca_id, "parent_id": root_id}),
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
                    "set_parent",
                    json!({"entity_id": cb_id, "parent_id": root_id}),
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
                    "set_parent",
                    json!({"entity_id": gc_id, "parent_id": ca_id}),
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
            .execute("get_entity_subtree_size", json!({"entity_id": root_id}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(
            out.content["size"].as_u64().unwrap(),
            4,
            "root + 2 children + 1 grandchild = 4"
        );
    }

    #[test]
    fn mcp_get_entity_sibling_count_counts_same_parent() {
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
                        {"name": "SibParent"},
                        {"name": "SibA"},
                        {"name": "SibB"},
                        {"name": "SibC"},
                        {"name": "Unrelated"}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (par_id, sib_a_id, sib_b_id, sib_c_id) = {
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
            let p = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("SibParent"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let a = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("SibA"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let b = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("SibB"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let c = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("SibC"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (p, a, b, c)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "set_parent",
                    json!({"entity_id": sib_a_id, "parent_id": par_id}),
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
                    "set_parent",
                    json!({"entity_id": sib_b_id, "parent_id": par_id}),
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
                    "set_parent",
                    json!({"entity_id": sib_c_id, "parent_id": par_id}),
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
            .execute("get_entity_sibling_count", json!({"entity_id": sib_a_id}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(
            out.content["sibling_count"].as_u64().unwrap(),
            2,
            "SibA has 2 siblings (SibB, SibC); itself excluded"
        );
    }

    #[test]
    fn mcp_get_entity_parent_chain_returns_ancestors() {
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
                        {"name": "GrandParent"},
                        {"name": "Parent"},
                        {"name": "Child"}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (gp_id, parent_id, child_id) = {
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
            let g = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("GrandParent"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let p = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("Parent"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let c = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("Child"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (g, p, c)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "set_parent",
                    json!({"entity_id": parent_id, "parent_id": gp_id}),
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
                    "set_parent",
                    json!({"entity_id": child_id, "parent_id": parent_id}),
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
            .execute("get_entity_parent_chain", json!({"entity_id": child_id}))
            .unwrap();
        assert!(out.is_ok());
        let chain: Vec<u64> = out.content["chain"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_u64().unwrap())
            .collect();
        assert_eq!(chain[0], child_id, "first = child itself");
        assert_eq!(chain[1], parent_id, "second = direct parent");
        assert_eq!(chain[2], gp_id, "third = grandparent");
    }

    #[test]
    fn mcp_get_entity_tags_returns_tag_list() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "TaggedEnt"}))
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
                .find(|e| e["name"].as_str() == Some("TaggedEnt"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("tag_entity", json!({"entity_id": eid, "tag": "player"}))
                .unwrap();
        }
        app.update();
        app.update();
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("tag_entity", json!({"entity_id": eid, "tag": "visible"}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity_tags", json!({"entity_id": eid}))
            .unwrap();
        assert!(out.is_ok());
        let tags: Vec<&str> = out.content["tags"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t.as_str())
            .collect();
        assert!(tags.contains(&"player"), "should have player tag");
        assert!(tags.contains(&"visible"), "should have visible tag");
        assert_eq!(tags.len(), 2);
    }

    #[test]
    fn mcp_get_entities_with_no_tags_filters_untagged() {
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
                        {"name": "NoTagEnt"},
                        {"name": "HasTagEnt"}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (no_tag_id, has_tag_id) = {
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
            let n = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("NoTagEnt"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let h = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("HasTagEnt"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (n, h)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "tag_entity",
                    json!({"entity_id": has_tag_id, "tag": "tagged"}),
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
            .execute("get_entities_with_no_tags", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let ids: Vec<u64> = out.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_u64().unwrap())
            .collect();
        assert!(
            ids.contains(&no_tag_id),
            "NoTagEnt should be in untagged list"
        );
        assert!(
            !ids.contains(&has_tag_id),
            "HasTagEnt should not be in untagged list"
        );
    }

    #[test]
    fn mcp_select_entities_in_radius_adds_to_selection() {
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
                        {"name": "RadSelNear", "position": [2.0, 0.0, 0.0]},
                        {"name": "RadSelFar",  "position": [50.0, 0.0, 0.0]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (near_id, far_id) = {
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
            let n = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("RadSelNear"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let f = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("RadSelFar"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (n, f)
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "select_entities_in_radius",
                    json!({"cx": 0.0, "cy": 0.0, "cz": 0.0, "radius": 10.0}),
                )
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
        assert!(ids.contains(&near_id), "RadSelNear should be selected");
        assert!(!ids.contains(&far_id), "RadSelFar should not be selected");
    }

    #[test]
    fn mcp_is_entity_selected_returns_correct_status() {
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
                        {"name": "IsSelA"},
                        {"name": "IsSelB"}
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
                .find(|e| e["name"].as_str() == Some("IsSelA"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let b = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("IsSelB"))
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
                .execute("select_entity", json!({"entity_id": id_a}))
                .unwrap();
        }

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out_a = mcp
            .0
            .lock()
            .unwrap()
            .execute("is_entity_selected", json!({"entity_id": id_a}))
            .unwrap();
        let out_b = mcp
            .0
            .lock()
            .unwrap()
            .execute("is_entity_selected", json!({"entity_id": id_b}))
            .unwrap();
        assert!(out_a.is_ok());
        assert_eq!(
            out_a.content["selected"].as_bool().unwrap(),
            true,
            "IsSelA should be selected"
        );
        assert_eq!(
            out_b.content["selected"].as_bool().unwrap(),
            false,
            "IsSelB should not be selected"
        );
    }

    #[test]
    fn mcp_get_entities_in_radius_filters_by_distance() {
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
                        {"name": "Near", "position": [1.0, 0.0, 0.0]},
                        {"name": "Far",  "position": [100.0, 0.0, 0.0]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (near_id, far_id) = {
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
            let n = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("Near"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let f = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("Far"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (n, f)
        };

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "get_entities_in_radius",
                json!({"cx": 0.0, "cy": 0.0, "cz": 0.0, "radius": 10.0}),
            )
            .unwrap();
        assert!(out.is_ok());
        let ids: Vec<u64> = out.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_u64().unwrap())
            .collect();
        assert!(
            ids.contains(&near_id),
            "Near (dist=1) should be in radius 10"
        );
        assert!(
            !ids.contains(&far_id),
            "Far (dist=100) should not be in radius 10"
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
            .execute("get_entity_position", json!({"entity_id": eid}))
            .unwrap();
        let p = &pos.content["position"];
        assert!((p[0].as_f64().unwrap()).abs() < 1e-3, "x=0");
        assert!((p[1].as_f64().unwrap()).abs() < 1e-3, "y=0");
        assert!((p[2].as_f64().unwrap()).abs() < 1e-3, "z=0");
        let sc = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity_scale", json!({"entity_id": eid}))
            .unwrap();
        let s = &sc.content["scale"];
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
            .execute("get_entity_scale", json!({"entity_id": eid}))
            .unwrap();
        assert!(out.is_ok());
        let s = &out.content["scale"];
        assert!((s[0].as_f64().unwrap() - 3.0).abs() < 1e-3, "sx=3");
        assert!((s[1].as_f64().unwrap() - 3.0).abs() < 1e-3, "sy=3");
        assert!((s[2].as_f64().unwrap() - 3.0).abs() < 1e-3, "sz=3");
    }

    #[test]
    fn mcp_get_entities_not_visible_excludes_visible_entities() {
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
                        {"name": "HideMe"},
                        {"name": "KeepMe"}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (hide_id, keep_id) = {
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
            let h = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("HideMe"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let k = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("KeepMe"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (h, k)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("hide_entity", json!({"entity_id": hide_id}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entities_not_visible", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let ids: Vec<u64> = out.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_u64().unwrap())
            .collect();
        assert!(
            ids.contains(&hide_id),
            "HideMe should be in not-visible list"
        );
        assert!(
            !ids.contains(&keep_id),
            "KeepMe should not be in not-visible list"
        );
    }

    #[test]
    fn mcp_get_entity_camera_info_returns_fov() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "spawn_camera",
                    json!({"fov_y_degrees": 75.0, "position": [0.0, 0.0, 10.0]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let cam_id = {
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
                .find(|e| !e["camera_fov"].is_null())
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity_camera_info", json!({"entity_id": cam_id}))
            .unwrap();
        assert!(out.is_ok());
        assert!(
            (out.content["fov_y_degrees"].as_f64().unwrap() - 75.0).abs() < 1.0,
            "fov should be ~75"
        );
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
            .execute("get_entity_position", json!({"entity_id": id_a}))
            .unwrap();
        let pb = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity_position", json!({"entity_id": id_b}))
            .unwrap();
        assert!(
            (pa.content["position"][1].as_f64().unwrap() - 3.0).abs() < 1e-3,
            "MoveSelA y should be 3"
        );
        assert!(
            (pb.content["position"][1].as_f64().unwrap() - 3.0).abs() < 1e-3,
            "MoveSelB y should be 3"
        );
        assert!(
            (pb.content["position"][0].as_f64().unwrap() - 5.0).abs() < 1e-3,
            "MoveSelB x should still be 5"
        );
    }

    #[test]
    fn mcp_get_entity_light_info_returns_light_details() {
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
                        "color": [1.0, 0.5, 0.0],
                        "intensity": 200.0,
                        "range": 15.0,
                        "position": [0.0, 3.0, 0.0]
                    }),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let light_id = {
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
                .find(|e| !e["light_type"].is_null())
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity_light_info", json!({"entity_id": light_id}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["light_type"].as_str().unwrap(), "point");
        assert!((out.content["intensity"].as_f64().unwrap() - 200.0).abs() < 1.0);
        assert!((out.content["range"].as_f64().unwrap() - 15.0).abs() < 1.0);
    }

    #[test]
    fn mcp_get_selected_entity_count_reflects_selection() {
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
                        {"name": "SelCountA"},
                        {"name": "SelCountB"},
                        {"name": "SelCountC"}
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
                .find(|e| e["name"].as_str() == Some("SelCountA"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let b = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("SelCountB"))
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

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_selected_entity_count", json!({}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(
            out.content["count"].as_u64().unwrap(),
            2,
            "2 entities selected"
        );
    }

    #[test]
    fn mcp_get_entity_children_returns_direct_children() {
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
                        {"name": "ChildA"},
                        {"name": "ChildB"},
                        {"name": "Unrelated"}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (parent_id, child_a_id, child_b_id) = {
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
            let p = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("Parent"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let a = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("ChildA"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let b = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("ChildB"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (p, a, b)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute(
                "set_parent",
                json!({"entity_id": child_a_id, "parent_id": parent_id}),
            )
            .unwrap();
            m.execute(
                "set_parent",
                json!({"entity_id": child_b_id, "parent_id": parent_id}),
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
            .execute("get_entity_children", json!({"entity_id": parent_id}))
            .unwrap();
        assert!(out.is_ok());
        let ids: Vec<u64> = out.content["children"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_u64().unwrap())
            .collect();
        assert!(ids.contains(&child_a_id), "ChildA should be listed");
        assert!(ids.contains(&child_b_id), "ChildB should be listed");
        assert_eq!(ids.len(), 2, "only 2 direct children expected");
    }

    #[test]
    fn mcp_get_root_entities_returns_parentless_entities() {
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
                        {"name": "RootA"},
                        {"name": "RootB"},
                        {"name": "Child"}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (root_a_id, root_b_id, child_id) = {
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
                .find(|e| e["name"].as_str() == Some("RootA"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let b = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("RootB"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let c = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("Child"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (a, b, c)
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "set_parent",
                    json!({"entity_id": child_id, "parent_id": root_a_id}),
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
            .execute("get_root_entities", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let ids: Vec<u64> = out.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_u64().unwrap())
            .collect();
        assert!(ids.contains(&root_a_id), "RootA should be root");
        assert!(ids.contains(&root_b_id), "RootB should be root");
        assert!(!ids.contains(&child_id), "Child should not be root");
    }

    #[test]
    fn mcp_get_entities_named_returns_exact_matches() {
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
                        {"name": "Exactly"},
                        {"name": "Exactly"},
                        {"name": "ExactlyNot"}
                    ]}),
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
            .execute("get_entities_named", json!({"name": "Exactly"}))
            .unwrap();
        assert!(out.is_ok());
        let entities = out.content["entities"].as_array().unwrap();
        assert_eq!(entities.len(), 2, "two entities named 'Exactly'");
        assert!(
            entities
                .iter()
                .all(|e| e["name"].as_str() == Some("Exactly")),
            "all results should be named Exactly"
        );
    }

    #[test]
    fn mcp_get_scene_entity_names_returns_name_list() {
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
                        {"name": "NameListA"},
                        {"name": "NameListB"}
                    ]}),
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
            .execute("get_scene_entity_names", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let names = out.content["names"].as_array().unwrap();
        let name_strs: Vec<&str> = names.iter().filter_map(|n| n.as_str()).collect();
        assert!(
            name_strs.contains(&"NameListA"),
            "NameListA should be in names"
        );
        assert!(
            name_strs.contains(&"NameListB"),
            "NameListB should be in names"
        );
    }

    #[test]
    fn mcp_get_entity_mesh_id_returns_mesh_id() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "MeshIdEnt"}))
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
                .find(|e| e["name"].as_str() == Some("MeshIdEnt"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("attach_mesh", json!({"entity_id": eid, "mesh_id": 77}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity_mesh_id", json!({"entity_id": eid}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(
            out.content["mesh_id"].as_u64().unwrap(),
            77,
            "mesh_id should be 77"
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
            .execute("get_entities_with_mesh", json!({}))
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
    fn mcp_get_entities_by_mesh_id_filters_by_mesh_id() {
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
                        {"name": "Mesh42A"},
                        {"name": "Mesh42B"},
                        {"name": "Mesh99"}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (id42a, id42b, id99) = {
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
                .find(|e| e["name"].as_str() == Some("Mesh42A"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let b = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("Mesh42B"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let c = ents
                .iter()
                .find(|e| e["name"].as_str() == Some("Mesh99"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (a, b, c)
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("attach_mesh", json!({"entity_id": id42a, "mesh_id": 42}))
                .unwrap();
        }
        app.update();
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("attach_mesh", json!({"entity_id": id42b, "mesh_id": 42}))
                .unwrap();
        }
        app.update();
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("attach_mesh", json!({"entity_id": id99, "mesh_id": 99}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entities_by_mesh_id", json!({"mesh_id": 42}))
            .unwrap();
        assert!(out.is_ok());
        let ids: Vec<u64> = out.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_u64().unwrap())
            .collect();
        assert!(ids.contains(&id42a), "Mesh42A should have mesh_id 42");
        assert!(ids.contains(&id42b), "Mesh42B should have mesh_id 42");
        assert!(!ids.contains(&id99), "Mesh99 has mesh_id 99, not 42");
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
            .execute("get_entity_position", json!({"entity_id": id_a}))
            .unwrap();
        let pb = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity_position", json!({"entity_id": id_b}))
            .unwrap();
        assert!(
            (pa.content["position"][1].as_f64().unwrap() - 10.0).abs() < 1e-3,
            "AlignA y should be 10"
        );
        assert!(
            (pb.content["position"][1].as_f64().unwrap() - 10.0).abs() < 1e-3,
            "AlignB y should be 10"
        );
    }

    #[test]
    fn mcp_get_entity_position_returns_position_only() {
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
                        {"name": "PosOnly", "position": [5.0, 6.0, 7.0]}
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
                .find(|e| e["name"].as_str() == Some("PosOnly"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity_position", json!({"entity_id": eid}))
            .unwrap();
        assert!(out.is_ok());
        let pos = &out.content["position"];
        assert!((pos[0].as_f64().unwrap() - 5.0).abs() < 1e-3, "x=5");
        assert!((pos[1].as_f64().unwrap() - 6.0).abs() < 1e-3, "y=6");
        assert!((pos[2].as_f64().unwrap() - 7.0).abs() < 1e-3, "z=7");
    }

    #[test]
    fn mcp_get_entities_with_mesh_returns_meshed_entities() {
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
                        {"name": "WithMesh"},
                        {"name": "NoMesh"}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let mesh_eid = {
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
                .find(|e| e["name"].as_str() == Some("WithMesh"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("attach_mesh", json!({"entity_id": mesh_eid, "mesh_id": 99}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entities_with_mesh", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let ids: Vec<u64> = out.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_u64().unwrap())
            .collect();
        assert!(ids.contains(&mesh_eid), "WithMesh should have mesh");
        let no_mesh_id = {
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
                .find(|e| e["name"].as_str() == Some("NoMesh"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        assert!(!ids.contains(&no_mesh_id), "NoMesh should not have mesh");
    }

    #[test]
    fn mcp_get_entity_rotation_returns_rotation_values() {
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
                    json!({"entities": [{"name": "RotEnt", "position": [0.0,0.0,0.0]}]}),
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
                .find(|e| e["name"].as_str() == Some("RotEnt"))
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
                    json!({"entity_id": eid, "rx": 10.0, "ry": 20.0, "rz": 30.0}),
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
            .execute("get_entity_rotation", json!({"entity_id": eid}))
            .unwrap();
        assert!(out.is_ok());
        let rot = &out.content["rotation"];
        assert!(
            (rot[0].as_f64().unwrap() - 10.0).abs() < 1e-3,
            "rx should be 10"
        );
        assert!(
            (rot[1].as_f64().unwrap() - 20.0).abs() < 1e-3,
            "ry should be 20"
        );
        assert!(
            (rot[2].as_f64().unwrap() - 30.0).abs() < 1e-3,
            "rz should be 30"
        );
    }

    #[test]
    fn mcp_get_entity_scale_returns_scale_values() {
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
                    json!({"entities": [{"name": "ScaleEnt", "position": [0.0,0.0,0.0]}]}),
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
                .find(|e| e["name"].as_str() == Some("ScaleEnt"))
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
                    "set_scale",
                    json!({"entity_id": eid, "sx": 2.0, "sy": 3.0, "sz": 4.0}),
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
            .execute("get_entity_scale", json!({"entity_id": eid}))
            .unwrap();
        assert!(out.is_ok());
        let scale = &out.content["scale"];
        assert!(
            (scale[0].as_f64().unwrap() - 2.0).abs() < 1e-3,
            "sx should be 2"
        );
        assert!(
            (scale[1].as_f64().unwrap() - 3.0).abs() < 1e-3,
            "sy should be 3"
        );
        assert!(
            (scale[2].as_f64().unwrap() - 4.0).abs() < 1e-3,
            "sz should be 4"
        );
    }

    #[test]
    fn mcp_get_selected_entity_count_returns_selection_size() {
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
                        {"name": "SelCnt1"},
                        {"name": "SelCnt2"},
                        {"name": "SelCnt3"}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (id1, id2) = {
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
            let a = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("SelCnt1"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let b = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("SelCnt2"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (a, b)
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("select_entity", json!({"entity_id": id1}))
                .unwrap();
            m.execute("select_entity", json!({"entity_id": id2}))
                .unwrap();
        }

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_selected_entity_count", json!({}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["count"], 2, "two entities selected");
    }

    #[test]
    fn mcp_get_entities_sorted_by_distance_returns_ascending_order() {
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
                        {"name": "SortFar",  "position": [50.0, 0.0, 0.0]},
                        {"name": "SortMid",  "position": [10.0, 0.0, 0.0]},
                        {"name": "SortNear", "position": [1.0, 0.0, 0.0]}
                    ]}),
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
            .execute(
                "get_entities_sorted_by_distance",
                json!({"x": 0.0, "y": 0.0, "z": 0.0}),
            )
            .unwrap();
        assert!(out.is_ok());
        let entities = out.content["entities"].as_array().unwrap();
        let names: Vec<&str> = entities.iter().filter_map(|e| e["name"].as_str()).collect();
        let near_idx = names.iter().position(|&n| n == "SortNear").unwrap();
        let mid_idx = names.iter().position(|&n| n == "SortMid").unwrap();
        let far_idx = names.iter().position(|&n| n == "SortFar").unwrap();
        assert!(near_idx < mid_idx, "SortNear should come before SortMid");
        assert!(mid_idx < far_idx, "SortMid should come before SortFar");
    }

    #[test]
    fn mcp_get_farthest_entity_returns_most_distant() {
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
                        {"name": "FarA", "position": [1.0, 0.0, 0.0]},
                        {"name": "FarB", "position": [100.0, 0.0, 0.0]}
                    ]}),
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
            .execute("get_farthest_entity", json!({"x": 0.0, "y": 0.0, "z": 0.0}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(
            out.content["entity"]["name"].as_str().unwrap(),
            "FarB",
            "FarB at [100,0,0] is farther from origin than FarA at [1,0,0]"
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
    fn mcp_get_nearest_entity_returns_closest() {
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
                        {"name": "NearA", "position": [1.0, 0.0, 0.0]},
                        {"name": "NearB", "position": [10.0, 0.0, 0.0]}
                    ]}),
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
            .execute("get_nearest_entity", json!({"x": 0.0, "y": 0.0, "z": 0.0}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(
            out.content["entity"]["name"].as_str().unwrap(),
            "NearA",
            "NearA at [1,0,0] is closer to origin than NearB at [10,0,0]"
        );
    }

    #[test]
    fn mcp_get_entity_distance_returns_euclidean_distance() {
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
                        {"name": "DistA", "position": [0.0, 0.0, 0.0]},
                        {"name": "DistB", "position": [3.0, 4.0, 0.0]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (a_id, b_id) = {
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
            let a = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("DistA"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let b = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("DistB"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (a, b)
        };

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "get_entity_distance",
                json!({"entity_id_a": a_id, "entity_id_b": b_id}),
            )
            .unwrap();
        assert!(out.is_ok());
        let dist = out.content["distance"].as_f64().unwrap();
        assert!(
            (dist - 5.0).abs() < 1e-3,
            "distance between [0,0,0] and [3,4,0] should be 5.0, got {}",
            dist
        );
    }

    #[test]
    fn mcp_get_entity_count_by_type_returns_breakdown() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute("spawn_entity", json!({"name": "TypeGeneric"}))
                .unwrap();
        }
        app.update();
        app.update();

        let generic_id = {
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
                .find(|e| e["name"].as_str() == Some("TypeGeneric"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute(
                "attach_mesh",
                json!({"entity_id": generic_id, "mesh_id": 1}),
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
                    "spawn_camera",
                    json!({"fov_y_degrees": 60.0, "position": [0.0, 0.0, 5.0]}),
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
            .execute("get_entity_count_by_type", json!({}))
            .unwrap();
        assert!(out.is_ok());
        assert!(
            out.content["mesh"].as_u64().unwrap() >= 1,
            "at least 1 mesh entity"
        );
        assert!(
            out.content["camera"].as_u64().unwrap() >= 1,
            "at least 1 camera entity"
        );
        assert!(out.content["total"].as_u64().unwrap() >= 2, "total >= 2");
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
    fn mcp_get_entities_with_tag_filters_by_tag() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute(
                "batch_spawn",
                json!({"entities": [
                    {"name": "TaggedA"},
                    {"name": "TaggedB"},
                    {"name": "Untagged"}
                ]}),
            )
            .unwrap();
        }
        app.update();
        app.update();

        let (tagged_a_id, tagged_b_id) = {
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
            let a = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("TaggedA"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let b = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("TaggedB"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (a, b)
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute(
                "tag_entity",
                json!({"entity_id": tagged_a_id, "tag": "enemy"}),
            )
            .unwrap();
        }
        app.update();
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute(
                "tag_entity",
                json!({"entity_id": tagged_b_id, "tag": "enemy"}),
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
            .execute("get_entities_with_tag", json!({"tag": "enemy"}))
            .unwrap();
        assert!(out.is_ok());
        let ids: Vec<u64> = out.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_u64().unwrap())
            .collect();
        assert!(ids.contains(&tagged_a_id), "TaggedA should have enemy tag");
        assert!(ids.contains(&tagged_b_id), "TaggedB should have enemy tag");
        assert_eq!(ids.len(), 2, "exactly 2 entities with enemy tag");
    }

    #[test]
    fn mcp_count_entities_returns_total() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        let before = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("count_entities", json!({}))
                .unwrap()
                .content["count"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "CntA"}, {"name": "CntB"}, {"name": "CntC"}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let after = mcp
            .0
            .lock()
            .unwrap()
            .execute("count_entities", json!({}))
            .unwrap()
            .content["count"]
            .as_u64()
            .unwrap();
        assert_eq!(
            after,
            before + 3,
            "count should increase by 3 after batch_spawn"
        );
    }

    #[test]
    fn mcp_get_hidden_entities_returns_only_hidden() {
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
                        {"name": "HidA"},
                        {"name": "HidB"},
                        {"name": "HidC"}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let hide_id = {
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
                .find(|e| e["name"].as_str() == Some("HidB"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("hide_entity", json!({"entity_id": hide_id}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_hidden_entities", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let names: Vec<&str> = out.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|e| e["name"].as_str())
            .collect();
        assert!(names.contains(&"HidB"), "HidB should be hidden");
        assert!(!names.contains(&"HidA"), "HidA should not be hidden");
        assert!(!names.contains(&"HidC"), "HidC should not be hidden");
    }

    #[test]
    fn mcp_get_entities_without_parent_returns_root_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "NParRoot"}))
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
                .find(|e| e["name"].as_str() == Some("NParRoot"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "NParChild"}))
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
                .find(|e| e["name"].as_str() == Some("NParChild"))
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
            .execute("get_entities_without_parent", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let ids: Vec<u64> = out.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_u64().unwrap())
            .collect();
        assert!(ids.contains(&root_id), "root should be in no-parent list");
        assert!(
            !ids.contains(&child_id),
            "child with parent should not be in no-parent list"
        );
    }

    #[test]
    fn mcp_get_visible_entities_returns_only_visible() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let m = mcp.0.lock().unwrap();
            m.execute(
                "batch_spawn",
                json!({"entities": [
                    {"name": "VisA"},
                    {"name": "VisB"},
                    {"name": "VisC"}
                ]}),
            )
            .unwrap();
        }
        app.update();
        app.update();

        let hide_id = {
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
                .find(|e| e["name"].as_str() == Some("VisB"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("hide_entity", json!({"entity_id": hide_id}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_visible_entities", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let names: Vec<&str> = out.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|e| e["name"].as_str())
            .collect();
        assert!(names.contains(&"VisA"), "VisA should be visible");
        assert!(!names.contains(&"VisB"), "VisB should be hidden");
        assert!(names.contains(&"VisC"), "VisC should be visible");
    }

    #[test]
    fn mcp_select_entities_by_name_pattern_selects_matching() {
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
                        {"name": "Enemy_01"},
                        {"name": "Enemy_02"},
                        {"name": "Player_01"}
                    ]}),
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
                    "select_entities_by_name_pattern",
                    json!({"pattern": "Enemy"}),
                )
                .unwrap();
            assert!(out.is_ok());
            assert_eq!(out.content["matched_count"], 2);
        }

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let sel = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_selection", json!({}))
            .unwrap();
        let ids = sel.content["selected_ids"].as_array().unwrap();
        assert_eq!(ids.len(), 2, "exactly 2 enemies selected");
    }

    #[test]
    fn mcp_get_entity_depth_returns_hierarchy_depth() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "DRoot"}))
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
                .find(|e| e["name"].as_str() == Some("DRoot"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "DChild"}))
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
                .find(|e| e["name"].as_str() == Some("DChild"))
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
        let root_out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity_depth", json!({"entity_id": root_id}))
            .unwrap();
        assert_eq!(root_out.content["depth"], 0, "root has depth 0");

        let child_out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity_depth", json!({"entity_id": child_id}))
            .unwrap();
        assert_eq!(child_out.content["depth"], 1, "child has depth 1");
    }

    #[test]
    fn mcp_get_scene_center_returns_average_position() {
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
                        {"name": "SC1", "position": [0.0, 0.0, 0.0]},
                        {"name": "SC2", "position": [4.0, 0.0, 0.0]},
                        {"name": "SC3", "position": [2.0, 6.0, 0.0]}
                    ]}),
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
            .execute("get_scene_center", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let center = &out.content["center"];
        assert!(
            (center[0].as_f64().unwrap() - 2.0).abs() < 1e-3,
            "center.x should be (0+4+2)/3=2"
        );
        assert!(
            (center[1].as_f64().unwrap() - 2.0).abs() < 1e-3,
            "center.y should be (0+0+6)/3=2"
        );
    }

    #[test]
    fn mcp_get_entities_in_aabb_filters_by_bounding_box() {
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
                        {"name": "InBox",  "position": [2.0, 2.0, 2.0]},
                        {"name": "OutBox", "position": [20.0, 20.0, 20.0]}
                    ]}),
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
            .execute(
                "get_entities_in_aabb",
                json!({
                    "min_x": 0.0, "min_y": 0.0, "min_z": 0.0,
                    "max_x": 5.0, "max_y": 5.0, "max_z": 5.0
                }),
            )
            .unwrap();
        assert!(out.is_ok());
        let names: Vec<&str> = out.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|e| e["name"].as_str())
            .collect();
        assert!(names.contains(&"InBox"), "InBox should be inside AABB");
        assert!(!names.contains(&"OutBox"), "OutBox should be outside AABB");
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
    fn mcp_get_entity_full_name_returns_path_string() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "FNRoot"}))
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
                .find(|e| e["name"].as_str() == Some("FNRoot"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "FNChild"}))
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
                .find(|e| e["name"].as_str() == Some("FNChild"))
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
            .execute("get_entity_full_name", json!({"entity_id": child_id}))
            .unwrap();
        assert!(out.is_ok());
        let full_name = out.content["full_name"].as_str().unwrap();
        assert!(
            full_name.contains("FNRoot"),
            "full name should contain parent"
        );
        assert!(
            full_name.contains("FNChild"),
            "full name should contain child"
        );
        assert!(full_name.contains('/'), "full name should use / separator");
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
    fn mcp_get_entity_is_leaf_returns_correct_value() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "LeafParent"}))
                .unwrap();
        }
        app.update();
        app.update();

        let parent_id = {
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
                .find(|e| e["name"].as_str() == Some("LeafParent"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "LeafChild"}))
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
                .find(|e| e["name"].as_str() == Some("LeafChild"))
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
                    json!({"entity_id": child_id, "parent_id": parent_id}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let mcp = mcp.0.lock().unwrap();

        let parent_leaf = mcp
            .execute("get_entity_is_leaf", json!({"entity_id": parent_id}))
            .unwrap();
        assert_eq!(
            parent_leaf.content["is_leaf"], false,
            "parent with child should not be leaf"
        );

        let child_leaf = mcp
            .execute("get_entity_is_leaf", json!({"entity_id": child_id}))
            .unwrap();
        assert_eq!(
            child_leaf.content["is_leaf"], true,
            "child with no children should be leaf"
        );
    }

    #[test]
    fn mcp_get_entities_with_camera_returns_camera_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let mcp = mcp.0.lock().unwrap();
            mcp.execute(
                "batch_spawn",
                json!({"entities": [{"name": "Plain2", "position": [0.0,0.0,0.0]}]}),
            )
            .unwrap();
            mcp.execute(
                "spawn_camera",
                json!({"fov_y_degrees": 45.0, "position": [0.0, 5.0, 10.0]}),
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
            .execute("get_entities_with_camera", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let entities = out.content["entities"].as_array().unwrap();
        assert_eq!(entities.len(), 1, "should have exactly 1 camera entity");
        assert!(
            !entities[0]["camera_fov"].is_null(),
            "camera entity should have camera_fov"
        );
    }

    #[test]
    fn mcp_get_entities_with_light_returns_all_light_types() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let mcp = mcp.0.lock().unwrap();
            mcp.execute(
                "batch_spawn",
                json!({"entities": [{"name": "NoLight", "position": [0.0,0.0,0.0]}]}),
            )
            .unwrap();
            mcp.execute("spawn_point_light", json!({"color": [1.0,1.0,1.0], "intensity": 1.0, "range": 10.0, "position": [1.0,0.0,0.0]})).unwrap();
            mcp.execute("spawn_directional_light", json!({"direction": [0.0,-1.0,0.0], "color": [1.0,1.0,1.0], "ambient": [0.1,0.1,0.1]})).unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entities_with_light", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let entities = out.content["entities"].as_array().unwrap();
        assert_eq!(
            entities.len(),
            2,
            "should have 2 light entities (point + directional)"
        );
        let types: Vec<&str> = entities
            .iter()
            .filter_map(|e| e["light_type"].as_str())
            .collect();
        assert!(types.contains(&"point"), "should include point light");
        assert!(
            types.contains(&"directional"),
            "should include directional light"
        );
    }

    #[test]
    fn mcp_get_entity_tag_count_returns_number_of_tags() {
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
                    json!({"entities": [{"name": "MultiTag", "position": [0.0,0.0,0.0]}]}),
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
                .find(|e| e["name"].as_str() == Some("MultiTag"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        for tag in &["alpha", "beta", "gamma"] {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("tag_entity", json!({"entity_id": entity_id, "tag": tag}))
                .unwrap();
            app.update();
            app.update();
        }

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity_tag_count", json!({"entity_id": entity_id}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["count"], 3, "should have 3 tags");
    }

    #[test]
    fn mcp_get_entities_without_mesh_excludes_mesh_entities() {
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
                        {"name": "NoMesh",   "position": [0.0, 0.0, 0.0]},
                        {"name": "WithMesh", "position": [1.0, 0.0, 0.0]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (no_mesh_id, with_mesh_id) = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let list = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap();
            let entities = list.content["entities"].as_array().unwrap();
            let n = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("NoMesh"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let w = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("WithMesh"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (n, w)
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "attach_mesh",
                    json!({"entity_id": with_mesh_id, "mesh_id": 42}),
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
            .execute("get_entities_without_mesh", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let ids: Vec<u64> = out.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|e| e["id"].as_u64())
            .collect();
        assert!(ids.contains(&no_mesh_id), "NoMesh should be in result");
        assert!(
            !ids.contains(&with_mesh_id),
            "WithMesh should not be in result"
        );
    }

    #[test]
    fn mcp_get_entity_world_position_sums_parent_chain() {
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
                        {"name": "WPRoot", "position": [10.0, 0.0, 0.0]}
                    ]}),
                )
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
                .find(|e| e["name"].as_str() == Some("WPRoot"))
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
                    "batch_spawn",
                    json!({"entities": [
                        {"name": "WPChild", "position": [5.0, 0.0, 0.0]}
                    ]}),
                )
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
                .find(|e| e["name"].as_str() == Some("WPChild"))
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
            .execute("get_entity_world_position", json!({"entity_id": child_id}))
            .unwrap();
        assert!(out.is_ok());
        let wp = &out.content["world_position"];
        assert!(
            (wp[0].as_f64().unwrap() - 15.0).abs() < 1e-3,
            "world x should be 10+5=15"
        );
    }

    #[test]
    fn mcp_list_tag_names_returns_unique_tags() {
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
                        {"name": "TagA", "position": [0.0,0.0,0.0]},
                        {"name": "TagB", "position": [1.0,0.0,0.0]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (id_a, id_b) = {
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
                .find(|e| e["name"].as_str() == Some("TagA"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let b = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("TagB"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (a, b)
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let mcp = mcp.0.lock().unwrap();
            mcp.execute("tag_entity", json!({"entity_id": id_a, "tag": "enemy"}))
                .unwrap();
            mcp.execute("tag_entity", json!({"entity_id": id_b, "tag": "enemy"}))
                .unwrap();
            mcp.execute("tag_entity", json!({"entity_id": id_a, "tag": "boss"}))
                .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_tag_names", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let tags: Vec<&str> = out.content["tags"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert!(tags.contains(&"enemy"), "should contain enemy");
        assert!(tags.contains(&"boss"), "should contain boss");
        // deduplication: "enemy" appears on 2 entities but should only be listed once
        let enemy_count = tags.iter().filter(|&&t| t == "enemy").count();
        assert_eq!(enemy_count, 1, "enemy should appear only once");
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
    fn mcp_get_entity_children_count_returns_correct_count() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "CCParent"}))
                .unwrap();
        }
        app.update();
        app.update();

        let parent_id = {
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
                .find(|e| e["name"].as_str() == Some("CCParent"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let mcp = mcp.0.lock().unwrap();
            mcp.execute("spawn_entity", json!({"name": "CCChild1"}))
                .unwrap();
            mcp.execute("spawn_entity", json!({"name": "CCChild2"}))
                .unwrap();
        }
        app.update();
        app.update();

        let (c1_id, c2_id) = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let list = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap();
            let entities = list.content["entities"].as_array().unwrap();
            let c1 = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("CCChild1"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let c2 = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("CCChild2"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (c1, c2)
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let mcp = mcp.0.lock().unwrap();
            mcp.execute(
                "set_parent",
                json!({"entity_id": c1_id, "parent_id": parent_id}),
            )
            .unwrap();
            mcp.execute(
                "set_parent",
                json!({"entity_id": c2_id, "parent_id": parent_id}),
            )
            .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let mcp = mcp.0.lock().unwrap();

        let out = mcp
            .execute("get_entity_children_count", json!({"entity_id": parent_id}))
            .unwrap();
        assert_eq!(out.content["count"], 2, "parent should have 2 children");

        let leaf = mcp
            .execute("get_entity_children_count", json!({"entity_id": c1_id}))
            .unwrap();
        assert_eq!(leaf.content["count"], 0, "child should have 0 children");
    }

    #[test]
    fn mcp_select_entities_in_range_selects_nearby() {
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
                        {"name": "RNear", "position": [2.0, 0.0, 0.0]},
                        {"name": "RFar",  "position": [50.0, 0.0, 0.0]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (near_id, far_id) = {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let list = mcp
                .0
                .lock()
                .unwrap()
                .execute("list_entities", json!({}))
                .unwrap();
            let entities = list.content["entities"].as_array().unwrap();
            let n = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("RNear"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let f = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("RFar"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (n, f)
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute(
                    "select_entities_in_range",
                    json!({"cx": 0.0, "cy": 0.0, "cz": 0.0, "radius": 10.0}),
                )
                .unwrap();
        }

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let mcp = mcp.0.lock().unwrap();
        let sel = mcp.execute("get_selection", json!({})).unwrap();
        let selected: Vec<u64> = sel.content["selected_ids"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_u64())
            .collect();
        assert!(selected.contains(&near_id), "RNear should be selected");
        assert!(!selected.contains(&far_id), "RFar should not be selected");
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
    fn mcp_get_entities_in_range_returns_nearby_entities() {
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
                        {"name": "Near",  "position": [1.0, 0.0, 0.0]},
                        {"name": "Far",   "position": [100.0, 0.0, 0.0]}
                    ]}),
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
            .execute(
                "get_entities_in_range",
                json!({"cx": 0.0, "cy": 0.0, "cz": 0.0, "radius": 10.0}),
            )
            .unwrap();
        assert!(out.is_ok());
        let names: Vec<&str> = out.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|e| e["name"].as_str())
            .collect();
        assert!(names.contains(&"Near"), "Near should be within range");
        assert!(!names.contains(&"Far"), "Far should be outside range");
    }

    #[test]
    fn mcp_get_leaf_entities_returns_entities_without_children() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let mcp = mcp.0.lock().unwrap();
            mcp.execute("spawn_entity", json!({"name": "ParentLeaf"}))
                .unwrap();
        }
        app.update();
        app.update();

        let parent_id = {
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
                .find(|e| e["name"].as_str() == Some("ParentLeaf"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "ChildLeaf"}))
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
                .find(|e| e["name"].as_str() == Some("ChildLeaf"))
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
                    json!({"entity_id": child_id, "parent_id": parent_id}),
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
            .execute("get_leaf_entities", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let leaf_ids: Vec<u64> = out.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|e| e["id"].as_u64())
            .collect();
        assert!(leaf_ids.contains(&child_id), "ChildLeaf should be a leaf");
        assert!(
            !leaf_ids.contains(&parent_id),
            "ParentLeaf should not be a leaf since it has children"
        );
    }

    #[test]
    fn mcp_get_entity_depth_returns_correct_depth() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let mcp = mcp.0.lock().unwrap();
            mcp.execute("spawn_entity", json!({"name": "Root"}))
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
                .find(|e| e["name"].as_str() == Some("Root"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "Child"}))
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
                .find(|e| e["name"].as_str() == Some("Child"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "GrandChild"}))
                .unwrap();
        }
        app.update();
        app.update();

        let grandchild_id = {
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
                .find(|e| e["name"].as_str() == Some("GrandChild"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let mcp = mcp.0.lock().unwrap();
            mcp.execute(
                "set_parent",
                json!({"entity_id": child_id, "parent_id": root_id}),
            )
            .unwrap();
            mcp.execute(
                "set_parent",
                json!({"entity_id": grandchild_id, "parent_id": child_id}),
            )
            .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let mcp = mcp.0.lock().unwrap();

        let root_depth = mcp
            .execute("get_entity_depth", json!({"entity_id": root_id}))
            .unwrap();
        assert_eq!(root_depth.content["depth"], 0, "root depth should be 0");

        let child_depth = mcp
            .execute("get_entity_depth", json!({"entity_id": child_id}))
            .unwrap();
        assert_eq!(child_depth.content["depth"], 1, "child depth should be 1");

        let gc_depth = mcp
            .execute("get_entity_depth", json!({"entity_id": grandchild_id}))
            .unwrap();
        assert_eq!(gc_depth.content["depth"], 2, "grandchild depth should be 2");
    }

    #[test]
    fn mcp_count_selected_returns_selection_size() {
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
                        {"name": "C", "position": [2.0, 0.0, 0.0]}
                    ]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let (id_a, id_b) = {
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
                .find(|e| e["name"].as_str() == Some("A"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let b = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("B"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (a, b)
        };

        // initially 0
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("count_selected", json!({}))
                .unwrap();
            assert_eq!(out.content["count"], 0);
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let mcp = mcp.0.lock().unwrap();
            mcp.execute("select_entity", json!({"entity_id": id_a}))
                .unwrap();
            mcp.execute("select_entity", json!({"entity_id": id_b}))
                .unwrap();
        }

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("count_selected", json!({}))
                .unwrap();
            assert_eq!(out.content["count"], 2);
        }
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
    fn mcp_get_sibling_entities_returns_same_parent_entities() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        // spawn parent + two children
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "Parent"}))
                .unwrap();
        }
        app.update();
        app.update();

        let parent_id = {
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
                .find(|e| e["name"].as_str() == Some("Parent"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let mcp = mcp.0.lock().unwrap();
            mcp.execute("spawn_entity", json!({"name": "SibA"}))
                .unwrap();
            mcp.execute("spawn_entity", json!({"name": "SibB"}))
                .unwrap();
        }
        app.update();
        app.update();

        let (sib_a, sib_b) = {
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
                .find(|e| e["name"].as_str() == Some("SibA"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let b = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("SibB"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (a, b)
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let mcp = mcp.0.lock().unwrap();
            mcp.execute(
                "set_parent",
                json!({"entity_id": sib_a, "parent_id": parent_id}),
            )
            .unwrap();
            mcp.execute(
                "set_parent",
                json!({"entity_id": sib_b, "parent_id": parent_id}),
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
            .execute("get_sibling_entities", json!({"entity_id": sib_a}))
            .unwrap();
        assert!(out.is_ok());
        let siblings = out.content["entities"].as_array().unwrap();
        let ids: Vec<u64> = siblings.iter().filter_map(|e| e["id"].as_u64()).collect();
        assert!(ids.contains(&sib_b), "SibB should be a sibling of SibA");
        assert!(
            !ids.contains(&sib_a),
            "SibA should not be listed as own sibling"
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
    fn mcp_get_entity_path_returns_ancestor_chain() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let mcp = mcp.0.lock().unwrap();
            mcp.execute("spawn_entity", json!({"name": "GrandParent"}))
                .unwrap();
        }
        app.update();
        app.update();

        let gp_id = {
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
                .find(|e| e["name"].as_str() == Some("GrandParent"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "Child"}))
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
                .find(|e| e["name"].as_str() == Some("Child"))
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
                    json!({"entity_id": child_id, "parent_id": gp_id}),
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
            .execute("get_entity_path", json!({"entity_id": child_id}))
            .unwrap();
        assert!(out.is_ok());
        let path = out.content["path"].as_array().unwrap();
        assert_eq!(
            path.len(),
            2,
            "path should have 2 entries: GrandParent → Child"
        );
        assert_eq!(path[0]["name"], "GrandParent", "first in path is root");
        assert_eq!(path[1]["name"], "Child", "last in path is leaf");
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
    fn mcp_find_nearest_entity_returns_closest() {
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
                        {"name": "Near",  "position": [1.0, 0.0, 0.0]},
                        {"name": "Far",   "position": [100.0, 0.0, 0.0]}
                    ]}),
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
            .execute("find_nearest_entity", json!({"x": 0.0, "y": 0.0, "z": 0.0}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(
            out.content["entity"]["name"], "Near",
            "nearest entity should be Near"
        );
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
    fn mcp_get_entity_distance_returns_correct_value() {
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
                        {"name": "P1", "position": [0.0, 0.0, 0.0]},
                        {"name": "P2", "position": [3.0, 4.0, 0.0]}
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
                .find(|e| e["name"].as_str() == Some("P1"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            let id2 = entities
                .iter()
                .find(|e| e["name"].as_str() == Some("P2"))
                .unwrap()["id"]
                .as_u64()
                .unwrap();
            (id1, id2)
        };

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "get_entity_distance",
                json!({"entity_id_a": id1, "entity_id_b": id2}),
            )
            .unwrap();
        assert!(out.is_ok());
        let dist = out.content["distance"].as_f64().unwrap();
        assert!(
            (dist - 5.0).abs() < 1e-3,
            "distance should be 5 (3-4-5 triangle), got {}",
            dist
        );
    }

    #[test]
    fn mcp_get_scene_bounds_covers_all_positions() {
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
                        {"name": "A", "position": [-5.0, 0.0, 0.0]},
                        {"name": "B", "position": [10.0, 3.0, -2.0]}
                    ]}),
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
            .execute("get_scene_bounds", json!({}))
            .unwrap();
        assert!(out.is_ok());
        let min = &out.content["min"];
        let max = &out.content["max"];
        assert!(
            (min[0].as_f64().unwrap() - (-5.0)).abs() < 1e-4,
            "min x should be -5"
        );
        assert!(
            (max[0].as_f64().unwrap() - 10.0).abs() < 1e-4,
            "max x should be 10"
        );
        assert!(
            (max[1].as_f64().unwrap() - 3.0).abs() < 1e-4,
            "max y should be 3"
        );
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
    fn mcp_get_entities_by_type_filters_correctly() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        // spawn plain entity, point light, camera
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let mcp = mcp.0.lock().unwrap();
            mcp.execute("spawn_entity", json!({"name": "Plain"}))
                .unwrap();
            mcp.execute(
                "spawn_point_light",
                json!({
                    "color": [1.0, 1.0, 1.0], "intensity": 1.0, "range": 10.0,
                    "position": [0.0, 0.0, 0.0]
                }),
            )
            .unwrap();
            mcp.execute(
                "spawn_camera",
                json!({"fov_y_degrees": 60.0, "position": [0.0, 0.0, 0.0]}),
            )
            .unwrap();
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let mcp = mcp.0.lock().unwrap();

        let lights = mcp
            .execute("get_entities_by_type", json!({"entity_type": "light"}))
            .unwrap();
        assert!(lights.is_ok());
        assert!(
            lights.content["entities"].as_array().unwrap().len() >= 1,
            "at least 1 light"
        );

        let cameras = mcp
            .execute("get_entities_by_type", json!({"entity_type": "camera"}))
            .unwrap();
        assert!(cameras.is_ok());
        assert!(
            cameras.content["entities"].as_array().unwrap().len() >= 1,
            "at least 1 camera"
        );

        let plains = mcp
            .execute("get_entities_by_type", json!({"entity_type": "plain"}))
            .unwrap();
        assert!(plains.is_ok());
        let plain_names: Vec<&str> = plains.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|e| e["name"].as_str())
            .collect();
        assert!(
            plain_names.contains(&"Plain"),
            "Plain entity should be in plain type"
        );
    }

    #[test]
    fn mcp_get_root_entities_and_get_children() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "Root"}))
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
                .find(|e| e["name"].as_str() == Some("Root"))
                .unwrap()["id"]
                .as_u64()
                .unwrap()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("spawn_entity", json!({"name": "Child"}))
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
                .find(|e| e["name"].as_str() == Some("Child"))
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

        // get_root_entities: Root present, Child absent
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("get_root_entities", json!({}))
                .unwrap();
            assert!(out.is_ok());
            let ids: Vec<u64> = out.content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|e| e["id"].as_u64())
                .collect();
            assert!(ids.contains(&root_id), "Root should be in root entities");
            assert!(
                !ids.contains(&child_id),
                "Child should not be in root entities"
            );
        }

        // get_children: Child present
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute("get_children", json!({"entity_id": root_id}))
                .unwrap();
            assert!(out.is_ok());
            let ids: Vec<u64> = out.content["entities"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|e| e["id"].as_u64())
                .collect();
            assert!(ids.contains(&child_id), "Child should be in children");
        }
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
    fn mcp_tag_entity_and_find_by_tag() {
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
                    json!({"entities": [{"name": "Rock"}, {"name": "Tree"}, {"name": "Rock2"}]}),
                )
                .unwrap();
        }
        app.update();
        app.update();

        let ids: Vec<u64> = {
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
                .filter(|e| matches!(e["name"].as_str(), Some("Rock") | Some("Rock2")))
                .map(|e| e["id"].as_u64().unwrap())
                .collect()
        };

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            for &id in &ids {
                mcp.0
                    .lock()
                    .unwrap()
                    .execute("tag_entity", json!({"entity_id": id, "tag": "static"}))
                    .unwrap();
            }
        }
        app.update();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let result = mcp
                .0
                .lock()
                .unwrap()
                .execute("find_entities_by_tag", json!({"tag": "static"}))
                .unwrap();
            assert!(result.is_ok());
            assert_eq!(result.content["count"], 2);
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
            color: glam::Vec3::new(0.5, 0.5, 0.5),
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
                color: glam::Vec3::new(0.8, 0.8, 0.8),
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
        assert!((lights[0].inner_angle - 0.3).abs() < 1e-4);
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
    fn mcp_find_entities_by_name_filters_case_insensitive() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.world_mut()
            .spawn(bsengine_scene::Name("PlayerHero".to_string()));
        app.world_mut()
            .spawn(bsengine_scene::Name("EnemyArcher".to_string()));
        app.world_mut()
            .spawn(bsengine_scene::Name("PlayerMage".to_string()));
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute("find_entities_by_name", json!({"query": "player"}))
            .expect("find_entities_by_name not found");
        assert!(result.is_ok());
        let entities = result.content["entities"].as_array().unwrap();
        assert_eq!(entities.len(), 2, "expected 2 player entities");
        let names: Vec<_> = entities.iter().filter_map(|e| e["name"].as_str()).collect();
        assert!(names.contains(&"PlayerHero"));
        assert!(names.contains(&"PlayerMage"));
        assert!(!names.contains(&"EnemyArcher"));
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
            Transform::from_translation(Vec3::ZERO),
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
                Transform::from_translation(Vec3::ZERO),
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
                Transform::from_translation(Vec3::ZERO),
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
    fn mcp_save_scene_writes_ron_file() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.world_mut().spawn((
            Name("Castle".to_string()),
            Transform::from_translation(Vec3::new(5.0, 0.0, 0.0)),
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
                Transform::from_translation(Vec3::new(3.0, 1.0, 0.0)),
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
                .map(|(n, t)| (n.0.as_str(), t.translation))
                .collect();
            let found = results
                .iter()
                .find(|(name, _)| *name == "Tower")
                .expect("Tower not found after load");
            assert!((found.1.x - 3.0).abs() < 1e-4, "wrong x: {}", found.1.x);
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
                Transform::from_translation(Vec3::ZERO),
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
}
