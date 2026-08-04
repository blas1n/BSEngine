//! Stable asset identity: the [`AssetGuid`] type and the `.meta` sidecar that
//! stores it beside the asset.
//!
//! An asset's path is how a human finds it and a terrible way for a project to
//! refer to it: renaming `fox.glb` silently breaks every scene, script and
//! material that named it. The identity here is the fix — a value that is
//! minted once, travels with the asset in its sidecar, and does not change when
//! the file moves.

use bevy_app::{App, Plugin, Startup, Update};
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::prelude::{Res, World};
use bsengine_core::ProjectDir;
use std::fmt;
use std::io;
use std::str::FromStr;
use tracing::{info, warn};

/// What a scan found, and the lookups the rest of item 30 asks of it.
pub mod index;
/// Recording a rename the watcher saw happen: moving the `.meta` along with the
/// asset and remembering the path it left, which is the only record a move made
/// while the engine is running would otherwise leave.
pub(crate) mod rename;
/// Walking a project's `assets/` directory and giving every file that deserves
/// an identity a sidecar holding one.
pub mod scan;
/// The `.meta` sidecar file that pins an asset's identity next to the asset.
pub mod sidecar;

pub use index::AssetIndex;
pub use scan::scan;
pub use sidecar::{
    empty_hash, measure_file, sidecar_path, Sidecar, SidecarError, SIDECAR_EXTENSION,
};

/// A stable identity for one asset file, independent of where it lives.
///
/// Randomly generated once, when a scan first finds an asset with no sidecar,
/// and never derived from the file's contents: a content-derived id would
/// change every time an artist saved the file, breaking every reference —
/// catastrophic in an engine whose whole point this item is.
///
/// The `#[serde(transparent)]` is load-bearing. Without it `ron` writes a
/// newtype struct as `guid: ("0193…")`, parenthesised; the sidecar format is
/// specified as a bare string, and sub-item B puts the same value in scene RON
/// where the extra parentheses would be noise a human has to explain.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct AssetGuid(uuid::Uuid);

impl AssetGuid {
    /// Generates a fresh, random identity.
    //
    // No `Default`, deliberately, and the clippy lint that asks for one is
    // wrong here in both directions. Deriving it would hand out the nil UUID,
    // so every asset that was default-constructed rather than scanned would
    // share one "identity" — the exact collision this type exists to prevent.
    // Writing `Default::default() == new()` is no better: it makes any
    // `#[derive(Default)]` on a containing struct mint a real identity by
    // accident, silently and at a distance. Minting an identity is a decision,
    // so it is spelled out at every call site.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl fmt::Display for AssetGuid {
    /// Writes the canonical lowercase hyphenated form, which is what both the
    /// sidecar and (from sub-item B) scene RON store.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for AssetGuid {
    type Err = AssetGuidParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        uuid::Uuid::parse_str(text)
            .map(Self)
            .map_err(|_| AssetGuidParseError(text.to_string()))
    }
}

/// The error [`AssetGuid::from_str`] returns for text that is not an identity.
///
/// Deliberately does not expose the underlying `uuid` error: which UUID crate
/// backs [`AssetGuid`] is an implementation detail, and a caller that matched
/// on it would freeze that choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetGuidParseError(String);

impl fmt::Display for AssetGuidParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "not a valid asset GUID: `{}`", self.0)
    }
}

impl std::error::Error for AssetGuidParseError {}

