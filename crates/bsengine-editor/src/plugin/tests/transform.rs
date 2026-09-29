//! Transform tools on one entity.

use super::*;

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
