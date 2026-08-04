use crate::identity::{AssetGuid, AssetIndex};
use bevy_asset::{Asset, AssetServer, Assets, Handle};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::sync::{LazyLock, Mutex, PoisonError, RwLock};

/// How an asset gets from disk into `Assets<T>` — one real, working
/// mechanism per variant (mirrors Unreal's blocking `LoadObject` vs.
/// `FStreamableManager`'s async load, as two things a caller picks between,
/// not a hint):
///
/// `Sync` calls the loader function directly and inserts the result
/// immediately (blocking, zero-latency). `Async` calls `bevy_asset`'s own
/// `AssetServer::load`, which requires a registered `AssetLoader` for `T` —
/// multi-frame latency, but automatically tracked by the file watcher.
///
/// Nothing in the engine passes `Sync` any more. All four consumers (glTF,
/// custom shaders, skybox, audio) load asynchronously, and the only `Sync`
/// call sites left in the workspace are this module's own unit tests. The
/// variant is kept, working and supported, because blocking-vs-streaming is
/// the caller's choice to make: a game that wants to stall on a small asset
/// behind a loading screen should not have to reimplement it.
///
/// # Polling an `Async` load correctly
///
/// `Async` returns a `Handle` unconditionally, so a missing or malformed file
/// is indistinguishable from a slow one at the call site. A consumer must
/// therefore:
///
/// 1. **Request once and retain the handle**, rather than re-requesting each
///    frame while polling. Re-calling `AssetServer::load` for a path whose
///    state is `Failed` resets it to `Loading` and restarts the load
///    (`bevy_asset` 0.14.2, `server/info.rs:216-221`). Because `Failed` is set
///    in `PreUpdate` and consumers poll in `Update`, a re-requesting loop
///    erases the failure before it can observe it — retrying forever and
///    spawning a fresh filesystem task every frame. Retaining the handle also
///    keeps it strong, which nothing else does between frames.
/// 2. **Treat "absent from `Assets<T>`" as inconclusive** and ask
///    `asset_server.load_state(&handle)`; on `LoadState::Failed(e)`, warn and
///    stop. Otherwise a bad path fails silently and permanently, which is
///    worse than `Sync`'s warn-once-and-give-up.
///
/// The resulting shape — request once, retain the handle, poll the retained
/// handle — is what every consumer in this engine implements:
///
/// ```ignore
/// // First frame for this path: request, and keep what `load` returns
/// // somewhere that outlives the frame (a component, or a resource keyed
/// // by path).
/// let handle = bsengine_asset::load(
///     LoadMode::Async, &asset_server, &mut assets, path, sync_loader,
/// )?;
/// pending.insert(path.to_owned(), Pending::Loading(handle));
///
/// // Every later frame: poll the handle that was stored. Never call
/// // `load` (or `AssetServer::load`) for this path again.
/// match assets.get(&handle) {
///     Some(asset) => { /* use it, and drop the pending entry */ }
///     None => {
///         if let LoadState::Failed(e) = asset_server.load_state(&handle) {
///             warn!("{path}: {e}");
///             // Record "gave up", so the next frame does not re-request
///             // it and reset the failure back to `Loading`.
///             pending.insert(path.to_owned(), Pending::GaveUp);
///         }
///     }
/// }
/// ```
///
/// The live implementations are `bsengine_gltf::GltfPlugin` (a per-entity
/// pending component), `bsengine_render::RenderPlugin` (a path-keyed map for
/// custom shaders, a single slot for the skybox) and `bsengine_scripting`'s
/// sound loading. Their pending state is private to each system on purpose:
/// `CustomShader.path`, `SkyboxPath` and friends stay plain `String`s, so
/// scene RON, the scripting API and the MCP tools are unaffected by how a
/// load is tracked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadMode {
    /// Load and insert synchronously, right now, on the calling thread.
    Sync,
    /// Queue an asynchronous load via `AssetServer`. Requires an
    /// `AssetLoader` for `T` to already be registered (via
    /// `app.register_asset_loader(...)`) — see each asset type's own
    /// `*Loader` for the concrete implementation.
    Async,
}

