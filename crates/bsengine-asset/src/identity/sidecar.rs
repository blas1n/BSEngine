//! The `.meta` sidecar: an asset's identity, written beside the asset itself.
//!
//! ```ron
//! (
//!     guid: "0193a7c1-8f2e-7c44-9d61-3b5a0e7f2c19",
//!     hash: "blake3:9f2c1d…",
//!     size: Some(4096),
//!     former_paths: ["assets/models/old_fox.glb"],
//! )
//! ```
//!
//! The sidecar lives next to the asset because that is the only place it
//! survives the things artists actually do — copying a folder, moving a file in
//! Explorer, committing to git. A central database would go stale on the first
//! of those.
//!
//! # Why the `blake3:` prefix
//!
//! [`measure_file`] returns the algorithm's name alongside the digest. A future
//! change of algorithm then reads as a *different* hash rather than as a
//! matching one, so the mismatch is detectable instead of silently comparing
//! blake3 bytes against something else's and concluding the file changed — or
//! worse, that it did not.
//!
//! # Why `size` is beside the hash, and why it is an `Option`
//!
//! The hash is only worth what its freshness is worth: orphan recovery matches
//! a lost sidecar to a moved asset by contents, so a `hash` recorded when the
//! identity was minted and never refreshed would stop matching the first time
//! the asset was edited — turning recovery back into the fresh-GUID behaviour
//! this module exists to beat. Refreshing it by re-hashing every asset on every
//! scan is not an option either: that is a full read of every texture and mesh
//! in the project at every startup.
//!
//! `size` is the cheap discriminator that makes the refresh affordable. A
//! [`scan`](super::scan::scan) stats each identified asset and re-hashes only
//! when the length on disk differs from the length recorded here, so an
//! unchanged project still hashes nothing.
//!
//! It is an `Option` because sidecars written before this field existed have no
//! size at all, and `None` says exactly that: *unknown*, so re-hash once and
//! record what was found. A plain `u64` would have to spell "unknown" as `0`,
//! which is also the length of a legitimately empty asset — the one value where
//! the two states must not be confused, since collapsing them would leave every
//! empty asset re-hashed forever *and* make the migration untestable.

use super::AssetGuid;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

/// Extension appended to the asset's own file name to name its sidecar.
pub const SIDECAR_EXTENSION: &str = "meta";

/// Names the algorithm [`measure_file`] used, so a later change is detectable.
const HASH_PREFIX: &str = "blake3:";

/// Returns the sidecar path for an asset, by *appending* `.meta` to the whole
/// file name.
///
/// Appending rather than replacing the extension is deliberate: `fox.glb` and
/// `fox.png` are different assets, and replacing would give both of them
/// `fox.meta` — one identity silently overwriting the other.
pub fn sidecar_path(asset_path: impl AsRef<Path>) -> PathBuf {
    let mut name = asset_path.as_ref().as_os_str().to_os_string();
    name.push(".");
    name.push(SIDECAR_EXTENSION);
    PathBuf::from(name)
}

/// Hashes a file's contents and reports its length: the two facts a [`Sidecar`]
/// records about an asset, and the only two a scan ever needs to read it for.
///
/// Returns the `blake3:`-prefixed hex digest and the byte count.
///
/// Streams the file rather than reading it whole — a `.glb` or a 4K texture is
/// routinely hundreds of megabytes, and a scan touches every asset in the
/// project.
///
/// Both facts come from one open handle, so the length really is the length of
/// the bytes that were hashed. Statting the path separately would let an asset
/// rewritten between the two calls be recorded with a size its own hash
/// contradicts — a sidecar that then looks stale to every later scan and
/// re-hashes the asset forever.
pub fn measure_file(path: impl AsRef<Path>) -> io::Result<(String, u64)> {
    let file = std::fs::File::open(path)?;
    let size = file.metadata()?.len();
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(&file)?;
    Ok((format!("{HASH_PREFIX}{}", hasher.finalize().to_hex()), size))
}

/// The digest [`measure_file`] returns for a file with no contents at all.
///
/// Every empty file in a project hashes to this one value, which makes it the
/// least discriminating answer the function has: matching on it says only "both
/// of these are empty", and empty placeholders are an ordinary thing to have
/// several of. Orphan recovery refuses to re-pair an identity on this hash for
/// that reason.
///
/// Computed rather than written down, so it cannot disagree with
/// [`measure_file`] the day the algorithm behind the `blake3:` prefix changes.
pub fn empty_hash() -> String {
    format!("{HASH_PREFIX}{}", blake3::hash(b"").to_hex())
}

