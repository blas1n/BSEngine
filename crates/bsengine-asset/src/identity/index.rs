//! The index a [`scan`](super::scan::scan) builds: what every identity it found
//! belongs to, and the questions the rest of item 30 will ask of it.
//!
//! # The three lookups, and who each is for
//!
//! * `path → guid` and `guid → path`, both directions of the same fact, because
//!   sub-item B has to write an identity into scene RON in place of a path and
//!   sub-item C has to turn it back into something the loader can open.
//! * `former path → guid`, so a reference to a path that no longer exists can
//!   name the asset that used to be there instead of failing — sub-item D.
//!
//! There is deliberately no `hash → guid`. Orphan recovery is the only thing
//! that has ever wanted one, and it cannot use this index for it: recovery has
//! to match sidecars that are *not* in the index (their assets are gone) against
//! assets that are not in it either (they have no sidecar yet), so it builds its
//! own maps over exactly those two sets. An index-wide hash map would answer a
//! question nobody asks, and would have to be kept correct forever anyway.
//!
//! # Why two collisions get two different answers
//!
//! A project can hold contradictions — copying an asset *together with its
//! `.meta`* is one keystroke in Explorer, and Unity users hit exactly this
//! constantly. The index refuses to invent a resolution for any of them, but
//! what "refuse" means depends on what would be lost:
//!
//! * **Two assets claiming one GUID**: the first is kept whole and the second
//!   is not indexed at all. Not because the first is more likely to be right,
//!   but because every reference already stored against that GUID points at
//!   *something*, and dropping both would break the innocent original as
//!   punishment for the copy. The second file is reported as unidentified,
//!   which is true: its identity is contested, so it has none.
//! * **Two assets claiming one former path**: neither answers. Nothing points
//!   at a former path yet — it is a hint for recovering a *lost* reference — so
//!   an arbitrary answer buys nothing and costs the chance of silently
//!   recovering a reference to the wrong asset. A loud "I don't know" is
//!   strictly better than a quiet wrong answer.
//!
//! # Neither answer may vary by machine
//!
//! The former-path policy does not depend on arrival order at all: contested is
//! contested whichever claimant came first.
//!
//! The GUID policy does — "the first is kept" is only a rule if *first* means
//! the same thing everywhere — and that is settled by the caller rather than
//! here. [`scan`](super::scan::scan) walks each directory in sorted name order
//! precisely so that the file it feeds in first is the same file on every
//! machine; raw `read_dir` order varies by filesystem, and two developers would
//! otherwise get two different answers for the same project. This type keeps
//! the guarantee it can — a total order in, a total order out — and the walk
//! supplies the order.

use super::AssetGuid;
use bevy_ecs::prelude::Resource;
use std::collections::BTreeMap;
use std::fmt;

/// Which asset a former path points at — or that too many do.
///
/// The `Contested` case is not an error state to be repaired; it is the honest
/// answer for a lookup that more than one asset can satisfy, and it is sticky:
/// a third claimant cannot resolve it back into a single one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Claim {
    /// Exactly one asset claims this key.
    Sole(AssetGuid),
    /// Two or more assets claim it, so it identifies nobody.
    Contested,
}

impl Claim {
    /// The claimant, if there is exactly one.
    fn sole(self) -> Option<AssetGuid> {
        match self {
            Self::Sole(guid) => Some(guid),
            Self::Contested => None,
        }
    }

    /// Folds in another asset's claim on the same key.
    ///
    /// The same asset claiming twice — a sidecar whose `former_paths` lists one
    /// path more than once — is not a contest, because there is still only one
    /// answer to give.
    fn joined_by(self, guid: AssetGuid) -> Self {
        match self {
            Self::Sole(existing) if existing == guid => Self::Sole(existing),
            Self::Sole(_) | Self::Contested => Self::Contested,
        }
    }
}

/// What a [`scan`](super::scan::scan) found: every asset it could identify, and
/// the ways the rest of item 30 needs to look one up.
///
/// The maps are private and there is no public way to add to them, so an
/// identity in here came from a sidecar on disk — nothing outside this module
/// can mint one and hand it out as if a scan had found it.
///
/// `BTreeMap` rather than `HashMap` so iteration order is stable, which costs
/// nothing at these sizes and makes any future dump of the index diffable.
///
/// A `Resource` because [`AssetIdentityPlugin`](super::AssetIdentityPlugin)
/// publishes the scan's result as one. The index is plain data and knows
/// nothing about the ECS beyond that; the derive is what lets the plugin insert
/// it directly instead of wrapping it in a newtype every consumer would have to
/// unwrap.
#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct AssetIndex {
    /// Where each identity currently lives. Kept in step with `by_path` by
    /// [`AssetIndex::insert`], which is the only thing that writes either.
    by_guid: BTreeMap<AssetGuid, String>,
    /// The identity of the asset at each project-relative path.
    by_path: BTreeMap<String, AssetGuid>,
    /// Paths assets have been known by before, from their sidecars.
    by_former_path: BTreeMap<String, Claim>,
}

