//! [`AssetStatuses`]: "what happened to this asset?", answerable from code.
//!
//! # Why this exists
//!
//! A failed asset load in `bevy_asset` produces a `LoadState::Failed` on an
//! `AssetInfo` nobody is looking at, and — at best — a `warn!` line. That has
//! now cost this engine three separate incidents:
//!
//! * [`crate::plugin::AssetPlugin`] resolved every path against the wrong
//!   root, so `games/mini-arena` ran with no fox mesh and no glow shader for
//!   two phases of work. The only symptom was a `WARN` nobody read.
//! * A missing glTF retried silently forever until a give-up warning was
//!   added (see [`crate::load_mode`]).
//! * [`crate::watcher`] would have reloaded nothing at all on a path-spelling
//!   mismatch, emitting no output whatsoever.
//!
//! Each time, the engine *knew*. It just had no way to say so to anything but
//! a log. This module records what it knew, keyed by the path the caller
//! spelled, so that "nothing happened" becomes distinguishable from "it
//! failed, and here is why".
//!
//! # What it costs
//!
//! [`collect_asset_statuses`] re-reads every recorded path once per frame:
//! one `AssetServer::get_path_id` (an `RwLock` read plus two hash lookups)
//! and one `load_state` (the same again) per entry, whether or not anything
//! changed. That is deliberate — a status that is only refreshed on an event
//! goes stale exactly in the cases this module exists to catch — but it does
//! make the system O(recorded paths) every frame, so the map is not a place
//! to put a path per *entity*. See [`AssetStatuses`] for what bounds it.
//!
//! It also takes one uncontended `Mutex` lock per frame to drain
//! [`record_asset_request`]'s channel, and a load site pays one more lock plus
//! (for a path not already pending) one `String` allocation per *request* —
//! not per frame, because every consumer in this engine requests a path once
//! and then polls the handle it kept.

use bevy_app::{App, Plugin, Update};
use bevy_asset::{AssetServer, LoadState, UntypedAssetLoadFailedEvent};
use bevy_ecs::prelude::{EventReader, Res, ResMut, Resource};
use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex, PoisonError};

/// What the engine knows about one asset path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetStatus {
    /// Never requested. Distinct from bevy's `NotLoaded`: nothing ever asked.
    Unknown,
    /// Requested; still resolving.
    Loading,
    /// Resolved and available.
    Loaded,
    /// The load failed, with the reason as `bevy_asset` reported it.
    Failed(String),
}

/// Every asset path the engine has something to say about, keyed by the path
/// string as it was spelled at the load site.
///
/// # Why the map is private
///
/// A status is a *finding*, not a setting. If callers could insert into this
/// map, a script or an editor panel could report "Loaded" for a path that
/// never loaded, which is precisely the failure mode the whole module exists
/// to remove. Entries are therefore only ever written by
/// [`collect_asset_statuses`], from `bevy_asset`'s own state; readers get
/// [`AssetStatuses::get`].
///
/// # What bounds its growth
///
/// One entry per *distinct path* the engine has requested, plus one per
/// distinct path that has failed to load without going through a request this
/// crate saw. Paths come from scenes, script calls and shader/audio
/// references — a fixed set for a given project, not something that grows with
/// entity count or with time. A path is recorded once no matter how many times
/// it is requested or retried, because the path is the key.
///
/// The one way to defeat that is to generate fresh path strings at runtime
/// (e.g. `format!("assets/tile_{i}.png")` over an unbounded `i`). Nothing in
/// this engine does, and a caller that did would already be leaking
/// `AssetInfo`s inside `bevy_asset` itself.
///
/// Entries are never removed. A path that failed stays reported as failed
/// until something loads it successfully — see [`collect_asset_statuses`] for
/// why that is the wanted behaviour and not a leak of stale state.
#[derive(Resource, Debug, Default)]
pub struct AssetStatuses {
    by_path: HashMap<String, AssetStatus>,
}

