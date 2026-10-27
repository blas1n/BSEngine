// Copyright 2018-2025 the Deno authors. MIT license.

use std::io::Error;
use std::io::ErrorKind;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use sys_traits::FsCanonicalize;
use sys_traits::FsCreateDirAll;
use sys_traits::FsMetadata;
use sys_traits::FsOpen;
use sys_traits::FsRemoveFile;
use sys_traits::FsRename;
use sys_traits::OpenOptions;
use sys_traits::SystemRandom;
use sys_traits::ThreadSleep;

use crate::get_atomic_path;

/// Canonicalizes a path which might be non-existent by going up the
/// ancestors until it finds a directory that exists, canonicalizes
/// that path, then adds back the remaining path components.
///
/// Note: When using this, you should be aware that a symlink may
/// subsequently be created along this path by some other code.
pub fn canonicalize_path_maybe_not_exists(
  sys: &impl FsCanonicalize,
  mut path: &Path,
) -> std::io::Result<PathBuf> {
  let mut names_stack = Vec::new();
  loop {
    match sys.fs_canonicalize(path) {
      Ok(mut canonicalized_path) => {
        for name in names_stack.into_iter().rev() {
          canonicalized_path = canonicalized_path.join(name);
        }
        return Ok(canonicalized_path);
      }
      Err(err) if err.kind() == ErrorKind::NotFound => {
        names_stack.push(match path.file_name() {
          Some(name) => name.to_owned(),
          None => return Err(err),
        });
        path = match path.parent() {
          // When the provided path is a relative path (e.g. `foo/bar.txt`),
          // `path.parent()` ends up being the empty string as documented in
          // `std::path::Path::parent()` after going up the ancestor path.
          // In this case, what this function should return is a path joining
          // the current path with the provided path i.e. `{cwd}/foo/bar.txt`,
          // so we set `path` to `.` to indicate the current directory.
          Some(parent) if parent.as_os_str().is_empty() => Path::new("."),
          Some(parent) => parent,
          None => return Err(err),
        };
      }
      Err(err) => return Err(err),
    }
  }
}

#[sys_traits::auto_impl]
pub trait AtomicWriteFileWithRetriesSys:
  AtomicWriteFileSys + ThreadSleep
{
}

pub fn atomic_write_file_with_retries<TSys: AtomicWriteFileWithRetriesSys>(
  sys: &TSys,
  file_path: &Path,
  data: &[u8],
  mode: u32,
) -> std::io::Result<()> {
  let mut count = 0;
  loop {
    match atomic_write_file(sys, file_path, data, mode) {
      Ok(()) => return Ok(()),
      Err(err) => {
        if count >= 5 {
          // too many retries, return the error
          return Err(err);
        }
        count += 1;
        let sleep_ms = std::cmp::min(50, 10 * count);
        sys.thread_sleep(std::time::Duration::from_millis(sleep_ms));
      }
    }
  }
}

#[sys_traits::auto_impl]
pub trait AtomicWriteFileSys:
  FsCreateDirAll + FsMetadata + FsOpen + FsRemoveFile + FsRename + SystemRandom
{
}

/// Writes the file to the file system at a temporary path, then
/// renames it to the destination in a single sys call in order
/// to never leave the file system in a corrupted state.
///
/// This also handles creating the directory if a NotFound error
/// occurs.
pub fn atomic_write_file<TSys: AtomicWriteFileSys>(
  sys: &TSys,
  file_path: &Path,
  data: &[u8],
  mode: u32,
) -> std::io::Result<()> {
  fn atomic_write_file_raw<TSys: AtomicWriteFileSys>(
    sys: &TSys,
    temp_file_path: &Path,
    file_path: &Path,
    data: &[u8],
    mode: u32,
  ) -> std::io::Result<()> {
    let mut options = OpenOptions::new_write();
    options.mode = Some(mode);
    let mut file = sys.fs_open(temp_file_path, &options)?;
    file.write_all(data)?;
    sys
      .fs_rename(temp_file_path, file_path)
      .inspect_err(|_err| {
        // clean up the created temp file on error
        let _ = sys.fs_remove_file(temp_file_path);
      })
  }

  let temp_file_path = get_atomic_path(sys, file_path);

  if let Err(write_err) =
    atomic_write_file_raw(sys, &temp_file_path, file_path, data, mode)
  {
    if write_err.kind() == ErrorKind::NotFound {
      let parent_dir_path = file_path.parent().unwrap();
      match sys.fs_create_dir_all(parent_dir_path) {
        Ok(()) => {
          return atomic_write_file_raw(
            sys,
            &temp_file_path,
            file_path,
            data,
            mode,
          )
          .map_err(|err| add_file_context_to_err(file_path, err));
        }
        Err(create_err) => {
          if !sys.fs_exists(parent_dir_path).unwrap_or(false) {
            return Err(Error::new(
              create_err.kind(),
              format!(
                "{:#} (for '{}')\nCheck the permission of the directory.",
                create_err,
                parent_dir_path.display()
              ),
            ));
          }
        }
      }
    }
    return Err(add_file_context_to_err(file_path, write_err));
  }
  Ok(())
}

