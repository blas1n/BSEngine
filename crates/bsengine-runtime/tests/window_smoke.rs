//! Does a window actually open, and does anything get drawn in it?
//!
//! ⚠️ The only file in this repo whose tests open a real window. Everything
//! else runs headless: the test harness builds a `fast_render` surface with no
//! window at all, and CI has only ever exercised that. So "the engine builds
//! and its whole suite passes on Linux" -- true on every PR -- has never meant
//! "a game opens on Linux". This is the step between the two.
//!
//! Runs the shipped executable rather than assembling an app here, for two
//! reasons. winit refuses to build an event loop off the main thread and
//! libtest runs every test on a worker thread, so in-process is not an option;
//! and a subprocess covers `main`'s own argument handling and the binary a
//! player launches, which a hand-assembled `App` would not.
//!
//! The second test here is the only one in the repo that watches a
//! *script-spawned glTF* reach the GPU. The `--test` host the E2E replays run
//! in has no `GltfPlugin` (it needs the GPU registries), so a headless test
//! can prove a script put the right components on an entity but never that a
//! mesh came out the other end. This window can.

use std::path::{Path, PathBuf};
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

/// Runs the shipped executable on `project` for `frames` frames and returns
/// what it reported: `(frames drawn, most draw calls in any one frame)`.
///
/// Panics -- with the runtime's own output -- if the window never closes
/// itself, exits non-zero, or never prints its report line, so every caller
/// gets the same three diagnostics without repeating them.
fn run_frames(project: &Path, frames: u32) -> (u32, u32) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_bsengine-runtime"))
        .arg(project)
        .arg("--frames")
        .arg(frames.to_string())
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
                    "the window never closed itself after {frames} frames -- \
                     killed it at {:?}. Either it never reached {frames} \
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

    let reported = stdout
        .lines()
        .find_map(|line| line.strip_prefix("frames="))
        .unwrap_or_else(|| {
            panic!(
                "the runtime never reported a frame count, so it exited before \
                 finishing {frames} frames\n--- stdout ---\n{stdout}\n\
                 --- stderr ---\n{stderr}"
            )
        });
    let (drawn, draw_calls) = reported
        .split_once(" draw_calls=")
        .unwrap_or_else(|| panic!("malformed report line: frames={reported}"));
    let drawn: u32 = drawn.parse().expect("frame count is a number");
    let draw_calls: u32 = draw_calls.trim().parse().expect("draw calls is a number");
    assert_eq!(drawn, frames, "the window delivered the wrong frame count");
    (drawn, draw_calls)
}

#[test]
fn a_window_opens_and_draws_frames() {
    let project = project("cube-evader");
    assert!(
        project.join("project.toml").is_file(),
        "fixture project is missing at {}",
        project.display()
    );

    // ⚠️ Asserting on draw calls, not just on the frame count. A window that
    // opens and presents nothing still ticks `Update` happily and would reach
    // its frame count either way.
    let (frames, draw_calls) = run_frames(&project, FRAMES);
    assert!(
        draw_calls > 0,
        "{frames} frames ran but nothing was ever drawn -- the window is open \
         and the renderer is submitting empty frames"
    );
}

/// Enough frames for a 160 KB glTF to be requested by a script's first
/// `onUpdate`, read off disk, uploaded, and drawn -- with room to spare, since
/// the report is the *maximum* draw calls over the run and a late frame
/// counts as well as an early one. Sized for the fast case, not the slow one:
/// a window that presents without vsync can tick hundreds of frames in the
/// time one asynchronous file read takes, and a budget counted in frames
/// shrinks in wall-clock terms exactly when the machine is quickest. Two
/// seconds at 60 Hz. Not a latency budget.
const SPAWN_FRAMES: u32 = 120;

