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

/// Runs the executable on `project` with its per-user directory at `user`
/// and `extra` arguments, returning its exit status and stderr.
///
/// Stderr goes to a file, not a pipe. The game starts a crash monitor, and on
/// Windows a child process inherits its parent's inheritable handles -- the
/// game's stderr pipe included, whatever the monitor's own stdio is set to.
/// Read through a pipe, a monitor that wrongly outlived the game would hold
/// it open and the read would wait for ever: the test would hang rather than
/// fail. (It did, for days, under a mutation that kept the monitor alive.)
fn run(project: &Path, user: &Path, extra: &[&str]) -> (std::process::ExitStatus, String) {
    let stderr_path = user.with_extension(format!("stderr-{}.txt", extra.join("")));
    std::fs::create_dir_all(stderr_path.parent().unwrap()).unwrap();
    let stderr_file = std::fs::File::create(&stderr_path).unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_bsengine-runtime"))
        .env(bsengine_core::crash::USER_DIR_ENV, user)
        .arg(project)
        .args(extra)
        .stdout(std::process::Stdio::null())
        .stderr(stderr_file)
        .status()
        .expect("failed to launch the runtime executable");
    let stderr = std::fs::read_to_string(&stderr_path).unwrap_or_default();
    (status, stderr)
}

/// Whether process `pid` is still running.
fn process_exists(pid: u32) -> bool {
    if cfg!(windows) {
        let out = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .output()
            .expect("tasklist");
        String::from_utf8_lossy(&out.stdout).contains(&format!("\"{pid}\""))
    } else {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .expect("kill")
            .success()
    }
}

fn kill_process(pid: u32) {
    let _ = if cfg!(windows) {
        Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .status()
    } else {
        Command::new("kill").args(["-9", &pid.to_string()]).status()
    };
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

    let (status, stderr) = run(&project, &user, &[]);
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

    // The native crash monitor the game started (see the test below) goes
    // when the game does: a panic is not a native crash, so it has nothing
    // to write, and a monitor that outlived every game would pile up one
    // process per launch.
    let monitor: u32 = stderr
        .split("monitor process ")
        .nth(1)
        .and_then(|rest| rest.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|pid| pid.parse().ok())
        .unwrap_or_else(|| panic!("premise: the monitor was started. stderr:\n{stderr}"));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while process_exists(monitor) && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    if process_exists(monitor) {
        // Killed before failing: a monitor left behind holds every handle it
        // inherited -- the test runner's output pipe among them on Windows --
        // and would turn this failure into a hang of the whole run.
        kill_process(monitor);
        panic!("the crash monitor ({monitor}) outlived the game");
    }

    let first = reports(&user);
    assert_eq!(
        first.len(),
        1,
        "one panic, one report -- and no minidump: {first:?}"
    );
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

    let (status, _) = run(&project, &user, &[]);
    assert!(!status.success());
    assert_eq!(reports(&user).len(), 2, "the second crash adds a report");
    assert!(
        user.join("logs/game-prev.log").exists(),
        "the first run's log is kept beside the second's"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// A project that starts cleanly: nothing in it panics.
fn sound_project(root: &Path) -> PathBuf {
    let dir = root.join("project");
    std::fs::create_dir_all(dir.join("assets/scenes")).unwrap();
    std::fs::write(
        dir.join("project.toml"),
        format!("[project]\nname = \"{PROJECT_NAME}\"\nentry_scene = \"assets/scenes/main.ron\"\n"),
    )
    .unwrap();
    std::fs::write(dir.join("assets/scenes/main.ron"), "(entities: [])").unwrap();
    dir
}

/// A native crash -- a write through a null pointer, which no panic hook
/// sees -- leaves a minidump and a report beside it, written by the crash
/// monitor the game started, and named for the game's process. The report
/// names the exception and the dump and carries the log's tail; the dump is
/// a minidump (its `MDMP` signature) with the crashed process in it, not an
/// empty file.
#[test]
fn a_native_crash_leaves_a_minidump_and_a_report() {
    let root = std::env::temp_dir().join(format!("bsengine_native_crash_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let project = sound_project(&root);
    let user = root.join("user");

    let (status, stderr) = run(&project, &user, &["--force-crash"]);
    assert!(
        !status.success(),
        "premise: the forced crash ended the game. stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("panicked"),
        "premise: it was a native crash, not a panic -- the panic hook \
         would have written a report of its own. stderr:\n{stderr}"
    );

    // The monitor writes the report after the dump, and may still be at it
    // when the game it watched is gone.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let (dumps, texts) = loop {
        let found = reports(&user);
        let pick = |ext: &str| -> Vec<PathBuf> {
            found
                .iter()
                .filter(|p| p.extension().is_some_and(|e| e == ext))
                .cloned()
                .collect()
        };
        let (dumps, texts) = (pick("dmp"), pick("txt"));
        if !texts.is_empty() || std::time::Instant::now() > deadline {
            break (dumps, texts);
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    };
    assert_eq!(dumps.len(), 1, "one minidump: {:?}", reports(&user));
    assert_eq!(texts.len(), 1, "one report, the native one: {texts:?}");
    assert_eq!(
        dumps[0].file_stem(),
        texts[0].file_stem(),
        "the dump and its report share a name"
    );

    let dump = std::fs::read(&dumps[0]).unwrap();
    assert_eq!(&dump[..4], b"MDMP", "a minidump, by its signature");
    assert!(
        dump.len() > 4096,
        "with the process in it: {} bytes",
        dump.len()
    );

    let report = std::fs::read_to_string(&texts[0]).unwrap();
    let exception = if cfg!(windows) {
        "EXCEPTION_ACCESS_VIOLATION"
    } else if cfg!(target_os = "macos") {
        "EXC_BAD_ACCESS"
    } else {
        "SIGSEGV"
    };
    let dump_name = dumps[0].file_name().unwrap().to_string_lossy().into_owned();
    for expected in [
        "project:  Crash Probe (BSEngine ",
        "message:  native crash: ",
        exception,
        dump_name.as_str(),
        // The log's tail, as a panic report carries it.
        "Crash Probe: logging to",
        "native crash capture on",
    ] {
        assert!(
            report.contains(expected),
            "the report lacks {expected:?}:\n{report}"
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}
