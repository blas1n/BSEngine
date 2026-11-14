use std::sync::OnceLock;

static LOGGING_INIT: OnceLock<()> = OnceLock::new();

/// Initializes the global `tracing` subscriber for the engine, writing to
/// stderr with a `bsengine=debug,warn` default filter (overridable via the
/// standard `RUST_LOG` env var). Safe to call more than once; only the first
/// call takes effect.
pub fn init_logging() {
    LOGGING_INIT.get_or_init(|| {
        // A browser build has no stderr: its log is the page's console.
        #[cfg(target_arch = "wasm32")]
        tracing_wasm::set_as_global_default();
        // Nor does an Android app: its stderr goes nowhere, and its log is
        // logcat (`adb logcat -s bsengine`).
        #[cfg(target_os = "android")]
        {
            use tracing_subscriber::{fmt, EnvFilter};
            fmt()
                .with_env_filter(EnvFilter::new("bsengine=debug,warn"))
                .with_ansi(false)
                .without_time()
                .with_writer(logcat::Writer::default)
                .init();
        }
        #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
        {
            use tracing_subscriber::{fmt, EnvFilter};
            fmt()
                .with_env_filter(
                    EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| EnvFilter::new("bsengine=debug,warn")),
                )
                .with_writer(std::io::stderr)
                .init();
        }
    });
}

/// [`init_logging`], plus a copy of every line in the file at `log_path` --
/// what a shipped game, which has no console, leaves behind. Plain text in
/// the file (no colour codes); stderr keeps its colours.
///
/// A log already at `log_path` is kept as `<stem>-prev.<ext>`, replacing the
/// one before it: this run's log and the last run's, as Unity keeps
/// `Player.log` and `Player-prev.log`. The run that crashed is usually the
/// one a player reports *after* restarting.
///
/// Returns `Ok(false)` and does nothing if logging was already set up -- the
/// first set-up wins, so this has to run before anything calls
/// [`init_logging`] (which `new_app` does).
pub fn init_logging_with_file(log_path: &std::path::Path) -> std::io::Result<bool> {
    if LOGGING_INIT.get().is_some() {
        return Ok(false);
    }
    if let Some(dir) = log_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    rotate_log(log_path)?;
    let file = std::fs::File::create(log_path)?;
    let mut installed = false;
    LOGGING_INIT.get_or_init(|| {
        use tracing_subscriber::layer::SubscriberExt;
        use tracing_subscriber::util::SubscriberInitExt;
        use tracing_subscriber::{fmt, EnvFilter};
        tracing_subscriber::registry()
            .with(
                EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| EnvFilter::new("bsengine=debug,warn")),
            )
            .with(fmt::layer().with_writer(std::io::stderr))
            .with(
                fmt::layer()
                    .with_ansi(false)
                    .with_writer(std::sync::Mutex::new(file)),
            )
            .init();
        installed = true;
    });
    Ok(installed)
}

/// Moves an existing log at `path` to `<stem>-prev.<ext>`, over any previous
/// one. Nothing to move is not an error.
pub fn rotate_log(path: &std::path::Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let prev = match path.extension() {
        Some(ext) => path.with_file_name(format!("{stem}-prev.{}", ext.to_string_lossy())),
        None => path.with_file_name(format!("{stem}-prev")),
    };
    std::fs::rename(path, prev)
}

/// Lines to logcat, tagged `bsengine`, through `liblog` -- which every
/// Android process has, so no crate is needed for one function.
#[cfg(target_os = "android")]
mod logcat {
    use std::ffi::CString;
    use std::os::raw::{c_char, c_int};

    #[link(name = "log")]
    extern "C" {
        fn __android_log_write(prio: c_int, tag: *const c_char, text: *const c_char) -> c_int;
    }

    /// `ANDROID_LOG_INFO`: the level is already in the formatted line, so
    /// every line goes at one priority that logcat shows by default.
    const INFO: c_int = 4;

    /// One `tracing` event's text, written to logcat when it ends: the
    /// formatter writes an event in pieces, and each logcat write is an entry.
    #[derive(Default)]
    pub(super) struct Writer(Vec<u8>);

    impl std::io::Write for Writer {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Drop for Writer {
        fn drop(&mut self) {
            let text = String::from_utf8_lossy(&self.0);
            let text = text.trim_end();
            if text.is_empty() {
                return;
            }
            // An interior NUL would end the entry early; none is expected.
            let text = CString::new(text.replace('\0', " ")).unwrap_or_default();
            // SAFETY: both pointers are NUL-terminated strings that outlive
            // the call.
            unsafe { __android_log_write(INFO, c"bsengine".as_ptr(), text.as_ptr()) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This run's log replaces nothing it should keep: the last run's moves
    /// to `-prev`, and the one before that is gone.
    #[test]
    fn rotating_keeps_exactly_the_previous_run() {
        let dir = std::env::temp_dir().join(format!("bsengine_rotate_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("game.log");
        let prev = dir.join("game-prev.log");

        rotate_log(&log).unwrap();
        assert!(!prev.exists(), "nothing to rotate on a first run");

        std::fs::write(&log, "run 1").unwrap();
        rotate_log(&log).unwrap();
        std::fs::write(&log, "run 2").unwrap();
        rotate_log(&log).unwrap();
        assert!(!log.exists(), "the current log was moved aside");
        assert_eq!(std::fs::read_to_string(&prev).unwrap(), "run 2");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn init_logging_can_be_called_multiple_times() {
        // OnceLock ensures second call is a no-op, not a panic
        init_logging();
        init_logging();
    }

    #[test]
    fn logging_macros_work_after_init() {
        init_logging();
        tracing::info!("bsengine-core logging test");
        tracing::debug!("debug message");
        tracing::warn!("warn message");
        // If we reach here without panic, logging is working
    }
}
