//! Error context wrapper for sys_traits operations.
//!
//! This module provides [`SysWithPathsInErrors`], a wrapper that adds operation and path
//! context to errors returned by sys_traits methods.
//!
//! # Example
//!
//! ```no_run
//! use sys_traits::PathsInErrorsExt;
//! # #[cfg(feature = "real")]
//! use sys_traits::impls::RealSys;
//!
//! # #[cfg(feature = "real")]
//! # fn example() -> std::io::Result<()> {
//! let sys = RealSys;
//!
//! // Without context:
//! // sys.fs_read("/path/to/file")?;
//! // Error: No such file or directory (os error 2)
//!
//! // With context:
//! sys.with_paths_in_errors().fs_read("/path/to/file")?;
//! // Error: failed to read '/path/to/file': No such file or directory (os error 2)
//! # Ok(())
//! # }
//! ```

use std::borrow::Cow;
use std::error::Error;
use std::fmt;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::BaseFsCanonicalize;
use crate::BaseFsChown;
use crate::BaseFsCloneFile;
use crate::BaseFsCopy;
use crate::BaseFsCreateDir;
use crate::BaseFsCreateJunction;
use crate::BaseFsHardLink;
use crate::BaseFsMetadata;
use crate::BaseFsOpen;
use crate::BaseFsRead;
use crate::BaseFsReadDir;
use crate::BaseFsReadLink;
use crate::BaseFsRemoveDir;
use crate::BaseFsRemoveDirAll;
use crate::BaseFsRemoveFile;
use crate::BaseFsRename;
use crate::BaseFsSetFileTimes;
use crate::BaseFsSetPermissions;
use crate::BaseFsSetSymlinkFileTimes;
use crate::BaseFsSymlinkChown;
use crate::BaseFsSymlinkDir;
use crate::BaseFsSymlinkFile;
use crate::BaseFsWrite;
use crate::CreateDirOptions;
use crate::FileType;
use crate::FsFile;
use crate::FsFileAsRaw;
use crate::FsFileIsTerminal;
use crate::FsFileLock;
use crate::FsFileLockMode;
use crate::FsFileMetadata;
use crate::FsFileSetLen;
use crate::FsFileSetPermissions;
use crate::FsFileSetTimes;
use crate::FsFileSyncAll;
use crate::FsFileSyncData;
use crate::FsFileTimes;
use crate::FsMetadata;
use crate::FsMetadataValue;
use crate::FsRead;
use crate::OpenOptions;

use crate::boxed::BoxedFsFile;
use crate::boxed::BoxedFsMetadataValue;
use crate::boxed::FsOpenBoxed;

/// An error that includes context about the operation that failed.
#[derive(Debug)]
pub struct OperationError {
  operation: &'static str,
  kind: OperationErrorKind,
  /// The underlying I/O error.
  pub err: io::Error,
}

impl OperationError {
  /// Returns the operation name (e.g., "read", "write", "copy").
  pub fn operation(&self) -> &'static str {
    self.operation
  }

  /// Returns the error context kind.
  pub fn kind(&self) -> &OperationErrorKind {
    &self.kind
  }
}

impl fmt::Display for OperationError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "failed to {}", self.operation)?;
    match &self.kind {
      OperationErrorKind::WithPath(path) => write!(f, " '{}'", path)?,
      OperationErrorKind::WithTwoPaths(from, to) => {
        write!(f, " '{}' to '{}'", from, to)?
      }
    }
    write!(f, ": {}", self.err)
  }
}

impl Error for OperationError {}

/// The kind of context associated with an operation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationErrorKind {
  /// Single path context.
  WithPath(String),
  /// Two path context (e.g., copy, rename).
  WithTwoPaths(String, String),
}

/// A wrapper that adds error context to sys_traits operations.
///
/// Use [`PathsInErrorsExt::with_paths_in_errors`] to create an instance.
#[derive(Debug)]
pub struct SysWithPathsInErrors<'a, T: ?Sized>(pub &'a T);

// These implementations of Clone and Copy are needed in order to get this
// working when `T` does not implement `Clone` or `Copy`
impl<T: ?Sized> Copy for SysWithPathsInErrors<'_, T> {}

impl<T: ?Sized> Clone for SysWithPathsInErrors<'_, T> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<'a, T: ?Sized> SysWithPathsInErrors<'a, T> {
  /// Creates a new `SysWithPathsInErrors` wrapper.
  pub fn new(inner: &'a T) -> Self {
    Self(inner)
  }

  /// Returns a reference to the inner value.
  #[allow(clippy::should_implement_trait)]
  pub fn as_ref(&self) -> &T {
    // WARNING: Do not implement deref or anything like that on this struct
    // because we do not want to accidentally have this being able to be passed
    // into functions for a trait. That would lead to the error being wrapped
    // multiple times.
    self.0
  }
}