/// Scans `<ProjectDir>/assets` once at `Startup` and publishes the result as an
/// [`AssetIndex`] resource, so everything after it can ask what identity an
/// asset has without walking the disk again.
///
/// # Who registers it, and what has to be true about the order
///
/// All three hosts do: `bsengine-runtime`'s windowed app and its `--test` app,
/// and `bsengine-editor-app`. It was registered by none of them while nothing
/// read the index; `bsengine-scene` resolving a scene reference by identity is
/// the reader that changed that.
///
/// **Registering it is not enough on its own, and the way it fails is
/// invisible.** The reader is `ScenePlugin`'s spawn, which is also a `Startup`
/// system, and "both in `Startup`" leaves them unordered. A spawn that finds no
/// index falls back to the path the scene stores — deliberately, so a scene
/// loads identically with and without one — so a scene that resolved too early
/// still loads, still spawns, and still says nothing. The whole feature would
/// be inert with no symptom at all.
///
/// Two things make that impossible rather than unlikely:
///
/// * `ScenePlugin` declares `.after(build_asset_index)`, so the schedule
///   settles the order however a host happens to list its plugins — and three
///   hosts list them three ways. The constraint names a system type; in an app
///   that never adds this plugin there is no instance to order against and it
///   costs nothing.
/// * [`build_asset_index`] inserts the resource **straight into the world**
///   rather than queuing it through `Commands`, so ordering is all the edge
///   above has to buy. See that function for why the difference is not
///   stylistic.
///
/// # What it does when there is nothing to scan
///
/// With no `ProjectDir`, an empty one (the editor's default) or no `assets`
/// directory under it, the plugin logs once at info and inserts an **empty**
/// index. It never falls back to the process working directory: that is the
/// repository root, and scanning it would sidecar `target/` and `.git/` — the
/// same reason [`crate::watcher`] exists instead of `bevy_asset`'s own
/// `file_watcher` feature. An assets-less project is normal (a fresh project,
/// the editor before a project is opened), so it is reported at info and not as
/// an error, and the resource is inserted either way: a consumer can then
/// depend on the index existing rather than on it existing *sometimes*.
///
/// The scan runs in `Startup`, so it happens exactly once per app, never per
/// frame, and the resource is in place before the first `Update`.
///
/// # The second half: reaching paths that are not in the `World`
///
/// The index is also published to [`crate::load_mode`], which is what lets
/// [`crate::load_async`] recover a path an asset has moved away from — the only
/// mechanism that reaches an asset path spelled inside a JavaScript string
/// literal, since nothing can give one an identity. That has to travel out of
/// the ECS because `load_async` is handed an `&AssetServer` and a path and has
/// no `World` to consult; see [`crate::load_mode::publish_asset_index`] for the
/// side channel, and this module's private `republish_asset_index` for what
/// keeps it current after a live rename.
pub struct AssetIdentityPlugin;

impl Plugin for AssetIdentityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, build_asset_index)
            .add_systems(Update, republish_asset_index);
    }
}

/// Keeps the index [`crate::load_async`] resolves former paths against in step
/// with the one in the `World`.
///
/// [`build_asset_index`] publishes the scan's result directly, so this exists
/// for what happens *after* `Startup`: [`crate::watcher`] re-points the
/// `AssetIndex` resource when it sees an asset renamed while the app is
/// running, and that is exactly the case former-path recovery is most for — a
/// file renamed the normal way, in an editor, with the game up. Without this,
/// recovery would work only for moves made while the engine was *not* running
/// and would look broken to everyone else.
///
/// Costs one change-detection check per frame and nothing else: the index
/// changes at `Startup` and then only when a rename arrives, and
/// [`crate::load_mode::publish_asset_index`] compares before it copies, so even
/// a change that leaves the index equal does not clone it.
///
/// `Option<Res<..>>` because a resource can be removed and a system that
/// panicked over it would take the app down for a diagnostic feature.
fn republish_asset_index(index: Option<Res<AssetIndex>>, project_dir: Option<Res<ProjectDir>>) {
    let Some(index) = index.filter(|index| index.is_changed()) else {
        return;
    };
    let project_dir = project_dir.map(|dir| dir.0.clone()).unwrap_or_default();
    crate::load_mode::publish_asset_index(&project_dir, &index);
}