/// Loads `path` as a `T`, dispatching on `mode`. `Sync` runs `sync_loader`
/// directly and inserts via `Assets::add` (no caching/deduplication — every
/// call re-runs `sync_loader`, matching this engine's pre-item-23 behavior,
/// since none of the four migrated call sites cached before either).
/// `Async` calls `AssetServer::load::<T>(path)` and ignores `sync_loader`
/// entirely (the registered `AssetLoader` is used instead).
///
/// # Status recording
///
/// The `Async` arm goes through [`load_async`], so every path requested this
/// way is reported by [`crate::AssetStatuses`] from the moment it is asked
/// for, rather than only if it later fails.
///
/// The `Sync` arm is deliberately **not** recorded. It never touches the
/// `AssetServer` — it runs `sync_loader` and puts the value straight into
/// `Assets<T>` — so there is nothing for `collect_asset_statuses` to observe:
/// the path would sit at `Loading` forever, which is a worse answer than
/// `Unknown` because it looks like an answer. A `Sync` caller already has the
/// verdict in its hand, as the `Result` this returns.
pub fn load<T, E>(
    mode: LoadMode,
    asset_server: &AssetServer,
    assets: &mut Assets<T>,
    path: &str,
    sync_loader: impl FnOnce(&str) -> Result<T, E>,
) -> Result<Handle<T>, String>
where
    T: Asset,
    E: std::fmt::Display,
{
    match mode {
        LoadMode::Sync => {
            let value = sync_loader(path).map_err(|e| format!("{path}: {e}"))?;
            Ok(assets.add(value))
        }
        LoadMode::Async => Ok(load_async::<T>(asset_server, path)),
    }
}

/// Requests `path` asynchronously and records the request, with no
/// `LoadMode` and no `sync_loader` to supply.
///
/// This is [`load`]'s `Async` arm on its own, for the callers that have no
/// synchronous alternative to offer. The skybox is the reason it exists:
/// `upload_pending_skybox` loads a `TextureAsset`, and this codebase has no
/// synchronous texture loader — only `TextureAssetLoader`, for the async
/// path — so calling [`load`] there would mean inventing a `sync_loader`
/// closure that can never run, to satisfy an arm that is never taken. It
/// called `AssetServer::load` directly instead, which is exactly how it
/// escaped status recording.
///
/// Prefer this over `AssetServer::load` anywhere in the engine. The two do the
/// same thing, except that a path requested through `AssetServer::load` is
/// invisible to [`crate::AssetStatuses`] until it fails — and "it loaded" and
/// "nothing ever asked" both read back as [`crate::AssetStatus::Unknown`],
/// which is the ambiguity that let a game run with no mesh and no shader for
/// two phases of work.
///
/// Infallible, and returns the `Handle<T>` rather than a `Result<Handle<T>>`:
/// `AssetServer::load` hands back a handle for any path at all, and whether
/// the file exists is only knowable frames later. Poll the returned handle —
/// see [`LoadMode`] for the shape every consumer in this engine uses, and why
/// re-requesting instead of polling erases the very failure it is looking for.
///
/// # Recovering a path an asset has moved away from
///
/// A path nothing occupies any more, which some asset's sidecar remembers as a
/// path it used to live at, loads from **where that asset is now** — and says
/// so, out loud, every time it is a new path saying it. See this module's
/// private `recover_former_path` for the order, the cost and why the warning is
/// the point rather than a courtesy.
///
/// This is the only route that reaches an asset path spelled inside a
/// JavaScript string literal — `playSound("assets/sounds/hit.wav")`,
/// `Bsengine.setShader(self, "assets/shaders/glow.wgsl")`, the skybox. Nothing
/// can give those an identity, because nothing but the running script knows
/// those characters are a path at all; remembering where the asset went is all
/// there is.
pub fn load_async<T: Asset>(asset_server: &AssetServer, path: &str) -> Handle<T> {
    // Owned either way — `AssetServer::load` takes an owned `AssetPath` — so
    // recovery costs no allocation the previous spelling did not already make.
    let path = recover_former_path(path).unwrap_or_else(|| path.to_owned());
    // The path actually handed to `AssetServer`, not the one the caller
    // spelled: a status is a report of what happened to a *load*, and the
    // stale spelling never becomes one. `AssetStatuses` would in any case
    // refuse it — `collect_asset_statuses` only adopts a requested path its own
    // `AssetServer` recognises, and nothing ever asked that server for this one.
    crate::status::record_asset_request(&path);
    asset_server.load::<T>(path)
}

