//! Recording a rename the engine watched happen: moving the asset's `.meta`
//! along with it and remembering the path it left.
//!
//! # Why the scan is not enough
//!
//! [`scan`](super::scan)'s orphan recovery already restores an identity to an
//! asset that moved — but only for a move made while the engine was **not**
//! running, that left the sidecar behind, and whose contents still hash to what
//! that sidecar recorded. Every one of those conditions is about reconstructing
//! after the fact what nobody was there to see.
//!
//! The common case is the other one: an artist renames a file in the editor, or
//! in Explorer, with the game running. Until this module existed the engine
//! watched that happen and threw the interesting half away —
//! [`drain_asset_changes`](crate::watcher) reloads the destination and drops the
//! source, because the source no longer `is_file()`.
//!
//! The information was never missing, only discarded. `notify-debouncer-full`
//! stitches the backend's two halves into a single event carrying **both**
//! paths, old first; that is pinned, on the platforms CI covers, by
//! `a_rename_is_reported_with_both_the_old_and_the_new_path` in
//! [`crate::watcher`]. This module is what keeps it.
//!
//! # Why a former path is worth writing a file for
//!
//! Because a path spelled inside a JavaScript string literal —
//! `Bsengine.setShader(self, "assets/shaders/glow.wgsl")`, or tilt-run's
//! `const NEXT_SCENE = "assets/scenes/level2.ron"` — is not something any index
//! can rewrite. Nothing in the engine knows those characters are a path. The
//! only way such a reference survives a rename is for the project to be able to
//! answer *"what used to be here?"*, and the only place that answer can come
//! from is a note made when the move happened.
//!
//! # An atomic save is a rename too
//!
//! Writing a temporary file and renaming it over the target is how careful
//! software saves — this crate's own [`Sidecar::write`] included — so a rename
//! is at least as often a *save* as a move. Recording the temporary's name as a
//! former path would drop a line of garbage into a committed file on every
//! single save, and `former_paths` would fill with names that never identified
//! anything.
//!
//! **What tells the two apart is the sidecar, checked on disk rather than
//! guessed at from the name.** A temporary that existed for a few milliseconds
//! has never been scanned, so nothing ever wrote a `.meta` beside it; an asset
//! that has been in the project long enough for anything to reference it has
//! one. So "is there a `<old>.meta`?" *is* the question "was there an identity
//! here to move?", which is the question that has to be answered before
//! anything can be moved anyway. There is no second heuristic to keep in step
//! with reality.
//!
//! It is worth being clear about what is *not* doing the work, because the
//! obvious alternatives both fail. The temporary's **extension** proves
//! nothing: an editor that saves by writing `tex_tmp.png` and renaming it over
//! `tex.png` uses an extension the scan identifies, and is still a save
//! (`a_save_through_a_temporary_that_looks_like_an_asset_records_nothing`
//! pins exactly that). Its **lifetime** is not observable either — by the time
//! the debounced event arrives the temporary is already gone, so "did this file
//! exist a moment ago?" has no answer to read.
//!
//! # What this deliberately does not cover
//!
//! * **A rename that crosses the watched directory's boundary.** Moving an
//!   asset out of `assets/` reports only the half that is inside it, so there
//!   is no pairing to act on and the sidecar is left behind — where the next
//!   scan finds it orphaned and can still recover the identity by contents.
//!   Moving one *in* likewise arrives as a plain creation, and the file is
//!   given an identity by the next scan in the ordinary way. Both are the
//!   offline path, which is exactly what it is for.
//! * **Renaming a whole directory.** `assets/models` → `assets/meshes` is
//!   reported as one rename of the directory, and every asset under it keeps
//!   its sidecar simply by travelling with it — no identity is lost. What is
//!   *not* recorded is a former path for each of those assets, so a reference
//!   naming the old directory cannot be recovered even though a reference by
//!   identity resolves fine. Fixing that means rewriting every sidecar in the
//!   moved subtree, which is a different job from recording one move, and it is
//!   left for its own change rather than smuggled in here.
//!
//! # And this module's own writes are renames as well
//!
//! [`Sidecar::write`] is atomic: it writes `<asset>.meta.tmp-…` and renames it
//! over `<asset>.meta`. The watcher therefore sees every sidecar this module
//! writes come back as a rename event of its own. Anything reported with a
//! `.meta` at either end is dropped before the disk is touched, so the recorder
//! cannot feed itself.

use super::index::{AssetIndex, Insertion};
use super::{sidecar_path, Sidecar, SIDECAR_EXTENSION};
use std::path::Path;
use tracing::{info, warn};

