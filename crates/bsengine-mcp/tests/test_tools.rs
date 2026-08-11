use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use bsengine_mcp::{test_tools, McpTool, SessionRegistry};
use serde_json::json;

/// Builds `bsengine-runtime` (if not already up to date) and returns the
/// path to its executable, by parsing `cargo build --message-format=json`.
/// Same approach as `tests/session.rs` — `CARGO_BIN_EXE_<name>` only works
/// for a package's own binaries, not a different package's.
fn runtime_bin_path() -> &'static PathBuf {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let output = std::process::Command::new(env!("CARGO"))
            .args([
                "build",
                "-p",
                "bsengine-runtime",
                "--bin",
                "bsengine-runtime",
                "--message-format=json",
            ])
            .output()
            .expect("failed to run cargo build for bsengine-runtime");
        assert!(
            output.status.success(),
            "cargo build -p bsengine-runtime failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let msg: serde_json::Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if msg.get("reason").and_then(|v| v.as_str()) == Some("compiler-artifact")
                && msg
                    .get("target")
                    .and_then(|t| t.get("name"))
                    .and_then(|v| v.as_str())
                    == Some("bsengine-runtime")
            {
                if let Some(exe) = msg.get("executable").and_then(|v| v.as_str()) {
                    return PathBuf::from(exe);
                }
            }
        }
        panic!("could not locate bsengine-runtime executable in cargo build output");
    })
}

fn test_registry() -> Arc<SessionRegistry> {
    let games_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../games");
    Arc::new(SessionRegistry::new(runtime_bin_path().clone(), games_root))
}

fn find<'a>(tools: &'a [McpTool], name: &str) -> &'a McpTool {
    tools
        .iter()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("tool {name} not found"))
}

#[test]
fn builds_fourteen_tools() {
    let tools = test_tools(test_registry());
    assert_eq!(tools.len(), 14);
}

#[test]
fn full_session_round_trip() {
    let tools = test_tools(test_registry());

    let start = find(&tools, "test_session_start");
    let out = (start.handler)(json!({"game": "cube-evader"}));
    assert!(out.is_ok(), "{:?}", out.error);
    let session_id = out.content["session_id"].as_str().unwrap().to_string();

    let press = find(&tools, "test_press_key");
    let out = (press.handler)(json!({"session_id": session_id, "key": "W"}));
    assert!(out.is_ok(), "{:?}", out.error);

    let step = find(&tools, "test_step");
    let out = (step.handler)(json!({"session_id": session_id, "frames": 20}));
    assert!(out.is_ok(), "{:?}", out.error);
    assert_eq!(out.content["frame"], 20);

    let assert_tool = find(&tools, "test_assert");
    let out = (assert_tool.handler)(json!({
        "session_id": session_id,
        "query": {"tool": "get_transform", "args": {"name": "Player"}},
        "path": "z",
        "op": "<",
        "value": -1.5,
        "label": "player moved forward",
    }));
    assert!(out.is_ok(), "{:?}", out.error);
    assert_eq!(out.content["passed"], true, "{:?}", out.content);

    let stop = find(&tools, "test_session_stop");
    let out = (stop.handler)(json!({"session_id": session_id}));
    assert!(out.is_ok(), "{:?}", out.error);
}

#[test]
fn get_pixel_and_screenshot_work_through_test_query_state() {
    let tools = test_tools(test_registry());

    let start = find(&tools, "test_session_start");
    let out = (start.handler)(json!({"game": "cube-evader"}));
    assert!(out.is_ok(), "{:?}", out.error);
    let session_id = out.content["session_id"].as_str().unwrap().to_string();

    let step = find(&tools, "test_step");
    let out = (step.handler)(json!({"session_id": session_id, "frames": 1}));
    assert!(out.is_ok(), "{:?}", out.error);

    let query = find(&tools, "test_query_state");
    let out = (query.handler)(json!({
        "session_id": session_id,
        "tool": "get_pixel",
        "args": {"x": 0, "y": 0},
    }));
    assert!(out.is_ok(), "{:?}", out.error);
    assert!(out.content["luma"].is_number(), "{:?}", out.content);

    let out = (query.handler)(json!({
        "session_id": session_id,
        "tool": "screenshot",
        "args": {},
    }));
    assert!(out.is_ok(), "{:?}", out.error);
    assert_eq!(out.content["format"], "png");
    assert!(out.content["data_base64"].is_string());
}