impl AssetStatuses {
    /// What is known about `path`, or [`AssetStatus::Unknown`] if nothing ever
    /// asked for it.
    ///
    /// `path` must be spelled the way the load site spelled it — these are the
    /// engine-form, CWD-relative, forward-slashed paths that
    /// `bsengine_core::resolve_project_path` produces, not canonicalised or
    /// absolute ones. `bevy_asset` keys its own tables the same way (which is
    /// why [`crate::watcher`] has to reconstruct that exact spelling), so a
    /// mismatched spelling reads as `Unknown` rather than as an error.
    pub fn get(&self, path: &str) -> AssetStatus {
        self.by_path
            .get(path)
            .cloned()
            .unwrap_or(AssetStatus::Unknown)
    }

    /// Every path this engine has something to say about, paired with what it
    /// knows, in no particular order.
    ///
    /// # Why this does not reopen the map
    ///
    /// [`Self::get`] is private-map-shaped for a reason (see the type docs):
    /// a status is a finding, and a caller that could write one could report
    /// `Loaded` for an asset that never loaded. Handing out `&str` keys and
    /// `&AssetStatus` values keeps that intact — there is no way back through
    /// a shared reference to insert, remove or overwrite an entry.
    ///
    /// # Why it is needed at all
    ///
    /// [`Self::get`] can only answer about a path the caller already knows to
    /// name, and `bevy_asset` exposes no way to enumerate the paths it knows.
    /// A consumer that has to *mirror* the whole picture rather than ask
    /// about one path has nowhere else to get it — concretely,
    /// `bsengine_scripting` refreshes a thread-local copy of this map every
    /// frame because V8 runs its scripts on a thread that cannot touch the
    /// `World` at all, so `Bsengine.getAssetStatus` has no `World` to consult
    /// at the moment a script asks.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &AssetStatus)> {
        self.by_path
            .iter()
            .map(|(path, status)| (path.as_str(), status))
    }
}

/// Paths that [`record_asset_request`] has been told about and no
/// [`collect_asset_statuses`] run has claimed yet.
///
/// # Why a process-global side channel, stated plainly
///
/// [`crate::load`] is handed `&AssetServer` and `&mut Assets<T>`. It has no
/// `World`, no `Commands` and no way to reach a second `Resource`, so the
/// recording has to travel out of it somehow. The two candidates were:
///
/// * **Thread `AssetStatuses` through the signature.** Every call site
///   changes, and every *system* that loads anything grows another
///   `SystemParam` — including `bsengine_scripting`'s, which reaches `load`
///   from an exclusive-`World` op and would have to fetch the resource by
///   hand, and `bsengine_render`'s, which is already crowded with the
///   16-parameter ceiling in mind. It would also make the resource mandatory
///   for anyone who merely wants to load a file.
/// * **A process-global channel** that the load funnels push to and the
///   collector drains. Call sites are untouched; an app that never adds
///   [`AssetStatusPlugin`] never notices.
///
/// This is the second one. It **is** global mutable state — said out loud
/// rather than buried, because that is the cost. What keeps it from being the
/// dangerous kind:
///
/// * It carries *requests*, never verdicts. A claimed path enters
///   [`AssetStatuses`] as [`AssetStatus::Loading`] and is immediately
///   re-read from the `AssetServer`, so nothing here can publish "Loaded" for
///   an asset that never loaded — the property [`AssetStatuses`]'s private
///   map exists to protect.
/// * It is a `HashSet`, so a path requested a thousand times is one entry and
///   costs one lock and no allocation on every request after the first.
/// * Several `App`s in one process do not steal each other's requests. That
///   is not hypothetical: this crate's own test run is a dozen `App`s across
///   parallel test threads. [`collect_asset_statuses`] claims only paths its
///   *own* `AssetServer` knows about and leaves the rest in place.
///
/// # What bounds it
///
/// In a running engine it empties every frame: `AssetServer::load` creates
/// the path's `AssetInfo` before it returns, so the very next collector run
/// recognises and claims it.
///
/// An app that adds `AssetPlugin` but not [`AssetStatusPlugin`] never drains
/// it at all. That is bounded by the number of *distinct* paths that app
/// requests — the same bound [`AssetStatuses`] itself has, and for the same
/// reason (see there): nothing in this engine synthesises fresh path strings
/// at runtime. The residue is a `String` per path, and it is never walked by
/// anybody, because the only thing that walks it is the collector that app
/// does not have.
///
/// The one entry that can linger in an app that *does* collect is a path
/// whose handle was dropped before the next `Update` — the `AssetInfo` is
/// gone, so no collector recognises it. Same bound, and the path is one
/// nobody kept a handle to.
static REQUESTED_PATHS: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Mutex::default);

