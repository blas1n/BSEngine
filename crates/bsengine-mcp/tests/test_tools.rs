use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

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
fn builds_eleven_tools() {
    let tools = test_tools(test_registry());
    assert_eq!(tools.len(), 11);
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
