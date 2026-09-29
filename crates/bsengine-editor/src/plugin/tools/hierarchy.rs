//! MCP tools: Parenting.

use super::*;

/// Registers this group's tools; see [`super::register_all`].
pub(super) fn register(mcp: &McpRegistryResource, cx: &ToolContext) {
    let ToolContext { cmd_queue, .. } = cx;

    // set_parent
    let queue23 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "set_parent".to_string(),
        description: "Set the parent of an entity to establish a hierarchy (applied next frame)"
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "integer" },
                "parent_id": { "type": "integer" }
            },
            "required": ["entity_id", "parent_id"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            let parent_id = match input["parent_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing parent_id"),
            };
            queue23.lock().unwrap().push(EditorCommand::SetParent {
                entity_id,
                parent_id,
            });
            McpToolOutput::success(
                json!({"status": "queued", "entity_id": entity_id, "parent_id": parent_id}),
            )
        }),
    });

    // remove_parent
    let queue24 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "remove_parent".to_string(),
        description: "Remove the parent of an entity, making it a root entity (applied next frame)"
            .to_string(),
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
            queue24
                .lock()
                .unwrap()
                .push(EditorCommand::RemoveParent { entity_id });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });
}