impl AssetIndex {
    /// How many assets the scan gave an identity to.
    ///
    /// Not the number of files under `assets/`: files the allow-list rejects
    /// are not assets, and an asset whose sidecar could not be read or written,
    /// or whose identity another file already claims, is deliberately absent.
    pub fn len(&self) -> usize {
        self.by_path.len()
    }

    /// Whether the scan identified nothing at all.
    pub fn is_empty(&self) -> bool {
        self.by_path.is_empty()
    }

    /// The identity of the asset at `path`, which must be spelled
    /// project-relative with forward slashes — `assets/models/fox.glb`.
    pub fn guid_for_path(&self, path: &str) -> Option<AssetGuid> {
        self.by_path.get(path).copied()
    }

    /// Where the asset with this identity currently lives, project-relative.
    ///
    /// `None` means this scan found no file carrying that identity — the asset
    /// was deleted, or is somewhere the scan does not look. A reference holding
    /// the GUID is stale, which is a thing sub-item C can say out loud; before
    /// item 30 the same situation was a path that silently loaded nothing.
    pub fn path_for_guid(&self, guid: AssetGuid) -> Option<&str> {
        self.by_guid.get(&guid).map(String::as_str)
    }

    /// The asset that *used to* live at `path`, for recovering a reference that
    /// still names where an asset was before it moved.
    ///
    /// A path some asset currently occupies never answers here, even if another
    /// sidecar also lists it as a former path. Otherwise a perfectly good
    /// reference would read as stale, and sub-item D would warn about a path
    /// that resolves fine.
    ///
    /// That check is made *here* rather than by refusing to record such a path
    /// in the first place, and the difference is not cosmetic: at the moment an
    /// asset is inserted, the file that will later occupy its former path may
    /// not have been walked yet. Deciding at insert time would answer this
    /// question differently depending on `read_dir` order.
    pub fn guid_for_former_path(&self, path: &str) -> Option<AssetGuid> {
        if self.by_path.contains_key(path) {
            return None;
        }
        self.by_former_path.get(path).copied().and_then(Claim::sole)
    }

    /// Records one identified asset, and says whether it took.
    ///
    /// Deliberately all-or-nothing: an asset the index refuses is absent from
    /// every map, so `guid_for_path` and `path_for_guid` cannot disagree about
    /// it. Recording half of a contradiction is how a lookup starts depending
    /// on which direction you asked from.
    ///
    /// Not public, and not merely for tidiness: an identity that did not come
    /// from a sidecar on disk is one that changes on the next run, so the only
    /// thing allowed to put one in here is the scan that read it.
    pub(super) fn insert(
        &mut self,
        guid: AssetGuid,
        path: &str,
        former_paths: &[impl AsRef<str>],
    ) -> Insertion {
        if let Some(kept_path) = self.by_guid.get(&guid) {
            return Insertion::DuplicateGuid {
                kept_path: kept_path.clone(),
            };
        }
        if let Some(&kept_guid) = self.by_path.get(path) {
            return Insertion::DuplicatePath { kept_guid };
        }

        self.by_guid.insert(guid, path.to_string());
        self.by_path.insert(path.to_string(), guid);
        for former in former_paths {
            claim(&mut self.by_former_path, former.as_ref(), guid);
        }

        Insertion::Recorded
    }
}

/// Adds `guid` to whoever already claims `key`, contesting the entry if that is
/// somebody else.
fn claim(map: &mut BTreeMap<String, Claim>, key: &str, guid: AssetGuid) {
    match map.get_mut(key) {
        Some(existing) => *existing = existing.joined_by(guid),
        None => {
            map.insert(key.to_string(), Claim::Sole(guid));
        }
    }
}

/// What [`AssetIndex::insert`] did with one asset.
///
/// Returned rather than logged so the index stays a pure data structure the
/// scan can test without reading its own output, and so a caller can tell
/// "indexed" from "not indexed" without inspecting the maps. Its [`Display`]
/// carries the explanation a human needs, which is why the caller does not have
/// to know the variants to report one.
///
/// [`Display`]: std::fmt::Display
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Insertion {
    /// The asset is in the index.
    ///
    /// A former path it happens to share with another asset is contested and
    /// will answer for neither, but that costs this asset nothing — its own
    /// identity is unambiguous.
    Recorded,
    /// Rejected: another asset already carries this GUID, and is kept.
    DuplicateGuid {
        /// The path that keeps the identity.
        kept_path: String,
    },
    /// Rejected: another identity already holds this path, and is kept.
    ///
    /// Unreachable from a directory walk, which visits each path once; here
    /// because the alternative is two maps that disagree the day something
    /// other than a walk fills the index.
    DuplicatePath {
        /// The identity that keeps the path.
        kept_guid: AssetGuid,
    },
}