/// Extension trait that provides the [`with_paths_in_errors`](PathsInErrorsExt::with_paths_in_errors) method.
///
/// Import this trait to use `.with_paths_in_errors()` on any type.
pub trait PathsInErrorsExt {
  /// Wraps `self` in a [`SysWithPathsInErrors`] that includes paths in error messages.
  fn with_paths_in_errors(&self) -> SysWithPathsInErrors<'_, Self> {
    SysWithPathsInErrors(self)
  }
}

impl<T: ?Sized> PathsInErrorsExt for T {}

/// A file wrapper that includes the path in error messages.
///
/// Returned by [`SysWithPathsInErrors::fs_open`].
#[derive(Debug)]
pub struct FsFileWithPathsInErrors<F> {
  file: F,
  path: PathBuf,
}

impl<F> FsFileWithPathsInErrors<F> {
  /// Creates a new file wrapper with path context.
  pub fn new(file: F, path: PathBuf) -> Self {
    Self { file, path }
  }

  /// Returns a reference to the path.
  pub fn path(&self) -> &Path {
    &self.path
  }

  /// Returns a reference to the inner file.
  pub fn inner(&self) -> &F {
    &self.file
  }

  /// Returns a mutable reference to the inner file.
  pub fn inner_mut(&mut self) -> &mut F {
    &mut self.file
  }

  /// Consumes the wrapper and returns the inner file.
  pub fn into_inner(self) -> F {
    self.file
  }

  fn wrap_err(&self, operation: &'static str, err: io::Error) -> io::Error {
    err_with_path(operation, &self.path, err)
  }
}

impl<F: io::Read> io::Read for FsFileWithPathsInErrors<F> {
  fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    self.file.read(buf).map_err(|e| self.wrap_err("read", e))
  }
}

impl<F: io::Write> io::Write for FsFileWithPathsInErrors<F> {
  fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    self.file.write(buf).map_err(|e| self.wrap_err("write", e))
  }

  fn flush(&mut self) -> io::Result<()> {
    self.file.flush().map_err(|e| self.wrap_err("flush", e))
  }
}

impl<F: io::Seek> io::Seek for FsFileWithPathsInErrors<F> {
  fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
    self.file.seek(pos).map_err(|e| self.wrap_err("seek", e))
  }
}

impl<F: FsFileIsTerminal> FsFileIsTerminal for FsFileWithPathsInErrors<F> {
  fn fs_file_is_terminal(&self) -> bool {
    self.file.fs_file_is_terminal()
  }
}

impl<F: FsFileLock> FsFileLock for FsFileWithPathsInErrors<F> {
  fn fs_file_lock(&mut self, mode: FsFileLockMode) -> io::Result<()> {
    self
      .file
      .fs_file_lock(mode)
      .map_err(|e| self.wrap_err("lock", e))
  }

  fn fs_file_try_lock(&mut self, mode: FsFileLockMode) -> io::Result<()> {
    self
      .file
      .fs_file_try_lock(mode)
      .map_err(|e| self.wrap_err("try lock", e))
  }

  fn fs_file_unlock(&mut self) -> io::Result<()> {
    self
      .file
      .fs_file_unlock()
      .map_err(|e| self.wrap_err("unlock", e))
  }
}

impl<F: FsFileMetadata> FsFileMetadata for FsFileWithPathsInErrors<F> {
  fn fs_file_metadata(&self) -> io::Result<BoxedFsMetadataValue> {
    self
      .file
      .fs_file_metadata()
      .map_err(|e| self.wrap_err("stat", e))
  }
}

impl<F: FsFileSetPermissions> FsFileSetPermissions
  for FsFileWithPathsInErrors<F>
{
  fn fs_file_set_permissions(&mut self, mode: u32) -> io::Result<()> {
    self
      .file
      .fs_file_set_permissions(mode)
      .map_err(|e| self.wrap_err("set permissions", e))
  }
}

impl<F: FsFileSetTimes> FsFileSetTimes for FsFileWithPathsInErrors<F> {
  fn fs_file_set_times(&mut self, times: FsFileTimes) -> io::Result<()> {
    self
      .file
      .fs_file_set_times(times)
      .map_err(|e| self.wrap_err("set file times", e))
  }
}

impl<F: FsFileSetLen> FsFileSetLen for FsFileWithPathsInErrors<F> {
  fn fs_file_set_len(&mut self, size: u64) -> io::Result<()> {
    self
      .file
      .fs_file_set_len(size)
      .map_err(|e| self.wrap_err("truncate", e))
  }
}

impl<F: FsFileSyncAll> FsFileSyncAll for FsFileWithPathsInErrors<F> {
  fn fs_file_sync_all(&mut self) -> io::Result<()> {
    self
      .file
      .fs_file_sync_all()
      .map_err(|e| self.wrap_err("sync", e))
  }
}

impl<F: FsFileSyncData> FsFileSyncData for FsFileWithPathsInErrors<F> {
  fn fs_file_sync_data(&mut self) -> io::Result<()> {
    self
      .file
      .fs_file_sync_data()
      .map_err(|e| self.wrap_err("sync data", e))
  }
}

impl<F: FsFileAsRaw> FsFileAsRaw for FsFileWithPathsInErrors<F> {
  #[cfg(windows)]
  fn fs_file_as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> {
    self.file.fs_file_as_raw_handle()
  }

