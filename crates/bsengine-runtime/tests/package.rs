//! `bsengine-runtime --package <dir>` end to end.
//!
//! The cook's own tests, in `bsengine_asset::cook`, check *which* files it
//! collects. None of them can tell whether the set it collected is enough to
//! play the game — that is a question about the engine, not about the walk.
//! Replaying a real recorded playthrough against the packaged directory answers
//! it: "the build starts" would pass on one missing every asset the recording
//! touches, and reproducing a recorded playthrough would not.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

/// The packaged output, removed on drop.
struct Output(PathBuf);

impl Drop for Output {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

impl Output {
    fn new() -> Self {
        static N: AtomicU32 = AtomicU32::new(0);
        Output(std::env::temp_dir().join(format!(
            "bsengine-package-e2e-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        )))
    }
}

/// The repository root, from this crate's manifest directory.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repository root")
        .to_path_buf()
}

/// Packages `games/mini-arena` in `mode` and replays its recording against the
/// result, returning the build so a caller can assert on its shape.
///
/// Shared by both modes on purpose: the two builds must be interchangeable from
/// the game's point of view, and running them through the identical assertions
/// is what says so.
fn package_and_replay(mode: &str) -> Output {
    package_and_replay_project(
        "games/mini-arena",
        "assets/tests/basic-playthrough.testlog.json",
        mode,
    )
}

/// The same, for any project and recording.
fn package_and_replay_project(project_rel: &str, recording_rel: &str, mode: &str) -> Output {
    let root = repo_root();
    let project = root.join(project_rel);
    let output = Output::new();

    let status = Command::new(env!("CARGO_BIN_EXE_bsengine-runtime"))
        .arg("--package")
        .arg(&project)
        .arg("--out")
        .arg(&output.0)
        .arg("--mode")
        .arg(mode)
        .status()
        .expect("run --package");
    assert!(
        status.success(),
        "packaging {project_rel} as {mode} failed: {status}"
    );

    // The recording is not an asset — nothing references it, and `scan`
    // excludes `assets/tests/` by name — so it is correctly absent from the
    // build, and the replay reads it from the source project instead.
    assert!(
        !output.0.join("assets/tests").exists(),
        "test recordings are not assets and must not ship"
    );
    let recording = project.join(recording_rel);

    let packaged_exe = output.0.join(
        Path::new(env!("CARGO_BIN_EXE_bsengine-runtime"))
            .file_name()
            .expect("runtime file name"),
    );
    assert!(packaged_exe.is_file(), "the build must carry an executable");

    // `current_dir` is not incidental. `bevy_asset` roots at the process
    // working directory, not at the project directory it is handed, so a
    // packaged build run from anywhere else looks for its textures beside
    // whatever launched it. Running from inside the build is what a player
    // does, and the first version of this test -- which did not -- passed
    // while the game silently failed to load `checker.png`.
    let run = Command::new(&packaged_exe)
        .current_dir(&output.0)
        .arg("--test")
        .arg(&output.0)
        .arg("--replay")
        .arg(&recording)
        .output()
        .expect("run the packaged build");
    let log = String::from_utf8_lossy(&run.stderr);

    assert!(
        run.status.success(),
        "the {mode} build of {project_rel} must reproduce its recorded \
         playthrough, which it cannot do if the cook left out an asset the game \
         needs to play: {}\n{log}",
        run.status
    );

    // The replay alone is not enough, and this is the assertion that says so.
    // mini-arena's recording still passes with `checker.png` missing -- nothing
    // it does depends on that texture -- so a build that shipped without it
    // would look fine here. What cannot be faked is the engine asking for an
    // asset and not finding it: that is precisely the cook having left
    // something out, whether or not the recording happens to notice.
    let failures: Vec<&str> = log
        .lines()
        .filter(|line| line.contains("Path not found") || line.contains("failed to load"))
        .collect();
    assert!(
        failures.is_empty(),
        "the {mode} build asked for {} asset(s) it did not have:\n{}",
        failures.len(),
        failures.join("\n")
    );

    output
}

#[test]
fn a_packaged_game_replays_its_recording_without_the_editor() {
    let output = package_and_replay("loose");
    assert!(
        output.0.join("assets").is_dir(),
        "a loose build carries its assets as ordinary files"
    );
}

/// The archive half.
///
/// The no-loose-assets assertion is the load-bearing one. Without it, a pak
/// build that silently fell back to reading files would pass every assertion
/// above identically — the archive could be empty, or never opened — and the
/// whole feature would be unproven. With nothing on disk to fall back to,
/// a passing replay can only mean the archive was read.
#[test]
fn a_pak_packaged_game_replays_its_recording_from_the_archive() {
    let output = package_and_replay("pak");

    assert!(
        output.0.join("game.pak").is_file(),
        "the build must carry an archive"
    );
    assert!(
        !output.0.join("assets").exists(),
        "a pak build must not also carry loose assets -- with them present the \
         replay proves nothing about the archive, since it could have read the \
         files instead"
    );
}

/// The scene shim, which no `AssetReader` covers.
///
/// `tilt-run` is the only game that calls `Bsengine.loadScene`, and clearing
/// level 1 transitions into level 2 — so this drives the *runtime* scene-load
/// site (`scene_systems::handle_scene_load`) rather than the entry-scene one
/// that every other test exercises. A pak build whose shim only handled the
/// entry scene would start fine, play level 1 from the archive, and die at the
/// transition.
#[test]
fn a_pak_build_loads_a_second_scene_through_the_shim() {
    let output = package_and_replay_project(
        "games/tilt-run",
        "assets/tests/level1-clear.testlog.json",
        "pak",
    );

    assert!(
        !output.0.join("assets").exists(),
        "nothing to fall back to, so the second scene came out of the archive"
    );
}
