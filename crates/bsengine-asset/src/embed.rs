//! An archive carried *inside* the executable: how `--mode single` writes
//! it, and how the runtime finds it in its own binary.
//!
//! # Where the precedent comes from
//!
//! Unity ships `exe + Data/`, Unreal `exe + Content/Paks/`; neither makes a
//! single file. Godot does: "Embed PCK" appends the `.pck` to the export
//! template and writes the archive's offset and a magic at the very end, and
//! the running binary reads its own tail to find it. That is what this does.
//! Godot also declines to embed on macOS, and so does this engine: a Mach-O
//! with bytes past its load commands fails code-signature validation, and on
//! Apple silicon an unsignable binary does not run. There the archive sits
//! beside the binary instead (see [`crate::cook::package`]).
//!
//! # Layout
//!
//! ```text
//! executable, exactly as built
//! archive (a `.pak`, see `crate::pak`)
//! u64   archive offset, from the start of the file   ┐
//! u64   archive length                               ├ trailer, 24 bytes
//! "BSEMBED1"                                         ┘
//! ```
//!
//! Little-endian. The trailer sits at the very end so a reader finds it with
//! one seek from the file's length, without knowing how long the executable
//! part is -- which it cannot know, since the runtime that reads this is the
//! same binary in every build and its size is whatever the compiler made it.
//!
//! # A trailer is checked, not trusted
//!
//! The magic says an archive should be here; the offsets say where. A file
//! that carries the magic but whose offsets do not add up to "the archive ends
//! exactly where the trailer begins" is a damaged build, and reads as an
//! error rather than as "no archive" -- the runtime must not start a game out
//! of loose files that happen to lie around when the player double-clicked a
//! broken single-file build.

use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

/// The last eight bytes of a single-file build. The digit is the trailer's
/// layout version.
const MAGIC: &[u8; 8] = b"BSEMBED1";

/// Offset, length, magic.
const TRAILER_LEN: usize = 8 + 8 + MAGIC.len();

/// Where an embedded archive lies in a file: its offset and length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Trailer {
    offset: u64,
    len: u64,
}

/// Reads the trailer at the end of `tail` -- the last [`TRAILER_LEN`] bytes
/// of a file of `file_len` bytes -- and checks it against that length.
///
/// `Ok(None)` when there is no magic, which is every plain executable.
///
/// # Errors
///
/// When the magic is there but the archive it describes does not end exactly
/// where the trailer begins.
fn parse_trailer(tail: &[u8], file_len: u64) -> io::Result<Option<Trailer>> {
    if tail.len() < TRAILER_LEN || &tail[tail.len() - MAGIC.len()..] != MAGIC {
        return Ok(None);
    }
    let at = tail.len() - TRAILER_LEN;
    let offset = u64::from_le_bytes(tail[at..at + 8].try_into().expect("8 bytes"));
    let len = u64::from_le_bytes(tail[at + 8..at + 16].try_into().expect("8 bytes"));
    let trailer_start = file_len - TRAILER_LEN as u64;
    if offset.checked_add(len) != Some(trailer_start) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "the embedded archive trailer says the archive spans {offset}..{} in a \
                 file whose trailer begins at {trailer_start}; the build is damaged",
                offset.saturating_add(len)
            ),
        ));
    }
    Ok(Some(Trailer { offset, len }))
}

/// The executable's own bytes: everything before an embedded archive, or all
/// of `bytes` when there is none.
///
/// # Errors
///
/// When `bytes` ends in a trailer whose offsets do not fit -- see the module
/// documentation.
pub fn strip(bytes: &[u8]) -> io::Result<&[u8]> {
    let tail_start = bytes.len().saturating_sub(TRAILER_LEN);
    match parse_trailer(&bytes[tail_start..], bytes.len() as u64)? {
        Some(trailer) => Ok(&bytes[..trailer.offset as usize]),
        None => Ok(bytes),
    }
}

/// `exe` with `archive` embedded after it, ready to write as a single-file
/// build.
///
/// An executable that already carries an archive -- `--package` run from a
/// single-file build -- has it replaced, not nested: the runtime reads the
/// *last* trailer, so a nested one would leave the old archive as dead bytes
/// inside every build made from it, growing by one game each time.
///
/// # Errors
///
/// When `exe` ends in a damaged trailer.
pub fn append(exe: &[u8], archive: &[u8]) -> io::Result<Vec<u8>> {
    let exe = strip(exe)?;
    let mut out = Vec::with_capacity(exe.len() + archive.len() + TRAILER_LEN);
    out.extend_from_slice(exe);
    out.extend_from_slice(archive);
    out.extend_from_slice(&(exe.len() as u64).to_le_bytes());
    out.extend_from_slice(&(archive.len() as u64).to_le_bytes());
    out.extend_from_slice(MAGIC);
    Ok(out)
}