/// One end of a rename: where the file sits on disk, and the same file spelled
/// the way the index and every stored reference name it.
///
/// A pair rather than four loose arguments because the two spellings of one end
/// have to travel together. A `from` path passed with a `to` relative path would
/// write one asset's history onto another asset's sidecar, and nothing about two
/// adjacent `&str` parameters would have stopped it.
pub(crate) struct Endpoint<'a> {
    /// The path as the watcher reported it — absolute, and what the sidecar
    /// sits beside.
    pub path: &'a Path,
    /// The same file, project-relative and forward-slashed:
    /// `assets/models/fox.glb`. This is what the index is keyed by, what a
    /// scene stores and what a script spells.
    pub relative: &'a str,
}

/// Moves an asset's identity to wherever the asset just went, and records where
/// it was.
///
/// Silently does nothing unless there is a sidecar beside `from`: see the module
/// docs on why the presence of that file is exactly the question being asked,
/// and why an atomic save must cost nothing and say nothing.
///
/// `index` is optional because the watcher runs in hosts that have not
/// registered [`AssetIdentityPlugin`](super::AssetIdentityPlugin) — the disk is
/// the record either way, and an index that is not there is not a reason to
/// leave a sidecar behind at a path the asset has left.
pub(crate) fn record_rename(from: Endpoint, to: Endpoint, index: Option<&mut AssetIndex>) {
    // This module's own writes come back as renames; so does a user moving a
    // `.meta` by hand, which is a thing the scan is far better placed to sort
    // out than a single event is. Neither is an asset moving.
    if is_sidecar(from.path) || is_sidecar(to.path) {
        return;
    }

    let from_meta = sidecar_path(from.path);
    let sidecar = match Sidecar::read(&from_meta) {
        Ok(Some(sidecar)) => sidecar,
        // Nothing was identified here, so nothing moved: a temporary file from
        // an atomic save, a `.md`, an editor swap file, a `.testlog.json`. This
        // is the overwhelmingly common case — it happens on every save — and it
        // is why this branch says nothing at all.
        Ok(None) => return,
        // A sidecar that exists and will not parse is the one state the scan
        // deliberately refuses to repair, and moving a file it cannot read
        // would be a worse version of the same decision. Left exactly where it
        // is, which also leaves the next scan the chance to recover the
        // identity by contents.
        Err(e) => {
            warn!(
                "asset identity: {} was renamed to {}, but the sidecar beside it \
                 ({}) exists and cannot be read ({e}); it is left untouched, so \
                 the identity does not follow the asset and a reference to the \
                 old path cannot be recovered. Repair or delete the file by hand",
                from.relative,
                to.relative,
                from_meta.display()
            );
            return;
        }
    };

    // Renaming *over* an existing asset is an ordinary thing to do — replacing
    // one texture with another — and the file being replaced has an identity of
    // its own that references already point at. Overwriting it would silently
    // repoint every one of them at different bytes, which is the exact failure
    // item 30 exists to end, arrived at by the feature meant to prevent it.
    let to_meta = sidecar_path(to.path);
    if to_meta.exists() {
        warn!(
            "asset identity: {} was renamed over {}, which already has an \
             identity of its own in {}; replacing it would silently repoint \
             every reference to {} at a different asset, so nothing was moved \
             and the identity {} was carrying is left in {}. If that is the \
             identity to keep, replace the sidecar by hand and rescan",
            from.relative,
            to.relative,
            to_meta.display(),
            to.relative,
            from.relative,
            from_meta.display()
        );
        return;
    }

    let mut moved = sidecar;
    moved.remember_former_path(from.relative);

    // Written to the new location *before* the old one is removed, and the
    // order is the whole safety argument. Interrupted between the two, this
    // leaves two sidecars claiming one GUID — which the next scan reports and
    // tells the user to resolve by deleting one. Interrupted the other way
    // round it would leave an asset with no identity and no orphan to recover
    // it from, so the next scan would mint a fresh GUID and every reference
    // would break, silently and permanently.
    if let Err(e) = moved.write(&to_meta) {
        warn!(
            "asset identity: {} was renamed to {}, but writing {} failed ({e}); \
             the sidecar stays at the old path, where the next scan will find it \
             orphaned and can still recover the identity from the asset's \
             contents",
            from.relative,
            to.relative,
            to_meta.display()
        );
        return;
    }

    if let Err(e) = std::fs::remove_file(&from_meta) {
        warn!(
            "asset identity: {} has taken over the identity {} was holding, but \
             that sidecar could not be deleted ({e}); delete it by hand, or every \
             later scan will report an orphan claiming an identity that is \
             already in use",
            to.relative,
            from_meta.display()
        );
    }

    info!(
        "asset identity: {} was renamed to {}; its identity moved with it and \
         the old path is recorded, so a reference that still names it can be \
         recovered",
        from.relative, to.relative
    );

    // In-memory last, and only after the disk agrees: an index that claimed a
    // move no sidecar records would answer differently before and after a
    // restart, which is the split-brain this whole item is about.
    if let Some(index) = index {
        match index.moved(from.relative, to.relative, &moved) {
            Insertion::Recorded => {}
            rejected @ (Insertion::DuplicateGuid { .. } | Insertion::DuplicatePath { .. }) => {
                warn!(
                    "asset identity: {} moved on disk but will not be in the \
                     index until the next restart — {rejected}",
                    to.relative
                )
            }
        }
    }
}