/// Scans for identities, or explains once why it is not going to.
///
/// Always inserts an index, including on failure. The alternative — leaving the
/// resource absent when the scan fails — would push every future consumer into
/// `Option<Res<AssetIndex>>` and make "this project has no identified assets"
/// indistinguishable from "the scan never ran", which is the split-brain the
/// crate docs describe this engine having already shipped once.
///
/// # Why this takes `&mut World` rather than `Commands`
///
/// So that the index is *published* when this returns rather than queued.
/// `Commands::insert_resource` lands at the schedule's next sync point, and an
/// unordered `Startup` has none until the end of the whole schedule — so with
/// `Commands` the resource appeared only after **every** `Startup` system had
/// run, `ScenePlugin`'s spawn included, no matter which of the two the
/// executor happened to start with. That is not a hypothetical: it is what the
/// first version of this function did, and the test that caught it
/// (`bsengine-scene`'s
/// `a_scene_resolves_against_the_index_the_identity_plugin_publishes`) failed
/// in *both* registration orders.
///
/// An ordering edge alone would in fact have fixed it, because Bevy inserts a
/// sync point at a dependency edge whose upstream system has deferred buffers.
/// That is a fix that depends on a schedule-build setting
/// (`ScheduleBuildSettings::auto_insert_apply_deferred`) staying on, to repair
/// a failure whose only symptom is a feature quietly not happening. Writing to
/// the world directly means the edge has to buy nothing but order.
///
/// Public so a consumer in another crate can name it in an `.after(..)`; see
/// [`AssetIdentityPlugin`] for why that ordering has to be spelled out.
pub fn build_asset_index(world: &mut World) {
    let configured_dir = world
        .get_resource::<ProjectDir>()
        .map(|dir| dir.0.clone())
        .filter(|d| !d.is_empty());
    let index = match configured_dir.clone() {
        None => {
            info!("asset identity: no project directory set, nothing indexed");
            AssetIndex::default()
        }
        Some(project_dir) => match scan(&project_dir) {
            Ok(index) => {
                info!(
                    "asset identity: indexed {} assets under {project_dir}/assets",
                    index.len()
                );
                index
            }
            // The assets-less project: routine, and the one error scan()
            // documents for a directory that simply is not there.
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                info!("asset identity: {project_dir}/assets does not exist, nothing indexed");
                AssetIndex::default()
            }
            // Anything else — a permission error, a file where the directory
            // should be — is an anomaly rather than a configuration, so it is
            // louder. It still must not stop the app: identity is not yet load
            // bearing, and taking down a game over it would be a far worse
            // trade than starting with no index.
            Err(e) => {
                warn!("asset identity: cannot read {project_dir}/assets ({e}), nothing indexed");
                AssetIndex::default()
            }
        },
    };
    // Published before the resource is inserted rather than left to
    // `republish_asset_index`, so that a load which happens before the first
    // `Update` — anything a `Startup` system requests — resolves against the
    // same index a scene spawned in that schedule does.
    crate::load_mode::publish_asset_index(&configured_dir.unwrap_or_default(), &index);
    world.insert_resource(index);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{unique, ProbeDir};

    #[test]
    fn a_guid_round_trips_through_its_text_form() {
        // Sub-item B stores the identity as scene-RON text, which reaches it
        // through Display/FromStr rather than through serde.
        let original = AssetGuid::new();
        let parsed: AssetGuid = original.to_string().parse().expect("parse");
        assert_eq!(parsed, original);
    }

    #[test]
    fn the_text_form_is_the_canonical_hyphenated_uuid() {
        let text = AssetGuid::new().to_string();
        assert_eq!(text.len(), 36, "expected hyphenated form, got `{text}`");
        assert_eq!(text.matches('-').count(), 4, "`{text}`");
        assert!(
            text.chars().all(|c| c.is_ascii_hexdigit() || c == '-'),
            "`{text}`"
        );
        assert_eq!(text.to_lowercase(), text, "`{text}`");
    }

    #[test]
    fn text_that_is_not_a_guid_is_an_error_not_a_panic() {
        // Hand-edited scene RON is the expected source of this.
        for bad in ["", "not-a-guid", "0193a7c1-8f2e-7c44-9d61"] {
            assert!(
                bad.parse::<AssetGuid>().is_err(),
                "`{bad}` should not parse"
            );
        }
    }

    #[test]
    fn the_parse_error_names_the_offending_text() {
        let err = "not-a-guid".parse::<AssetGuid>().unwrap_err();
        assert!(
            err.to_string().contains("not-a-guid"),
            "a developer staring at a broken scene file needs to be told which \
             spelling was rejected, got: {err}"
        );
    }

    // ---- the plugin -------------------------------------------------------

    /// Writes a probe asset, creating whatever directories it needs.
    fn write_file(path: &std::path::Path, contents: &[u8]) {
        std::fs::create_dir_all(path.parent().expect("probe path has a parent"))
            .expect("create probe directories");
        std::fs::write(path, contents).expect("write probe file");
    }

    /// Builds an app holding nothing but this plugin and, optionally, a
    /// `ProjectDir`, and runs `Startup`.
    ///
    /// Deliberately `App::new()` rather than `bsengine_app::new_app`: the
    /// plugin's only input is `ProjectDir`, so anything else in the app would
    /// be scenery that could hide a missing dependency rather than expose one.
    fn run_startup(project_dir: Option<&str>) -> App {
        let mut app = App::new();
        if let Some(dir) = project_dir {
            app.insert_resource(ProjectDir(dir.to_string()));
        }
        app.add_plugins(AssetIdentityPlugin);
        app.update();
        app
    }

    #[test]
    fn the_plugin_publishes_an_index_of_the_project_it_is_pointed_at() {
        let probe = ProbeDir(std::env::temp_dir().join(unique("identity-plugin")));
        write_file(&probe.0.join("assets/models/fox.glb"), b"fake glb");
        write_file(&probe.0.join("assets/scenes/main.ron"), b"()");

        let app = run_startup(Some(&probe.0.display().to_string()));
        let index = app
            .world()
            .get_resource::<AssetIndex>()
            .expect("the plugin must publish an index");

        assert_eq!(index.len(), 2);
        assert!(
            index.guid_for_path("assets/models/fox.glb").is_some(),
            "an asset must be reachable by the path a scene spells it with"
        );
    }

    // The editor opens with an empty ProjectDir, and a fresh project has no
    // assets yet. Neither is an error, and neither may be answered by scanning
    // the working directory instead -- that is the repository root, `target/`
    // and `.git/` included, which is the same trap `watcher` exists to avoid.
    #[test]
    fn an_empty_or_missing_project_dir_publishes_an_empty_index() {
        for project_dir in [None, Some(String::new()), Some(unique("absent"))] {
            let mut app = run_startup(project_dir.as_deref());

            let index = app.world().get_resource::<AssetIndex>().expect(
                "the index must be published even with nothing to scan, \
                         so a consumer can depend on it existing",
            );
            assert!(
                index.is_empty(),
                "ProjectDir {project_dir:?} must produce an empty index"
            );
            // And the app must keep running rather than have startup abort.
            app.update();
        }
        assert!(
            !std::path::Path::new("assets").exists(),
            "this test only proves the working directory was not scanned while \
             there is no `assets` directory beside it -- there is one now, so \
             the assertion above has quietly stopped meaning anything"
        );
    }

    // Walking a project's whole asset tree is Startup work. Running it per
    // frame would hash and stat every asset every frame, and -- worse -- would
    // silently pick up files an editor session created, making the index depend
    // on when it was read.
    #[test]
    fn the_scan_happens_once_rather_than_every_frame() {
        let probe = ProbeDir(std::env::temp_dir().join(unique("identity-once")));
        write_file(&probe.0.join("assets/models/fox.glb"), b"fake glb");

        let mut app = run_startup(Some(&probe.0.display().to_string()));
        assert_eq!(app.world().resource::<AssetIndex>().len(), 1);

        // A file that appears after startup. A per-frame scan would index it
        // and write it a sidecar; a Startup-only one leaves both alone.
        let latecomer = probe.0.join("assets/models/cube.glb");
        write_file(&latecomer, b"fake glb 2");
        for _ in 0..5 {
            app.update();
        }

        assert_eq!(
            app.world().resource::<AssetIndex>().len(),
            1,
            "the index changed after Startup, so the scan is running again"
        );
        assert!(
            !probe.0.join("assets/models/cube.glb.meta").exists(),
            "a sidecar appeared for a file created after Startup, so the scan is \
             running again and writing to disk every frame"
        );
        assert!(latecomer.exists(), "probe file vanished");
    }
}
