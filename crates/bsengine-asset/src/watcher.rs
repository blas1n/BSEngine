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
//!
//! # A rename is two facts, and reloading only needs one of them
//!
//! An asset renamed while the game runs arrives here as a single event naming
//! **both** paths, old first (measured in
//! `a_rename_is_reported_with_both_the_old_and_the_new_path`). Reloading cares
//! only about the destination — the source no longer exists, and reloading a
//! path that does not is how a handle ends up permanently `Failed` — so this
//! module used to drop the old half on the floor.
//!
//! That old half is the only in-engine record that a rename ever happened, and
//! `identity::rename` is what it is for: a path
//! spelled inside a JavaScript string literal cannot be rewritten by any index,
//! so the only thing that can save such a reference is the project remembering
//! where the asset used to be. Every paired event is therefore handed to that
//! recorder before the reload logic discards the source.

use crate::identity::rename::{record_rename, Endpoint};
use crate::identity::scan::ASSETS_DIR;
use crate::identity::AssetIndex;
use crate::plugin::AssetRoot;
use bevy_app::{App, Plugin, Startup, Update};
use bevy_asset::{AssetPath, AssetServer};
use bevy_ecs::prelude::{Commands, Res, ResMut, Resource};
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
/// fails. 200ms is long enough to swallow those bursts on an idle machine
/// (measured: five back-to-back writes collapse into one event) and short
/// enough that a save still feels instantaneous to the person who made it.
///
/// It is not what makes a burst reload *once*, though, and nothing here relies
/// on it doing so: whether a burst lands inside one window is the debouncer's
/// decision on a timing budget this engine does not control, and a loaded
/// machine splits one readily. [`drain_asset_changes`] dedupes by path across
/// everything it drains, so a burst the window failed to merge still reloads
/// once — see `a_burst_of_saves_reloads_the_asset_exactly_once`.
const DEBOUNCE: Duration = Duration::from_millis(200);

