//! Sweeping files nobody has used for a while out of a cache directory.
//!
//! Both caches under a project's `.bsengine_cache/` are keyed by content:
//! a thumbnail by its source's path and mtime, a mip chain by a hash of its
//! pixels. That is what makes a re-imported image land in a new file
//! instead of corrupting the old one -- and also what makes the old file
//! an orphan nothing will ever open again. Before this module existed both
//! caches said so in their own tests ("orphans are not cleaned up") and
//! simply grew with every edit.
//!
//! The reference engines' caches are all content-keyed the same way, and
//! the one with a stated policy is Unreal's derived data cache: a
//! filesystem backend with `DeleteUnused` and an `UnusedFileAge` in days,
//! swept at startup, with files touched on use so that "unused" means what
//! it says rather than "old". (Unity re-imports whatever `Library/` lacks
//! and its Accelerator evicts by size; Godot leaves `.godot/imported/` to
//! grow.) This follows Unreal: [`sweep_unused_files`] removes every file
//! directly under a cache directory whose modification time is older than
//! [`UNUSED_FILE_AGE`], each cache's owner runs it once at startup before
//! the first read, and [`touch`] refreshes a file's modification time on
//! every cache hit.
//!
//! Modification time, not access time: Windows keeps NTFS access-time
//! updates off by default and Linux mounts with `relatime`, so a file's
//! atime says nothing reliable about use on either. A touch on hit costs
//! one metadata write per file per session and is what Unreal does.

use std::path::Path;
use std::time::{Duration, SystemTime};

/// How long a cache file may go unused before a sweep removes it. Unreal's
/// shipped configurations put the local derived data cache's
/// `UnusedFileAge` between 10 and 34 days, and 5.4's delete-only legacy
/// cache at 8; two weeks sits inside that range and is long enough that a
/// project left over a holiday still opens with its thumbnails and mip
/// chains in place.
pub const UNUSED_FILE_AGE: Duration = Duration::from_secs(14 * 24 * 60 * 60);

/// What one [`sweep_unused_files`] call did, for the log line and for
/// tests that want the sweep's effect as numbers rather than as a
/// directory listing.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SweepReport {
    /// Files removed because they went unused for longer than the limit.
    pub removed: usize,
    /// Files left in place: used recently, or not removable.
    pub kept: usize,
    /// Bytes the removed files took on disk.
    pub bytes_freed: u64,
}

/// Removes every file directly under `root` whose modification time is
/// more than `max_age` before `now`. Subdirectories are left alone (neither
/// cache makes any, and a sweep that recursed would be one typo away from
/// walking a project), as are files whose time cannot be read or lies in
/// the future. A `root` that does not exist is a no-op -- the sweep never
/// creates a cache directory, so a project that never needed one does not
/// grow a `.bsengine_cache/` from being opened.
pub fn sweep_unused_files(root: &Path, max_age: Duration, now: SystemTime) -> SweepReport {
    let mut report = SweepReport::default();
    let Ok(entries) = std::fs::read_dir(root) else {
        return report;
    };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else {
            report.kept += 1;
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let unused_for = meta
            .modified()
            .ok()
            .and_then(|mtime| now.duration_since(mtime).ok());
        let stale = unused_for.is_some_and(|age| age > max_age);
        if stale && std::fs::remove_file(entry.path()).is_ok() {
            report.removed += 1;
            report.bytes_freed += meta.len();
        } else {
            report.kept += 1;
        }
    }
    if report.removed > 0 {
        tracing::info!(
            "[cache] removed {} file(s), {} bytes, unused for over {} days under {}",
            report.removed,
            report.bytes_freed,
            max_age.as_secs() / (24 * 60 * 60),
            root.display()
        );
    }
    report
}

