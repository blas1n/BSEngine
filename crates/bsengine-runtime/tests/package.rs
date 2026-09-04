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

    let status = Command::new(&packaged_exe)
        .arg("--test")
        .arg(&output.0)
        .arg("--replay")
        .arg(&recording)
        .status()
        .expect("run the packaged build");
    assert!(
        status.success(),
        "the packaged build must reproduce mini-arena's recorded playthrough, \
         which it cannot do if the cook left out an asset the game loads: {status}"
    );
}