/// One asset's identity record, stored beside the asset as `<asset>.meta`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sidecar {
    /// The asset's stable identity. Minted once and never rewritten.
    pub guid: AssetGuid,
    /// `measure_file`'s digest of the asset's contents when the sidecar was
    /// last written, `blake3:`-prefixed. Lets a scan tell an edited file from
    /// an untouched one without re-reading everything downstream.
    pub hash: String,
    /// The asset's length in bytes when `hash` was computed, or `None` for a
    /// sidecar written before this field existed.
    ///
    /// This is what keeps `hash` fresh at one `stat` per asset rather than one
    /// full read — see the module docs. `None` means *unknown*, not *empty*:
    /// a scan re-hashes such an asset once and records what it finds.
    //
    // The `default` is belt and braces rather than the thing doing the work:
    // `ron` already reads an absent field of `Option` type as `None`, so
    // removing this attribute today changes nothing and no test can tell. It
    // stays because "absent is allowed" is a property of *this format*, not a
    // convenience of the crate that happens to implement it, and the day either
    // that crate tightens or the sidecar moves to another serialiser is the day
    // every existing project's sidecars would otherwise stop parsing at once.
    #[serde(default)]
    pub size: Option<u64>,
    /// Paths this asset has previously been known by, oldest first, in the
    /// engine's forward-slash project-relative form. Sub-item A's orphan
    /// recovery writes these; nothing reads them yet.
    pub former_paths: Vec<String>,
}

impl Sidecar {
    /// Records a path this asset used to live at, unless it is recorded
    /// already.
    ///
    /// The deduplication is the whole reason this is a method rather than a
    /// `push` at each call site. An asset moved back and forth between two
    /// directories — one designer reorganising a folder and changing their mind
    /// — would otherwise grow this list on every hop, forever, in a file that is
    /// committed to the project. With it, the list can only ever hold the
    /// *distinct* paths the asset has really occupied.
    ///
    /// Both things that record a move go through here: the scan's orphan
    /// recovery, for a move made while the engine was not running, and
    /// `identity::rename`, for one made while it was. Two copies of one policy
    /// is how one of them ends up not having it.
    pub fn remember_former_path(&mut self, path: &str) {
        if !self.former_paths.iter().any(|former| former == path) {
            self.former_paths.push(path.to_string());
        }
    }

    /// Renders the sidecar as the RON text that goes on disk.
    pub fn to_ron(&self) -> Result<String, SidecarError> {
        // `new_line("\n")` overrides ron's platform default, which is CRLF on
        // Windows. Sidecars are committed alongside the assets they identify,
        // and a format that changed shape with the author's OS would show every
        // file as modified the first time it was touched on the other one.
        let config = ron::ser::PrettyConfig::new()
            .new_line("\n".to_string())
            .compact_arrays(true);
        let mut text = ron::ser::to_string_pretty(self, config)?;
        text.push('\n');
        Ok(text)
    }

    /// Parses sidecar text produced by [`Sidecar::to_ron`].
    ///
    /// Every failure is an [`Err`], never a panic: a `.meta` is an ordinary
    /// file that a human can hand-edit or a crashed export can truncate, and
    /// one broken sidecar must cost its own asset's identity, not the process.
    pub fn from_ron(text: &str) -> Result<Self, SidecarError> {
        Ok(ron::from_str(text)?)
    }

