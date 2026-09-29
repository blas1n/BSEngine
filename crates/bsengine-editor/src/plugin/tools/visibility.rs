//! MCP tools: Showing and hiding entities.

use super::*;

/// Registers this group's tools; see [`super::register_all`].
pub(super) fn register(mcp: &McpRegistryResource, cx: &ToolContext) {
    let ToolContext {
        snapshot,
        cmd_queue,
        ..
    } = cx;

    // toggle_entity_visibility
    let snap_tev = snapshot.clone();
    let queue34 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "toggle_entity_visibility".to_string(),
        description: "Toggle the visible state of an entity".to_string(),
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
            let s = snap_tev.lock().unwrap();
            let visible = match s.entities.iter().find(|e| e.id == entity_id) {
                Some(e) => e.visible,
                None => return McpToolOutput::error("entity not found"),
            };
            drop(s);
            queue34.lock().unwrap().push(EditorCommand::SetVisible {
                entity_id,
                visible: !visible,
            });
            McpToolOutput::success(json!({"entity_id": entity_id, "new_visible": !visible}))
        }),
    });

    // set_all_visible
    let snap_sav = snapshot.clone();
    let queue_sav = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "set_all_visible".to_string(),
        description: "Set visibility of every entity in the scene".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "visible": { "type": "boolean" } },
            "required": ["visible"]
        })),
        handler: Box::new(move |input| {
            let visible = match input["visible"].as_bool() {
                Some(v) => v,
                None => return McpToolOutput::error("missing visible"),
            };
            let ids: Vec<u64> = {
                let s = snap_sav.lock().unwrap();
                s.entities.iter().map(|e| e.id).collect()
            };
            let count = ids.len() as u64;
            let mut q = queue_sav.lock().unwrap();
            for id in ids {
                q.push(crate::snapshot::EditorCommand::SetVisible {
                    entity_id: id,
                    visible,
                });
            }
            McpToolOutput::success(json!({"updated_count": count}))
        }),
    });

    // toggle_visible
    let snap_toggle = snapshot.clone();
    let queue_toggle = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "toggle_visible".to_string(),
        description: "Toggle the visible state of an entity (applied next frame)".to_string(),
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
            let current = snap_toggle
                .lock()
                .unwrap()
                .entities
                .iter()
                .find(|e| e.id == entity_id)
                .map(|e| e.visible)
                .unwrap_or(true);
            queue_toggle
                .lock()
                .unwrap()
                .push(EditorCommand::SetVisible {
                    entity_id,
                    visible: !current,
                });
            McpToolOutput::success(
                json!({"status": "queued", "entity_id": entity_id, "new_visible": !current}),
            )
        }),
    });

    // hide_entity
    let queue27 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "hide_entity".to_string(),
        description: "Mark an entity as hidden (visible=false, applied next frame)".to_string(),
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
            queue27.lock().unwrap().push(EditorCommand::SetVisible {
                entity_id,
                visible: false,
            });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });

    // show_entity
    let queue28 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "show_entity".to_string(),
        description: "Mark an entity as visible (visible=true, applied next frame)".to_string(),
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
            queue28.lock().unwrap().push(EditorCommand::SetVisible {
                entity_id,
                visible: true,
            });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });
}
