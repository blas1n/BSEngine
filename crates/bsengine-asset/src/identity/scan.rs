//! The scan: walk a project's `assets/` directory and make sure every file
//! that deserves an identity has one.
//!
//! This is the half of asset identity that runs: [`sidecar`](super::sidecar)
//! defines what an identity *is*, and [`scan`] is what puts one next to every
//! asset and reports what it found. Nothing consumes the result yet — that is
//! deliberate, so introducing identity cannot change how anything currently
//! behaves.
//!
//! # Paths are project-relative
//!
//! Everything in an [`AssetIndex`] is keyed by the path *relative to the
//! project directory*, forward-slashed: `assets/models/fox.glb`. That is not a
//! new convention — it is exactly the spelling scene RON already uses and
//! exactly what `bsengine_core::resolve_project_path` takes, so a later
//! sub-item can look a scene's asset reference up in the index without
//! re-spelling it first. [`scan`] therefore takes the **project** directory and
//! walks its `assets/` subdirectory, which makes the keys project-relative by
//! construction rather than by a join that someone has to remember to do.
//!
//! # Nothing here fails the scan
//!
//! One unreadable file, one hand-mangled `.meta`, one directory the process
//! cannot open: each costs *that asset* its identity and nothing else. A scan
//! that aborted would be the one place a stray file could stop the engine
//! starting, which is a much worse failure than an asset the index does not
//! know about. The only error [`scan`] returns is the one that means it never
//! started at all — see its docs.
//!
//! # Recovering an identity whose sidecar was left behind
//!
//! Assets are moved by things that are not this engine — `git mv`, Explorer, an
//! artist's export script — and any of those can carry the asset without its
//! `.meta`. Unity, faced with that, treats the file as brand new: it mints a
//! fresh GUID and *deletes* the orphaned `.meta`, so every reference breaks and
//! the documented remedy is to restore from version control. Godot asks you to
//! move the `.uid` file by hand.
//!
//! Our assets are ordinary files we can hash, so [`scan`] can do better. A
//! sidecar with no asset beside it, and an asset with no sidecar whose contents
//! hash to exactly what that sidecar recorded, are almost certainly the same
//! asset — so the identity moves to where the file went, and the orphan is
//! deleted rather than left for every later scan to find again.
//!
//! **Only when the pairing is unambiguous**: exactly one orphan and exactly one
//! candidate. Files that share contents are ordinary rather than exotic — empty
//! placeholders, a template duplicated three times, two exports of the same mesh
//! — and with two of either, a pairing is a coin flip. A wrong one silently
//! reattaches every reference to the wrong file, which is worse than the
//! missing-asset error it would have replaced. So an ambiguous match is refused:
//! both sides take fresh identities, and a warning names everything the scan
//! could not tell apart, because a user who lost an identity deserves to learn
//! it here rather than when a scene fails to load.
//!
//! # What "the same contents" is not enough to prove
//!
//! Equal hashes are strong evidence and weak proof, and two cases make the
//! difference big enough to guard:
//!
//! * **A hash says nothing about what kind of file this is.** Small `.ron` and
//!   `.js` files collide easily — stubs, templates, one-liners, `()` — and this
//!   repository already contains byte-identical files under different names.
//!   Handing a scene's identity to a script that happens to hold the same bytes
//!   would silently repoint every reference at a file that cannot even be
//!   loaded in its place. So a pairing must also **agree on extension**, which
//!   is free (both sides are already known) and cannot be wrong in the way the
//!   hash can.
//! * **An empty file's hash is the least discriminating value there is.** Every
//!   empty file in the project has it, and several empty placeholders is a
//!   normal thing to have, so matching on it says only "both of these are
//!   empty". Recovery **refuses empty contents outright** rather than treating
//!   that as evidence.
//!
//! Size deliberately adds nothing here and is not checked: equal hashes already
//! imply equal contents and therefore equal length, so a size comparison would
//! reject nothing a hash comparison accepted. Size earns its place in the
//! sidecar for a different job — see [`Sidecar::size`] and [`refresh`] — namely
//! keeping the recorded hash *fresh* cheaply, without which every guard above
//! would be guarding a hash that stopped describing the file months ago.

use super::index::{AssetIndex, Insertion};
use super::{empty_hash, measure_file, sidecar_path, AssetGuid, Sidecar, SIDECAR_EXTENSION};
use std::collections::BTreeMap;
use std::fs::ReadDir;
use std::io;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// The subdirectory of a project that holds its assets, and the only place
/// [`scan`] looks.
const ASSETS_DIR: &str = "assets";

/// A directory directly under `assets/` that [`scan`] never descends into.
///
/// `assets/tests/*.testlog.json` are recorded input traces for the E2E
/// harness, not assets: nothing loads them, nothing references them by path,
/// and a recording that gained an identity would only mean a `.meta` per
/// recording for nobody to read. Excluded by *name* rather than by extension so
/// that a `.ron` or `.js` fixture dropped in beside the recordings is excluded
/// too — the reason to skip this directory is what it is for, not what happens
/// to be in it today.
const RECORDINGS_DIR: &str = "tests";

/// Extensions that earn a file an identity.
///
/// # Why this is not `watcher::RELOADABLE_EXTENSIONS`, nor derived from it
///
/// The two lists look almost the same and answer different questions — "can
/// `bevy_asset` reload this?" versus "does this deserve an identity?" — and the
/// tempting simplification (define this one as that one plus `js` and `ron`) is
/// unsafe in a way that is easy to miss, because **the cost of over-inclusion
/// is opposite in the two lists**.
///
/// `RELOADABLE_EXTENSIONS` can afford to be generous, and its own documentation
/// says so outright: an extension listed there that no loader serves reaches
/// the `AssetServer` check, finds nothing, and is dropped — "so when in doubt,
/// add the extension". Here, an extension listed by mistake means a `.meta`
/// written next to every file that has it, forever, in the user's source tree.
/// Deriving this list from that one would quietly import a deliberate
/// permissiveness into the one place it does damage.
///
/// So the overlap is copied, not shared, and the two are expected to drift:
/// `js` and `ron` are here and not there precisely because neither ever reaches
/// `bevy_asset` (scenes go through `std::fs::read_to_string`, scripts through
/// `bsengine-scripting`), and a later sub-item that gives scripts real hot
/// reload will add `js` *there* without changing anything here.
///
/// # Why an allow-list rather than a deny-list
///
/// `games/mini-arena/assets/models/CREDITS.md` is a real file under a real
/// `assets/` directory. A deny-list would have to name `.md` — and then the
/// next README, `.txt`, `.blend1` backup and editor swap file, each discovered
/// only after it had already been sidecared in someone's working copy.
///
/// # Why scenes are in here
///
/// `.ron` scenes are referenced by path from `project.toml`'s `entry_scene` and
/// from JavaScript — `games/tilt-run/assets/scripts/goal_level1.js` holds
/// `const NEXT_SCENE = "assets/scenes/level2.ron"`. Renaming a scene today
/// silently breaks tilt-run's level chain, which is exactly the failure item 30
/// exists to end.
const IDENTIFIED_EXTENSIONS: &[&str] = &[
    // Everything `bevy_asset` serves in this engine, mirroring
    // `watcher::RELOADABLE_EXTENSIONS` for the reasons above.
    "glb", "gltf", "png", "jpg", "jpeg", "hdr", "wgsl", "wav", "ogg", "mp3", "flac",
    // Referenced by path but loaded outside `bevy_asset` entirely, so they
    // never appear in that list and are exactly what item 30 adds.
    "js", "ron",
];

