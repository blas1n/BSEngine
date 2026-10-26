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

// A wait_until timeout reports itself as ok:true with passed:false (so it
// renders like a failed assertion rather than a protocol error), which means
// run_replay_mode has to inspect `passed` for wait_until exactly as it does
// for assert. Without that, this recording — waiting for an x the player can
// never reach — would replay as a silent success.
#[test]
fn replay_fails_when_wait_until_times_out() {
    let log_path = write_log(
        r#"[
            {"cmd":"step","frames":5},
            {"cmd":"wait_until","query":{"tool":"get_transform","args":{"name":"Player"}},"path":"x","op":">","value":1000000000.0,"max_frames":5,"label":"player reaches an impossible x"}
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
    assert!(
        !status.success(),
        "expected a timed-out wait_until to fail the replay: {status:?}"
    );
}

/// Replays `actions_json` against cube-evader with `extra` after the log path,
/// returning whether it passed and what it printed to stderr.
fn replay(actions_json: &str, extra: &[&str]) -> (bool, String) {
    let log_path = write_log(actions_json);
    let output = Command::new(env!("CARGO_BIN_EXE_bsengine-runtime"))
        .arg("--test")
        .arg(game_fixture_path())
        .arg("--replay")
        .arg(&log_path)
        .args(extra)
        .output()
        .expect("failed to run replay");
    std::fs::remove_file(&log_path).ok();
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// A recording that reads one pixel, which `get_pixel` refuses outside the
/// frame -- so whether it passes says how big the frame the replay rendered is.
fn reads_pixel(x: u32, y: u32) -> String {
    format!(
        r#"[
            {{"cmd":"step","frames":2}},
            {{"cmd":"assert","query":{{"tool":"get_pixel","args":{{"x":{x},"y":{y}}}}},"path":"luma","op":">=","value":0,"label":"pixel ({x}, {y}) is in the frame"}}
        ]"#
    )
}

// CI replays with `--frame-divisor 8` because on WARP a frame's cost is its
// pixels (see `test_mode::SIMULATION_FRAME_DIVISOR`). The flag has to reach
// the renderer, and leaving it off has to leave the game's own 1280x720 --
// both are read here through the frame `get_pixel` sees, in the binary CI
// runs, not through the function the flag is parsed into.
#[test]
fn a_frame_divisor_renders_the_replay_at_that_fraction_of_the_window() {
    // The premise: without the flag, the frame is the manifest's, so the
    // centre of a 1280x720 frame is in it.
    let (passed, log) = replay(&reads_pixel(640, 360), &[]);
    assert!(
        passed,
        "without --frame-divisor the frame is 1280x720:\n{log}"
    );

    // Divided by 8 it is 160x90: the old centre is outside it ...
    let (passed, log) = replay(&reads_pixel(640, 360), &["--frame-divisor", "8"]);
    assert!(!passed, "a 160x90 frame has no pixel (640, 360)");
    assert!(
        log.contains("outside the 160x90 frame"),
        "the failure must be the frame's size, not something else:\n{log}"
    );

    // ... and its last pixel is still in it, so the frame is not smaller than
    // asked either.
    let (passed, log) = replay(&reads_pixel(159, 89), &["--frame-divisor", "8"]);
    assert!(passed, "a 160x90 frame has a pixel (159, 89):\n{log}");
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
