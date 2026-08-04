//! `bsengine-runtime --fixup <dir>` end to end, as a user and as `bsengine-mcp`
//! invoke it.
//!
//! The logic itself is covered by `bsengine_asset::identity::fixup`'s own tests.
//! What only this file can prove is that the mode is *reachable*: a `fixup` that
//! works perfectly but is not wired into `main` is a feature nobody can run, and
//! nothing inside `bsengine-asset` would notice.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

/// Where the model ended up, and the path a scene still names it by.
const MODEL_NOW: &str = "assets/models/fox.glb";
const MODEL_WAS: &str = "assets/models/old_fox.glb";
/// The same for a sound, which only a *script* names — the half `fixup` must
/// report rather than repair.
const SOUND_NOW: &str = "assets/sounds/hit.wav";
const SOUND_WAS: &str = "assets/sounds/thud.wav";

const MODEL_GUID: &str = "0193a7c1-8f2e-7c44-9d61-3b5a0e7f2c19";
const SOUND_GUID: &str = "0193a7c1-8f2e-7c44-9d61-3b5a0e7f2c20";

/// A throwaway project in which both assets have already moved, removed on
/// drop.
struct Probe(PathBuf);

impl Drop for Probe {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("create directories");
    std::fs::write(path, contents).expect("write");
}

impl Probe {
    /// # Why the sidecars are written as literal text
    ///
    /// This crate could build them through `bsengine_asset::identity::Sidecar`,
    /// and deliberately does not: writing the RON out by hand is what makes this
    /// test also a check that the on-disk format a user (or a previous release)
    /// left in their project is still one `fixup` can read. `size: None` is the
    /// pre-size spelling every already-committed sidecar has, so the scan
    /// re-hashes each asset once and keeps its identity — exactly what a real
    /// project's first run does.
    fn create() -> Self {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "bsengine-fixup-cli-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));

        write(
            &dir.join("project.toml"),
            "[project]\nname = \"Fixup Probe\"\nentry_scene = \"assets/scenes/main.ron\"\n",
        );
        for (asset, guid, was) in [
            (MODEL_NOW, MODEL_GUID, MODEL_WAS),
            (SOUND_NOW, SOUND_GUID, SOUND_WAS),
        ] {
            write(&dir.join(asset), "not really an asset");
            write(
                &dir.join(format!("{asset}.meta")),
                &format!(
                    "(guid: \"{guid}\", hash: \"blake3:stale\", size: None, \
                     former_paths: [\"{was}\"])\n"
                ),
            );
        }
        write(
            &dir.join("assets/scenes/main.ron"),
            &format!(
                "SceneDescriptor(entities: [\n    \
                 EntityDescriptor(name: \"Fox\", gltf: Some((guid: \"{MODEL_GUID}\", \
                 path: \"{MODEL_WAS}\"))),\n])\n"
            ),
        );
        write(
            &dir.join("assets/scripts/probe.js"),
            &format!("function onUpdate(self) {{\n  Bsengine.playSound(\"{SOUND_WAS}\");\n}}\n"),
        );
        Self(dir)
    }
}

fn run_fixup(dir: &Path, extra: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_bsengine-runtime"))
        .arg("--fixup")
        .arg(dir)
        .args(extra)
        .output()
        .expect("failed to run bsengine-runtime --fixup")
}

/// The mode exists, does its job, and says so — with no window, no renderer and
/// no game booted.
#[test]
fn the_cli_rewrites_the_scene_and_reports_the_script() {
    let probe = Probe::create();
    let script = probe.0.join("assets/scripts/probe.js");
    let before = std::fs::read(&script).expect("read script");

    let output = run_fixup(&probe.0, &[]);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    assert!(
        output.status.success(),
        "fixup exited {:?}\nstdout:\n{stdout}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        std::fs::read_to_string(probe.0.join("assets/scenes/main.ron"))
            .expect("read scene")
            .contains(MODEL_NOW),
        "the scene was not rewritten; fixup said:\n{stdout}"
    );
    assert_eq!(
        std::fs::read(&script).expect("read script"),
        before,
        "fixup rewrote a JavaScript file"
    );
    // The report is the whole point of the mode: a run that fixed everything
    // and printed nothing would leave the user unable to tell it from a no-op.
    assert!(
        stdout.contains(MODEL_WAS) && stdout.contains(MODEL_NOW),
        "{stdout}"
    );
    assert!(
        stdout.contains("probe.js") && stdout.contains(SOUND_WAS) && stdout.contains(SOUND_NOW),
        "the script reference has to be named well enough to act on:\n{stdout}"
    );
}

/// `--json` is what `bsengine-mcp`'s `game_fixup` parses, so stdout has to be
/// the report and nothing else. Anything the scan wants to say belongs on
/// stderr; a stray line on stdout would break every machine caller at once.
#[test]
fn the_json_report_is_the_only_thing_on_stdout() {
    let probe = Probe::create();

    let output = run_fixup(&probe.0, &["--json"]);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    let report: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("stdout is not one JSON document ({e}):\n{stdout}"));
    assert_eq!(report["rewritten"][0]["from"], MODEL_WAS, "{report:#}");
    assert_eq!(report["rewritten"][0]["to"], MODEL_NOW, "{report:#}");
    assert_eq!(report["scripts"][0]["stale_path"], SOUND_WAS, "{report:#}");
    assert_eq!(report["scripts"][0]["now_at"], SOUND_NOW, "{report:#}");
    assert_eq!(report["scripts"][0]["line"], 2, "{report:#}");
    assert_eq!(
        report["pruned"][0]["former_path"], MODEL_WAS,
        "the former path the rewrite made unnecessary must be forgotten: {report:#}"
    );
    assert_eq!(
        report["retained"][0]["former_path"], SOUND_WAS,
        "the one a script still names must be kept: {report:#}"
    );
}

/// A project directory that is not one. Reported on stderr and exited non-zero,
/// rather than printing an empty report that reads as "nothing to fix" — which
/// is what a caller would act on if the failure were quiet.
#[test]
fn a_directory_with_no_assets_fails_loudly() {
    let dir = std::env::temp_dir().join(format!("bsengine-fixup-empty-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create directory");
    let probe = Probe(dir);

    let output = run_fixup(&probe.0, &[]);

    assert!(
        !output.status.success(),
        "stdout was: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("assets"),
        "the message has to say what was missing: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
