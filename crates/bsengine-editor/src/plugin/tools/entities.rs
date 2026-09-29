//! MCP tools: Creating, removing, duplicating and renaming entities.

use super::*;

/// Registers this group's tools; see [`super::register_all`].
pub(super) fn register(mcp: &McpRegistryResource, cx: &ToolContext) {
    let ToolContext {
        snapshot,
        cmd_queue,
        ..
    } = cx;

    // spawn_entity
    let queue = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "spawn_entity".to_string(),
        description: "Spawn a new named entity (applied next frame)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "name": { "type": "string", "description": "Entity name" } },
            "required": ["name"]
        })),
        handler: Box::new(move |input| {
            let name = input["name"].as_str().unwrap_or("Entity").to_string();
            queue
                .lock()
                .unwrap()
                .push(EditorCommand::SpawnNamed(name.clone()));
            McpToolOutput::success(json!({"status": "queued", "name": name}))
        }),
    });

    // despawn_entity
    let queue3 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "despawn_entity".to_string(),
        description: "Despawn an entity by ID (applied next frame)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "id": { "type": "number", "description": "Entity ID" } },
            "required": ["id"]
        })),
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

    // rename_entity
    let queue12 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "rename_entity".to_string(),
        description: "Rename an entity by ID (applied next frame)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "number", "description": "Entity ID" },
                "name":      { "type": "string", "description": "New name" }
            },
            "required": ["entity_id", "name"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(v) => v,
                None => return McpToolOutput::error("missing numeric 'entity_id' field"),
            };
            let name = match input["name"].as_str() {
                Some(n) => n.to_string(),
                None => return McpToolOutput::error("missing string 'name' field"),
            };
            queue12
                .lock()
                .unwrap()
                .push(EditorCommand::RenameEntity { entity_id, name });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });

    // batch_spawn
    let queue16 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "batch_spawn".to_string(),
        description: "Spawn multiple named entities at once (applied next frame)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entities": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name":     { "type": "string" },
                            "position": { "type": "array", "items": {"type":"number"}, "description": "[x,y,z]" }
                        },
                        "required": ["name"]
                    }
                }
            },
            "required": ["entities"]
        })),
        handler: Box::new(move |input| {
            let items = match input["entities"].as_array() {
                Some(a) => a,
                None => return McpToolOutput::error("missing array 'entities' field"),
            };
            let entries: Vec<(String, Option<[f32; 3]>)> = items
                .iter()
                .filter_map(|item| {
                    let name = item["name"].as_str()?.to_string();
                    let pos = parse_vec3_input(&item["position"]);
                    Some((name, pos))
                })
                .collect();
            let count = entries.len();
            queue16
                .lock()
                .unwrap()
                .push(EditorCommand::BatchSpawn { entries });
            McpToolOutput::success(json!({"status": "queued", "count": count}))
        }),
    });

    // duplicate_entity
    let queue19 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "duplicate_entity".to_string(),
        description: "Duplicate an entity, copying its name, transform, mesh, and camera components (applied next frame)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "integer" }
            },
            "required": ["entity_id"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            queue19
                .lock()
                .unwrap()
                .push(EditorCommand::DuplicateEntity { entity_id });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });

    // rename_entity_with_suffix
    let queue32 = cmd_queue.clone();
    let snap_rews = snapshot.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "rename_entity_with_suffix".to_string(),
        description: "Append a suffix to an entity's current name".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "integer" },
                "suffix": { "type": "string" }
            },
            "required": ["entity_id", "suffix"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            let suffix = match input["suffix"].as_str() {
                Some(s) => s.to_string(),
                None => return McpToolOutput::error("missing suffix"),
            };
            let s = snap_rews.lock().unwrap();
            let current_name = match s.entities.iter().find(|e| e.id == entity_id) {
                Some(e) => e.name.clone().unwrap_or_default(),
                None => return McpToolOutput::error("entity not found"),
            };
            drop(s);
            let new_name = format!("{}{}", current_name, suffix);
            queue32.lock().unwrap().push(EditorCommand::RenameEntity {
                entity_id,
                name: new_name.clone(),
            });
            McpToolOutput::success(json!({"entity_id": entity_id, "new_name": new_name}))
        }),
    });

    // rename_entities_with_prefix
    let snap_rewp = snapshot.clone();
    let queue_rewp = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "rename_entities_with_prefix".to_string(),
        description:
            "Rename all entities whose name starts with old_prefix by replacing it with new_prefix"
                .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "old_prefix": { "type": "string" },
                "new_prefix": { "type": "string" }
            },
            "required": ["old_prefix", "new_prefix"]
        })),
        handler: Box::new(move |input| {
            let old_p = match input["old_prefix"].as_str() {
                Some(p) => p.to_string(),
                None => return McpToolOutput::error("missing old_prefix"),
            };
            let new_p = match input["new_prefix"].as_str() {
                Some(p) => p.to_string(),
                None => return McpToolOutput::error("missing new_prefix"),
            };
            let renames: Vec<(u64, String)> = {
                let s = snap_rewp.lock().unwrap();
                s.entities
                    .iter()
                    .filter_map(|e| {
                        let name = e.name.as_deref()?;
                        if name.starts_with(&*old_p) {
                            let new_name = format!("{}{}", new_p, &name[old_p.len()..]);
                            Some((e.id, new_name))
                        } else {
                            None
                        }
                    })
                    .collect()
            };
            let count = renames.len() as u64;
            let mut q = queue_rewp.lock().unwrap();
            for (id, name) in renames {
                q.push(crate::snapshot::EditorCommand::RenameEntity {
                    entity_id: id,
                    name,
                });
            }
            McpToolOutput::success(json!({"renamed_count": count}))
        }),
    });
}