/// Walks `<project_dir>/assets`, giving every eligible file a `.meta` sidecar
/// if it does not already have one, and returns what it found.
///
/// An asset that already has a readable sidecar keeps the identity in it —
/// that is the entire point, since an identity that changed across a restart
/// would make every reference stored against it worthless. Only a file with no
/// sidecar at all is hashed, and even then it is given a *fresh* identity only
/// once orphan recovery has failed to find it an old one; see the module docs.
///
/// # What it skips, silently
///
/// * Anything whose extension is not in [`IDENTIFIED_EXTENSIONS`], which is
///   most of what a `.gitignore`-clean `assets/` directory does not contain and
///   all of what it accidentally does — READMEs, `.blend1` backups, swap files.
/// * `assets/tests/`, the E2E recordings.
/// * The `.meta` sidecars themselves, or a rescan would sidecar the sidecars.
/// * Symlinks, junctions and anything else that is neither a plain file nor a
///   plain directory. Not following a directory link is what keeps a link cycle
///   from hanging startup, and startup is where this runs; a linked *file*
///   could be followed safely but is skipped with it, because its sidecar would
///   land beside the link rather than beside the asset, and the identity would
///   then describe this project's link instead of the thing being identified.
///   Logged at `debug!` — a deliberate policy, not an anomaly.
///
/// # What it warns about, and still continues past
///
/// A file that cannot be hashed, a sidecar that cannot be written, a directory
/// that cannot be opened, a sidecar that exists but cannot be parsed, and a
/// sidecar whose identity another file already claims — the copy-paste case
/// [`AssetIndex`] describes. Each names the path and leaves that one asset out
/// of the index. See the module docs for why none of these aborts the scan.
///
/// Orphan recovery adds four more, each about an identity that was *not*
/// recovered: a match too ambiguous to act on, an orphan nothing in the project
/// matches, an orphan whose asset was empty, and an orphan holding an identity
/// some live asset already carries.
///
/// # Errors
///
/// Only when `<project_dir>/assets` cannot be opened at all, including when it
/// does not exist. That is a different kind of problem from anything found
/// *inside* the directory: it almost always means the project directory is
/// wrong, and answering with an empty index would report "this project has no
/// assets" for what is really "I looked in the wrong place" — the same
/// split-brain the crate docs describe the engine having already shipped once.
/// Callers for which an assets-less project is normal can check for
/// [`io::ErrorKind::NotFound`].
///
/// # Cost
///
/// A rescan of an unchanged project hashes **nothing**. The work is one
/// `read_dir` per directory, one small `read_to_string` per asset for its
/// sidecar, and one `stat` per asset to compare the length on disk against the
/// length that sidecar records. Only a length that differs earns a re-hash, and
/// only that asset — see [`refresh`], which is what keeps `Sidecar::hash`
/// meaning "the contents now" rather than "the contents when this identity was
/// minted", at a cost that does not grow with the size of the project.
///
/// Orphan recovery does not change that. It hashes only assets that have *no*
/// sidecar, and those have to be hashed anyway to mint one, so each is hashed
/// exactly once and the digest serves both purposes. A project where nothing has
/// moved has no such assets and no orphans, and pays for recovery with one
/// comparison against an empty list.
pub fn scan(project_dir: impl AsRef<Path>) -> io::Result<AssetIndex> {
    let root = project_dir.as_ref().join(ASSETS_DIR);
    let entries = std::fs::read_dir(&root)?;

    let mut index = AssetIndex::default();
    let mut found = Found::default();
    walk(entries, ASSETS_DIR, &mut index, &mut found);
    settle(found, &mut index);
    Ok(index)
}

/// What one walk found and deliberately did not act on.
///
/// Both halves are deferred for the same reason: whether a file with no sidecar
/// should be given a *fresh* identity depends on whether an orphaned sidecar
/// elsewhere in the project is holding the one it already had, and no walk knows
/// that until it has finished walking. Deferring the writes also means nothing
/// is created inside a directory while that directory is still being iterated.
#[derive(Default)]
struct Found {
    /// Sidecars whose asset is no longer beside them.
    orphans: Vec<Orphan>,
    /// Eligible assets with no sidecar: each needs an identity, and each is a
    /// candidate for an orphan's.
    unidentified: Vec<Candidate>,
}

/// A sidecar left behind by an asset that is not next to it any more.
struct Orphan {
    /// The sidecar file itself, to be deleted once its identity has moved.
    meta: PathBuf,
    /// The project-relative path of the asset it identifies, which no file
    /// occupies — recorded as a former path if the identity is recovered.
    was: String,
    /// The lowercased extension of the asset it identifies, which a candidate
    /// has to share before contents are allowed to mean anything. Lowercased
    /// because `FOX.GLB` and `fox.glb` are the same kind of file, and a rename
    /// that only changed the casing must not cost the identity.
    extension: String,
    /// The identity it is holding on to.
    sidecar: Sidecar,
}

/// An eligible file with no sidecar of its own.
struct Candidate {
    /// Where the file is, for reading and for writing its sidecar beside it.
    path: PathBuf,
    /// The same file spelled the way a scene would reference it.
    relative: String,
    /// Its lowercased extension, matched against an orphan's — see [`Orphan`].
    extension: String,
}

/// What reading an asset that has no sidecar found out about its contents.
///
/// The pair is produced by one [`measure_file`] call so the two always describe
/// the same bytes, and it is used twice: to look for an orphan that recorded
/// exactly these contents, and — when there is none — as what the freshly
/// minted sidecar records.
struct Contents {
    /// The `blake3:`-prefixed digest.
    hash: String,
    /// The length in bytes, which [`refresh`] later uses to notice an edit.
    size: u64,
}

/// Recurses through one already-opened directory, whose project-relative path
/// is `prefix` (`assets`, then `assets/models`, …).
///
/// Takes the opened [`ReadDir`] rather than a path so there is exactly one
/// place that decides what a failed open means: [`scan`] returns it for the
/// root and warns for everything below, and neither can drift into doing the
/// other's thing.
fn walk(entries: ReadDir, prefix: &str, index: &mut AssetIndex, found: &mut Found) {
    // Sorted by name, because one of this scan's outcomes really does depend on
    // arrival order: when two assets claim one GUID — an asset copied together
    // with its `.meta` — [`AssetIndex`] keeps whichever it was given first. Raw
    // `read_dir` order is the filesystem's own, so without this the same
    // project would report a different file as unidentified on a colleague's
    // machine, or on the same machine after a defragment. Sorting makes
    // "first" mean the same thing everywhere; collecting is what makes sorting
    // possible, and costs one directory's worth of entries at a time rather
    // than the whole tree's.
    let mut entries: Vec<_> = entries.collect();
    entries.sort_by_key(|entry| entry.as_ref().ok().map(std::fs::DirEntry::file_name));

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                warn!(
                    "asset identity: cannot read an entry of {prefix} ({e}); \
                     whatever it is will have no identity this run"
                );
                continue;
            }
        };

        // `DirEntry::file_type` describes the entry itself and does not follow
        // links, which is what makes the symlink skip below cycle-proof.
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(e) => {
                warn!(
                    "asset identity: cannot tell what {} is ({e}); skipping it",
                    entry.path().display()
                );
                continue;
            }
        };

        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            warn!(
                "asset identity: {} is not valid UTF-8, so it cannot be spelled \
                 the way a scene would reference it; it will have no identity",
                entry.path().display()
            );
            continue;
        };
        let relative = format!("{prefix}/{name}");

        if file_type.is_dir() {
            if prefix == ASSETS_DIR && name == RECORDINGS_DIR {
                debug!("asset identity: {relative} holds E2E recordings, not assets; skipping");
                continue;
            }
            match std::fs::read_dir(entry.path()) {
                Ok(entries) => walk(entries, &relative, index, found),
                Err(e) => warn!(
                    "asset identity: cannot open {relative} ({e}); nothing inside \
                     it will have an identity this run"
                ),
            }
        } else if file_type.is_file() {
            identify(&entry.path(), &relative, index, found);
        } else {
            debug!(
                "asset identity: {relative} is a link rather than a file or a \
                 directory; the scan does not follow links, so it has no identity"
            );
        }
    }
}

/// Gives one file an identity, if it is the kind of file that gets one.
fn identify(path: &Path, relative: &str, index: &mut AssetIndex, found: &mut Found) {
    let Some(extension) = path.extension().and_then(|e| e.to_str()) else {
        return;
    };
    // Lowercased because `FOX.GLB` is an ordinary thing to find on a
    // case-insensitive filesystem, and an asset that silently missed out on an
    // identity because of how its extension was typed would be found much later
    // than it was created. Matches how `watcher::reconstruct` reads extensions.
    let extension = extension.to_ascii_lowercase();

    // Stated rather than left to fall out of the allow-list not containing
    // `meta`: what stops `fox.glb.meta.meta` appearing on the second scan
    // should be visible at the point it would happen, not inferred from a list
    // someone may later add to.
    if extension == SIDECAR_EXTENSION {
        note_if_orphaned(path, relative, found);
        return;
    }
    if !is_identified(&extension) {
        return;
    }

    let meta = sidecar_path(path);
    match Sidecar::read(&meta) {
        // The ordinary rescan: this asset was identified on some earlier run
        // and keeps what it was given. Its record of the *contents* may still
        // need catching up — see `refresh` — but the identity never changes.
        Ok(Some(existing)) => {
            let existing = refresh(path, relative, &meta, existing);
            record(&existing, relative, index);
        }

        // Never seen *here* before — which is not the same as never seen. An
        // orphaned sidecar somewhere else in the project may be holding this
        // file's identity, and the walk cannot know until it has finished, so
        // the decision waits for `settle`.
        Ok(None) => found.unidentified.push(Candidate {
            path: path.to_path_buf(),
            relative: relative.to_string(),
            extension,
        }),

        // A sidecar that exists and will not parse. Minting a replacement here
        // would turn "someone hand-edited this badly" into "every reference to
        // this asset now points at nothing", silently and permanently, which is
        // the single worst thing this module could do. So it is left exactly as
        // it is and a human is told where to look.
        Err(e) => warn!(
            "asset identity: {} exists but cannot be read ({e}); leaving it \
             untouched and treating {relative} as unidentified. A fresh identity \
             would silently break every reference to this asset, so repair or \
             delete the file by hand",
            meta.display()
        ),
    }
}

