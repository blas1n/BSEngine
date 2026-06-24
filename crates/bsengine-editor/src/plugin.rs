use crate::snapshot::{
    EditorCommand, EditorCommandQueueResource, EditorSnapshot, EditorSnapshotResource, EntityInfo,
    SharedCommandQueue, SharedSnapshot,
};
use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::{Commands, Entity, ParamSet, Query};
use bsengine_core::{Camera, DirectionalLight, GlobalTransform, PointLight, SpotLight, Transform};
use bsengine_ecs::Res;
use bsengine_mcp::{McpRegistryResource, McpTool, McpToolOutput};
use bsengine_render::MeshRenderer;
use bsengine_scene::{EntityDescriptor, Name, SceneDescriptor};
use serde_json::json;
use std::sync::{Arc, Mutex};

fn update_editor_snapshot(
    snapshot_res: Res<EditorSnapshotResource>,
    query: Query<(
        Entity,
        Option<&Name>,
        Option<&Transform>,
        Option<&MeshRenderer>,
        Option<&PointLight>,
        Option<&DirectionalLight>,
        Option<&SpotLight>,
        Option<&Camera>,
    )>,
) {
    let mut snapshot = snapshot_res.0.lock().unwrap();
    snapshot.entities = query
        .iter()
        .map(|(e, name, transform, mesh, pt, dir, spot, cam)| {
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
                mesh_id: mesh.map(|m| m.mesh_id),
                light_type,
                light_color,
                light_intensity,
                light_range,
                camera_fov: cam.map(|c| c.fov_y_radians.to_degrees()),
            }
        })
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

        app.insert_resource(EditorSnapshotResource(snapshot.clone()));
        app.insert_resource(EditorCommandQueueResource(cmd_queue.clone()));
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
                    "properties": { "id": { "type": "number", "description": "Entity ID" } },
                    "required": ["id"]
                })),
                handler: Box::new(move |input| {
                    let id = match input["id"].as_u64() {
                        Some(v) => v,
                        None => return McpToolOutput::error("missing numeric 'id' field"),
                    };
                    let s = snap2.lock().unwrap();
                    match s.entities.iter().find(|e| e.id == id) {
                        Some(e) => McpToolOutput::success(json!({
                            "id": e.id,
                            "name": e.name,
                            "position": e.position,
                            "mesh_id": e.mesh_id,
                            "light_type": e.light_type,
                            "light_color": e.light_color,
                            "light_intensity": e.light_intensity,
                            "light_range": e.light_range,
                        })),
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
            .execute("get_entity", json!({"id": eid.index() as u64}))
            .expect("get_entity not found");
        assert!(result.is_ok(), "error: {:?}", result.error);
        assert_eq!(result.content["name"], "Shield");
        let pos = &result.content["position"];
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
            .execute("get_entity", json!({"id": eid.index() as u64}))
            .unwrap();
        let color = result.content["light_color"].as_array().unwrap();
        assert!((color[0].as_f64().unwrap() - 0.8).abs() < 1e-3);
        assert!(
            result.content["light_intensity"].is_null(),
            "directional has no intensity"
        );
        assert!(
            result.content["light_range"].is_null(),
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
            .execute("get_entity", json!({"id": eid.index() as u64}))
            .unwrap();
        assert_eq!(result.content["light_type"], "directional");
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
            .execute("get_entity", json!({"id": eid.index() as u64}))
            .expect("get_entity not found");
        assert!(result.is_ok());
        assert_eq!(result.content["mesh_id"], 55);
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
