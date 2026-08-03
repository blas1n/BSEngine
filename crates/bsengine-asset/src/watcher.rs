//! Filesystem watching for asset hot reload.
//!
//! Currently this module holds only the measurement the watcher's design rests
//! on: what path spelling `notify` actually hands back for a changed file. See
//! the test below.

#[cfg(test)]
mod tests {
    use notify_debouncer_full::{
        new_debouncer,
        notify::{RecursiveMode, Watcher},
        DebounceEventResult, DebouncedEvent,
    };
    use std::path::{Path, PathBuf};
    use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
    use std::time::{Duration, Instant};

    /// Debounce window. Short enough to keep the test quick, long enough to
    /// swallow the several OS notifications one save produces.
    const DEBOUNCE: Duration = Duration::from_millis(200);
    /// Hard ceiling on every wait in this test. A hung test in CI is far worse
    /// than a failing one, so nothing here ever blocks unbounded.
    const HARD_TIMEOUT: Duration = Duration::from_secs(10);

    /// Removes its directory on drop, including when the test panics.
    struct ProbeDir(PathBuf);

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

    fn unique(tag: &str) -> String {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        format!(
            "bsengine-watch-probe-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        )
    }

    /// The asset path every probe touches, relative to the watch root.
    fn nested() -> PathBuf {
        Path::new("assets").join("models").join("thing.txt")
    }

    /// Creates `<root>/assets/models/thing.txt` and returns the cleanup guard.
    fn make_tree(root: PathBuf) -> ProbeDir {
        std::fs::create_dir_all(root.join("assets").join("models")).unwrap();
        std::fs::write(root.join(nested()), b"before").unwrap();
        ProbeDir(root)
    }

    /// Collects debounced batches until nothing has arrived for `quiet`, or
    /// until `HARD_TIMEOUT` — whichever comes first.
    fn collect(rx: &Receiver<DebounceEventResult>, quiet: Duration) -> Vec<DebouncedEvent> {
        let deadline = Instant::now() + HARD_TIMEOUT;
        let mut out = Vec::new();
        let mut idle_since = Instant::now();
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(25)) {
                Ok(Ok(events)) => {
                    out.extend(events);
                    idle_since = Instant::now();
                }
                Ok(Err(errors)) => panic!("watcher reported errors: {errors:?}"),
                Err(RecvTimeoutError::Timeout) => {
                    if !out.is_empty() && idle_since.elapsed() >= quiet {
                        return out;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        out
    }

    /// Watches `watch_root` recursively, writes the nested file `writes` times
    /// back to back, and returns everything the debouncer emitted.
    fn probe(watch_root: &Path, writes: usize) -> Vec<DebouncedEvent> {
        let (tx, rx) = mpsc::channel();
        let mut debouncer = new_debouncer(DEBOUNCE, None, tx).unwrap();
        debouncer
            .watcher()
            .watch(watch_root, RecursiveMode::Recursive)
            .unwrap();
        debouncer
            .cache()
            .add_root(watch_root, RecursiveMode::Recursive);

        // Let the backend settle, then discard anything setup stirred up, so
        // the collected events can only be the writes below.
        std::thread::sleep(DEBOUNCE * 3);
        while rx.try_recv().is_ok() {}

        let file = watch_root.join(nested());
        for i in 0..writes {
            std::fs::write(&file, format!("after {i}").as_bytes()).unwrap();
        }
        collect(&rx, DEBOUNCE * 3)
    }

    /// Asserts the facts that hold for every watch-root spelling and returns
    /// the reported path, so each caller can additionally pin what is specific
    /// to its spelling.
    fn assert_common(label: &str, watch_root: &Path, events: &[DebouncedEvent]) -> PathBuf {
        assert!(
            !events.is_empty(),
            "[{label}] no event arrived within {HARD_TIMEOUT:?} for a write under \
             {}; either the backend never started watching or the debouncer \
             dropped the notification",
            watch_root.display()
        );

        let mut paths: Vec<&PathBuf> = events.iter().flat_map(|e| e.event.paths.iter()).collect();
        paths.sort();
        paths.dedup();
        assert_eq!(
            paths.len(),
            1,
            "[{label}] one changed file must name exactly one path, got {paths:?}"
        );
        let reported = paths[0].clone();

        assert!(
            reported.is_absolute(),
            "[{label}] the event path {} is not absolute -- if this ever \
             changes, the reconstruction below must change with it",
            reported.display()
        );

        // The whole reconstruction recipe, in one line: notify absolutises the
        // watch root against the process CWD and appends the OS-relative
        // remainder, so stripping `current_dir().join(watch_root)` recovers the
        // asset's path relative to the assets directory.
        let absolutised = std::env::current_dir().unwrap().join(watch_root);
        assert_eq!(
            reported.strip_prefix(&absolutised).ok(),
            Some(nested().as_path()),
            "[{label}] stripping {} from {} must yield the nested asset path; \
             this is exactly how the watcher will rebuild the engine-form path",
            absolutised.display(),
            reported.display()
        );

        reported
    }

    // A file watcher has to hand `AssetServer::reload` a path spelled the way
    // the asset was loaded, because a mismatch is a *silent* no-op (see
    // `reload_tolerates_separator_style_but_not_a_canonicalised_path` in
    // bsengine-gltf). Assets are loaded in the engine's form: `ProjectDir`
    // joined with a scene-relative path using forward slashes, *relative to the
    // process CWD* -- e.g. `games/mini-arena/assets/models/fox.glb`. This pins
    // what notify actually reports, since that is the input the reconstruction
    // has to work from.
    //
    // Measured on Windows, one write to `<root>/assets/models/thing.txt`:
    //
    //   watch root passed to watch()   reported path
    //   ----------------------------   -------------
    //   C:\...\Temp\probe-abs (abs)    C:\...\Temp\probe-abs\assets\models\thing.txt
    //   probe-rel (relative)           <CWD>\probe-rel\assets\models\thing.txt
    //   ../../target/probe (relative)  <CWD>\../../target/probe\assets\models\thing.txt
    //
    // Three facts follow, and all three are asserted below:
    //
    //   1. The reported path is *always absolute*, even when watch() was given
    //      a relative root. The engine's form is relative to the CWD, so
    //      forwarding the event path verbatim would silently reload nothing.
    //      Reconstruction is required.
    //   2. notify does not normalise: it is exactly
    //      `current_dir().join(watch_root)` with the OS-relative remainder
    //      appended. The `..` segments and forward slashes of the third row
    //      survive untouched, so `strip_prefix(current_dir().join(watch_root))`
    //      recovers `assets\models\thing.txt` in every row -- nested files
    //      included.
    //   3. It is *not* the canonicalised spelling -- no `\\?\` verbatim prefix.
    //      That matters because the canonicalised spelling is the one bevy
    //      refuses to match.
    //
    // If row 2 ever stops holding after a notify upgrade, this test fails and
    // the reconstruction in the watcher must be re-derived from whatever the
    // new measurement says.
    #[test]
    fn notify_reports_cwd_absolutised_paths_even_for_a_relative_watch_root() {
        // Row 1: absolute watch root, outside the source tree.
        let abs_root = std::env::temp_dir().join(unique("abs"));
        let _abs_guard = make_tree(abs_root.clone());
        let abs_events = probe(&abs_root, 1);
        let abs_reported = assert_common("abs", &abs_root, &abs_events);
        assert_eq!(
            abs_reported,
            abs_root.join(nested()),
            "an absolute watch root must come back verbatim, not re-spelled"
        );

        // Row 2: relative watch root, no `..`, directly under the CWD. This is
        // the shape the engine actually uses (`<ProjectDir>/assets`).
        let rel_root = PathBuf::from(unique("rel"));
        let _rel_guard = make_tree(rel_root.clone());
        let rel_events = probe(&rel_root, 1);
        let rel_reported = assert_common("rel", &rel_root, &rel_events);
        assert!(
            !rel_reported.starts_with(&rel_root),
            "a relative watch root does NOT come back relative -- it came back \
             as {}, which is why the watcher cannot forward it to \
             AssetServer::reload unchanged",
            rel_reported.display()
        );

        // Row 3: relative watch root spelled with `..` and forward slashes, to
        // show notify performs no normalisation whatsoever.
        let odd_root = PathBuf::from(format!("../../target/{}", unique("odd")));
        let _odd_guard = make_tree(odd_root.clone());
        let odd_events = probe(&odd_root, 1);
        assert_common("odd", &odd_root, &odd_events);

        // Not the canonicalised spelling: `fs::canonicalize` on Windows returns
        // a `\\?\`-prefixed path, and that is precisely the spelling
        // AssetServer::reload silently ignores.
        #[cfg(windows)]
        {
            assert!(
                !abs_reported.to_string_lossy().starts_with(r"\\?\"),
                "notify reported a verbatim-prefixed path ({}); bevy will not \
                 match that spelling",
                abs_reported.display()
            );
            // One `fs::write` collapses to exactly one debounced event here.
            // Left platform-specific on purpose: inotify and FSEvents split a
            // write into data and metadata notifications that the debouncer
            // reports separately, so only the distinct-path count above is
            // portable.
            assert_eq!(
                abs_events.len(),
                1,
                "one write produced {} debounced events: {:?}",
                abs_events.len(),
                abs_events.iter().map(|e| e.event.kind).collect::<Vec<_>>()
            );
        }
    }

    // A save is rarely one write -- editors truncate, write, and flush, and glTF
    // exporters rewrite a file several times. This pins that the 200ms window
    // actually coalesces a burst instead of firing a reload per write.
    //
    // Measured on Windows: five back-to-back `fs::write` calls collapse to a
    // single `Modify(Any)` event. The assertion is deliberately weaker than
    // "exactly one" -- the exact count is a backend detail (inotify and
    // FSEvents split a write into separate data and metadata notifications) and
    // a burst that straddles the window legitimately produces two. What must
    // never regress is that N writes stop producing N reloads.
    #[test]
    fn a_burst_of_writes_coalesces_into_fewer_events_than_writes() {
        const WRITES: usize = 5;

        let root = std::env::temp_dir().join(unique("burst"));
        let _guard = make_tree(root.clone());
        let events = probe(&root, WRITES);
        let reported = assert_common("burst", &root, &events);
        assert_eq!(reported, root.join(nested()));

        assert!(
            events.len() < WRITES,
            "{WRITES} back-to-back writes produced {} debounced events, so the \
             {DEBOUNCE:?} window is not coalescing: {:?}",
            events.len(),
            events.iter().map(|e| e.event.kind).collect::<Vec<_>>()
        );
    }
}
