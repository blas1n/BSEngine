//! Where a game keeps its log and its crash reports, and the panic hook that
//! writes one.
//!
//! A shipped game has no console: on Windows it is not even attached to one.
//! Before this module the engine logged to stderr only, so a player's crash
//! left nothing behind -- no log, no message, no backtrace -- and a bug report
//! was "it closed". All three reference engines keep both on disk, under a
//! per-user directory rather than beside the executable (which may be
//! read-only, and is shared between users): Unity's `Player.log` and
//! `Crash_<date>` folders under `LocalLow/<company>/<product>`, Unreal's
//! `Saved/Logs` and `Saved/Crashes` under `%LOCALAPPDATA%/<Game>`, Godot's
//! `user://logs`. This is that: [`init_for_project`] opens the log file and
//! installs a panic hook that writes a report beside it.
//!
//! This module catches panics. A native crash -- a segfault inside a GPU
//! driver -- does not unwind and does not reach a panic hook; `native_crash`
//! catches those, with a signal/SEH handler and a minidump written by a
//! monitor process, and [`init_for_project`] installs both.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

/// Environment variable that replaces the per-user directory outright -- for
/// tests and CI, which must not write into a real user profile, and for a
/// player who wants the files somewhere else.
pub const USER_DIR_ENV: &str = "BSENGINE_USER_DIR";

/// Log lines copied into a crash report: the end of the log, which is what
/// led up to the crash.
pub const REPORT_LOG_LINES: usize = 200;

/// Crash reports kept; older ones are removed as new ones are written, so a
/// game that crashes on every start does not fill the disk. (Godot rotates
/// its logs the same way.)
pub const REPORTS_KEPT: usize = 20;

/// The paths [`init_for_project`] set up.
#[derive(Debug, Clone)]
pub struct UserPaths {
    /// The per-user directory everything below lives in.
    pub dir: PathBuf,
    /// This run's log file.
    pub log: PathBuf,
    /// Where crash reports are written.
    pub crashes: PathBuf,
}

/// The per-user directory for a project: `%LOCALAPPDATA%\<name>` on Windows,
/// `~/Library/Application Support/<name>` on macOS, `$XDG_DATA_HOME/<name>`
/// (or `~/.local/share/<name>`) elsewhere -- or [`USER_DIR_ENV`] when set.
pub fn user_data_dir(project_name: &str) -> PathBuf {
    let base = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    };
    user_data_dir_from(
        std::env::var(USER_DIR_ENV).ok().as_deref(),
        base,
        project_name,
    )
}

/// [`user_data_dir`] with the environment passed in, so it can be tested
/// without touching the process's environment (which every other test in the
/// binary shares).
pub fn user_data_dir_from(
    override_dir: Option<&str>,
    platform_base: Option<PathBuf>,
    project_name: &str,
) -> PathBuf {
    if let Some(dir) = override_dir.filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    // No home directory at all (a stripped-down container): the working
    // directory, which is where the game was already writing its saves.
    platform_base
        .unwrap_or_else(|| PathBuf::from("."))
        .join(folder_name(project_name))
}