/// The archive embedded in the file at `path`, or `None` when the file
/// carries no trailer -- an ordinary executable.
///
/// Reads the trailer and then the archive, and nothing else: the executable
/// part of a debug runtime is hundreds of megabytes, and a game's startup
/// should not read it twice.
///
/// # Errors
///
/// When the file cannot be read, or carries a damaged trailer.
pub fn read_embedded(path: &Path) -> io::Result<Option<Vec<u8>>> {
    let mut file = std::fs::File::open(path)?;
    let file_len = file.metadata()?.len();
    if file_len < TRAILER_LEN as u64 {
        return Ok(None);
    }
    file.seek(SeekFrom::Start(file_len - TRAILER_LEN as u64))?;
    let mut tail = [0u8; TRAILER_LEN];
    file.read_exact(&mut tail)?;
    let Some(trailer) = parse_trailer(&tail, file_len)? else {
        return Ok(None);
    };
    file.seek(SeekFrom::Start(trailer.offset))?;
    let mut archive = vec![0u8; trailer.len as usize];
    file.read_exact(&mut archive)?;
    Ok(Some(archive))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("bsengine-embed-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create");
        dir
    }

    /// The round trip, through a real file: what `--mode single` writes is
    /// what the runtime reads back, and the executable's own bytes come
    /// first and untouched.
    #[test]
    fn an_appended_archive_is_read_back_from_the_file_and_the_executable_is_intact() {
        let dir = scratch("roundtrip");
        let exe = b"MZ this is the executable";
        let archive = b"BSPK\0 pretend archive bytes";
        let single = append(exe, archive).expect("append");
        assert!(single.starts_with(exe), "the executable's bytes come first");
        assert_eq!(single.len(), exe.len() + archive.len() + TRAILER_LEN);
        let path = dir.join("game.exe");
        std::fs::write(&path, &single).expect("write");

        assert_eq!(
            read_embedded(&path).expect("read").as_deref(),
            Some(archive.as_slice()),
            "the archive comes back byte for byte"
        );
        assert_eq!(strip(&single).expect("strip"), exe.as_slice());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Every ordinary executable -- and every file shorter than a trailer.
    #[test]
    fn a_plain_executable_carries_no_archive() {
        let dir = scratch("plain");
        let plain = dir.join("plain.exe");
        std::fs::write(&plain, b"MZ nothing appended here, at all, in this file").expect("write");
        assert_eq!(read_embedded(&plain).expect("read"), None);
        assert_eq!(strip(b"MZ").expect("strip"), b"MZ".as_slice());

        let short = dir.join("short.exe");
        std::fs::write(&short, b"MZ").expect("write");
        assert_eq!(read_embedded(&short).expect("read"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The magic alone is not an archive. A trailer whose offsets do not put
    /// the archive exactly before it is a damaged build and must be an
    /// error, not `None` -- `None` would start the game out of loose files.
    #[test]
    fn a_trailer_whose_offsets_do_not_fit_is_an_error_not_an_absence() {
        let dir = scratch("damaged");
        let mut single = append(b"MZ exe", b"archive").expect("append");
        // Premise: intact, it reads.
        let path = dir.join("ok.exe");
        std::fs::write(&path, &single).expect("write");
        assert!(read_embedded(&path).expect("read").is_some());

        // Corrupt the length field: the archive now claims to end past the
        // trailer's start.
        let len_at = single.len() - TRAILER_LEN + 8;
        single[len_at..len_at + 8].copy_from_slice(&(u64::MAX / 2).to_le_bytes());
        let path = dir.join("damaged.exe");
        std::fs::write(&path, &single).expect("write");
        assert!(read_embedded(&path).is_err(), "damaged: an error");
        assert!(strip(&single).is_err());

        // A file that is just the magic, with nothing that could be a trailer
        // in front of it, is too short to hold one and reads as plain.
        assert_eq!(strip(MAGIC).expect("strip"), MAGIC.as_slice());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `--package` run from a single-file build: the old archive is replaced,
    /// not buried under the new one.
    #[test]
    fn appending_to_a_single_file_build_replaces_its_archive() {
        let exe = b"MZ exe";
        let first = append(exe, b"first archive").expect("append");
        let second = append(&first, b"second").expect("append again");
        assert_eq!(strip(&second).expect("strip"), exe.as_slice());
        assert_eq!(
            second.len(),
            exe.len() + b"second".len() + TRAILER_LEN,
            "no trace of the first archive remains"
        );
    }
}