    /// Reads the sidecar at `path`, distinguishing *absent* from *broken*.
    ///
    /// `Ok(None)` means there is no sidecar there yet — the ordinary case for
    /// an asset a scan has never seen, answered by minting an identity.
    /// `Err` means one exists and could not be read or understood, which is a
    /// different situation entirely: overwriting it would discard an identity
    /// that references elsewhere may still point at. Callers must not collapse
    /// the two.
    pub fn read(path: impl AsRef<Path>) -> Result<Option<Self>, SidecarError> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::from_ron(&text).map(Some),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(SidecarError::Io(e)),
        }
    }

    /// Writes the sidecar to `path`, replacing whatever was there.
    ///
    /// # Why this goes via a temporary file
    ///
    /// Writing in place truncates the existing sidecar and then fills it, so a
    /// crash, a full disk or a killed process between the two leaves a
    /// half-written `.meta` on disk. That is not merely a lost write: a sidecar
    /// that exists and will not parse is the one state
    /// [`scan`](super::scan::scan) deliberately refuses to repair, because
    /// overwriting it could discard an identity that references still point at.
    /// The asset would be stuck without an identity until a human noticed the
    /// warning and deleted the file by hand.
    ///
    /// So the text goes to a uniquely named temporary file *in the same
    /// directory* — the same directory because a rename across volumes is not
    /// atomic and, on Windows, not even permitted — and is renamed over the
    /// target, which either happens completely or not at all. A reader
    /// therefore only ever sees the old sidecar or the new one.
    pub fn write(&self, path: impl AsRef<Path>) -> Result<(), SidecarError> {
        let path = path.as_ref();
        let text = self.to_ron()?;
        let temp = temp_path(path);

        // Cleaned up on either failure: a leftover temporary is litter in the
        // user's source tree that nothing would ever come back for.
        if let Err(e) = std::fs::write(&temp, text) {
            std::fs::remove_file(&temp).ok();
            return Err(SidecarError::Io(e));
        }
        if let Err(e) = std::fs::rename(&temp, path) {
            std::fs::remove_file(&temp).ok();
            return Err(SidecarError::Io(e));
        }
        Ok(())
    }
}

/// Names the temporary file [`Sidecar::write`] renames over `path`.
///
/// Beside the target rather than in the system temp directory, so the rename
/// stays within one volume. Tagged with the process id and a counter so two
/// writers — two processes, or two threads of one — cannot pick the same
/// temporary and rename each other's half-written bytes into place.
///
/// The suffix is deliberately not one [`scan`](super::scan::scan) identifies,
/// so a temporary left behind by a killed process is ignored rather than
/// sidecared.
fn temp_path(path: &Path) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);

    let mut name = path.as_os_str().to_os_string();
    name.push(format!(
        ".tmp-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    PathBuf::from(name)
}

/// What can go wrong reading, writing or parsing a [`Sidecar`].
///
/// Split three ways because the recoveries differ: [`SidecarError::Io`] is
/// usually about the filesystem and worth retrying or reporting, while
/// [`SidecarError::Parse`] means this particular file is rubbish and the asset
/// should be treated as unidentified.
#[derive(Debug)]
pub enum SidecarError {
    /// The sidecar could not be read from or written to disk.
    Io(io::Error),
    /// The sidecar's text is not valid RON, or does not describe a `Sidecar`.
    Parse(Box<ron::error::SpannedError>),
    /// A `Sidecar` could not be rendered as RON.
    Serialize(Box<ron::Error>),
}

impl std::fmt::Display for SidecarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "sidecar i/o failed: {e}"),
            Self::Parse(e) => write!(f, "sidecar is not valid RON: {e}"),
            Self::Serialize(e) => write!(f, "sidecar could not be serialised: {e}"),
        }
    }
}

impl std::error::Error for SidecarError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Parse(e) => Some(e),
            Self::Serialize(e) => Some(e),
        }
    }
}

impl From<io::Error> for SidecarError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<ron::error::SpannedError> for SidecarError {
    fn from(e: ron::error::SpannedError) -> Self {
        Self::Parse(Box::new(e))
    }
}