/// Whether an already-lowercased extension earns a file an identity.
fn is_identified(extension: &str) -> bool {
    IDENTIFIED_EXTENSIONS.contains(&extension)
}

/// Brings an existing sidecar's record of the asset's contents up to date, for
/// one `stat` per asset rather than one full read.
///
/// # Why a hash that is never refreshed is worse than no hash
///
/// [`Sidecar::hash`] is the *only* thread orphan recovery has back to a lost
/// identity. Left at "the contents when this identity was minted", it stops
/// describing the file the first time an artist saves over it — so edit an asset
/// today, rename it next month, and recovery finds nothing that matches and
/// mints a fresh GUID. That is precisely Unity's behaviour, arrived at silently,
/// in the feature built to beat it. The staleness is worst exactly where it
/// hurts most, too: an asset nobody has touched in a year is the one whose hash
/// is still correct.
///
/// Re-hashing everything on every scan is the obvious fix and not an affordable
/// one — it is a full read of every mesh and texture in the project at every
/// startup.
///
/// # The length on disk is the gate
///
/// An asset whose size still matches what its sidecar recorded is taken to be
/// unchanged and is not opened at all, so a rescan of an untouched project
/// still hashes **nothing**. A size that differs earns exactly one re-hash, and
/// the refreshed pair is written back so the next scan is cheap again.
///
/// A sidecar with no size at all — every sidecar written before that field
/// existed — reads as *unknown* rather than as zero, and is re-hashed once for
/// the same reason. That is what makes the format change a migration rather
/// than a mass re-identification: the GUID is never touched, so every reference
/// already stored against it keeps resolving.
///
/// # What it deliberately misses
///
/// An edit that leaves the length unchanged. Catching it would cost the full
/// read this whole design exists to avoid, and its consequence is bounded and
/// loud: recovery declines to re-pair and says so, rather than pairing wrongly.
fn refresh(path: &Path, relative: &str, meta: &Path, sidecar: Sidecar) -> Sidecar {
    let size = match path.metadata() {
        Ok(metadata) => metadata.len(),
        // The file was walked moments ago, so this is an anomaly rather than an
        // absence. Keeping the identity *and* the contents it records is
        // strictly better than losing either over one failed `stat`.
        Err(e) => {
            warn!(
                "asset identity: cannot read the size of {relative} ({e}); it \
                 keeps its identity, but the contents its sidecar records may \
                 be stale, so a later move may not be recoverable"
            );
            return sidecar;
        }
    };
    if sidecar.size == Some(size) {
        return sidecar;
    }

    let (hash, size) = match measure_file(path) {
        Ok(measured) => measured,
        Err(e) => {
            warn!(
                "asset identity: {relative} has changed since its sidecar was \
                 written but cannot be re-read ({e}); it keeps its identity, \
                 and the contents its sidecar records stay stale"
            );
            return sidecar;
        }
    };

    let refreshed = Sidecar {
        hash,
        size: Some(size),
        ..sidecar
    };
    if let Err(e) = refreshed.write(meta) {
        // Nothing is lost that was not already lost: the identity is untouched
        // on disk and still correct, and only the freshness of the hash fails
        // to persist. The next scan sees the same mismatch and tries again.
        warn!(
            "asset identity: {relative} has changed, but recording that in {} \
             failed ({e}); it keeps its identity, and the next scan will try \
             again",
            meta.display()
        );
    }
    // Returned whether or not the write took: the identity and the former
    // paths — everything `record` reads — are the same in both, and the
    // caller's job is to index the asset, not to mirror the disk.
    refreshed
}

/// Notes a sidecar whose asset is not beside it — the situation the whole of
/// orphan recovery exists for.
///
/// The `is_file` check is one `stat` per sidecar, which is nothing next to the
/// `read_to_string` the same directory already costs for the asset it belongs
/// to, and it is what makes the answer independent of the order `read_dir`
/// happened to yield the two files in.
fn note_if_orphaned(meta: &Path, relative: &str, found: &mut Found) {
    // `fox.glb.meta` identifies `fox.glb`: strip the extension this module
    // appended, exactly as `sidecar_path` added it, so `fox.glb.meta` and
    // `fox.png.meta` stay two different questions.
    let asset = meta.with_extension("");
    let Some((was, _)) = relative.rsplit_once('.') else {
        return;
    };

    // A `.meta` beside something this scan would never have identified is not an
    // orphan of ours; it is a stray file, and recovering an identity onto a file
    // type we do not sidecar would be inventing a rule nothing else follows.
    //
    // The extension is kept rather than discarded once checked: recovery needs
    // it to refuse a candidate that merely shares this asset's bytes without
    // being the same kind of file at all.
    let extension = asset
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .filter(|e| is_identified(e));
    let Some(extension) = extension else {
        return;
    };
    if asset.is_file() {
        return;
    }

    match Sidecar::read(meta) {
        Ok(Some(sidecar)) => found.orphans.push(Orphan {
            meta: meta.to_path_buf(),
            was: was.to_string(),
            extension,
            sidecar,
        }),
        // Gone between the walk and the read; there is nothing left to recover.
        Ok(None) => {}
        Err(e) => warn!(
            "asset identity: {} identifies {was}, which is not there, and cannot \
             itself be read ({e}); the identity it holds cannot be recovered. \
             Repair or delete the file by hand",
            meta.display()
        ),
    }
}

/// Settles every identity the walk deferred: each asset without a sidecar takes
/// back an orphaned identity where — and only where — exactly one orphan and
/// exactly one asset can possibly be talking about each other, and a fresh one
/// otherwise.
fn settle(found: Found, index: &mut AssetIndex) {
    let Found {
        orphans,
        unidentified,
    } = found;

    // Before anything is counted, let alone paired: an orphan whose identity a
    // live asset already carries is not a competitor for it.
    let orphans = recoverable(orphans, index);

    // The gate, and the whole cost of recovery in a project where nothing has
    // moved: one comparison against an empty list. Note what this branch does
    // *not* save — the hashing below is not recovery's, it is the hashing that
    // minting an identity has always required.
    if orphans.is_empty() {
        for candidate in &unidentified {
            if let Some(contents) = read_contents(candidate) {
                mint(candidate, &contents, index);
            }
        }
        return;
    }

    // Read once, used twice: to look for an orphan that recorded these exact
    // contents, and — when there is none — as what the sidecar minted below
    // records. An asset that cannot be read is dropped here, warned about, and
    // is neither a candidate nor minted.
    let candidates: Vec<(Candidate, Contents)> = unidentified
        .into_iter()
        .filter_map(|candidate| read_contents(&candidate).map(|contents| (candidate, contents)))
        .collect();

    // Keyed by extension *and* hash, so a `.ron` scene and a `.js` script that
    // happen to hold the same bytes are never in the same bucket — see the
    // module docs on what a hash alone does not prove.
    let by_contents = group(
        candidates
            .iter()
            .map(|(candidate, contents)| (candidate.extension.as_str(), contents.hash.as_str())),
    );
    let orphans_by_contents = group(
        orphans
            .iter()
            .map(|orphan| (orphan.extension.as_str(), orphan.sidecar.hash.as_str())),
    );

    let empty = empty_hash();
    // "Settled", not "recovered": a repair whose write failed leaves the asset
    // unidentified this run *and* still must not be minted, because minting is
    // what would make the loss permanent. See `repair`.
    let mut settled = vec![false; candidates.len()];
    for (&(extension, hash), orphaned) in &orphans_by_contents {
        // Refused before it is even looked up. Every empty file in the project
        // hashes to this one value, so a match on it is not evidence of
        // anything, and several empty placeholders is an ordinary thing to have.
        if hash == empty {
            for &orphan in orphaned {
                warn!(
                    "asset identity: {} was left behind by an asset that is no \
                     longer beside it, and that asset was empty. Every empty \
                     file hashes to the same value, so contents cannot say which \
                     file it became, and the identity it holds cannot be \
                     recovered. If the asset is gone for good, delete this \
                     sidecar",
                    orphans[orphan].meta.display()
                );
            }
            continue;
        }

        let matched: &[usize] = by_contents
            .get(&(extension, hash))
            .map_or(&[], Vec::as_slice);
        match (orphaned.as_slice(), matched) {
            // Nothing in the project has these contents any more. An asset that
            // was edited as well as moved lands here and is indistinguishable
            // from one that was deleted, which is the honest limit of hashing.
            (_, []) => {
                for &orphan in orphaned {
                    warn!(
                        "asset identity: {} was left behind by an asset that is \
                         no longer beside it, and no .{extension} file in the \
                         project has the contents it recorded, so the identity \
                         it holds cannot be recovered. An asset that was edited \
                         as well as moved hashes differently and cannot be told \
                         apart from a new one; if the asset is gone for good, \
                         delete this sidecar",
                        orphans[orphan].meta.display()
                    );
                }
            }
            ([orphan], [position]) => {
                let (candidate, contents) = &candidates[*position];
                settled[*position] = repair(&orphans[*orphan], candidate, contents, index);
            }
            // The refusal this whole design turns on. Naming both sides is the
            // product: the user moved files, lost their identities, and this
            // line is the only chance to learn it before a scene fails to load.
            _ => warn!(
                "asset identity: {} were left behind by assets that are no longer \
                 beside them, and {} have exactly those contents. Which sidecar \
                 belongs to which file is a coin flip, and a wrong guess silently \
                 reattaches every reference to the wrong asset, so no guess was \
                 made: those assets have been given fresh identities and the old \
                 ones are unrecoverable. To undo this, move each sidecar next to \
                 the asset it belongs to, replacing the fresh one, and rescan",
                listed(
                    orphaned
                        .iter()
                        .map(|&i| orphans[i].meta.display().to_string())
                ),
                listed(matched.iter().map(|&i| candidates[i].0.relative.clone())),
            ),
        }
    }

    for (position, (candidate, contents)) in candidates.iter().enumerate() {
        if !settled[position] {
            mint(candidate, contents, index);
        }
    }
}

