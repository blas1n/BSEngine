use crate::snapshot::{
    EditorSnapshot, EditorSnapshotResource, EditorSpawnQueueResource, EntityInfo, SharedSnapshot,
    SharedSpawnQueue,
};
use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::{Commands, Entity, Query};
use bsengine_ecs::Res;
use bsengine_mcp::{McpRegistryResource, McpTool, McpToolOutput};
use bsengine_scene::Name;
use serde_json::json;
use std::sync::{Arc, Mutex};

fn update_editor_snapshot(
    snapshot_res: Res<EditorSnapshotResource>,
    query: Query<(Entity, Option<&Name>)>,
) {
    let mut snapshot = snapshot_res.0.lock().unwrap();
    snapshot.entities = query
        .iter()
        .map(|(e, name)| EntityInfo {
            id: e.index() as u64,
            name: name.map(|n| n.0.clone()),
        })
        .collect();
}

fn process_spawn_commands(queue_res: Res<EditorSpawnQueueResource>, mut commands: Commands) {
    let mut queue = queue_res.0.lock().unwrap();
    for name in queue.drain(..) {
        commands.spawn(Name(name));
    }
}

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        let snapshot: SharedSnapshot = Arc::new(Mutex::new(EditorSnapshot::default()));
        let spawn_queue: SharedSpawnQueue = Arc::new(Mutex::new(Vec::new()));

        app.insert_resource(EditorSnapshotResource(snapshot.clone()));
        app.insert_resource(EditorSpawnQueueResource(spawn_queue.clone()));
        app.add_systems(Update, update_editor_snapshot);
        app.add_systems(Update, process_spawn_commands);

        if let Some(mut mcp) = app.world_mut().get_resource_mut::<McpRegistryResource>() {
            let snap = snapshot.clone();
            mcp.0.register(McpTool {
                name: "list_entities".to_string(),
                description: "List all entities in the scene with their names and IDs".to_string(),
                handler: Box::new(move |_input| {
                    let s = snap.lock().unwrap();
                    McpToolOutput::success(json!({
                        "entities": s.entities.iter().map(|e| json!({
                            "id": e.id,
                            "name": e.name,
                        })).collect::<Vec<_>>()
                    }))
                }),
            });

            let queue = spawn_queue.clone();
            mcp.0.register(McpTool {
                name: "spawn_entity".to_string(),
                description: "Spawn a new named entity in the scene (applied next frame)"
                    .to_string(),
                handler: Box::new(move |input| {
                    let name = input["name"].as_str().unwrap_or("Entity").to_string();
                    queue.lock().unwrap().push(name.clone());
                    McpToolOutput::success(json!({"status": "queued", "name": name}))
                }),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EditorPlugin;
    use bsengine_app::new_app;
    use bsengine_mcp::McpPlugin;
    use bsengine_scene::Name;
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

        // Process the queued spawn
        app.update();

        let mut q = app.world_mut().query::<&Name>();
        let names: Vec<_> = q.iter(app.world()).map(|n| n.0.as_str()).collect();
        assert!(names.contains(&"Sword"), "Sword not spawned: {:?}", names);
    }
}
