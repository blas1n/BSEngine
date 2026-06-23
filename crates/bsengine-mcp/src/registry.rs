use std::collections::HashMap;
use serde_json::Value;
use crate::tool::{McpTool, McpToolOutput};

pub struct McpToolRegistry {
    tools: HashMap<String, McpTool>,
}

impl McpToolRegistry {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }

    pub fn register(&mut self, tool: McpTool) {
        self.tools.insert(tool.name.clone(), tool);
    }

    pub fn list_tools(&self) -> Vec<&McpTool> {
        self.tools.values().collect()
    }

    pub fn execute(&self, name: &str, input: Value) -> Result<McpToolOutput, String> {
        let tool = self.tools.get(name)
            .ok_or_else(|| format!("Tool '{name}' not found"))?;
        Ok((tool.handler)(input))
    }
}

impl Default for McpToolRegistry {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use crate::tool::{McpTool, McpToolOutput};

    fn make_echo_tool() -> McpTool {
        McpTool {
            name: "echo".to_string(),
            description: "Echoes input".to_string(),
            handler: Box::new(|input| McpToolOutput::success(json!({"echo": input}))),
        }
    }

    #[test]
    fn registry_register_and_list() {
        let mut reg = McpToolRegistry::new();
        reg.register(make_echo_tool());
        let tools = reg.list_tools();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "echo");
    }

    #[test]
    fn registry_execute_existing_tool() {
        let mut reg = McpToolRegistry::new();
        reg.register(make_echo_tool());
        let result = reg.execute("echo", json!({"msg": "hello"}));
        assert!(result.is_ok());
        assert!(result.unwrap().is_ok());
    }

    #[test]
    fn registry_execute_nonexistent_returns_error() {
        let reg = McpToolRegistry::new();
        let result = reg.execute("nonexistent", json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn registry_multiple_tools() {
        let mut reg = McpToolRegistry::new();
        reg.register(make_echo_tool());
        reg.register(McpTool {
            name: "version".to_string(),
            description: "Returns version".to_string(),
            handler: Box::new(|_| McpToolOutput::success(json!({"version": "0.1.0"}))),
        });
        assert_eq!(reg.list_tools().len(), 2);
        let v = reg.execute("version", json!({})).unwrap();
        assert_eq!(v.content["version"], "0.1.0");
    }
}