/// Every project whose [`AssetIndex`] is currently published, keyed by the
/// `ProjectDir` its paths are relative to.
///
/// # Why a process-global side channel, again
///
/// [`load_async`] is handed `&AssetServer` and a path. It has no `World`, no
/// `Commands` and no way to reach a `Resource`, which is the same corner
/// [`crate::status`] argued its way out of and for the same reason: threading
/// the index through would change every call site in four crates and make an
/// ECS resource mandatory for anyone who merely wants to load a file. See
/// [`crate::status::record_asset_request`]'s neighbouring `REQUESTED_PATHS` for
/// the fuller version of that argument — this is the same shape, and it is
/// stated out loud here rather than buried.
///
/// What keeps it from being the dangerous kind of global:
///
/// * It holds only what a [`scan`](crate::identity::scan) read off disk.
///   Nothing here can mint an identity — [`AssetIndex`]'s maps are private and
///   its `insert` is crate-private to the identity module — so the worst a
///   wrong publication can do is fail to recover a path.
/// * **Keyed by project directory, so several `App`s in one process do not
///   overwrite each other.** That is not hypothetical: this crate's test run is
///   a dozen `App`s across parallel test threads, and an editor hosting a game
///   is two `AssetServer`s over one project. A single slot would let one app's
///   publication silently disable another's recovery, which is a flaky test
///   today and a feature that works "sometimes" tomorrow.
/// * A lookup only ever *reads*; the sole writer is
///   [`publish_asset_index`], which `AssetIdentityPlugin` calls.
///
/// # What bounds it
///
/// One entry per distinct project directory the process has published, which is
/// one in every host and one per probe in a test run. Entries are replaced, not
/// accumulated, when the same project publishes again.
static PUBLISHED_INDEXES: LazyLock<RwLock<BTreeMap<String, AssetIndex>>> =
    LazyLock::new(RwLock::default);

/// Stale paths [`recover_former_path`] has already reported, so a load site
/// that fires every frame says it once.
///
/// See [`recover_former_path`] for why silence-after-the-first is the right
/// trade and what it costs.
static REPORTED_RECOVERIES: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Mutex::default);

/// Publishes `index` as the answer [`load_async`] resolves former paths
/// against, for assets under `project_dir`.
///
/// Called by `AssetIdentityPlugin` — once when the scan publishes the index,
/// and again whenever something changes it, which today means
/// [`crate::watcher`] recording a rename that happened while the app was
/// running. Re-publishing replaces this project's entry outright; a project
/// that never publishes is simply not consulted.
///
/// `project_dir` is spelled exactly as the `ProjectDir` resource holds it,
/// because that is what `bsengine_core::resolve_project_path` prefixes onto
/// every path a load site is given, and stripping it back off is how an index
/// keyed project-relative answers a question asked in the engine's own form.
/// An empty `project_dir` — the editor before a project is opened — means the
/// two forms coincide.
pub fn publish_asset_index(project_dir: &str, index: &AssetIndex) {
    let mut published = PUBLISHED_INDEXES
        .write()
        .unwrap_or_else(PoisonError::into_inner);
    match published.get_mut(project_dir) {
        // Compared before cloning: the mirror this is called from re-publishes
        // on any change to the resource, and an index that did not actually
        // change is not worth a copy of every path in the project.
        Some(existing) if existing == index => {}
        Some(existing) => existing.clone_from(index),
        None => {
            published.insert(project_dir.to_owned(), index.clone());
        }
    }
}

