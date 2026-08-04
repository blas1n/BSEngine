//! The `.meta` sidecar: an asset's identity, written beside the asset itself.
//!
//! ```ron
//! (
//!     guid: "0193a7c1-8f2e-7c44-9d61-3b5a0e7f2c19",
//!     hash: "blake3:9f2c1d…",
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
//! [`hash_file`] returns the algorithm's name alongside the digest. A future
//! change of algorithm then reads as a *different* hash rather than as a
//! matching one, so the mismatch is detectable instead of silently comparing
//! blake3 bytes against something else's and concluding the file changed — or
//! worse, that it did not.

use super::AssetGuid;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

/// Extension appended to the asset's own file name to name its sidecar.
pub const SIDECAR_EXTENSION: &str = "meta";

/// Names the algorithm [`hash_file`] used, so a later change is detectable.
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

/// Hashes a file's contents, returning the `blake3:`-prefixed hex digest.
///
/// Streams the file rather than reading it whole — a `.glb` or a 4K texture is
/// routinely hundreds of megabytes, and a scan touches every asset in the
/// project.
pub fn hash_file(path: impl AsRef<Path>) -> io::Result<String> {
    let file = std::fs::File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(file)?;
    Ok(format!("{HASH_PREFIX}{}", hasher.finalize().to_hex()))
}

/// One asset's identity record, stored beside the asset as `<asset>.meta`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sidecar {
    /// The asset's stable identity. Minted once and never rewritten.
    pub guid: AssetGuid,
    /// `hash_file`'s digest of the asset's contents when the sidecar was last
    /// written, `blake3:`-prefixed. Lets a scan tell an edited file from an
    /// untouched one without re-reading everything downstream.
    pub hash: String,
    /// Paths this asset has previously been known by, oldest first, in the
    /// engine's forward-slash project-relative form. Sub-item A's orphan
    /// recovery writes these; nothing reads them yet.
    pub former_paths: Vec<String>,
}

impl Sidecar {
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
    pub fn write(&self, path: impl AsRef<Path>) -> Result<(), SidecarError> {
        std::fs::write(path, self.to_ron()?)?;
        Ok(())
    }
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

    fn sample() -> Sidecar {
        Sidecar {
            guid: AssetGuid::new(),
            hash: "blake3:abc123".to_string(),
            former_paths: vec!["assets/models/old.glb".to_string()],
        }
    }

    #[test]
    fn a_sidecar_round_trips_through_ron() {
        let original = Sidecar {
            guid: AssetGuid::new(),
            hash: "blake3:abc123".to_string(),
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
            former_paths: vec!["assets/models/old_fox.glb".to_string()],
        }
        .to_ron()
        .expect("serialise");

        assert_eq!(
            text,
            "(\n    guid: \"0193a7c1-8f2e-7c44-9d61-3b5a0e7f2c19\",\n    \
             hash: \"blake3:9f2c1d\",\n    \
             former_paths: [\"assets/models/old_fox.glb\"],\n)\n",
            "the sidecar format is documented in this module and read by humans \
             editing .meta files by hand"
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
    fn hash_file_prefixes_the_algorithm_and_tracks_contents() {
        let path = unique("hash");
        std::fs::write(&path, b"fox").expect("write");
        let first = hash_file(&path).expect("hash");

        std::fs::write(&path, b"fox!").expect("write");
        let changed = hash_file(&path).expect("hash");

        std::fs::write(&path, b"fox").expect("write");
        let again = hash_file(&path).expect("hash");
        std::fs::remove_file(&path).ok();

        assert!(first.starts_with(HASH_PREFIX), "{first}");
        assert_eq!(first, again, "identical contents must hash identically");
        assert_ne!(first, changed, "an edit must change the hash");
    }

    // Cross-checks the digest against a value computed by the one-shot API
    // rather than the streaming one, so a broken stream loop cannot agree with
    // itself and look correct.
    #[test]
    fn hash_file_matches_a_one_shot_hash_of_the_same_bytes() {
        let path = unique("crosscheck");
        // Larger than blake3's internal read buffer, so the streaming path
        // really does loop rather than swallowing the file in one read.
        let bytes: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&path, &bytes).expect("write");
        let streamed = hash_file(&path).expect("hash");
        std::fs::remove_file(&path).ok();

        let expected = format!("{HASH_PREFIX}{}", blake3::hash(&bytes).to_hex());
        assert_eq!(streamed, expected);
    }

    #[test]
    fn hash_file_reports_a_missing_file_rather_than_hashing_nothing() {
        let err = hash_file(unique("absent")).expect_err("must fail");
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
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
