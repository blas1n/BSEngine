//! Filesystem watching for asset hot reload, scoped to `<ProjectDir>/assets`.
//!
//! The scoping is the reason this exists at all rather than `bevy_asset`'s own
//! `file_watcher` feature: that one watches `bevy_asset`'s asset root, which
//! for this engine is the process CWD (see `plugin::asset_source_root`) — the
//! whole repository, `target/` and `.git/` included.
//!
//! # The path spelling problem
//!
//! `AssetServer::reload` matches on the path *string* the asset was loaded
//! with, and a mismatch is a **silent** no-op: no warning, no event, nothing
//! happens (`reload_tolerates_separator_style_but_not_a_canonicalised_path` in
//! `bsengine-gltf` pins exactly how much variation it tolerates — separator
//! direction yes, a canonicalised spelling no).
//!
//! Assets are loaded in the engine's own form:
//! `bsengine_core::resolve_project_path` joins `ProjectDir` with a
//! scene-relative path using forward slashes, *relative to the process CWD* —
//! e.g. `games/mini-arena/assets/models/fox.glb`.
//!
//! `notify` hands back something else entirely: an absolute path, even when
//! `watch()` was given a relative root. Measured in
//! `notify_reports_cwd_absolutised_paths_even_for_a_relative_watch_root`
//! below, it is exactly `current_dir().join(watch_root)` with the OS-relative
//! remainder appended, with no normalisation whatsoever. So the whole
//! reconstruction is one `strip_prefix` of that absolutised root, re-joined
//! onto the engine-form root — see [`reconstruct`].

use bevy_app::{App, Plugin, Startup, Update};
use bevy_asset::AssetServer;
use bevy_ecs::prelude::{Commands, Res, Resource};
use bsengine_core::ProjectDir;
use notify_debouncer_full::{
    new_debouncer,
    notify::{RecommendedWatcher, RecursiveMode, Watcher},
    DebounceEventResult, Debouncer, FileIdMap,
};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Mutex;
use std::time::Duration;
use tracing::{info, warn};

/// How long a path must stay quiet before its change is reported.
///
/// A save is rarely one write: editors truncate then write then flush, and
/// glTF exporters rewrite a file several times in a row. Reloading per raw
/// notification would both fire N reloads for one save and race the writer —
/// a loader that opens the file mid-truncate sees a half-written asset and
/// fails. 200ms is long enough to swallow those bursts (measured:
/// `a_burst_of_writes_coalesces_into_fewer_events_than_writes` collapses five
/// back-to-back writes into one event) and short enough that a save still
/// feels instantaneous to the person who made it.
const DEBOUNCE: Duration = Duration::from_millis(200);

/// Extensions `bevy_asset` can actually serve in this engine, and nothing
/// else.
///
/// `AssetServer::reload` on a path no loader ever loaded is a *silent* no-op,
/// so forwarding e.g. a `.ron` scene edit or a `.js` script edit would log a
/// reload that provably does nothing — scenes go through
/// `std::fs::read_to_string` and scripts through `bsengine-scripting`, neither
/// of which involves `bevy_asset`. Filtering keeps the "reloading X" log line
/// honest, which is the only diagnostic that makes the silent-no-op class of
/// bug findable at all.
///
/// Every entry corresponds to a loader some plugin registers:
///
/// * `glb`, `gltf` — `bsengine_gltf::GltfSourceLoader`
/// * `png`, `jpg`, `jpeg`, `hdr` — [`crate::TextureAssetLoader`] (the
///   workspace pins `image` to exactly the png+jpeg+hdr features)
/// * `wgsl` — `bsengine_render::ShaderSourceLoader`
/// * `wav`, `ogg`, `mp3`, `flac` — `bsengine_audio::AudioSourceLoader` (the
///   workspace pins `kira` to exactly those four codecs)
///
/// None of those loaders declares `AssetLoader::extensions()` — every load
/// site in the engine is type-directed (`AssetServer::load::<T>(path)`), so
/// there is no registry to interrogate and this list is maintained by hand.
/// Adding a loader without adding its extension here costs a hot reload, not
/// correctness.
const RELOADABLE_EXTENSIONS: &[&str] = &[
    "glb", "gltf", "png", "jpg", "jpeg", "hdr", "wgsl", "wav", "ogg", "mp3", "flac",
];

