//! Reading the scene: list, get, query, components, hierarchy.

use super::*;

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
            bad.error
                .as_deref()
                .unwrap_or("")
                .contains("unknown field `tagz`"),
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
    assert!(
        ids.contains(&prop),
        "the tagged entity is selected: {ids:?}"
    );
    assert!(
        !ids.contains(&other),
        "premise: the other one is not: {ids:?}"
    );
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
fn mcp_save_load_scene_round_trip_preserves_reflected_component_attached_via_set_reflected_component(
) {
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
    let out =
        {
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
        registry
            .execute(
                "set_reflected_component",
                json!({
                    "entity_id": eid.index() as u64,
                    "type_path": "not::a::real::Type",
                    "value_json": "{}",
                }),
            )
            .expect("tool should be registered")
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
        registry
            .execute(
                "set_reflected_component",
                json!({
                    "entity_id": eid.index() as u64,
                    "type_path": "bsengine_core::nav_mesh_agent::NavMeshAgent",
                    "value_json": r#"{"speed": "not a number"}"#,
                }),
            )
            .expect("tool should be registered")
    };
    assert!(!out.is_ok());
}