  #[cfg(unix)]
  fn fs_file_as_raw_fd(&self) -> Option<std::os::fd::RawFd> {
    self.file.fs_file_as_raw_fd()
  }
}

impl<F: FsFile> FsFile for FsFileWithPathsInErrors<F> {}

// helper to create single-path errors wrapped in io::Error
fn err_with_path(
  operation: &'static str,
  path: &Path,
  err: io::Error,
) -> io::Error {
  io::Error::new(
    err.kind(),
    OperationError {
      operation,
      kind: OperationErrorKind::WithPath(path.to_string_lossy().into_owned()),
      err,
    },
  )
}

// helper to create two-path errors wrapped in io::Error
fn err_with_two_paths(
  operation: &'static str,
  from: &Path,
  to: &Path,
  err: io::Error,
) -> io::Error {
  io::Error::new(
    err.kind(),
    OperationError {
      operation,
      kind: OperationErrorKind::WithTwoPaths(
        from.to_string_lossy().into_owned(),
        to.to_string_lossy().into_owned(),
      ),
      err,
    },
  )
}

// == FsCanonicalize ==

impl<T: BaseFsCanonicalize> SysWithPathsInErrors<'_, T> {
  pub fn fs_canonicalize(&self, path: impl AsRef<Path>) -> io::Result<PathBuf> {
    let path = path.as_ref();
    self
      .0
      .base_fs_canonicalize(path)
      .map_err(|e| err_with_path("canonicalize", path, e))
  }
}

// == FsChown ==

impl<T: BaseFsChown> SysWithPathsInErrors<'_, T> {
  pub fn fs_chown(
    &self,
    path: impl AsRef<Path>,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> io::Result<()> {
    let path = path.as_ref();
    self
      .0
      .base_fs_chown(path, uid, gid)
      .map_err(|e| err_with_path("chown", path, e))
  }
}

// == FsSymlinkChown ==

impl<T: BaseFsSymlinkChown> SysWithPathsInErrors<'_, T> {
  pub fn fs_symlink_chown(
    &self,
    path: impl AsRef<Path>,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> io::Result<()> {
    let path = path.as_ref();
    self
      .0
      .base_fs_symlink_chown(path, uid, gid)
      .map_err(|e| err_with_path("chown symlink", path, e))
  }
}

// == FsCloneFile ==

impl<T: BaseFsCloneFile> SysWithPathsInErrors<'_, T> {
  pub fn fs_clone_file(
    &self,
    from: impl AsRef<Path>,
    to: impl AsRef<Path>,
  ) -> io::Result<()> {
    let from = from.as_ref();
    let to = to.as_ref();
    self
      .0
      .base_fs_clone_file(from, to)
      .map_err(|e| err_with_two_paths("clone", from, to, e))
  }
}

// == FsCopy ==

impl<T: BaseFsCopy> SysWithPathsInErrors<'_, T> {
  pub fn fs_copy(
    &self,
    from: impl AsRef<Path>,
    to: impl AsRef<Path>,
  ) -> io::Result<u64> {
    let from = from.as_ref();
    let to = to.as_ref();
    self
      .0
      .base_fs_copy(from, to)
      .map_err(|e| err_with_two_paths("copy", from, to, e))
  }
}

// == FsCreateDir ==

impl<T: BaseFsCreateDir> SysWithPathsInErrors<'_, T> {
  pub fn fs_create_dir(
    &self,
    path: impl AsRef<Path>,
    options: &CreateDirOptions,
  ) -> io::Result<()> {
    let path = path.as_ref();
    self
      .0
      .base_fs_create_dir(path, options)
      .map_err(|e| err_with_path("create directory", path, e))
  }

  pub fn fs_create_dir_all(&self, path: impl AsRef<Path>) -> io::Result<()> {
    let path = path.as_ref();
    self
      .0
      .base_fs_create_dir(
        path,
        &CreateDirOptions {
          recursive: true,
          mode: None,
        },
      )
      .map_err(|e| err_with_path("create directory", path, e))
  }
}

// == FsHardLink ==

impl<T: BaseFsHardLink> SysWithPathsInErrors<'_, T> {
  pub fn fs_hard_link(
    &self,
    src: impl AsRef<Path>,
    dst: impl AsRef<Path>,
  ) -> io::Result<()> {
    let src = src.as_ref();
    let dst = dst.as_ref();
    self
      .0
      .base_fs_hard_link(src, dst)
      .map_err(|e| err_with_two_paths("hard link", src, dst, e))
  }
}

// == FsCreateJunction ==

impl<T: BaseFsCreateJunction> SysWithPathsInErrors<'_, T> {
  pub fn fs_create_junction(
    &self,
    original: impl AsRef<Path>,
    junction: impl AsRef<Path>,
  ) -> io::Result<()> {
    let original = original.as_ref();
    let junction = junction.as_ref();
    self
      .0
      .base_fs_create_junction(original, junction)
      .map_err(|e| err_with_two_paths("create junction", original, junction, e))
  }
}

// == FsMetadata ==

