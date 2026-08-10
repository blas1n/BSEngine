//! Roadmap item 30, the one condition unit tests cannot reach: **rename an
//! asset and watch a reference to its old path recover**, from the filesystem
//! event all the way to the game still playing.
//!
//! Every link is already covered on its own — `bsengine-asset`'s watcher tests
//! prove a rename records a former path, `bsengine-scene`'s resolution tests
//! prove a stale reference resolves through one, `bsengine-asset`'s
//! `load_mode` tests prove the same for a path spelled in JavaScript. Nothing
//! joined them. Each of those tests builds the state the next one consumes
//! *by hand*, so all three could keep passing while the chain between them was
//! broken — which is exactly how the `FileIdMap` bug sub-item D found had
//! survived: every unit test around it passed, because none of them ever asked
//! a real watcher for a real rename.
//!
//! # Why this is not a `--replay` recording
//!
//! It cannot be one, and saying so is more useful than approximating it.
//!
//! * **The protocol has no command that touches the filesystem.** `Command`
//!   (see `test_protocol.rs`) is step / key / mouse / query / assert /
//!   wait_until / shutdown. A recording can drive a game; it cannot rename a
//!   file underneath one.
//! * **The replay app does not run the watcher.** `build_test_app` leaves
//!   `AssetWatcherPlugin` out on purpose — it would add a background thread
//!   and frame-to-frame variation to the one mode that pins its clocks to stay
//!   reproducible. Adding it so that a recording could observe a rename would
//!   trade the harness's determinism for this single test, which is a bad
//!   trade and would regress the fix that made tilt-run's level-5 recordings
//!   stop failing on CI.
//!
//! So the coverage is built the closest honest way instead: this test performs
//! the rename against a **real running engine with the real watcher**, and then
//! hands the project it left behind to the **real `bsengine-runtime --test
//! --replay` binary**, whose recording asserts the recovery the ordinary way —
//! through `wait_until` on gameplay state. The rename half is in-process
//! because `AssetWatcherPlugin` is registered only by `run_windowed`, which
//! needs a window and a GPU; there is no headless binary that runs it.
//!
//! # Why a throwaway project rather than a game under `games/`
//!
//! A rename here is not just a rename: it moves a `.meta` sidecar and appends a
//! line to it. Doing that inside `games/` would edit **committed** files, and a
//! failure between the rename and the restore would leave the repository dirty
//! — the failure mode the restore exists to prevent. It would also race the
//! seven concurrent recordings that share `games/tilt-run`. And the fixture
//! needs a reference that resolves *only* through a former path, which is a
//! state no shipped game should ever be checked in holding. A temp directory
//! removed on drop has none of those problems and can leave nothing behind.
//!
//! # What is deliberately not asserted here
//!
//! The other half of sub-item D — a path spelled inside a JavaScript string
//! literal, recovered by `load_async` — is not in this recording, because its
//! only headless observable is an audio asset's status and this workspace
//! decodes only `.flac`/`.mp3` (see `bsengine-audio`'s `minimal_flac_silence`
//! for why a `.wav` fixture would not load). That half is covered by
//! `load_mode`'s own tests. What this file adds is the chain, and a *scene*
//! reference is the one whose recovery shows up as the game working.

use bsengine_asset::identity::{sidecar_path, Sidecar};
use bsengine_asset::test_support::{unique, ProbeDir};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

/// Where the script starts life, and the path the scene never stops naming.
const SCRIPT_WAS: &str = "assets/scripts/mover.js";
/// Where the rename puts it. Nothing in the fixture names this path — the only
/// way the engine can reach it is by remembering the move.
const SCRIPT_NOW: &str = "assets/scripts/walker.js";
/// Spelled the way a real game spells one, under the directory the scan skips.
const RECORDING: &str = "assets/tests/rename-recovery.testlog.json";

/// Hard ceiling on every wait here. A hung test in CI is worse than a failing
/// one, so nothing in this file blocks unbounded.
const HARD_TIMEOUT: Duration = Duration::from_secs(30);

/// How long to leave the watcher alone before and after touching the tree, in
/// multiples of `bsengine-asset`'s 200ms debounce window. Matches what that
/// crate's own watcher tests settle for.
const SETTLE: Duration = Duration::from_millis(600);

/// The phrase only a former-path recovery emits. Asserted on rather than the
/// whole sentence so a reworded warning does not fail this test, while a
/// *silent* recovery — the thing sub-item D refuses to allow — still does.
const RECOVERY_PHRASE: &str = "used to live there and is now at";

/// One entity, one script, and the script is the only thing in the project that
/// can move it. Nothing else — no physics body, no second script — so the
/// movement the recording waits for has exactly one possible cause.
const SCRIPT: &str = "\
const STEP = 0.1;

function onUpdate() {
    const t = Bsengine.getPosition(\"Runner\");
    if (!t) return;
    Bsengine.setPosition(\"Runner\", t.x + STEP, t.y, t.z);
}
";

fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().expect("every fixture file has a parent"))
        .expect("create the fixture's directories");
    std::fs::write(path, contents).expect("write a fixture file");
}

