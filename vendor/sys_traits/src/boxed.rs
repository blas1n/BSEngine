use std::borrow::Cow;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::time::SystemTime;

use crate::BaseFsMetadata;
use crate::BaseFsOpen;
use crate::BaseFsReadDir;
use crate::FileType;
use crate::FsDirEntry;
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
use crate::FsMetadataValue;
use crate::OpenOptions;

// == FsOpenBoxed ==

pub struct BoxedFsFile(pub Box<dyn FsFile + 'static>);

impl io::Read for BoxedFsFile {
  #[inline]
  fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    self.0.read(buf)
  }
}

impl io::Seek for BoxedFsFile {
  #[inline]
  fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
    self.0.seek(pos)
  }
}

impl io::Write for BoxedFsFile {
  #[inline]
  fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    self.0.write(buf)
  }

  #[inline]
  fn flush(&mut self) -> io::Result<()> {
    self.0.flush()
  }
}

impl FsFileAsRaw for BoxedFsFile {
  #[cfg(windows)]
  #[inline]
  fn fs_file_as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> {
    self.0.fs_file_as_raw_handle()
  }

  /// Returns the raw file descriptor on Unix platforms only
  /// or `None` when the file doesn't support it (ex. in-memory file system).
  #[cfg(unix)]
  #[inline]
  fn fs_file_as_raw_fd(&self) -> Option<std::os::fd::RawFd> {
    self.0.fs_file_as_raw_fd()
  }
}

impl FsFileIsTerminal for BoxedFsFile {
  #[inline]
  fn fs_file_is_terminal(&self) -> bool {
    self.0.fs_file_is_terminal()
  }
}

impl FsFileLock for BoxedFsFile {
  #[inline]
  fn fs_file_lock(&mut self, mode: FsFileLockMode) -> io::Result<()> {
    self.0.fs_file_lock(mode)
  }
  #[inline]
  fn fs_file_try_lock(&mut self, mode: FsFileLockMode) -> io::Result<()> {
    self.0.fs_file_try_lock(mode)
  }
  #[inline]
  fn fs_file_unlock(&mut self) -> io::Result<()> {
    self.0.fs_file_unlock()
  }
}

impl FsFileMetadata for BoxedFsFile {
  #[inline]
  fn fs_file_metadata(&self) -> io::Result<BoxedFsMetadataValue> {
    self.0.fs_file_metadata()
  }
}

impl FsFileSetLen for BoxedFsFile {
  #[inline]
  fn fs_file_set_len(&mut self, size: u64) -> io::Result<()> {
    self.0.fs_file_set_len(size)
  }
}

impl FsFileSetPermissions for BoxedFsFile {
  #[inline]
  fn fs_file_set_permissions(&mut self, perm: u32) -> io::Result<()> {
    self.0.fs_file_set_permissions(perm)
  }
}

impl FsFileSetTimes for BoxedFsFile {
  #[inline]
  fn fs_file_set_times(&mut self, times: FsFileTimes) -> io::Result<()> {
    self.0.fs_file_set_times(times)
  }
}

impl FsFileSyncAll for BoxedFsFile {
  #[inline]
  fn fs_file_sync_all(&mut self) -> io::Result<()> {
    self.0.fs_file_sync_all()
  }
}

impl FsFileSyncData for BoxedFsFile {
  #[inline]
  fn fs_file_sync_data(&mut self) -> io::Result<()> {
    self.0.fs_file_sync_data()
  }
}

impl FsFile for BoxedFsFile {}

pub trait FsOpenBoxed {
  fn fs_open_boxed(
    &self,
    path: &Path,
    open_options: &OpenOptions,
  ) -> io::Result<BoxedFsFile>;
}

impl<TFile: FsFile + 'static, T: BaseFsOpen<File = TFile>> FsOpenBoxed for T {
  fn fs_open_boxed(
    &self,
    path: &Path,
    open_options: &OpenOptions,
  ) -> io::Result<BoxedFsFile> {
    self
      .base_fs_open(path, open_options)
      .map(|file| BoxedFsFile(Box::new(file)))
  }
}

// == FsMetadataBoxed ==

#[derive(Debug)]
pub struct BoxedFsMetadataValue(pub Box<dyn FsMetadataValue>);

impl BoxedFsMetadataValue {
  pub fn new<T: FsMetadataValue + 'static>(metadata: T) -> Self {
    Self(Box::new(metadata))
  }
}

impl FsMetadataValue for BoxedFsMetadataValue {
  #[inline]
  fn file_type(&self) -> FileType {
    self.0.file_type()
  }

  #[inline]
  fn len(&self) -> u64 {
    self.0.len()
  }

  #[inline]
  fn accessed(&self) -> io::Result<SystemTime> {
    self.0.accessed()
  }

  #[inline]
  fn changed(&self) -> io::Result<SystemTime> {
    self.0.changed()
  }

  #[inline]
  fn created(&self) -> io::Result<SystemTime> {
    self.0.created()
  }

  #[inline]
  fn modified(&self) -> io::Result<SystemTime> {
    self.0.modified()
  }

  #[inline]
  fn dev(&self) -> io::Result<u64> {
    self.0.dev()
  }

  #[inline]
  fn ino(&self) -> io::Result<u64> {
    self.0.ino()
  }