/// Sets `path`'s modification time to now, so that the next sweep sees a
/// file that was just used. Best effort: a cache on a read-only volume
/// cannot be touched and cannot be swept either, so failing quietly loses
/// nothing.
pub fn touch(path: &Path) {
    // Opened for writing because Windows will not let a read handle change
    // the timestamps; nothing is written.
    if let Ok(file) = std::fs::OpenOptions::new().write(true).open(path) {
        let _ = file.set_modified(SystemTime::now());
    }
}

/// One day, for tests that age files.
#[cfg(test)]
pub(crate) const DAY: Duration = Duration::from_secs(24 * 60 * 60);

/// Writes `bytes` to `path` with its modification time set `age` before
/// `now` -- the fixture every sweep test in this crate builds on, so it
/// asserts its own premise: the filesystem really did take the time it was
/// given, and a test that then finds the file kept or removed is seeing the
/// sweep and not a fixture that never aged anything.
#[cfg(test)]
pub(crate) fn write_aged(path: &Path, bytes: &[u8], now: SystemTime, age: Duration) {
    std::fs::write(path, bytes).unwrap();
    set_mtime(path, now - age);
}

/// Sets `path`'s modification time to `then` and checks the filesystem kept
/// it (to within its own granularity).
#[cfg(test)]
pub(crate) fn set_mtime(path: &Path, then: SystemTime) {
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(then)
        .unwrap();
    let got = mtime(path);
    let drift = got
        .duration_since(then)
        .or_else(|_| then.duration_since(got))
        .unwrap();
    assert!(
        drift < Duration::from_secs(2),
        "premise: the filesystem must keep the mtime this fixture sets (got {got:?} for {then:?})"
    );
}

/// `path`'s modification time.
#[cfg(test)]
pub(crate) fn mtime(path: &Path) -> SystemTime {
    std::fs::metadata(path).unwrap().modified().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("bse_cache_sweep_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    /// Old files go, recent ones stay, the report says which and how much,
    /// and a subdirectory (with an old file inside) is not entered.
    #[test]
    fn a_sweep_removes_only_files_unused_for_longer_than_the_limit() {
        let dir = scratch("basic");
        let now = SystemTime::now();
        write_aged(&dir.join("old.mips"), &[7u8; 1000], now, 15 * DAY);
        write_aged(&dir.join("older.png"), &[7u8; 24], now, 400 * DAY);
        write_aged(&dir.join("recent.mips"), &[7u8; 10], now, 13 * DAY);
        write_aged(&dir.join("fresh.png"), &[7u8; 10], now, Duration::ZERO);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        write_aged(&dir.join("sub").join("old.png"), &[7u8; 10], now, 100 * DAY);

        let report = sweep_unused_files(&dir, 14 * DAY, now);

        assert_eq!(
            report,
            SweepReport {
                removed: 2,
                kept: 2,
                bytes_freed: 1024,
            }
        );
        assert_eq!(names(&dir), ["fresh.png", "recent.mips", "sub"]);
        assert!(
            dir.join("sub").join("old.png").is_file(),
            "a sweep must not descend into subdirectories"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// No directory, no sweep -- and, the half that matters, no directory
    /// afterwards either.
    #[test]
    fn a_missing_root_is_a_no_op_and_is_not_created() {
        let dir = scratch("missing");
        let root = dir.join("never_made");
        let report = sweep_unused_files(&root, 14 * DAY, SystemTime::now());
        assert_eq!(report, SweepReport::default());
        assert!(
            !root.exists(),
            "a sweep must not create the cache directory"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A touched file reads as used now, so the sweep that would have
    /// removed it keeps it.
    #[test]
    fn touching_a_file_saves_it_from_the_next_sweep() {
        let dir = scratch("touch");
        let now = SystemTime::now();
        write_aged(&dir.join("used.mips"), &[1u8; 10], now, 30 * DAY);
        write_aged(&dir.join("unused.mips"), &[1u8; 10], now, 30 * DAY);

        touch(&dir.join("used.mips"));

        let report = sweep_unused_files(&dir, 14 * DAY, now);
        assert_eq!((report.removed, report.kept), (1, 1));
        assert_eq!(names(&dir), ["used.mips"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
