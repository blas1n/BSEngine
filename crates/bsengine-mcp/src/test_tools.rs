//! MCP tools that drive a headless `bsengine-runtime --test` session
//! through `SessionRegistry`: `test_session_start`/`test_session_stop` plus
//! passthrough control tools (`test_step`, `test_press_key`, ...).

use std::sync::Arc;

use serde_json::{json, Value};

use crate::session::SessionRegistry;
use crate::tool::{McpTool, McpToolOutput};

/// Builds the full set of `test_*` tools bound to a shared `SessionRegistry`.
pub fn test_tools(registry: Arc<SessionRegistry>) -> Vec<McpTool> {
    let mut tools = vec![start_tool(registry.clone()), stop_tool(registry.clone())];
    tools.extend(passthrough_tools(registry));
    tools
}

fn start_tool(registry: Arc<SessionRegistry>) -> McpTool {
    McpTool {
        name: "test_session_start".to_string(),
        description: "Starts a headless bsengine-runtime --test session for the given game \
            and returns a session_id to pass to the other test_* tools."
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "game": { "type": "string", "description": "Game folder name under games/" },
            },
            "required": ["game"],
        })),
        handler: Box::new(move |args| {
            let game = match args.get("game").and_then(|v| v.as_str()) {
                Some(g) => g,
                None => return McpToolOutput::error("missing required field: game"),
            };
            match registry.start_session(game) {
                Ok(session_id) => McpToolOutput::success(json!({ "session_id": session_id })),
                Err(e) => McpToolOutput::error(&e),
            }
        }),
    }
}

fn stop_tool(registry: Arc<SessionRegistry>) -> McpTool {
    McpTool {
        name: "test_session_stop".to_string(),
        description: "Stops a headless test session and terminates its child process."
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "session_id": { "type": "string" },
            },
            "required": ["session_id"],
        })),
        handler: Box::new(move |args| {
            let session_id = match args.get("session_id").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return McpToolOutput::error("missing required field: session_id"),
            };
            match registry.stop_session(session_id) {
                Ok(()) => McpToolOutput::success(json!({ "stopped": session_id })),
                Err(e) => McpToolOutput::error(&e),
            }
        }),
    }
}

struct PassthroughSpec {
    tool_name: &'static str,
    child_cmd: &'static str,
    description: &'static str,
    input_schema: Value,
}

fn passthrough_tools(registry: Arc<SessionRegistry>) -> Vec<McpTool> {
    passthrough_specs()
        .into_iter()
        .map(|spec| build_passthrough_tool(spec, registry.clone()))
        .collect()
}

fn passthrough_specs() -> Vec<PassthroughSpec> {
    vec![
        PassthroughSpec {
            tool_name: "test_step",
            child_cmd: "step",
            description: "Advances the session's simulation by `frames` ticks, holding any \
                currently-pressed input constant across all of them.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" },
                    "frames": { "type": "integer", "minimum": 1 },
                },
                "required": ["session_id", "frames"],
            }),
        },
        PassthroughSpec {
            tool_name: "test_press_key",
            child_cmd: "press_key",
            description: "Injects a synthetic key-press into the session (same key names as \
                Bsengine.isKeyPressed: W A S D Space Enter Escape Up Down Left Right).",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" },
                    "key": { "type": "string" },
                },
                "required": ["session_id", "key"],
            }),
        },
        PassthroughSpec {
            tool_name: "test_release_key",
            child_cmd: "release_key",
            description: "Releases a previously-injected key press.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" },
                    "key": { "type": "string" },
                },
                "required": ["session_id", "key"],
            }),
        },
        PassthroughSpec {
            tool_name: "test_press_mouse",
            child_cmd: "press_mouse",
            description: "Injects a synthetic mouse-button press (0=Left, 1=Right, 2=Middle).",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" },
                    "button": { "type": "integer", "minimum": 0, "maximum": 2 },
                },
                "required": ["session_id", "button"],
            }),
        },
        PassthroughSpec {
            tool_name: "test_release_mouse",
            child_cmd: "release_mouse",
            description: "Releases a previously-injected mouse-button press.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" },
                    "button": { "type": "integer", "minimum": 0, "maximum": 2 },
                },
                "required": ["session_id", "button"],
            }),
        },
        PassthroughSpec {
            tool_name: "test_query_state",
            child_cmd: "query",
            description: "Reads live world state. `tool` is one of get_transform, get_visible, \
                get_entity_names; `args` are that query's parameters (e.g. {\"name\": \"Player\"}).",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" },
                    "tool": { "type": "string" },
                    "args": { "type": "object" },
                },
                "required": ["session_id", "tool", "args"],
            }),
        },
        PassthroughSpec {
            tool_name: "test_assert",
            child_cmd: "assert",
            description: "Runs `query`, extracts `path` (dot notation) from its result, and \
                compares it against `value` with `op` (==, !=, >, >=, <, <=, exists). Mechanical, \
                replayable — records exactly what a saved test log will re-check in CI.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" },
                    "query": {
                        "type": "object",
                        "properties": {
                            "tool": { "type": "string" },
                            "args": { "type": "object" },
                        },
                        "required": ["tool", "args"],
                    },
                    "path": { "type": "string" },
                    "op": { "type": "string" },
                    "value": {},
                    "label": { "type": "string" },
                },
                "required": ["session_id", "query", "path", "op", "value", "label"],
            }),
        },
    ]
}

fn build_passthrough_tool(spec: PassthroughSpec, registry: Arc<SessionRegistry>) -> McpTool {
    let child_cmd = spec.child_cmd;
    McpTool {
        name: spec.tool_name.to_string(),
        description: spec.description.to_string(),
        input_schema: Some(spec.input_schema),
        handler: Box::new(move |args| {
            let session_id = match args.get("session_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return McpToolOutput::error("missing required field: session_id"),
            };
            let command = match build_child_command(child_cmd, &args) {
                Ok(c) => c,
                Err(e) => return McpToolOutput::error(&e),
            };
            match registry.send(&session_id, command) {
                Ok(response) => mcp_output_from_response(response),
                Err(e) => McpToolOutput::error(&e),
            }
        }),
    }
}

/// Builds the child protocol command from MCP tool arguments: same fields
/// minus `session_id`, plus a `cmd` discriminator.
fn build_child_command(child_cmd: &str, args: &Value) -> Result<Value, String> {
    let mut obj = args
        .as_object()
        .cloned()
        .ok_or_else(|| "tool arguments must be an object".to_string())?;
    obj.remove("session_id");
    obj.insert("cmd".to_string(), json!(child_cmd));
    Ok(Value::Object(obj))
}

fn mcp_output_from_response(response: Value) -> McpToolOutput {
    let ok = response.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    if ok {
        McpToolOutput::success(response.get("data").cloned().unwrap_or(Value::Null))
    } else {
        let message = response
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error from test session")
            .to_string();
        McpToolOutput::error(&message)
    }
}
