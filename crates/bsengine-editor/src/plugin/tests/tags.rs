//! Tag tools.

use super::*;

#[test]
fn mcp_set_visibility_by_tag_hides_all_entities_with_given_tag() {
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

    // Tag A and B with "prop"
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let reg = mcp.0.lock().unwrap();
        reg.execute("tag_entity", json!({"entity_id": a_id, "tag": "prop"}))
            .unwrap();
        reg.execute("tag_entity", json!({"entity_id": b_id, "tag": "prop"}))
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
                "set_visibility_by_tag",
                json!({"tag": "prop", "visible": false}),
            )
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["affected_count"], 2);
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
    assert!(!a["visible"].as_bool().unwrap(), "A hidden");
    assert!(!b["visible"].as_bool().unwrap(), "B hidden");
    assert!(c["visible"].as_bool().unwrap(), "C still visible");
}

#[test]
fn mcp_group_entities_by_tag_returns_map_of_tag_to_entity_ids() {
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

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("tag_entity", json!({"entity_id": id_a, "tag": "hero"}))
            .unwrap();
    }
    app.update();
    app.update();
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("tag_entity", json!({"entity_id": id_b, "tag": "enemy"}))
            .unwrap();
    }
    app.update();
    app.update();

    let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
    let out = mcp
        .0
        .lock()
        .unwrap()
        .execute("group_entities_by_tag", json!({}))
        .unwrap();
    assert!(out.is_ok());
    let groups = &out.content["groups"];
    let hero_ids: Vec<u64> = groups["hero"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e.as_u64().unwrap())
        .collect();
    let enemy_ids: Vec<u64> = groups["enemy"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e.as_u64().unwrap())
        .collect();
    assert!(hero_ids.contains(&id_a));
    assert!(enemy_ids.contains(&id_b));
}

#[test]
fn mcp_batch_tag_entities_adds_tag_to_multiple_entities() {
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
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "batch_tag_entities",
                json!({"entity_ids": [ids[0], ids[1]], "tag": "group"}),
            )
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["tagged_count"].as_u64().unwrap(), 2);
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
    let tagged: Vec<u64> = ents
        .iter()
        .filter(|e| {
            e["tags"]
                .as_array()
                .map(|t| t.iter().any(|v| v.as_str() == Some("group")))
                .unwrap_or(false)
        })
        .map(|e| e["id"].as_u64().unwrap())
        .collect();
    assert_eq!(tagged.len(), 2, "A and B tagged with 'group'");
}

#[test]
fn mcp_batch_untag_entities_removes_tag_from_multiple_entities() {
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
    for &eid in &ids {
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("tag_entity", json!({"entity_id": eid, "tag": "remove_me"}))
                .unwrap();
        }
        app.update();
        app.update();
    }
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "batch_untag_entities",
                json!({"entity_ids": [ids[0], ids[1]], "tag": "remove_me"}),
            )
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["untagged_count"].as_u64().unwrap(), 2);
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
        let tags = e["tags"].as_array().unwrap();
        assert!(
            !tags.iter().any(|t| t.as_str() == Some("remove_me")),
            "tag removed from all entities"
        );
    }
}

#[test]
fn mcp_clear_all_tags_from_entity_removes_all_tags() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("spawn_entity", json!({"name": "MultiTagged"}))
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
            .find(|e| e["name"].as_str() == Some("MultiTagged"))
            .unwrap()["id"]
            .as_u64()
            .unwrap()
    };
    for tag in &["foo", "bar"] {
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("tag_entity", json!({"entity_id": eid, "tag": tag}))
                .unwrap();
        }
        app.update();
        app.update();
    }
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("clear_all_tags_from_entity", json!({"entity_id": eid}))
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
    let tags = ents.iter().find(|e| e["id"].as_u64() == Some(eid)).unwrap()["tags"]
        .as_array()
        .unwrap();
    assert!(tags.is_empty(), "all tags cleared");
}

#[test]
fn mcp_copy_tags_from_entity_copies_all_tags() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("spawn_entity", json!({"name": "TagSource"}))
            .unwrap();
        m.execute("spawn_entity", json!({"name": "TagDest"}))
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
            .find(|e| e["name"].as_str() == Some("TagSource"))
            .unwrap()["id"]
            .as_u64()
            .unwrap();
        let d = ents
            .iter()
            .find(|e| e["name"].as_str() == Some("TagDest"))
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
            .execute("tag_entity", json!({"entity_id": src_id, "tag": "special"}))
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
                "copy_tags_from_entity",
                json!({"source_id": src_id, "target_id": dst_id}),
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
        .execute("get_entity", json!({"entity_id": dst_id}))
        .unwrap();
    assert!(out.is_ok());
    let tags: Vec<&str> = out.content["entity"]["tags"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t.as_str())
        .collect();
    assert!(tags.contains(&"special"), "special tag copied to dest");
}

#[test]
fn mcp_clear_all_tags_removes_all_tags_from_all_entities() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("spawn_entity", json!({"name": "CatA"})).unwrap();
        m.execute("spawn_entity", json!({"name": "CatB"})).unwrap();
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
            .find(|e| e["name"].as_str() == Some("CatA"))
            .unwrap()["id"]
            .as_u64()
            .unwrap();
        let b = ents
            .iter()
            .find(|e| e["name"].as_str() == Some("CatB"))
            .unwrap()["id"]
            .as_u64()
            .unwrap();
        (a, b)
    };
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("tag_entity", json!({"entity_id": id_a, "tag": "enemy"}))
            .unwrap();
    }
    app.update();
    app.update();
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("tag_entity", json!({"entity_id": id_b, "tag": "ally"}))
            .unwrap();
    }
    app.update();
    app.update();
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("clear_all_tags", json!({}))
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
        let tags = e["tags"].as_array().map(|a| a.len()).unwrap_or(0);
        assert_eq!(
            tags,
            0,
            "all tags cleared on {}",
            e["name"].as_str().unwrap_or("?")
        );
    }
}

#[test]
fn inspector_cmd_tag_and_untag_entity_via_existing_pipeline() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let eid = app.world_mut().spawn(Name("Target".to_string())).id();
    app.update();

    {
        let mut insp = app.world_mut().resource_mut::<InspectorState>();
        insp.cmd_queue.push(InspectorCmd::TagEntity {
            id: eid.index() as u64,
            tag: "enemy".to_string(),
        });
    }
    app.update();
    assert_eq!(
        app.world().get::<Tags>(eid).map(|t| t.0.clone()),
        Some(vec!["enemy".to_string()]),
    );

    {
        let mut insp = app.world_mut().resource_mut::<InspectorState>();
        insp.cmd_queue.push(InspectorCmd::UntagEntity {
            id: eid.index() as u64,
            tag: "enemy".to_string(),
        });
    }
    app.update();
    assert_eq!(
        app.world().get::<Tags>(eid).map(|t| t.0.clone()),
        Some(vec![]),
    );
}
