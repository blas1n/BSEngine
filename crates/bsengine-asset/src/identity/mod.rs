//! Stable asset identity: the [`AssetGuid`] type and the `.meta` sidecar that
//! stores it beside the asset.
//!
//! An asset's path is how a human finds it and a terrible way for a project to
//! refer to it: renaming `fox.glb` silently breaks every scene, script and
//! material that named it. The identity here is the fix — a value that is
//! minted once, travels with the asset in its sidecar, and does not change when
//! the file moves.

use bevy_app::{App, Plugin, Startup};
use bevy_ecs::prelude::{Commands, Res};
use bsengine_core::ProjectDir;
use std::fmt;
use std::io;
use std::str::FromStr;
use tracing::{info, warn};

/// What a scan found, and the lookups the rest of item 30 asks of it.
pub mod index;
/// Walking a project's `assets/` directory and giving every file that deserves
/// an identity a sidecar holding one.
pub mod scan;
/// The `.meta` sidecar file that pins an asset's identity next to the asset.
pub mod sidecar;

pub use index::AssetIndex;
pub use scan::scan;
pub use sidecar::{hash_file, sidecar_path, Sidecar, SidecarError, SIDECAR_EXTENSION};

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
/// # No host registers this, on purpose
///
/// **Nothing adds this plugin — not `bsengine-app`, not the runtime, not the
/// editor — and that is a decision, not an oversight.** Sub-item A of roadmap
/// item 30 builds the identity machinery and deliberately changes no existing
/// behaviour; **nothing in the engine reads [`AssetIndex`] yet**. Registering
/// the plugin before there is a reader would walk every project's `assets/`
/// directory at every startup, hash every asset the first time, and write a
/// `.meta` beside each one, all to hand the answer to nobody — a cost, a slower
/// startup and a pile of new files in the user's source tree, with no benefit
/// to weigh against any of it.
///
/// Sub-item B is where a reader appears (scene references resolved by identity
/// rather than by path), and registering this plugin is part of that change,
/// not this one. Until then the only callers are this crate's own tests.
///
/// If you are here because you noticed a plugin that nothing installs: that is
/// the intended state, and this paragraph is the answer.
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
pub struct AssetIdentityPlugin;

impl Plugin for AssetIdentityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, build_asset_index);
    }
}

/// Scans for identities, or explains once why it is not going to.
///
/// Always inserts an index, including on failure. The alternative — leaving the
/// resource absent when the scan fails — would push every future consumer into
/// `Option<Res<AssetIndex>>` and make "this project has no identified assets"
/// indistinguishable from "the scan never ran", which is the split-brain the
/// crate docs describe this engine having already shipped once.
fn build_asset_index(mut commands: Commands, project_dir: Option<Res<ProjectDir>>) {
    let index = match project_dir
        .map(|dir| dir.0.clone())
        .filter(|d| !d.is_empty())
    {
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
    commands.insert_resource(index);
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