/// A project whose scene draws nothing by itself -- a camera, a sun, and one
/// entity carrying only a script -- so every draw call in the report is that
/// script's doing. `spawn` picks whether the script actually spawns the fox
/// or is an empty `onUpdate`, and nothing else differs: the control run is
/// the same project minus the one call under test, which is what makes the
/// comparison a measurement of that call rather than of the renderer's
/// fixed passes.
///
/// Built under the temp directory rather than checked in: the fox is copied
/// from mini-arena (CC-BY, credited there), and a fixture that exists only
/// while the test runs cannot rot in the tree.
fn script_spawn_project(spawn: bool) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "bsengine-script-spawn-{}-{}",
        std::process::id(),
        if spawn { "spawn" } else { "control" }
    ));
    let _ = std::fs::remove_dir_all(&dir);
    for sub in ["assets/scenes", "assets/scripts", "assets/models"] {
        std::fs::create_dir_all(dir.join(sub)).expect("fixture dirs");
    }
    std::fs::copy(
        project("mini-arena").join("assets/models/fox.glb"),
        dir.join("assets/models/fox.glb"),
    )
    .expect("fox.glb is mini-arena's, and must exist there");

    // Small on purpose: CI's Linux runner rasterises in software, and the
    // window's size is the one knob that scales every frame's cost.
    std::fs::write(
        dir.join("project.toml"),
        "[project]\n\
         name = \"Script Spawn\"\n\
         entry_scene = \"assets/scenes/main.ron\"\n\
         \n\
         [window]\n\
         title = \"Script Spawn\"\n\
         width = 320\n\
         height = 240\n",
    )
    .expect("project.toml");

    // Same camera framing mini-arena gives its fox at the origin, so the
    // spawned one is on screen -- a mesh outside the frustum is culled and
    // would draw nothing, indistinguishable from a spawn that failed.
    std::fs::write(
        dir.join("assets/scenes/main.ron"),
        r#"SceneDescriptor(entities: [
    EntityDescriptor(
        name: "Camera",
        camera: true,
        transform: Some((position: (0.0, 2.0, 6.0))),
        look_at: Some((0.0, 0.5, 0.0)),
    ),
    EntityDescriptor(
        name: "Sun",
        directional_light: Some((direction: (-0.4, -0.8, -0.4), color: (1.0, 1.0, 1.0), ambient: (0.2, 0.2, 0.2))),
    ),
    EntityDescriptor(
        name: "Spawner",
        script: Some("assets/scripts/spawner.js"),
    ),
])"#,
    )
    .expect("main.ron");

    // The argument is a scene entity block in JS spelling -- the exact shape
    // `Bsengine.spawn` documents -- and the fox is scaled as mini-arena's is.
    let script = if spawn {
        "let done = false;\n\
         function onUpdate(name) {\n\
             if (done) return;\n\
             done = true;\n\
             Bsengine.spawn({\n\
                 name: \"Fox\",\n\
                 transform: { position: [0, 0, 0], scale: [0.02, 0.02, 0.02] },\n\
                 gltf: \"assets/models/fox.glb\",\n\
             });\n\
         }\n"
    } else {
        "function onUpdate(name) {}\n"
    };
    std::fs::write(dir.join("assets/scripts/spawner.js"), script).expect("spawner.js");
    dir
}

/// A glTF named in a `Bsengine.spawn` call is drawn -- not merely queued, not
/// merely a `GltfAsset` on an entity, but submitted to the GPU by the shipped
/// binary. Measured against a control run of the same project whose script
/// spawns nothing, so the renderer's fixed per-frame passes (whatever they
/// are on this platform) cancel out and only the spawned mesh remains.
///
/// The premise is asserted by construction: the control *is* the mutation
/// "drop the gltf from the spawn", and if it drew as much as the spawn run
/// the test could not tell the two apart and fails. A test that only checked
/// `draw_calls > 0` on the spawn run would pass on a renderer that draws a
/// skybox quad regardless.
#[test]
fn a_gltf_spawned_by_a_script_is_actually_drawn() {
    let control = script_spawn_project(false);
    let spawn = script_spawn_project(true);

    let (_, control_draws) = run_frames(&control, SPAWN_FRAMES);
    let (_, spawn_draws) = run_frames(&spawn, SPAWN_FRAMES);

    let _ = std::fs::remove_dir_all(&control);
    let _ = std::fs::remove_dir_all(&spawn);

    // Printed on success too, so a passing run in CI's log still says by how
    // much -- a margin of one is a very different thing from a margin of ten
    // when this next fails on a platform nobody can reproduce locally.
    println!("control_draws={control_draws} spawn_draws={spawn_draws}");
    assert!(
        spawn_draws > control_draws,
        "a script spawned assets/models/fox.glb and the window drew no more than the same \
         scene without it (spawn run: {spawn_draws} draw calls in its busiest frame, control: \
         {control_draws}) -- the spawn never produced a mesh, or produced one nothing drew"
    );
}
