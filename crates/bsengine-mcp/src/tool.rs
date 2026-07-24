use serde_json::Value;

/// Result of running an `McpTool`'s handler: either success content or an
/// error message, never both.
#[derive(Debug)]
pub struct McpToolOutput {
    /// The tool's return value on success (`Value::Null` on error).
    pub content: Value,
    /// The failure message, present only when the tool call failed.
    pub error: Option<String>,
}

impl McpToolOutput {
    /// Builds a successful output wrapping the given content.
    pub fn success(content: Value) -> Self {
        Self {
            content,
            error: None,
        }
    }

    /// Builds a failed output carrying the given error message.
    pub fn error(msg: &str) -> Self {
        Self {
            content: Value::Null,
            error: Some(msg.to_string()),
        }
    }

    /// Returns true if the tool call succeeded (no error message set).
    pub fn is_ok(&self) -> bool {
        self.error.is_none()
    }
}

/// A single MCP tool: its name/description/input schema for discovery, plus
/// the handler that executes it.
pub struct McpTool {
    /// Unique tool name, used to look it up and to invoke it via `tools/call`.
    pub name: String,
    /// Human-readable description shown to MCP clients via `tools/list`.
    pub description: String,
    /// JSON Schema describing the handler's expected input, if any.
    pub input_schema: Option<Value>,
    /// The function invoked with the call's input to produce an `McpToolOutput`.
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
            input_schema: None,
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
            input_schema: None,
            handler: Box::new(|input| McpToolOutput::success(json!({"echo": input}))),
        };
        let result = (tool.handler)(json!({"msg": "hello"}));
        assert!(result.is_ok());
        assert_eq!(result.content["echo"]["msg"], "hello");
    }
}
