//! MCP tools that drive a headless `bsengine-runtime --test` session
//! through `SessionRegistry`: `test_session_start`/`test_session_stop` plus
//! passthrough control tools (`test_step`, `test_press_key`, ...).

use std::sync::Arc;

use serde_json::{json, Value};

use crate::session::SessionRegistry;
use crate::tool::{McpTool, McpToolOutput};

/// Builds the full set of `test_*` tools bound to a shared `SessionRegistry`.
pub fn test_tools(registry: Arc<SessionRegistry>) -> Vec<McpTool> {
    let mut tools = vec![
        start_tool(registry.clone()),
        stop_tool(registry.clone()),
        save_recording_tool(registry.clone()),
        run_replay_tool(registry.clone()),
        asset_status_tool(registry.clone()),
    ];
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
        description: "Stops a headless test session and terminates its child process.".to_string(),
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

fn save_recording_tool(registry: Arc<SessionRegistry>) -> McpTool {
    McpTool {
        name: "test_save_recording".to_string(),
        description: "Saves the session's accumulated commands (step/press/release/assert/\
            wait_until, in order) as games/<game>/tests/<name>.testlog.json — replayable with \
            test_run_replay or `bsengine-runtime --test <game> --replay <file>` with no AI \
            involved."
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "session_id": { "type": "string" },
                "name": { "type": "string", "description": "Recording name, no extension" },
            },
            "required": ["session_id", "name"],
        })),
        handler: Box::new(move |args| {
            let session_id = match args.get("session_id").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return McpToolOutput::error("missing required field: session_id"),
            };
            let name = match args.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => return McpToolOutput::error("missing required field: name"),
            };
            match registry.save_recording(session_id, name) {
                Ok(path) => McpToolOutput::success(json!({ "saved": path.display().to_string() })),
                Err(e) => McpToolOutput::error(&e),
            }
        }),
    }
}

fn run_replay_tool(registry: Arc<SessionRegistry>) -> McpTool {
    McpTool {
        name: "test_run_replay".to_string(),
        description: "Runs a saved recording (games/<game>/tests/<name>.testlog.json) through \
            bsengine-runtime --test --replay with no AI involved — the same check CI's \
            'E2E replays' step runs over every recording under \
            games/*/assets/tests/. Lets Claude verify a recording still passes without \
            needing a CI run."
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "game": { "type": "string" },
                "name": { "type": "string" },
            },
            "required": ["game", "name"],
        })),
        handler: Box::new(move |args| {
            let game = match args.get("game").and_then(|v| v.as_str()) {
                Some(g) => g,
                None => return McpToolOutput::error("missing required field: game"),
            };
            let name = match args.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => return McpToolOutput::error("missing required field: name"),
            };
            match registry.run_replay(game, name) {
                Ok(result) => McpToolOutput::success(
                    json!({ "passed": result.passed, "stderr": result.stderr }),
                ),
                Err(e) => McpToolOutput::error(&e),
            }
        }),
    }
}

/// Asks a live session what became of one asset path.
///
/// # Why a tool of its own rather than only `test_query_state`
///
/// `get_asset_status` is reachable through `test_query_state` too (and
/// through `test_assert`/`test_wait_until`, which is the point of putting it
/// in the runtime's query table at all — a recording can now wait for a mesh
/// to load, or assert that nothing failed). But a query tool's name lives
/// inside another tool's *description*, and an agent that never reads that
/// sentence never learns the question is askable. This whole phase exists
/// because a failure that was only a `tracing::warn!` went unread across two
/// phases of work; hiding its replacement one level down would repeat the
/// mistake in a new place. `tools/list` names this one directly.
///
/// # Why `test_`-prefixed
///
/// It needs a `session_id`, and in this server that prefix is what says so —
/// every tool that talks to a live session carries it, every tool that does
/// not (`game_create`, `scene_write`, `game_validate`) does not. The plan
/// called this tool `get_asset_status`, on the assumption that an MCP tool
/// could read `AssetStatuses` directly; it cannot, because the MCP server is
/// a separate process from the engine and reaches it only through a spawned
/// `bsengine-runtime --test` child. The query *inside* the engine keeps the
/// plain name; the tool that needs a session is named like the other tools
/// that need one.
fn asset_status_tool(registry: Arc<SessionRegistry>) -> McpTool {
    McpTool {
        name: "test_get_asset_status".to_string(),
        description: "Reports what the engine knows about one asset path in a live test \
            session, as \"loaded\", \"loading\", \"failed: <reason>\" or \"unknown\" — the \
            same four answers Bsengine.getAssetStatus gives a script. \"unknown\" means \
            nothing ever requested that path, which is deliberately NOT the same answer as \
            a failure: a misspelled path reads \"unknown\", a real path that broke reads \
            \"failed: \" plus the reason. `path` must be spelled exactly as the load site \
            spelled it — the project directory this session was started with, then the \
            scene-/script-relative part, forward-slashed (e.g. \
            \"games/mini-arena/assets/models/fox.glb\"). An absolute or otherwise \
            re-spelled path reads \"unknown\", not an error. Also available as the \
            get_asset_status query tool, so test_assert/test_wait_until can gate a \
            recording on an asset actually loading."
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "session_id": { "type": "string" },
                "path": {
                    "type": "string",
                    "description": "Asset path as the load site spelled it, e.g. \
                        games/mini-arena/assets/models/fox.glb",
                },
            },
            "required": ["session_id", "path"],
        })),
        handler: Box::new(move |args| {
            let session_id = match args.get("session_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return McpToolOutput::error("missing required field: session_id"),
            };
            let path = match args.get("path").and_then(|v| v.as_str()) {
                Some(p) => p.to_string(),
                None => return McpToolOutput::error("missing required field: path"),
            };
            let command = json!({
                "cmd": "query",
                "tool": "get_asset_status",
                "args": { "path": path },
            });
            let response = match registry.send(&session_id, command) {
                Ok(r) => r,
                Err(e) => return McpToolOutput::error(&e),
            };
            // An object, unlike the bare string the JS op returns, because
            // every other tool here answers with one — and the echoed `path`
            // is what makes an `"unknown"` readable: it shows the spelling
            // that was actually asked about, which is the one thing that
            // separates "nobody requested it" from "you asked about a
            // different path than the engine loaded".
            match mcp_output_from_response(response) {
                out if out.is_ok() => {
                    McpToolOutput::success(json!({ "path": path, "status": out.content }))
                }
                err => err,
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
                get_entity_names, get_hud_text, get_asset_status; `args` are that query's \
                parameters (e.g. {\"name\": \"Player\"}, {\"id\": \"1\"}, {\"path\": \
                \"games/mini-arena/assets/models/fox.glb\"}).",
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
        PassthroughSpec {
            tool_name: "test_wait_until",
            child_cmd: "wait_until",
            description: "Advances the session until `query`'s `path` satisfies `op` against \
                `value`, or until `max_frames` frames have passed. Prefer this over test_step \
                when waiting for something to happen (a character to arrive, an entity to \
                despawn, an asset to finish loading): a fixed frame count only reproduces on \
                the machine that recorded it, because wall-clock-driven gameplay covers a \
                different distance per frame on different hardware.",
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
                    "max_frames": { "type": "integer", "minimum": 0 },
                    "label": { "type": "string" },
                },
                "required": ["session_id", "query", "path", "op", "value", "max_frames", "label"],
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
    let ok = response
        .get("ok")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
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