/// Extensions `bevy_asset` can actually serve in this engine, and nothing
/// else.
///
/// This is a cheap early-out, not the last word. `AssetServer::reload` on a
/// path no loader ever loaded is a *silent* no-op, and an extension match only
/// proves the asset *type* is servable — never that this particular path was
/// ever loaded. [`drain_asset_changes`] therefore asks `AssetServer` directly
/// before reloading anything; this list just avoids bothering it about the
/// `.ron` scene edits that make up much of what a save touches (scenes go
/// through `std::fs::read_to_string`, which does not involve `bevy_asset` at
/// all).
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
/// * `js` — `bsengine_scripting::script_asset::ScriptSourceLoader`
///
/// None of those loaders declares `AssetLoader::extensions()` — every load
/// site in the engine is type-directed (`AssetServer::load::<T>(path)`), so
/// there is no registry to interrogate and this list is maintained by hand.
///
/// # `js` was deliberately absent until roadmap item 31
///
/// Scripts used to be read with `std::fs::read_to_string` at `PostStartup`,
/// so a changed `.js` had nothing on the `bevy_asset` side to reload and
/// listing it here would only have produced a `debug!` line per save. Item 31
/// made a script a `Handle<ScriptSource>` that its entity retains for life,
/// which is what makes `AssetEvent::Modified` reach
/// `bsengine_scripting::plugin::reexecute_modified_scripts` — so the entry
/// below is now the first half of the reload that matters most in this
/// engine, scripts being the bulk of what a scene references.
///
/// # Not to be merged with `identity::scan`'s `IDENTIFIED_EXTENSIONS`
///
/// The two lists overlap heavily and answer different questions ("can
/// `bevy_asset` reload this?" versus "does this deserve a `.meta` sidecar?"),
/// and the cost of a wrong entry runs opposite ways — see that constant's
/// docs. `js` moving into this list *is* the divergence they were kept
/// separate for; `ron` is still in that one and not this one.
const RELOADABLE_EXTENSIONS: &[&str] = &[
    "glb", "gltf", "png", "jpg", "jpeg", "hdr", "wgsl", "wav", "ogg", "mp3", "flac", "js",
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
    // The *absolutised* root, not the one `watch()` was given, and the
    // difference is the whole reason a rename can be recorded at all.
    //
    // This cache is what stitches a backend's two rename halves into one event
    // naming both paths, on the backends that do not emit a rename cookie —
    // Windows among them. It does so by looking the reported path up in a
    // `HashMap<PathBuf, FileId>` keyed by *exact* path equality, and the paths
    // it is asked about are the ones `notify` reports: CWD-absolutised, always,
    // even for a relative watch root (which is precisely what
    // `notify_reports_cwd_absolutised_paths_even_for_a_relative_watch_root`
    // measures). Seeded with the relative root, every lookup misses, no rename
    // is ever paired, and the old path — the only thing that makes a stale
    // reference recoverable — is never reported at all.
    //
    // Nothing said so before this because nothing read the old path: an
    // unpaired rename still reloads the destination perfectly well, so the
    // whole failure was invisible. Linux hides it further, since inotify's
    // cookie pairs renames without consulting the cache at all.
    debouncer
        .cache()
        .add_root(&strip_base, RecursiveMode::Recursive);

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
///   touches is `.ron`, a `.meta` sidecar, an editor swap file or a directory,
///   and announcing each of those would bury the lines that matter.
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

/// Rebuilds the spelling an asset's *identity* is keyed by — project-relative
/// and forward-slashed, `assets/models/fox.glb` — from the path `notify`
/// reported.
///
/// Deliberately not [`reconstruct`], and the difference is not cosmetic:
///
/// * That one produces the `<ProjectDir>/assets/…` form `AssetServer::reload`
///   matches, because it is answering "what was this loaded as". A `.meta`
///   sidecar, a scene reference and an [`AssetIndex`] key are all spelled
///   *project-relative* instead, which is what makes them portable between a
///   project opened from the editor and the same project run from its own
///   directory.
/// * That one also refuses everything outside `RELOADABLE_EXTENSIONS`, which
///   excludes `.ron` — one of the two file types whose references live in
///   plain text and are therefore the ones a rename hurts most (`.js` is the
///   other, and joined that list in item 31). Filtering by what `bevy_asset`
///   can reload would drop exactly the cases item 30 exists for.
///
/// Returns `None` for a path that is not under the watch root, and for the
/// watch root itself. Silently, unlike [`reconstruct`]: a rename that moves a
/// file *out of* `assets/` reports one path this can spell and one it cannot,
/// which is a thing that legitimately happens rather than an anomaly.
fn project_relative(changed: &Path, strip_base: &Path) -> Option<String> {
    let relative = changed.strip_prefix(strip_base).ok()?;
    let relative = relative.to_str()?.replace('\\', "/");
    if relative.is_empty() {
        return None;
    }
    Some(format!("{ASSETS_DIR}/{relative}"))
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
///
/// [`AssetIndex`] is optional because a host may run the watcher without
/// registering `AssetIdentityPlugin`. A rename is still recorded on disk in that
/// case — the sidecars are the record, and the index is a cache of them.
fn drain_asset_changes(
    mut commands: Commands,
    watcher: Option<Res<AssetWatcher>>,
    asset_server: Res<AssetServer>,
    mut index: Option<ResMut<AssetIndex>>,
) {
    let Some(watcher) = watcher else {
        return;
    };

    let mut changed: Vec<String> = Vec::new();
    // Collected rather than acted on inside the loop below, for the same reason
    // `changed` is: recording a rename writes a sidecar, and doing that while
    // holding the queue's lock would mean the watcher thread's channel is held
    // across filesystem i/o for no benefit. Order is preserved, which matters —
    // an asset renamed twice in one window has to be followed hop by hop.
    let mut renamed: Vec<(PathBuf, PathBuf)> = Vec::new();
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
                    for event in batch.iter() {
                        // Two paths means a rename, old first: `Modify(Name(
                        // Both))` is the only kind `notify` defines that carries
                        // a pair, and the debouncer stitches a backend's
                        // separate halves into exactly that. Matched on the
                        // *pairing* rather than on the kind, deliberately —
                        // `a_rename_is_reported_with_both_the_old_and_the_new
                        // _path` pins the pair and leaves a backend free to
                        // spell the kind its own way.
                        if let [from, to] = event.event.paths.as_slice() {
                            renamed.push((from.clone(), to.clone()));
                        }
                        for path in event.event.paths.iter() {
                            let Some(engine_path) =
                                reconstruct(path, &watcher.strip_base, &watcher.engine_root)
                            else {
                                continue;
                            };
                            // A deleted file must not be reloaded: the load
                            // would fail and leave the handle permanently
                            // `Failed`, where leaving it alone keeps the last
                            // good version resident. A rename-style atomic save
                            // still reloads, because its destination does exist
                            // by now — and the rename above will have found no
                            // sidecar beside the temporary it came from, so the
                            // save costs the asset's identity nothing.
                            if !path.is_file() {
                                continue;
                            }
                            if !changed.iter().any(|c| c == &engine_path) {
                                changed.push(engine_path);
                            }
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

    // Before the reloads, because a rename's *identity* half is about where the
    // asset went and the reload half is only about its new contents: doing the
    // record first means a reload that somehow panicked could not cost the
    // project a former path.
    for (from, to) in renamed {
        // Both halves have to be spellable for the move to mean anything. A
        // rename that leaves the watched directory names a destination this
        // cannot spell, and one that arrives from outside names a source it
        // cannot; neither is a move *within* the project, and inventing a
        // project-relative path for a file that is not in the project would put
        // a former path in a sidecar that no reference could ever match.
        let (Some(from_relative), Some(to_relative)) = (
            project_relative(&from, &watcher.strip_base),
            project_relative(&to, &watcher.strip_base),
        ) else {
            continue;
        };
        record_rename(
            Endpoint {
                path: &from,
                relative: &from_relative,
            },
            Endpoint {
                path: &to,
                relative: &to_relative,
            },
            index.as_deref_mut(),
        );
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

    /// Watches `watch_root` recursively, writes the nested file once, and
    /// returns everything the debouncer emitted.
    // Item 30 sub-item D recovers a stale asset path — the kind embedded in a
    // JS string literal — by asking the index where that path's asset went.
    // That only works if something records the move, and today only orphan
    // recovery does: a rename made while the watcher is running is currently
    // reported as a change to the destination and nothing else, because
    // `drain_asset_changes` drops the source (it no longer `is_file()`).
    //
    // The information is not missing, only discarded. Measured here:
    // `notify-debouncer-full`'s `FileIdMap` stitches the backend's two halves
    // into one `Modify(Name(Both))` carrying **both** paths, old first.
    //
    // This test exists so that stays true. If a platform or a `notify` upgrade
    // ever reports a rename without the old path, sub-item D's recovery loses
    // its only in-engine source of former paths — and would do so silently,
    // since a rename would still look like an ordinary change to the
    // destination.
    #[test]
    fn a_rename_is_reported_with_both_the_old_and_the_new_path() {
        let root = std::env::temp_dir().join(unique("rename-probe"));
        let _guard = make_tree(root.clone());

        let (tx, rx) = mpsc::channel();
        let mut debouncer = new_debouncer(DEBOUNCE, None, tx).unwrap();
        debouncer
            .watcher()
            .watch(&root, RecursiveMode::Recursive)
            .unwrap();
        debouncer.cache().add_root(&root, RecursiveMode::Recursive);

        std::thread::sleep(DEBOUNCE * 3);
        while rx.try_recv().is_ok() {}

        let from = root.join(nested());
        let to = root.join("assets").join("models").join("renamed.txt");
        std::fs::rename(&from, &to).unwrap();

        let events = collect(&rx, DEBOUNCE * 3);
        let rendered: Vec<String> = events
            .iter()
            .map(|e| format!("{:?} paths={:?}", e.event.kind, e.event.paths))
            .collect();

        // Deliberately not asserting the exact `EventKind`: what sub-item D
        // needs is the *pairing*, and a backend is entitled to spell it
        // differently. What it may not do is drop the old path, which is the
        // only thing that makes a stale reference recoverable.
        let paired = events
            .iter()
            .find(|e| e.event.paths.len() >= 2)
            .unwrap_or_else(|| {
                panic!(
                    "a rename reported no event carrying both paths, so nothing \
                     records where {from:?} went; sub-item D's recovery has no \
                     in-engine source of former paths on this platform:\n{}",
                    rendered.join("\n")
                )
            });

        assert!(
            paired.event.paths.contains(&from) && paired.event.paths.contains(&to),
            "the paired event must name both the old and the new path, got {:?}",
            paired.event.paths
        );
        assert_eq!(
            paired.event.paths[0], from,
            "old path must come first, or a recorder cannot tell which is which"
        );
    }

    // The pairing above was measured with an *absolute* watch root. The engine
    // watches a **relative** one -- `<ProjectDir>/assets`, relative to the
    // process CWD -- and on a backend that emits no rename cookie, that
    // difference decides whether a rename is reported as a pair at all.
    //
    // `notify-debouncer-full` stitches the two halves by looking each reported
    // path up in a `HashMap<PathBuf, FileId>` keyed on *exact* path equality,
    // and the paths it is asked about are the ones notify reports: always
    // CWD-absolutised, even for a relative watch root (measured in
    // `notify_reports_cwd_absolutised_paths_even_for_a_relative_watch_root`).
    // So the cache has to be seeded with the **absolutised** root even though
    // `watch()` is handed the relative one. Seeded with the relative spelling
    // every lookup misses, and on Windows a rename arrives as two unrelated
    // events with nothing to say they are the same file.
    //
    // Not hypothetical: that is exactly what `start_asset_watcher` did until
    // something finally needed the old path, and nothing noticed for as long as
    // it stood, because an unpaired rename still reloads the destination
    // perfectly well. Linux hid it too -- inotify's cookie pairs renames
    // without consulting the cache at all -- so this is the shape of failure CI
    // alone would never have found.
    #[test]
    fn a_relative_watch_root_pairs_a_rename_when_the_cache_is_absolutised() {
        let root = PathBuf::from(unique("rename-relative"));
        let _guard = make_tree(root.clone());

        let (tx, rx) = mpsc::channel();
        let mut debouncer = new_debouncer(DEBOUNCE, None, tx).unwrap();
        debouncer
            .watcher()
            .watch(&root, RecursiveMode::Recursive)
            .unwrap();
        // What `start_asset_watcher` does: the cache is given what notify will
        // report, not what `watch()` was given.
        let absolutised = std::env::current_dir().unwrap().join(&root);
        debouncer
            .cache()
            .add_root(&absolutised, RecursiveMode::Recursive);

        std::thread::sleep(DEBOUNCE * 3);
        while rx.try_recv().is_ok() {}

        let renamed = Path::new("assets").join("models").join("renamed.txt");
        std::fs::rename(root.join(nested()), root.join(&renamed)).unwrap();

        let events = collect(&rx, DEBOUNCE * 3);
        let rendered: Vec<String> = events
            .iter()
            .map(|e| format!("{:?} paths={:?}", e.event.kind, e.event.paths))
            .collect();
        let paired = events
            .iter()
            .find(|e| e.event.paths.len() >= 2)
            .unwrap_or_else(|| {
                panic!(
                    "a rename under a relative watch root reported no event \
                     carrying both paths, so the watcher cannot tell a rename \
                     from a delete plus a create and no former path is ever \
                     recorded:\n{}",
                    rendered.join("\n")
                )
            });
        assert_eq!(
            paired.event.paths,
            [absolutised.join(nested()), absolutised.join(&renamed)],
            "old first, new second, both in the absolutised spelling the \
             watcher strips its root from"
        );
    }

    fn probe(watch_root: &Path) -> Vec<DebouncedEvent> {
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

        std::fs::write(watch_root.join(nested()), b"after").unwrap();
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
        let abs_events = probe(&abs_root);
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
        let rel_events = probe(&rel_root);
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
        let odd_events = probe(&odd_root);
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

        // (texture reloads, decoy reloads). Only the decoy's count is asserted
        // on; the texture's says when to stop waiting. How many reloads a save
        // is worth is `a_burst_of_saves_reloads_the_asset_exactly_once`'s
        // subject, and it is not a question this test's timing can answer.
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

    // A save is rarely one write -- editors truncate, write and flush, and glTF
    // exporters rewrite a file several times in a row. What must never regress
    // is that N writes stop producing N reloads.
    //
    // That guarantee is *not* the debouncer's, and this test deliberately does
    // not ask the debouncer for it. How a burst gets batched is a third-party
    // crate's decision on a timing budget this engine does not control, and it
    // is not stable across machines. Measured, on the same 200ms window:
    //
    //   * idle Windows: one `fs::write` arrives as one `Modify(Any)`, and five
    //     back-to-back writes collapse into a single event.
    //   * loaded Linux CI: nothing collapses. Each write arrives as its own
    //     `Modify(Data(Any))` + `Access(Close(Write))` pair, and consecutive
    //     writes straddle the window, so five writes can arrive as five
    //     separate batches.
    //
    // The engine tolerates both, because `drain_asset_changes` dedupes by path
    // across everything it drains -- so the burst below is deliberately spaced
    // *wider* than the debounce window, which forces one batch per write on
    // every platform and under every load. That is the hostile case, not the
    // lucky one: a test that only passes when the batches merge would be
    // pinning the debouncer's behaviour rather than the engine's.
    //
    // Two things make the count well-defined rather than a race:
    //
    //  * No frame runs during the burst or the settle that follows it.
    //    `drain_asset_changes` is the only thing that empties the queue, so
    //    every batch is still queued when the single `app.update()` below
    //    drains it in one go. (A real game does run frames in between, and
    //    would reload once per batch it drains; the dedupe is per drain. This
    //    test measures the dedupe, so it hands the drain the whole burst.)
    //  * The watcher resource is removed immediately after that one drain, so
    //    a batch that the runner delivers late cannot slip a second reload in
    //    behind the assertion's back.
    #[test]
    fn a_burst_of_saves_reloads_the_asset_exactly_once() {
        use crate::types::TextureAsset;
        use bevy_asset::{AssetEvent, Assets};
        use bevy_ecs::event::{Events, ManualEventReader};
        use bsengine_app::new_app;

        /// Writes in the burst. Each is its own debounced batch (see above),
        /// so this is also how many reloads a broken dedupe would produce.
        const WRITES: usize = 3;

        const AFTER: [u8; 4] = [200, 100, 50, 255];

        let project = unique("burst");
        let root = PathBuf::from(&project);
        std::fs::create_dir_all(root.join("assets")).unwrap();
        let _guard = ProbeDir(root.clone());

        let texture = root.join("assets").join("tex.png");
        std::fs::write(&texture, png_bytes([10, 20, 30, 255])).unwrap();

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
        assert!(
            app.world().get_resource::<AssetWatcher>().is_some(),
            "the watcher must have started for an existing <ProjectDir>/assets"
        );

        // Give the OS backend time to actually begin delivering, then drop
        // both the asset events the initial load produced and anything already
        // queued for the watcher, so what is counted below can only be the
        // burst.
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

        // The burst. Spaced past the window on purpose -- see above -- and with
        // no frames in between, so each write ends up as its own queued batch.
        for i in 0..WRITES {
            let pixel = if i + 1 == WRITES {
                AFTER
            } else {
                [i as u8, 0, 0, 255]
            };
            std::fs::write(&texture, png_bytes(pixel)).unwrap();
            if i + 1 < WRITES {
                std::thread::sleep(DEBOUNCE * 2);
            }
        }

        // Long enough for the last batch to have been flushed and queued: the
        // debouncer emits a path once it has been quiet for `DEBOUNCE`, so this
        // is several times its own budget, which is the margin a loaded runner
        // needs. If it is somehow not enough the drain below finds an empty
        // queue and the wait after it fails, rather than anything passing by
        // accident.
        std::thread::sleep(DEBOUNCE * 8);

        // One frame, one drain of the whole queued burst -- and one `reload`,
        // because the drain dedupes by path. This is the entire property.
        app.update();

        // From here nothing can reload anything: `drain_asset_changes` returns
        // immediately when this resource is absent. Whatever the debouncer
        // still has to say is now inert, so the count below is exactly what the
        // single drain above dispatched.
        app.world_mut().remove_resource::<AssetWatcher>();

        let mut modified = 0usize;
        let count = |app: &App,
                     reader: &mut ManualEventReader<AssetEvent<TextureAsset>>,
                     modified: &mut usize| {
            let events = app.world().resource::<Events<AssetEvent<TextureAsset>>>();
            for event in reader.read(events) {
                if matches!(event, AssetEvent::Modified { id } if *id == handle.id()) {
                    *modified += 1;
                }
            }
        };

        run_until(&mut app, "the burst reloaded the texture", |app| {
            count(app, &mut reader, &mut modified);
            modified > 0
        });

        // A second reload, if the dedupe ever stopped deduping, would land a
        // frame or two behind the first -- so keep draining rather than
        // concluding from the first one that there was only one.
        let settle = Instant::now() + DEBOUNCE * 5;
        while Instant::now() < settle {
            app.update();
            count(&app, &mut reader, &mut modified);
            std::thread::sleep(Duration::from_millis(5));
        }

        assert_eq!(
            modified, 1,
            "a burst of {WRITES} saves to one asset must reload it exactly \
             once, however the debouncer chose to batch it -- {modified} \
             reloads arrived, which means drain_asset_changes is no longer \
             collapsing a queue by path"
        );
        assert_eq!(
            app.world()
                .resource::<Assets<TextureAsset>>()
                .get(&handle)
                .map(|t| t.data.clone()),
            Some(AFTER.to_vec()),
            "the one reload must have re-read the file, and must have picked up \
             the last write of the burst rather than an earlier one"
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

    // The end-to-end property sub-item D is built on, driven through the real
    // plugin: an asset renamed under `<ProjectDir>/assets` while the app runs
    // keeps its identity, its sidecar moves with it, and the path it left is
    // recorded -- on disk *and* in the live index -- so a reference that still
    // names the old path has something to resolve against.
    //
    // Before this, the pairing arrived and the old half was dropped, so
    // `former_paths` could only ever be written by the scan's orphan recovery
    // -- which needs the move to have happened while the engine was *not*
    // running, and needs the contents to be unchanged since. A rename made the
    // normal way, in an editor with the game up, recorded nothing at all.
    //
    // The scan runs here too, rather than the sidecar being hand-written: it is
    // what mints the identity this test follows, and its own atomic sidecar
    // write is itself a rename the watcher sees -- so this covers the recorder
    // not reacting to its own file format as well.
    #[test]
    fn a_rename_moves_the_sidecar_along_and_records_the_old_path() {
        use crate::identity::{sidecar_path, AssetIdentityPlugin, AssetIndex, Sidecar};
        use bsengine_app::new_app;

        let project = unique("moved");
        let root = PathBuf::from(&project);
        std::fs::create_dir_all(root.join("assets")).unwrap();
        let _guard = ProbeDir(root.clone());

        let before = root.join("assets").join("tex.png");
        let after = root.join("assets").join("sprite.png");
        std::fs::write(&before, png_bytes([10, 20, 30, 255])).unwrap();
        // Renamed in the same burst and never an asset: a `.meta` beside it
        // would be litter in the user's source tree that nothing ever reads.
        let note = root.join("assets").join("CREDITS.md");
        std::fs::write(&note, b"not an asset").unwrap();

        let mut app = new_app();
        app.insert_resource(ProjectDir(project.clone()));
        app.add_plugins(crate::plugin::AssetPlugin);
        app.add_plugins(AssetIdentityPlugin);
        app.add_plugins(AssetWatcherPlugin);
        app.update();

        let minted = Sidecar::read(sidecar_path(&before))
            .expect("read the minted sidecar")
            .expect("the scan must have identified the fixture");
        assert!(
            minted.former_paths.is_empty(),
            "the fixture has never moved, so anything already here would make \
             the assertion below meaningless"
        );

        // Let the backend actually begin delivering, then drain everything
        // startup stirred up -- the scan's own sidecar write included.
        std::thread::sleep(DEBOUNCE * 3);
        app.update();

        std::fs::rename(&before, &after).unwrap();
        std::fs::rename(&note, root.join("assets").join("NOTES.md")).unwrap();

        run_until(&mut app, "the sidecar followed the renamed asset", |_| {
            sidecar_path(&after).exists()
        });

        let moved = Sidecar::read(sidecar_path(&after))
            .expect("read the moved sidecar")
            .expect("present");
        assert_eq!(
            moved.guid, minted.guid,
            "a rename must not change the identity -- every reference already \
             stored against it points at this asset"
        );
        assert_eq!(
            moved.former_paths,
            ["assets/tex.png"],
            "the path the asset left is the only thing that can recover a \
             reference spelled inside a JS string literal, which no index can \
             rewrite"
        );
        assert!(
            !sidecar_path(&before).exists(),
            "a sidecar left at the old name turns a clean rename into an orphan \
             the next scan has to guess at by content hash"
        );

        let index = app.world().resource::<AssetIndex>();
        assert_eq!(
            index.guid_for_path("assets/sprite.png"),
            Some(minted.guid),
            "the asset has to be findable where it now is, without a restart"
        );
        assert_eq!(
            index.path_for_guid(minted.guid),
            Some("assets/sprite.png"),
            "and the identity has to point at where it now is, or the two \
             directions of one fact disagree until the app is restarted"
        );
        assert_eq!(
            index.guid_for_former_path("assets/tex.png"),
            Some(minted.guid),
            "this is the lookup sub-item D recovers a stale reference with, and \
             the whole reason the old half of the event is kept"
        );

        // A duplicate or late-arriving event must not append a second former
        // path, and the `.md` must never grow a sidecar at either name.
        let settle = Instant::now() + DEBOUNCE * 5;
        while Instant::now() < settle {
            app.update();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            Sidecar::read(sidecar_path(&after))
                .expect("read")
                .expect("present")
                .former_paths,
            ["assets/tex.png"],
            "the same move recorded twice would grow a committed file every \
             time the watcher repeated itself"
        );
        for name in ["CREDITS.md", "NOTES.md"] {
            assert!(
                !root.join("assets").join(format!("{name}.meta")).exists(),
                "{name} is not an asset, so renaming it must write nothing at all"
            );
        }
    }

    // The other half of the same event: an atomic save -- write a temporary,
    // rename it over the target -- is a rename too, and is what every careful
    // editor and this crate's own `Sidecar::write` do on every save. It must
    // still reload, and it must record *nothing*, or `former_paths` fills with
    // the names of temporary files that never identified anything.
    //
    // Two things make this test able to fail for the right reason:
    //
    //  * The temporary is spelled `tex_tmp.png` -- an extension the scan
    //    identifies and a name no pattern could reject -- so the only thing
    //    that can tell it from a move is the sidecar it never had.
    //  * It is created a full debounce window *before* the rename. Written and
    //    renamed inside one window, `notify-debouncer-full` collapses the pair
    //    into a plain create at the destination and no rename is ever reported,
    //    so the recorder would not be reached at all and this would pass
    //    without testing anything.
    #[test]
    fn an_atomic_save_reloads_the_asset_and_records_no_former_path() {
        use crate::identity::{sidecar_path, AssetIdentityPlugin, Sidecar};
        use crate::types::TextureAsset;
        use bevy_asset::{AssetEvent, Assets};
        use bevy_ecs::event::{Events, ManualEventReader};
        use bsengine_app::new_app;

        const BEFORE: [u8; 4] = [10, 20, 30, 255];
        const AFTER: [u8; 4] = [200, 100, 50, 255];

        let project = unique("saved");
        let root = PathBuf::from(&project);
        std::fs::create_dir_all(root.join("assets")).unwrap();
        let _guard = ProbeDir(root.clone());

        let texture = root.join("assets").join("tex.png");
        let temp = root.join("assets").join("tex_tmp.png");
        std::fs::write(&texture, png_bytes(BEFORE)).unwrap();

        let mut app = new_app();
        app.insert_resource(ProjectDir(project.clone()));
        app.add_plugins(crate::plugin::AssetPlugin);
        app.add_plugins(AssetIdentityPlugin);
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
        let minted = Sidecar::read(sidecar_path(&texture))
            .expect("read the minted sidecar")
            .expect("the scan must have identified the fixture");

        // The temporary, written and then left alone long enough for the
        // debouncer to flush it -- see above.
        std::thread::sleep(DEBOUNCE * 3);
        std::fs::write(&temp, png_bytes(AFTER)).unwrap();
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

        std::fs::rename(&temp, &texture).unwrap();

        let mut modified = 0usize;
        let count = |app: &App,
                     reader: &mut ManualEventReader<AssetEvent<TextureAsset>>,
                     modified: &mut usize| {
            let events = app.world().resource::<Events<AssetEvent<TextureAsset>>>();
            for event in reader.read(events) {
                if matches!(event, AssetEvent::Modified { id } if *id == handle.id()) {
                    *modified += 1;
                }
            }
        };

        let (_, logs) = capture_warnings(|| {
            run_until(&mut app, "the atomically saved texture reloaded", |app| {
                count(app, &mut reader, &mut modified);
                modified > 0
            });
            let settle = Instant::now() + DEBOUNCE * 5;
            while Instant::now() < settle {
                app.update();
                count(&app, &mut reader, &mut modified);
                std::thread::sleep(Duration::from_millis(5));
            }
        });

        assert_eq!(
            app.world()
                .resource::<Assets<TextureAsset>>()
                .get(&handle)
                .map(|t| t.data.clone()),
            Some(AFTER.to_vec()),
            "an atomic save must still hot reload -- that is how most editors \
             save, and breaking it would break hot reload for most of them"
        );

        let saved = Sidecar::read(sidecar_path(&texture))
            .expect("read")
            .expect("present");
        assert_eq!(
            saved.guid, minted.guid,
            "a save must not change the asset's identity"
        );
        assert!(
            saved.former_paths.is_empty(),
            "a save is not a move: recording the temporary's name would put a \
             line of garbage in a committed file on every single save, and \
             would do it for every asset -- got {:?}",
            saved.former_paths
        );
        assert!(
            !sidecar_path(&temp).exists(),
            "the temporary must not have been given an identity of its own"
        );
        assert!(
            !logs.contains("asset identity"),
            "and the save must be silent about identity -- a warning per save \
             is its own kind of broken -- got:\n{logs}"
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

        // Item 31 gave scripts a real loader and a retained handle, so a `.js`
        // edit is now the reload this whole module matters most for. It used
        // to sit in the rejected list below; a change that quietly puts it
        // back has to fail here rather than turn script hot reload off with no
        // symptom other than edits doing nothing.
        assert_eq!(
            reconstruct(
                &strip_base.join("scripts").join("player.js"),
                &strip_base,
                engine_root
            )
            .as_deref(),
            Some("games/mini-arena/assets/scripts/player.js"),
            "a script must survive the extension filter -- `ScriptSourceLoader` \
             serves it and its entity retains the handle, so reloading it really \
             dispatches"
        );

        for rejected in ["scene.ron", "meta.toml", "model.bin", "README"] {
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
    // `.ron`, a `.meta` or an editor swap file -- and logging each one would
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