/// A project whose scene names its script by a **bare path** — the pre-item-30
/// spelling every scene in `games/` was written in, and the only one for which
/// a former path is the sole thing standing between a rename and a reference
/// that loads nothing. An identified `(guid, path)` reference would be rescued
/// by sub-item B's GUID lookup and would never reach the code this test is
/// about.
fn create_fixture() -> ProbeDir {
    let root = std::env::temp_dir().join(unique("rename-e2e"));

    write(
        &root.join("project.toml"),
        "[project]\nname = \"Rename Recovery\"\nentry_scene = \"assets/scenes/main.ron\"\n",
    );
    write(&root.join(SCRIPT_WAS), SCRIPT);
    write(
        &root.join("assets/scenes/main.ron"),
        &format!(
            "SceneDescriptor(entities: [\n    \
             EntityDescriptor(\n        \
             name: \"Runner\",\n        \
             transform: Some((position: (0.0, 0.0, 0.0))),\n        \
             script: Some(\"{SCRIPT_WAS}\"),\n    \
             ),\n])\n"
        ),
    );
    write(&root.join(RECORDING), RECORDING_JSON);

    ProbeDir(root)
}

/// The recording, written beside the fixture exactly where `games/*/assets/tests/`
/// keeps the shipped ones.
///
/// `wait_until` rather than a fixed frame count, for the reason item 24 Phase 1
/// introduced it: the number of frames a script-driven entity needs to cover a
/// distance is not a property of the game, and pinning one makes the recording
/// fail on whichever machine is not the one it was recorded on.
///
/// The `assert` before the wait is the vacuity guard. Without it a `wait_until`
/// whose predicate already holds passes having stepped zero frames, which would
/// report "recovered" for a project where nothing ran at all.
const RECORDING_JSON: &str = r#"{
  "game": "rename-recovery",
  "scene": "assets/scenes/main.ron",
  "actions": [
    { "cmd": "step", "frames": 1 },
    {
      "cmd": "assert",
      "query": { "tool": "get_transform", "args": { "name": "Runner" } },
      "path": "x",
      "op": "<",
      "value": 1.0,
      "label": "Runner exists and has not yet reached the wait's target, so the wait below has to actually wait (a predicate that already holds would pass having run nothing)"
    },
    {
      "cmd": "wait_until",
      "query": { "tool": "get_transform", "args": { "name": "Runner" } },
      "path": "x",
      "op": ">=",
      "value": 1.0,
      "max_frames": 600,
      "label": "Runner moved: the scene still names the script at its old path, so it only moves if the engine found the file the script was renamed to (0.1 per frame, so ~10 frames of the 600 budgeted)"
    },
    { "cmd": "shutdown" }
  ]
}
"#;