/// Notes that something asked for `path`, so that [`AssetStatuses`] can report
/// on it before — and whether or not — anything goes wrong.
///
/// This is the half `bevy_asset` cannot provide: `UntypedAssetLoadFailedEvent`
/// tells us about paths that *failed*, but there is no API anywhere on
/// `AssetServer` to enumerate the paths it knows (`AssetInfos`' map is
/// private). Without this, a path that loaded fine and a path nobody ever
/// mentioned are the same answer — `Unknown` — which is the ambiguity this
/// module exists to remove.
///
/// Deliberately **not** `pub`: the only ways in are [`crate::load`] and
/// [`crate::load_async`], which record what they are about to hand to
/// `AssetServer::load`. A caller that could record freely could report
/// `Loading` for a path nothing ever requested, and the answer would be worth
/// no more than the log line it replaces. Consumers outside this crate that
/// need their loads tracked call [`crate::load_async`] instead of
/// `AssetServer::load`.
///
/// Poisoning is recovered from rather than propagated. A panic elsewhere while
/// this lock was held must not turn every later asset load in the process into
/// a panic of its own — and what is behind the lock is a set of *notes to
/// look at a path*, every one of which the collector re-checks against the
/// `AssetServer` before it means anything. There is no invariant here for a
/// panic to have broken.
pub(crate) fn record_asset_request(path: &str) {
    let mut requested = REQUESTED_PATHS
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    // Checked before inserting so a repeat request costs a hash lookup rather
    // than a `String` allocation that `HashSet::insert` would immediately
    // throw away.
    if !requested.contains(path) {
        requested.insert(path.to_owned());
    }
}

