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

use super::index::{AssetIndex, Insertion};
use super::{hash_file, sidecar_path, AssetGuid, Sidecar, SIDECAR_EXTENSION};
use std::fs::ReadDir;
use std::io;
use std::path::Path;
use tracing::{debug, warn};

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
/// sidecar at all is hashed and given a fresh [`AssetGuid`].
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
/// A rescan of an unchanged project hashes **nothing**: a readable sidecar
/// answers the question by itself, so the work is one `read_dir` per directory
/// and one small `read_to_string` per asset. Hashing happens once per asset
/// ever, when its identity is first minted. The consequence is that
/// `Sidecar::hash` means "the contents when this sidecar was written", not
/// "the contents now" — anything that needs the latter has to hash the file
/// itself rather than trust the field.
pub fn scan(project_dir: impl AsRef<Path>) -> io::Result<AssetIndex> {
    let root = project_dir.as_ref().join(ASSETS_DIR);
    let entries = std::fs::read_dir(&root)?;

    let mut index = AssetIndex::default();
    walk(entries, ASSETS_DIR, &mut index);
    Ok(index)
}

/// Recurses through one already-opened directory, whose project-relative path
/// is `prefix` (`assets`, then `assets/models`, …).
///
/// Takes the opened [`ReadDir`] rather than a path so there is exactly one
/// place that decides what a failed open means: [`scan`] returns it for the
/// root and warns for everything below, and neither can drift into doing the
/// other's thing.
fn walk(entries: ReadDir, prefix: &str, index: &mut AssetIndex) {
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
                Ok(entries) => walk(entries, &relative, index),
                Err(e) => warn!(
                    "asset identity: cannot open {relative} ({e}); nothing inside \
                     it will have an identity this run"
                ),
            }
        } else if file_type.is_file() {
            identify(&entry.path(), &relative, index);
        } else {
            debug!(
                "asset identity: {relative} is a link rather than a file or a \
                 directory; the scan does not follow links, so it has no identity"
            );
        }
    }
}

/// Gives one file an identity, if it is the kind of file that gets one.
fn identify(path: &Path, relative: &str, index: &mut AssetIndex) {
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
        return;
    }
    if !IDENTIFIED_EXTENSIONS.contains(&extension.as_str()) {
        return;
    }

    let meta = sidecar_path(path);
    match Sidecar::read(&meta) {
        // The ordinary rescan: this asset was identified on some earlier run
        // and keeps what it was given.
        Ok(Some(existing)) => record(&existing, relative, index),

        // Never seen before, so mint one.
        Ok(None) => {
            let hash = match hash_file(path) {
                Ok(hash) => hash,
                Err(e) => {
                    warn!(
                        "asset identity: cannot read {relative} to hash it ({e}); \
                         it will have no identity until the next scan that can"
                    );
                    return;
                }
            };
            let sidecar = Sidecar {
                guid: AssetGuid::new(),
                hash,
                former_paths: Vec::new(),
            };
            if let Err(e) = sidecar.write(&meta) {
                // Not indexed: an identity that did not reach disk is a
                // different identity on the next run, so handing this one out
                // would be worse than admitting the asset is unidentified.
                warn!(
                    "asset identity: cannot write {} ({e}); {relative} will have \
                     no identity, and would be given a different one every run if \
                     it did",
                    meta.display()
                );
                return;
            }
            record(&sidecar, relative, index);
        }

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

/// Puts one asset's sidecar into the index, and tells a human when the index
/// will not have it.
///
/// The index refuses a contradiction rather than resolving it — see
/// [`AssetIndex`] — and a refusal that nobody reported would leave an asset
/// quietly unidentified with nothing to act on, which is the failure mode this
/// whole module is built to avoid.
fn record(sidecar: &Sidecar, relative: &str, index: &mut AssetIndex) {
    match index.insert(sidecar.guid, relative, &sidecar.hash, &sidecar.former_paths) {
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

    // A rescan reads sidecars; it must not rewrite them, and in particular must
    // not re-hash every asset in the project to do it. The stale hash below is
    // the visible consequence of that decision: `Sidecar::hash` records the
    // contents *when the sidecar was written*, and anything that needs the
    // contents *now* has to hash the file itself.
    #[test]
    fn a_rescan_neither_rewrites_the_sidecar_nor_rehashes_the_asset() {
        let probe = ProbeDir(std::env::temp_dir().join(unique("identity-cost")));
        let asset = probe.0.join("assets/models/fox.glb");
        write_file(&asset, b"fake glb");
        let meta = probe.0.join("assets/models/fox.glb.meta");

        scan(&probe.0).expect("scan");
        let after_first = read_text(&meta);

        // An edit big enough to change the hash, so a rescan that re-hashed
        // would provably write different bytes.
        write_file(&asset, b"a different fake glb");
        scan(&probe.0).expect("rescan");

        assert_eq!(
            read_text(&meta),
            after_first,
            "a rescan must leave an existing sidecar untouched; rewriting it \
             means hashing every asset in the project on every startup"
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