/// The property `get_pixel_and_screenshot_work_through_test_query_state` does
/// not cover: that a real `screenshot` payload from a real running game,
/// wrapped by a real `McpServer::handle_message`'s `tools/call` path, comes
/// out as an MCP image content block. That test calls `test_query_state`'s
/// handler directly, bypassing `handle_message`/`content_block` entirely, so
/// nothing proved the two halves — real PNG bytes, and the JSON-RPC layer
/// that recognizes them — actually compose. `server.rs`'s own
/// `tools_call_wraps_a_png_payload_as_an_image_content_block` proves
/// `content_block`'s detection logic against a synthetic fixture; this proves
/// the same wrapping against the genuine `screenshot` tool's output.
#[test]
fn screenshot_reaches_an_mcp_client_as_an_image_content_block() {
    use std::sync::Mutex;

    use bsengine_mcp::{McpServer, McpToolRegistry};

    let tools = test_tools(test_registry());

    // Session lifecycle via direct handler calls, same pattern as every
    // other test in this file — the part under test is the query call, not
    // session setup.
    let start = find(&tools, "test_session_start");
    let out = (start.handler)(json!({"game": "cube-evader"}));
    assert!(out.is_ok(), "{:?}", out.error);
    let session_id = out.content["session_id"].as_str().unwrap().to_string();

    let step = find(&tools, "test_step");
    let out = (step.handler)(json!({"session_id": session_id, "frames": 1}));
    assert!(out.is_ok(), "{:?}", out.error);

    // Now hand the same tools (still bound to the session that's live above)
    // to a real McpServer, and drive the screenshot query the way an actual
    // MCP client would: a JSON-RPC tools/call request through handle_message.
    let mut registry = McpToolRegistry::new();
    for tool in tools {
        registry.register(tool);
    }
    let server = McpServer::new(Arc::new(Mutex::new(registry)));

    let resp = server
        .handle_message(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "test_query_state",
                "arguments": {
                    "session_id": session_id,
                    "tool": "screenshot",
                    "args": {},
                },
            },
        }))
        .expect("tools/call must produce a response");

    assert!(
        !resp["result"]["isError"].as_bool().unwrap_or(false),
        "tools/call reported an error: {resp:?}"
    );
    let content = &resp["result"]["content"][0];
    assert_eq!(content["type"], "image", "{resp:?}");
    assert_eq!(content["mimeType"], "image/png", "{resp:?}");
    assert!(
        content["data"].as_str().is_some_and(|d| !d.is_empty()),
        "the image block must carry real PNG bytes, not an empty placeholder: {resp:?}"
    );
}

// Being present in `passthrough_specs()` is not the same as reaching an MCP
// client: the tool only works if the assembled list carries it and its
// `child_cmd` is a command the runtime actually parses. Drive it against a
// live session rather than asserting on the source, so a rename on either
// side of the protocol boundary fails here. The predicate is the one
// `full_session_round_trip` shows W reaches within 20 frames, so the
// generous frame budget is slack, not a race.
#[test]
fn wait_until_reaches_a_live_session() {
    let tools = test_tools(test_registry());

    let out = (find(&tools, "test_session_start").handler)(json!({"game": "cube-evader"}));
    assert!(out.is_ok(), "{:?}", out.error);
    let session_id = out.content["session_id"].as_str().unwrap().to_string();

    (find(&tools, "test_press_key").handler)(json!({"session_id": session_id, "key": "W"}));

    let out = (find(&tools, "test_wait_until").handler)(json!({
        "session_id": session_id,
        "query": {"tool": "get_transform", "args": {"name": "Player"}},
        "path": "z",
        "op": "<",
        "value": -1.5,
        "max_frames": 600,
        "label": "player moved forward",
    }));
    assert!(out.is_ok(), "{:?}", out.error);
    assert_eq!(out.content["passed"], true, "{:?}", out.content);

    (find(&tools, "test_session_stop").handler)(json!({"session_id": session_id}));
}