/// Folds this frame's asset-load failures, the paths requested since the last
/// run, and the `AssetServer`'s current per-path load states into
/// [`AssetStatuses`].
///
/// Runs in `Update`, which is after the `PreUpdate` system where `bevy_asset`
/// turns its internal load results into `LoadState` changes and
/// `UntypedAssetLoadFailedEvent`s — so a load that resolves during frame N is
/// visible in [`AssetStatuses`] within frame N.
///
/// One system covers every asset type. `UntypedAssetLoadFailedEvent` is
/// registered once by `bevy_asset::AssetPlugin`, not per type, and carries the
/// path and the error, so glTF, shaders, textures and audio are all handled
/// here without this crate knowing those types exist.
///
/// # Why requests have to be pushed in
///
/// Failures arrive on their own. Everything else does not: `bevy_asset`
/// exposes no way to ask an `AssetServer` which paths it knows, so this system
/// can refresh paths it has already heard of but can never *discover* one.
/// [`record_asset_request`] is where it hears of them — see there for what
/// that costs.
///
/// # Failure is sticky, and a successful load is what clears it
///
/// The hard case is telling "the same asset that already failed" apart from
/// "a fresh attempt is under way", because both can read back as
/// `LoadState::NotLoaded`. Resolved against `bevy_asset-0.14.2`:
///
/// * `Failed` is **never** overwritten by `NotLoaded` in place. The only
///   assignments to `load_state` are `Loading` (`server/info.rs:140`, `:217`),
///   `Loaded` (`server/info.rs:478`) and `Failed` (`server/info.rs:594`).
///   `NotLoaded` is only ever `AssetInfo::new`'s initial value
///   (`server/info.rs:47`), or the
///   `unwrap_or(LoadState::NotLoaded)` default `AssetServer::load_state`
///   returns for an id it has no info for (`server/mod.rs:917`).
/// * So when a recorded path reads back `NotLoaded`, what actually happened is
///   that `get_path_id` found nothing: the last handle was dropped and
///   `track_assets` evicted the `AssetInfo`. That is the same failed asset
///   with nobody holding it any more — emphatically not a fresh unknown.
/// * A **successful reload does** overwrite it. `process_asset_load` assigns
///   `LoadState::Loaded` unconditionally (`server/info.rs:478`), so the moment
///   the fixed file loads, the path reads `Loaded`.
/// * A **re-request** of a failed path additionally passes through `Loading`
///   first, because `get_or_create_path_handle_internal` resets
///   `Failed(_) -> Loading` under `HandleLoadingMode::Request`
///   (`server/info.rs:213-219`). `AssetServer::reload` does *not*: it passes
///   the existing handle straight to `load_internal`, skipping that reset
///   (`server/mod.rs:529-533`), so a reload goes `Failed -> Loaded` with no
///   intermediate `Loading`. Both routes end somewhere that is not
///   `NotLoaded`.
///
/// The rule that falls out is therefore uniform, with no `Failed`-specific
/// special case: **`NotLoaded` carries no information and never overwrites
/// anything**; `Loading`, `Loaded` and `Failed` each do. A failure survives
/// its handle being dropped, and is cleared the instant the path loads.
///
/// # Known limitation
///
/// A path that failed, whose handle was then dropped, and which is never
/// requested again keeps reporting `Failed` forever, even if the file is
/// fixed on disk. Nothing asked, so nothing looked, and `bevy_asset` exposes
/// no way to enumerate or re-check a path it is no longer tracking. Reporting
/// the last thing actually observed is the honest answer here; the alternative
/// — silently downgrading it to `Unknown` — is the exact "nothing happened"
/// ambiguity this module removes.
pub fn collect_asset_statuses(
    mut failures: EventReader<UntypedAssetLoadFailedEvent>,
    asset_server: Res<AssetServer>,
    mut statuses: ResMut<AssetStatuses>,
) {
    for event in failures.read() {
        statuses.by_path.insert(
            event.path.to_string(),
            AssetStatus::Failed(event.error.to_string()),
        );
    }

    // Paths requested since the last run join the map here, ahead of the
    // refresh below, so a path requested this frame reports its real state
    // this frame rather than one frame late.
    {
        let mut requested = REQUESTED_PATHS
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        requested.retain(|path| {
            // Whether *this* app's `AssetServer` knows the path is what says
            // whose request it was. Several `App`s share this channel (this
            // crate's own test run is exactly that), and claiming a path this
            // server never heard of would both steal it from the app that did
            // request it and park it here as a `Loading` that can never
            // resolve, since every later refresh would read `NotLoaded`.
            if asset_server.get_path_id(path).is_none() {
                return true;
            }
            // `or_insert`, not `insert`: a request must never overwrite a
            // verdict. A re-request of a failed path does move it back to
            // `Loading` — but by way of `bevy_asset` actually resetting the
            // load state, which the refresh below reads, not because the
            // caller said so.
            statuses
                .by_path
                .entry(path.clone())
                .or_insert(AssetStatus::Loading);
            false
        });
    }

    for (path, status) in statuses.by_path.iter_mut() {
        // `&String` rather than `&str` on purpose: `AssetPath`'s `&str`
        // conversion is `From<&'static str>`, so the borrowed-key spelling is
        // the only one that compiles without cloning every path every frame.
        let state = asset_server
            .get_path_id(path)
            .map_or(LoadState::NotLoaded, |id| asset_server.load_state(id));

        match state {
            // Carries no information — it is both "never started" and "the
            // AssetInfo is gone". Whatever we last observed is still the best
            // answer, which is what keeps a failure from being erased by its
            // own handle being dropped.
            LoadState::NotLoaded => {}
            LoadState::Loading => *status = AssetStatus::Loading,
            LoadState::Loaded => *status = AssetStatus::Loaded,
            LoadState::Failed(error) => *status = AssetStatus::Failed(error.to_string()),
        }
    }
}