/// Runs frames until `done`, or panics after [`HARD_TIMEOUT`]. Bounded by the
/// wall clock rather than a frame count because what is being waited on is an
/// OS filesystem notification, not a fixed amount of work.
fn run_until(app: &mut bevy_app::App, what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + HARD_TIMEOUT;
    while Instant::now() < deadline {
        app.update();
        if done() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("{what} did not happen within {HARD_TIMEOUT:?}");
}

/// Link one: renames the script while a real engine is running, through the
/// real `AssetWatcherPlugin`, and returns only once the move is on disk.
///
/// The app is built and dropped entirely inside this function so that the
/// watcher's thread and its OS handles are gone before the child process below
/// scans the same directory — and, on Windows, before `ProbeDir` tries to
/// remove it.
///
/// The three plugins are the ones `run_windowed` gives a running game
/// (`main.rs`): `AssetPlugin` for the `AssetServer` and the asset root,
/// `AssetIdentityPlugin` for the scan that mints the sidecar, `AssetWatcherPlugin`
/// for the watch. Nothing else is needed to move a file, and adding the rest of
/// the engine would only add ways for this half to fail for reasons that are not
/// about renaming.
fn rename_the_script_with_the_engine_running(root: &Path) {
    let project_dir = root
        .to_str()
        .expect("the temp directory is not valid UTF-8")
        .to_string();

    let mut app = bsengine_app::new_app();
    app.insert_resource(bsengine_core::ProjectDir(project_dir));
    app.add_plugins(bsengine_asset::AssetPlugin);
    app.add_plugins(bsengine_asset::AssetIdentityPlugin);
    app.add_plugins(bsengine_asset::AssetWatcherPlugin);
    app.update();

    let was = root.join(SCRIPT_WAS);
    let now = root.join(SCRIPT_NOW);
    let minted = Sidecar::read(sidecar_path(&was))
        .expect("read the sidecar the scan just wrote")
        .expect("the scan must give a .js file under assets/ an identity");
    assert!(
        minted.former_paths.is_empty(),
        "the fixture has never moved, so anything already recorded here would \
         make the assertion after the rename meaningless"
    );

    // Let the backend actually begin delivering, then drain everything startup
    // stirred up — the scan's own sidecar writes included, which are themselves
    // renames (`Sidecar::write` is atomic).
    std::thread::sleep(SETTLE);
    app.update();

    std::fs::rename(&was, &now).expect("rename the script");

    run_until(&mut app, "the sidecar followed the renamed script", || {
        sidecar_path(&now).exists()
    });

    let moved = Sidecar::read(sidecar_path(&now))
        .expect("read the moved sidecar")
        .expect("present");
    assert_eq!(
        moved.guid, minted.guid,
        "a rename must not change the identity"
    );
    assert_eq!(
        moved.former_paths,
        [SCRIPT_WAS],
        "without this line on disk there is nothing for the next process to \
         recover the scene's reference with"
    );
    assert!(
        !sidecar_path(&was).exists(),
        "a sidecar left at the old name turns a clean rename into an orphan"
    );
}

/// Link two and three: the shipped E2E harness, on the project the rename left
/// behind.
///
/// Returns the child's output so the caller can read the warning off stderr —
/// "the reference recovered" and "the engine said so" are two different claims
/// and this test makes both.
fn replay(root: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_bsengine-runtime"))
        .arg("--test")
        .arg(root)
        .arg("--replay")
        .arg(root.join(RECORDING))
        // Pinned rather than inherited: the recovery warning is half of what
        // this test asserts, and whether it reaches stderr would otherwise
        // depend on whatever `RUST_LOG` the person running the suite happens to
        // have exported. This is the level `init_logging`'s own default enables.
        .env("RUST_LOG", "bsengine=warn")
        .output()
        .expect("failed to run bsengine-runtime --test --replay")
}

/// The whole chain, in one test, because it is one claim: a project whose
/// script has been renamed out from under its scene still plays, and says why.
///
/// Split into two tests it would be possible for the first to leave state the
/// second silently depended on, which is the shape of coupling this file exists
/// to remove.
#[test]
fn a_renamed_asset_is_still_found_by_a_scene_that_names_its_old_path() {
    // Declared before anything that could panic, so it is dropped last and the
    // directory goes away on the failure path as well as the success one.
    let probe = create_fixture();
    let root = probe.0.clone();

    rename_the_script_with_the_engine_running(&root);

    let output = replay(&root);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    assert!(
        output.status.success(),
        "the recording failed, which means the scene's reference to \
         '{SCRIPT_WAS}' did not find the script at '{SCRIPT_NOW}' and the entity \
         never moved\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // A recovery nobody is told about is a permanent invisible redirect — the
    // accumulated-forwarding pain Unreal documents — so silence here is a
    // failure even though the game ran.
    assert!(
        stderr.contains(RECOVERY_PHRASE),
        "the engine recovered the reference without saying so; nothing else \
         will ever tell the developer, since the scene file still names the old \
         path\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains(SCRIPT_WAS) && stderr.contains(SCRIPT_NOW),
        "the warning has to name both paths or the developer cannot tell which \
         reference to fix, nor whether what it found is the right \
         file\nstderr:\n{stderr}"
    );
}

/// The same project with the memory of the move taken away, to show what this
/// test is actually measuring.
///
/// Deleting the `former_paths` line is the smallest edit that removes only
/// sub-item D: the script is still there under its new name, the scene is
/// unchanged, the identity is intact — the one thing missing is the record of
/// where the asset used to be. The recording must fail, and it must fail at the
/// wait rather than by crashing, because that is the difference between "the
/// reference did not recover" and "the harness broke".
///
/// Without this, the test above would pass just as happily against an engine
/// that recovered nothing but happened to move the entity for some other reason.
#[test]
fn without_the_recorded_former_path_the_same_recording_fails() {
    let probe = create_fixture();
    let root = probe.0.clone();

    rename_the_script_with_the_engine_running(&root);

    // Rewritten through `Sidecar` rather than by string surgery, so this is the
    // same file shape the scan reads back and the test cannot pass because it
    // wrote something unparseable.
    let path = sidecar_path(root.join(SCRIPT_NOW));
    let mut sidecar = Sidecar::read(&path)
        .expect("read the moved sidecar")
        .expect("present");
    sidecar.former_paths.clear();
    sidecar.write(&path).expect("write the forgetful sidecar");

    let output = replay(&root);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    assert!(
        !output.status.success(),
        "with nothing remembering the move, the scene's reference to \
         '{SCRIPT_WAS}' names a file that does not exist — the script cannot \
         load and the entity cannot move, so a passing recording would mean the \
         recording is not measuring the recovery\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("Runner moved"),
        "it has to fail at the wait, naming it, rather than by crashing \
         somewhere earlier\nstderr:\n{stderr}"
    );
    assert!(
        !stderr.contains(RECOVERY_PHRASE),
        "nothing should have been recovered here\nstderr:\n{stderr}"
    );
}
