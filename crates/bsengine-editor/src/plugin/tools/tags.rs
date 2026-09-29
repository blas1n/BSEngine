//! MCP tools: Tags on entities.

use super::*;

/// Registers this group's tools; see [`super::register_all`].
pub(super) fn register(mcp: &McpRegistryResource, cx: &ToolContext) {
    let ToolContext {
        snapshot,
        cmd_queue,
        ..
    } = cx;

    // tag_entity
    let queue25 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "tag_entity".to_string(),
        description: "Add a string tag to an entity (applied next frame)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "integer" },
                "tag": { "type": "string" }
            },
            "required": ["entity_id", "tag"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            let tag = match input["tag"].as_str() {
                Some(t) => t.to_string(),
                None => return McpToolOutput::error("missing tag"),
            };
            queue25
                .lock()
                .unwrap()
                .push(EditorCommand::TagEntity { entity_id, tag });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });

    // untag_entity
    let queue26 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "untag_entity".to_string(),
        description: "Remove a string tag from an entity (applied next frame)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "integer" },
                "tag": { "type": "string" }
            },
            "required": ["entity_id", "tag"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            let tag = match input["tag"].as_str() {
                Some(t) => t.to_string(),
                None => return McpToolOutput::error("missing tag"),
            };
            queue26
                .lock()
                .unwrap()
                .push(EditorCommand::UntagEntity { entity_id, tag });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });

    // set_visibility_by_tag
    let snap_svbt = snapshot.clone();
    let queue90 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "set_visibility_by_tag".to_string(),
        description: "Show or hide all entities that have a given tag; returns {affected_count}"
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "tag": { "type": "string" },
                "visible": { "type": "boolean" }
            },
            "required": ["tag", "visible"]
        })),
        handler: Box::new(move |input| {
            let tag = match input["tag"].as_str() {
                Some(t) => t.to_string(),
                None => return McpToolOutput::error("missing tag"),
            };
            let visible = match input["visible"].as_bool() {
                Some(v) => v,
                None => return McpToolOutput::error("missing visible"),
            };
            let s = snap_svbt.lock().unwrap();
            let ids: Vec<u64> = s
                .entities
                .iter()
                .filter(|e| e.tags.contains(&tag))
                .map(|e| e.id)
                .collect();
            drop(s);
            let count = ids.len() as u64;
            let mut q = queue90.lock().unwrap();
            for entity_id in ids {
                q.push(EditorCommand::SetVisible { entity_id, visible });
            }
            McpToolOutput::success(json!({"affected_count": count}))
        }),
    });

    // group_entities_by_tag
    let snap_gebt = snapshot.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "group_entities_by_tag".to_string(),
        description: "Return a map of tag → list of entity IDs that have that tag".to_string(),
        input_schema: Some(json!({"type": "object", "properties": {}})),
        handler: Box::new(move |_input| {
            let s = snap_gebt.lock().unwrap();
            let mut groups: std::collections::HashMap<String, Vec<u64>> =
                std::collections::HashMap::new();
            for e in &s.entities {
                for tag in &e.tags {
                    groups.entry(tag.clone()).or_default().push(e.id);
                }
            }
            let groups_json: serde_json::Map<String, serde_json::Value> = groups
                .into_iter()
                .map(|(k, v)| {
                    (
                        k,
                        serde_json::Value::Array(v.into_iter().map(|id| json!(id)).collect()),
                    )
                })
                .collect();
            McpToolOutput::success(json!({"groups": serde_json::Value::Object(groups_json)}))
        }),
    });

    // select/deselect/count for intensity_above, intensity_below, range_above

    // select/deselect for position; select/deselect/count for position_above, position_below, position_in_box

    // batch_tag_entities
    let queue36 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "batch_tag_entities".to_string(),
        description: "Add a tag to multiple entities at once".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_ids": { "type": "array", "items": { "type": "integer" } },
                "tag": { "type": "string" }
            },
            "required": ["entity_ids", "tag"]
        })),
        handler: Box::new(move |input| {
            let tag = match input["tag"].as_str() {
                Some(t) => t.to_string(),
                None => return McpToolOutput::error("missing tag"),
            };
            let ids: Vec<u64> = match input["entity_ids"].as_array() {
                Some(arr) => arr.iter().filter_map(|v| v.as_u64()).collect(),
                None => return McpToolOutput::error("missing entity_ids"),
            };
            let count = ids.len() as u64;
            let mut q = queue36.lock().unwrap();
            for entity_id in ids {
                q.push(EditorCommand::TagEntity {
                    entity_id,
                    tag: tag.clone(),
                });
            }
            McpToolOutput::success(json!({"tagged_count": count}))
        }),
    });

    // batch_untag_entities
    let queue37 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "batch_untag_entities".to_string(),
        description: "Remove a tag from multiple entities at once".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_ids": { "type": "array", "items": { "type": "integer" } },
                "tag": { "type": "string" }
            },
            "required": ["entity_ids", "tag"]
        })),
        handler: Box::new(move |input| {
            let tag = match input["tag"].as_str() {
                Some(t) => t.to_string(),
                None => return McpToolOutput::error("missing tag"),
            };
            let ids: Vec<u64> = match input["entity_ids"].as_array() {
                Some(arr) => arr.iter().filter_map(|v| v.as_u64()).collect(),
                None => return McpToolOutput::error("missing entity_ids"),
            };
            let count = ids.len() as u64;
            let mut q = queue37.lock().unwrap();
            for entity_id in ids {
                q.push(EditorCommand::UntagEntity {
                    entity_id,
                    tag: tag.clone(),
                });
            }
            McpToolOutput::success(json!({"untagged_count": count}))
        }),
    });

    // clear_all_tags_from_entity
    let snap_catfe = snapshot.clone();
    let queue35 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "clear_all_tags_from_entity".to_string(),
        description: "Remove all tags from an entity".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "entity_id": { "type": "integer" } },
            "required": ["entity_id"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            let s = snap_catfe.lock().unwrap();
            let tags = match s.entities.iter().find(|e| e.id == entity_id) {
                Some(e) => e.tags.clone(),
                None => return McpToolOutput::error("entity not found"),
            };
            drop(s);
            let mut q = queue35.lock().unwrap();
            for tag in tags {
                q.push(EditorCommand::UntagEntity { entity_id, tag });
            }
            McpToolOutput::success(json!({"entity_id": entity_id, "status": "queued"}))
        }),
    });

    // copy_tags_from_entity
    let snap_cte = snapshot.clone();
    let queue30 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "copy_tags_from_entity".to_string(),
        description: "Copy all tags from a source entity to a target entity".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "source_id": { "type": "integer" },
                "target_id": { "type": "integer" }
            },
            "required": ["source_id", "target_id"]
        })),
        handler: Box::new(move |input| {
            let source_id = match input["source_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing source_id"),
            };
            let target_id = match input["target_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing target_id"),
            };
            let tags = {
                let s = snap_cte.lock().unwrap();
                match s.entities.iter().find(|e| e.id == source_id) {
                    Some(e) => e.tags.clone(),
                    None => return McpToolOutput::error("source entity not found"),
                }
            };
            let count = tags.len() as u64;
            let mut q = queue30.lock().unwrap();
            for tag in tags {
                q.push(EditorCommand::TagEntity {
                    entity_id: target_id,
                    tag,
                });
            }
            McpToolOutput::success(json!({"status": "queued", "tags_copied": count}))
        }),
    });

    // clear_all_tags
    let snap_cat = snapshot.clone();
    let queue_cat = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "clear_all_tags".to_string(),
        description: "Remove all tags from every entity in the scene".to_string(),
        input_schema: Some(json!({"type": "object", "properties": {}})),
        handler: Box::new(move |_input| {
            let pairs: Vec<(u64, String)> = {
                let s = snap_cat.lock().unwrap();
                s.entities
                    .iter()
                    .flat_map(|e| e.tags.iter().map(move |t| (e.id, t.clone())))
                    .collect()
            };
            let count = pairs.len() as u64;
            let mut q = queue_cat.lock().unwrap();
            for (id, tag) in pairs {
                q.push(crate::snapshot::EditorCommand::UntagEntity { entity_id: id, tag });
            }
            McpToolOutput::success(json!({"removed_tag_count": count}))
        }),
    });
}
