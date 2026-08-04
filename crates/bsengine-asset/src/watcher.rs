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
//! onto the engine-form root — see `reconstruct`.
//!
//! # What a reload is worth
//!
//! Spelling the path correctly is necessary and not sufficient: `reload` is
//! also a silent no-op for a path nothing has *loaded*, however well spelled.
//! The extension filter (`RELOADABLE_EXTENSIONS`) settles the asset's type
//! and `AssetServer::get_path_ids` settles the path, so the `info!` line that
//! says "reloading" is only emitted when a reload will really be dispatched;
//! the case where it will not is reported at `debug!` rather than dropped.

use crate::plugin::AssetRoot;
use bevy_app::{App, Plugin, Startup, Update};
use bevy_asset::{AssetPath, AssetServer};
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
use tracing::{debug, info, warn};

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
/// This is a cheap early-out, not the last word. `AssetServer::reload` on a
/// path no loader ever loaded is a *silent* no-op, and an extension match only
/// proves the asset *type* is servable — never that this particular path was
/// ever loaded. [`drain_asset_changes`] therefore asks `AssetServer` directly
/// before reloading anything; this list just avoids bothering it about the
/// `.ron` scene edits and `.js` script edits that make up most of what a save
/// touches (scenes go through `std::fs::read_to_string` and scripts through
/// `bsengine-scripting`, neither of which involves `bevy_asset` at all).
///
/// The two ways to get this list wrong are therefore **not** symmetric:
///
/// * An **omission** still costs a hot reload. An asset type missing from here
///   is dropped before anything else gets a say, and the edit does nothing.
/// * An **over-inclusion** is harmless. A `.txt` listed here would reach the
///   `AssetServer` check, find nothing loaded under that path, and be logged
///   and dropped there instead.
///
/// So when in doubt, add the extension.
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
    /// [`AssetRoot`]`.join(<ProjectDir>/assets)` — the prefix every path
    /// `notify` reports carries, and therefore the prefix [`reconstruct`]
    /// strips. Taken from the asset root `AssetPlugin` published rather than
    /// read from the CWD again, so it cannot drift out of agreement with the
    /// root `AssetServer::reload` resolves against.
    strip_base: PathBuf,
    /// `<ProjectDir>/assets` in the engine's own spelling, which is what the
    /// reconstructed paths are re-joined onto.
    engine_root: String,
}