/// A project name made safe as one path component: characters a file system
/// may refuse become `_`, and an empty result becomes a generic name.
pub fn folder_name(project_name: &str) -> String {
    let cleaned: String = project_name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').trim();
    if trimmed.is_empty() {
        "BSEngine Game".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Opens the project's log file and installs the crash handler: the whole of
/// what a game does at start-up. Call before the first `new_app()`, which
/// would otherwise set up stderr-only logging first (the first set-up wins).
///
/// Failing to create the directory or the file is reported and skipped, never
/// fatal: a game that cannot write a log should still run.
pub fn init_for_project(project_name: &str) -> UserPaths {
    let dir = user_data_dir(project_name);
    let paths = UserPaths {
        log: dir.join("logs").join("game.log"),
        crashes: dir.join("crashes"),
        dir,
    };
    let log = match crate::logging::init_logging_with_file(&paths.log) {
        Ok(true) => Some(paths.log.clone()),
        Ok(false) => {
            tracing::warn!("logging was already set up; no log file this run");
            None
        }
        Err(e) => {
            crate::logging::init_logging();
            tracing::warn!("cannot open the log file {}: {e}", paths.log.display());
            None
        }
    };
    tracing::info!(
        "{project_name}: logging to {}, crash reports to {}",
        paths.log.display(),
        paths.crashes.display()
    );
    let context = format!("{project_name} (BSEngine {})", env!("CARGO_PKG_VERSION"));
    // Native crashes: a monitor process writes a minidump and a report into
    // the same directory. Without it the game still runs, with panic reports
    // only -- crash capture is never why a game does not start.
    #[cfg(not(target_arch = "wasm32"))]
    match crate::native_crash::attach(&paths.crashes, log.as_deref(), &context) {
        Ok(monitor) => {
            tracing::info!("native crash capture on (minidumps; monitor process {monitor})")
        }
        Err(e) => tracing::warn!("native crashes will leave no minidump: {e}"),
    }
    install_crash_handler(paths.crashes.clone(), log, context);
    paths
}

/// Set while a report is being written, so a panic *inside* the hook -- a
/// full disk, a poisoned lock -- does not recurse into it forever.
static WRITING_REPORT: AtomicBool = AtomicBool::new(false);

/// Installs a panic hook that writes a report into `crash_dir` for every
/// panic, on any thread, after the hook that was there before -- so the
/// message still reaches stderr exactly as it did.
///
/// Every panic, not only the one that ends the process: a panicking worker
/// thread is a bug whether or not the game survives it, and its report is
/// the only trace it leaves.
pub fn install_crash_handler(crash_dir: PathBuf, log_path: Option<PathBuf>, context: String) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        previous(info);
        if WRITING_REPORT.swap(true, Ordering::SeqCst) {
            return;
        }
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "<non-string panic payload>".to_string());
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown>".to_string());
        let thread = std::thread::current();
        let thread = thread.name().unwrap_or("<unnamed>").to_string();
        // Forced: the default hook prints one only with RUST_BACKTRACE set,
        // which no player's machine has.
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        let report = CrashReport {
            context: &context,
            thread: &thread,
            message: &message,
            location: &location,
            backtrace: &backtrace,
        };
        match write_crash_report(&crash_dir, log_path.as_deref(), &report, SystemTime::now()) {
            Ok(path) => eprintln!("crash report written to {}", path.display()),
            Err(e) => eprintln!(
                "could not write a crash report to {}: {e}",
                crash_dir.display()
            ),
        }
        WRITING_REPORT.store(false, Ordering::SeqCst);
    }));
}

/// What a crash report says about the panic itself.
pub struct CrashReport<'a> {
    /// The project and engine version.
    pub context: &'a str,
    /// The panicking thread's name.
    pub thread: &'a str,
    /// The panic message.
    pub message: &'a str,
    /// `file:line:column` of the panic.
    pub location: &'a str,
    /// The captured backtrace.
    pub backtrace: &'a str,
}

/// Writes one report into `crash_dir` -- named for its UTC time and the
/// process, so two in the same second from two runs do not collide -- with
/// the last [`REPORT_LOG_LINES`] of the log appended, then prunes the
/// directory to [`REPORTS_KEPT`]. Returns the report's path.
pub fn write_crash_report(
    crash_dir: &Path,
    log_path: Option<&Path>,
    report: &CrashReport<'_>,
    now: SystemTime,
) -> std::io::Result<PathBuf> {
    write_crash_report_for(std::process::id(), crash_dir, log_path, report, now)
}