impl<T: BaseFsMetadata> SysWithPathsInErrors<'_, T> {
  pub fn fs_metadata(&self, path: impl AsRef<Path>) -> io::Result<T::Metadata> {
    let path = path.as_ref();
    self
      .0
      .base_fs_metadata(path)
      .map_err(|e| err_with_path("stat", path, e))
  }

  pub fn fs_symlink_metadata(
    &self,
    path: impl AsRef<Path>,
  ) -> io::Result<T::Metadata> {
    let path = path.as_ref();
    self
      .0
      .base_fs_symlink_metadata(path)
      .map_err(|e| err_with_path("lstat", path, e))
  }

  pub fn fs_is_file(&self, path: impl AsRef<Path>) -> io::Result<bool> {
    Ok(self.fs_metadata(path)?.file_type() == FileType::File)
  }

  pub fn fs_is_dir(&self, path: impl AsRef<Path>) -> io::Result<bool> {
    Ok(self.fs_metadata(path)?.file_type() == FileType::Dir)
  }

  pub fn fs_is_symlink(&self, path: impl AsRef<Path>) -> io::Result<bool> {
    Ok(self.fs_symlink_metadata(path)?.file_type() == FileType::Symlink)
  }

  pub fn fs_exists(&self, path: impl AsRef<Path>) -> io::Result<bool> {
    let path = path.as_ref();
    match self.0.base_fs_exists(path) {
      Ok(exists) => Ok(exists),
      Err(e) => Err(err_with_path("stat", path, e)),
    }
  }

  pub fn fs_exists_no_err(&self, path: impl AsRef<Path>) -> bool {
    self.0.base_fs_exists_no_err(path.as_ref())
  }

  pub fn fs_is_file_no_err(&self, path: impl AsRef<Path>) -> bool {
    self.0.fs_is_file_no_err(path)
  }

  pub fn fs_is_dir_no_err(&self, path: impl AsRef<Path>) -> bool {
    self.0.fs_is_dir_no_err(path)
  }

  pub fn fs_is_symlink_no_err(&self, path: impl AsRef<Path>) -> bool {
    self.0.fs_is_symlink_no_err(path)
  }
}

// == FsOpen ==

impl<T: BaseFsOpen> SysWithPathsInErrors<'_, T> {
  pub fn fs_open(
    &self,
    path: impl AsRef<Path>,
    options: &OpenOptions,
  ) -> io::Result<FsFileWithPathsInErrors<T::File>> {
    let path = path.as_ref();
    let file = self
      .0
      .base_fs_open(path, options)
      .map_err(|e| err_with_path("open", path, e))?;
    Ok(FsFileWithPathsInErrors::new(file, path.to_path_buf()))
  }
}

// == FsOpenBoxed ==

impl<T: FsOpenBoxed + ?Sized> SysWithPathsInErrors<'_, T> {
  pub fn fs_open_boxed(
    &self,
    path: impl AsRef<Path>,
    options: &OpenOptions,
  ) -> io::Result<FsFileWithPathsInErrors<BoxedFsFile>> {
    let path = path.as_ref();
    let file = self
      .0
      .fs_open_boxed(path, options)
      .map_err(|e| err_with_path("open", path, e))?;
    Ok(FsFileWithPathsInErrors::new(file, path.to_path_buf()))
  }
}

// == FsRead ==

impl<T: BaseFsRead> SysWithPathsInErrors<'_, T> {
  pub fn fs_read(
    &self,
    path: impl AsRef<Path>,
  ) -> io::Result<Cow<'static, [u8]>> {
    let path = path.as_ref();
    self
      .0
      .base_fs_read(path)
      .map_err(|e| err_with_path("read", path, e))
  }

  pub fn fs_read_to_string(
    &self,
    path: impl AsRef<Path>,
  ) -> io::Result<Cow<'static, str>> {
    let path = path.as_ref();
    self
      .0
      .fs_read_to_string(path)
      .map_err(|e| err_with_path("read", path, e))
  }

  pub fn fs_read_to_string_lossy(
    &self,
    path: impl AsRef<Path>,
  ) -> io::Result<Cow<'static, str>> {
    let path = path.as_ref();
    self
      .0
      .fs_read_to_string_lossy(path)
      .map_err(|e| err_with_path("read", path, e))
  }
}

// == FsReadDir ==

impl<T: BaseFsReadDir> SysWithPathsInErrors<'_, T> {
  pub fn fs_read_dir(
    &self,
    path: impl AsRef<Path>,
  ) -> io::Result<Box<dyn Iterator<Item = io::Result<T::ReadDirEntry>>>> {
    let path = path.as_ref();
    self
      .0
      .base_fs_read_dir(path)
      .map_err(|e| err_with_path("read directory", path, e))
  }
}

// == FsReadLink ==

impl<T: BaseFsReadLink> SysWithPathsInErrors<'_, T> {
  pub fn fs_read_link(&self, path: impl AsRef<Path>) -> io::Result<PathBuf> {
    let path = path.as_ref();
    self
      .0
      .base_fs_read_link(path)
      .map_err(|e| err_with_path("read link", path, e))
  }
}

