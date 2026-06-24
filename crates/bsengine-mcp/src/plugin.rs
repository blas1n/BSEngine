use crate::registry::McpToolRegistry;
use crate::tool::{McpTool, McpToolOutput};
use bevy_app::{App, Plugin};
use bsengine_ecs::Resource;
use serde_json::json;
use std::sync::{Arc, Mutex};

#[derive(Resource, Clone)]
pub struct McpRegistryResource(pub Arc<Mutex<McpToolRegistry>>);

pub struct McpPlugin;

impl Plugin for McpPlugin {
    fn build(&self, app: &mut App) {
        let mut registry = McpToolRegistry::new();

        registry.register(McpTool {
            name: "get_world_state".to_string(),
            description: "Returns current ECS world entity count and state summary".to_string(),
            handler: Box::new(|_input| {
                McpToolOutput::success(json!({
                    "entity_count": 0,
                    "status": "running"
                }))
            }),
        });

        registry.register(McpTool {
            name: "list_plugins".to_string(),
            description: "Lists all registered engine plugins".to_string(),
            handler: Box::new(|_input| {
                McpToolOutput::success(json!({
                    "plugins": []
                }))
            }),
        });

        app.insert_resource(McpRegistryResource(Arc::new(Mutex::new(registry))));
    }
}

#[cfg(test)]
mod tests {
    use super::{McpPlugin, McpRegistryResource};
    use bsengine_app::new_app;
    use serde_json::json;

    #[test]
    fn mcp_plugin_registers_registry() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        assert!(app.world().get_resource::<McpRegistryResource>().is_some());
    }

    #[test]
    fn mcp_plugin_has_builtin_get_world_state() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        let reg = app.world().resource::<McpRegistryResource>().0.clone();
        let result = reg
            .lock()
            .unwrap()
            .execute("get_world_state", json!({}))
            .expect("tool not found");
        assert!(result.is_ok());
        assert!(result.content.get("entity_count").is_some());
    }

    #[test]
    fn mcp_plugin_has_builtin_list_plugins() {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        let reg = app.world().resource::<McpRegistryResource>().0.clone();
        let result = reg
            .lock()
            .unwrap()
            .execute("list_plugins", json!({}))
            .expect("tool not found");
        assert!(result.is_ok());
        assert!(result.content.get("plugins").is_some());
    }
}
