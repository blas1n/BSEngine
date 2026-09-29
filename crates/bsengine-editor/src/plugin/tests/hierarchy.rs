//! Parenting tools.

use super::*;

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
fn inspector_cmd_set_and_remove_parent_reparents_via_existing_pipeline() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let parent = app.world_mut().spawn(Name("Parent".to_string())).id();
    let child = app.world_mut().spawn(Name("Child".to_string())).id();
    app.update();

    {
        let mut insp = app.world_mut().resource_mut::<InspectorState>();
        insp.cmd_queue.push(InspectorCmd::SetParent {
            id: child.index() as u64,
            parent_id: parent.index() as u64,
        });
    }
    app.update();
    assert_eq!(
        app.world().get::<Parent>(child).map(|p| p.0),
        Some(parent),
        "child should be parented after SetParent"
    );

    {
        let mut insp = app.world_mut().resource_mut::<InspectorState>();
        insp.cmd_queue.push(InspectorCmd::RemoveParent {
            id: child.index() as u64,
        });
    }
    app.update();
    assert!(
        app.world().get::<Parent>(child).is_none(),
        "child should be unparented after RemoveParent"
    );
}