#[test]
fn record_save_and_replay_round_trip() {
    let tools = test_tools(test_registry());

    let start = find(&tools, "test_session_start");
    let out = (start.handler)(json!({"game": "cube-evader"}));
    assert!(out.is_ok(), "{:?}", out.error);
    let session_id = out.content["session_id"].as_str().unwrap().to_string();

    (find(&tools, "test_press_key").handler)(json!({"session_id": session_id, "key": "W"}));
    (find(&tools, "test_step").handler)(json!({"session_id": session_id, "frames": 20}));
    (find(&tools, "test_assert").handler)(json!({
        "session_id": session_id,
        "query": {"tool": "get_transform", "args": {"name": "Player"}},
        "path": "z", "op": "<", "value": -1.5,
        "label": "player moved forward",
    }));

    let save = find(&tools, "test_save_recording");
    let out = (save.handler)(json!({"session_id": session_id, "name": "round-trip-test"}));
    assert!(out.is_ok(), "{:?}", out.error);

    (find(&tools, "test_session_stop").handler)(json!({"session_id": session_id}));

    let replay = find(&tools, "test_run_replay");
    let out = (replay.handler)(json!({"game": "cube-evader", "name": "round-trip-test"}));
    assert!(out.is_ok(), "{:?}", out.error);
    assert_eq!(
        out.content["passed"], true,
        "replay stderr: {}",
        out.content["stderr"]
    );

    let saved_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../games/cube-evader/tests/round-trip-test.testlog.json");
    std::fs::remove_file(saved_path).ok();
}

/// Name of the throwaway game inside a [`ProbeGame`]'s games-root.
const PROBE_GAME: &str = "probe";

/// Requested by the probe game's script, and impossible to load.
const BROKEN_SOUND: &str = "assets/sounds/does-not-exist.ogg";

/// Never requested by anything. Same shape as [`BROKEN_SOUND`] and equally
/// nonexistent — the only difference is that nothing asked for it, which is
/// the difference the whole status API exists to expose.
const QUIET_SOUND: &str = "assets/sounds/nothing-ever-asked.ogg";

/// A throwaway game project whose script requests one asset that cannot load,
/// plus the `SessionRegistry` rooted at it. Removed on drop.
///
/// # Why it lives under the crate directory rather than in the temp dir
///
/// `bevy_asset`'s root is the process CWD (see `bsengine_asset::plugin`), and
/// every asset path this engine produces is *relative* to it — the engine has
/// never loaded an absolute asset path anywhere. Cargo runs an integration
/// test with the CWD set to the package root, and the spawned
/// `bsengine-runtime --test` child inherits it, so a games-root spelled
/// relative to that gives the child the exact path shape a real game has.
/// `crates/*/bsengine-status-probe-*` is already gitignored for the sibling
/// probes in `bsengine-asset`, which exist for the same reason.
struct ProbeGame {
    games_root: PathBuf,
}