/// Watches `<ProjectDir>/assets` and asks `AssetServer` to reload any asset
/// file that changes on disk, so an edit made while the game is running takes
/// effect without a restart.
///
/// Requires [`crate::plugin::AssetPlugin`] (for `AssetServer`) and a
/// `ProjectDir` resource inserted before `Startup` runs. With no `ProjectDir`,
/// an empty one (the editor's default) or a missing `assets` directory, the
/// plugin logs once at info and starts no watcher — it never falls back to
/// watching the CWD.
///
/// The rebuilding half of hot reload is not here: every asset consumer already
/// rebuilds its GPU state from `AssetEvent::Modified`. This plugin only
/// answers "which paths changed".
pub struct AssetWatcherPlugin;

impl Plugin for AssetWatcherPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, start_asset_watcher)
            .add_systems(Update, drain_asset_changes);
    }
}

/// Live watch state. Absent when watching is disabled — see
/// [`AssetWatcherPlugin`].
#[derive(Resource)]
struct AssetWatcher {
    /// Held purely for its `Drop`, which stops the watch: dropping the
    /// resource (i.e. dropping the `World` at app exit) sets the debouncer
    /// thread's stop flag and closes the OS watch handle.
    ///
    /// The `Mutex` is a `Send + Sync` shim, not synchronisation — nothing ever
    /// locks it. `notify`'s platform watchers are `Send` but not `Sync`, and a
    /// bevy `Resource` must be both.
    _debouncer: Mutex<Debouncer<RecommendedWatcher, FileIdMap>>,
    /// Receiving end of the watcher thread's channel. Same `Mutex` rationale:
    /// `Receiver` is `Send` but not `Sync`. Only ever `try_recv`'d, never
    /// `recv`'d, so the ECS never blocks on the watcher thread.
    events: Mutex<Receiver<DebounceEventResult>>,
    /// `current_dir().join(<ProjectDir>/assets)` — the prefix every path
    /// `notify` reports carries, and therefore the prefix [`reconstruct`]
    /// strips.
    strip_base: PathBuf,
    /// `<ProjectDir>/assets` in the engine's own spelling, which is what the
    /// reconstructed paths are re-joined onto.
    engine_root: String,
}

/// Starts the watcher, or explains once why it is not starting.
fn start_asset_watcher(mut commands: Commands, project_dir: Option<Res<ProjectDir>>) {
    let Some(project_dir) = project_dir.map(|p| p.0.clone()).filter(|p| !p.is_empty()) else {
        info!("asset hot reload: no project directory set, not watching");
        return;
    };

    // Engine form, matching resolve_project_path: forward slashes, relative to
    // the process CWD unless ProjectDir is itself absolute.
    let engine_root = format!("{project_dir}/assets");
    let watch_root = PathBuf::from(&engine_root);
    if !watch_root.is_dir() {
        info!("asset hot reload: {engine_root} does not exist, not watching");
        return;
    }

    // Deliberately NOT canonicalize(): notify reports the CWD-absolutised
    // spelling, never the canonical one, and on Windows canonicalize() returns
    // a `\\?\` path that would never strip.
    let strip_base = match std::env::current_dir() {
        Ok(cwd) => cwd.join(&watch_root),
        Err(e) => {
            warn!("asset hot reload: cannot read the working directory ({e}), not watching");
            return;
        }
    };

    let (tx, rx) = mpsc::channel();
    let mut debouncer = match new_debouncer(DEBOUNCE, None, tx) {
        Ok(d) => d,
        Err(e) => {
            warn!("asset hot reload: cannot start the file watcher ({e}), not watching");
            return;
        }
    };
    if let Err(e) = debouncer
        .watcher()
        .watch(&watch_root, RecursiveMode::Recursive)
    {
        warn!("asset hot reload: cannot watch {engine_root} ({e}), not watching");
        return;
    }
    debouncer
        .cache()
        .add_root(&watch_root, RecursiveMode::Recursive);

    info!("asset hot reload: watching {engine_root}");
    commands.insert_resource(AssetWatcher {
        _debouncer: Mutex::new(debouncer),
        events: Mutex::new(rx),
        strip_base,
        engine_root,
    });
}

