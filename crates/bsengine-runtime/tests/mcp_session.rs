use std::path::PathBuf;
use std::sync::OnceLock;

use bsengine_mcp::SessionRegistry;

/// The `bsengine-runtime` this test binary was built with.
///
/// Cargo builds a package's own binaries for its integration tests in the same
/// invocation and with the same features, so this is always the runtime under
/// test and never costs a build. These tests lived in `bsengine-mcp` until
/// 2026-10-24, where the only way to the binary was a nested
/// `cargo build -p bsengine-runtime`: right after a workspace `cargo test`,
/// that build resolved features differently and recompiled 23 crates (167 s
/// locally, about 200 s on Windows CI) -- a wait every test in the binary sat
/// behind, `send_to_unknown_session_errors` included, which starts nothing.
fn runtime_bin_path() -> &'static PathBuf {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| PathBuf::from(env!("CARGO_BIN_EXE_bsengine-runtime")))
}

fn test_registry() -> SessionRegistry {
    let games_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../games");
    SessionRegistry::new(runtime_bin_path().clone(), games_root)
}

#[test]
fn start_session_returns_unique_ids() {
    let registry = test_registry();
    let id1 = registry.start_session("cube-evader").unwrap();
    let id2 = registry.start_session("cube-evader").unwrap();
    assert_ne!(id1, id2);
    registry.stop_session(&id1).unwrap();
    registry.stop_session(&id2).unwrap();
}

#[test]
fn send_step_returns_frame_count() {
    let registry = test_registry();
    let id = registry.start_session("cube-evader").unwrap();
    let resp = registry
        .send(&id, serde_json::json!({"cmd": "step", "frames": 5}))
        .unwrap();
    assert_eq!(resp["ok"], true);
    assert_eq!(resp["data"]["frame"], 5);
    registry.stop_session(&id).unwrap();
}

#[test]
fn send_to_unknown_session_errors() {
    let registry = test_registry();
    let result = registry.send("no-such-session", serde_json::json!({"cmd": "shutdown"}));
    assert!(result.is_err());
}

#[test]
fn stop_unknown_session_errors() {
    let registry = test_registry();
    assert!(registry.stop_session("no-such-session").is_err());
}

#[test]
fn save_recording_records_non_query_commands_only() {
    let registry = test_registry();
    let id = registry.start_session("cube-evader").unwrap();

    registry
        .send(&id, serde_json::json!({"cmd": "press_key", "key": "W"}))
        .unwrap();
    registry
        .send(&id, serde_json::json!({"cmd": "step", "frames": 5}))
        .unwrap();
    registry
        .send(
            &id,
            serde_json::json!({"cmd": "query", "tool": "get_entity_names", "args": {}}),
        )
        .unwrap();

    let path = registry.save_recording(&id, "example").unwrap();
    let content = std::fs::read_to_string(&path).unwrap();
    let log: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(log["game"], "cube-evader");
    let actions = log["actions"].as_array().unwrap();
    assert_eq!(
        actions.len(),
        2,
        "query should not be recorded: {actions:?}"
    );
    assert_eq!(actions[0]["cmd"], "press_key");
    assert_eq!(actions[1]["cmd"], "step");

    registry.stop_session(&id).unwrap();
    std::fs::remove_file(&path).ok();
}
