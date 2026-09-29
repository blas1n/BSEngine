//! The selection tools.

use super::*;

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
fn mcp_toggle_entity_selection_toggles_selection_state() {
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
                    {"name": "Entity"},
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
            .content["entities"][0]["id"]
            .as_u64()
            .unwrap()
    };

    let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();

    let out = mcp
        .0
        .lock()
        .unwrap()
        .execute("toggle_entity_selection", json!({"entity_id": entity_id}))
        .unwrap();
    assert!(out.is_ok());
    assert_eq!(
        out.content["selected"], true,
        "toggled from unselected → selected"
    );

    let out2 = mcp
        .0
        .lock()
        .unwrap()
        .execute("toggle_entity_selection", json!({"entity_id": entity_id}))
        .unwrap();
    assert_eq!(
        out2.content["selected"], false,
        "toggled from selected → unselected"
    );

    let out3 = mcp
        .0
        .lock()
        .unwrap()
        .execute("toggle_entity_selection", json!({"entity_id": entity_id}))
        .unwrap();
    assert_eq!(out3.content["selected"], true, "toggled back to selected");
}

#[test]
fn mcp_shrink_selection_to_roots_keeps_only_topmost_selected() {
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
                    {"name": "Root"},
                    {"name": "Child"},
                    {"name": "Grandchild"},
                    {"name": "IndepRoot"},
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
    let (root_id, child_id, gc_id, indep_id) = (
        id_of("Root"),
        id_of("Child"),
        id_of("Grandchild"),
        id_of("IndepRoot"),
    );

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let reg = mcp.0.lock().unwrap();
        reg.execute(
            "set_parent",
            json!({"entity_id": child_id, "parent_id": root_id}),
        )
        .unwrap();
        reg.execute(
            "set_parent",
            json!({"entity_id": gc_id, "parent_id": child_id}),
        )
        .unwrap();
        // select all
        reg.execute("select_entity", json!({"entity_id": root_id}))
            .unwrap();
        reg.execute("select_entity", json!({"entity_id": child_id}))
            .unwrap();
        reg.execute("select_entity", json!({"entity_id": gc_id}))
            .unwrap();
        reg.execute("select_entity", json!({"entity_id": indep_id}))
            .unwrap();
    }
    app.update();
    app.update();

    let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
    let out = mcp
        .0
        .lock()
        .unwrap()
        .execute("shrink_selection_to_roots", json!({}))
        .unwrap();
    assert!(out.is_ok());
    assert_eq!(
        out.content["removed_count"], 2,
        "Child and Grandchild removed"
    );

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
        .map(|v| v.as_u64().unwrap())
        .collect();
    assert!(ids.contains(&root_id), "Root kept");
    assert!(ids.contains(&indep_id), "IndepRoot kept");
    assert!(!ids.contains(&child_id), "Child removed");
    assert!(!ids.contains(&gc_id), "Grandchild removed");
}

#[test]
fn mcp_expand_selection_to_subtrees_adds_all_descendants() {
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
                    {"name": "Root"},
                    {"name": "Child"},
                    {"name": "Grandchild"},
                    {"name": "Unrelated"},
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
    let (root_id, child_id, gc_id, unrel_id) = (
        id_of("Root"),
        id_of("Child"),
        id_of("Grandchild"),
        id_of("Unrelated"),
    );

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let reg = mcp.0.lock().unwrap();
        reg.execute(
            "set_parent",
            json!({"entity_id": child_id, "parent_id": root_id}),
        )
        .unwrap();
        reg.execute(
            "set_parent",
            json!({"entity_id": gc_id, "parent_id": child_id}),
        )
        .unwrap();
        reg.execute("select_entity", json!({"entity_id": root_id}))
            .unwrap();
    }
    app.update();
    app.update();

    let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
    let out = mcp
        .0
        .lock()
        .unwrap()
        .execute("expand_selection_to_subtrees", json!({}))
        .unwrap();
    assert!(out.is_ok());
    assert_eq!(out.content["added_count"], 2, "Child and Grandchild added");

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
        .map(|v| v.as_u64().unwrap())
        .collect();
    assert!(ids.contains(&root_id));
    assert!(ids.contains(&child_id), "Child added");
    assert!(ids.contains(&gc_id), "Grandchild added");
    assert!(!ids.contains(&unrel_id), "Unrelated not added");
}