// == FsRemoveDir ==

impl<T: BaseFsRemoveDir> SysWithPathsInErrors<'_, T> {
  pub fn fs_remove_dir(&self, path: impl AsRef<Path>) -> io::Result<()> {
    let path = path.as_ref();
    self
      .0
      .base_fs_remove_dir(path)
      .map_err(|e| err_with_path("remove directory", path, e))
  }
}

// == FsRemoveDirAll ==

impl<T: BaseFsRemoveDirAll> SysWithPathsInErrors<'_, T> {
  pub fn fs_remove_dir_all(&self, path: impl AsRef<Path>) -> io::Result<()> {
    let path = path.as_ref();
    self
      .0
      .base_fs_remove_dir_all(path)
      .map_err(|e| err_with_path("remove directory", path, e))
  }
}

// == FsRemoveFile ==

impl<T: BaseFsRemoveFile> SysWithPathsInErrors<'_, T> {
  pub fn fs_remove_file(&self, path: impl AsRef<Path>) -> io::Result<()> {
    let path = path.as_ref();
    self
      .0
      .base_fs_remove_file(path)
      .map_err(|e| err_with_path("remove", path, e))
  }
}

// == FsRename ==

impl<T: BaseFsRename> SysWithPathsInErrors<'_, T> {
  pub fn fs_rename(
    &self,
    from: impl AsRef<Path>,
    to: impl AsRef<Path>,
  ) -> io::Result<()> {
    let from = from.as_ref();
    let to = to.as_ref();
    self
      .0
      .base_fs_rename(from, to)
      .map_err(|e| err_with_two_paths("rename", from, to, e))
  }
}

// == FsSetFileTimes ==

impl<T: BaseFsSetFileTimes> SysWithPathsInErrors<'_, T> {
  pub fn fs_set_file_times(
    &self,
    path: impl AsRef<Path>,
    atime: SystemTime,
    mtime: SystemTime,
  ) -> io::Result<()> {
    let path = path.as_ref();
    self
      .0
      .base_fs_set_file_times(path, atime, mtime)
      .map_err(|e| err_with_path("set file times", path, e))
  }
}

// == FsSetSymlinkFileTimes ==

impl<T: BaseFsSetSymlinkFileTimes> SysWithPathsInErrors<'_, T> {
  pub fn fs_set_symlink_file_times(
    &self,
    path: impl AsRef<Path>,
    atime: SystemTime,
    mtime: SystemTime,
  ) -> io::Result<()> {
    let path = path.as_ref();
    self
      .0
      .base_fs_set_symlink_file_times(path, atime, mtime)
      .map_err(|e| err_with_path("set symlink file times", path, e))
  }
}

// == FsSetPermissions ==

impl<T: BaseFsSetPermissions> SysWithPathsInErrors<'_, T> {
  pub fn fs_set_permissions(
    &self,
    path: impl AsRef<Path>,
    mode: u32,
  ) -> io::Result<()> {
    let path = path.as_ref();
    self
      .0
      .base_fs_set_permissions(path, mode)
      .map_err(|e| err_with_path("set permissions", path, e))
  }
}

// == FsSymlinkDir ==

impl<T: BaseFsSymlinkDir> SysWithPathsInErrors<'_, T> {
  pub fn fs_symlink_dir(
    &self,
    original: impl AsRef<Path>,
    link: impl AsRef<Path>,
  ) -> io::Result<()> {
    let original = original.as_ref();
    let link = link.as_ref();
    self
      .0
      .base_fs_symlink_dir(original, link)
      .map_err(|e| err_with_two_paths("symlink directory", original, link, e))
  }
}

// == FsSymlinkFile ==

impl<T: BaseFsSymlinkFile> SysWithPathsInErrors<'_, T> {
  pub fn fs_symlink_file(
    &self,
    original: impl AsRef<Path>,
    link: impl AsRef<Path>,
  ) -> io::Result<()> {
    let original = original.as_ref();
    let link = link.as_ref();
    self
      .0
      .base_fs_symlink_file(original, link)
      .map_err(|e| err_with_two_paths("symlink", original, link, e))
  }
}

// == FsWrite ==

impl<T: BaseFsWrite> SysWithPathsInErrors<'_, T> {
  pub fn fs_write(
    &self,
    path: impl AsRef<Path>,
    data: impl AsRef<[u8]>,
  ) -> io::Result<()> {
    let path = path.as_ref();
    self
      .0
      .base_fs_write(path, data.as_ref())
      .map_err(|e| err_with_path("write", path, e))
  }
}

#[cfg(all(test, feature = "memory"))]
mod tests {
  use super::*;
  use crate::impls::InMemorySys;
  use crate::FsCreateDir;
  use crate::FsMetadata;
  use crate::FsRead;
  use crate::FsWrite;
  use std::io::Read;
  use std::io::Write;