/// A games-root directory name no other probe in this process will pick.
fn unique_root_name() -> String {
    static N: AtomicU32 = AtomicU32::new(0);
    format!(
        "bsengine-status-probe-mcp-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    )
}

impl ProbeGame {
    fn create() -> Self {
        Self::create_at(PathBuf::from(unique_root_name()))
    }

    /// The same probe, rooted the way the **deployed** server roots one.
    ///
    /// `.mcp.json` hands `bsengine-mcp-server` an absolute root,
    /// `src/bin/server.rs` joins `games` onto it, and
    /// `SessionRegistry::start_session` joins the game name and passes the
    /// result to `--test` verbatim — so the running engine's `ProjectDir`, and
    /// therefore every key in `AssetStatuses`, is absolute. [`Self::create`]'s
    /// relative root produces short, plausible-looking keys instead, which is
    /// exactly how a wrong documented spelling stayed green here.
    ///
    /// Still physically under the CWD, for the reason the type docs give: it is
    /// only the *spelling* handed to the child that this varies.
    fn create_absolute() -> Self {
        let cwd = std::env::current_dir().expect("cannot determine current directory");
        Self::create_at(cwd.join(unique_root_name()))
    }

    fn create_at(games_root: PathBuf) -> Self {
        let game = games_root.join(PROBE_GAME);
        std::fs::create_dir_all(game.join("assets/scenes")).unwrap();
        std::fs::create_dir_all(game.join("assets/scripts")).unwrap();
        std::fs::write(
            game.join("project.toml"),
            "[project]\nname = \"Asset Status Probe\"\nentry_scene = \"assets/scenes/main.ron\"\n",
        )
        .unwrap();
        std::fs::write(
            game.join("assets/scenes/main.ron"),
            "SceneDescriptor(entities: [\n    \
             EntityDescriptor(name: \"Probe\", script: Some(\"assets/scripts/probe.js\")),\n])\n",
        )
        .unwrap();
        // Every frame rather than once: `playSound` deduplicates by path
        // (`SoundLoads`), so this requests the asset exactly once no matter how
        // many frames the session is stepped, and needs no first-frame hook.
        std::fs::write(
            game.join("assets/scripts/probe.js"),
            format!("function onUpdate(self) {{ Bsengine.playSound(\"{BROKEN_SOUND}\"); }}\n"),
        )
        .unwrap();
        Self { games_root }
    }

    /// The key `AssetStatuses` holds for `relative`, spelled exactly as the
    /// engine spells it: `resolve_project_path` joins the project directory
    /// with the script-relative path, and the project directory is what
    /// `SessionRegistry` passes to `--test` — `games_root/<game>`.
    ///
    /// Derived rather than hardcoded on purpose: the spelling is the one thing
    /// a caller of this tool has to get right, so a test that quietly used a
    /// different one would be testing nothing.
    fn asset_key(&self, relative: &str) -> String {
        format!("{}/{relative}", self.games_root.join(PROBE_GAME).display())
    }

    fn registry(&self) -> Arc<SessionRegistry> {
        Arc::new(SessionRegistry::new(
            runtime_bin_path().clone(),
            self.games_root.clone(),
        ))
    }
}

impl Drop for ProbeGame {
    fn drop(&mut self) {
        // Retried: the child process may still be exiting, and Windows refuses
        // to remove a directory anything still has open.
        for _ in 0..40 {
            if std::fs::remove_dir_all(&self.games_root).is_ok() || !self.games_root.exists() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

/// The property this tool exists for, driven end to end against a live
/// session: a path whose load *failed* and a path *nothing ever requested*
/// must not read the same.
///
/// Both halves are load-bearing. A test that only checked the failure would
/// pass against a tool that answered `"failed: ..."` for every path — and
/// that is exactly the shape of the bug this phase exists to prevent, since a
/// status nobody can trust is no better than the `tracing::warn!` it replaces.
///
/// Driven through a real session rather than asserted on the source, for the
/// same reason `wait_until_reaches_a_live_session` is: the tool only works if
/// the assembled tool list carries it *and* the query name it sends is one the
/// runtime actually parses, and neither is visible from this crate alone.
#[test]
fn asset_status_tells_a_failed_load_from_a_path_nothing_requested() {
    let probe = ProbeGame::create();
    let tools = test_tools(probe.registry());

    let out = (find(&tools, "test_session_start").handler)(json!({"game": PROBE_GAME}));
    assert!(out.is_ok(), "{:?}", out.error);
    let session_id = out.content["session_id"].as_str().unwrap().to_string();

    let result_of = |path: &str| -> serde_json::Value {
        let out = (find(&tools, "test_get_asset_status").handler)(
            json!({"session_id": session_id, "path": path}),
        );
        assert!(out.is_ok(), "{:?}", out.error);
        assert_eq!(
            out.content["path"],
            json!(path),
            "the tool must echo the path it was asked about, or an `unknown` is unreadable"
        );
        out.content
    };
    let status_of = |path: &str| -> String {
        let content = result_of(path);
        content["status"]
            .as_str()
            .unwrap_or_else(|| panic!("status must be a string, got {content:?}"))
            .to_string()
    };

    let broken = probe.asset_key(BROKEN_SOUND);
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut broken_status = status_of(&broken);
    while !broken_status.starts_with("failed: ") {
        assert!(
            Instant::now() < deadline,
            "the probe's load never resolved, so this test proves nothing; last status \
             was {broken_status:?} for {broken}"
        );
        let out =
            (find(&tools, "test_step").handler)(json!({"session_id": session_id, "frames": 10}));
        assert!(out.is_ok(), "{:?}", out.error);
        broken_status = status_of(&broken);
    }

    let reason = broken_status.trim_start_matches("failed: ");
    assert!(
        !reason.trim().is_empty(),
        "a failure with an empty reason is no better than the warn! it replaces"
    );
    let lowered = reason.to_lowercase();
    assert!(
        lowered.contains("not found") || lowered.contains("does-not-exist"),
        "the reason must name what went wrong or what it went wrong on, got {reason:?}"
    );

    let quiet = result_of(&probe.asset_key(QUIET_SOUND));
    let quiet_status = quiet["status"].as_str().unwrap();
    assert_eq!(
        quiet_status, "unknown",
        "a path nothing ever requested must read `unknown`, not a failure and not a load"
    );
    assert_ne!(
        quiet_status, broken_status,
        "if silence and failure read the same, the tool answers nothing"
    );

    // The half that turns an unguessable answer into an obvious one. `unknown`
    // is the only answer a caller cannot tell from a typo, so it ships the
    // spellings the engine does hold — and the broken sound, which is known,
    // has to be among them or the listing is decoration.
    let known = quiet["known_paths"]
        .as_array()
        .unwrap_or_else(|| panic!("an `unknown` must carry known_paths, got {quiet:?}"));
    assert!(
        known.contains(&json!(broken)),
        "known_paths must list the paths this session actually requested, or it \
         cannot correct a spelling; got {known:?}"
    );
    assert!(
        !known.contains(&json!(probe.asset_key(QUIET_SOUND))),
        "known_paths is what the engine knows, not an echo of the question: {known:?}"
    );

    (find(&tools, "test_session_stop").handler)(json!({"session_id": session_id}));
}

/// The Critical regression: the spelling this tool *documents* must be the
/// spelling that works, in the configuration that actually ships.
///
/// Every other probe here uses a **relative** games-root, where the engine's
/// key comes out short and plausible. The deployed server's is absolute
/// (`.mcp.json` → `server.rs` → `SessionRegistry::start_session` → `--test`,
/// each step joining verbatim), so the live key is an absolute path with mixed
/// separators that no caller can reconstruct. That configuration gap — not the
/// key derivation, which was always right — is what let the tool tell agents to
/// pass `games/mini-arena/assets/models/fox.glb` and answer `"unknown"`: the
/// one answer meaning "nothing ever requested that path".
///
/// So this asks the way the description now tells an agent to ask —
/// project-relative — under an absolute root, and separately confirms the
/// fully-qualified key still answers, because resolution must add a spelling
/// rather than replace one.
#[test]
fn asset_status_resolves_a_project_relative_path_under_an_absolute_games_root() {
    let probe = ProbeGame::create_absolute();
    assert!(
        probe.games_root.is_absolute(),
        "precondition: this test is only meaningful against an absolute games-root, \
         which is the one the deployed server has"
    );
    let tools = test_tools(probe.registry());

    let out = (find(&tools, "test_session_start").handler)(json!({"game": PROBE_GAME}));
    assert!(out.is_ok(), "{:?}", out.error);
    let session_id = out.content["session_id"].as_str().unwrap().to_string();

    let status_of = |path: &str| -> String {
        let out = (find(&tools, "test_get_asset_status").handler)(
            json!({"session_id": session_id, "path": path}),
        );
        assert!(out.is_ok(), "{:?}", out.error);
        out.content["status"]
            .as_str()
            .unwrap_or_else(|| panic!("status must be a string, got {:?}", out.content))
            .to_string()
    };

    // BROKEN_SOUND verbatim: exactly the string the probe's script passes to
    // `playSound`, and exactly the shape the tool's description now documents.
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut relative_status = status_of(BROKEN_SOUND);
    while !relative_status.starts_with("failed: ") {
        assert!(
            Instant::now() < deadline,
            "the project-relative spelling never reported the failed load; last status \
             was {relative_status:?} for {BROKEN_SOUND} under {}",
            probe.games_root.display()
        );
        let out =
            (find(&tools, "test_step").handler)(json!({"session_id": session_id, "frames": 10}));
        assert!(out.is_ok(), "{:?}", out.error);
        relative_status = status_of(BROKEN_SOUND);
    }

    assert_eq!(
        status_of(&probe.asset_key(BROKEN_SOUND)),
        relative_status,
        "the engine's own fully-qualified key must keep answering — resolution adds \
         a spelling, it does not swap one for another"
    );

    assert_eq!(
        status_of(QUIET_SOUND),
        "unknown",
        "resolving a project-relative path must not make every path answerable: a \
         path nothing requested still reads `unknown`"
    );

    (find(&tools, "test_session_stop").handler)(json!({"session_id": session_id}));
}

/// A tool that is built but never enumerated is inert — the exact failure
/// this branch already hit once, when `AssetStatusPlugin` was implemented and
/// added by nothing outside its own tests. So this walks the whole path
/// `src/bin/server.rs` walks: register every tool into an `McpToolRegistry`,
/// serve it, and ask over JSON-RPC the way a client does.
#[test]
fn asset_status_tool_is_enumerated_by_the_server() {
    use std::sync::Mutex;

    use bsengine_mcp::{game_tools, McpServer, McpToolRegistry};

    let mut registry = McpToolRegistry::new();
    for tool in game_tools(PathBuf::from(".")) {
        registry.register(tool);
    }
    for tool in test_tools(test_registry()) {
        registry.register(tool);
    }

    let server = McpServer::new(Arc::new(Mutex::new(registry)));
    let response = server
        .handle_message(json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}))
        .expect("tools/list must produce a response");
    let listed = response["result"]["tools"].as_array().unwrap();

    let tool = listed
        .iter()
        .find(|t| t["name"] == "test_get_asset_status")
        .unwrap_or_else(|| {
            let names: Vec<&str> = listed.iter().filter_map(|t| t["name"].as_str()).collect();
            panic!("test_get_asset_status is not enumerated; tools/list has {names:?}")
        });
    let required = tool["inputSchema"]["required"].as_array().unwrap();
    assert!(
        required.contains(&json!("session_id")) && required.contains(&json!("path")),
        "both arguments must be advertised, or a client cannot call this: {required:?}"
    );
}

// ---- game_fixup ---------------------------------------------------------

/// A game in which one asset a *scene* names and one a *script* names have both
/// already moved. Sidecars written as literal RON, because this crate does not
/// depend on the engine and writing them by hand is also a check that the
/// format a real project holds on disk is the one `fixup` reads.
struct FixupProbe {
    games_root: PathBuf,
}

impl FixupProbe {
    const GAME: &'static str = "fixup-probe";
    const MODEL_NOW: &'static str = "assets/models/fox.glb";
    const MODEL_WAS: &'static str = "assets/models/old_fox.glb";
    const SOUND_NOW: &'static str = "assets/sounds/hit.wav";
    const SOUND_WAS: &'static str = "assets/sounds/thud.wav";
    const MODEL_GUID: &'static str = "0193a7c1-8f2e-7c44-9d61-3b5a0e7f2c19";
    const SOUND_GUID: &'static str = "0193a7c1-8f2e-7c44-9d61-3b5a0e7f2c20";

    fn create() -> Self {
        let games_root = PathBuf::from(unique_root_name());
        let game = games_root.join(Self::GAME);
        let write = |relative: &str, contents: &str| {
            let path = game.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        };

        write(
            "project.toml",
            "[project]\nname = \"Fixup Probe\"\nentry_scene = \"assets/scenes/main.ron\"\n",
        );
        for (asset, guid, was) in [
            (Self::MODEL_NOW, Self::MODEL_GUID, Self::MODEL_WAS),
            (Self::SOUND_NOW, Self::SOUND_GUID, Self::SOUND_WAS),
        ] {
            write(asset, "not really an asset");
            write(
                &format!("{asset}.meta"),
                &format!(
                    "(guid: \"{guid}\", hash: \"blake3:stale\", size: None, \
                     former_paths: [\"{was}\"])\n"
                ),
            );
        }
        write(
            "assets/scenes/main.ron",
            &format!(
                "SceneDescriptor(entities: [\n    EntityDescriptor(name: \"Fox\", \
                 gltf: Some((guid: \"{}\", path: \"{}\"))),\n])\n",
                Self::MODEL_GUID,
                Self::MODEL_WAS
            ),
        );
        write(
            "assets/scripts/probe.js",
            &format!(
                "function onUpdate(self) {{\n  Bsengine.playSound(\"{}\");\n}}\n",
                Self::SOUND_WAS
            ),
        );
        Self { games_root }
    }

    fn file(&self, relative: &str) -> PathBuf {
        self.games_root.join(Self::GAME).join(relative)
    }

    fn registry(&self) -> Arc<SessionRegistry> {
        Arc::new(SessionRegistry::new(
            runtime_bin_path().clone(),
            self.games_root.clone(),
        ))
    }
}

impl Drop for FixupProbe {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.games_root).ok();
    }
}

