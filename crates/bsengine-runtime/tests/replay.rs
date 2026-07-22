use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

fn game_fixture_path() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{manifest_dir}/../../games/cube-evader")
}

// Tests in this file run as threads within the same test-binary process, so
// `std::process::id()` alone is identical across them; add a per-call
// counter to keep temp log paths from colliding under parallel test runs.
static LOG_COUNTER: AtomicU64 = AtomicU64::new(0);

fn write_log(actions_json: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir();
    let unique = LOG_COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = dir.join(format!(
        "bsengine-replay-test-{}-{unique}.json",
        std::process::id()
    ));
    std::fs::write(
        &path,
        format!(r#"{{"game":"cube-evader","actions":{actions_json}}}"#),
    )
    .unwrap();
    path
}

#[test]
fn replay_passes_when_all_assertions_hold() {
    let log_path = write_log(
        r#"[
            {"cmd":"press_key","key":"W"},
            {"cmd":"step","frames":20},
            {"cmd":"assert","query":{"tool":"get_transform","args":{"name":"Player"}},"path":"z","op":"<","value":-1.5,"label":"player moved forward"}
        ]"#,
    );

    let status = Command::new(env!("CARGO_BIN_EXE_bsengine-runtime"))
        .arg("--test")
        .arg(game_fixture_path())
        .arg("--replay")
        .arg(&log_path)
        .status()
        .expect("failed to run replay");

    std::fs::remove_file(&log_path).ok();
    assert!(status.success(), "expected replay to pass: {status:?}");
}

#[test]
fn replay_fails_when_assertion_does_not_hold() {
    let log_path = write_log(
        r#"[
            {"cmd":"step","frames":5},
            {"cmd":"assert","query":{"tool":"get_transform","args":{"name":"Player"}},"path":"z","op":"<","value":-100,"label":"impossible without holding W"}
        ]"#,
    );

    let status = Command::new(env!("CARGO_BIN_EXE_bsengine-runtime"))
        .arg("--test")
        .arg(game_fixture_path())
        .arg("--replay")
        .arg(&log_path)
        .status()
        .expect("failed to run replay");

    std::fs::remove_file(&log_path).ok();
    assert!(!status.success(), "expected replay to fail: {status:?}");
}