  #[test]
  fn test_error_display_single_path() {
    let sys = InMemorySys::default();
    let err = sys
      .with_paths_in_errors()
      .fs_read("/nonexistent")
      .unwrap_err();
    let inner = err.get_ref().unwrap();
    let op_err = inner.downcast_ref::<OperationError>().unwrap();
    assert_eq!(op_err.operation(), "read");
    assert_eq!(
      op_err.kind(),
      &OperationErrorKind::WithPath("/nonexistent".to_string())
    );
    assert_eq!(
      err.to_string(),
      format!("failed to read '/nonexistent': {}", op_err.err)
    );
  }

  #[test]
  fn test_error_display_two_paths() {
    let sys = InMemorySys::default();
    let err = sys
      .with_paths_in_errors()
      .fs_copy("/src", "/dst")
      .unwrap_err();
    let inner = err.get_ref().unwrap();
    let op_err = inner.downcast_ref::<OperationError>().unwrap();
    assert_eq!(op_err.operation(), "copy");
    assert_eq!(
      op_err.kind(),
      &OperationErrorKind::WithTwoPaths("/src".to_string(), "/dst".to_string())
    );
    assert_eq!(
      err.to_string(),
      format!("failed to copy '/src' to '/dst': {}", op_err.err)
    );
  }

