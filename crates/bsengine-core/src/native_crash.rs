//! Native crashes: a minidump and a report when the process dies without
//! panicking -- a segfault in a GPU driver, a null pointer behind FFI, an
//! illegal instruction.
//!
//! [`crate::crash`] catches panics, which unwind through Rust and reach a
//! hook. A native crash does neither: it is a signal (Unix) or a structured
//! exception (Windows), and the process is in no state to be trusted. Unity
//! (UnityCrashHandler), Unreal (CrashReportClient) and Crashpad all answer it
//! the same way, and this is that shape:
//!
//! - The game, at start-up, launches a **second copy of its own executable**
//!   as a monitor ([`MONITOR_ARG`]) and connects to it.
//! - A signal/SEH handler in the game does as little as it can: it tells the
//!   monitor which exception it was, asks it for a dump, and waits.
//! - The monitor -- a healthy process -- reads the crashed one's memory and
//!   writes a minidump (`crash-<UTC>-<pid>.dmp`) and, beside it, the same
//!   text report a panic writes (`.txt`: the exception, the dump's name, the
//!   log's tail), into the crash directory the panic reports use.
//!
//! The dump is written from outside because a crashed process cannot be
//! relied on to write it -- its heap may be the thing that broke -- and
//! because on Linux it cannot: a minidump of the threads is taken with
//! `ptrace`, which a process cannot do to itself.
//!
//! The monitor exits when the game does, crash or not, and if it cannot be
//! started the game runs anyway with panic reports only: crash capture is
//! never a reason a game does not start.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

/// The argument that makes a BSEngine executable the crash monitor:
/// `<exe> --crash-monitor <socket> <crash dir> [<log file>]`. An executable
/// that calls [`attach`] must check for it first ([`run_monitor_if_asked`]),
/// since [`attach`] starts the monitor by running itself.
pub const MONITOR_ARG: &str = "--crash-monitor";

/// Message kinds the game sends the monitor.
const MSG_CONTEXT: u32 = 1;
const MSG_EXCEPTION: u32 = 2;

/// How long the game waits for the monitor it started to accept a
/// connection before running without one.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// When this process was started as the monitor, runs it until the game it
/// watches exits, and returns `true` -- the caller then returns from `main`.
/// Otherwise returns `false` at once.
pub fn run_monitor_if_asked() -> bool {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    if args.get(1).is_none_or(|a| a != MONITOR_ARG) {
        return false;
    }
    let (Some(socket), Some(crash_dir)) = (args.get(2), args.get(3)) else {
        eprintln!("{MONITOR_ARG} <socket> <crash dir> [<log file>]");
        std::process::exit(2);
    };
    if let Err(e) = run_monitor(
        Path::new(socket),
        PathBuf::from(crash_dir),
        args.get(4).map(PathBuf::from),
    ) {
        eprintln!("crash monitor: {e}");
    }
    true
}

/// What the monitor knows about the game it watches.
#[derive(Default)]
struct Watched {
    /// The project and engine version, as the panic report names them.
    context: String,
    /// The game's process id: the dump and report are named for it, not
    /// for the monitor.
    pid: u32,
    /// The exception or signal, once the handler has sent it.
    exception: Option<u64>,
    /// The dump being written.
    dump: Option<PathBuf>,
    /// When the dump was started. The report is named for the same moment,
    /// not for when it is written -- a dump can take more than a second, and
    /// the two must sort together.
    crashed_at: Option<SystemTime>,
}

struct Monitor {
    crash_dir: PathBuf,
    log: Option<PathBuf>,
    watched: Mutex<Watched>,
}

impl minidumper::ServerHandler for Monitor {
    fn create_minidump_file(&self) -> Result<(std::fs::File, PathBuf), std::io::Error> {
        std::fs::create_dir_all(&self.crash_dir)?;
        let mut watched = self.watched.lock().unwrap_or_else(|e| e.into_inner());
        let now = SystemTime::now();
        watched.crashed_at = Some(now);
        let stamp = crate::crash::utc_stamp(now);
        let path = self
            .crash_dir
            .join(format!("crash-{stamp}-{}.dmp", watched.pid));
        let file = std::fs::File::create(&path)?;
        watched.dump = Some(path.clone());
        Ok((file, path))
    }

