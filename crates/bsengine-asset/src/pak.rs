//! The `.pak` container: one file holding a packaged build's assets.
//!
//! # Why a custom container rather than zip
//!
//! Every shipping engine surveyed uses one — Unreal `.pak`, Unity's
//! AssetBundle, Godot `.pck` — and the reason is random access. A runtime reads
//! one asset at a known offset, so any compression has to be per-entry for that
//! to survive; Unity's advice to prefer chunked LZ4 over whole-archive LZMA at
//! runtime is the same constraint stated from the other side. Starting
//! uncompressed keeps offset reads trivially correct and leaves per-entry
//! compression as an additive change to the entry record rather than a rewrite.
//!
//! id Tech's `.pk3` is a real zip and a real precedent, and zip would buy
//! inspectability with an ordinary tool. Declined for a new dependency tree and
//! for the control over layout a custom index keeps — with the cost taken
//! knowingly, since a `.pak` cannot be opened with 7-zip. The loose build mode
//! is the answer when somebody needs to see the files.
//!
//! # Layout
//!
//! ```text
//! "BSPK\0"                      magic
//! u32                           format version
//! u32                           entry count
//! entry × count {
//!     u32   path length
//!     bytes path (UTF-8, '/'-separated, project-relative)
//!     u64   offset into the blob
//!     u64   length
//! }
//! blob                          contents, back to back
//! ```
//!
//! All integers little-endian. Offsets are relative to the start of the blob
//! rather than of the file, so the index can grow without rewriting every
//! offset in it.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

/// Identifies the format, and its absence identifies a file that is not one.
pub(crate) const MAGIC: &[u8; 5] = b"BSPK\0";

/// The format this build writes and can read.
const VERSION: u32 = 1;

/// One opened archive: the index, and the bytes it points into.
///
/// The whole file is held in memory. A packaged game's assets are the same
/// bytes it would otherwise have on disk and are read once at startup, which is
/// what makes [`Self::get`] a slice rather than a seek — but it does mean this
/// is not the shape for an archive larger than memory. Recorded as a limit
/// rather than hidden: the projects this ships for are megabytes.
pub struct Pak {
    /// Path to (offset, length) within [`Self::blob`].
    index: BTreeMap<String, (u64, u64)>,
    blob: Vec<u8>,
}

/// Written by hand rather than derived: a derived impl would print the whole
/// blob — every byte of every asset — into any message that formats a `Pak`,
/// including a test failure. The shape is what a reader actually wants to know.
impl std::fmt::Debug for Pak {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pak")
            .field("entries", &self.index.len())
            .field("bytes", &self.blob.len())
            .finish()
    }
}

impl Pak {
    /// Reads an archive from disk.
    ///
    /// # Errors
    ///
    /// When the file cannot be read, or is not a readable archive of a version
    /// this build understands.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        Self::from_bytes(std::fs::read(path)?)
    }

    /// Parses an archive already in memory.
    ///
    /// # Errors
    ///
    /// When the bytes are not an archive, are a version this build does not
    /// understand, or carry an entry pointing outside the blob.
    pub fn from_bytes(bytes: Vec<u8>) -> io::Result<Self> {
        let invalid = |what: String| io::Error::new(io::ErrorKind::InvalidData, what);
        let mut at = 0usize;

        let take = |at: &mut usize, n: usize| -> io::Result<&[u8]> {
            let end = at
                .checked_add(n)
                .ok_or_else(|| invalid("archive is truncated".to_string()))?;
            let slice = bytes
                .get(*at..end)
                .ok_or_else(|| invalid("archive is truncated".to_string()))?;
            *at = end;
            Ok(slice)
        };

        if take(&mut at, MAGIC.len())? != MAGIC.as_slice() {
            return Err(invalid("not a BSPK archive".to_string()));
        }
        let version = u32::from_le_bytes(take(&mut at, 4)?.try_into().expect("4 bytes"));
        if version != VERSION {
            return Err(invalid(format!(
                "archive format version {version}, but this build reads {VERSION}"
            )));
        }
        let count = u32::from_le_bytes(take(&mut at, 4)?.try_into().expect("4 bytes"));

        let mut index = BTreeMap::new();
        for _ in 0..count {
            let path_len = u32::from_le_bytes(take(&mut at, 4)?.try_into().expect("4 bytes"));
            let path = std::str::from_utf8(take(&mut at, path_len as usize)?)
                .map_err(|_| invalid("an entry path is not UTF-8".to_string()))?
                .to_string();
            let offset = u64::from_le_bytes(take(&mut at, 8)?.try_into().expect("8 bytes"));
            let length = u64::from_le_bytes(take(&mut at, 8)?.try_into().expect("8 bytes"));
            index.insert(path, (offset, length));
        }

        let blob = bytes[at..].to_vec();
        // Checked once here rather than on every `get`, so a corrupt archive
        // fails at open with a message naming the entry instead of returning
        // `None` forever and reading as "that asset was never packed".
        for (path, (offset, length)) in &index {
            let end = offset
                .checked_add(*length)
                .ok_or_else(|| invalid(format!("{path}'s length overflows")))?;
            if end > blob.len() as u64 {
                return Err(invalid(format!("{path} points past the end of the archive")));
            }
        }

        Ok(Self { index, blob })
    }

    /// The bytes stored for `path`, or `None` if the archive has no such entry.
    pub fn get(&self, path: &str) -> Option<&[u8]> {
        let (offset, length) = self.index.get(path)?;
        // Bounds were checked once at open, so this cannot be out of range.
        Some(&self.blob[*offset as usize..(*offset + *length) as usize])
    }

    /// Every path in the archive.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.index.keys().map(String::as_str)
    }

    /// Whether any entry lives under `prefix` — which is how a directory exists
    /// at all in a container that stores no directory records.
    pub fn has_prefix(&self, prefix: &str) -> bool {
        let with_slash = format!("{}/", prefix.trim_end_matches('/'));
        self.index.keys().any(|path| path.starts_with(&with_slash))
    }
}