  #[test]
  fn test_error_preserves_kind() {
    let sys = InMemorySys::default();
    let err = sys
      .with_paths_in_errors()
      .fs_read("/nonexistent")
      .unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::NotFound);
  }

  #[test]
  fn test_error_downcast_to_operation_error() {
    let sys = InMemorySys::default();
    let err = sys
      .with_paths_in_errors()
      .fs_read("/nonexistent")
      .unwrap_err();
    let inner = err.get_ref().unwrap();
    let op_err = inner.downcast_ref::<OperationError>().unwrap();
    assert_eq!(op_err.operation(), "read");
    assert_eq!(
      op_err.kind(),
      &OperationErrorKind::WithPath("/nonexistent".to_string())
    );
  }

  #[test]
  fn test_fs_read_success() {
    let sys = InMemorySys::new_with_cwd("/");
    sys.fs_write("/test.txt", b"hello").unwrap();
    let data = sys.with_paths_in_errors().fs_read("/test.txt").unwrap();
    assert_eq!(&*data, b"hello");
  }

  #[test]
  fn test_fs_read_to_string_success() {
    let sys = InMemorySys::new_with_cwd("/");
    sys.fs_write("/test.txt", b"hello").unwrap();
    let data = sys
      .with_paths_in_errors()
      .fs_read_to_string("/test.txt")
      .unwrap();
    assert_eq!(&*data, "hello");
  }

  #[test]
  fn test_fs_read_to_string_lossy_success() {
    let sys = InMemorySys::new_with_cwd("/");
    sys.fs_write("/test.txt", b"hello").unwrap();
    let data = sys
      .with_paths_in_errors()
      .fs_read_to_string_lossy("/test.txt")
      .unwrap();
    assert_eq!(&*data, "hello");
  }

  #[test]
  fn test_fs_write_success() {
    let sys = InMemorySys::new_with_cwd("/");
    sys
      .with_paths_in_errors()
      .fs_write("/test.txt", b"hello")
      .unwrap();
    let data = sys.fs_read("/test.txt").unwrap();
    assert_eq!(&*data, b"hello");
  }

  #[test]
  fn test_fs_write_error() {
    let sys = InMemorySys::default();
    // writing to a path in a non-existent directory should fail
    let err = sys
      .with_paths_in_errors()
      .fs_write("/nonexistent/test.txt", b"hello")
      .unwrap_err();
    let inner = err.get_ref().unwrap();
    let op_err = inner.downcast_ref::<OperationError>().unwrap();
    assert_eq!(op_err.operation(), "write");
    assert_eq!(
      op_err.kind(),
      &OperationErrorKind::WithPath("/nonexistent/test.txt".to_string())
    );
  }

  #[test]
  fn test_fs_create_dir() {
    let sys = InMemorySys::default();
    sys
      .with_paths_in_errors()
      .fs_create_dir("/newdir", &CreateDirOptions::default())
      .unwrap();
    assert!(sys.fs_is_dir("/newdir").unwrap());
  }

  #[test]
  fn test_fs_create_dir_all() {
    let sys = InMemorySys::default();
    sys
      .with_paths_in_errors()
      .fs_create_dir_all("/a/b/c")
      .unwrap();
    assert!(sys.fs_is_dir("/a/b/c").unwrap());
  }

  #[test]
  fn test_fs_remove_file() {
    let sys = InMemorySys::new_with_cwd("/");
    sys.fs_write("/test.txt", b"hello").unwrap();
    sys
      .with_paths_in_errors()
      .fs_remove_file("/test.txt")
      .unwrap();
    assert!(!sys.fs_exists("/test.txt").unwrap());
  }

  #[test]
  fn test_fs_remove_file_error() {
    let sys = InMemorySys::default();
    let err = sys
      .with_paths_in_errors()
      .fs_remove_file("/nonexistent")
      .unwrap_err();
    let inner = err.get_ref().unwrap();
    let op_err = inner.downcast_ref::<OperationError>().unwrap();
    assert_eq!(op_err.operation(), "remove");
    assert_eq!(
      op_err.kind(),
      &OperationErrorKind::WithPath("/nonexistent".to_string())
    );
  }

  #[test]
  fn test_fs_remove_dir() {
    let sys = InMemorySys::default();
    sys
      .fs_create_dir("/testdir", &CreateDirOptions::default())
      .unwrap();
    sys
      .with_paths_in_errors()
      .fs_remove_dir("/testdir")
      .unwrap();
    assert!(!sys.fs_exists("/testdir").unwrap());
  }

  #[test]
  fn test_fs_rename() {
    let sys = InMemorySys::new_with_cwd("/");
    sys.fs_write("/old.txt", b"hello").unwrap();
    sys
      .with_paths_in_errors()
      .fs_rename("/old.txt", "/new.txt")
      .unwrap();
    assert!(!sys.fs_exists("/old.txt").unwrap());
    assert!(sys.fs_exists("/new.txt").unwrap());
  }

  #[test]
  fn test_fs_rename_error() {
    let sys = InMemorySys::default();
    let err = sys
      .with_paths_in_errors()
      .fs_rename("/nonexistent", "/new.txt")
      .unwrap_err();
    let inner = err.get_ref().unwrap();
    let op_err = inner.downcast_ref::<OperationError>().unwrap();
    assert_eq!(op_err.operation(), "rename");
    assert_eq!(
      op_err.kind(),
      &OperationErrorKind::WithTwoPaths(
        "/nonexistent".to_string(),
        "/new.txt".to_string()
      )
    );
  }

  #[test]
  fn test_fs_copy() {
    let sys = InMemorySys::new_with_cwd("/");
    sys.fs_write("/src.txt", b"hello").unwrap();
    let bytes = sys
      .with_paths_in_errors()
      .fs_copy("/src.txt", "/dst.txt")
      .unwrap();
    assert_eq!(bytes, 5);
    assert_eq!(&*sys.fs_read("/dst.txt").unwrap(), b"hello");
  }

  #[test]
  fn test_fs_metadata() {
    let sys = InMemorySys::new_with_cwd("/");
    sys.fs_write("/test.txt", b"hello").unwrap();
    let meta = sys.with_paths_in_errors().fs_metadata("/test.txt").unwrap();
    assert_eq!(meta.file_type(), FileType::File);
    assert_eq!(meta.len(), 5);
  }

  #[test]
  fn test_fs_metadata_error() {
    let sys = InMemorySys::default();
    let err = sys
      .with_paths_in_errors()
      .fs_metadata("/nonexistent")
      .unwrap_err();
    let inner = err.get_ref().unwrap();
    let op_err = inner.downcast_ref::<OperationError>().unwrap();
    assert_eq!(op_err.operation(), "stat");
    assert_eq!(
      op_err.kind(),
      &OperationErrorKind::WithPath("/nonexistent".to_string())
    );
  }

  #[test]
  fn test_fs_is_file() {
    let sys = InMemorySys::new_with_cwd("/");
    sys.fs_write("/test.txt", b"hello").unwrap();
    assert!(sys.with_paths_in_errors().fs_is_file("/test.txt").unwrap());
    sys
      .fs_create_dir("/testdir", &CreateDirOptions::default())
      .unwrap();
    assert!(!sys.with_paths_in_errors().fs_is_file("/testdir").unwrap());
  }

  #[test]
  fn test_fs_is_dir() {
    let sys = InMemorySys::new_with_cwd("/");
    sys
      .fs_create_dir("/testdir", &CreateDirOptions::default())
      .unwrap();
    assert!(sys.with_paths_in_errors().fs_is_dir("/testdir").unwrap());
    sys.fs_write("/test.txt", b"hello").unwrap();
    assert!(!sys.with_paths_in_errors().fs_is_dir("/test.txt").unwrap());
  }

  #[test]
  fn test_fs_read_dir() {
    let sys = InMemorySys::new_with_cwd("/");
    sys
      .fs_create_dir("/testdir", &CreateDirOptions::default())
      .unwrap();
    sys.fs_write("/testdir/a.txt", b"a").unwrap();
    sys.fs_write("/testdir/b.txt", b"b").unwrap();
    let entries: Vec<_> = sys
      .with_paths_in_errors()
      .fs_read_dir("/testdir")
      .unwrap()
      .collect::<Result<_, _>>()
      .unwrap();
    assert_eq!(entries.len(), 2);
  }

  #[test]
  fn test_fs_read_dir_error() {
    let sys = InMemorySys::default();
    let result = sys.with_paths_in_errors().fs_read_dir("/nonexistent");
    let err = match result {
      Ok(_) => panic!("expected error"),
      Err(e) => e,
    };
    let inner = err.get_ref().unwrap();
    let op_err = inner.downcast_ref::<OperationError>().unwrap();
    assert_eq!(op_err.operation(), "read directory");
    assert_eq!(
      op_err.kind(),
      &OperationErrorKind::WithPath("/nonexistent".to_string())
    );
  }

  #[test]
  fn test_fs_hard_link() {
    let sys = InMemorySys::new_with_cwd("/");
    sys.fs_write("/original.txt", b"hello").unwrap();
    sys
      .with_paths_in_errors()
      .fs_hard_link("/original.txt", "/link.txt")
      .unwrap();
    assert_eq!(&*sys.fs_read("/link.txt").unwrap(), b"hello");
  }

  #[test]
  fn test_fs_hard_link_error() {
    let sys = InMemorySys::default();
    let err = sys
      .with_paths_in_errors()
      .fs_hard_link("/nonexistent", "/link.txt")
      .unwrap_err();
    let inner = err.get_ref().unwrap();
    let op_err = inner.downcast_ref::<OperationError>().unwrap();
    assert_eq!(op_err.operation(), "hard link");
    assert_eq!(
      op_err.kind(),
      &OperationErrorKind::WithTwoPaths(
        "/nonexistent".to_string(),
        "/link.txt".to_string()
      )
    );
  }

  #[test]
  fn test_fs_open_error() {
    let sys = InMemorySys::default();
    let err = sys
      .with_paths_in_errors()
      .fs_open("/nonexistent", &OpenOptions::default())
      .unwrap_err();
    let inner = err.get_ref().unwrap();
    let op_err = inner.downcast_ref::<OperationError>().unwrap();
    assert_eq!(op_err.operation(), "open");
    assert_eq!(
      op_err.kind(),
      &OperationErrorKind::WithPath("/nonexistent".to_string())
    );
  }

  #[test]
  fn test_fs_open_success() {
    let sys = InMemorySys::new_with_cwd("/");
    sys.fs_write("/test.txt", b"hello").unwrap();
    let mut file = sys
      .with_paths_in_errors()
      .fs_open(
        "/test.txt",
        &OpenOptions {
          read: true,
          ..Default::default()
        },
      )
      .unwrap();
    let mut buf = [0u8; 5];
    file.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"hello");
  }

  #[test]
  fn test_fs_file_read_write_success() {
    let sys = InMemorySys::new_with_cwd("/");
    // create and write via wrapped file
    let mut file = sys
      .with_paths_in_errors()
      .fs_open(
        "/test.txt",
        &OpenOptions {
          write: true,
          create: true,
          ..Default::default()
        },
      )
      .unwrap();
    file.write_all(b"hello").unwrap();
    drop(file);

    // read via wrapped file
    let mut file = sys
      .with_paths_in_errors()
      .fs_open(
        "/test.txt",
        &OpenOptions {
          read: true,
          ..Default::default()
        },
      )
      .unwrap();
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).unwrap();
    assert_eq!(&buf, b"hello");
  }

  #[test]
  fn test_fs_file_path_accessor() {
    let sys = InMemorySys::new_with_cwd("/");
    sys.fs_write("/test.txt", b"hello").unwrap();
    let file = sys
      .with_paths_in_errors()
      .fs_open(
        "/test.txt",
        &OpenOptions {
          read: true,
          ..Default::default()
        },
      )
      .unwrap();
    assert_eq!(file.path(), Path::new("/test.txt"));
  }

  #[test]
  fn test_fs_exists() {
    let sys = InMemorySys::new_with_cwd("/");
    sys.fs_write("/test.txt", b"hello").unwrap();
    assert!(sys.with_paths_in_errors().fs_exists("/test.txt").unwrap());
    assert!(!sys
      .with_paths_in_errors()
      .fs_exists("/nonexistent")
      .unwrap());
  }

  #[test]
  fn test_fs_exists_no_err() {
    let sys = InMemorySys::new_with_cwd("/");
    sys.fs_write("/test.txt", b"hello").unwrap();
    assert!(sys.with_paths_in_errors().fs_exists_no_err("/test.txt"));
    assert!(!sys.with_paths_in_errors().fs_exists_no_err("/nonexistent"));
  }

  #[test]
  fn test_fs_is_file_no_err() {
    let sys = InMemorySys::new_with_cwd("/");
    sys
      .fs_create_dir("/dir", &CreateDirOptions::default())
      .unwrap();
    sys.fs_write("/dir/file.txt", b"hello").unwrap();
    assert!(sys
      .with_paths_in_errors()
      .fs_is_file_no_err("/dir/file.txt"));
    assert!(!sys.with_paths_in_errors().fs_is_file_no_err("/dir"));
    assert!(!sys.with_paths_in_errors().fs_is_file_no_err("/nonexistent"));
  }

  #[test]
  fn test_fs_is_dir_no_err() {
    let sys = InMemorySys::new_with_cwd("/");
    sys
      .fs_create_dir("/dir", &CreateDirOptions::default())
      .unwrap();
    sys.fs_write("/dir/file.txt", b"hello").unwrap();
    assert!(sys.with_paths_in_errors().fs_is_dir_no_err("/dir"));
    assert!(!sys.with_paths_in_errors().fs_is_dir_no_err("/dir/file.txt"));
    assert!(!sys.with_paths_in_errors().fs_is_dir_no_err("/nonexistent"));
  }
}