/// The tool an agent will actually reach for, driven the way an agent drives
/// it: no session started, no game running, one call against a project
/// directory.
///
/// Driven against a real project rather than asserted on the source, for the
/// same reason `wait_until_reaches_a_live_session` is: this tool only works if
/// the assembled list carries it *and* the `--fixup` mode it spawns is one the
/// runtime actually parses, and neither is visible from this crate alone.
#[test]
fn game_fixup_rewrites_a_scene_and_reports_a_script_without_touching_it() {
    let probe = FixupProbe::create();
    let tools = test_tools(probe.registry());
    let script = probe.file("assets/scripts/probe.js");
    let before = std::fs::read(&script).expect("read script");

    let out = (find(&tools, "game_fixup").handler)(json!({"game": FixupProbe::GAME}));
    assert!(out.is_ok(), "{:?}", out.error);

    assert_eq!(out.content["clean"], true, "{:?}", out.content);
    assert_eq!(
        out.content["rewritten"][0]["from"],
        FixupProbe::MODEL_WAS,
        "{:?}",
        out.content
    );
    assert_eq!(
        out.content["rewritten"][0]["to"],
        FixupProbe::MODEL_NOW,
        "{:?}",
        out.content
    );
    assert!(
        std::fs::read_to_string(probe.file("assets/scenes/main.ron"))
            .expect("read scene")
            .contains(FixupProbe::MODEL_NOW),
        "the tool reported a rewrite the file does not have"
    );

    assert_eq!(
        std::fs::read(&script).expect("read script"),
        before,
        "game_fixup rewrote a JavaScript file"
    );
    let reported = &out.content["scripts"][0];
    assert_eq!(
        reported["stale_path"],
        FixupProbe::SOUND_WAS,
        "{reported:?}"
    );
    assert_eq!(reported["now_at"], FixupProbe::SOUND_NOW, "{reported:?}");
    assert_eq!(reported["line"], 2, "{reported:?}");
    assert!(
        reported["file"]
            .as_str()
            .unwrap_or_default()
            .contains("probe.js"),
        "{reported:?}"
    );

    assert_eq!(
        out.content["pruned"][0]["former_path"],
        FixupProbe::MODEL_WAS,
        "{:?}",
        out.content
    );
    assert_eq!(
        out.content["retained"][0]["former_path"],
        FixupProbe::SOUND_WAS,
        "a former path a script still names must be kept: {:?}",
        out.content
    );
}

