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

#[test]
fn a_packaged_game_replays_its_recording_without_the_editor() {
    let root = repo_root();
    let project = root.join("games/mini-arena");
    let output = Output::new();

    let status = Command::new(env!("CARGO_BIN_EXE_bsengine-runtime"))
        .arg("--package")
        .arg(&project)
        .arg("--out")
        .arg(&output.0)
        .status()
        .expect("run --package");
    assert!(status.success(), "packaging mini-arena failed: {status}");

    // The recording is not an asset — nothing references it, and `scan`
    // excludes `assets/tests/` by name — so it is correctly absent from the
    // build, and the replay reads it from the source project instead.
    assert!(
        !output.0.join("assets/tests").exists(),
        "test recordings are not assets and must not ship"
    );
    let recording = project.join("assets/tests/basic-playthrough.testlog.json");

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
        "the packaged build must reproduce mini-arena's recorded playthrough, \
         which it cannot do if the cook left out an asset the game needs to \
         play: {}\n{log}",
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
        "the packaged build asked for {} asset(s) it did not have:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