/// Rebuilds the spelling `changed` was *loaded* with, from the spelling
/// `notify` *reported* it with.
///
/// `strip_base` is `current_dir().join(<ProjectDir>/assets)` and `engine_root`
/// is `<ProjectDir>/assets`; see the module docs for why that pair is the
/// whole transformation.
///
/// Returns `None` — meaning "do not reload this" — when the path is outside
/// the watch root, has no extension, or has an extension no registered loader
/// can serve. Existence is checked by the caller, not here, so this stays a
/// pure function of its arguments.
fn reconstruct(changed: &Path, strip_base: &Path, engine_root: &str) -> Option<String> {
    let relative = changed.strip_prefix(strip_base).ok()?;

    let extension = changed.extension()?.to_str()?.to_ascii_lowercase();
    if !RELOADABLE_EXTENSIONS.contains(&extension.as_str()) {
        return None;
    }

    // Forward slashes to match resolve_project_path exactly. bevy tolerates
    // either direction, but an identical string keeps the logs readable and
    // keeps this honest about what it is reproducing.
    let relative = relative.to_str()?.replace('\\', "/");
    Some(format!("{engine_root}/{relative}"))
}

/// Drains everything the watcher thread has posted and reloads it. Never
/// waits: on a frame where nothing changed this is one uncontended lock and
/// one `try_recv` that returns `Empty`.
fn drain_asset_changes(watcher: Option<Res<AssetWatcher>>, asset_server: Res<AssetServer>) {
    let Some(watcher) = watcher else {
        return;
    };

    let mut changed: Vec<String> = Vec::new();
    {
        let events = match watcher.events.lock() {
            Ok(events) => events,
            // Only reachable if a previous drain panicked mid-lock. Nothing
            // useful to do but stop watching rather than poison every frame.
            Err(_) => return,
        };
        loop {
            match events.try_recv() {
                Ok(Ok(batch)) => {
                    for path in batch.iter().flat_map(|e| e.event.paths.iter()) {
                        let Some(engine_path) =
                            reconstruct(path, &watcher.strip_base, &watcher.engine_root)
                        else {
                            continue;
                        };
                        // A deleted file must not be reloaded: the load would
                        // fail and leave the handle permanently `Failed`,
                        // where leaving it alone keeps the last good version
                        // resident. A rename-style atomic save still reloads,
                        // because its destination does exist by now.
                        if !path.is_file() {
                            continue;
                        }
                        if !changed.iter().any(|c| c == &engine_path) {
                            changed.push(engine_path);
                        }
                    }
                }
                Ok(Err(errors)) => warn!("asset hot reload: watcher error: {errors:?}"),
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }
    }

    for path in changed {
        info!("asset hot reload: {path} changed on disk, reloading");
        asset_server.reload(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify_debouncer_full::DebouncedEvent;
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::Instant;

    /// Hard ceiling on every wait in this module's tests. A hung test in CI is
    /// far worse than a failing one, so nothing here ever blocks unbounded.
    const HARD_TIMEOUT: Duration = Duration::from_secs(20);

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

    // ---- the plugin itself ------------------------------------------------

    /// A valid 1x1 PNG with the given pixel, encoded in memory so the same
    /// bytes can be written under any filename (the negative half of the
    /// hot-reload test needs valid image bytes stored as `.ron`).
    fn png_bytes(rgba: [u8; 4]) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(1, 1, image::Rgba(rgba));
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    /// Runs frames until `done`, or panics with `what` after `HARD_TIMEOUT`.
    /// Bounded by wall clock rather than a frame count because what is being
    /// waited on is a filesystem notification, not a fixed amount of work.
    fn run_until(app: &mut App, what: &str, mut done: impl FnMut(&mut App) -> bool) {
        let deadline = Instant::now() + HARD_TIMEOUT;
        while Instant::now() < deadline {
            app.update();
            if done(app) {
                return;
            }
            // Yield instead of spinning: the thing being waited on happens on
            // another thread, on the OS's schedule.
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("{what} did not happen within {HARD_TIMEOUT:?}");
    }

    // The end-to-end property, driven through the real plugin rather than a
    // hand-rolled imitation of it: an asset saved under `<ProjectDir>/assets`
    // while the app is running produces `AssetEvent::Modified` for the handle
    // it was loaded with, and the reloaded bytes are the new ones.
    //
    // Two things make this test able to fail for the right reasons:
    //
    //  * `ProjectDir` is *relative*, which is the shape the engine really uses.
    //    With an absolute one, the path notify reports happens to equal the
    //    engine form, and forwarding it unchanged would pass -- the
    //    reconstruction would be untested.
    //  * A second file holds the *same valid PNG bytes* under a `.ron` name and
    //    is loaded as a texture too. `AssetServer::load::<T>` is type-directed,
    //    not extension-directed, so bevy serves it happily; the only reason it
    //    must not be reloaded is this module's extension filter. That makes the
    //    filter observable end-to-end instead of only by unit test: without it,
    //    the `.ron` handle gets a `Modified` too.
    #[test]
    fn a_saved_asset_is_reloaded_and_a_non_asset_file_is_not() {
        use crate::types::TextureAsset;
        use bevy_asset::{AssetEvent, Assets};
        use bevy_ecs::event::{Events, ManualEventReader};
        use bsengine_app::new_app;

        const BEFORE: [u8; 4] = [10, 20, 30, 255];
        const AFTER: [u8; 4] = [200, 100, 50, 255];

        // A project directory relative to the CWD (which cargo sets to the
        // crate root), spelled with forward slashes exactly as
        // `resolve_project_path` would. `.gitignore` covers the name.
        let project = unique("hot");
        let root = PathBuf::from(&project);
        std::fs::create_dir_all(root.join("assets")).unwrap();
        let _guard = ProbeDir(root.clone());

        let texture = root.join("assets").join("tex.png");
        let decoy = root.join("assets").join("notes.ron");
        std::fs::write(&texture, png_bytes(BEFORE)).unwrap();
        std::fs::write(&decoy, png_bytes(BEFORE)).unwrap();

        let mut app = new_app();
        app.insert_resource(ProjectDir(project.clone()));
        app.add_plugins(crate::plugin::AssetPlugin);
        app.add_plugins(AssetWatcherPlugin);

        let (texture_handle, decoy_handle) = {
            let server = app.world().resource::<AssetServer>();
            (
                server.load::<TextureAsset>(format!("{project}/assets/tex.png")),
                server.load::<TextureAsset>(format!("{project}/assets/notes.ron")),
            )
        };

        run_until(&mut app, "both assets finished loading", |app| {
            let assets = app.world().resource::<Assets<TextureAsset>>();
            assets.get(&texture_handle).is_some() && assets.get(&decoy_handle).is_some()
        });
        assert_eq!(
            app.world()
                .resource::<Assets<TextureAsset>>()
                .get(&texture_handle)
                .map(|t| t.data.clone()),
            Some(BEFORE.to_vec()),
            "the fixture must load as the pixel it was written with"
        );
        assert!(
            app.world().get_resource::<AssetWatcher>().is_some(),
            "the watcher must have started for an existing <ProjectDir>/assets"
        );

        // Give the OS backend time to actually begin delivering notifications,
        // then discard every event the initial load produced so anything read
        // below can only come from the writes.
        std::thread::sleep(DEBOUNCE * 3);
        let mut reader: ManualEventReader<AssetEvent<TextureAsset>> = app
            .world_mut()
            .resource_mut::<Events<AssetEvent<TextureAsset>>>()
            .get_reader();
        app.update();
        {
            let events = app.world().resource::<Events<AssetEvent<TextureAsset>>>();
            let _ = reader.read(events).count();
        }

        // The edit an artist would make, plus the same edit to the non-asset
        // file, in one burst so both land in the same debounce window.
        std::fs::write(&texture, png_bytes(AFTER)).unwrap();
        std::fs::write(&decoy, png_bytes(AFTER)).unwrap();

        // (texture reloads, decoy reloads). Counted rather than merely
        // detected, so an over-eager watcher that fires per raw write instead
        // of per debounced batch stays visible here.
        let mut modified = (0usize, 0usize);
        let count = |app: &App,
                     reader: &mut ManualEventReader<AssetEvent<TextureAsset>>,
                     modified: &mut (usize, usize)| {
            let events = app.world().resource::<Events<AssetEvent<TextureAsset>>>();
            for event in reader.read(events) {
                if let AssetEvent::Modified { id } = event {
                    if *id == texture_handle.id() {
                        modified.0 += 1;
                    } else if *id == decoy_handle.id() {
                        modified.1 += 1;
                    }
                }
            }
        };

        run_until(&mut app, "the saved texture reloaded", |app| {
            count(app, &mut reader, &mut modified);
            modified.0 > 0
        });

        // Both files were written in the same burst, so if the extension
        // filter were absent the decoy's reload would already be in flight.
        // Keep draining for a while anyway rather than relying on that.
        let settle = Instant::now() + DEBOUNCE * 5;
        while Instant::now() < settle {
            app.update();
            count(&app, &mut reader, &mut modified);
            std::thread::sleep(Duration::from_millis(5));
        }

        let decoy_modified = modified.1;
        assert_eq!(
            decoy_modified, 0,
            "notes.ron is not something bevy_asset serves in this engine, so \
             saving it must produce no reload -- {decoy_modified} arrived, \
             which means the extension filter is not filtering"
        );
        assert_eq!(
            app.world()
                .resource::<Assets<TextureAsset>>()
                .get(&texture_handle)
                .map(|t| t.data.clone()),
            Some(AFTER.to_vec()),
            "the reload must have re-read the file, not just re-announced the \
             old bytes"
        );
    }

    // A *deleted* asset is deliberately not reloaded, which is a decision
    // rather than something that fell out: the reload would fail and leave the
    // handle permanently `LoadState::Failed`, where skipping it keeps the last
    // good version resident and on screen. Renaming a file out of the way
    // mid-edit is a normal thing to do, and the game should not visibly break
    // for it. (An atomic save -- write a temp file, rename over the target --
    // still reloads, because the rename's destination does exist by the time
    // the debounce window closes.)
    #[test]
    fn a_deleted_asset_is_not_reloaded() {
        use crate::types::TextureAsset;
        use bevy_asset::{Assets, LoadState};
        use bsengine_app::new_app;

        const BEFORE: [u8; 4] = [10, 20, 30, 255];

        let project = unique("gone");
        let root = PathBuf::from(&project);
        std::fs::create_dir_all(root.join("assets")).unwrap();
        let _guard = ProbeDir(root.clone());

        let texture = root.join("assets").join("tex.png");
        std::fs::write(&texture, png_bytes(BEFORE)).unwrap();

        let mut app = new_app();
        app.insert_resource(ProjectDir(project.clone()));
        app.add_plugins(crate::plugin::AssetPlugin);
        app.add_plugins(AssetWatcherPlugin);

        let handle = {
            let server = app.world().resource::<AssetServer>();
            server.load::<TextureAsset>(format!("{project}/assets/tex.png"))
        };
        run_until(&mut app, "the asset finished loading", |app| {
            app.world()
                .resource::<Assets<TextureAsset>>()
                .get(&handle)
                .is_some()
        });

        std::thread::sleep(DEBOUNCE * 3);
        std::fs::remove_file(&texture).unwrap();

        let settle = Instant::now() + DEBOUNCE * 5;
        while Instant::now() < settle {
            app.update();
            std::thread::sleep(Duration::from_millis(5));
        }

        let state = app.world().resource::<AssetServer>().load_state(&handle);
        assert!(
            !matches!(state, LoadState::Failed(_)),
            "deleting the file must not push the handle into {state:?} -- that \
             state is permanent for anything still polling load_state"
        );
        assert_eq!(
            app.world()
                .resource::<Assets<TextureAsset>>()
                .get(&handle)
                .map(|t| t.data.clone()),
            Some(BEFORE.to_vec()),
            "the last good version must stay resident after a delete"
        );
    }

    // The editor opens with an empty ProjectDir. Watching the CWD instead
    // would mean watching the repository root -- `target/`, `.git/` and all --
    // which is the entire reason this module exists rather than bevy_asset's
    // own file_watcher feature.
    #[test]
    fn an_empty_or_missing_project_dir_starts_no_watcher() {
        use bsengine_app::new_app;

        for project_dir in [None, Some(String::new()), Some(unique("absent"))] {
            let mut app = new_app();
            if let Some(dir) = &project_dir {
                app.insert_resource(ProjectDir(dir.clone()));
            }
            app.add_plugins(crate::plugin::AssetPlugin);
            app.add_plugins(AssetWatcherPlugin);
            app.update();

            assert!(
                app.world().get_resource::<AssetWatcher>().is_none(),
                "ProjectDir {project_dir:?} must not start a watcher"
            );
            // And the Update system must survive the resource being absent.
            app.update();
        }
    }

    // Unit-level companion to the end-to-end test: the same reconstruction,
    // exercised on the path shapes a running watcher meets, without waiting on
    // the filesystem. `strip_base`/`engine_root` here are the exact pair
    // `start_asset_watcher` builds.
    #[test]
    fn reconstruct_rebuilds_the_engine_form_and_rejects_the_rest() {
        let engine_root = "games/mini-arena/assets";
        let strip_base = std::env::current_dir().unwrap().join(engine_root);

        assert_eq!(
            reconstruct(
                &strip_base.join("models").join("fox.glb"),
                &strip_base,
                engine_root
            )
            .as_deref(),
            Some("games/mini-arena/assets/models/fox.glb"),
            "a nested asset must come back in the spelling it was loaded with"
        );

        assert_eq!(
            reconstruct(&strip_base.join("SKY.PNG"), &strip_base, engine_root).as_deref(),
            Some("games/mini-arena/assets/SKY.PNG"),
            "the extension match is case-insensitive, but the path is not \
             re-cased -- bevy matches the string, so only the real spelling works"
        );

        for rejected in ["scene.ron", "player.js", "meta.toml", "model.bin", "README"] {
            assert_eq!(
                reconstruct(&strip_base.join(rejected), &strip_base, engine_root),
                None,
                "{rejected} has no registered loader, so reloading it would be a \
                 silent no-op and a misleading log line"
            );
        }

        assert_eq!(
            reconstruct(
                &std::env::current_dir()
                    .unwrap()
                    .join("target")
                    .join("a.png"),
                &strip_base,
                engine_root
            ),
            None,
            "a path outside the watch root must be dropped, not mangled"
        );
    }
}