/// [`write_crash_report`] for process `pid` rather than this one: the native
/// crash monitor (`native_crash`) writes the report of the game it watched,
/// named for the game, so the report and its minidump sort together.
pub fn write_crash_report_for(
    pid: u32,
    crash_dir: &Path,
    log_path: Option<&Path>,
    report: &CrashReport<'_>,
    now: SystemTime,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(crash_dir)?;
    let stamp = utc_stamp(now);
    let mut path = crash_dir.join(format!("crash-{stamp}-{pid}.txt"));
    // A second panic in the same second, same process: number it.
    let mut n = 1;
    while path.exists() {
        n += 1;
        path = crash_dir.join(format!("crash-{stamp}-{pid}-{n}.txt"));
    }
    let mut out = std::fs::File::create(&path)?;
    writeln!(out, "BSEngine crash report")?;
    writeln!(out, "time:     {stamp} UTC")?;
    writeln!(out, "project:  {}", report.context)?;
    writeln!(
        out,
        "os:       {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    )?;
    writeln!(out, "thread:   {}", report.thread)?;
    writeln!(out, "message:  {}", report.message)?;
    writeln!(out, "location: {}", report.location)?;
    writeln!(out)?;
    writeln!(out, "backtrace:")?;
    writeln!(out, "{}", report.backtrace)?;
    if let Some(log) = log_path {
        writeln!(out)?;
        writeln!(
            out,
            "log (last {REPORT_LOG_LINES} lines of {}):",
            log.display()
        )?;
        match std::fs::read_to_string(log) {
            Ok(text) => {
                let lines: Vec<&str> = text.lines().collect();
                for line in &lines[lines.len().saturating_sub(REPORT_LOG_LINES)..] {
                    writeln!(out, "{line}")?;
                }
            }
            Err(e) => writeln!(out, "<cannot read the log: {e}>")?,
        }
    }
    out.flush()?;
    drop(out);
    prune_reports(crash_dir, REPORTS_KEPT);
    Ok(path)
}

/// Removes all but the newest `keep` reports in `dir`, and as many minidumps
/// (a native crash's `.dmp`, see `native_crash`) -- the dumps are the large
/// files, so they are the ones that must not pile up. Newest by name, which
/// starts with the UTC time and so sorts in time order -- modification times
/// can be touched by a copy or a backup tool, names cannot.
pub fn prune_reports(dir: &Path, keep: usize) {
    for extension in [".txt", ".dmp"] {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let mut reports: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("crash-") && n.ends_with(extension))
            })
            .collect();
        reports.sort();
        let excess = reports.len().saturating_sub(keep);
        for old in &reports[..excess] {
            let _ = std::fs::remove_file(old);
        }
    }
}