/// Drops the orphans that can never be recovered onto anything, telling the
/// user about each, and keeps the rest.
///
/// A sidecar *copied* rather than moved leaves one of these: the file it names
/// is gone, but the identity inside it is on a file that really exists. Handing
/// it out again would put two `.meta` files claiming one GUID into the user's
/// source tree, which is the contradiction [`AssetIndex`] refuses to resolve —
/// so it is never created in the first place.
///
/// Removing them *before* anything is grouped is what makes this more than
/// tidiness. Left in the pool, such an orphan still counts as a claimant on its
/// contents, so a second orphan that shares them — one that has a perfectly
/// unambiguous asset waiting for it — is pushed into the "two of these, no way
/// to tell them apart" refusal by a competitor that was never eligible to win.
/// One identity is then lost to the presence of another that could not have
/// been recovered either way.
///
/// This is a filter and not the guard: an identity can also go live *during*
/// settling, when an earlier pairing records one. [`repair`] makes the same
/// check at the moment it would hand an identity out, which is the choke point
/// this pass cannot be.
fn recoverable(orphans: Vec<Orphan>, index: &AssetIndex) -> Vec<Orphan> {
    orphans
        .into_iter()
        .filter(|orphan| match index.path_for_guid(orphan.sidecar.guid) {
            None => true,
            Some(live) => {
                warn_copied(&orphan.meta, live);
                false
            }
        })
        .collect()
}

/// Reports a sidecar whose identity is on a file that is still there, and says
/// what to do with it.
///
/// One message for both places that can discover this — see [`recoverable`] —
/// because it is one situation, and a user who saw two different explanations
/// of it would reasonably think they had two different problems.
fn warn_copied(meta: &Path, live: &str) {
    warn!(
        "asset identity: {} was left behind by an asset that is no longer \
         beside it, but the identity it holds already belongs to {live}, so it \
         was copied rather than moved. One identity cannot mean two files, so \
         it can never be recovered onto anything: delete this sidecar",
        meta.display()
    );
}

/// Buckets files that agree on both extension and hash together, keeping each
/// item's position.
///
/// A `BTreeMap` so that a project with several orphans reports them in the same
/// order on every machine, rather than in whatever order the hashes happened to
/// land in a bucket array.
fn group<'a>(
    contents: impl Iterator<Item = (&'a str, &'a str)>,
) -> BTreeMap<(&'a str, &'a str), Vec<usize>> {
    let mut grouped: BTreeMap<(&str, &str), Vec<usize>> = BTreeMap::new();
    for (position, key) in contents.enumerate() {
        grouped.entry(key).or_default().push(position);
    }
    grouped
}

/// Comma-joins paths for a message whose whole job is to name everything the
/// scan refused to choose between.
fn listed(paths: impl Iterator<Item = String>) -> String {
    paths.collect::<Vec<_>>().join(", ")
}

/// Hashes and measures an asset that has no sidecar, reporting the file it
/// could not read.
fn read_contents(candidate: &Candidate) -> Option<Contents> {
    match measure_file(&candidate.path) {
        Ok((hash, size)) => Some(Contents { hash, size }),
        Err(e) => {
            warn!(
                "asset identity: cannot read {} to hash it ({e}); it will have no \
                 identity until the next scan that can",
                candidate.relative
            );
            None
        }
    }
}

/// Moves an orphaned sidecar's identity onto the asset that turned up with its
/// contents, and says whether the candidate still needs one of its own.
///
/// `true` means settled: do not mint. That covers the write having failed as
/// well as the repair having worked, because minting a fresh identity as a
/// fallback would be the one unrecoverable move — see below.
fn repair(
    orphan: &Orphan,
    candidate: &Candidate,
    contents: &Contents,
    index: &mut AssetIndex,
) -> bool {
    // The last gate before an identity is handed out, and the only one that can
    // see an identity which went live *during* this settling — recovered onto
    // some earlier candidate by a repair a moment ago. `recoverable` cannot:
    // it runs once, before any of that has happened.
    if let Some(live) = index.path_for_guid(orphan.sidecar.guid) {
        warn_copied(&orphan.meta, live);
        return false;
    }

    let mut sidecar = orphan.sidecar.clone();
    // The hash is already equal — it is why these two were paired — but the
    // *size* may not be recorded at all, because the orphan can be a sidecar
    // written before that field existed. Writing the pair the candidate was
    // just measured with is what stops the very next scan re-hashing an asset
    // it has this second finished hashing.
    sidecar.hash = contents.hash.clone();
    sidecar.size = Some(contents.size);
    // Deduplicated, so that a file moved back and forth cannot grow this list
    // without bound: it can only ever hold the distinct paths the asset has
    // actually occupied.
    if !sidecar.former_paths.contains(&orphan.was) {
        sidecar.former_paths.push(orphan.was.clone());
    }

    let meta = sidecar_path(&candidate.path);
    if let Err(e) = sidecar.write(&meta) {
        // The orphan is deliberately left where it is, so the next scan sees
        // the same pair and tries the same repair. Minting a fresh identity as
        // a fallback would be the one unrecoverable move: the asset would stop
        // being a candidate, and the identity really would be lost.
        warn!(
            "asset identity: {} has the contents {} recorded, but writing {} \
             failed ({e}); it has no identity this run, and the orphaned sidecar \
             is left in place so the next scan can try again",
            candidate.relative,
            orphan.meta.display(),
            meta.display()
        );
        return true;
    }

    if let Err(e) = std::fs::remove_file(&orphan.meta) {
        warn!(
            "asset identity: {} recovered the identity {} was holding, but that \
             sidecar could not be deleted ({e}); delete it by hand, or every \
             later scan will report an orphan that has already been recovered",
            candidate.relative,
            orphan.meta.display()
        );
    }

    info!(
        "asset identity: {} has the contents {} was identified by and no sidecar \
         of its own, so it keeps that identity instead of being treated as a new \
         asset",
        candidate.relative, orphan.was
    );
    record(&sidecar, &candidate.relative, index);
    true
}

/// Gives an asset that has never been identified a fresh identity.
fn mint(candidate: &Candidate, contents: &Contents, index: &mut AssetIndex) {
    let sidecar = Sidecar {
        guid: AssetGuid::new(),
        hash: contents.hash.clone(),
        size: Some(contents.size),
        former_paths: Vec::new(),
    };
    let meta = sidecar_path(&candidate.path);
    if let Err(e) = sidecar.write(&meta) {
        // Not indexed: an identity that did not reach disk is a different
        // identity on the next run, so handing this one out would be worse than
        // admitting the asset is unidentified.
        warn!(
            "asset identity: cannot write {} ({e}); {} will have no identity, and \
             would be given a different one every run if it did",
            meta.display(),
            candidate.relative
        );
        return;
    }
    record(&sidecar, &candidate.relative, index);
}