/// Serialises `entries` into archive bytes.
///
/// # Errors
///
/// When a path is longer than `u32::MAX` bytes, which no real asset path is.
pub(crate) fn write_pak_bytes(entries: &[(String, Vec<u8>)]) -> io::Result<Vec<u8>> {
    let mut index = Vec::new();
    let mut blob = Vec::new();
    for (path, contents) in entries {
        let path_len = u32::try_from(path.len()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, format!("{path} is too long"))
        })?;
        index.extend_from_slice(&path_len.to_le_bytes());
        index.extend_from_slice(path.as_bytes());
        index.extend_from_slice(&(blob.len() as u64).to_le_bytes());
        index.extend_from_slice(&(contents.len() as u64).to_le_bytes());
        blob.extend_from_slice(contents);
    }

    let mut out = Vec::with_capacity(MAGIC.len() + 8 + index.len() + blob.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    out.extend_from_slice(&index);
    out.extend_from_slice(&blob);
    Ok(out)
}

/// Writes an archive of `entries` to `path`.
///
/// # Errors
///
/// When serialisation fails, or the file cannot be written.
pub fn write_pak(path: impl AsRef<Path>, entries: &[(String, Vec<u8>)]) -> io::Result<()> {
    std::fs::write(path, write_pak_bytes(entries)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file whose bytes *are* the magic is the case a naive reader gets
    /// wrong, so it lives in the shared fixture rather than in its own test.
    fn fixture() -> Vec<(String, Vec<u8>)> {
        vec![
            ("assets/a.txt".to_string(), b"first".to_vec()),
            ("assets/nested/b.bin".to_string(), MAGIC.to_vec()),
            ("assets/c.txt".to_string(), b"third".to_vec()),
        ]
    }

    /// Paired on purpose: a reader returning the whole blob for every path
    /// would satisfy a single-entry round-trip, and one returning the first
    /// entry for every path would satisfy "some bytes came back".
    #[test]
    fn every_entry_reads_back_byte_identical() {
        let bytes = write_pak_bytes(&fixture()).expect("write");
        let pak = Pak::from_bytes(bytes).expect("open");

        for (path, expected) in fixture() {
            assert_eq!(
                pak.get(&path).map(<[u8]>::to_vec),
                Some(expected),
                "{path} did not read back byte-identical"
            );
        }
    }

    /// Distinct contents are what make the test above meaningful; this pins
    /// that the fixture actually has them, so it cannot rot into three
    /// identical entries that agree no matter what `get` returns.
    #[test]
    fn the_fixture_entries_are_distinguishable() {
        let all: Vec<Vec<u8>> = fixture().into_iter().map(|(_, bytes)| bytes).collect();
        let mut unique = all.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), all.len(), "fixture entries must differ");
    }

    #[test]
    fn a_path_not_in_the_archive_is_absent_not_empty() {
        let pak = Pak::from_bytes(write_pak_bytes(&fixture()).expect("write")).expect("open");
        assert_eq!(
            pak.get("assets/missing.txt"),
            None,
            "an absent path must be None, never empty bytes -- empty bytes read \
             as a valid zero-length asset"
        );
    }

    #[test]
    fn a_file_that_is_not_a_pak_is_rejected() {
        assert!(Pak::from_bytes(b"not an archive".to_vec()).is_err());
    }

    #[test]
    fn an_entry_pointing_past_the_end_is_rejected_at_open() {
        let mut bytes = write_pak_bytes(&fixture()).expect("write");
        // The first entry's length field: magic(5) + version(4) + count(4) +
        // path_len(4) + path("assets/a.txt" = 12) + offset(8).
        let length_at = 5 + 4 + 4 + 4 + 12 + 8;
        bytes[length_at..length_at + 8].copy_from_slice(&u64::MAX.to_le_bytes());

        let error = Pak::from_bytes(bytes).expect_err("a corrupt entry must be refused");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn the_archive_lists_exactly_what_was_written() {
        let pak = Pak::from_bytes(write_pak_bytes(&fixture()).expect("write")).expect("open");
        let mut listed: Vec<&str> = pak.paths().collect();
        listed.sort_unstable();
        assert_eq!(
            listed,
            vec!["assets/a.txt", "assets/c.txt", "assets/nested/b.bin"]
        );
    }

    #[test]
    fn a_directory_exists_when_something_lives_under_it() {
        let pak = Pak::from_bytes(write_pak_bytes(&fixture()).expect("write")).expect("open");
        assert!(pak.has_prefix("assets"));
        assert!(pak.has_prefix("assets/nested"));
        assert!(
            !pak.has_prefix("assets/a.txt"),
            "a file is not a directory, even though its path is a prefix of nothing"
        );
        assert!(!pak.has_prefix("elsewhere"));
    }
}