/// A tool that is built but never enumerated is inert. Same walk
/// `src/bin/server.rs` takes, asked the way a client asks.
#[test]
fn game_fixup_is_enumerated_by_the_server() {
    use std::sync::Mutex;

    use bsengine_mcp::{game_tools, McpServer, McpToolRegistry};

    let mut registry = McpToolRegistry::new();
    for tool in game_tools(PathBuf::from(".")) {
        registry.register(tool);
    }
    for tool in test_tools(test_registry()) {
        registry.register(tool);
    }

    let server = McpServer::new(Arc::new(Mutex::new(registry)));
    let response = server
        .handle_message(json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}))
        .expect("tools/list must produce a response");
    let listed = response["result"]["tools"].as_array().unwrap();

    let tool = listed
        .iter()
        .find(|t| t["name"] == "game_fixup")
        .unwrap_or_else(|| {
            let names: Vec<&str> = listed.iter().filter_map(|t| t["name"].as_str()).collect();
            panic!("game_fixup is not enumerated; tools/list has {names:?}")
        });
    assert_eq!(
        tool["inputSchema"]["required"].as_array().unwrap(),
        &vec![json!("game")],
        "the tool takes a game and no session, and has to advertise exactly that"
    );
    // The one thing a caller must not have to discover by experiment.
    let description = tool["description"].as_str().unwrap_or_default();
    assert!(
        description.contains("JavaScript"),
        "the description has to say that JavaScript is reported and not edited, \
         or an agent will assume the tool finished the job: {description}"
    );
}