impl fmt::Display for Insertion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Recorded => write!(f, "indexed"),
            Self::DuplicateGuid { kept_path } => write!(
                f,
                "its GUID already identifies {kept_path}, and which file a GUID \
                 means must not depend on the order a directory happens to be \
                 read in. This is what copying an asset together with its .meta \
                 file does — delete this one's .meta and rescan, and it will be \
                 given an identity of its own"
            ),
            Self::DuplicatePath { kept_guid } => write!(
                f,
                "that path is already identified by {kept_guid}, and one file \
                 cannot have two identities"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An asset that has never moved. Named because a bare `&[]` cannot tell
    /// the compiler which kind of empty slice it is.
    const NO_FORMER_PATHS: &[&str] = &[];

    /// The paths in these tests are spelled the way a scene references an asset
    /// — project-relative, forward slashes — because that is what the index is
    /// keyed by and a test using some other spelling would prove nothing.
    fn index_with(entries: &[(AssetGuid, &str, &[&str])]) -> AssetIndex {
        let mut index = AssetIndex::default();
        for (guid, path, former) in entries {
            index.insert(*guid, path, former);
        }
        index
    }

    #[test]
    fn the_index_answers_both_directions_and_remembers_former_paths() {
        let mut index = AssetIndex::default();
        let guid = AssetGuid::new();
        index.insert(guid, "assets/models/fox.glb", &["assets/models/old.glb"]);

        assert_eq!(index.path_for_guid(guid), Some("assets/models/fox.glb"));
        assert_eq!(index.guid_for_path("assets/models/fox.glb"), Some(guid));
        assert_eq!(
            index.guid_for_former_path("assets/models/old.glb"),
            Some(guid),
            "a former path is what lets a stale reference in JS recover in sub-item D"
        );
        assert_eq!(index.guid_for_former_path("assets/models/fox.glb"), None);
    }

    // The previous test's last assertion only covers an asset listing its *own*
    // current path, which a sidecar this crate wrote never does. The realistic
    // shape is two assets: one moved away from a path, another moved into it.
    // Both orders are asserted because `read_dir` order varies by filesystem,
    // and a guard applied when an asset is inserted rather than when the index
    // is asked would pass one order and fail the other.
    #[test]
    fn a_path_another_asset_now_occupies_never_answers_as_a_former_one() {
        let moved_away = AssetGuid::new();
        let moved_in = AssetGuid::new();
        let contested = "assets/models/fox.glb";

        for (order, index) in [
            (
                "occupant first",
                index_with(&[
                    (moved_in, contested, &[]),
                    (moved_away, "assets/models/fox_old.glb", &[contested]),
                ]),
            ),
            (
                "occupant last",
                index_with(&[
                    (moved_away, "assets/models/fox_old.glb", &[contested]),
                    (moved_in, contested, &[]),
                ]),
            ),
        ] {
            assert_eq!(
                index.guid_for_former_path(contested),
                None,
                "{order}: a path that resolves perfectly well must not also read \
                 as stale, or sub-item D warns about a reference that works"
            );
            assert_eq!(
                index.guid_for_path(contested),
                Some(moved_in),
                "{order}: the occupant still owns the path"
            );
        }
    }

    // The copy-paste case: an artist duplicates an asset in Explorer and the
    // .meta comes with it. Overwriting would make "which file does this GUID
    // mean?" depend on directory iteration order — two developers, two answers,
    // for the same project.
    #[test]
    fn a_second_asset_claiming_a_guid_is_refused_and_the_first_kept_whole() {
        let guid = AssetGuid::new();
        let mut index = AssetIndex::default();
        index.insert(guid, "assets/models/fox.glb", &["assets/a.glb"]);

        let outcome = index.insert(guid, "assets/models/fox_copy.glb", &["assets/b.glb"]);

        assert_eq!(
            outcome,
            Insertion::DuplicateGuid {
                kept_path: "assets/models/fox.glb".to_string()
            },
            "the caller has to be able to report which file kept the identity"
        );
        assert_eq!(
            index.path_for_guid(guid),
            Some("assets/models/fox.glb"),
            "the first claimant keeps the identity every existing reference \
             already points at"
        );
    }

    // A refusal has to leave *nothing* behind, or the index answers differently
    // depending on which direction it is asked from. Every map is checked,
    // including the two the copy touched only incidentally.
    #[test]
    fn a_refused_asset_is_absent_from_every_map_not_just_the_guid_one() {
        let guid = AssetGuid::new();
        let mut index = AssetIndex::default();
        index.insert(guid, "assets/models/fox.glb", NO_FORMER_PATHS);

        index.insert(
            guid,
            "assets/models/fox_copy.glb",
            &["assets/models/gone.glb"],
        );

        assert_eq!(
            index.guid_for_path("assets/models/fox_copy.glb"),
            None,
            "an asset whose identity is contested has none, and saying otherwise \
             would make guid_for_path and path_for_guid contradict each other"
        );
        assert_eq!(
            index.guid_for_former_path("assets/models/gone.glb"),
            None,
            "a former path recorded for an asset that was never indexed would \
             recover references to an identity that answers no path"
        );
        assert_eq!(index.len(), 1);
    }

    #[test]
    fn a_second_identity_claiming_a_path_is_refused_and_the_first_kept() {
        let first = AssetGuid::new();
        let second = AssetGuid::new();
        let mut index = AssetIndex::default();
        index.insert(first, "assets/models/fox.glb", NO_FORMER_PATHS);

        let outcome = index.insert(second, "assets/models/fox.glb", NO_FORMER_PATHS);

        assert_eq!(outcome, Insertion::DuplicatePath { kept_guid: first });
        assert_eq!(index.guid_for_path("assets/models/fox.glb"), Some(first));
        assert_eq!(
            index.path_for_guid(second),
            None,
            "the refused identity must not be left pointing at a path it does \
             not own"
        );
    }

    // Two sidecars listing the same former path is the same class of question as
    // two claiming a GUID, and gets the opposite answer for a reason: nothing
    // points at a former path yet, so an arbitrary winner buys nothing and risks
    // recovering a reference to the wrong asset. Both orders again, because
    // "whichever came first" is no less filesystem-dependent than "whichever
    // came last".
    #[test]
    fn a_former_path_two_assets_claim_answers_for_neither() {
        let one = AssetGuid::new();
        let two = AssetGuid::new();
        let contested = "assets/models/old.glb";

        for (order, index) in [
            (
                "one first",
                index_with(&[
                    (one, "assets/a.glb", &[contested]),
                    (two, "assets/b.glb", &[contested]),
                ]),
            ),
            (
                "two first",
                index_with(&[
                    (two, "assets/b.glb", &[contested]),
                    (one, "assets/a.glb", &[contested]),
                ]),
            ),
        ] {
            assert_eq!(
                index.guid_for_former_path(contested),
                None,
                "{order}: a contested former path must answer nothing rather \
                 than pick a winner by directory order"
            );
            assert_eq!(
                index.guid_for_path("assets/a.glb"),
                Some(one),
                "{order}: contesting a hint must not cost either asset its own \
                 identity"
            );
            assert_eq!(index.guid_for_path("assets/b.glb"), Some(two), "{order}");
        }
    }

    // A third claimant must not resolve the contest back to a single answer.
    #[test]
    fn a_contested_former_path_stays_contested() {
        let index = index_with(&[
            (AssetGuid::new(), "assets/a.glb", &["assets/old.glb"]),
            (AssetGuid::new(), "assets/b.glb", &["assets/old.glb"]),
            (AssetGuid::new(), "assets/c.glb", &["assets/old.glb"]),
        ]);

        assert_eq!(index.guid_for_former_path("assets/old.glb"), None);
    }

    // One asset listing the same former path twice — a hand-edited sidecar —
    // is not two assets disagreeing, and treating it as one would silently cost
    // a real recovery hint.
    #[test]
    fn one_asset_listing_a_former_path_twice_does_not_contest_itself() {
        let guid = AssetGuid::new();
        let index = index_with(&[(guid, "assets/a.glb", &["assets/old.glb", "assets/old.glb"])]);

        assert_eq!(index.guid_for_former_path("assets/old.glb"), Some(guid));
    }

    #[test]
    fn an_empty_index_answers_nothing_rather_than_panicking() {
        let index = AssetIndex::default();
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
        assert_eq!(index.guid_for_path("assets/models/fox.glb"), None);
        assert_eq!(index.path_for_guid(AssetGuid::new()), None);
        assert_eq!(index.guid_for_former_path("assets/models/fox.glb"), None);
    }

    // The rejection is reported to a human by a scan that knows only the path it
    // was walking, so everything else it needs has to be in the message.
    #[test]
    fn a_rejection_explains_itself_well_enough_to_act_on() {
        let message = Insertion::DuplicateGuid {
            kept_path: "assets/models/fox.glb".to_string(),
        }
        .to_string();

        assert!(
            message.contains("assets/models/fox.glb"),
            "the file that kept the identity is the one to compare against; \
             got: {message}"
        );
        assert!(
            message.contains(".meta"),
            "the fix is to delete the copy's sidecar, and a warning that does \
             not say so leaves the developer guessing; got: {message}"
        );
    }
}
