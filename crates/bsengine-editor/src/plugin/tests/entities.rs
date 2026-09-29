//! Spawning, despawning, duplicating and renaming.

use super::*;

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
