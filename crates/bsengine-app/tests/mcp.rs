use bsengine_app::new_app;
use bsengine_mcp::{McpPlugin, McpRegistryResource, McpTool, McpToolOutput};
use serde_json::json;

#[test]
fn mcp_full_tool_workflow() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.update();

    // Verify built-in tools exist
    {
        let reg = app.world().resource::<McpRegistryResource>().0.clone();
        let locked = reg.lock().unwrap();
        let tools = locked.list_tools();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(
            names.contains(&"get_world_state"),
            "missing get_world_state: {:?}",
            names
        );
        assert!(
            names.contains(&"list_plugins"),
            "missing list_plugins: {:?}",
            names
        );
    }

    // Register custom tool at runtime
    {
        let reg = app.world().resource::<McpRegistryResource>().0.clone();
        reg.lock().unwrap().register(McpTool {
            name: "custom_tool".to_string(),
            description: "A custom game tool".to_string(),
            input_schema: None,
            handler: Box::new(|_| McpToolOutput::success(json!({"status": "custom_ok"}))),
        });
    }

    // Execute custom tool
    {
        let reg = app.world().resource::<McpRegistryResource>().0.clone();
        let result = reg
            .lock()
            .unwrap()
            .execute("custom_tool", json!({}))
            .expect("custom tool not found");
        assert!(result.is_ok());
        assert_eq!(result.content["status"], "custom_ok");
    }
}
