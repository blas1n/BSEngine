//! Probe helpers shared by this crate's tests.
//!
//! Both things this crate does to the filesystem — watching `<ProjectDir>/assets`
//! for changes, and walking it to hand out identities — can only be tested
//! against a real directory tree, and both need the same two awkward pieces:
//! a temp directory that really goes away afterwards, and a way to assert that
//! a diagnostic reached the developer rather than merely that the state behind
//! it changed.
//!
//! These started in `watcher`'s test module and were lifted here when the
//! identity scan needed them too. Copying them would have meant two Drop
//! guards to keep working on Windows and two probe-name conventions for
//! `.gitignore` to cover.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Removes its directory on drop, including when the test panics.
pub(crate) struct ProbeDir(pub(crate) PathBuf);

impl Drop for ProbeDir {
    fn drop(&mut self) {
        // Windows can briefly refuse the removal while the watcher's
        // handles are being torn down; retry rather than leave litter in
        // the source tree.
        for _ in 0..20 {
            if std::fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

/// A probe name no other test — or concurrent run of the same test — collides
/// with.
///
/// The `bsengine-watch-probe-` prefix is kept for every caller, watcher or not:
/// `.gitignore` covers `crates/*/bsengine-watch-probe-*`, and the watcher tests
/// deliberately place one probe under the crate root rather than the temp
/// directory (a *relative* path is the spelling they exist to measure). A
/// second prefix would need a second ignore entry, and the run that discovered
/// it was missing would be the one that committed the litter.
pub(crate) fn unique(tag: &str) -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    format!(
        "bsengine-watch-probe-{tag}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    )
}

/// Collects everything `tracing` emits on this thread, so a test can assert a
/// diagnostic actually reached the developer rather than only that the state
/// behind it changed. That distinction is this crate's recurring subject:
/// hot reload doing nothing and an asset silently losing its identity are both
/// failures with no symptom other than the line that reports them.
///
/// Thread-local (`tracing::subscriber::with_default`), so parallel tests cannot
/// see each other's output. It captures ECS systems too, because this workspace
/// builds `bevy_ecs` without `multi_threaded` and its single-threaded executor
/// runs systems on the caller's thread.
#[derive(Clone, Default)]
pub(crate) struct LogSink(Arc<Mutex<Vec<u8>>>);

impl LogSink {
    fn contents(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

impl std::io::Write for LogSink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl tracing_subscriber::fmt::MakeWriter<'_> for LogSink {
    type Writer = Self;
    fn make_writer(&self) -> Self {
        self.clone()
    }
}

/// Runs `body` with every `WARN` on this thread captured.
///
/// Deliberately capped at `WARN`: `bsengine_app::new_app` installs a global
/// subscriber whose filter is the process-wide one, and pinning this to the
/// level that is enabled under every filter keeps these assertions independent
/// of `RUST_LOG`.
pub(crate) fn capture_warnings<T>(body: impl FnOnce() -> T) -> (T, String) {
    let sink = LogSink::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(sink.clone())
        .with_ansi(false)
        .with_max_level(tracing::Level::WARN)
        .finish();
    let out = tracing::subscriber::with_default(subscriber, body);
    let logs = sink.contents();
    (out, logs)
}