impl From<ron::Error> for SidecarError {
    fn from(e: ron::Error) -> Self {
        Self::Serialize(Box::new(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A temp path no other test (or concurrent run of this one) will collide
    /// with, matching `watcher.rs`'s convention rather than adding a dev-dep.
    fn unique(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        std::env::temp_dir().join(format!(
            "bsengine-sidecar-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ))
    }

    /// A directory of this test's own, for the cases that have to assert on
    /// *everything* sitting beside a sidecar rather than on the sidecar alone.
    fn probe_dir(tag: &str) -> crate::test_support::ProbeDir {
        let dir = unique(tag);
        std::fs::create_dir_all(&dir).expect("create probe directory");
        crate::test_support::ProbeDir(dir)
    }

    /// The names in a probe directory, sorted so a failure reads the same way
    /// whatever order the filesystem yields them in.
    fn names_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .expect("read probe directory")
            .map(|entry| {
                entry
                    .expect("probe directory entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }

    fn sample() -> Sidecar {
        Sidecar {
            guid: AssetGuid::new(),
            hash: "blake3:abc123".to_string(),
            size: Some(4096),
            former_paths: vec!["assets/models/old.glb".to_string()],
        }
    }

    #[test]
    fn a_sidecar_round_trips_through_ron() {
        let original = Sidecar {
            guid: AssetGuid::new(),
            hash: "blake3:abc123".to_string(),
            size: Some(4096),
            former_paths: vec!["assets/models/old.glb".to_string()],
        };
        let text = original.to_ron().expect("serialise");
        let parsed = Sidecar::from_ron(&text).expect("parse");
        assert_eq!(parsed, original);
    }

    #[test]
    fn two_new_guids_differ() {
        assert_ne!(AssetGuid::new(), AssetGuid::new());
    }

    #[test]
    fn a_malformed_sidecar_is_an_error_not_a_panic() {
        // A hand-edited or half-written .meta must not take the engine down --
        // the scan recovers by treating the asset as unidentified.
        assert!(Sidecar::from_ron("this is not ron").is_err());
    }

    // The round-trip test above passes for any format both halves agree on,
    // including ones no human can read and sub-item B cannot embed. This one
    // pins the bytes.
    #[test]
    fn the_on_disk_shape_is_the_documented_one() {
        let text = Sidecar {
            guid: "0193a7c1-8f2e-7c44-9d61-3b5a0e7f2c19"
                .parse()
                .expect("guid"),
            hash: "blake3:9f2c1d".to_string(),
            size: Some(4096),
            former_paths: vec!["assets/models/old_fox.glb".to_string()],
        }
        .to_ron()
        .expect("serialise");

        assert_eq!(
            text,
            "(\n    guid: \"0193a7c1-8f2e-7c44-9d61-3b5a0e7f2c19\",\n    \
             hash: \"blake3:9f2c1d\",\n    \
             size: Some(4096),\n    \
             former_paths: [\"assets/models/old_fox.glb\"],\n)\n",
            "the sidecar format is documented in this module and read by humans \
             editing .meta files by hand"
        );
    }

    // The format change `size` makes. Every sidecar already committed in every
    // project predates the field, and a scan that could not read one would mint
    // a fresh identity for the whole project at once -- the single worst outcome
    // this module has, arrived at by a migration rather than by a bug.
    #[test]
    fn a_sidecar_written_before_size_existed_reads_as_size_unknown() {
        let text = "(guid: \"0193a7c1-8f2e-7c44-9d61-3b5a0e7f2c19\", \
                    hash: \"blake3:9f2c1d\", former_paths: [])";

        let parsed =
            Sidecar::from_ron(text).expect("a sidecar from before the size field must still parse");

        assert_eq!(
            parsed.size, None,
            "a missing size is unknown, and has to read as unknown so a scan \
             knows to re-hash once rather than trust a length nobody recorded"
        );
        assert_eq!(
            parsed.guid.to_string(),
            "0193a7c1-8f2e-7c44-9d61-3b5a0e7f2c19",
            "the identity is the one thing the migration must not touch"
        );
    }

    // ron writes a newtype struct parenthesised -- `guid: ("0193…")` -- unless
    // told otherwise, and the derive that does so is one attribute away from
    // being dropped by accident.
    #[test]
    fn the_guid_is_a_bare_string_not_a_parenthesised_newtype() {
        let text = sample().to_ron().expect("serialise");
        assert!(
            text.contains("guid: \""),
            "expected a bare quoted GUID, got:\n{text}"
        );
        assert!(
            !text.contains("guid: ("),
            "the serde(transparent) on AssetGuid has been lost:\n{text}"
        );
    }

    // Newline style is not cosmetic here: sidecars are committed next to their
    // assets, and ron's default is CRLF on Windows and LF everywhere else.
    #[test]
    fn the_text_uses_lf_on_every_platform() {
        let text = sample().to_ron().expect("serialise");
        assert!(!text.contains('\r'), "CR found in:\n{text:?}");
        assert!(text.ends_with('\n'), "no trailing newline in:\n{text:?}");
    }

    #[test]
    fn a_written_sidecar_reads_back_identical() {
        let path = unique("write");
        let original = sample();
        original.write(&path).expect("write");
        let read = Sidecar::read(&path).expect("read").expect("present");
        std::fs::remove_file(&path).ok();
        assert_eq!(read, original);
    }

    // ---- the write is atomic ----------------------------------------------

    // A rename across volumes is not atomic and, on Windows, not permitted at
    // all -- so a temporary in the system temp directory would turn every
    // sidecar write on a project stored off the system drive into a failure.
    #[test]
    fn the_write_temporary_sits_beside_the_sidecar_and_is_never_reused() {
        let target = PathBuf::from("assets/models/fox.glb.meta");

        let temp = temp_path(&target);

        assert_eq!(
            temp.parent(),
            target.parent(),
            "the temporary has to be on the same volume as the target, or the \
             rename that replaces it is neither atomic nor allowed; got {temp:?}"
        );
        assert_ne!(temp, target, "the temporary must not be the target itself");
        assert_ne!(
            temp_path(&target),
            temp_path(&target),
            "two writers picking one temporary would rename each other's \
             half-written bytes into place, which is the failure this whole \
             mechanism exists to prevent"
        );
    }

    #[test]
    fn a_successful_write_leaves_nothing_beside_the_sidecar() {
        let dir = probe_dir("residue");
        let path = dir.0.join("fox.glb.meta");

        sample().write(&path).expect("write");
        sample().write(&path).expect("overwrite");

        assert_eq!(
            names_in(&dir.0),
            ["fox.glb.meta"],
            "a temporary left in the assets directory is litter in the user's \
             source tree that nothing ever comes back for"
        );
    }

    // The failure path, which is the one that actually leaks: a directory where
    // the sidecar should be cannot be renamed over, so the write fails *after*
    // the temporary has already been created.
    #[test]
    fn a_write_that_cannot_complete_leaves_no_temporary_behind() {
        let dir = probe_dir("failed-write");
        let path = dir.0.join("fox.glb.meta");
        std::fs::create_dir(&path).expect("create the obstruction");

        sample()
            .write(&path)
            .expect_err("a sidecar cannot replace a directory");

        assert_eq!(
            names_in(&dir.0),
            ["fox.glb.meta"],
            "the failed write left its temporary behind"
        );
    }

    // The point of the whole mechanism, and the only test here that can tell an
    // in-place write from a renamed one. Writing in place truncates the file
    // and then fills it, so there is a window in which the sidecar on disk is
    // empty or half-written -- and a `.meta` that exists and will not parse is
    // exactly the state `scan` refuses to repair, leaving the asset without an
    // identity until a human deletes the file by hand. A crash is what lands in
    // that window in the field; a concurrent reader is how a test gets there.
    #[test]
    fn a_reader_never_catches_a_half_written_sidecar() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let dir = probe_dir("atomic");
        let path = dir.0.join("fox.glb.meta");
        // Long enough that the gap between truncating and filling is wide
        // enough for a reader to land in. A three-line sidecar has the same
        // race and a reader would seldom win it.
        let sidecar = Sidecar {
            guid: AssetGuid::new(),
            hash: "blake3:abc123".to_string(),
            size: Some(4096),
            former_paths: (0..400)
                .map(|i| format!("assets/models/generation_{i}/fox.glb"))
                .collect(),
        };
        sidecar.write(&path).expect("seed the sidecar");

        let writing = Arc::new(AtomicBool::new(true));
        let writer = std::thread::spawn({
            let (path, sidecar, writing) = (path.clone(), sidecar.clone(), writing.clone());
            move || {
                for _ in 0..1_000 {
                    // Ignored rather than asserted: Windows can refuse the
                    // rename for as long as a reader holds the target open,
                    // and whether an individual write got through is not what
                    // this test measures.
                    let _ = sidecar.write(&path);
                }
                writing.store(false, Ordering::Relaxed);
            }
        });

        let mut reads = 0u32;
        while writing.load(Ordering::Relaxed) {
            match Sidecar::read(&path) {
                Ok(_) => reads += 1,
                // The open itself being refused mid-replace is a reader seeing
                // *nothing*, which is one of the two states it is allowed to
                // see. Only parseable-or-absent is the invariant.
                Err(SidecarError::Io(_)) => {}
                Err(e) => panic!(
                    "a reader caught a sidecar mid-write ({e}); the same window \
                     a crash lands in leaves a permanently unparseable .meta, \
                     and scan deliberately will not repair one"
                ),
            }
        }
        writer.join().expect("writer thread");

        assert!(
            reads > 0,
            "the reader never completed a read, so the race was never actually run"
        );
    }

    // Task 2's scan branches on exactly this: no sidecar means mint one, a
    // broken sidecar means something is wrong and the identity must not be
    // silently replaced. Collapsing them would make the scan overwrite
    // identities it merely failed to parse.
    #[test]
    fn a_missing_sidecar_is_ok_none_while_a_broken_one_is_err() {
        let missing = unique("missing");
        assert!(
            matches!(Sidecar::read(&missing), Ok(None)),
            "an asset with no sidecar yet is the ordinary case, not an error"
        );

        let broken = unique("broken");
        std::fs::write(&broken, "this is not ron").expect("write");
        let result = Sidecar::read(&broken);
        std::fs::remove_file(&broken).ok();
        assert!(
            matches!(result, Err(SidecarError::Parse(_))),
            "a sidecar that exists but will not parse must not read as absent, \
             got: {result:?}"
        );
    }

    // A truncated write is the realistic corruption: valid RON as far as it
    // goes, missing a field. It must not deserialise into a half-built record.
    #[test]
    fn a_truncated_sidecar_is_a_parse_error() {
        let text = "(\n    guid: \"0193a7c1-8f2e-7c44-9d61-3b5a0e7f2c19\",\n";
        assert!(Sidecar::from_ron(text).is_err());
    }

    #[test]
    fn a_sidecar_whose_guid_is_not_a_guid_is_a_parse_error() {
        let text = "(guid: \"nonsense\", hash: \"blake3:a\", former_paths: [])";
        assert!(
            Sidecar::from_ron(text).is_err(),
            "a corrupt GUID must be rejected, not accepted as an identity"
        );
    }

    #[test]
    fn measure_file_prefixes_the_algorithm_and_tracks_contents() {
        let path = unique("hash");
        std::fs::write(&path, b"fox").expect("write");
        let (first, _) = measure_file(&path).expect("hash");

        std::fs::write(&path, b"fox!").expect("write");
        let (changed, _) = measure_file(&path).expect("hash");

        std::fs::write(&path, b"fox").expect("write");
        let (again, _) = measure_file(&path).expect("hash");
        std::fs::remove_file(&path).ok();

        assert!(first.starts_with(HASH_PREFIX), "{first}");
        assert_eq!(first, again, "identical contents must hash identically");
        assert_ne!(first, changed, "an edit must change the hash");
    }

    // Cross-checks the digest against a value computed by the one-shot API
    // rather than the streaming one, so a broken stream loop cannot agree with
    // itself and look correct.
    #[test]
    fn measure_file_matches_a_one_shot_hash_of_the_same_bytes() {
        let path = unique("crosscheck");
        // Larger than blake3's internal read buffer, so the streaming path
        // really does loop rather than swallowing the file in one read.
        let bytes: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&path, &bytes).expect("write");
        let (streamed, size) = measure_file(&path).expect("hash");
        std::fs::remove_file(&path).ok();

        let expected = format!("{HASH_PREFIX}{}", blake3::hash(&bytes).to_hex());
        assert_eq!(streamed, expected);
        // The size a sidecar records has to be the length of the bytes the hash
        // was taken over: a sidecar whose two fields describe different
        // revisions of the file looks stale to every later scan and re-hashes
        // the asset forever.
        assert_eq!(size, bytes.len() as u64);
    }

    #[test]
    fn measure_file_reports_a_missing_file_rather_than_hashing_nothing() {
        let err = measure_file(unique("absent")).expect_err("must fail");
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    // Orphan recovery refuses to re-pair an identity on the empty digest, so a
    // constant that had drifted from what measure_file actually returns for an
    // empty file would silently switch that refusal off -- and the guard that
    // stops two empty placeholders swapping identities would be gone with it.
    #[test]
    fn empty_hash_is_exactly_what_measure_file_answers_for_an_empty_file() {
        let path = unique("empty");
        std::fs::write(&path, b"").expect("write");
        let (hashed, size) = measure_file(&path).expect("hash");
        std::fs::remove_file(&path).ok();

        assert_eq!(hashed, empty_hash());
        assert_eq!(size, 0);
    }

    // Replacing the extension instead of appending it would give fox.glb and
    // fox.png the same sidecar, so one asset's identity would overwrite the
    // other's on the next scan.
    #[test]
    fn sidecar_path_appends_and_keeps_similar_assets_apart() {
        let glb = sidecar_path("assets/models/fox.glb");
        let png = sidecar_path("assets/models/fox.png");
        assert_eq!(glb, PathBuf::from("assets/models/fox.glb.meta"));
        assert_ne!(glb, png);
    }
}