/// Puts one asset's sidecar into the index, and tells a human when the index
/// will not have it.
///
/// The index refuses a contradiction rather than resolving it — see
/// [`AssetIndex`] — and a refusal that nobody reported would leave an asset
/// quietly unidentified with nothing to act on, which is the failure mode this
/// whole module is built to avoid.
fn record(sidecar: &Sidecar, relative: &str, index: &mut AssetIndex) {
    match index.insert(sidecar.guid, relative, &sidecar.former_paths) {
        Insertion::Recorded => {}
        rejected @ (Insertion::DuplicateGuid { .. } | Insertion::DuplicatePath { .. }) => {
            warn!("asset identity: {relative} will have no identity this run — {rejected}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{capture_warnings, unique, ProbeDir};
    use std::path::PathBuf;

    /// Writes a probe file, creating whatever directories it needs.
    fn write_file(path: &Path, contents: &[u8]) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create probe directories");
        }
        std::fs::write(path, contents).expect("write probe file");
    }

    /// Reads a sidecar's raw text, so a test can assert on the bytes on disk
    /// rather than on a value that round-tripped through the parser.
    fn read_text(path: &Path) -> String {
        std::fs::read_to_string(path).expect("read sidecar")
    }

    #[test]
    fn a_scan_gives_every_eligible_asset_a_sidecar_and_skips_the_rest() {
        let probe = ProbeDir(std::env::temp_dir().join(unique("identity-scan")));
        let assets = probe.0.join("assets");
        write_file(&assets.join("models/fox.glb"), b"fake glb");
        write_file(&assets.join("scripts/player.js"), b"// script");
        write_file(&assets.join("scenes/main.ron"), b"()");
        write_file(&assets.join("models/CREDITS.md"), b"# credits");

        let index = scan(&probe.0).expect("scan");

        assert!(assets.join("models/fox.glb.meta").exists());
        assert!(assets.join("scripts/player.js.meta").exists());
        assert!(assets.join("scenes/main.ron.meta").exists());
        assert!(
            !assets.join("models/CREDITS.md.meta").exists(),
            "a README is not an asset; the allow-list is what stops the scan \
             sidecaring every stray file"
        );
        assert_eq!(index.len(), 3);
    }

    #[test]
    fn a_second_scan_keeps_the_guids_the_first_one_assigned() {
        // The whole point: identity has to survive a restart, or references
        // stored against it are worthless.
        let probe = ProbeDir(std::env::temp_dir().join(unique("identity-rescan")));
        write_file(&probe.0.join("assets/models/fox.glb"), b"fake glb");

        let first = scan(&probe.0).expect("scan");
        let second = scan(&probe.0).expect("rescan");

        assert_eq!(
            first.guid_for_path("assets/models/fox.glb"),
            second.guid_for_path("assets/models/fox.glb"),
            "a rescan must reuse the sidecar, not mint a new identity"
        );
    }

    // The one failure mode where doing the obvious thing is catastrophic. A
    // `.meta` that will not parse is indistinguishable, to a scan that does not
    // look, from one that is absent -- and "absent" means mint. Minting here
    // would replace an identity that scenes may already reference, with nothing
    // said and no way back.
    #[test]
    fn a_malformed_sidecar_is_left_alone_rather_than_replaced() {
        let probe = ProbeDir(std::env::temp_dir().join(unique("identity-broken")));
        let asset = probe.0.join("assets/models/fox.glb");
        write_file(&asset, b"fake glb");
        let meta = probe.0.join("assets/models/fox.glb.meta");
        write_file(&meta, b"this is not ron");

        let (index, logs) = capture_warnings(|| scan(&probe.0).expect("scan"));

        assert_eq!(
            read_text(&meta),
            "this is not ron",
            "the broken sidecar must be exactly as the developer left it -- \
             overwriting it is how a hand-edit becomes a permanently broken \
             reference"
        );
        assert_eq!(
            index.guid_for_path("assets/models/fox.glb"),
            None,
            "an asset whose identity could not be read is unidentified, not \
             newly identified"
        );
        assert!(
            logs.contains("fox.glb.meta"),
            "silently skipping the asset would leave nothing to act on; the \
             warning must name the file to repair -- got:\n{logs}"
        );
    }

    // Excluded by directory name, not by extension: today the recordings are
    // `.json` and the allow-list would reject them anyway, so a scan that only
    // *looked* like it skipped this directory would pass. A `.ron` in there is
    // what tells the two apart.
    #[test]
    fn the_recordings_directory_is_skipped_whatever_is_in_it() {
        let probe = ProbeDir(std::env::temp_dir().join(unique("identity-tests")));
        write_file(
            &probe.0.join("assets/tests/level1-clear.testlog.json"),
            b"[]",
        );
        write_file(&probe.0.join("assets/tests/fixture.ron"), b"()");
        write_file(&probe.0.join("assets/scenes/main.ron"), b"()");

        let index = scan(&probe.0).expect("scan");

        assert!(
            !probe.0.join("assets/tests/fixture.ron.meta").exists(),
            "assets/tests holds E2E recordings, not assets -- skipping it by \
             extension only would sidecar this one"
        );
        assert_eq!(
            index.guid_for_path("assets/tests/fixture.ron"),
            None,
            "nothing under assets/tests belongs in the index"
        );
        assert!(index.guid_for_path("assets/scenes/main.ron").is_some());
        assert_eq!(index.len(), 1);
    }

    // Windows filesystems are case-insensitive and artists' exporters are not
    // consistent. An asset that missed out on an identity because of how its
    // extension happened to be typed would be discovered long after it was
    // created, by a reference that broke.
    #[test]
    fn an_extension_is_matched_whatever_case_it_is_typed_in() {
        let probe = ProbeDir(std::env::temp_dir().join(unique("identity-case")));
        write_file(&probe.0.join("assets/models/FOX.GLB"), b"fake glb");

        let index = scan(&probe.0).expect("scan");

        assert!(
            probe.0.join("assets/models/FOX.GLB.meta").exists(),
            "FOX.GLB is the same kind of file as fox.glb"
        );
        assert!(
            index.guid_for_path("assets/models/FOX.GLB").is_some(),
            "the path is indexed in the spelling it really has -- re-casing it \
             would make it unfindable on a case-sensitive filesystem"
        );
    }

    // ---- keeping the recorded contents fresh, cheaply ---------------------

    // The cost guarantee the whole design rests on: a rescan of an untouched
    // project reads no asset at all. Proved with an edit that keeps the file's
    // *length* -- the one change a size check cannot see -- because that is the
    // only edit whose invisibility shows the scan consulted the length rather
    // than the contents. An edit of a different length would look identical
    // here whether the scan hashed everything or nothing.
    #[test]
    fn a_rescan_does_not_read_an_asset_whose_length_has_not_changed() {
        let probe = ProbeDir(std::env::temp_dir().join(unique("identity-cost")));
        let asset = probe.0.join("assets/models/fox.glb");
        write_file(&asset, b"fake glb");
        let meta = probe.0.join("assets/models/fox.glb.meta");

        scan(&probe.0).expect("scan");
        let after_first = read_text(&meta);

        write_file(&asset, b"FAKE GLB");
        scan(&probe.0).expect("rescan");

        assert_eq!(
            read_text(&meta),
            after_first,
            "the sidecar changed, so the asset was hashed -- and if a rescan \
             hashes this one it hashes every mesh and texture in the project, \
             on every startup"
        );
    }

    // The other half of the same trade. An asset whose length *has* changed is
    // re-hashed exactly once, so `Sidecar::hash` keeps meaning "the contents
    // now" -- and the identity is untouched while that happens, because a
    // refresh that minted a new GUID would break every reference on the first
    // save.
    #[test]
    fn an_edited_asset_has_its_recorded_contents_refreshed_but_not_its_identity() {
        let probe = ProbeDir(std::env::temp_dir().join(unique("identity-refresh")));
        let asset = probe.0.join("assets/models/fox.glb");
        write_file(&asset, b"fake glb");
        let meta = probe.0.join("assets/models/fox.glb.meta");
        let original = scan(&probe.0)
            .expect("scan")
            .guid_for_path("assets/models/fox.glb")
            .unwrap();

        write_file(&asset, b"a different fake glb");
        let index = scan(&probe.0).expect("rescan");

        let (hash, size) = measure_file(&asset).expect("measure the edited asset");
        let sidecar = Sidecar::read(&meta).expect("read").expect("present");
        assert_eq!(
            sidecar.hash, hash,
            "the sidecar still records the contents this asset had before it \
             was edited"
        );
        assert_eq!(
            sidecar.size,
            Some(size),
            "the new length has to be recorded too, or the next scan re-hashes \
             this asset again -- and every scan after it"
        );
        assert_eq!(
            index.guid_for_path("assets/models/fox.glb"),
            Some(original),
            "saving an asset must never change its identity"
        );
        assert_eq!(sidecar.guid, original, "nor on disk");
    }

    // Why the refresh is worth its `stat`, stated as the failure it prevents.
    // An asset is edited, scanned a few times as anyone's project is, and only
    // months later moved without its sidecar. If the hash were still the one
    // minted with the identity, nothing in the project would match it and the
    // scan would mint a fresh GUID -- exactly Unity's behaviour, in the feature
    // built to beat it, arrived at silently.
    #[test]
    fn an_asset_edited_long_before_it_moves_is_still_recovered() {
        let probe = ProbeDir(std::env::temp_dir().join(unique("identity-stale")));
        let models = probe.0.join("assets/models");
        write_file(&models.join("fox.glb"), b"fake glb contents");
        let original = scan(&probe.0)
            .expect("scan")
            .guid_for_path("assets/models/fox.glb")
            .unwrap();

        // Ordinary work: the asset is edited where it has always lived, and the
        // engine starts a few more times.
        write_file(&models.join("fox.glb"), b"fake glb contents, retouched");
        scan(&probe.0).expect("rescan after the edit");
        scan(&probe.0).expect("and again");

        // Only now does it move, and without its `.meta`.
        std::fs::rename(models.join("fox.glb"), models.join("renamed.glb")).unwrap();

        let after = scan(&probe.0).expect("rescan after the move");

        assert_eq!(
            after.guid_for_path("assets/models/renamed.glb"),
            Some(original),
            "the asset was edited months before it moved, and a hash frozen at \
             the moment the identity was minted no longer describes it -- so \
             recovery finds nothing and silently mints a new identity"
        );
    }

    // The format change `size` makes, seen from the only angle that matters:
    // every sidecar already committed in every project has no size field. If a
    // scan could not read one, or read it as a reason to re-identify, one
    // upgrade would mint fresh GUIDs for a whole project at once.
    #[test]
    fn a_sidecar_from_before_the_size_field_keeps_its_identity_and_is_rehashed_once() {
        let probe = ProbeDir(std::env::temp_dir().join(unique("identity-migrate")));
        let asset = probe.0.join("assets/models/fox.glb");
        write_file(&asset, b"fake glb");
        let meta = probe.0.join("assets/models/fox.glb.meta");
        let guid = AssetGuid::new();
        // Exactly the shape this module wrote until the field existed, with a
        // hash that no longer matches the file so the re-hash is visible.
        write_file(
            &meta,
            format!("(guid: \"{guid}\", hash: \"blake3:stale\", former_paths: [])").as_bytes(),
        );

        let index = scan(&probe.0).expect("scan");

        assert_eq!(
            index.guid_for_path("assets/models/fox.glb"),
            Some(guid),
            "the migration must not touch the identity; re-minting here would \
             break every reference in the project in one startup"
        );
        let (hash, size) = measure_file(&asset).expect("measure");
        let migrated = Sidecar::read(&meta).expect("read").expect("present");
        assert_eq!(migrated.guid, guid);
        assert_eq!(
            migrated.hash, hash,
            "an unknown size means the recorded contents cannot be trusted, so \
             they have to be re-read once"
        );
        assert_eq!(
            migrated.size,
            Some(size),
            "and recorded, or this asset is re-hashed on every scan forever"
        );

        // Once, though. The second scan has a size to compare against.
        let after_migration = read_text(&meta);
        write_file(&asset, b"FAKE GLB");
        scan(&probe.0).expect("rescan");
        assert_eq!(
            read_text(&meta),
            after_migration,
            "the migrated sidecar was re-read again; the size it now records is \
             what stops that"
        );
    }

    #[test]
    fn the_sidecars_are_not_themselves_given_sidecars() {
        let probe = ProbeDir(std::env::temp_dir().join(unique("identity-nested")));
        write_file(&probe.0.join("assets/models/fox.glb"), b"fake glb");

        scan(&probe.0).expect("scan");
        let index = scan(&probe.0).expect("rescan");

        assert!(
            !probe.0.join("assets/models/fox.glb.meta.meta").exists(),
            "a sidecar is not an asset; sidecaring it would double the file \
             count of every assets directory on every scan"
        );
        assert_eq!(index.len(), 1);
    }

    // Copying an asset together with its .meta is one drag in Explorer, and it
    // is how two files come to claim one identity. The index keeps whichever it
    // saw first, which means *neither* file is guaranteed to be the one indexed
    // -- so this asserts the shape of the outcome rather than naming a winner,
    // and that the developer is told which file to repair.
    #[test]
    fn an_asset_copied_along_with_its_sidecar_does_not_steal_the_original_identity() {
        let probe = ProbeDir(std::env::temp_dir().join(unique("identity-copied")));
        let assets = probe.0.join("assets/models");
        let shared = Sidecar {
            guid: AssetGuid::new(),
            hash: "blake3:abc".to_string(),
            size: None,
            former_paths: Vec::new(),
        };
        for name in ["fox.glb", "fox_copy.glb"] {
            write_file(&assets.join(name), b"fake glb");
            shared
                .write(sidecar_path(assets.join(name)))
                .expect("write sidecar");
        }

        let (index, logs) = capture_warnings(|| scan(&probe.0).expect("scan"));

        let original = index.guid_for_path("assets/models/fox.glb");
        let copy = index.guid_for_path("assets/models/fox_copy.glb");
        assert_eq!(
            index.len(),
            1,
            "one identity means one asset; indexing both would make \
             path_for_guid pick a file by directory order"
        );
        let (kept, rejected) = match (original, copy) {
            (Some(_), None) => ("assets/models/fox.glb", "fox_copy.glb"),
            (None, Some(_)) => ("assets/models/fox_copy.glb", "fox.glb"),
            other => panic!("exactly one of the two must be indexed, got {other:?}"),
        };
        assert_eq!(
            index.path_for_guid(shared.guid),
            Some(kept),
            "the identity has to point back at the file that kept it"
        );
        assert!(
            logs.contains(rejected),
            "the developer has to be told which file to delete the .meta from, \
             or an asset is silently unidentified -- got:\n{logs}"
        );
    }

    // The previous test asserts the *shape* of the outcome without naming a
    // winner. Which file wins still has to be the same file everywhere, or one
    // developer sees an asset indexed that a colleague sees reported as
    // unidentified, for the same commit.
    //
    // The names are chosen to tell a sorted walk from an unsorted one on this
    // filesystem: NTFS yields directory entries ordered case-insensitively
    // (`a.glb`, then `B.glb`), while the scan sorts by the bytes of the name
    // (`B` is 0x42, `a` is 0x61, so `B.glb` first). Two names of the same case
    // would agree under both orders and prove nothing here.
    #[test]
    fn which_of_two_files_claiming_one_guid_keeps_it_does_not_follow_the_filesystem() {
        let probe = ProbeDir(std::env::temp_dir().join(unique("identity-order")));
        let models = probe.0.join("assets/models");
        let shared = Sidecar {
            guid: AssetGuid::new(),
            hash: "blake3:abc".to_string(),
            size: None,
            former_paths: Vec::new(),
        };
        for name in ["a.glb", "B.glb"] {
            write_file(&models.join(name), b"fake glb");
            shared
                .write(sidecar_path(models.join(name)))
                .expect("write sidecar");
        }

        let index = scan(&probe.0).expect("scan");

        assert_eq!(
            index.path_for_guid(shared.guid),
            Some("assets/models/B.glb"),
            "the walk has to impose its own order on a directory; taking the \
             filesystem's would hand the identity to a.glb here and to B.glb on \
             a filesystem that reads back in a different order"
        );
        assert_eq!(index.guid_for_path("assets/models/a.glb"), None);
    }

    // ---- what a matching hash is not enough to prove ----------------------

    // Small `.ron` and `.js` files collide constantly -- `()` is an entire
    // scene and an entire expression -- and this repository already contains
    // byte-identical files under different names. Letting contents alone decide
    // would hand a scene's identity to a script, and every reference to that
    // scene would then resolve to a file that cannot be loaded in its place.
    #[test]
    fn recovery_refuses_a_file_that_shares_only_the_bytes_and_not_the_kind() {
        let probe = ProbeDir(std::env::temp_dir().join(unique("identity-extension")));
        let assets = probe.0.join("assets");
        let shared = b"()";
        write_file(&assets.join("scenes/main.ron"), shared);
        let scene = scan(&probe.0)
            .expect("scan")
            .guid_for_path("assets/scenes/main.ron")
            .unwrap();

        // The scene is deleted, leaving its sidecar, and a script that happens
        // to hold exactly the same bytes turns up elsewhere in the project.
        std::fs::remove_file(assets.join("scenes/main.ron")).unwrap();
        write_file(&assets.join("scripts/stub.js"), shared);

        let (second, logs) = capture_warnings(|| scan(&probe.0).expect("rescan"));

        let stub = second
            .guid_for_path("assets/scripts/stub.js")
            .expect("the script still needs an identity of its own");
        assert_ne!(
            stub, scene,
            "a script inherited a scene's identity because their bytes matched; \
             every reference to that scene now resolves to a file no scene \
             loader can read"
        );
        assert!(
            assets.join("scenes/main.ron.meta").exists(),
            "the scene's sidecar was deleted as though the identity had been \
             successfully moved"
        );
        assert!(
            logs.contains("main.ron.meta"),
            "an identity that could not be recovered has to name the sidecar \
             still holding it -- got:\n{logs}"
        );
    }

    // The other half: same extension, same hash, and still not evidence,
    // because the hash is the one every empty file in the project has. Two
    // empty placeholders is an ordinary thing to have, and the whole point of
    // this test is that nothing else here can tell them apart.
    #[test]
    fn recovery_refuses_two_empty_files_however_exactly_their_hashes_agree() {
        let probe = ProbeDir(std::env::temp_dir().join(unique("identity-empty")));
        let scenes = probe.0.join("assets/scenes");
        write_file(&scenes.join("main.ron"), b"");
        let main = scan(&probe.0)
            .expect("scan")
            .guid_for_path("assets/scenes/main.ron")
            .unwrap();

        std::fs::remove_file(scenes.join("main.ron")).unwrap();
        write_file(&scenes.join("level2.ron"), b"");

        let (second, logs) = capture_warnings(|| scan(&probe.0).expect("rescan"));

        let level2 = second
            .guid_for_path("assets/scenes/level2.ron")
            .expect("the new placeholder still needs an identity of its own");
        assert_ne!(
            level2, main,
            "an unrelated empty file was handed the deleted scene's identity; \
             emptiness is not evidence that two files are the same asset"
        );
        assert!(
            scenes.join("main.ron.meta").exists(),
            "the sidecar was deleted as though the identity had moved"
        );
        assert!(
            logs.contains("main.ron.meta"),
            "the refusal has to name the sidecar it refused for -- got:\n{logs}"
        );
    }

    // The case both Unity and Godot lose. Unity mints a new GUID and deletes the
    // orphaned .meta; Godot asks you to move the .uid by hand. Our assets are
    // ordinary files with hashes, so the identity can follow the contents.
    #[test]
    fn a_moved_asset_is_repaired_from_its_orphaned_sidecar() {
        let probe = ProbeDir(std::env::temp_dir().join(unique("identity-orphan")));
        let models = probe.0.join("assets/models");
        write_file(&models.join("fox.glb"), b"fake glb contents");
        let first = scan(&probe.0).expect("scan");
        let original = first.guid_for_path("assets/models/fox.glb").unwrap();

        // The case both other engines lose: the file moved outside the engine
        // and its sidecar stayed behind.
        std::fs::rename(models.join("fox.glb"), models.join("renamed.glb")).unwrap();

        let second = scan(&probe.0).expect("rescan");
        assert_eq!(
            second.guid_for_path("assets/models/renamed.glb"),
            Some(original),
            "an orphaned sidecar whose hash matches exactly one unidentified \
             asset must be re-paired, not replaced with a new identity"
        );
    }

    #[test]
    fn an_ambiguous_hash_match_is_refused_rather_than_guessed() {
        let probe = ProbeDir(std::env::temp_dir().join(unique("identity-ambiguous")));
        let models = probe.0.join("assets/models");
        write_file(&models.join("a.glb"), b"identical");
        write_file(&models.join("b.glb"), b"identical");
        let first = scan(&probe.0).expect("scan");
        let was_a = first.guid_for_path("assets/models/a.glb").unwrap();
        let was_b = first.guid_for_path("assets/models/b.glb").unwrap();

        std::fs::rename(models.join("a.glb"), models.join("c.glb")).unwrap();
        std::fs::rename(models.join("b.glb"), models.join("d.glb")).unwrap();

        let second = scan(&probe.0).expect("rescan");
        let c = second.guid_for_path("assets/models/c.glb").unwrap();
        let d = second.guid_for_path("assets/models/d.glb").unwrap();

        assert_ne!(c, d, "two assets must never share one identity");
        // Two orphans, two candidates, identical hashes: any pairing would be
        // a coin flip, so both must get fresh identities. Reusing `was_a` or
        // `was_b` here would mean the scan guessed -- and a guess that lands
        // wrong silently reattaches every reference to the wrong file.
        for (label, guid) in [("c", c), ("d", d)] {
            assert!(
                guid != was_a && guid != was_b,
                "{label} reused an orphaned identity; the match was ambiguous \
                 and should have been refused"
            );
        }
    }

    // A refusal the user is not told about is indistinguishable from the silent
    // re-identification this whole module exists to prevent: the identities are
    // gone either way, and only the warning says why while it can still be undone.
    #[test]
    fn the_refusal_to_guess_names_every_file_it_could_not_tell_apart() {
        let probe = ProbeDir(std::env::temp_dir().join(unique("identity-ambiguous-log")));
        let models = probe.0.join("assets/models");
        write_file(&models.join("a.glb"), b"identical");
        write_file(&models.join("b.glb"), b"identical");
        scan(&probe.0).expect("scan");

        std::fs::rename(models.join("a.glb"), models.join("c.glb")).unwrap();
        std::fs::rename(models.join("b.glb"), models.join("d.glb")).unwrap();
        let (_, logs) = capture_warnings(|| scan(&probe.0).expect("rescan"));

        for name in ["a.glb", "b.glb", "c.glb", "d.glb"] {
            assert!(
                logs.contains(name),
                "a warning that does not name {name} leaves the developer with \
                 four files and no idea which two the scan refused to pair -- \
                 got:\n{logs}"
            );
        }
    }

    // Recovery has to be a one-time repair. An orphan left on disk would be
    // rediscovered by every later scan, and once something else moved into the
    // path it names, it would offer up an identity that already lives elsewhere.
    #[test]
    fn a_repair_deletes_the_orphan_so_no_later_scan_finds_it_again() {
        let probe = ProbeDir(std::env::temp_dir().join(unique("identity-orphan-gone")));
        let models = probe.0.join("assets/models");
        write_file(&models.join("fox.glb"), b"fake glb contents");
        let original = scan(&probe.0)
            .expect("scan")
            .guid_for_path("assets/models/fox.glb")
            .unwrap();

        std::fs::rename(models.join("fox.glb"), models.join("renamed.glb")).unwrap();
        scan(&probe.0).expect("rescan");

        assert!(
            !models.join("fox.glb.meta").exists(),
            "the orphan must be deleted once its identity has moved"
        );
        assert!(models.join("renamed.glb.meta").exists());

        let (third, logs) = capture_warnings(|| scan(&probe.0).expect("third scan"));
        assert_eq!(
            third.guid_for_path("assets/models/renamed.glb"),
            Some(original),
            "the recovered identity has to survive the scan after the repair"
        );
        assert!(
            logs.is_empty(),
            "a repaired project is an ordinary one; anything reported here is an \
             orphan the repair failed to clear -- got:\n{logs}"
        );
    }

    // The honest limit of hashing: contents are the only thread back to the lost
    // identity, so an asset that was edited as well as moved cannot be recovered.
    // What it must not do is fail quietly -- a reference that will not resolve is
    // worth a line now rather than a missing mesh later.
    #[test]
    fn an_asset_edited_as_well_as_moved_is_reported_rather_than_recovered() {
        let probe = ProbeDir(std::env::temp_dir().join(unique("identity-edited")));
        let models = probe.0.join("assets/models");
        write_file(&models.join("fox.glb"), b"fake glb contents");
        let original = scan(&probe.0)
            .expect("scan")
            .guid_for_path("assets/models/fox.glb")
            .unwrap();

        std::fs::rename(models.join("fox.glb"), models.join("renamed.glb")).unwrap();
        write_file(&models.join("renamed.glb"), b"fake glb contents, retouched");

        let (second, logs) = capture_warnings(|| scan(&probe.0).expect("rescan"));

        let now = second
            .guid_for_path("assets/models/renamed.glb")
            .expect("the asset still has to be identified, just not recovered");
        assert_ne!(
            now, original,
            "the contents no longer match, so claiming this is the same asset \
             would be a guess made on no evidence at all"
        );
        assert!(
            logs.contains("fox.glb.meta"),
            "an identity that cannot be recovered must name the sidecar holding \
             it, or the loss is silent -- got:\n{logs}"
        );
    }

    // Every hop an asset makes is a former path worth recording, and a hop back
    // to somewhere it has already been is not. Without the check, an asset moved
    // between two directories in a loop would grow this list forever.
    #[test]
    fn a_recovered_asset_records_where_it_was_without_repeating_itself() {
        let probe = ProbeDir(std::env::temp_dir().join(unique("identity-former")));
        let models = probe.0.join("assets/models");
        write_file(&models.join("fox.glb"), b"fake glb contents");
        scan(&probe.0).expect("scan");

        for (from, to) in [
            ("fox.glb", "renamed.glb"),
            ("renamed.glb", "fox.glb"),
            ("fox.glb", "renamed.glb"),
        ] {
            std::fs::rename(models.join(from), models.join(to)).unwrap();
            scan(&probe.0).expect("rescan");
        }

        let sidecar = Sidecar::read(models.join("renamed.glb.meta"))
            .expect("read")
            .expect("present");
        assert_eq!(
            sidecar.former_paths,
            ["assets/models/fox.glb", "assets/models/renamed.glb"],
            "three moves between two paths are two former paths; appending \
             unconditionally would let a file moved back and forth grow its \
             sidecar without bound"
        );
    }

    // A sidecar copied rather than moved leaves an orphan whose identity a real
    // file is still using. Re-pairing it would write a second .meta claiming one
    // GUID into the user's source tree -- the contradiction AssetIndex refuses to
    // resolve, made permanent on disk.
    #[test]
    fn an_orphan_whose_identity_is_still_in_use_is_not_handed_out_twice() {
        let probe = ProbeDir(std::env::temp_dir().join(unique("identity-inuse")));
        let models = probe.0.join("assets/models");
        write_file(&models.join("fox.glb"), b"fake glb contents");
        let original = scan(&probe.0)
            .expect("scan")
            .guid_for_path("assets/models/fox.glb")
            .unwrap();

        std::fs::copy(models.join("fox.glb.meta"), models.join("gone.glb.meta"))
            .expect("copy the sidecar rather than move it");
        write_file(&models.join("twin.glb"), b"fake glb contents");

        let (second, logs) = capture_warnings(|| scan(&probe.0).expect("rescan"));

        assert_eq!(
            second.guid_for_path("assets/models/fox.glb"),
            Some(original),
            "the file that really holds the identity must keep it"
        );
        let twin = second
            .guid_for_path("assets/models/twin.glb")
            .expect("the new file still needs an identity of its own");
        assert_ne!(twin, original, "one identity cannot mean two files");
        assert!(
            logs.contains("gone.glb.meta"),
            "the stray sidecar is the thing to delete, so the warning has to \
             name it -- got:\n{logs}"
        );
        assert!(
            logs.contains("delete this sidecar"),
            "every other refusal in this module ends by telling the user what \
             to do about the file it named; naming a file and stopping leaves a \
             warning that recurs on every startup with no way to act on it -- \
             got:\n{logs}"
        );
    }

    // An orphan holding an identity that a live asset already carries can never
    // be recovered onto anything. Left in the pool it is still counted, so it
    // turns a *different* orphan's perfectly unambiguous match into a coin flip
    // and costs an identity that was there to be recovered -- a refusal caused
    // entirely by a competitor that could never have won.
    #[test]
    fn an_orphan_whose_identity_is_already_live_does_not_block_another_recovery() {
        let probe = ProbeDir(std::env::temp_dir().join(unique("identity-blocked")));
        let models = probe.0.join("assets/models");
        // Two assets that happen to share their contents -- a texture copied
        // rather than referenced, an export made twice.
        write_file(&models.join("fox.glb"), b"fake glb contents");
        write_file(&models.join("twin.glb"), b"fake glb contents");
        let first = scan(&probe.0).expect("scan");
        let fox = first.guid_for_path("assets/models/fox.glb").unwrap();
        let twin = first.guid_for_path("assets/models/twin.glb").unwrap();

        // A sidecar copied rather than moved: `gone.glb` never existed, and the
        // identity inside this `.meta` is on `fox.glb`, which is still there.
        std::fs::copy(models.join("fox.glb.meta"), models.join("gone.glb.meta"))
            .expect("copy the sidecar rather than move it");
        // And, separately, a real move: `twin.glb` left its sidecar behind.
        std::fs::rename(models.join("twin.glb"), models.join("moved.glb")).unwrap();

        let (second, logs) = capture_warnings(|| scan(&probe.0).expect("rescan"));

        assert_eq!(
            second.guid_for_path("assets/models/moved.glb"),
            Some(twin),
            "the only orphan that could have been recovered lost its match to \
             one that never could be; a stray .meta must not cost an unrelated \
             asset its identity"
        );
        assert_eq!(
            second.guid_for_path("assets/models/fox.glb"),
            Some(fox),
            "the file that really holds the identity must keep it"
        );
        assert!(
            !models.join("twin.glb.meta").exists(),
            "the recovered orphan has to be cleared, or every later scan finds \
             it again"
        );
        assert!(
            logs.contains("gone.glb.meta"),
            "the stray sidecar is still the thing to delete, so it still has to \
             be named -- got:\n{logs}"
        );
    }

    // The case the pre-filter above cannot see, because the identity is not
    // live when it runs: two orphans hold the same GUID and record *different*
    // contents, so neither is ambiguous and both look recoverable -- until the
    // first is recovered and makes that GUID live for the second. Without the
    // check `repair` makes at the moment it hands one out, the second asset
    // would get a `.meta` claiming a GUID another file already has, be refused
    // by the index for exactly that reason, and end up with a sidecar on disk
    // and no identity in the index.
    #[test]
    fn an_identity_that_goes_live_mid_settle_is_not_handed_out_a_second_time() {
        let probe = ProbeDir(std::env::temp_dir().join(unique("identity-midsettle")));
        let models = probe.0.join("assets/models");
        write_file(&models.join("one.glb"), b"one glb");
        write_file(&models.join("two.glb"), b"two glb contents");

        // One GUID in two sidecars -- a `.meta` copied and then hand-edited, or
        // a badly resolved merge -- each recording its own asset's contents, so
        // the two orphans land in different buckets and neither pairing is a
        // coin flip.
        let guid = AssetGuid::new();
        for name in ["one.glb", "two.glb"] {
            let (hash, size) = measure_file(models.join(name)).expect("measure");
            Sidecar {
                guid,
                hash,
                size: Some(size),
                former_paths: Vec::new(),
            }
            .write(sidecar_path(models.join(name)))
            .expect("write sidecar");
        }

        // Both assets move away, so when settling begins that GUID is on no
        // file at all and both orphans look perfectly recoverable.
        std::fs::rename(models.join("one.glb"), models.join("moved_one.glb")).unwrap();
        std::fs::rename(models.join("two.glb"), models.join("moved_two.glb")).unwrap();

        let (index, logs) = capture_warnings(|| scan(&probe.0).expect("scan"));

        assert_eq!(
            index.len(),
            2,
            "both assets are real files that need an identity; the one that \
             lost the contested GUID must be given a fresh one rather than \
             left with a sidecar the index refuses"
        );
        let one = index.guid_for_path("assets/models/moved_one.glb").unwrap();
        let two = index.guid_for_path("assets/models/moved_two.glb").unwrap();
        assert_ne!(one, two, "one identity cannot mean two files");
        assert!(
            one == guid || two == guid,
            "whichever was settled first should still have recovered the \
             identity; refusing both would lose one that was recoverable"
        );
        assert!(
            logs.contains("delete this sidecar"),
            "the loser's sidecar is the thing to remove, and the message has to \
             say so -- got:\n{logs}"
        );
    }

    // An assets directory that is not there is not "a project with no assets" —
    // it is almost always a project directory that is wrong, and answering with
    // an empty index would report the first while meaning the second. The crate
    // docs describe the engine having already shipped a whole game without its
    // meshes for exactly this class of mistake.
    #[test]
    fn a_missing_assets_directory_is_an_error_not_an_empty_success() {
        let missing = PathBuf::from(unique("identity-absent"));
        let err = scan(&missing).expect_err("a missing assets directory must be reported");
        assert_eq!(
            err.kind(),
            io::ErrorKind::NotFound,
            "the error has to be distinguishable, so a caller for which an \
             assets-less project is normal can say so; got {err:?}"
        );
    }
}