/// Starts the watcher, or explains once why it is not starting.
fn start_asset_watcher(
    mut commands: Commands,
    project_dir: Option<Res<ProjectDir>>,
    asset_root: Option<Res<AssetRoot>>,
) {
    let Some(project_dir) = project_dir.map(|p| p.0.clone()).filter(|p| !p.is_empty()) else {
        info!("asset hot reload: no project directory set, not watching");
        return;
    };

    // The prefix notify's paths will carry has to be the *same* root
    // bevy_asset resolves loads against, or every reported path strips to
    // something AssetServer has never heard of and hot reload does nothing
    // without saying so. Taking it from AssetPlugin rather than reading the
    // CWD again is what makes that impossible by construction.
    let Some(asset_root) = asset_root.map(|r| r.0.clone()) else {
        warn!(
            "asset hot reload: AssetPlugin has not run, so the asset root is unknown, \
             not watching"
        );
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
    let strip_base = asset_root.join(&watch_root);

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
/// Returns `None` — meaning "do not reload this" — when the path has no
/// extension, has an extension no registered loader can serve, or cannot be
/// expressed relative to the watch root. Existence is checked by the caller,
/// not here.
///
/// The two reasons are not equally interesting, which is why the second one
/// warns and the first one says nothing:
///
/// * An unservable extension is the **normal** case. Most of what a save
///   touches is `.ron`, `.js`, an editor swap file or a directory, and
///   announcing each of those would bury the lines that matter.
/// * A path that will not strip is an **anomaly**, by construction: `notify`
///   only ever reports paths under the root it was told to watch, and
///   [`start_asset_watcher`] builds `strip_base` from that same root. If it
///   fires anyway the symptom is hot reload doing nothing at all, forever,
///   with nothing in the log to say why — the exact failure this module exists
///   to prevent. It cannot happen on the backends CI covers; it is reachable
///   where the backend reports a *resolved* path the watch root's own spelling
///   does not prefix (macOS FSEvents reached through a symlink). So it warns,
///   naming both paths, because the difference between them is the whole
///   diagnosis.
///
/// The extension is checked first so that only the anomaly's *relevant* half
/// is reported: on a backend that resolves paths, everything fails to strip,
/// and warning about each `.DS_Store` alongside the `.png` that actually
/// mattered would drown it.
fn reconstruct(changed: &Path, strip_base: &Path, engine_root: &str) -> Option<String> {
    let extension = changed.extension()?.to_str()?.to_ascii_lowercase();
    if !RELOADABLE_EXTENSIONS.contains(&extension.as_str()) {
        return None;
    }

    let Ok(relative) = changed.strip_prefix(strip_base) else {
        warn!(
            "asset hot reload: the watcher reported {}, which is not under the \
             watched root {} and so cannot be matched to a loaded asset; this \
             edit will not reload. Hot reload needs the two spellings to share a \
             prefix",
            changed.display(),
            strip_base.display()
        );
        return None;
    };

    // Forward slashes to match resolve_project_path exactly. bevy tolerates
    // either direction, but an identical string keeps the logs readable and
    // keeps this honest about what it is reproducing.
    let Some(relative) = relative.to_str().map(|r| r.replace('\\', "/")) else {
        warn!(
            "asset hot reload: {} is not valid UTF-8, so it cannot be spelled the \
             way it would have been loaded; this edit will not reload",
            changed.display()
        );
        return None;
    };
    Some(format!("{engine_root}/{relative}"))
}

/// Whether `AssetServer::reload(path)` would provably do nothing.
///
/// The extension filter upstream of this proves the *type* is one `bevy_asset`
/// serves; it cannot prove this particular path was ever loaded, and `reload`
/// on a path nothing holds is a silent no-op. `get_path_ids` is the exact
/// question: in `bevy_asset-0.14.2`, `reload` reloads the handles
/// `get_path_ids` reports (`server/mod.rs`, via `get_path_handles`, which is
/// `get_path_ids` filtered) and otherwise falls back to `should_reload`
/// (`server/info.rs`), which is `is_path_alive` — that same lookup again —
/// or a live *labeled* sub-asset. No loader in this workspace emits labeled
/// sub-assets, so an empty result leaves `reload` with nothing to do on either
/// branch.
///
/// One-way by design: empty means "definitely nothing happens", so suppressing
/// the reload is safe. A non-empty result only means "probably reloads", and
/// reloading anyway is the harmless direction.
fn reload_would_do_nothing(asset_server: &AssetServer, path: &str) -> bool {
    asset_server.get_path_ids(AssetPath::parse(path)).is_empty()
}

/// Drains everything the watcher thread has posted and reloads it. Never
/// waits: on a frame where nothing changed this is one uncontended lock and
/// one `try_recv` that returns `Empty`.
///
/// The two ways this can stop working for good — a poisoned lock and a
/// disconnected channel — both end by removing [`AssetWatcher`]. That is what
/// keeps their warnings from repeating: this system's very first act is to
/// return when the resource is absent, so each is said exactly once and the
/// per-frame work stops with it. A bare `warn!` would fire every frame,
/// because neither condition ever heals.
fn drain_asset_changes(
    mut commands: Commands,
    watcher: Option<Res<AssetWatcher>>,
    asset_server: Res<AssetServer>,
) {
    let Some(watcher) = watcher else {
        return;
    };

    let mut changed: Vec<String> = Vec::new();
    {
        let events = match watcher.events.lock() {
            Ok(events) => events,
            // Only reachable if a previous drain panicked mid-lock. The poison
            // is permanent, so there is nothing to retry — say so once and
            // stop, rather than failing silently on every frame from here on.
            Err(_) => {
                warn!(
                    "asset hot reload: the change queue was poisoned by an earlier \
                     panic; hot reload has stopped and edits to {} will no longer \
                     take effect until the app is restarted",
                    watcher.engine_root
                );
                commands.remove_resource::<AssetWatcher>();
                return;
            }
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
                Err(TryRecvError::Empty) => break,
                // The sender lives in the debouncer's thread, so this means
                // that thread is gone and no further change will ever arrive.
                // Indistinguishable from `Empty` if left unsaid, which would
                // leave hot reload looking merely idle forever.
                Err(TryRecvError::Disconnected) => {
                    warn!(
                        "asset hot reload: the file watcher thread has stopped; hot \
                         reload has stopped with it and edits to {} will no longer \
                         take effect until the app is restarted",
                        watcher.engine_root
                    );
                    commands.remove_resource::<AssetWatcher>();
                    break;
                }
            }
        }
    }

    for path in changed {
        // The extension filter only proved this is a *kind* of file bevy_asset
        // can serve; see `reload_would_do_nothing` for why this second gate is
        // the exact question and the first one is not. Saying "reloading" here
        // would otherwise be a claim about something that provably does not
        // happen.
        //
        // Not silent, though: an unreferenced asset is a perfectly ordinary
        // thing to edit, and someone asking why nothing happened deserves the
        // reason rather than no line at all.
        if reload_would_do_nothing(&asset_server, &path) {
            debug!(
                "asset hot reload: {path} changed on disk, but nothing in the scene \
                 has loaded it, so there is nothing to reload"
            );
            continue;
        }
        info!("asset hot reload: {path} changed on disk, reloading");
        asset_server.reload(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Shared with the identity scan's tests, which need the same probe
    // directory and the same "did the developer actually get told?" capture.
    // `unique` still spells its probes `bsengine-watch-probe-*` for every
    // caller — see its docs for why that prefix outlived this module.
    use crate::test_support::{capture_warnings, unique, ProbeDir};
    use notify_debouncer_full::DebouncedEvent;
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::Instant;

    /// Hard ceiling on every wait in this module's tests. A hung test in CI is
    /// far worse than a failing one, so nothing here ever blocks unbounded.
    const HARD_TIMEOUT: Duration = Duration::from_secs(20);

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

    // `reconstruct` refuses paths for two very different reasons, and the
    // difference is the whole point of this test.
    //
    // An unservable extension is routine -- most of what a save touches is a
    // `.ron`, a `.js` or an editor swap file -- and logging each one would
    // bury everything else. But a path that will not strip is an *anomaly*:
    // notify only ever reports paths under the root it was told to watch, so
    // by construction this cannot happen. Where it does happen -- a backend
    // that reports the *resolved* path, i.e. macOS FSEvents reached through a
    // symlink, which neither CI runner covers -- the symptom is hot reload
    // doing nothing at all with not one line to say why. That is precisely the
    // silent-failure class this module was written to make findable, so it is
    // the one case that must never be swallowed, and the warning has to name
    // both paths because the difference between them *is* the diagnosis.
    #[test]
    fn a_path_that_will_not_strip_warns_while_a_routine_rejection_stays_quiet() {
        let engine_root = "games/mini-arena/assets";
        let strip_base = std::env::current_dir().unwrap().join(engine_root);

        // A real asset, under the watched directory, reported with a spelling
        // that shares no prefix with the root the watcher holds.
        let resolved = PathBuf::from(if cfg!(windows) {
            r"D:\elsewhere\mini-arena\assets\models\fox.glb"
        } else {
            "/elsewhere/mini-arena/assets/models/fox.glb"
        });

        let (rebuilt, logs) = capture_warnings(|| reconstruct(&resolved, &strip_base, engine_root));
        assert_eq!(
            rebuilt, None,
            "a path outside the watch root must still be refused -- warning about \
             it is not licence to reload a guess"
        );
        assert!(
            logs.contains(&resolved.display().to_string()),
            "the warning must name the path that was reported, or there is no way \
             to see what spelling the backend actually used -- got:\n{logs}"
        );
        assert!(
            logs.contains(&strip_base.display().to_string()),
            "the warning must name the root that was expected, or there is no way \
             to see what it failed to match against -- got:\n{logs}"
        );

        // The routine half: refused just as firmly, and silently, because this
        // one happens constantly and means nothing.
        let (rebuilt, logs) = capture_warnings(|| {
            reconstruct(&strip_base.join("scene.ron"), &strip_base, engine_root)
        });
        assert_eq!(rebuilt, None);
        assert!(
            logs.is_empty(),
            "a file no loader serves is the normal case, not an anomaly; warning \
             about each one would bury the anomaly above -- got:\n{logs}"
        );
    }

    // An extension match proves the asset *type* is one bevy_asset serves. It
    // says nothing about whether *this path* was ever loaded, and
    // `AssetServer::reload` on a path nothing holds is a silent no-op -- so
    // without a second gate, saving any asset the scene does not reference
    // logs "reloading" for something that provably does not reload.
    //
    // Both files here are `.png` written the same way, so the extension filter
    // cannot tell them apart; that is asserted rather than assumed below.
    // `get_path_ids` can, and exactly: it is the same lookup `reload` itself
    // consults on both of its branches.
    #[test]
    fn only_a_path_something_has_loaded_is_worth_reloading() {
        use crate::types::TextureAsset;
        use bevy_asset::Assets;
        use bsengine_app::new_app;

        let project = unique("gate");
        let root = PathBuf::from(&project);
        std::fs::create_dir_all(root.join("assets")).unwrap();
        let _guard = ProbeDir(root.clone());

        std::fs::write(
            root.join("assets").join("used.png"),
            png_bytes([10, 20, 30, 255]),
        )
        .unwrap();
        std::fs::write(
            root.join("assets").join("unused.png"),
            png_bytes([40, 50, 60, 255]),
        )
        .unwrap();

        let mut app = new_app();
        app.insert_resource(ProjectDir(project.clone()));
        app.add_plugins(crate::plugin::AssetPlugin);

        // Only one of the two is ever referenced. The handle is held for the
        // rest of the test: dropping it would make the asset stop being
        // "alive", which is the very thing being measured.
        let referenced = format!("{project}/assets/used.png");
        let unreferenced = format!("{project}/assets/unused.png");
        let handle = {
            let server = app.world().resource::<AssetServer>();
            server.load::<TextureAsset>(referenced.clone())
        };
        run_until(&mut app, "the referenced asset finished loading", |app| {
            app.world()
                .resource::<Assets<TextureAsset>>()
                .get(&handle)
                .is_some()
        });

        // The extension filter passes both, identically -- so it cannot be
        // what distinguishes them.
        let engine_root = format!("{project}/assets");
        let strip_base = std::env::current_dir().unwrap().join(&engine_root);
        for file in ["used.png", "unused.png"] {
            assert_eq!(
                reconstruct(&strip_base.join(file), &strip_base, &engine_root).as_deref(),
                Some(format!("{engine_root}/{file}").as_str()),
                "{file} must survive the extension filter; if it did not, the gate \
                 below would be untested"
            );
        }

        let server = app.world().resource::<AssetServer>();
        assert!(
            !reload_would_do_nothing(server, &referenced),
            "{referenced} is loaded and alive, so a reload of it really dispatches \
             -- suppressing that would silently break hot reload for every asset"
        );
        assert!(
            reload_would_do_nothing(server, &unreferenced),
            "nothing has ever loaded {unreferenced}, so AssetServer::reload on it \
             does nothing at all; announcing it as a reload would be a lie, and a \
             lie in the one log line that makes silent no-ops findable"
        );
    }

    // A watcher thread that has died leaves a channel that returns
    // `Disconnected` forever. Treated as `Empty` it is indistinguishable from
    // an idle watcher, so hot reload looks fine and is dead -- but a bare
    // `warn!` in a system that runs every frame would repeat sixty times a
    // second, which is its own kind of useless.
    //
    // Retiring the resource resolves both at once: the warning is said exactly
    // once because the system's first act is to return when the resource is
    // absent, and the per-frame work stops with it. Asserting the second
    // `update` is silent is what actually pins the anti-spam property.
    #[test]
    fn a_dead_watcher_thread_is_reported_once_and_not_every_frame() {
        use bsengine_app::new_app;

        let mut app = new_app();
        app.add_plugins(crate::plugin::AssetPlugin);
        app.add_plugins(AssetWatcherPlugin);
        app.update();
        assert!(
            app.world().get_resource::<AssetWatcher>().is_none(),
            "no ProjectDir, so Startup must not have installed a watcher of its own"
        );

        // Exactly the state a dead watcher thread leaves behind: a receiver
        // whose sender is gone. The debouncer beside it is real but idle --
        // `AssetWatcher` holds one purely for its `Drop`.
        let (tx, rx) = mpsc::channel();
        drop(tx);
        let (live_tx, _live_rx) = mpsc::channel();
        app.insert_resource(AssetWatcher {
            _debouncer: Mutex::new(new_debouncer(DEBOUNCE, None, live_tx).unwrap()),
            events: Mutex::new(rx),
            strip_base: std::env::current_dir().unwrap().join("probe/assets"),
            engine_root: "probe/assets".to_string(),
        });

        let (_, logs) = capture_warnings(|| app.update());
        assert!(
            logs.contains("probe/assets"),
            "a dead watcher thread must say so, naming what is no longer being \
             watched -- got:\n{logs}"
        );
        assert!(
            app.world().get_resource::<AssetWatcher>().is_none(),
            "the watcher must be retired once its thread is gone; leaving it in \
             place is what would make the warning repeat every frame"
        );

        let (_, logs) = capture_warnings(|| {
            app.update();
            app.update();
        });
        assert!(
            !logs.contains("asset hot reload"),
            "the warning must be said once, not once per frame -- two further \
             frames produced:\n{logs}"
        );
    }
}