/// `YYYYMMDD-HHMMSS` in UTC. Written out rather than pulled from a date
/// crate: it is one conversion (Howard Hinnant's days-to-civil) and a crash
/// handler is the last place to want another dependency's failure modes.
pub fn utc_stamp(t: SystemTime) -> String {
    let secs = t
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bsengine_crash_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_override_wins_and_the_name_is_made_path_safe() {
        assert_eq!(
            user_data_dir_from(Some("/tmp/x"), Some(PathBuf::from("/base")), "Game"),
            PathBuf::from("/tmp/x")
        );
        assert_eq!(
            user_data_dir_from(None, Some(PathBuf::from("/base")), "Mini: Arena?"),
            PathBuf::from("/base").join("Mini_ Arena_")
        );
        assert_eq!(
            user_data_dir_from(Some(""), Some(PathBuf::from("/base")), "Game"),
            PathBuf::from("/base").join("Game"),
            "an empty override is no override"
        );
        for escape in ["../..", "..", "a/../b", "C:\\x"] {
            let name = folder_name(escape);
            assert!(
                !name.contains(['/', '\\', ':']) && name != ".." && name != ".",
                "{escape:?} became {name:?}, which could leave the base directory"
            );
        }
        assert_eq!(folder_name("  "), "BSEngine Game");
    }

    #[test]
    fn utc_stamps_are_calendar_dates() {
        assert_eq!(utc_stamp(SystemTime::UNIX_EPOCH), "19700101-000000");
        // 2000-02-29 12:34:56 UTC: a leap day in a century leap year.
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(951_827_696);
        assert_eq!(utc_stamp(t), "20000229-123456");
        // 2026-09-30 23:59:59 UTC.
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(1_790_812_799);
        assert_eq!(utc_stamp(t), "20260930-235959");
    }

    /// The report carries every field, and the *end* of the log: the first
    /// lines of a long log are cut, the last are kept.
    #[test]
    fn a_report_has_the_panic_and_the_end_of_the_log() {
        let dir = temp_dir("report");
        let log = dir.join("game.log");
        let text: String = (0..REPORT_LOG_LINES + 50)
            .map(|i| format!("log line {i}\n"))
            .collect();
        std::fs::write(&log, text).unwrap();
        let report = CrashReport {
            context: "Probe (BSEngine 0.0.0)",
            thread: "main",
            message: "boom",
            location: "src/game.rs:1:2",
            backtrace: "  0: frame_zero",
        };
        let path = write_crash_report(
            &dir.join("crashes"),
            Some(&log),
            &report,
            SystemTime::UNIX_EPOCH,
        )
        .unwrap();
        let written = std::fs::read_to_string(&path).unwrap();
        for expected in [
            "project:  Probe (BSEngine 0.0.0)",
            "thread:   main",
            "message:  boom",
            "location: src/game.rs:1:2",
            "  0: frame_zero",
            &format!("log line {}", REPORT_LOG_LINES + 49),
        ] {
            assert!(
                written.contains(expected),
                "missing {expected:?} in:\n{written}"
            );
        }
        assert!(
            !written.contains("log line 49\n"),
            "only the last {REPORT_LOG_LINES} log lines are copied"
        );
        assert!(written.contains("log line 50\n"), "the first line kept");
        assert!(path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("crash-19700101-000000-"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Two reports in the same second of the same process get two files.
    #[test]
    fn reports_in_the_same_second_do_not_overwrite_each_other() {
        let dir = temp_dir("same_second");
        let report = CrashReport {
            context: "c",
            thread: "t",
            message: "m",
            location: "l",
            backtrace: "b",
        };
        let a = write_crash_report(&dir, None, &report, SystemTime::UNIX_EPOCH).unwrap();
        let b = write_crash_report(&dir, None, &report, SystemTime::UNIX_EPOCH).unwrap();
        assert_ne!(a, b);
        assert!(a.exists() && b.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Past [`REPORTS_KEPT`], the oldest go -- and only reports: anything
    /// else a player left in the folder stays.
    #[test]
    fn old_reports_are_pruned_and_nothing_else_is() {
        let dir = temp_dir("prune");
        for i in 0..REPORTS_KEPT + 5 {
            std::fs::write(dir.join(format!("crash-2026{i:04}-000000-1.txt")), "x").unwrap();
        }
        std::fs::write(dir.join("notes.txt"), "mine").unwrap();
        prune_reports(&dir, REPORTS_KEPT);
        let mut left: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(left.len(), REPORTS_KEPT + 1);
        assert!(left.contains(&"notes.txt".to_string()));
        assert!(
            !left.contains(&"crash-20260004-000000-1.txt".to_string()),
            "oldest gone"
        );
        assert!(
            left.contains(&"crash-20260005-000000-1.txt".to_string()),
            "newest kept"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Native crashes' minidumps are pruned as their reports are -- they are
    /// the large files -- each kind to [`REPORTS_KEPT`] on its own.
    #[test]
    fn old_minidumps_are_pruned_too() {
        let dir = temp_dir("prune_dmp");
        for i in 0..REPORTS_KEPT + 3 {
            std::fs::write(dir.join(format!("crash-2026{i:04}-000000-1.dmp")), "MDMP").unwrap();
            std::fs::write(dir.join(format!("crash-2026{i:04}-000000-1.txt")), "x").unwrap();
        }
        prune_reports(&dir, REPORTS_KEPT);
        let left: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        let count = |ext: &str| left.iter().filter(|n| n.ends_with(ext)).count();
        assert_eq!(count(".dmp"), REPORTS_KEPT, "{left:?}");
        assert_eq!(count(".txt"), REPORTS_KEPT, "{left:?}");
        assert!(
            !left.contains(&"crash-20260000-000000-1.dmp".to_string()),
            "the oldest dump went"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