/// Records what became of every asset the engine tried to load, into
/// [`AssetStatuses`].
///
/// Requires `bevy_asset`'s own `AssetPlugin` — which
/// [`crate::plugin::AssetPlugin`] installs — for the `AssetServer` resource
/// and for the `UntypedAssetLoadFailedEvent` registration. Beyond that it
/// depends on nothing in this engine, so it can be added to any `bevy_asset`
/// app on its own.
pub struct AssetStatusPlugin;

impl Plugin for AssetStatusPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AssetStatuses>()
            .add_systems(Update, collect_asset_statuses);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::AssetPlugin;
    use crate::types::TextureAsset;
    use bsengine_app::new_app;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    /// How long a probe waits for a filesystem-backed load to resolve before
    /// giving up. Loads here are a single missing/tiny local file, so this is
    /// a hang guard rather than a budget.
    const DEADLINE: Duration = Duration::from_secs(10);

    /// Removes its directory on drop, including when the test panics.
    struct ProbeDir(PathBuf);

    impl Drop for ProbeDir {
        fn drop(&mut self) {
            for _ in 0..20 {
                if std::fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }

    /// A probe directory name, relative to the crate root.
    ///
    /// It has to be **under the CWD** rather than in the temp dir: the asset
    /// root is the process CWD (see [`crate::plugin`]), so a path outside it
    /// is not addressable as an asset path at all.
    fn unique(tag: &str) -> String {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        format!(
            "bsengine-status-probe-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        )
    }

    /// Runs frames until `done` accepts the status of `path`, or [`DEADLINE`]
    /// passes. Returns whatever the status was at the end either way, so the
    /// caller's assertion reports the real value rather than a timeout.
    fn pump_until(
        app: &mut bsengine_app::App,
        path: &str,
        done: impl Fn(&AssetStatus) -> bool,
    ) -> AssetStatus {
        let deadline = Instant::now() + DEADLINE;
        loop {
            app.update();
            let status = app.world().resource::<AssetStatuses>().get(path);
            if done(&status) || Instant::now() >= deadline {
                return status;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn write_png(path: &std::path::Path) {
        image::RgbaImage::from_pixel(1, 1, image::Rgba([1, 2, 3, 255]))
            .save(path)
            .unwrap();
    }

    /// Requests `path` the way the engine's consumers do — through
    /// [`crate::load`], not by reaching for the `AssetServer` directly. The
    /// difference is the whole point of these two tests: a status that only
    /// appears when someone calls `AssetServer::load` by hand would report
    /// nothing about the paths the engine actually loads.
    fn request_texture(
        app: &mut bsengine_app::App,
        path: &str,
    ) -> bevy_asset::Handle<TextureAsset> {
        let server = app.world().resource::<AssetServer>().clone();
        let mut assets = app
            .world_mut()
            .resource_mut::<bevy_asset::Assets<TextureAsset>>();
        crate::load_mode::load(
            crate::load_mode::LoadMode::Async,
            &server,
            &mut assets,
            path,
            // Never called: `LoadMode::Async` ignores the sync loader, and
            // there is no synchronous texture loader in this codebase.
            |p| Err::<TextureAsset, String>(format!("no synchronous texture loader for {p}")),
        )
        .expect("LoadMode::Async is infallible")
    }

    /// The success direction, and the one `bevy_asset` cannot supply by
    /// itself: a path that loaded must say so.
    ///
    /// Failures announce themselves through `UntypedAssetLoadFailedEvent`;
    /// nothing announces a success, and there is no API to enumerate the paths
    /// an `AssetServer` knows. So without recording the request, "it loaded"
    /// and "nothing ever asked" are the same answer — `Unknown` — which is
    /// exactly the ambiguity that let a game run with no mesh and no shader.
    #[test]
    fn a_successful_load_through_the_engine_funnel_is_reported_as_loaded() {
        let dir = unique("loaded");
        let _guard = ProbeDir(PathBuf::from(&dir));
        std::fs::create_dir_all(&dir).unwrap();
        let rel = format!("{dir}/present-texture.png");
        write_png(&PathBuf::from(&rel));

        let mut app = new_app();
        app.add_plugins((AssetPlugin, AssetStatusPlugin));

        // Retained for the whole test, as every consumer in this engine
        // retains it: dropping it lets `track_assets` evict the `AssetInfo`,
        // which is a different property entirely.
        let _handle = request_texture(&mut app, &rel);

        let status = pump_until(&mut app, &rel, |s| matches!(s, AssetStatus::Loaded));
        assert_eq!(
            status,
            AssetStatus::Loaded,
            "a file that exists and loads must report Loaded; anything else means \
             a successful load is indistinguishable from one nobody requested"
        );
    }

    /// Recording a *request* must not be mistaken for recording a *result*:
    /// a path whose file does not exist goes through the same funnel, and
    /// must never read `Loaded` on any frame on its way to `Failed`.
    ///
    /// A guard, not the distinguishing test — that is the one above. Neither
    /// assertion here can fail merely because request-time recording is
    /// missing: a local miss usually fails within the first frame, so the
    /// failure event alone puts the path on the map. What this pins is the
    /// direction the *implementation* could go wrong in — reporting a request
    /// as a result, which would call every broken path in a project fine.
    #[test]
    fn a_requested_path_that_cannot_load_is_never_reported_as_loaded() {
        let missing = format!("{}/never-arrives.png", unique("unresolved"));

        let mut app = new_app();
        app.add_plugins((AssetPlugin, AssetStatusPlugin));
        let _handle = request_texture(&mut app, &missing);

        // One frame is enough to be *known*: `AssetServer::load` creates the
        // `AssetInfo` before it returns, so the first collector run already
        // recognises the path as its own.
        app.update();
        assert_ne!(
            app.world().resource::<AssetStatuses>().get(&missing),
            AssetStatus::Unknown,
            "a requested path must be reported from the frame after it is asked \
             for, not only once it fails"
        );

        // Whether the failure has landed yet is a race with the filesystem,
        // so every state except `Loaded` is allowed on the way — `Loaded` is
        // allowed on no frame at all, because this file does not exist.
        let deadline = Instant::now() + DEADLINE;
        loop {
            app.update();
            let status = app.world().resource::<AssetStatuses>().get(&missing);
            assert_ne!(
                status,
                AssetStatus::Loaded,
                "a path whose file does not exist must never read Loaded — \
                 recording a request is not recording a result"
            );
            if matches!(status, AssetStatus::Failed(_)) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "the requested load never resolved, so this proves nothing; last \
                 status was {status:?}"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// The failure direction: a path that cannot possibly resolve ends up
    /// `Failed`, carrying a reason that actually names the problem.
    ///
    /// Drives the real plugins and a real `AssetServer::load`. Building an
    /// `AssetStatuses` by hand and reading it back would assert only that
    /// `HashMap` works.
    #[test]
    fn a_load_that_cannot_succeed_is_reported_as_failed_with_a_reason() {
        let missing = format!("{}/absent-texture.png", unique("missing"));

        let mut app = new_app();
        app.add_plugins((AssetPlugin, AssetStatusPlugin));

        // Retained for the whole test: dropping it would let `track_assets`
        // evict the `AssetInfo`, which is a *different* property (pinned by
        // `a_recorded_failure_survives_its_handle_being_dropped`).
        let _handle = {
            let server = app.world().resource::<AssetServer>();
            server.load::<TextureAsset>(missing.clone())
        };

        let status = pump_until(&mut app, &missing, |s| matches!(s, AssetStatus::Failed(_)));

        let AssetStatus::Failed(reason) = &status else {
            panic!("expected Failed for a path that cannot exist, got {status:?}");
        };
        assert!(
            !reason.trim().is_empty(),
            "a failure with an empty reason is no better than the warn! it replaces"
        );
        let lowered = reason.to_lowercase();
        assert!(
            lowered.contains("not found") || lowered.contains("absent-texture.png"),
            "the reason must name what went wrong or what it went wrong on, got {reason:?}"
        );
    }

    /// The other direction: silence is reported as silence.
    ///
    /// If an unrequested path came back `Failed`, every caller would have to
    /// second-guess the answer, and the API would be worse than the log.
    #[test]
    fn a_path_nothing_ever_requested_is_unknown_not_failed() {
        let mut app = new_app();
        app.add_plugins((AssetPlugin, AssetStatusPlugin));

        // Same shape as the failing path above, and equally nonexistent — the
        // only difference is that nothing asked for it.
        let never_asked = format!("{}/never-requested.png", unique("quiet"));

        for _ in 0..8 {
            app.update();
        }

        assert_eq!(
            app.world().resource::<AssetStatuses>().get(&never_asked),
            AssetStatus::Unknown,
            "a path nothing requested must read Unknown, not Failed and not Loading"
        );
    }

    /// A recorded failure must survive the `AssetInfo` behind it being
    /// evicted, because that reads back as `LoadState::NotLoaded` — the same
    /// failed asset, not a fresh unknown.
    #[test]
    fn a_recorded_failure_survives_its_handle_being_dropped() {
        let missing = format!("{}/dropped-texture.png", unique("dropped"));

        let mut app = new_app();
        app.add_plugins((AssetPlugin, AssetStatusPlugin));

        let handle = {
            let server = app.world().resource::<AssetServer>();
            server.load::<TextureAsset>(missing.clone())
        };
        let failed = pump_until(&mut app, &missing, |s| matches!(s, AssetStatus::Failed(_)));
        assert!(
            matches!(failed, AssetStatus::Failed(_)),
            "precondition: the load must fail first, got {failed:?}"
        );

        drop(handle);

        // Without this the test is vacuous: while the info still exists the
        // refresh reads `LoadState::Failed` and would keep the entry for a
        // reason that has nothing to do with the guard under test.
        let deadline = Instant::now() + DEADLINE;
        loop {
            app.update();
            if app
                .world()
                .resource::<AssetServer>()
                .get_path_id(&missing)
                .is_none()
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the AssetInfo was never evicted, so this probe proves nothing"
            );
            std::thread::sleep(Duration::from_millis(1));
        }

        // Several more frames, each one a chance for the refresh to read
        // `NotLoaded` and clobber the failure.
        for _ in 0..8 {
            app.update();
        }

        let status = app.world().resource::<AssetStatuses>().get(&missing);
        assert!(
            matches!(status, AssetStatus::Failed(_)),
            "a failure must not be erased by the NotLoaded that follows its handle \
             being dropped, got {status:?}"
        );
    }

    /// The transition the rustdoc claims: fix the file, reload, and the stale
    /// failure is gone. Without this, the API would keep reporting an error
    /// the user already fixed.
    #[test]
    fn a_successful_reload_clears_a_previously_recorded_failure() {
        let dir = unique("fixed");
        let _guard = ProbeDir(PathBuf::from(&dir));
        std::fs::create_dir_all(&dir).unwrap();
        let rel = format!("{dir}/late-texture.png");

        let mut app = new_app();
        app.add_plugins((AssetPlugin, AssetStatusPlugin));

        // Load it while it is still missing.
        let _handle = {
            let server = app.world().resource::<AssetServer>();
            server.load::<TextureAsset>(rel.clone())
        };
        let failed = pump_until(&mut app, &rel, |s| matches!(s, AssetStatus::Failed(_)));
        assert!(
            matches!(failed, AssetStatus::Failed(_)),
            "precondition: the first load must fail, got {failed:?}"
        );

        // Now the user "fixes the file" and the watcher reloads it.
        write_png(&PathBuf::from(&rel));
        app.world().resource::<AssetServer>().reload(rel.clone());

        let status = pump_until(&mut app, &rel, |s| matches!(s, AssetStatus::Loaded));
        assert_eq!(
            status,
            AssetStatus::Loaded,
            "a successful reload must clear the stale failure rather than keep \
             reporting an error the user already fixed"
        );
    }
}
