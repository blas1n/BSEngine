use std::path::PathBuf;
use std::sync::OnceLock;

use bsengine_mcp::SessionRegistry;

/// Builds `bsengine-runtime` (if not already up to date) and returns the
/// path to its executable, by parsing `cargo build --message-format=json`.
/// `CARGO_BIN_EXE_<name>` only works for a package's own binaries, not a
/// different package's — so this shells out instead of relying on it.
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
                && msg.get("target").and_then(|t| t.get("name")).and_then(|v| v.as_str())
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