/// Where `path` should load from, if `path` is somewhere an asset *used to* be
/// and nothing is there now.
///
/// # The order, and why the disk has the last word
///
/// 1. **Ask the index.** [`AssetIndex::guid_for_former_path`] already refuses a
///    path some asset currently occupies, so a reference that resolves normally
///    never reaches step 2 — it is not a former path at all.
/// 2. **Ask the filesystem.** A file at the spelled path wins over the memory of
///    the one that left it, always. The index is a snapshot taken at `Startup`,
///    so an asset dropped into the vacated name *after* the scan is invisible to
///    step 1; redirecting away from a file that is right there, because of a
///    move recorded before it existed, would be the exact "silently resolves
///    somewhere other than it names" this whole item exists to end.
/// 3. Otherwise: the asset's current location, and a warning.
///
/// # Cost on the ordinary path
///
/// One uncontended `RwLock` read and, per published project, a prefix check plus
/// one `BTreeMap` lookup that misses. **No syscall**: the `exists` check is
/// behind step 1, so a path that is not a former path of anything never touches
/// the disk. There is one published project in every host, and this crate's own
/// index lookups are `BTreeMap`s over a project's assets.
///
/// Only a genuine recovery — a path the index knows as former — pays a `stat`,
/// and only that one path.
///
/// # Why it warns, and why only once per path
///
/// Never silently. A path that resolves somewhere other than what it spells is
/// the ambiguity item 30 exists to remove, and a recovery nobody is told about
/// converts a broken reference into a permanent, invisible indirection layer —
/// which is precisely the accumulated-forwarding pain Unreal documents. This is
/// a development-time affordance with an expiry, and the warning is what makes
/// somebody go and spend it.
///
/// Once per path, though, because [`load_async`] is reachable from script
/// commands that can fire every frame: `playSound` on a stale path inside an
/// `onUpdate` would otherwise print sixty lines a second and bury the log it
/// exists to appear in. The set is keyed by the path as spelled, exactly as
/// `REQUESTED_PATHS` is, so a *different* stale reference still gets its own
/// line — what is suppressed is repetition, not information. Two `App`s in one
/// process share the suppression; a diagnostic that has already been printed
/// has already done its job, unlike a *status*, which is why
/// [`crate::status`]'s set is read rather than claimed and this one is claimed.
fn recover_former_path(path: &str) -> Option<String> {
    let (guid, current) = {
        let published = PUBLISHED_INDEXES
            .read()
            .unwrap_or_else(PoisonError::into_inner);
        published.iter().find_map(|(project_dir, index)| {
            let relative = project_relative(project_dir, path)?;
            let guid = index.guid_for_former_path(relative)?;
            let current = index.path_for_guid(guid)?;
            Some((guid, engine_form(project_dir, current)))
        })?
    };

    if Path::new(path).exists() {
        return None;
    }

    report_recovery(path, guid, &current);
    Some(current)
}

/// Strips `project_dir` off an engine-form path, giving the project-relative
/// spelling an [`AssetIndex`] is keyed by — the exact inverse of
/// `bsengine_core::resolve_project_path`, which is what put it there.
///
/// `None` when `path` is not under this project at all, which is how a process
/// holding two projects' indexes asks each one only about its own paths.
fn project_relative<'a>(project_dir: &str, path: &'a str) -> Option<&'a str> {
    if project_dir.is_empty() {
        return Some(path);
    }
    path.strip_prefix(project_dir)?.strip_prefix('/')
}

/// Puts `project_dir` back on, so the answer is spelled the way the caller
/// spelled the question — and the way `AssetServer` keys everything else.
fn engine_form(project_dir: &str, relative: &str) -> String {
    if project_dir.is_empty() {
        return relative.to_owned();
    }
    format!("{project_dir}/{relative}")
}

