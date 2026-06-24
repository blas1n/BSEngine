use crate::snapshot::{
    EditorCommand, EditorCommandQueueResource, EditorSnapshot, EditorSnapshotResource, EntityInfo,
    SharedCommandQueue, SharedSnapshot,
};
use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::{Commands, Entity, ParamSet, Query};
use bsengine_core::Transform;
use bsengine_ecs::Res;
use bsengine_mcp::{McpRegistryResource, McpTool, McpToolOutput};
use bsengine_scene::Name;
use serde_json::json;
use std::sync::{Arc, Mutex};

fn update_editor_snapshot(
    snapshot_res: Res<EditorSnapshotResource>,
    query: Query<(Entity, Option<&Name>, Option<&Transform>)>,
) {
    let mut snapshot = snapshot_res.0.lock().unwrap();
    snapshot.entities = query
        .iter()
        .map(|(e, name, transform)| EntityInfo {
            id: e.index() as u64,
            name: name.map(|n| n.0.clone()),
            position: transform.map(|t| t.translation.to_array()),
        })
        .collect();
}

fn process_editor_commands(
    queue_res: Res<EditorCommandQueueResource>,
    mut params: ParamSet<(Query<Entity>, Query<(Entity, &mut Transform)>)>,
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
        }
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

        if let Some(mut mcp) = app.world_mut().get_resource_mut::<McpRegistryResource>() {
            // list_entities
            let snap = snapshot.clone();
            mcp.0.register(McpTool {
                name: "list_entities".to_string(),
                description: "List all entities with their IDs, names, and positions".to_string(),
                handler: Box::new(move |_input| {
                    let s = snap.lock().unwrap();
                    McpToolOutput::success(json!({
                        "entities": s.entities.iter().map(|e| json!({
                            "id": e.id,
                            "name": e.name,
                            "position": e.position,
                        })).collect::<Vec<_>>()
                    }))
                }),
            });

            // get_entity
            let snap2 = snapshot.clone();
            mcp.0.register(McpTool {
                name: "get_entity".to_string(),
                description: "Get detailed info for a specific entity by ID".to_string(),
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
                        })),
                        None => McpToolOutput::error("entity not found"),
                    }
                }),
            });

            // spawn_entity
            let queue = cmd_queue.clone();
            mcp.0.register(McpTool {
                name: "spawn_entity".to_string(),
                description: "Spawn a new named entity (applied next frame)".to_string(),
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
            mcp.0.register(McpTool {
                name: "set_transform".to_string(),
                description: "Set the world position of an entity by ID (applied next frame)"
                    .to_string(),
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
            mcp.0.register(McpTool {
                name: "despawn_entity".to_string(),
                description: "Despawn an entity by ID (applied next frame)".to_string(),
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
        }
    }
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
                .execute("despawn_entity", json!({"id": eid.index() as u64}))
                .expect("despawn_entity not found");
        }
        app.update();

        let mut q = app.world_mut().query::<&Name>();
        let names: Vec<_> = q.iter(app.world()).map(|n| n.0.as_str()).collect();
        assert!(!names.contains(&"Temp"), "Temp still alive: {:?}", names);
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