#[test]
fn mcp_untag_all_selected_removes_tag_from_all_selected_entities() {
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
                    {"name": "C", "position": [2.0, 0.0, 0.0]},
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

    // Tag all three with "hero"
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let reg = mcp.0.lock().unwrap();
        for id in [a_id, b_id, c_id] {
            reg.execute("tag_entity", json!({"entity_id": id, "tag": "hero"}))
                .unwrap();
        }
    }
    app.update();
    app.update();

    // Select A and B, then untag them
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let reg = mcp.0.lock().unwrap();
        reg.execute("select_entity", json!({"entity_id": a_id}))
            .unwrap();
        reg.execute("select_entity", json!({"entity_id": b_id}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("untag_all_selected", json!({"tag": "hero"}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["untagged_count"], 2);
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
    let a = updated
        .iter()
        .find(|e| e["id"].as_u64().unwrap() == a_id)
        .unwrap();
    let b = updated
        .iter()
        .find(|e| e["id"].as_u64().unwrap() == b_id)
        .unwrap();
    let c = updated
        .iter()
        .find(|e| e["id"].as_u64().unwrap() == c_id)
        .unwrap();
    assert!(
        !a["tags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t.as_str() == Some("hero")),
        "A hero tag removed"
    );
    assert!(
        !b["tags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t.as_str() == Some("hero")),
        "B hero tag removed"
    );
    assert!(
        c["tags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t.as_str() == Some("hero")),
        "C hero tag kept"
    );
}

#[test]
fn mcp_tag_all_selected_applies_tag_to_all_selected_entities() {
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
                    {"name": "C", "position": [2.0, 0.0, 0.0]},
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

    // Select A and B
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let reg = mcp.0.lock().unwrap();
        reg.execute("select_entity", json!({"entity_id": a_id}))
            .unwrap();
        reg.execute("select_entity", json!({"entity_id": b_id}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("tag_all_selected", json!({"tag": "hero"}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["tagged_count"], 2);
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
    let a = updated
        .iter()
        .find(|e| e["id"].as_u64().unwrap() == a_id)
        .unwrap();
    let b = updated
        .iter()
        .find(|e| e["id"].as_u64().unwrap() == b_id)
        .unwrap();
    let c = updated
        .iter()
        .find(|e| e["id"].as_u64().unwrap() == c_id)
        .unwrap();
    assert!(
        a["tags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t.as_str() == Some("hero")),
        "A tagged"
    );
    assert!(
        b["tags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t.as_str() == Some("hero")),
        "B tagged"
    );
    assert!(
        !c["tags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t.as_str() == Some("hero")),
        "C not tagged"
    );
}

#[test]
fn mcp_offset_selection_rotation_adds_to_rotation_of_all_selected() {
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
    let (a_id, b_id) = (id_of("A"), id_of("B"));

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let reg = mcp.0.lock().unwrap();
        reg.execute("select_entity", json!({"entity_id": a_id}))
            .unwrap();
        reg.execute("select_entity", json!({"entity_id": b_id}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "offset_selection_rotation",
                json!({"rx": 10.0, "ry": 20.0, "rz": 30.0}),
            )
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["rotated_count"], 2);
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
    for id in [a_id, b_id] {
        let e = updated
            .iter()
            .find(|e| e["id"].as_u64().unwrap() == id)
            .unwrap();
        let rot = e["rotation"].as_array().unwrap();
        assert!((rot[0].as_f64().unwrap() - 10.0).abs() < 0.5, "rx=10");
        assert!((rot[1].as_f64().unwrap() - 20.0).abs() < 0.5, "ry=20");
        assert!((rot[2].as_f64().unwrap() - 30.0).abs() < 0.5, "rz=30");
    }
}

#[test]
fn mcp_offset_selection_scale_adds_to_scale_of_all_selected() {
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
    let (a_id, b_id) = (id_of("A"), id_of("B"));

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let reg = mcp.0.lock().unwrap();
        reg.execute("select_entity", json!({"entity_id": a_id}))
            .unwrap();
        reg.execute("select_entity", json!({"entity_id": b_id}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "offset_selection_scale",
                json!({"sx": 1.0, "sy": 2.0, "sz": 3.0}),
            )
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["scaled_count"], 2);
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
    for id in [a_id, b_id] {
        let e = updated
            .iter()
            .find(|e| e["id"].as_u64().unwrap() == id)
            .unwrap();
        let scale = e["scale"].as_array().unwrap();
        // default scale is [1,1,1], so after offset: [2,3,4]
        assert!((scale[0].as_f64().unwrap() - 2.0).abs() < 0.01, "sx=2");
        assert!((scale[1].as_f64().unwrap() - 3.0).abs() < 0.01, "sy=3");
        assert!((scale[2].as_f64().unwrap() - 4.0).abs() < 0.01, "sz=4");
    }
}

#[test]
fn mcp_offset_selection_position_moves_all_selected_entities() {
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
                    {"name": "B", "position": [10.0, 0.0, 0.0]},
                    {"name": "C", "position": [20.0, 0.0, 0.0]},
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

    // Select A and B only
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let reg = mcp.0.lock().unwrap();
        reg.execute("select_entity", json!({"entity_id": a_id}))
            .unwrap();
        reg.execute("select_entity", json!({"entity_id": b_id}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "offset_selection_position",
                json!({"dx": 5.0, "dy": 3.0, "dz": 1.0}),
            )
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["moved_count"], 2);
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
    let a = updated
        .iter()
        .find(|e| e["id"].as_u64().unwrap() == a_id)
        .unwrap();
    let b = updated
        .iter()
        .find(|e| e["id"].as_u64().unwrap() == b_id)
        .unwrap();
    let c = updated
        .iter()
        .find(|e| e["id"].as_u64().unwrap() == c_id)
        .unwrap();
    assert!(
        (a["position"][0].as_f64().unwrap() - 5.0).abs() < 0.01,
        "A.x=5"
    );
    assert!(
        (b["position"][0].as_f64().unwrap() - 15.0).abs() < 0.01,
        "B.x=15"
    );
    assert!(
        (c["position"][0].as_f64().unwrap() - 20.0).abs() < 0.01,
        "C unchanged"
    );
}

#[test]
fn mcp_scale_selection_uniformly_applies_scale_to_all_selected() {
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
    let (a_id, b_id) = (id_of("A"), id_of("B"));

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let reg = mcp.0.lock().unwrap();
        reg.execute("select_entity", json!({"entity_id": a_id}))
            .unwrap();
        reg.execute("select_entity", json!({"entity_id": b_id}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("scale_selection_uniformly", json!({"scale": 3.0}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["scaled_count"], 2);
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
    for id in [a_id, b_id] {
        let e = updated
            .iter()
            .find(|e| e["id"].as_u64().unwrap() == id)
            .unwrap();
        let scale = e["scale"].as_array().unwrap();
        assert!((scale[0].as_f64().unwrap() - 3.0).abs() < 0.01, "sx=3");
        assert!((scale[1].as_f64().unwrap() - 3.0).abs() < 0.01, "sy=3");
        assert!((scale[2].as_f64().unwrap() - 3.0).abs() < 0.01, "sz=3");
    }
}

#[test]
fn mcp_detach_selection_from_parent_removes_parent_from_selected() {
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
                    {"name": "Parent", "position": [0.0, 0.0, 0.0]},
                    {"name": "ChildA", "position": [1.0, 0.0, 0.0]},
                    {"name": "ChildB", "position": [2.0, 0.0, 0.0]},
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
    let (parent_id, a_id, b_id) = (id_of("Parent"), id_of("ChildA"), id_of("ChildB"));

    // Attach both children to parent
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute(
            "set_parent",
            json!({"entity_id": a_id, "parent_id": parent_id}),
        )
        .unwrap();
        m.execute(
            "set_parent",
            json!({"entity_id": b_id, "parent_id": parent_id}),
        )
        .unwrap();
    }
    app.update();
    app.update();

    // Select and detach
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("select_entity", json!({"entity_id": a_id}))
            .unwrap();
        m.execute("select_entity", json!({"entity_id": b_id}))
            .unwrap();
    }
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("detach_selection_from_parent", json!({}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["affected_count"].as_u64().unwrap(), 2);
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
    for e in &updated {
        if e["id"].as_u64().unwrap() == a_id || e["id"].as_u64().unwrap() == b_id {
            assert!(e["parent_id"].is_null(), "parent should be removed");
        }
    }
}

#[test]
fn mcp_reparent_selection_sets_new_parent_for_all_selected() {
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
                    {"name": "NewParent", "position": [0.0, 0.0, 0.0]},
                    {"name": "A", "position": [1.0, 0.0, 0.0]},
                    {"name": "B", "position": [2.0, 0.0, 0.0]},
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
    let (new_parent_id, a_id, b_id) = (id_of("NewParent"), id_of("A"), id_of("B"));

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("select_entity", json!({"entity_id": a_id}))
            .unwrap();
        m.execute("select_entity", json!({"entity_id": b_id}))
            .unwrap();
    }
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("reparent_selection", json!({"parent_id": new_parent_id}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["affected_count"].as_u64().unwrap(), 2);
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
    for e in &updated {
        if e["id"].as_u64().unwrap() == a_id || e["id"].as_u64().unwrap() == b_id {
            assert_eq!(
                e["parent_id"].as_u64().unwrap(),
                new_parent_id,
                "parent should be NewParent"
            );
        }
    }
}

#[test]
fn mcp_hide_all_selected_hides_all_selected_entities() {
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
    let (a_id, b_id) = (id_of("A"), id_of("B"));

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("select_entity", json!({"entity_id": a_id}))
            .unwrap();
        m.execute("select_entity", json!({"entity_id": b_id}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("hide_all_selected", json!({}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["affected_count"].as_u64().unwrap(), 2);
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
    for e in &updated {
        if e["id"].as_u64().unwrap() == a_id || e["id"].as_u64().unwrap() == b_id {
            assert!(!e["visible"].as_bool().unwrap(), "entity should be hidden");
        }
    }
}

#[test]
fn mcp_show_all_selected_shows_all_hidden_selected_entities() {
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
    let (a_id, b_id) = (id_of("A"), id_of("B"));

    // First hide them
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("hide_entity", json!({"entity_id": a_id}))
            .unwrap();
        m.execute("hide_entity", json!({"entity_id": b_id}))
            .unwrap();
    }
    app.update();
    app.update();

    // Select and show
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("select_entity", json!({"entity_id": a_id}))
            .unwrap();
        m.execute("select_entity", json!({"entity_id": b_id}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("show_all_selected", json!({}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["affected_count"].as_u64().unwrap(), 2);
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
    for e in &updated {
        if e["id"].as_u64().unwrap() == a_id || e["id"].as_u64().unwrap() == b_id {
            assert!(e["visible"].as_bool().unwrap(), "entity should be visible");
        }
    }
}

#[test]
fn mcp_move_selection_to_origin_moves_all_selected_to_zero() {
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
                    {"name": "A", "position": [5.0, 3.0, 1.0]},
                    {"name": "B", "position": [2.0, 7.0, 4.0]},
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
    let (a_id, b_id) = (id_of("A"), id_of("B"));

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("select_entity", json!({"entity_id": a_id}))
            .unwrap();
        m.execute("select_entity", json!({"entity_id": b_id}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("move_selection_to_origin", json!({}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["affected_count"].as_u64().unwrap(), 2);
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
    for e in &updated {
        if e["id"].as_u64().unwrap() == a_id || e["id"].as_u64().unwrap() == b_id {
            let pos = e["position"].as_array().unwrap();
            assert!((pos[0].as_f64().unwrap()).abs() < 0.01, "x should be 0");
            assert!((pos[1].as_f64().unwrap()).abs() < 0.01, "y should be 0");
            assert!((pos[2].as_f64().unwrap()).abs() < 0.01, "z should be 0");
        }
    }
}

#[test]
fn mcp_set_selection_uniform_scale_applies_same_scale_to_all() {
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
    let (a_id, b_id) = (id_of("A"), id_of("B"));

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("select_entity", json!({"entity_id": a_id}))
            .unwrap();
        m.execute("select_entity", json!({"entity_id": b_id}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("set_selection_uniform_scale", json!({"scale": 3.0}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["affected_count"].as_u64().unwrap(), 2);
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
    for e in &updated {
        if e["id"].as_u64().unwrap() == a_id || e["id"].as_u64().unwrap() == b_id {
            let scale = e["scale"].as_array().unwrap();
            for component in scale {
                assert!(
                    (component.as_f64().unwrap() - 3.0).abs() < 0.01,
                    "scale component should be 3.0"
                );
            }
        }
    }
}

#[test]
fn mcp_are_all_selected_visible_returns_true_only_when_all_visible() {
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
                    {"name": "VisA", "position": [0.0, 0.0, 0.0]},
                    {"name": "VisB", "position": [1.0, 0.0, 0.0]},
                    {"name": "Hid",  "position": [2.0, 0.0, 0.0]},
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
    let (vis_a, vis_b, hid_id) = (id_of("VisA"), id_of("VisB"), id_of("Hid"));

    // Hide one entity
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("hide_entity", json!({"entity_id": hid_id}))
            .unwrap();
    }
    app.update();
    app.update();

    // Select only visible — all_visible should be true
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("select_entity", json!({"entity_id": vis_a}))
            .unwrap();
        m.execute("select_entity", json!({"entity_id": vis_b}))
            .unwrap();
    }
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("are_all_selected_visible", json!({}))
            .unwrap();
        assert!(out.is_ok());
        assert!(out.content["all_visible"].as_bool().unwrap());
    }

    // Add hidden entity to selection — all_visible should now be false
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("select_entity", json!({"entity_id": hid_id}))
            .unwrap();
    }
    let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
    let out = mcp
        .0
        .lock()
        .unwrap()
        .execute("are_all_selected_visible", json!({}))
        .unwrap();
    assert!(out.is_ok());
    assert!(!out.content["all_visible"].as_bool().unwrap());
}

#[test]
fn mcp_align_entities_to_selection_pivot_aligns_x_axis() {
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
                    {"name": "Pivot", "position": [5.0, 1.0, 2.0]},
                    {"name": "Other", "position": [9.0, 3.0, 4.0]},
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
    let (pivot_id, other_id) = (id_of("Pivot"), id_of("Other"));

    // Select only "Other"; pivot specified explicitly via pivot_entity_id
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("select_entity", json!({"entity_id": other_id}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "align_entities_to_selection_pivot",
                json!({"pivot_entity_id": pivot_id, "axis": "x"}),
            )
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["affected_count"].as_u64().unwrap(), 1);
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
    let other = entities
        .iter()
        .find(|e| e["id"].as_u64().unwrap() == other_id)
        .unwrap();
    let pos = other["position"].as_array().unwrap();
    // Other's X should be aligned to pivot's X = 5.0; Y and Z unchanged
    assert!(
        (pos[0].as_f64().unwrap() - 5.0).abs() < 0.01,
        "X aligned to pivot"
    );
    assert!((pos[1].as_f64().unwrap() - 3.0).abs() < 0.01, "Y unchanged");
}

#[test]
fn mcp_rename_selection_replace_substitutes_substring_in_name() {
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
                    {"name": "OldHero",   "position": [0.0, 0.0, 0.0]},
                    {"name": "OldVillan", "position": [1.0, 0.0, 0.0]},
                    {"name": "Unrelated", "position": [2.0, 0.0, 0.0]},
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
    let (old_hero, old_villan) = (id_of("OldHero"), id_of("OldVillan"));

    // Select OldHero and OldVillan (not Unrelated)
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("select_entity", json!({"entity_id": old_hero}))
            .unwrap();
        m.execute("select_entity", json!({"entity_id": old_villan}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "rename_selection_replace",
                json!({"from": "Old", "to": "New"}),
            )
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["renamed_count"].as_u64().unwrap(), 2);
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
    let name_of = |id: u64| {
        ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["name"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(name_of(old_hero), "NewHero", "OldHero → NewHero");
    assert_eq!(name_of(old_villan), "NewVillan", "OldVillan → NewVillan");

    let unrelated_id = id_of("Unrelated");
    assert_eq!(name_of(unrelated_id), "Unrelated", "Unrelated unchanged");
}

#[test]
fn mcp_distribute_selection_along_y_spaces_entities_evenly() {
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
                    {"name": "Low",  "position": [0.0, 1.0, 0.0]},
                    {"name": "Mid",  "position": [0.0, 5.0, 0.0]},
                    {"name": "High", "position": [0.0, 9.0, 0.0]},
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
    let ids_all: Vec<u64> = all.iter().map(|e| e["id"].as_u64().unwrap()).collect();

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        for &id in &ids_all {
            m.execute("select_entity", json!({"entity_id": id}))
                .unwrap();
        }
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("distribute_selection_along_y", json!({"spacing": 3.0}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["distributed_count"].as_u64().unwrap(), 3);
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
    let mut ys: Vec<f32> = ents
        .iter()
        .map(|e| e["position"].as_array().unwrap()[1].as_f64().unwrap() as f32)
        .collect();
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert!(
        (ys[1] - ys[0] - 3.0).abs() < 0.01,
        "Y spacing first gap = 3"
    );
    assert!(
        (ys[2] - ys[1] - 3.0).abs() < 0.01,
        "Y spacing second gap = 3"
    );
}

#[test]
fn mcp_distribute_selection_along_x_spaces_entities_evenly() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    // Spawn 3 entities at random X positions; after distribution with spacing=5, they should be at 0, 5, 10
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute(
                "batch_spawn",
                json!({"entities": [
                    {"name": "First",  "position": [7.0, 0.0, 0.0]},
                    {"name": "Second", "position": [1.0, 0.0, 0.0]},
                    {"name": "Third",  "position": [4.0, 0.0, 0.0]},
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
    let ids_all: Vec<u64> = all.iter().map(|e| e["id"].as_u64().unwrap()).collect();

    // Select all
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        for &id in &ids_all {
            m.execute("select_entity", json!({"entity_id": id}))
                .unwrap();
        }
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("distribute_selection_along_x", json!({"spacing": 5.0}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["distributed_count"].as_u64().unwrap(), 3);
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
    let mut xs: Vec<f32> = ents
        .iter()
        .map(|e| e["position"].as_array().unwrap()[0].as_f64().unwrap() as f32)
        .collect();
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    // Should be [0, 5, 10] (sorted by original X: Second=1, Third=4, First=7 → spaced at 0, 5, 10)
    assert!(
        (xs[1] - xs[0] - 5.0).abs() < 0.01,
        "spacing between first and second = 5"
    );
    assert!(
        (xs[2] - xs[1] - 5.0).abs() < 0.01,
        "spacing between second and third = 5"
    );
}

#[test]
fn mcp_distribute_selection_along_z_spaces_entities_evenly() {
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
                    {"name": "P1", "position": [0.0, 0.0, 9.0]},
                    {"name": "P2", "position": [0.0, 0.0, 3.0]},
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
    let ids_all: Vec<u64> = all.iter().map(|e| e["id"].as_u64().unwrap()).collect();

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        for &id in &ids_all {
            m.execute("select_entity", json!({"entity_id": id}))
                .unwrap();
        }
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("distribute_selection_along_z", json!({"spacing": 10.0}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["distributed_count"].as_u64().unwrap(), 2);
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
    let mut zs: Vec<f32> = ents
        .iter()
        .map(|e| e["position"].as_array().unwrap()[2].as_f64().unwrap() as f32)
        .collect();
    zs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert!((zs[1] - zs[0] - 10.0).abs() < 0.01, "Z spacing = 10");
}

#[test]
fn mcp_align_selection_to_y_sets_same_y_for_all_selected() {
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
                    {"name": "A", "position": [1.0, 3.0, 5.0]},
                    {"name": "B", "position": [2.0, 7.0, 8.0]},
                    {"name": "C", "position": [4.0, 1.0, 2.0]},
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

    // Select A and B (not C)
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("select_entity", json!({"entity_id": a_id}))
            .unwrap();
        m.execute("select_entity", json!({"entity_id": b_id}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("align_selection_to_y", json!({"y": 5.0}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["aligned_count"].as_u64().unwrap(), 2);
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
    let pos_of = |id: u64| {
        let arr = ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["position"]
            .as_array()
            .unwrap()
            .clone();
        [
            arr[0].as_f64().unwrap() as f32,
            arr[1].as_f64().unwrap() as f32,
            arr[2].as_f64().unwrap() as f32,
        ]
    };
    let a_pos = pos_of(a_id);
    let b_pos = pos_of(b_id);
    let c_pos = pos_of(c_id);
    assert!((a_pos[1] - 5.0).abs() < 0.01, "A.y aligned to 5");
    assert!((a_pos[0] - 1.0).abs() < 0.01, "A.x unchanged");
    assert!((b_pos[1] - 5.0).abs() < 0.01, "B.y aligned to 5");
    assert!(
        (c_pos[1] - 1.0).abs() < 0.01,
        "C.y unchanged (not selected)"
    );
}

#[test]
fn mcp_align_selection_to_z_sets_same_z_for_all_selected() {
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
                    {"name": "X", "position": [0.0, 0.0, 1.0]},
                    {"name": "Y", "position": [0.0, 0.0, 9.0]},
                    {"name": "Z", "position": [0.0, 0.0, 4.0]},
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
    let (x_id, y_id, z_id) = (id_of("X"), id_of("Y"), id_of("Z"));

    // Select X and Y (not Z)
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("select_entity", json!({"entity_id": x_id}))
            .unwrap();
        m.execute("select_entity", json!({"entity_id": y_id}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("align_selection_to_z", json!({"z": 0.0}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["aligned_count"].as_u64().unwrap(), 2);
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
    let pos_of = |id: u64| {
        let arr = ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["position"]
            .as_array()
            .unwrap()
            .clone();
        [
            arr[0].as_f64().unwrap() as f32,
            arr[1].as_f64().unwrap() as f32,
            arr[2].as_f64().unwrap() as f32,
        ]
    };
    assert!((pos_of(x_id)[2]).abs() < 0.01, "X.z aligned to 0");
    assert!((pos_of(y_id)[2]).abs() < 0.01, "Y.z aligned to 0");
    assert!(
        (pos_of(z_id)[2] - 4.0).abs() < 0.01,
        "Z.z unchanged (not selected)"
    );
}

#[test]
fn mcp_align_selection_to_x_sets_same_x_for_all_selected() {
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
                    {"name": "P", "position": [1.0, 2.0, 3.0]},
                    {"name": "Q", "position": [5.0, 4.0, 6.0]},
                    {"name": "R", "position": [9.0, 0.0, 1.0]},
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
    let (p_id, q_id, r_id) = (id_of("P"), id_of("Q"), id_of("R"));

    // Select P and Q (not R)
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("select_entity", json!({"entity_id": p_id}))
            .unwrap();
        m.execute("select_entity", json!({"entity_id": q_id}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("align_selection_to_x", json!({"x": 0.0}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["aligned_count"].as_u64().unwrap(), 2);
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
    let pos_of = |id: u64| {
        let arr = ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["position"]
            .as_array()
            .unwrap()
            .clone();
        [
            arr[0].as_f64().unwrap() as f32,
            arr[1].as_f64().unwrap() as f32,
            arr[2].as_f64().unwrap() as f32,
        ]
    };
    let p_pos = pos_of(p_id);
    let q_pos = pos_of(q_id);
    let r_pos = pos_of(r_id);
    assert!((p_pos[0]).abs() < 0.01, "P.x aligned to 0");
    assert!((p_pos[1] - 2.0).abs() < 0.01, "P.y unchanged");
    assert!((q_pos[0]).abs() < 0.01, "Q.x aligned to 0");
    assert!(
        (r_pos[0] - 9.0).abs() < 0.01,
        "R.x unchanged (not selected)"
    );
}

#[test]
fn mcp_number_selection_renames_with_sequential_index() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("spawn_entity", json!({"name": "A"})).unwrap();
        m.execute("spawn_entity", json!({"name": "B"})).unwrap();
        m.execute("spawn_entity", json!({"name": "C"})).unwrap();
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
    let (a_id, b_id) = (id_of("A"), id_of("B"));

    // Select A and B
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("select_entity", json!({"entity_id": a_id}))
            .unwrap();
        m.execute("select_entity", json!({"entity_id": b_id}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("number_selection", json!({"prefix": "Item"}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["renamed_count"].as_u64().unwrap(), 2);
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
    let names: Vec<String> = ents
        .iter()
        .filter(|e| {
            let id = e["id"].as_u64().unwrap();
            id == a_id || id == b_id
        })
        .filter_map(|e| e["name"].as_str().map(|s| s.to_string()))
        .collect();
    assert!(
        names.contains(&"Item_1".to_string()) || names.contains(&"Item_2".to_string()),
        "At least one entity renamed to Item_N pattern"
    );
    assert_eq!(names.len(), 2, "Both selected entities were renamed");
}

#[test]
fn mcp_reset_selection_scale_sets_scale_to_one() {
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
                    {"name": "Big", "position": [0.0, 0.0, 0.0]},
                    {"name": "Small", "position": [5.0, 0.0, 0.0]},
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
    let (big_id, small_id) = (id_of("Big"), id_of("Small"));

    // Scale Big to (3,3,3) and Small to (0.5,0.5,0.5)
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute(
            "set_scale",
            json!({"entity_id": big_id, "sx": 3.0, "sy": 3.0, "sz": 3.0}),
        )
        .unwrap();
        m.execute(
            "set_scale",
            json!({"entity_id": small_id, "sx": 0.5, "sy": 0.5, "sz": 0.5}),
        )
        .unwrap();
    }
    app.update();
    app.update();

    // Select only Big
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("select_entity", json!({"entity_id": big_id}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("reset_selection_scale", json!({}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["reset_count"].as_u64().unwrap(), 1);
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
    let scale_of = |id: u64| {
        let arr = ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["scale"]
            .as_array()
            .unwrap()
            .clone();
        [
            arr[0].as_f64().unwrap() as f32,
            arr[1].as_f64().unwrap() as f32,
            arr[2].as_f64().unwrap() as f32,
        ]
    };
    let big_scale = scale_of(big_id);
    assert!(
        (big_scale[0] - 1.0).abs() < 0.01 && (big_scale[1] - 1.0).abs() < 0.01,
        "Big scale reset to 1"
    );
    let small_scale = scale_of(small_id);
    assert!((small_scale[0] - 0.5).abs() < 0.01, "Small scale unchanged");
}

#[test]
fn mcp_reset_selection_position_sets_position_to_origin() {
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
                    {"name": "A", "position": [3.0, 5.0, 7.0]},
                    {"name": "B", "position": [1.0, 2.0, 3.0]},
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
    let (a_id, b_id) = (id_of("A"), id_of("B"));

    // Select A only
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("select_entity", json!({"entity_id": a_id}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("reset_selection_position", json!({}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["reset_count"].as_u64().unwrap(), 1);
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
    let pos_of = |id: u64| {
        let arr = ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["position"]
            .as_array()
            .unwrap()
            .clone();
        [
            arr[0].as_f64().unwrap() as f32,
            arr[1].as_f64().unwrap() as f32,
            arr[2].as_f64().unwrap() as f32,
        ]
    };
    let a_pos = pos_of(a_id);
    assert!(
        (a_pos[0]).abs() < 0.01 && (a_pos[1]).abs() < 0.01 && (a_pos[2]).abs() < 0.01,
        "A position reset to origin"
    );
    let b_pos = pos_of(b_id);
    assert!((b_pos[0] - 1.0).abs() < 0.01, "B position unchanged");
}

#[test]
fn mcp_reset_selection_rotation_sets_rotation_to_zero() {
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
                    {"name": "R", "position": [0.0, 0.0, 0.0]},
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
    let r_id = all
        .iter()
        .find(|e| e["name"].as_str() == Some("R"))
        .unwrap()["id"]
        .as_u64()
        .unwrap();

    // Apply a rotation first
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute(
                "set_rotation",
                json!({"entity_id": r_id, "rx": 45.0, "ry": 30.0, "rz": 15.0}),
            )
            .unwrap();
    }
    app.update();
    app.update();

    // Select R
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("select_entity", json!({"entity_id": r_id}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("reset_selection_rotation", json!({}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["reset_count"].as_u64().unwrap(), 1);
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
    let rot = ents
        .iter()
        .find(|e| e["id"].as_u64() == Some(r_id))
        .unwrap()["rotation"]
        .as_array()
        .unwrap();
    assert!((rot[0].as_f64().unwrap()).abs() < 0.1, "rx reset to 0");
    assert!((rot[1].as_f64().unwrap()).abs() < 0.1, "ry reset to 0");
    assert!((rot[2].as_f64().unwrap()).abs() < 0.1, "rz reset to 0");
}

#[test]
fn mcp_toggle_visibility_on_selection_flips_visible_and_hidden() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("spawn_entity", json!({"name": "Vis"})).unwrap();
        m.execute("spawn_entity", json!({"name": "Hidden"}))
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
    let (vis_id, hid_id) = (id_of("Vis"), id_of("Hidden"));

    // Hide "Hidden"
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("hide_entity", json!({"entity_id": hid_id}))
            .unwrap();
    }
    app.update();
    app.update();

    // Select both
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("select_entity", json!({"entity_id": vis_id}))
            .unwrap();
        m.execute("select_entity", json!({"entity_id": hid_id}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("toggle_visibility_on_selection", json!({}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["toggled_count"].as_u64().unwrap(), 2);
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
    let vis_of = |id: u64| {
        ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["visible"]
            .as_bool()
            .unwrap()
    };
    assert!(!vis_of(vis_id), "Vis was visible, now hidden");
    assert!(vis_of(hid_id), "Hidden was hidden, now visible");
}

#[test]
fn mcp_expand_selection_to_children_adds_direct_children() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("spawn_entity", json!({"name": "Parent"}))
            .unwrap();
        m.execute("spawn_entity", json!({"name": "Child1"}))
            .unwrap();
        m.execute("spawn_entity", json!({"name": "Child2"}))
            .unwrap();
        m.execute("spawn_entity", json!({"name": "Orphan"}))
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
    let (par_id, c1_id, c2_id) = (id_of("Parent"), id_of("Child1"), id_of("Child2"));

    // Parent Child1 and Child2
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute(
            "set_parent",
            json!({"entity_id": c1_id, "parent_id": par_id}),
        )
        .unwrap();
        m.execute(
            "set_parent",
            json!({"entity_id": c2_id, "parent_id": par_id}),
        )
        .unwrap();
    }
    app.update();
    app.update();

    // Select only Parent
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("select_entity", json!({"entity_id": par_id}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("expand_selection_to_children", json!({}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["added_count"].as_u64().unwrap(), 2);
    }

    let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
    let sel: Vec<u64> = mcp
        .0
        .lock()
        .unwrap()
        .execute("get_selection", json!({}))
        .unwrap()
        .content["selected_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    assert!(sel.contains(&par_id));
    assert!(sel.contains(&c1_id));
    assert!(sel.contains(&c2_id));
    assert!(!sel.contains(&id_of("Orphan")));
}

#[test]
fn mcp_replace_tag_on_selection_swaps_tag_on_selected_entities() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("spawn_entity", json!({"name": "A"})).unwrap();
        m.execute("spawn_entity", json!({"name": "B"})).unwrap();
        m.execute("spawn_entity", json!({"name": "C"})).unwrap();
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

    // Tag A and B with "old_tag", C with "other"
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("tag_entity", json!({"entity_id": a_id, "tag": "old_tag"}))
            .unwrap();
        m.execute("tag_entity", json!({"entity_id": b_id, "tag": "old_tag"}))
            .unwrap();
        m.execute("tag_entity", json!({"entity_id": c_id, "tag": "other"}))
            .unwrap();
    }
    app.update();
    app.update();

    // Select A and C (not B)
    for id in [a_id, c_id] {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("select_entity", json!({"entity_id": id}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "replace_tag_on_selection",
                json!({"old_tag": "old_tag", "new_tag": "new_tag"}),
            )
            .unwrap();
        assert!(out.is_ok());
        // Only A is selected AND has "old_tag"
        assert_eq!(out.content["replaced_count"].as_u64().unwrap(), 1);
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
    let tags_of = |id: u64| {
        ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["tags"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect::<Vec<_>>()
    };
    assert!(
        tags_of(a_id).contains(&"new_tag".to_string()),
        "A should have new_tag"
    );
    assert!(
        !tags_of(a_id).contains(&"old_tag".to_string()),
        "A should not have old_tag"
    );
    assert!(
        tags_of(b_id).contains(&"old_tag".to_string()),
        "B (unselected) unchanged"
    );
    assert!(
        tags_of(c_id).contains(&"other".to_string()),
        "C other tag unchanged"
    );
}

#[test]
fn mcp_clear_tags_on_selection_removes_all_tags_from_selected() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("spawn_entity", json!({"name": "X"})).unwrap();
        m.execute("spawn_entity", json!({"name": "Y"})).unwrap();
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
    let (x_id, y_id) = (id_of("X"), id_of("Y"));

    // Tag X with two tags
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("tag_entity", json!({"entity_id": x_id, "tag": "alpha"}))
            .unwrap();
        m.execute("tag_entity", json!({"entity_id": x_id, "tag": "beta"}))
            .unwrap();
        m.execute("tag_entity", json!({"entity_id": y_id, "tag": "gamma"}))
            .unwrap();
    }
    app.update();
    app.update();

    // Select only X
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("select_entity", json!({"entity_id": x_id}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("clear_tags_on_selection", json!({}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["cleared_count"].as_u64().unwrap(), 1);
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
    let tags_of = |id: u64| {
        ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["tags"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect::<Vec<_>>()
    };
    assert!(tags_of(x_id).is_empty(), "X tags cleared");
    assert!(tags_of(y_id).contains(&"gamma".to_string()), "Y unchanged");
}

#[test]
fn mcp_add_prefix_to_selection_prepends_prefix_to_names() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("spawn_entity", json!({"name": "Goblin"}))
            .unwrap();
        m.execute("spawn_entity", json!({"name": "Orc"})).unwrap();
        m.execute("spawn_entity", json!({"name": "Unselected"}))
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
    let (gob_id, orc_id) = (id_of("Goblin"), id_of("Orc"));

    for id in [gob_id, orc_id] {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("select_entity", json!({"entity_id": id}))
            .unwrap();
    }
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("add_prefix_to_selection", json!({"prefix": "Enemy_"}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["renamed_count"].as_u64().unwrap(), 2);
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
    let name_of = |id: u64| {
        ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["name"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(name_of(gob_id), "Enemy_Goblin");
    assert_eq!(name_of(orc_id), "Enemy_Orc");
}

#[test]
fn mcp_add_suffix_to_selection_appends_suffix_to_names() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("spawn_entity", json!({"name": "Sword"})).unwrap();
        m.execute("spawn_entity", json!({"name": "Shield"}))
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
    let (sword_id, shield_id) = (id_of("Sword"), id_of("Shield"));

    for id in [sword_id, shield_id] {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("select_entity", json!({"entity_id": id}))
            .unwrap();
    }
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("add_suffix_to_selection", json!({"suffix": "_Prop"}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["renamed_count"].as_u64().unwrap(), 2);
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
    let name_of = |id: u64| {
        ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["name"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(name_of(sword_id), "Sword_Prop");
    assert_eq!(name_of(shield_id), "Shield_Prop");
}

#[test]
fn mcp_clone_selection_duplicates_all_selected_entities() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("spawn_entity", json!({"name": "A"})).unwrap();
        m.execute("spawn_entity", json!({"name": "B"})).unwrap();
    }
    app.update();
    app.update();

    let before_count = {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .len()
    };

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
    for id in [id_of("A"), id_of("B")] {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("select_entity", json!({"entity_id": id}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("clone_selection", json!({}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["duplicated_count"].as_u64().unwrap(), 2);
    }
    app.update();
    app.update();

    let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
    let after_count = mcp
        .0
        .lock()
        .unwrap()
        .execute("list_entities", json!({}))
        .unwrap()
        .content["entities"]
        .as_array()
        .unwrap()
        .len();
    assert_eq!(
        after_count,
        before_count + 2,
        "2 new entities after cloning 2 selected"
    );
}

#[test]
fn mcp_center_selection_moves_entities_to_origin() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    // Spawn two entities at (2,0,0) and (-2,0,0) — centroid is (0,0,0)
    // After centering, each moves by -(centroid) = (0,0,0), so positions remain
    // Let's use (4,0,0) and (2,0,0) — centroid is (3,0,0)
    // After centering: (4-3,0,0)=(1,0,0) and (2-3,0,0)=(-1,0,0)
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute(
                "batch_spawn",
                json!({
                    "entities": [
                        {"name": "A", "position": [4.0, 0.0, 0.0]},
                        {"name": "B", "position": [2.0, 0.0, 0.0]}
                    ]
                }),
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
    let (a_id, b_id) = (id_of("A"), id_of("B"));

    for id in [a_id, b_id] {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("select_entity", json!({"entity_id": id}))
            .unwrap();
    }
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("center_selection", json!({}))
            .unwrap();
        assert!(out.is_ok());
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
    let pos_of =
        |id: u64| ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["position"].clone();
    let ax = pos_of(a_id)[0].as_f64().unwrap();
    let bx = pos_of(b_id)[0].as_f64().unwrap();
    assert!(
        (ax - 1.0).abs() < 0.01,
        "A x should be 1.0 after center, got {}",
        ax
    );
    assert!(
        (bx - (-1.0)).abs() < 0.01,
        "B x should be -1.0 after center, got {}",
        bx
    );
}

#[test]
fn mcp_align_selection_to_ground_sets_y_to_zero() {
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
                json!({
                    "entities": [
                        {"name": "A", "position": [1.0, 5.0, 2.0]},
                        {"name": "B", "position": [3.0, -3.0, 0.0]}
                    ]
                }),
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
    let (a_id, b_id) = (id_of("A"), id_of("B"));

    for id in [a_id, b_id] {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("select_entity", json!({"entity_id": id}))
            .unwrap();
    }
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("align_selection_to_ground", json!({}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["aligned_count"].as_u64().unwrap(), 2);
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
    for id in [a_id, b_id] {
        let pos = &ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["position"];
        assert!(
            (pos[1].as_f64().unwrap()).abs() < 0.01,
            "Y should be 0 after aligning to ground"
        );
    }
}

#[test]
fn mcp_set_selection_rotation_applies_rotation_to_all_selected() {
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
                json!({
                    "entities": [
                        {"name": "A", "position": [0.0, 0.0, 0.0]},
                        {"name": "B", "position": [1.0, 0.0, 0.0]}
                    ]
                }),
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
    let (a_id, b_id) = (id_of("A"), id_of("B"));

    for id in [a_id, b_id] {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("select_entity", json!({"entity_id": id}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "set_selection_rotation",
                json!({"rx": 0.0, "ry": 90.0, "rz": 0.0}),
            )
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["rotated_count"].as_u64().unwrap(), 2);
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
    for id in [a_id, b_id] {
        let rot = &ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["rotation"];
        assert!(
            (rot[1].as_f64().unwrap() - 90.0).abs() < 0.1,
            "ry should be 90°"
        );
    }
}

#[test]
fn mcp_set_selection_scale_applies_scale_to_all_selected() {
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
                json!({
                    "entities": [
                        {"name": "A", "position": [0.0, 0.0, 0.0]},
                        {"name": "B", "position": [1.0, 0.0, 0.0]}
                    ]
                }),
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
    let (a_id, b_id) = (id_of("A"), id_of("B"));

    for id in [a_id, b_id] {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("select_entity", json!({"entity_id": id}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "set_selection_scale",
                json!({"sx": 3.0, "sy": 3.0, "sz": 3.0}),
            )
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["scaled_count"].as_u64().unwrap(), 2);
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
    for id in [a_id, b_id] {
        let sc = &ents.iter().find(|e| e["id"].as_u64() == Some(id)).unwrap()["scale"];
        assert!(
            (sc[0].as_f64().unwrap() - 3.0).abs() < 0.01,
            "sx should be 3.0"
        );
    }
}

#[test]
fn mcp_snap_selection_to_grid_rounds_position_to_grid_size() {
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
                json!({
                    "entities": [{"name": "E", "position": [1.3, 2.7, -0.4]}]
                }),
            )
            .unwrap();
    }
    app.update();
    app.update();

    let e_id = {
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
            .find(|e| e["name"].as_str() == Some("E"))
            .unwrap()["id"]
            .as_u64()
            .unwrap()
    };

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("select_entity", json!({"entity_id": e_id}))
            .unwrap();
    }
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("snap_selection_to_grid", json!({"grid_size": 1.0}))
            .unwrap();
        assert!(out.is_ok());
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
    let pos = ents
        .iter()
        .find(|e| e["id"].as_u64() == Some(e_id))
        .unwrap()["position"]
        .clone();
    let x = pos[0].as_f64().unwrap();
    let y = pos[1].as_f64().unwrap();
    let z = pos[2].as_f64().unwrap();
    assert!((x - 1.0).abs() < 0.01, "x snapped to 1.0, got {}", x);
    assert!((y - 3.0).abs() < 0.01, "y snapped to 3.0, got {}", y);
    assert!((z - 0.0).abs() < 0.01, "z snapped to 0.0, got {}", z);
}

#[test]
fn mcp_expand_selection_to_siblings_adds_all_same_parent_entities() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    // Root → [A, B, C]; Unrelated (no parent)
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("spawn_entity", json!({"name": "Root"})).unwrap();
        m.execute("spawn_entity", json!({"name": "A"})).unwrap();
        m.execute("spawn_entity", json!({"name": "B"})).unwrap();
        m.execute("spawn_entity", json!({"name": "C"})).unwrap();
        m.execute("spawn_entity", json!({"name": "Unrelated"}))
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
    let (root_id, a_id, b_id, c_id) = (id_of("Root"), id_of("A"), id_of("B"), id_of("C"));

    // Parent A, B, C under Root
    for child in [a_id, b_id, c_id] {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute(
                "set_parent",
                json!({"entity_id": child, "parent_id": root_id}),
            )
            .unwrap();
        app.update();
        app.update();
    }

    // Select A only; expand_selection_to_siblings should add B and C
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("select_entity", json!({"entity_id": a_id}))
            .unwrap();
    }
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("expand_selection_to_siblings", json!({}))
            .unwrap();
        assert!(out.is_ok());
    }

    let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
    let sel_out = mcp
        .0
        .lock()
        .unwrap()
        .execute("get_selection", json!({}))
        .unwrap();
    let sel_ids: Vec<u64> = sel_out.content["selected_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    assert!(sel_ids.contains(&a_id), "A in selection");
    assert!(sel_ids.contains(&b_id), "B added as sibling");
    assert!(sel_ids.contains(&c_id), "C added as sibling");
}

#[test]
fn mcp_set_selection_parent_parents_all_selected_entities() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("spawn_entity", json!({"name": "Root"})).unwrap();
        m.execute("spawn_entity", json!({"name": "A"})).unwrap();
        m.execute("spawn_entity", json!({"name": "B"})).unwrap();
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
    let (root_id, a_id, b_id) = (id_of("Root"), id_of("A"), id_of("B"));

    // Select A and B
    for id in [a_id, b_id] {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("select_entity", json!({"entity_id": id}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("set_selection_parent", json!({"parent_id": root_id}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["parented_count"].as_u64().unwrap(), 2);
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
    let parent_of = |id: u64| {
        ents.iter()
            .find(|e| e["id"].as_u64() == Some(id))
            .and_then(|e| e["parent_id"].as_u64())
    };
    assert_eq!(parent_of(a_id), Some(root_id), "A parent = Root");
    assert_eq!(parent_of(b_id), Some(root_id), "B parent = Root");
    assert!(parent_of(root_id).is_none(), "Root has no parent");
}

#[test]
fn mcp_unparent_selection_removes_parent_from_selected_entities() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("spawn_entity", json!({"name": "Root"})).unwrap();
        m.execute("spawn_entity", json!({"name": "A"})).unwrap();
        m.execute("spawn_entity", json!({"name": "B"})).unwrap();
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
    let (root_id, a_id, b_id) = (id_of("Root"), id_of("A"), id_of("B"));

    // Parent A and B to Root first
    for child in [a_id, b_id] {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute(
                "set_parent",
                json!({"entity_id": child, "parent_id": root_id}),
            )
            .unwrap();
        app.update();
        app.update();
    }

    // Select A only, then unparent
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("select_entity", json!({"entity_id": a_id}))
            .unwrap();
    }
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("unparent_selection", json!({}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["unparented_count"].as_u64().unwrap(), 1);
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
    let parent_of = |id: u64| {
        ents.iter()
            .find(|e| e["id"].as_u64() == Some(id))
            .and_then(|e| e["parent_id"].as_u64())
    };
    assert!(parent_of(a_id).is_none(), "A unparented");
    assert_eq!(parent_of(b_id), Some(root_id), "B still has Root parent");
}

#[test]
fn mcp_rotate_selection_by_rotates_all_selected_entities() {
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
                json!({
                    "entities": [
                        {"name": "A", "position": [0.0, 0.0, 0.0]},
                        {"name": "B", "position": [1.0, 0.0, 0.0]}
                    ]
                }),
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
    let find_id = |name: &str| {
        all.iter()
            .find(|e| e["name"].as_str() == Some(name))
            .unwrap()["id"]
            .as_u64()
            .unwrap()
    };
    let (id_a, id_b) = (find_id("A"), find_id("B"));

    // Select A only
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("select_entity", json!({"entity_id": id_a}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "rotate_selection_by",
                json!({"drx": 0.0, "dry": 45.0, "drz": 0.0}),
            )
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["rotated_count"].as_u64().unwrap(), 1);
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
    let rot_y_of = |id: u64| {
        ents.iter()
            .find(|e| e["id"].as_u64() == Some(id))
            .and_then(|e| e["rotation"].as_array())
            .map(|r| r[1].as_f64().unwrap())
            .unwrap_or(f64::NAN)
    };
    assert!(
        (rot_y_of(id_a) - 45.0).abs() < 0.1,
        "A rotated 45° on Y, got {}",
        rot_y_of(id_a)
    );
    assert!(
        (rot_y_of(id_b) - 0.0).abs() < 0.1,
        "B rotation unchanged, got {}",
        rot_y_of(id_b)
    );
}

#[test]
fn mcp_move_selection_by_moves_all_selected_entities() {
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
                json!({
                    "entities": [
                        {"name": "A", "position": [0.0, 0.0, 0.0]},
                        {"name": "B", "position": [10.0, 0.0, 0.0]},
                        {"name": "C", "position": [20.0, 0.0, 0.0]}
                    ]
                }),
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
    let find_id = |name: &str| {
        all.iter()
            .find(|e| e["name"].as_str() == Some(name))
            .unwrap()["id"]
            .as_u64()
            .unwrap()
    };
    let (id_a, id_b, id_c) = (find_id("A"), find_id("B"), find_id("C"));

    // Select A and B
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("select_entity", json!({"entity_id": id_a}))
            .unwrap();
    }
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("select_entity", json!({"entity_id": id_b}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "move_selection_by",
                json!({"dx": 5.0, "dy": 0.0, "dz": 0.0}),
            )
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["moved_count"].as_u64().unwrap(), 2);
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
    let pos_of = |id: u64| {
        ents.iter()
            .find(|e| e["id"].as_u64() == Some(id))
            .and_then(|e| e["position"].as_array())
            .map(|p| p[0].as_f64().unwrap())
            .unwrap_or(f64::NAN)
    };
    assert!(
        (pos_of(id_a) - 5.0).abs() < 0.01,
        "A moved to 5, got {}",
        pos_of(id_a)
    );
    assert!(
        (pos_of(id_b) - 15.0).abs() < 0.01,
        "B moved to 15, got {}",
        pos_of(id_b)
    );
    assert!(
        (pos_of(id_c) - 20.0).abs() < 0.01,
        "C unchanged, got {}",
        pos_of(id_c)
    );
}

#[test]
fn mcp_scale_selection_by_scales_all_selected_entities() {
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
                json!({
                    "entities": [
                        {"name": "A", "position": [0.0, 0.0, 0.0]},
                        {"name": "B", "position": [1.0, 0.0, 0.0]}
                    ]
                }),
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
    let find_id = |name: &str| {
        all.iter()
            .find(|e| e["name"].as_str() == Some(name))
            .unwrap()["id"]
            .as_u64()
            .unwrap()
    };
    let (id_a, id_b) = (find_id("A"), find_id("B"));

    // Set initial scale for A
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute(
                "set_scale",
                json!({"entity_id": id_a, "sx": 2.0, "sy": 2.0, "sz": 2.0}),
            )
            .unwrap();
    }
    app.update();
    app.update();

    // Select A only
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("select_entity", json!({"entity_id": id_a}))
            .unwrap();
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "scale_selection_by",
                json!({"sx": 3.0, "sy": 1.0, "sz": 1.0}),
            )
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["scaled_count"].as_u64().unwrap(), 1);
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
    let scale_x_of = |id: u64| {
        ents.iter()
            .find(|e| e["id"].as_u64() == Some(id))
            .and_then(|e| e["scale"].as_array())
            .map(|s| s[0].as_f64().unwrap())
            .unwrap_or(f64::NAN)
    };
    // A started at (2,2,2), multiplied by (3,1,1) → (6,2,2)
    assert!(
        (scale_x_of(id_a) - 6.0).abs() < 0.01,
        "A scale_x = 6, got {}",
        scale_x_of(id_a)
    );
    // B untouched — batch_spawn gives default scale [1,1,1]
    assert!(
        (scale_x_of(id_b) - 1.0).abs() < 0.01,
        "B scale_x unchanged, got {}",
        scale_x_of(id_b)
    );
}

#[test]
fn mcp_add_tag_to_selection_tags_all_selected_entities() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("spawn_entity", json!({"name": "A"})).unwrap();
        m.execute("spawn_entity", json!({"name": "B"})).unwrap();
        m.execute("spawn_entity", json!({"name": "C"})).unwrap();
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
    let (id_a, id_b, id_c) = (ids[0], ids[1], ids[2]);

    // Select A and B
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("select_entity", json!({"entity_id": id_a}))
            .unwrap();
    }
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("select_entity", json!({"entity_id": id_b}))
            .unwrap();
    }

    // Add tag to all selected
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("add_tag_to_selection", json!({"tag": "hero"}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["tagged_count"].as_u64().unwrap(), 2);
    }
    app.update();
    app.update();

    let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
    let snap = mcp
        .0
        .lock()
        .unwrap()
        .execute("get_entity", json!({"entity_id": id_a}))
        .unwrap();
    assert!(
        snap.content["entity"]["tags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t.as_str() == Some("hero")),
        "A tagged"
    );
    let snap_b = mcp
        .0
        .lock()
        .unwrap()
        .execute("get_entity", json!({"entity_id": id_b}))
        .unwrap();
    assert!(
        snap_b.content["entity"]["tags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t.as_str() == Some("hero")),
        "B tagged"
    );
    let snap_c = mcp
        .0
        .lock()
        .unwrap()
        .execute("get_entity", json!({"entity_id": id_c}))
        .unwrap();
    assert!(
        !snap_c.content["entity"]["tags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t.as_str() == Some("hero")),
        "C not tagged"
    );
}

#[test]
fn mcp_remove_tag_from_selection_untags_all_selected_entities() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("spawn_entity", json!({"name": "A"})).unwrap();
        m.execute("spawn_entity", json!({"name": "B"})).unwrap();
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
    let (id_a, id_b) = (ids[0], ids[1]);

    // Tag both entities
    for id in [id_a, id_b] {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("tag_entity", json!({"entity_id": id, "tag": "hero"}))
            .unwrap();
        app.update();
        app.update();
    }
    // Select A only
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("select_entity", json!({"entity_id": id_a}))
            .unwrap();
    }
    // Remove tag from selection
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute("remove_tag_from_selection", json!({"tag": "hero"}))
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["untagged_count"].as_u64().unwrap(), 1);
    }
    app.update();
    app.update();

    let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
    let snap_a = mcp
        .0
        .lock()
        .unwrap()
        .execute("get_entity", json!({"entity_id": id_a}))
        .unwrap();
    assert!(
        !snap_a.content["entity"]["tags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t.as_str() == Some("hero")),
        "A untagged"
    );
    let snap_b = mcp
        .0
        .lock()
        .unwrap()
        .execute("get_entity", json!({"entity_id": id_b}))
        .unwrap();
    assert!(
        snap_b.content["entity"]["tags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t.as_str() == Some("hero")),
        "B still tagged"
    );
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
        .execute("get_entity", json!({"entity_id": id_a}))
        .unwrap();
    let s = &out_a.content["entity"]["scale"];
    assert!((s[0].as_f64().unwrap() - 2.0).abs() < 1e-3, "ScaleA sx=2");
    assert!((s[1].as_f64().unwrap() - 3.0).abs() < 1e-3, "ScaleA sy=3");
    assert!((s[2].as_f64().unwrap() - 0.5).abs() < 1e-3, "ScaleA sz=0.5");
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
        .execute("get_entity", json!({"entity_id": id_a}))
        .unwrap();
    let pb = mcp
        .0
        .lock()
        .unwrap()
        .execute("get_entity", json!({"entity_id": id_b}))
        .unwrap();
    assert!(
        (pa.content["entity"]["position"][0].as_f64().unwrap() - 6.0).abs() < 1e-3,
        "OffA x = 1+5 = 6"
    );
    assert!(
        (pb.content["entity"]["position"][0].as_f64().unwrap() - 7.0).abs() < 1e-3,
        "OffB x = 2+5 = 7"
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
        .execute("get_entity", json!({"entity_id": id_a}))
        .unwrap();
    let pb = mcp
        .0
        .lock()
        .unwrap()
        .execute("get_entity", json!({"entity_id": id_b}))
        .unwrap();
    assert!(
        (pa.content["entity"]["position"][0].as_f64().unwrap() - (-5.0)).abs() < 1e-3,
        "MirrorA x should be -5"
    );
    assert!(
        (pa.content["entity"]["position"][1].as_f64().unwrap() - 2.0).abs() < 1e-3,
        "MirrorA y unchanged"
    );
    assert!(
        (pb.content["entity"]["position"][0].as_f64().unwrap() - (-10.0)).abs() < 1e-3,
        "MirrorB x should be -10"
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
        .execute("get_entity", json!({"entity_id": id_a}))
        .unwrap();
    let pb = mcp
        .0
        .lock()
        .unwrap()
        .execute("get_entity", json!({"entity_id": id_b}))
        .unwrap();
    assert!(
        (pa.content["entity"]["position"][1].as_f64().unwrap() - 3.0).abs() < 1e-3,
        "MoveSelA y should be 3"
    );
    assert!(
        (pb.content["entity"]["position"][1].as_f64().unwrap() - 3.0).abs() < 1e-3,
        "MoveSelB y should be 3"
    );
    assert!(
        (pb.content["entity"]["position"][0].as_f64().unwrap() - 5.0).abs() < 1e-3,
        "MoveSelB x should still be 5"
    );
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
        .execute("get_entity", json!({"entity_id": id_a}))
        .unwrap();
    let pb = mcp
        .0
        .lock()
        .unwrap()
        .execute("get_entity", json!({"entity_id": id_b}))
        .unwrap();
    assert!(
        (pa.content["entity"]["position"][1].as_f64().unwrap() - 10.0).abs() < 1e-3,
        "AlignA y should be 10"
    );
    assert!(
        (pb.content["entity"]["position"][1].as_f64().unwrap() - 10.0).abs() < 1e-3,
        "AlignB y should be 10"
    );
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
