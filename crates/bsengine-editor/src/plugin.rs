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
