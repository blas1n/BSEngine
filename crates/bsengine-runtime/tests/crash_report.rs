//! Does a crash in the shipped executable leave a report and a log behind?
//!
//! Runs the real binary -- a panic hook installed in-process would outlive
//! the test and catch every other test's panics, and only the binary shows
//! the hook is installed where a player's game installs it. The project is a
//! throwaway with an unknown key in its `[input]` table: the runtime refuses
//! to start on that with a panic, after the crash handler is in place and
//! before any window opens, so this needs no display.

use std::path::{Path, PathBuf};
use std::process::Command;

const PROJECT_NAME: &str = "Crash Probe";

/// A project whose start-up panics on `jump = ["NotAKey"]`.
fn crashing_project(root: &Path) -> PathBuf {
    let dir = root.join("project");
    std::fs::create_dir_all(dir.join("assets/scenes")).unwrap();
    std::fs::write(
        dir.join("project.toml"),
        format!(
            "[project]\nname = \"{PROJECT_NAME}\"\nentry_scene = \"assets/scenes/main.ron\"\n\n\
             [input.actions]\njump = [\"NotAKey\"]\n"
        ),
    )
    .unwrap();
    dir
}

/// Runs the executable on `project` with its per-user directory at `user`,
/// returning its exit status and stderr.
fn run(project: &Path, user: &Path) -> (std::process::ExitStatus, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_bsengine-runtime"))
        .env(bsengine_core::crash::USER_DIR_ENV, user)
        .arg(project)
        .output()
        .expect("failed to launch the runtime executable");
    (
        out.status,
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn reports(user: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(user.join("crashes"))
        .map(|dir| dir.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    found.sort();
    found
}

/// The panic produces a report naming the project, the message, where it
/// happened, the thread, a backtrace, and the log up to that point -- and
/// the log itself is on disk beside it. A second run keeps the first run's
/// log as `game-prev.log` and adds a second report rather than replacing
/// the first.
#[test]
fn a_panic_leaves_a_crash_report_and_the_log() {
    let root = std::env::temp_dir().join(format!("bsengine_crash_probe_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let project = crashing_project(&root);
    let user = root.join("user");

    let (status, stderr) = run(&project, &user);
    assert!(
        !status.success(),
        "premise: the bad binding stops the game. stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("Cannot use project.toml's [input] table"),
        "premise: it stopped for the reason this test set up. stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("crash report written to"),
        "the player is told where the report is. stderr:\n{stderr}"
    );

    let first = reports(&user);
    assert_eq!(first.len(), 1, "one panic, one report: {first:?}");
    let report = std::fs::read_to_string(&first[0]).unwrap();
    for expected in [
        "project:  Crash Probe (BSEngine ",
        "message:  Cannot use project.toml's [input] table",
        "NotAKey",
        "location: ",
        "main.rs",
        "thread:   main",
        "backtrace:",
        // A real frame, not "disabled backtrace": captured whether or not
        // RUST_BACKTRACE is set, which no player's machine has.
        "insert_input_actions",
        // The log's first line, copied in: the log was being written to
        // the file before the panic, and the report carries its tail.
        "Crash Probe: logging to",
    ] {
        assert!(
            report.contains(expected),
            "the report lacks {expected:?}:\n{report}"
        );
    }
    let log = user.join("logs/game.log");
    let log_text = std::fs::read_to_string(&log).expect("the log file is on disk");
    assert!(log_text.contains("Crash Probe: logging to"), "{log_text}");
    assert!(
        !log_text.contains("\u{1b}["),
        "the file gets plain text, not terminal colour codes"
    );

    let (status, _) = run(&project, &user);
    assert!(!status.success());
    assert_eq!(reports(&user).len(), 2, "the second crash adds a report");
    assert!(
        user.join("logs/game-prev.log").exists(),
        "the first run's log is kept beside the second's"
    );
    let _ = std::fs::remove_dir_all(&root);
}