#[test]
fn game_fixup_on_a_directory_that_is_not_a_game_errors() {
    let tools = test_tools(test_registry());
    let out = (find(&tools, "game_fixup").handler)(json!({"game": "no-such-game"}));
    assert!(!out.is_ok());
    assert!(out.error.unwrap().contains("project.toml"));
}

#[test]
fn game_fixup_missing_game_field_errors() {
    let tools = test_tools(test_registry());
    let out = (find(&tools, "game_fixup").handler)(json!({}));
    assert!(!out.is_ok());
    assert!(out.error.unwrap().contains("game"));
}

#[test]
fn asset_status_missing_path_errors() {
    let tools = test_tools(test_registry());
    let out = (find(&tools, "test_get_asset_status").handler)(json!({"session_id": "session-1"}));
    assert!(!out.is_ok());
    assert!(out.error.unwrap().contains("path"));
}

#[test]
fn start_missing_game_field_errors() {
    let tools = test_tools(test_registry());
    let start = find(&tools, "test_session_start");
    let out = (start.handler)(json!({}));
    assert!(!out.is_ok());
}

#[test]
fn passthrough_missing_session_id_errors() {
    let tools = test_tools(test_registry());
    let step = find(&tools, "test_step");
    let out = (step.handler)(json!({"frames": 1}));
    assert!(!out.is_ok());
    assert!(out.error.unwrap().contains("session_id"));
}
