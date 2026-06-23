use serde_json::Value;

#[derive(Debug)]
pub struct McpToolOutput {
    pub content: Value,
    pub error: Option<String>,
}

impl McpToolOutput {
    pub fn success(content: Value) -> Self {
        Self {
            content,
            error: None,
        }
    }

    pub fn error(msg: &str) -> Self {
        Self {
            content: Value::Null,
            error: Some(msg.to_string()),
        }
    }

    pub fn is_ok(&self) -> bool {
        self.error.is_none()
    }
}

pub struct McpTool {
    pub name: String,
    pub description: String,
    pub handler: Box<dyn Fn(Value) -> McpToolOutput + Send + Sync>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tool_output_success() {
        let out = McpToolOutput::success(json!({"result": 42}));
        assert!(out.is_ok());
        assert_eq!(out.content["result"], 42);
    }

    #[test]
    fn tool_output_error() {
        let out = McpToolOutput::error("something went wrong");
        assert!(!out.is_ok());
        assert!(out.error.as_ref().unwrap().contains("something went wrong"));
    }

    #[test]
    fn mcp_tool_has_name_and_description() {
        let tool = McpTool {
            name: "get_world_state".to_string(),
            description: "Returns current ECS world state".to_string(),
            handler: Box::new(|_input| McpToolOutput::success(json!({}))),
        };
        assert_eq!(tool.name, "get_world_state");
        assert!(tool.description.contains("ECS"));
    }

    #[test]
    fn mcp_tool_handler_returns_output() {
        let tool = McpTool {
            name: "echo".to_string(),
            description: "Echoes input".to_string(),
            handler: Box::new(|input| McpToolOutput::success(json!({"echo": input}))),
        };
        let result = (tool.handler)(json!({"msg": "hello"}));
        assert!(result.is_ok());
        assert_eq!(result.content["echo"]["msg"], "hello");
    }
}