    fn on_minidump_created(
        &self,
        result: Result<minidumper::MinidumpBinary, minidumper::Error>,
    ) -> minidumper::LoopAction {
        let watched = self.watched.lock().unwrap_or_else(|e| e.into_inner());
        let dump = watched
            .dump
            .as_deref()
            .and_then(Path::file_name)
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let location = match result {
            Ok(mut binary) => {
                use std::io::Write;
                let _ = binary.file.flush();
                format!("minidump {dump}, beside this report")
            }
            Err(e) => format!("no minidump: writing {dump} failed ({e})"),
        };
        let message = format!(
            "native crash: {}",
            watched
                .exception
                .map_or_else(|| "unknown exception".to_string(), describe_exception)
        );
        let report = crate::crash::CrashReport {
            context: &watched.context,
            thread: "<the crashing thread: see the minidump>",
            message: &message,
            location: &location,
            backtrace: "<native: in the minidump. Open it in Visual Studio or WinDbg, or \
                        run minidump-stackwalk on it with the build's symbols>",
        };
        if let Err(e) = crate::crash::write_crash_report_for(
            watched.pid,
            &self.crash_dir,
            self.log.as_deref(),
            &report,
            watched.crashed_at.unwrap_or_else(SystemTime::now),
        ) {
            eprintln!("crash monitor: cannot write the report: {e}");
        }
        minidumper::LoopAction::Exit
    }

    fn on_message(&self, kind: u32, buffer: Vec<u8>) {
        let mut watched = self.watched.lock().unwrap_or_else(|e| e.into_inner());
        match kind {
            MSG_CONTEXT => {
                let text = String::from_utf8_lossy(&buffer);
                if let Some((pid, context)) = text.split_once('\n') {
                    watched.pid = pid.parse().unwrap_or(0);
                    watched.context = context.to_string();
                }
            }
            MSG_EXCEPTION => {
                if let Ok(bytes) = <[u8; 8]>::try_from(buffer.as_slice()) {
                    watched.exception = Some(u64::from_le_bytes(bytes));
                }
            }
            _ => {}
        }
    }

    // The game exited -- normally, or killed without a crash to report. The
    // monitor's work is done either way.
    fn on_client_disconnected(&self, num_clients: usize) -> minidumper::LoopAction {
        if num_clients == 0 {
            minidumper::LoopAction::Exit
        } else {
            minidumper::LoopAction::Continue
        }
    }
}

/// Runs the monitor's server on `socket` until the game disconnects or a
/// dump has been written.
pub fn run_monitor(socket: &Path, crash_dir: PathBuf, log: Option<PathBuf>) -> Result<(), String> {
    let mut server = minidumper::Server::with_name(minidumper::SocketName::Path(socket))
        .map_err(|e| e.to_string())?;
    let shutdown = std::sync::atomic::AtomicBool::new(false);
    let monitor = Monitor {
        crash_dir,
        log,
        watched: Mutex::new(Watched::default()),
    };
    let result = server
        .run(Box::new(monitor), &shutdown, None)
        .map_err(|e| e.to_string());
    let _ = std::fs::remove_file(socket);
    result
}

/// What keeps native crash capture alive: the handler and the connection to
/// the monitor. [`attach`] leaks it -- it lasts as long as the process.
struct Attached {
    _handler: crash_handler::CrashHandler,
    _monitor: std::process::Child,
}