fn add_file_context_to_err(file_path: &Path, err: Error) -> Error {
  Error::new(
    err.kind(),
    format!("{:#} (for '{}')", err, file_path.display()),
  )
}

#[cfg(test)]
mod test {
  use std::path::Path;
  use std::path::PathBuf;

  use sys_traits::EnvCurrentDir;
  use sys_traits::EnvSetCurrentDir;
  use sys_traits::FsCanonicalize;
  use sys_traits::FsCreateDirAll;
  use sys_traits::FsRead;
  use sys_traits::FsSymlinkDir;
  use sys_traits::impls::InMemorySys;
  use sys_traits::impls::RealSys;

  use super::atomic_write_file_with_retries;
  use super::canonicalize_path_maybe_not_exists;

  #[test]
  fn test_canonicalize_path_maybe_not_exists_in_memory() {
    let sys = InMemorySys::default();

    // .
    // └── a
    //     └── b (cwd)
    //         └── c
    sys.fs_create_dir_all("/a/b/c").unwrap();
    sys.env_set_current_dir("/a/b").unwrap();

    let path = canonicalize_path_maybe_not_exists(&sys, Path::new("")).unwrap();
    assert_eq!(path, PathBuf::from("/a/b"));
    let path =
      canonicalize_path_maybe_not_exists(&sys, Path::new(".")).unwrap();
    assert_eq!(path, PathBuf::from("/a/b"));
    let path =
      canonicalize_path_maybe_not_exists(&sys, Path::new("d")).unwrap();
    assert_eq!(path, PathBuf::from("/a/b/d"));
    let path =
      canonicalize_path_maybe_not_exists(&sys, Path::new("./d")).unwrap();
    assert_eq!(path, PathBuf::from("/a/b/d"));
    let path =
      canonicalize_path_maybe_not_exists(&sys, Path::new("c")).unwrap();
    assert_eq!(path, PathBuf::from("/a/b/c"));
    let path =
      canonicalize_path_maybe_not_exists(&sys, Path::new("./c")).unwrap();
    assert_eq!(path, PathBuf::from("/a/b/c"));
    let path =
      canonicalize_path_maybe_not_exists(&sys, Path::new("c/d/e")).unwrap();
    assert_eq!(path, PathBuf::from("/a/b/c/d/e"));
    let path =
      canonicalize_path_maybe_not_exists(&sys, Path::new("./c/d/e")).unwrap();
    assert_eq!(path, PathBuf::from("/a/b/c/d/e"));
  }

  // Note: this test mutates the current working directory. Although it will be
  // restored at the end, if other tests relying on the cwd are run in parallel
  // to this test, they may fail.
  #[test]
  fn test_canonicalize_path_maybe_not_exists_real() {
    let sys = RealSys;
    let temp_dir = tempfile::tempdir().unwrap();

    // Save the original working directory to restore it later
    let original_cwd = sys.env_current_dir().unwrap();

    // .
    // ├── a
    // │   └── b
    // │       └── c
    // └── link -> a/b/c (cwd)
    sys
      .fs_create_dir_all(temp_dir.path().join("a/b/c"))
      .unwrap();
    sys
      .fs_symlink_dir(
        temp_dir.path().join("a/b/c"),
        temp_dir.path().join("link"),
      )
      .unwrap();
    let cwd = temp_dir.path().join("link");
    sys.env_set_current_dir(&cwd).unwrap();

    let canonicalized_temp_dir_path =
      sys.fs_canonicalize(temp_dir.path()).unwrap();

    let path =
      canonicalize_path_maybe_not_exists(&sys, Path::new(".")).unwrap();
    assert_eq!(path, canonicalized_temp_dir_path.join("a/b/c"));

    let path =
      canonicalize_path_maybe_not_exists(&sys, &PathBuf::from("d")).unwrap();
    assert_eq!(path, canonicalized_temp_dir_path.join("a/b/c/d"));

    let path =
      canonicalize_path_maybe_not_exists(&sys, Path::new("./d")).unwrap();
    assert_eq!(path, canonicalized_temp_dir_path.join("a/b/c/d"));

    let path =
      canonicalize_path_maybe_not_exists(&sys, Path::new("d/e")).unwrap();
    assert_eq!(path, canonicalized_temp_dir_path.join("a/b/c/d/e"));

    let path =
      canonicalize_path_maybe_not_exists(&sys, Path::new("./d/e")).unwrap();
    assert_eq!(path, canonicalized_temp_dir_path.join("a/b/c/d/e"));

    // Restore the original working directory
    sys.env_set_current_dir(&original_cwd).unwrap();
  }

  #[test]
  fn test_atomic_write_file() {
    let sys = RealSys;
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("a/b/c");
    atomic_write_file_with_retries(&sys, &path, b"data", 0o644).unwrap();
    assert_eq!(sys.fs_read_to_string(&path).unwrap(), "data");
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      let file = std::fs::metadata(path).unwrap();
      assert_eq!(file.permissions().mode(), 0o100644);
    }
  }
}