/// Says a recovery happened, the first time this path needs saying.
fn report_recovery(stale: &str, guid: AssetGuid, current: &str) {
    {
        let mut reported = REPORTED_RECOVERIES
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        // Checked before inserting so a per-frame load site costs a hash lookup
        // rather than a `String` allocation `HashSet::insert` throws away.
        if reported.contains(stale) {
            return;
        }
        reported.insert(stale.to_owned());
        // Poisoning is recovered from rather than propagated, as in
        // `crate::status`: what is behind this lock is a note about what has
        // already been printed, and there is no invariant a panic could have
        // broken. Turning every later asset load in the process into a panic
        // of its own would be a far worse trade.
    }
    tracing::warn!(
        "asset: nothing is at '{stale}' any more — asset {guid} used to live there and is now at \
         '{current}', so that is what will load. Update the reference that still names the old \
         path: recovering through a former path is a development-time convenience, not a \
         permanent redirect"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_asset::{Asset, AssetPlugin, Assets};
    use bevy_reflect::TypePath;
    use bsengine_app::new_app;

    #[derive(Asset, TypePath, Debug, PartialEq)]
    struct DummyAsset(String);

    #[test]
    fn load_sync_inserts_and_returns_handle() {
        let mut app = new_app();
        app.add_plugins(AssetPlugin::default());
        app.init_resource::<Assets<DummyAsset>>();
        let server = app.world().resource::<AssetServer>().clone();
        let mut assets = app.world_mut().resource_mut::<Assets<DummyAsset>>();
        let handle = load(LoadMode::Sync, &server, &mut assets, "fake/path.txt", |p| {
            Ok::<_, String>(DummyAsset(format!("loaded:{p}")))
        })
        .unwrap();
        assert_eq!(
            assets.get(&handle),
            Some(&DummyAsset("loaded:fake/path.txt".to_string()))
        );
    }

    // ---- resolving through a former path (roadmap item 30, sub-item D) ----
    //
    // This is the funnel every asset path spelled inside a JavaScript string
    // literal comes through — `playSound("assets/sounds/hit.wav")`,
    // `Bsengine.setShader(self, "assets/shaders/glow.wgsl")`, the skybox.
    // Sub-item B gave *scene* references an identity; nothing can give a string
    // literal one, so remembering where the asset went is the only mechanism
    // that reaches them.
    //
    // Every test drives the real thing: a real project directory, a real scan
    // that mints and then recovers a real sidecar, the real `AssetIdentityPlugin`
    // publishing the index, and `load_async` itself. The warnings are asserted
    // as text, because a path that silently resolves somewhere other than it
    // names is exactly the ambiguity this item exists to remove — a recovery
    // nobody is told about is a permanent invisible indirection layer, not a
    // feature.
    mod former_paths {
        use super::*;
        use crate::identity::{scan, AssetIdentityPlugin, AssetIndex};
        use crate::plugin::AssetPlugin;
        use crate::test_support::{capture_warnings, unique, ProbeDir};
        use crate::types::TextureAsset;
        use bevy_asset::Assets;
        use bsengine_core::ProjectDir;
        use std::path::PathBuf;
        use std::time::{Duration, Instant};

        /// Where the probe asset was when the reference to it was written, and
        /// where it is now. They share no substring, so an assertion that a
        /// warning names one cannot be satisfied by the other.
        const FORMER: &str = "assets/textures/logo.png";
        const CURRENT: &str = "assets/textures/emblem.png";

        /// A hang guard, not a budget: what is being waited for is one tiny
        /// local file being read.
        const DEADLINE: Duration = Duration::from_secs(10);

        /// The phrase only a recovery emits.
        ///
        /// The "silent" assertions look for this rather than for an empty log,
        /// because `bevy_asset` itself logs at `ERROR` for a path that does not
        /// exist — which is precisely what a load of a stale path *should*
        /// produce once nothing recovers it, and asserting on an empty log
        /// would make those tests pass for that reason instead of for theirs.
        const RECOVERY_PHRASE: &str = "used to live there";

        fn png_bytes() -> Vec<u8> {
            let image = image::RgbaImage::from_pixel(1, 1, image::Rgba([7, 8, 9, 255]));
            let mut buffer = std::io::Cursor::new(Vec::new());
            image
                .write_to(&mut buffer, image::ImageFormat::Png)
                .expect("encode probe png");
            buffer.into_inner()
        }

        /// A project holding one asset that has been renamed *with its sidecar
        /// left behind* — what `git mv`, Explorer and an artist's export script
        /// all do — so that the scan's own orphan recovery is what records the
        /// former path. Nothing here writes `former_paths` by hand.
        ///
        /// The directory is under the process CWD rather than the temp dir
        /// because the asset root is the CWD (see [`crate::plugin`]), and a
        /// path outside it is not addressable as an asset path at all. That is
        /// what lets these tests prove the recovered path really reaches disk
        /// rather than only that a string was rewritten. `.gitignore` covers
        /// the name [`unique`] produces.
        fn moved_asset_project(tag: &str) -> (String, ProbeDir) {
            let project = unique(tag);
            let root = PathBuf::from(&project);
            let guard = ProbeDir(root.clone());

            let former = root.join(FORMER);
            std::fs::create_dir_all(former.parent().expect("probe asset has a parent"))
                .expect("create probe directories");
            std::fs::write(&former, png_bytes()).expect("write probe asset");
            // Mints `logo.png.meta` where the asset started.
            scan(&root).expect("scan the probe project");
            std::fs::rename(&former, root.join(CURRENT)).expect("rename the probe asset");

            (project, guard)
        }

        /// An app over `project` with the index published, exactly as a host
        /// builds one.
        fn app_with_index(project: &str) -> bsengine_app::App {
            let mut app = new_app();
            app.insert_resource(ProjectDir(project.to_string()));
            app.add_plugins((AssetPlugin, AssetIdentityPlugin));
            app.update();
            app
        }

        /// Runs frames until `done`, or gives up after [`DEADLINE`].
        fn run_until(
            app: &mut bsengine_app::App,
            done: impl Fn(&bsengine_app::App) -> bool,
        ) -> bool {
            let deadline = Instant::now() + DEADLINE;
            loop {
                app.update();
                if done(app) {
                    return true;
                }
                if Instant::now() >= deadline {
                    return false;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        }

        /// The property, end to end: a reference to a path the asset has left
        /// loads the asset, and the developer is told.
        #[test]
        fn a_path_that_only_a_former_path_answers_loads_from_where_the_asset_went() {
            let (project, _guard) = moved_asset_project("former-load");
            let mut app = app_with_index(&project);
            assert!(
                app.world()
                    .resource::<AssetIndex>()
                    .guid_for_former_path(FORMER)
                    .is_some(),
                "precondition: the scan's orphan recovery must have recorded the \
                 move, or this test measures nothing"
            );

            let stale = format!("{project}/{FORMER}");
            let current = format!("{project}/{CURRENT}");
            assert!(
                !PathBuf::from(&stale).exists(),
                "precondition: the whole point is that this path is gone"
            );

            let server = app.world().resource::<AssetServer>().clone();
            let (handle, logs) = capture_warnings(|| load_async::<TextureAsset>(&server, &stale));

            assert_eq!(
                server.get_path(handle.id()).map(|p| p.to_string()),
                Some(current.clone()),
                "the load must have been made against where the asset is now"
            );
            assert!(
                run_until(&mut app, |app| app
                    .world()
                    .resource::<Assets<TextureAsset>>()
                    .get(&handle)
                    .is_some()),
                "the recovered path must actually reach the disk — a redirect \
                 that only rewrites a string is a redirect to nothing"
            );

            assert!(
                logs.contains(&stale),
                "a load that resolved somewhere other than what it was asked for \
                 must name what it was asked for, or the developer cannot find \
                 the reference to fix. Got: {logs}"
            );
            assert!(
                logs.contains(&current),
                "and must name what it loaded instead, or the warning reports a \
                 problem with no way to check it. Got: {logs}"
            );
        }

        /// The collision: an asset is renamed away, and then something new is
        /// created at the name it left. The file that is *there* wins over the
        /// memory of the one that left — every time, and without a word,
        /// because nothing is stale about a path that resolves.
        ///
        /// The new file is created *after* the scan on purpose. `AssetIndex`
        /// already refuses a former path some indexed asset occupies, so a
        /// newcomer the scan saw would be caught one layer up and this would
        /// prove nothing about the layer that has to catch the rest: the index
        /// is a `Startup` snapshot, and files appear after it.
        #[test]
        fn a_file_at_the_old_name_beats_the_memory_of_the_one_that_left() {
            let (project, _guard) = moved_asset_project("former-collision");
            let app = app_with_index(&project);
            assert!(
                app.world()
                    .resource::<AssetIndex>()
                    .guid_for_former_path(FORMER)
                    .is_some(),
                "precondition: the move must be recorded, or there is no memory \
                 for the new file to beat"
            );

            let stale = format!("{project}/{FORMER}");
            std::fs::write(&stale, png_bytes()).expect("write the newcomer");

            let server = app.world().resource::<AssetServer>().clone();
            let (handle, logs) = capture_warnings(|| load_async::<TextureAsset>(&server, &stale));

            assert_eq!(
                server.get_path(handle.id()).map(|p| p.to_string()),
                Some(stale.clone()),
                "a file that exists at the spelled path is the asset that was \
                 asked for; redirecting away from it because of a move recorded \
                 before it existed would load a real, wrong file"
            );
            assert!(
                !logs.contains(RECOVERY_PHRASE),
                "and nothing is stale here, so there is nothing to report. Got: {logs}"
            );
        }

        /// Recovery is loud once, not sixty times a second.
        ///
        /// `load_async` is reachable from script commands that fire every frame
        /// — a `playSound` on a stale path inside an `onUpdate` — and a warning
        /// per call would bury the log it exists to appear in. What must *not*
        /// be suppressed is the resolution itself: every call still loads from
        /// where the asset went.
        #[test]
        fn a_recovered_path_is_reported_once_however_often_it_is_loaded() {
            let (project, _guard) = moved_asset_project("former-repeat");
            let app = app_with_index(&project);

            let stale = format!("{project}/{FORMER}");
            let current = format!("{project}/{CURRENT}");
            let server = app.world().resource::<AssetServer>().clone();

            let (handles, logs) = capture_warnings(|| {
                (0..60)
                    .map(|_| load_async::<TextureAsset>(&server, &stale))
                    .collect::<Vec<_>>()
            });

            assert_eq!(
                logs.matches(&stale as &str).count(),
                1,
                "sixty loads produced more than one warning. Got: {logs}"
            );
            assert!(
                handles.iter().all(
                    |h| server.get_path(h.id()).map(|p| p.to_string()) == Some(current.clone())
                ),
                "suppressing the *warning* must not suppress the recovery: every \
                 one of those loads still has to reach the asset"
            );
        }

        /// The ordinary case stays ordinary. A path that resolves normally is
        /// loaded exactly as spelled, with no recovery and nothing said — in a
        /// project that has former paths recorded, so the lookup really is
        /// being consulted and really is declining to answer.
        #[test]
        fn a_path_that_resolves_normally_is_untouched_and_silent() {
            let (project, _guard) = moved_asset_project("former-quiet");
            let app = app_with_index(&project);

            let current = format!("{project}/{CURRENT}");
            let server = app.world().resource::<AssetServer>().clone();
            let (handle, logs) = capture_warnings(|| load_async::<TextureAsset>(&server, &current));

            assert_eq!(
                server.get_path(handle.id()).map(|p| p.to_string()),
                Some(current.clone())
            );
            assert!(!logs.contains(RECOVERY_PHRASE), "Got: {logs}");
        }

        /// An app that never publishes an index — most of this workspace's
        /// tests, and anything that adds `AssetPlugin` without
        /// `AssetIdentityPlugin` — loads what it was given. There is nothing to
        /// resolve against, which is not a fault to report.
        #[test]
        fn without_a_published_index_a_path_is_loaded_exactly_as_spelled() {
            let (project, _guard) = moved_asset_project("former-no-index");
            let mut app = new_app();
            app.add_plugins(AssetPlugin);
            app.update();

            let stale = format!("{project}/{FORMER}");
            let server = app.world().resource::<AssetServer>().clone();
            let (handle, logs) = capture_warnings(|| load_async::<TextureAsset>(&server, &stale));

            assert_eq!(
                server.get_path(handle.id()).map(|p| p.to_string()),
                Some(stale.clone())
            );
            assert!(!logs.contains(RECOVERY_PHRASE), "Got: {logs}");
        }
    }

    #[test]
    fn load_sync_propagates_loader_error() {
        let mut app = new_app();
        app.add_plugins(AssetPlugin::default());
        app.init_resource::<Assets<DummyAsset>>();
        let server = app.world().resource::<AssetServer>().clone();
        let mut assets = app.world_mut().resource_mut::<Assets<DummyAsset>>();
        let err = load(LoadMode::Sync, &server, &mut assets, "bad/path.txt", |_| {
            Err::<DummyAsset, _>("boom".to_string())
        })
        .unwrap_err();
        assert!(err.contains("bad/path.txt"));
        assert!(err.contains("boom"));
    }
}