/// Whether a path names a `.meta` sidecar rather than an asset.
///
/// Case-insensitively, because the filesystems this runs on are: a `FOX.GLB.META`
/// that slipped through would be treated as an asset and given a sidecar of its
/// own.
fn is_sidecar(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case(SIDECAR_EXTENSION))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::AssetGuid;
    use crate::test_support::{capture_warnings, unique, ProbeDir};
    use std::path::PathBuf;

    /// A probe directory holding `assets/`, and the sidecar-writing helpers the
    /// cases below share. Everything here works in the two spellings a rename
    /// needs at once — the real path on disk and the project-relative one — so
    /// that a test cannot accidentally assert about a pair that do not describe
    /// the same file.
    struct Probe(ProbeDir);

    impl Probe {
        fn new(tag: &str) -> Self {
            let dir = ProbeDir(std::env::temp_dir().join(unique(tag)));
            std::fs::create_dir_all(dir.0.join("assets")).expect("create the probe assets dir");
            Self(dir)
        }

        /// The absolute path of a project-relative spelling.
        fn at(&self, relative: &str) -> PathBuf {
            self.0 .0.join(relative)
        }

        /// Writes an asset and, unless `identified` is false, the sidecar a scan
        /// would have written beside it.
        fn asset(&self, relative: &str, identified: bool) -> Option<Sidecar> {
            std::fs::write(self.at(relative), b"fake asset contents").expect("write the asset");
            identified.then(|| {
                let sidecar = Sidecar {
                    guid: AssetGuid::new(),
                    hash: "blake3:abc123".to_string(),
                    size: Some(19),
                    former_paths: Vec::new(),
                };
                sidecar
                    .write(sidecar_path(self.at(relative)))
                    .expect("write the sidecar");
                sidecar
            })
        }

        /// Renames on disk and tells the recorder about it, exactly as the
        /// watcher does.
        fn rename(&self, from: &str, to: &str, index: Option<&mut AssetIndex>) -> String {
            let (from_path, to_path) = (self.at(from), self.at(to));
            std::fs::rename(&from_path, &to_path).expect("rename the file");
            let (_, logs) = capture_warnings(|| {
                record_rename(
                    Endpoint {
                        path: &from_path,
                        relative: from,
                    },
                    Endpoint {
                        path: &to_path,
                        relative: to,
                    },
                    index,
                )
            });
            logs
        }

        /// The sidecar beside a project-relative path, if there is one.
        fn sidecar(&self, relative: &str) -> Option<Sidecar> {
            Sidecar::read(sidecar_path(self.at(relative))).expect("read the sidecar")
        }
    }

    // The property this module exists for, at the level where the decision is
    // actually made: the identity follows the asset, and the path it left is
    // recorded, which is the only thing that can ever recover a reference
    // spelled inside a JS string literal.
    #[test]
    fn a_rename_takes_the_sidecar_along_and_records_the_old_path() {
        let probe = Probe::new("rename-unit");
        let before = probe.asset("assets/fox.glb", true).expect("identified");

        let logs = probe.rename("assets/fox.glb", "assets/renamed.glb", None);

        let after = probe
            .sidecar("assets/renamed.glb")
            .expect("the sidecar must be beside the asset at its new name");
        assert_eq!(
            after.guid, before.guid,
            "a rename must not change the identity -- that is the entire point \
             of having one"
        );
        assert_eq!(
            after.former_paths,
            ["assets/fox.glb"],
            "without the old path recorded, a reference naming it has nothing \
             to resolve against"
        );
        assert!(
            probe.sidecar("assets/fox.glb").is_none(),
            "a sidecar left at the old name is an orphan the next scan has to \
             guess at by content hash -- exactly the situation this avoids"
        );
        assert!(
            logs.is_empty(),
            "a clean rename must be quiet, got:\n{logs}"
        );
    }

    // The atomic save, which is the same filesystem operation and must produce
    // none of the above. Every editor and this crate's own `Sidecar::write` save
    // this way, so a recorder that could not tell the difference would append a
    // junk former path on *every save of every asset*.
    #[test]
    fn a_save_through_a_temporary_records_nothing_and_says_nothing() {
        let probe = Probe::new("rename-atomic");
        let target = probe.asset("assets/tex.png", true).expect("identified");
        // The temporary an editor writes: no sidecar, because nothing has ever
        // scanned it.
        probe.asset("assets/tex.png.tmp", false);

        let logs = probe.rename("assets/tex.png.tmp", "assets/tex.png", None);

        let after = probe.sidecar("assets/tex.png").expect("still identified");
        assert_eq!(
            after.guid, target.guid,
            "a save must not change an identity"
        );
        assert!(
            after.former_paths.is_empty(),
            "a save is not a move; recording the temporary's name would put a \
             line of garbage in a committed file on every save, got {:?}",
            after.former_paths
        );
        assert!(
            logs.is_empty(),
            "and it must do so silently -- a warning per save is its own kind of \
             broken, got:\n{logs}"
        );
    }

    // The hostile shape of the same case, and the reason the discriminator is
    // "has a sidecar" rather than anything about the name. `tex_tmp.png` has an
    // extension the scan identifies and a name nothing could pattern-match, so
    // the *only* thing that can refuse it is the sidecar that was never there.
    #[test]
    fn a_save_through_a_temporary_that_looks_like_an_asset_records_nothing() {
        let probe = Probe::new("rename-lookalike");
        let target = probe.asset("assets/tex.png", true).expect("identified");
        probe.asset("assets/tex_tmp.png", false);

        probe.rename("assets/tex_tmp.png", "assets/tex.png", None);

        let after = probe.sidecar("assets/tex.png").expect("still identified");
        assert_eq!(after.guid, target.guid);
        assert!(
            after.former_paths.is_empty(),
            "an identified extension is not evidence of an identity; only a \
             sidecar is, got {:?}",
            after.former_paths
        );
    }

    // Replacing one asset with another is an ordinary edit, and the file being
    // replaced has references pointing at *its* identity. Carrying the incoming
    // file's identity over the top would repoint every one of them at different
    // bytes -- silently, which is the failure this whole item exists to end.
    #[test]
    fn a_rename_over_an_asset_that_already_has_an_identity_changes_neither() {
        let probe = Probe::new("rename-over");
        let replacement = probe.asset("assets/new_fox.glb", true).expect("identified");
        let existing = probe.asset("assets/fox.glb", true).expect("identified");

        let logs = probe.rename("assets/new_fox.glb", "assets/fox.glb", None);

        let after = probe.sidecar("assets/fox.glb").expect("still identified");
        assert_eq!(
            after.guid, existing.guid,
            "the identity references already point at must survive being \
             written over"
        );
        assert!(after.former_paths.is_empty());
        assert!(
            probe.sidecar("assets/new_fox.glb").is_some(),
            "the incoming asset's identity is left where it is rather than \
             thrown away, so the next scan can still report it"
        );
        assert_ne!(replacement.guid, existing.guid, "the fixture is degenerate");
        assert!(
            logs.contains("assets/fox.glb"),
            "a refusal nobody is told about leaves an orphaned sidecar and no \
             explanation for it, got:\n{logs}"
        );
    }

    // `Sidecar::write` is itself a temp-file rename, so every sidecar this
    // module writes comes straight back to it as a rename event. Without the
    // guard the recorder would be reacting to its own output.
    #[test]
    fn a_sidecar_write_arriving_as_a_rename_is_not_treated_as_a_move() {
        let probe = Probe::new("rename-feedback");
        let identified = probe.asset("assets/fox.glb", true).expect("identified");
        // Exactly what `Sidecar::write` does: the real sidecar text in a
        // temporary beside the target, renamed over it.
        let temp = "assets/fox.glb.meta.tmp-1234-0";
        std::fs::write(
            probe.at(temp),
            identified.to_ron().expect("render the sidecar"),
        )
        .expect("write the temporary");

        probe.rename(temp, "assets/fox.glb.meta", None);

        let after = probe.sidecar("assets/fox.glb").expect("still identified");
        assert_eq!(after.guid, identified.guid);
        assert!(
            after.former_paths.is_empty(),
            "the watcher observing this module's own sidecar write must not \
             feed back into it, got {:?}",
            after.former_paths
        );
        assert!(
            !sidecar_path(probe.at("assets/fox.glb.meta")).exists(),
            "a `.meta` must never be given a `.meta` of its own"
        );
    }

    // A file the scan never identifies -- a README, an E2E recording -- is
    // renamed all the time and must cost nothing. A `.meta` appearing beside one
    // would be litter in the user's source tree that nothing ever reads.
    #[test]
    fn renaming_a_file_that_was_never_an_asset_writes_nothing() {
        let probe = Probe::new("rename-nonasset");
        probe.asset("assets/CREDITS.md", false);

        let logs = probe.rename("assets/CREDITS.md", "assets/NOTES.md", None);

        assert!(probe.sidecar("assets/NOTES.md").is_none());
        assert!(probe.sidecar("assets/CREDITS.md").is_none());
        assert!(logs.is_empty(), "got:\n{logs}");
    }

    // Same guarantee `a_recovered_asset_records_where_it_was_without_repeating_itself`
    // makes for the offline path, on the live one: an editor that saves by
    // renaming, or a designer moving a file between two folders repeatedly,
    // would otherwise grow a committed file without bound.
    #[test]
    fn moving_back_and_forth_records_each_distinct_path_exactly_once() {
        let probe = Probe::new("rename-loop");
        let original = probe.asset("assets/fox.glb", true).expect("identified");

        for (from, to) in [
            ("assets/fox.glb", "assets/renamed.glb"),
            ("assets/renamed.glb", "assets/fox.glb"),
            ("assets/fox.glb", "assets/renamed.glb"),
        ] {
            probe.rename(from, to, None);
        }

        let after = probe.sidecar("assets/renamed.glb").expect("identified");
        assert_eq!(after.guid, original.guid);
        assert_eq!(
            after.former_paths,
            ["assets/fox.glb", "assets/renamed.glb"],
            "three moves between two paths are two former paths; appending \
             unconditionally would let this list grow for as long as the app runs"
        );
    }

    // The in-memory half. A rename that only reached the disk would answer
    // "where is this asset?" correctly after a restart and wrongly until then,
    // which is worse than either answer on its own.
    #[test]
    fn the_index_answers_for_both_the_new_path_and_the_old_one_immediately() {
        let probe = Probe::new("rename-index");
        let sidecar = probe.asset("assets/fox.glb", true).expect("identified");
        let mut index = AssetIndex::default();
        index.insert(sidecar.guid, "assets/fox.glb", &sidecar.former_paths);

        probe.rename("assets/fox.glb", "assets/renamed.glb", Some(&mut index));

        assert_eq!(
            index.guid_for_path("assets/renamed.glb"),
            Some(sidecar.guid),
            "the asset has to be findable where it now is"
        );
        assert_eq!(
            index.path_for_guid(sidecar.guid),
            Some("assets/renamed.glb"),
            "and the identity has to point at where it now is, or the two \
             directions of one fact disagree"
        );
        assert_eq!(
            index.guid_for_former_path("assets/fox.glb"),
            Some(sidecar.guid),
            "this is the lookup that recovers a path spelled inside a JS string \
             literal, and it is the whole reason the move is recorded"
        );
        assert_eq!(
            index.guid_for_path("assets/fox.glb"),
            None,
            "nothing lives at the old path any more; saying otherwise would hand \
             out a path that cannot be opened"
        );
    }

    // An asset created after `Startup` has a sidecar (this module just wrote
    // one) but was never in the index, so the move has nothing to re-point. It
    // must still land in the index rather than be dropped for having arrived
    // late.
    #[test]
    fn an_asset_the_index_never_knew_about_is_recorded_where_it_moved_to() {
        let probe = Probe::new("rename-unknown");
        let sidecar = probe.asset("assets/fox.glb", true).expect("identified");
        let mut index = AssetIndex::default();

        probe.rename("assets/fox.glb", "assets/renamed.glb", Some(&mut index));

        assert_eq!(
            index.guid_for_path("assets/renamed.glb"),
            Some(sidecar.guid)
        );
        assert_eq!(
            index.guid_for_former_path("assets/fox.glb"),
            Some(sidecar.guid)
        );
    }
}
