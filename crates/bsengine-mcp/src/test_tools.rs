//! MCP tools that drive a headless `bsengine-runtime --test` session
//! through `SessionRegistry`: `test_session_start`/`test_session_stop` plus
//! passthrough control tools (`test_step`, `test_press_key`, ...).

use std::sync::Arc;

use serde_json::{json, Value};

use crate::session::SessionRegistry;
use crate::tool::{McpTool, McpToolOutput};

/// The one answer from `get_asset_status` that a caller cannot tell apart from
/// a typo, and so the only one worth spending a second round trip on.
///
/// Spelled out here rather than imported: this crate deliberately does not
/// depend on the engine (it drives `bsengine-runtime` as a child process over
/// a text protocol, and building the whole engine to read one string literal
/// would invert that). `bsengine_scripting::ops::render_asset_status` owns the
/// vocabulary; `asset_status_answers_unknown_with_the_paths_it_does_know`
/// pins this copy against a live session, so a rename there fails here.
const ASSET_STATUS_UNKNOWN: &str = "unknown";

/// Builds the full set of `test_*` tools bound to a shared `SessionRegistry`.
pub fn test_tools(registry: Arc<SessionRegistry>) -> Vec<McpTool> {
    let mut tools = vec![
        start_tool(registry.clone()),
        stop_tool(registry.clone()),
        save_recording_tool(registry.clone()),
        run_replay_tool(registry.clone()),
        asset_status_tool(registry.clone()),
        fixup_tool(registry.clone()),
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
///
/// # Why the documented spelling is project-relative
///
/// The engine's own key is `format!("{project_dir}/{path}")`, and
/// `project_dir` is whatever was handed to `--test` — which for this server is
/// [`SessionRegistry`]'s games root joined with the game name, i.e. an
/// *absolute* path derived from `server.rs`'s root argument. The live key for
/// mini-arena's fox is therefore
/// `f:\Works\BSEngine\games\mini-arena/assets/models/fox.glb`, mixed
/// separators and all. Documenting that would be documenting this machine's
/// checkout; documenting `games/mini-arena/assets/models/fox.glb`, as an
/// earlier revision of this description did, documents a key that does not
/// exist and answers `"unknown"` — the one answer that means "nothing ever
/// requested it". `bsengine_runtime::test_query::get_asset_status` resolves
/// project-relative paths against the session's own `ProjectDir` so the
/// spelling a caller can actually know is the spelling that works.
fn asset_status_tool(registry: Arc<SessionRegistry>) -> McpTool {
    McpTool {
        name: "test_get_asset_status".to_string(),
        description: "Reports what the engine knows about one asset path in a live test \
            session, as \"loaded\", \"loading\", \"failed: <reason>\" or \"unknown\" — the \
            same four answers Bsengine.getAssetStatus gives a script. \"unknown\" means \
            nothing ever requested that path, which is deliberately NOT the same answer as \
            a failure: a misspelled path reads \"unknown\", a real path that broke reads \
            \"failed: \" plus the reason. Spell `path` project-relative, exactly as a \
            scene or script spells it — e.g. \"assets/sounds/hit.wav\", the same string \
            you would pass to Bsengine.playSound. (The engine's own fully-qualified key, \
            this session's project directory followed by that, also works.) A \
            --test session now builds a renderer and a glTF importer too, so meshes, \
            shaders and textures are requested and tracked exactly as sounds and scripts \
            already were. When the \
            answer is \"unknown\", the result also carries `known_paths`: every path this \
            session does know about, so a spelling mistake is visible rather than \
            guessable. Also available as the get_asset_status query tool, so \
            test_assert/test_wait_until can gate a recording on an asset actually loading."
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "session_id": { "type": "string" },
                "path": {
                    "type": "string",
                    "description": "Project-relative asset path as a scene or script \
                        spells it, e.g. assets/sounds/hit.wav",
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
            let out = match mcp_output_from_response(response) {
                out if out.is_ok() => out,
                err => return err,
            };

            // Only on `unknown`, and only here rather than inside the query:
            // `get_asset_status` has to keep answering with a bare string, or
            // every `test_assert`/`test_wait_until` that compares it against
            // "loaded" — including the ones already saved in recordings —
            // would be comparing an object. A second round trip costs one
            // more line of protocol on the one answer that is ambiguous, and
            // nothing at all on the three that are not.
            if out.content == json!(ASSET_STATUS_UNKNOWN) {
                let known = registry.send(
                    &session_id,
                    json!({ "cmd": "query", "tool": "get_known_asset_paths", "args": {} }),
                );
                if let Some(paths) = known.ok().and_then(|r| {
                    let listing = mcp_output_from_response(r);
                    listing.is_ok().then_some(listing.content)
                }) {
                    return McpToolOutput::success(json!({
                        "path": path,
                        "status": out.content,
                        "known_paths": paths,
                    }));
                }
            }
            McpToolOutput::success(json!({ "path": path, "status": out.content }))
        }),
    }
}

/// Settles a project's stale asset references and forgets the former paths
/// nothing needs any more.
///
/// # Why it is `game_`-prefixed and lives in this file anyway
///
/// The prefix in this server says what a tool needs: `test_*` tools drive a
/// live `bsengine-runtime --test` child, `game_*` tools operate on a project
/// directory. `fixup` is squarely the second — it repairs the files a session
/// would load, needs no session, no engine and no running game, and is if
/// anything better run with nothing holding those files open. So it is named
/// `game_fixup`, beside `game_create` and `game_validate`.
///
/// It is built *here* rather than in `game_tools.rs` for one mechanical reason:
/// it reaches the work through a `bsengine-runtime` child process, and
/// [`SessionRegistry`] is what knows where that binary is. See
/// [`SessionRegistry::fixup`] for why the child process is the right boundary
/// rather than a direct call.
///
/// # Why an agent is a primary user
///
/// Everything `fixup` settles is something an agent caused or will trip over: a
/// rename made through an editor, a scene it wrote by hand, a script it is about
/// to read. And the report is the only place the JavaScript half of the problem
/// is *ever* stated — nothing rewrites those references, so an agent that never
/// asks will never learn they are stale until a load quietly returns nothing.
fn fixup_tool(registry: Arc<SessionRegistry>) -> McpTool {
    McpTool {
        name: "game_fixup".to_string(),
        description: "Settles a game's stale asset references: rewrites every scene reference \
            that only still resolves because the engine remembers where an asset used to be, \
            keeping its GUID, and then forgets the former paths nothing needs any more. \
            NEVER edits JavaScript — a path in a script can be built or concatenated, so stale \
            paths there are REPORTED instead, with file, line, the stale path and where the \
            asset went, for you to fix by hand. Needs no test session and no running game: it \
            works on the project directory alone, and is safe to run twice (a second run finds \
            nothing and changes nothing). Returns `rewritten` (scene edits applied), `scripts` \
            (stale paths in .js you must fix yourself), `pruned` (former paths forgotten), \
            `retained` (former paths kept, and what is still holding each one) and `problems` \
            (anything it could not do — a scene it could not write or parse). Run it after \
            renaming or moving an asset; the engine warns on every recovery it performs, and \
            this is what spends that warning."
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
            match registry.fixup(game) {
                // Reported as a success even when `clean` is false, because the
                // report is the answer either way: a run that rewrote nine
                // references and could not write the tenth file has nine
                // results the caller needs, and collapsing that into an error
                // string would throw them away.
                Ok(run) => {
                    let mut content = run.report;
                    if let Some(object) = content.as_object_mut() {
                        object.insert("clean".to_string(), json!(run.clean));
                    }
                    McpToolOutput::success(content)
                }
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
                get_entity_names, get_hud_text, get_asset_status, get_known_asset_paths; \
                `args` are that query's parameters (e.g. {\"name\": \"Player\"}, \
                {\"id\": \"1\"}, {\"path\": \"assets/sounds/hit.wav\"}, {}).",
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