  #[inline]
  fn mode(&self) -> io::Result<u32> {
    self.0.mode()
  }

  #[inline]
  fn nlink(&self) -> io::Result<u64> {
    self.0.nlink()
  }

  #[inline]
  fn uid(&self) -> io::Result<u32> {
    self.0.uid()
  }

  #[inline]
  fn gid(&self) -> io::Result<u32> {
    self.0.gid()
  }

  #[inline]
  fn rdev(&self) -> io::Result<u64> {
    self.0.rdev()
  }

  #[inline]
  fn blksize(&self) -> io::Result<u64> {
    self.0.blksize()
  }

  #[inline]
  fn blocks(&self) -> io::Result<u64> {
    self.0.blocks()
  }

  #[inline]
  fn is_block_device(&self) -> io::Result<bool> {
    self.0.is_block_device()
  }

  #[inline]
  fn is_char_device(&self) -> io::Result<bool> {
    self.0.is_char_device()
  }

  #[inline]
  fn is_fifo(&self) -> io::Result<bool> {
    self.0.is_fifo()
  }

  #[inline]
  fn is_socket(&self) -> io::Result<bool> {
    self.0.is_socket()
  }

  #[inline]
  fn file_attributes(&self) -> io::Result<u32> {
    self.0.file_attributes()
  }
}

pub trait FsMetadataBoxed {
  fn fs_metadata_boxed(&self, path: &Path) -> io::Result<BoxedFsMetadataValue>;
  fn fs_symlink_metadata_boxed(
    &self,
    path: &Path,
  ) -> io::Result<BoxedFsMetadataValue>;
}

impl<T: BaseFsMetadata + 'static> FsMetadataBoxed for T {
  fn fs_metadata_boxed(&self, path: &Path) -> io::Result<BoxedFsMetadataValue> {
    let metadata = self.base_fs_metadata(path)?;
    Ok(BoxedFsMetadataValue(Box::new(metadata)))
  }

  fn fs_symlink_metadata_boxed(
    &self,
    path: &Path,
  ) -> io::Result<BoxedFsMetadataValue> {
    let metadata = self.base_fs_symlink_metadata(path)?;
    Ok(BoxedFsMetadataValue(Box::new(metadata)))
  }
}

// == FsReadDirBoxed ==

#[derive(Debug)]
struct MappedMetadataFsDirEntry<T: FsDirEntry + 'static>(T);

impl<T: FsDirEntry + 'static> FsDirEntry for MappedMetadataFsDirEntry<T> {
  type Metadata = BoxedFsMetadataValue;

  #[inline]
  fn file_name(&self) -> Cow<'_, OsStr> {
    self.0.file_name()
  }

  #[inline]
  fn file_type(&self) -> io::Result<FileType> {
    self.0.file_type()
  }

  #[inline]
  fn metadata(&self) -> io::Result<Self::Metadata> {
    self
      .0
      .metadata()
      .map(|metadata| BoxedFsMetadataValue(Box::new(metadata)))
  }

  #[inline]
  fn path(&self) -> Cow<'_, Path> {
    self.0.path()
  }
}

#[derive(Debug)]
pub struct BoxedFsDirEntry(
  pub Box<dyn FsDirEntry<Metadata = BoxedFsMetadataValue>>,
);

impl BoxedFsDirEntry {
  pub fn new<T: FsDirEntry + 'static>(entry: T) -> Self {
    Self(Box::new(MappedMetadataFsDirEntry(entry)))
  }
}

impl FsDirEntry for BoxedFsDirEntry {
  type Metadata = BoxedFsMetadataValue;

  #[inline]
  fn file_name(&self) -> Cow<'_, OsStr> {
    self.0.file_name()
  }

  #[inline]
  fn file_type(&self) -> io::Result<FileType> {
    self.0.file_type()
  }

  #[inline]
  fn metadata(&self) -> io::Result<Self::Metadata> {
    self.0.metadata()
  }

  #[inline]
  fn path(&self) -> Cow<'_, Path> {
    self.0.path()
  }
}

// == EnvVarsBoxed ==

pub trait EnvVarsBoxed {
  fn env_vars_os_boxed(&self)
    -> Box<dyn Iterator<Item = (OsString, OsString)>>;
}

impl<T: crate::EnvVars> EnvVarsBoxed for T
where
  T::EnvVarsOs: 'static,
{
  fn env_vars_os_boxed(
    &self,
  ) -> Box<dyn Iterator<Item = (OsString, OsString)>> {
    Box::new(self.env_vars_os())
  }
}

// == FsReadDirBoxed ==

pub trait FsReadDirBoxed {
  fn fs_read_dir_boxed(
    &self,
    path: &Path,
  ) -> io::Result<Box<dyn Iterator<Item = io::Result<BoxedFsDirEntry>>>>;
}

impl<T: BaseFsReadDir> FsReadDirBoxed for T {
  fn fs_read_dir_boxed(
    &self,
    path: &Path,
  ) -> io::Result<Box<dyn Iterator<Item = io::Result<BoxedFsDirEntry>>>> {
    let iter = self.base_fs_read_dir(path)?;
    Ok(Box::new(
      iter.map(|result| result.map(BoxedFsDirEntry::new)),
    ))
  }
}