/// Starts the monitor (this executable, with [`MONITOR_ARG`]) and installs
/// the handler that asks it for a dump on a native crash. Reports go to
/// `crash_dir`, beside the panic reports, with `log`'s tail.
///
/// Returns the monitor's process id. Errors leave the game running without
/// native capture; the caller logs them.
pub fn attach(crash_dir: &Path, log: Option<&Path>, context: &str) -> Result<u32, String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot find this executable: {e}"))?;
    // Per process, in the temporary directory: two games running at once
    // each get their own monitor.
    let socket = std::env::temp_dir().join(format!("bsengine-crash-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&socket);
    let mut command = std::process::Command::new(exe);
    command.arg(MONITOR_ARG).arg(&socket).arg(crash_dir);
    if let Some(log) = log {
        command.arg(log);
    }
    // No inherited stdio: a monitor holding the game's stderr open would keep
    // whoever reads it (a launcher, a test) waiting after the game is gone.
    let mut monitor = command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("cannot start the crash monitor: {e}"))?;

    let started = std::time::Instant::now();
    let client = loop {
        if let Ok(client) = minidumper::Client::with_name(minidumper::SocketName::Path(&socket)) {
            break client;
        }
        if let Ok(Some(status)) = monitor.try_wait() {
            return Err(format!("the crash monitor exited at once ({status})"));
        }
        if started.elapsed() > CONNECT_TIMEOUT {
            let _ = monitor.kill();
            return Err("the crash monitor did not start listening".to_string());
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    client
        .send_message(MSG_CONTEXT, format!("{}\n{context}", std::process::id()))
        .map_err(|e| format!("cannot reach the crash monitor: {e}"))?;

    // SAFETY: `make_crash_event` requires the closure be safe to run in a
    // signal handler / exception filter. It allocates nothing: the exception
    // code goes as eight bytes, and the dump request is what the crate's
    // handler exists to make from there.
    #[allow(unsafe_code)]
    let handler = crash_handler::CrashHandler::attach(unsafe {
        crash_handler::make_crash_event(move |context: &crash_handler::CrashContext| {
            let _ = client.send_message(MSG_EXCEPTION, exception_code(context).to_le_bytes());
            // Delivered before the dump request: on macOS messages and the
            // crash request travel by different routes.
            let _ = client.ping();
            crash_handler::CrashEventResult::Handled(client.request_dump(context).is_ok())
        })
    })
    .map_err(|e| format!("cannot install the native crash handler: {e}"))?;
    // Ubuntu (and other Yama-enabled Linuxes) let a process be ptraced only
    // by its ancestors, unless it names the tracer; the monitor is a child.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    handler.set_ptracer(Some(monitor.id()));

    let pid = monitor.id();
    Box::leak(Box::new(Attached {
        _handler: handler,
        _monitor: monitor,
    }));
    Ok(pid)
}

/// Crashes this process the native way: a write through a null pointer, an
/// access violation on Windows and `SIGSEGV` elsewhere. For checking, on a
/// player's machine or in a test, that a crash leaves a minidump -- Unity's
/// `Utils.ForceCrash` and Unreal's `debug crash` exist for the same reason.
pub fn force_crash() -> ! {
    // SAFETY: none, deliberately: this is the crash. Volatile, so the write
    // is not optimised out as undefined behaviour the compiler may assume
    // away.
    #[allow(unsafe_code)]
    unsafe {
        std::ptr::null_mut::<u32>().write_volatile(0xDEAD);
    }
    // Unreachable unless address 0 is mapped, which no supported OS does.
    std::process::abort()
}

/// The exception or signal, as one number the monitor can name.
fn exception_code(context: &crash_handler::CrashContext) -> u64 {
    #[cfg(windows)]
    {
        u64::from(context.exception_code as u32)
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        u64::from(context.siginfo.ssi_signo)
    }
    #[cfg(target_os = "macos")]
    {
        context.exception.as_ref().map_or(0, |e| u64::from(e.kind))
    }
}

/// A name for an [`exception_code`], with the number, for the report.
pub fn describe_exception(code: u64) -> String {
    let name = if cfg!(windows) {
        match code as u32 {
            0xC000_0005 => "EXCEPTION_ACCESS_VIOLATION",
            0xC000_001D => "EXCEPTION_ILLEGAL_INSTRUCTION",
            0xC000_0094 => "EXCEPTION_INT_DIVIDE_BY_ZERO",
            0xC000_00FD => "EXCEPTION_STACK_OVERFLOW",
            0xC000_0409 => "STATUS_STACK_BUFFER_OVERRUN (fail fast)",
            0x8000_0003 => "EXCEPTION_BREAKPOINT",
            0xC000_0006 => "EXCEPTION_IN_PAGE_ERROR",
            _ => "",
        }
    } else if cfg!(target_os = "macos") {
        // Mach exception kinds (the signal comes later, from them).
        match code {
            1 => "EXC_BAD_ACCESS",
            2 => "EXC_BAD_INSTRUCTION",
            3 => "EXC_ARITHMETIC",
            6 => "EXC_BREAKPOINT",
            10 => "EXC_CRASH",
            _ => "",
        }
    } else {
        match code {
            4 => "SIGILL",
            5 => "SIGTRAP",
            6 => "SIGABRT",
            7 => "SIGBUS",
            8 => "SIGFPE",
            11 => "SIGSEGV",
            _ => "",
        }
    };
    if cfg!(windows) {
        format!("{name} (0x{code:08X})").trim_start().to_string()
    } else {
        format!("{name} ({code})").trim_start().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The names the report gives the usual crashes, for this platform.
    #[test]
    fn the_usual_crashes_have_names() {
        if cfg!(windows) {
            assert_eq!(
                describe_exception(0xC000_0005),
                "EXCEPTION_ACCESS_VIOLATION (0xC0000005)"
            );
        } else if cfg!(target_os = "macos") {
            assert_eq!(describe_exception(1), "EXC_BAD_ACCESS (1)");
        } else {
            assert_eq!(describe_exception(11), "SIGSEGV (11)");
        }
        // An unknown one is still reported, by number.
        let unknown = describe_exception(0x1234);
        assert!(
            unknown.contains(if cfg!(windows) { "0x00001234" } else { "4660" }),
            "{unknown}"
        );
    }
}
