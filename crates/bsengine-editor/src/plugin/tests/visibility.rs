//! Visibility tools.

use super::*;

#[test]
fn mcp_toggle_entity_visibility_flips_visible_state() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("spawn_entity", json!({"name": "Toggler"}))
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
            .find(|e| e["name"].as_str() == Some("Toggler"))
            .unwrap()["id"]
            .as_u64()
            .unwrap()
    };

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("toggle_entity_visibility", json!({"entity_id": eid}))
            .unwrap();
    }
    app.update();
    app.update();
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let v1 = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_entity", json!({"entity_id": eid}))
            .unwrap();
        assert!(
            !v1.content["entity"]["visible"].as_bool().unwrap(),
            "after first toggle: hidden"
        );
    }

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("toggle_entity_visibility", json!({"entity_id": eid}))
            .unwrap();
    }
    app.update();
    app.update();

    let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
    let v2 = mcp
        .0
        .lock()
        .unwrap()
        .execute("get_entity", json!({"entity_id": eid}))
        .unwrap();
    assert!(
        v2.content["entity"]["visible"].as_bool().unwrap(),
        "after second toggle: visible again"
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
            assert!(
                !e["visible"].as_bool().unwrap_or(true),
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
        assert!(
            e["visible"].as_bool().unwrap_or(false),
            "all should be visible"
        );
    }
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
