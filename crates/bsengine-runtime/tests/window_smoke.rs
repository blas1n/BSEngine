//! Does a window actually open, and does anything get drawn in it?
//!
//! ⚠️ The only test in this repo that opens a real window. Everything else runs
//! headless: the test harness builds a `fast_render` surface with no window at
//! all, and CI has only ever exercised that. So "the engine builds and its
//! whole suite passes on Linux" -- true on every PR -- has never meant "a game
//! opens on Linux". This is the step between the two.
//!
//! Runs the shipped executable rather than assembling an app here, for two
//! reasons. winit refuses to build an event loop off the main thread and
//! libtest runs every test on a worker thread, so in-process is not an option;
//! and a subprocess covers `main`'s own argument handling and the binary a
//! player launches, which a hand-assembled `App` would not.

use std::path::PathBuf;
use std::process::Command;

/// Kept well above the real cost -- a cold GPU driver and a shader cache miss
/// can make the first frame slow, and a flaky timeout is worse than a slow
/// test. It is a hang detector, not a performance budget.
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// How many frames to ask for. More than one, so a renderer that draws its
/// first frame and then wedges is still caught; small enough to stay quick.
const FRAMES: u32 = 5;

fn project(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../games")
        .join(name)
}

#[test]
fn a_window_opens_and_draws_frames() {
    let project = project("cube-evader");
    assert!(
        project.join("project.toml").is_file(),
        "fixture project is missing at {}",
        project.display()
    );

    let mut child = Command::new(env!("CARGO_BIN_EXE_bsengine-runtime"))
        .arg(&project)
        .arg("--frames")
        .arg(FRAMES.to_string())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to launch the runtime executable");

    // Wait by polling rather than `wait_with_output`, so a hung window is a
    // test failure with a message instead of a job that runs until CI's own
    // timeout kills it with no explanation.
    let started = std::time::Instant::now();
    let status = loop {
        match child.try_wait().expect("failed to poll the runtime") {
            Some(status) => break status,
            None if started.elapsed() > TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                panic!(
                    "the window never closed itself after {FRAMES} frames -- \
                     killed it at {:?}. Either it never reached {FRAMES} \
                     frames, or `AppExit` is not ending the event loop.",
                    TIMEOUT
                );
            }
            None => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    };

    let output = child
        .wait_with_output()
        .expect("failed to collect the runtime's output");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        status.success(),
        "the runtime exited with {status}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
    );

    // ⚠️ Asserting on draw calls, not just on the frame count. A window that
    // opens and presents nothing still ticks `Update` happily and would reach
    // its frame count either way.
    let reported = stdout
        .lines()
        .find_map(|line| line.strip_prefix("frames="))
        .unwrap_or_else(|| {
            panic!(
                "the runtime never reported a frame count, so it exited before \
                 finishing {FRAMES} frames\n--- stdout ---\n{stdout}\n\
                 --- stderr ---\n{stderr}"
            )
        });
    let (frames, draw_calls) = reported
        .split_once(" draw_calls=")
        .unwrap_or_else(|| panic!("malformed report line: frames={reported}"));
    let frames: u32 = frames.parse().expect("frame count is a number");
    let draw_calls: u32 = draw_calls.trim().parse().expect("draw calls is a number");

    assert_eq!(frames, FRAMES, "the window delivered the wrong frame count");
    assert!(
        draw_calls > 0,
        "{frames} frames ran but nothing was ever drawn -- the window is open \
         and the renderer is submitting empty frames\n--- stderr ---\n{stderr}"
    );
}
