use crate::{McpTool, McpToolOutput, McpToolRegistry};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::sync::{Arc, Mutex};

pub struct McpServer {
    registry: Arc<Mutex<McpToolRegistry>>,
}

impl McpServer {
    pub fn new(registry: Arc<Mutex<McpToolRegistry>>) -> Self {
        Self { registry }
    }

    /// Returns None for notifications (no response should be sent).
    pub fn handle_message(&self, request: Value) -> Option<Value> {
        let method = match request.get("method").and_then(|m| m.as_str()) {
            Some(m) => m,
            None => {
                let id = request.get("id").cloned().unwrap_or(Value::Null);
                return Some(self.error_response(id, -32600, "Invalid Request"));
            }
        };

        // Notifications have no "id" and must not receive a response.
        if request.get("id").is_none() || method.starts_with("notifications/") {
            return None;
        }

        let id = request["id"].clone();
        let params = request.get("params").cloned().unwrap_or(json!({}));

        let result = match method {
            "initialize" => self.handle_initialize(),
            "tools/list" => self.handle_tools_list(),
            "tools/call" => self.handle_tools_call(params),
            _ => return Some(self.error_response(id, -32601, "Method not found")),
        };

        Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result,
        }))
    }

    pub fn run_stdio(&self) {
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();
        for line in stdin.lock().lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };
            if line.trim().is_empty() {
                continue;
            }
            let request: Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => {
                    let resp = self.error_response(Value::Null, -32700, "Parse error");
                    let _ = writeln!(stdout, "{}", resp);
                    continue;
                }
            };
            if let Some(response) = self.handle_message(request) {
                let _ = writeln!(stdout, "{}", response);
                let _ = stdout.flush();
            }
        }
    }

    fn handle_initialize(&self) -> Value {
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "bsengine", "version": "0.1.0" },
        })
    }

    fn handle_tools_list(&self) -> Value {
        let reg = self.registry.lock().unwrap();
        let tools: Vec<Value> = reg
            .list_tools()
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "inputSchema": { "type": "object" },
                })
            })
            .collect();
        json!({ "tools": tools })
    }

    fn handle_tools_call(&self, params: Value) -> Value {
        let name = match params.get("name").and_then(|n| n.as_str()) {
            Some(n) => n.to_string(),
            None => {
                return json!({
                    "content": [{ "type": "text", "text": "missing 'name' param" }],
                    "isError": true,
                })
            }
        };
        let args = params.get("arguments").cloned().unwrap_or(json!({}));
        let reg = self.registry.lock().unwrap();
        match reg.execute(&name, args) {
            Ok(out) => {
                if out.is_ok() {
                    json!({
                        "content": [{ "type": "text", "text": out.content.to_string() }],
                    })
                } else {
                    json!({
                        "content": [{ "type": "text", "text": out.error.unwrap_or_default() }],
                        "isError": true,
                    })
                }
            }
            Err(e) => json!({
                "content": [{ "type": "text", "text": e }],
                "isError": true,
            }),
        }
    }

    fn error_response(&self, id: Value, code: i64, message: &str) -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": code, "message": message },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{McpTool, McpToolOutput, McpToolRegistry};
    use serde_json::json;

    fn make_server() -> McpServer {
        let mut reg = McpToolRegistry::new();
        reg.register(McpTool {
            name: "ping".to_string(),
            description: "Returns pong".to_string(),
            handler: Box::new(|_| McpToolOutput::success(json!({"pong": true}))),
        });
        McpServer::new(Arc::new(Mutex::new(reg)))
    }

    #[test]
    fn initialize_returns_protocol_version() {
        let server = make_server();
        let resp = server
            .handle_message(json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}
            }))
            .unwrap();
        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], 1);
        assert_eq!(resp["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(resp["result"]["serverInfo"]["name"], "bsengine");
    }

    #[test]
    fn tools_list_returns_registered_tools() {
        let server = make_server();
        let resp = server
            .handle_message(json!({
                "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}
            }))
            .unwrap();
        let tools = resp["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "ping");
        assert_eq!(tools[0]["description"], "Returns pong");
        assert!(tools[0]["inputSchema"].is_object());
    }

    #[test]
    fn tools_call_executes_tool() {
        let server = make_server();
        let resp = server
            .handle_message(json!({
                "jsonrpc": "2.0", "id": 3,
                "method": "tools/call",
                "params": { "name": "ping", "arguments": {} }
            }))
            .unwrap();
        assert!(resp.get("error").is_none());
        let content = resp["result"]["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "text");
        assert!(content[0]["text"].as_str().unwrap().contains("pong"));
    }

    #[test]
    fn tools_call_missing_tool_returns_error() {
        let server = make_server();
        let resp = server
            .handle_message(json!({
                "jsonrpc": "2.0", "id": 4,
                "method": "tools/call",
                "params": { "name": "no_such_tool", "arguments": {} }
            }))
            .unwrap();
        assert_eq!(resp["result"]["isError"], true);
    }

    #[test]
    fn unknown_method_returns_error() {
        let server = make_server();
        let resp = server
            .handle_message(json!({
                "jsonrpc": "2.0", "id": 5, "method": "unknown/method"
            }))
            .unwrap();
        assert!(resp.get("error").is_some());
        assert_eq!(resp["error"]["code"], -32601);
    }

    #[test]
    fn missing_method_returns_invalid_request() {
        let server = make_server();
        let resp = server
            .handle_message(json!({ "jsonrpc": "2.0", "id": 6 }))
            .unwrap();
        assert_eq!(resp["error"]["code"], -32600);
    }

    #[test]
    fn notification_initialized_returns_none() {
        let server = make_server();
        let resp = server.handle_message(json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }));
        assert!(resp.is_none(), "notifications must not produce a response");
    }

    #[test]
    fn notification_without_id_returns_none() {
        let server = make_server();
        let resp = server.handle_message(json!({
            "jsonrpc": "2.0",
            "method": "notifications/cancelled",
            "params": { "requestId": 3 }
        }));
        assert!(resp.is_none());
    }
}
