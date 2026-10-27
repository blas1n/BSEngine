use core::str;
use std::borrow::Cow;
use std::env::VarError;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io;
use std::io::Error;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

pub mod boxed;
pub mod ctx;
pub mod impls;

pub use sys_traits_macros::auto_impl;

pub use self::ctx::FsFileWithPathsInErrors;
pub use self::ctx::OperationError;
pub use self::ctx::OperationErrorKind;
pub use self::ctx::PathsInErrorsExt;
pub use self::ctx::SysWithPathsInErrors;

use self::boxed::BoxedFsFile;
use self::boxed::BoxedFsMetadataValue;

// #### ENVIRONMENT ####

// == EnvCurrentDir ==

pub trait EnvCurrentDir {
  fn env_current_dir(&self) -> io::Result<PathBuf>;
}

// == EnvSetCurrentDir ==

pub trait BaseEnvSetCurrentDir {
  #[doc(hidden)]
  fn base_env_set_current_dir(&self, path: &Path) -> io::Result<()>;
}

pub trait EnvSetCurrentDir: BaseEnvSetCurrentDir {
  #[inline]
  fn env_set_current_dir(&self, path: impl AsRef<Path>) -> io::Result<()> {
    self.base_env_set_current_dir(path.as_ref())
  }
}

impl<T: BaseEnvSetCurrentDir> EnvSetCurrentDir for T {}

// == EnvVar ==

pub trait BaseEnvVar {
  #[doc(hidden)]
  fn base_env_var_os(&self, key: &OsStr) -> Option<OsString>;
}

pub trait EnvVar: BaseEnvVar {
  #[inline]
  fn env_var_os(&self, key: impl AsRef<OsStr>) -> Option<OsString> {
    self.base_env_var_os(key.as_ref())
  }

  fn env_var(&self, key: impl AsRef<OsStr>) -> Result<String, VarError> {
    match self.env_var_os(key) {
      Some(val) => val.into_string().map_err(VarError::NotUnicode),
      None => Err(VarError::NotPresent),
    }
  }

  /// Helper to get a path from an environment variable.
  fn env_var_path(&self, key: impl AsRef<OsStr>) -> Option<PathBuf> {
    self
      .env_var_os(key)
      .and_then(|h| if h.is_empty() { None } else { Some(h) })
      .map(|value| {
        #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
        {
          impls::wasm_string_to_path(value.to_string_lossy().to_string())
        }
        #[cfg(any(not(target_arch = "wasm32"), not(feature = "wasm")))]
        {
          PathBuf::from(value)
        }
      })
  }
}

impl<T: BaseEnvVar> EnvVar for T {}

// == EnvVars ==

pub trait EnvVars {
  type EnvVarsOs: Iterator<Item = (OsString, OsString)>;

  fn env_vars_os(&self) -> Self::EnvVarsOs;

  fn env_vars(&self) -> EnvVarsStrings<Self::EnvVarsOs> {
    EnvVarsStrings(self.env_vars_os())
  }
}

pub struct EnvVarsStrings<I>(I);

impl<I: Iterator<Item = (OsString, OsString)>> Iterator for EnvVarsStrings<I> {
  type Item = (String, String);

  fn next(&mut self) -> Option<Self::Item> {
    loop {
      let (k, v) = self.0.next()?;
      if let (Ok(k), Ok(v)) = (k.into_string(), v.into_string()) {
        return Some((k, v));
      }
    }
  }
}

// == EnvRemoveVar ==

pub trait BaseEnvRemoveVar {
  #[doc(hidden)]
  fn base_env_remove_var(&self, key: &OsStr);
}

pub trait EnvRemoveVar: BaseEnvRemoveVar {
  fn env_remove_var(&self, key: impl AsRef<OsStr>) {
    self.base_env_remove_var(key.as_ref())
  }
}

impl<T: BaseEnvRemoveVar> EnvRemoveVar for T {}

// == EnvSetVar ==

pub trait BaseEnvSetVar {
  #[doc(hidden)]
  fn base_env_set_var(&self, key: &OsStr, value: &OsStr);
}

pub trait EnvSetVar: BaseEnvSetVar {
  fn env_set_var(&self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) {
    self.base_env_set_var(key.as_ref(), value.as_ref())
  }
}

impl<T: BaseEnvSetVar> EnvSetVar for T {}

// == EnvUmask ==

pub trait EnvUmask {
  fn env_umask(&self) -> io::Result<u32>;
}

// == EnvSetUmask ==

pub trait EnvSetUmask {
  fn env_set_umask(&self, umask: u32) -> io::Result<u32>;
}

// == EnvCacheDir ==

pub trait EnvCacheDir {
  fn env_cache_dir(&self) -> Option<PathBuf>;
}

// == EnvHomeDir ==

pub trait EnvHomeDir {
  fn env_home_dir(&self) -> Option<PathBuf>;
}

// == EnvProgramsDir ==

pub trait EnvProgramsDir {
  fn env_programs_dir(&self) -> Option<PathBuf>;
}

// == EnvTempDir ==

pub trait EnvTempDir {
  fn env_temp_dir(&self) -> io::Result<PathBuf>;
}

// #### FILE SYSTEM ####

#[cfg(windows)]
type CustomFlagsValue = u32;
#[cfg(not(windows))]
type CustomFlagsValue = i32;

#[derive(Default, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default, rename_all = "camelCase"))]
#[non_exhaustive] // so we can add properties without breaking people
pub struct OpenOptions {
  pub read: bool,
  pub write: bool,
  pub create: bool,
  pub truncate: bool,
  pub append: bool,
  pub create_new: bool,
  /// Unix only. Ignored on Windows.
  pub mode: Option<u32>,
  /// Custom flags to set on Unix or Windows.
  ///
  /// On Windows this is a u32, but on Unix it's an i32.
  ///
  /// Note: only provide flags that make sense for the current operating system.
  pub custom_flags: Option<CustomFlagsValue>,
  /// Windows only. Ignored on Unix.
  pub access_mode: Option<u32>,
  /// Windows only. Ignored on Unix.
  pub share_mode: Option<u32>,
  /// Windows only. Ignored on Unix.
  pub attributes: Option<u32>,
  /// Windows only. Ignored on Unix.
  pub security_qos_flags: Option<u32>,
}

impl OpenOptions {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn new_read() -> Self {
    Self {
      read: true,
      write: false,
      create: false,
      truncate: false,
      append: false,
      create_new: false,
      ..Default::default()
    }
  }

  // todo: make this an instance method in the next version
  #[deprecated(note = "use `new_write` instead")]
  pub fn write() -> Self {
    Self::new_write()
  }

  pub fn new_write() -> Self {
    Self {
      read: false,
      write: true,
      create: true,
      truncate: true,
      append: false,
      create_new: false,
      ..Default::default()
    }
  }

  pub fn new_append() -> Self {
    Self {
      read: false,
      write: true,
      create: false,
      truncate: false,
      append: true,
      create_new: false,
      ..Default::default()
    }
  }

  #[inline]
  pub fn read(&mut self) -> &mut Self {
    self.read = true;
    self
  }

  #[inline]
  pub fn create(&mut self) -> &mut Self {
    self.create = true;
    self
  }

  #[inline]
  pub fn truncate(&mut self) -> &mut Self {
    self.truncate = true;
    self
  }

  #[inline]
  pub fn append(&mut self) -> &mut Self {
    self.append = true;
    self
  }

  #[inline]
  pub fn create_new(&mut self) -> &mut Self {
    self.create_new = true;
    self
  }

  #[inline]
  pub fn mode(&mut self, mode: u32) -> &mut Self {
    self.mode = Some(mode);
    self
  }

  #[inline]
  pub fn custom_flags(&mut self, flags: CustomFlagsValue) -> &mut Self {
    self.custom_flags = Some(flags);
    self
  }

  #[inline]
  pub fn access_mode(&mut self, value: u32) -> &mut Self {
    self.access_mode = Some(value);
    self
  }

  #[inline]
  pub fn share_mode(&mut self, value: u32) -> &mut Self {
    self.share_mode = Some(value);
    self
  }

  #[inline]
  pub fn attributes(&mut self, value: u32) -> &mut Self {
    self.attributes = Some(value);
    self
  }

  #[inline]
  pub fn security_qos_flags(&mut self, value: u32) -> &mut Self {
    self.security_qos_flags = Some(value);
    self
  }
}

// == FsCanonicalize ==

pub trait BaseFsCanonicalize {
  #[doc(hidden)]
  fn base_fs_canonicalize(&self, path: &Path) -> io::Result<PathBuf>;
}

pub trait FsCanonicalize: BaseFsCanonicalize {
  #[inline]
  fn fs_canonicalize(&self, path: impl AsRef<Path>) -> io::Result<PathBuf> {
    self.base_fs_canonicalize(path.as_ref())
  }
}

impl<T: BaseFsCanonicalize> FsCanonicalize for T {}

// == FsChown ==

pub trait BaseFsChown {
  #[doc(hidden)]
  fn base_fs_chown(
    &self,
    path: &Path,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> io::Result<()>;
}

pub trait FsChown: BaseFsChown {
  #[inline]
  fn fs_chown(
    &self,
    path: impl AsRef<Path>,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> io::Result<()> {
    self.base_fs_chown(path.as_ref(), uid, gid)
  }
}

impl<T: BaseFsChown> FsChown for T {}

// == FsSymlinkChown ==

pub trait BaseFsSymlinkChown {
  #[doc(hidden)]
  fn base_fs_symlink_chown(
    &self,
    path: &Path,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> io::Result<()>;
}

pub trait FsSymlinkChown: BaseFsSymlinkChown {
  #[inline]
  fn fs_symlink_chown(
    &self,
    path: impl AsRef<Path>,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> io::Result<()> {
    self.base_fs_symlink_chown(path.as_ref(), uid, gid)
  }
}

impl<T: BaseFsSymlinkChown> FsSymlinkChown for T {}

// == FsCloneFile ==

pub trait BaseFsCloneFile {
  #[doc(hidden)]
  fn base_fs_clone_file(&self, from: &Path, to: &Path) -> io::Result<()>;
}

pub trait FsCloneFile: BaseFsCloneFile {
  #[inline]
  fn fs_clone_file(
    &self,
    from: impl AsRef<Path>,
    to: impl AsRef<Path>,
  ) -> io::Result<()> {
    self.base_fs_clone_file(from.as_ref(), to.as_ref())
  }
}

impl<T: BaseFsCloneFile> FsCloneFile for T {}

// == FsCopy ==

pub trait BaseFsCopy {
  #[doc(hidden)]
  fn base_fs_copy(&self, from: &Path, to: &Path) -> io::Result<u64>;
}

pub trait FsCopy: BaseFsCopy {
  #[inline]
  fn fs_copy(
    &self,
    from: impl AsRef<Path>,
    to: impl AsRef<Path>,
  ) -> io::Result<u64> {
    self.base_fs_copy(from.as_ref(), to.as_ref())
  }
}

impl<T: BaseFsCopy> FsCopy for T {}

// == FsCreateDir ==

#[derive(Default, Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default, rename_all = "camelCase"))]
#[non_exhaustive] // so we can add properties without breaking people
pub struct CreateDirOptions {
  pub recursive: bool,
  /// Unix only. Ignored on Windows.
  pub mode: Option<u32>,
}

impl CreateDirOptions {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn new_recursive() -> Self {
    Self {
      recursive: true,
      ..Default::default()
    }
  }

  #[inline]
  pub fn recursive(&mut self) -> &mut Self {
    self.recursive = true;
    self
  }

  #[inline]
  pub fn mode(&mut self, mode: u32) -> &mut Self {
    self.mode = Some(mode);
    self
  }
}

pub trait BaseFsCreateDir {
  #[doc(hidden)]
  fn base_fs_create_dir(
    &self,
    path: &Path,
    options: &CreateDirOptions,
  ) -> io::Result<()>;
}

pub trait FsCreateDir: BaseFsCreateDir {
  fn fs_create_dir(
    &self,
    path: impl AsRef<Path>,
    options: &CreateDirOptions,
  ) -> io::Result<()> {
    self.base_fs_create_dir(path.as_ref(), options)
  }
}

impl<T: BaseFsCreateDir> FsCreateDir for T {}

// == FsCreateDirAll ==

pub trait FsCreateDirAll: BaseFsCreateDir {
  fn fs_create_dir_all(&self, path: impl AsRef<Path>) -> io::Result<()> {
    self.base_fs_create_dir(
      path.as_ref(),
      &CreateDirOptions {
        recursive: true,
        mode: None,
      },
    )
  }
}

impl<T: BaseFsCreateDir> FsCreateDirAll for T {}

// == FsHardLink ==

pub trait BaseFsHardLink {
  #[doc(hidden)]
  fn base_fs_hard_link(&self, src: &Path, dst: &Path) -> io::Result<()>;
}

pub trait FsHardLink: BaseFsHardLink {
  fn fs_hard_link(
    &self,
    src: impl AsRef<Path>,
    dst: impl AsRef<Path>,
  ) -> io::Result<()> {
    self.base_fs_hard_link(src.as_ref(), dst.as_ref())
  }
}

impl<T: BaseFsHardLink> FsHardLink for T {}

// == FsCreateJunction ==

pub trait BaseFsCreateJunction {
  #[doc(hidden)]
  fn base_fs_create_junction(
    &self,
    original: &Path,
    junction: &Path,
  ) -> io::Result<()>;
}

pub trait FsCreateJunction: BaseFsCreateJunction {
  /// Creates an NTFS junction.
  fn fs_create_junction(
    &self,
    original: impl AsRef<Path>,
    junction: impl AsRef<Path>,
  ) -> io::Result<()> {
    self.base_fs_create_junction(original.as_ref(), junction.as_ref())
  }
}

impl<T: BaseFsCreateJunction> FsCreateJunction for T {}

// == FsMetadata ==

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileType {
  File,
  Dir,
  Symlink,
  Unknown,
}

impl FileType {
  pub fn is_dir(&self) -> bool {
    *self == Self::Dir
  }

  pub fn is_file(&self) -> bool {
    *self == Self::File
  }

  pub fn is_symlink(&self) -> bool {
    *self == Self::Symlink
  }
}

impl From<std::fs::FileType> for FileType {
  fn from(file_type: std::fs::FileType) -> Self {
    if file_type.is_file() {
      FileType::File
    } else if file_type.is_dir() {
      FileType::Dir
    } else if file_type.is_symlink() {
      FileType::Symlink
    } else {
      FileType::Unknown
    }
  }
}

#[allow(clippy::len_without_is_empty)]
pub trait FsMetadataValue: std::fmt::Debug {
  fn file_type(&self) -> FileType;
  fn len(&self) -> u64;
  fn accessed(&self) -> io::Result<SystemTime>;
  fn created(&self) -> io::Result<SystemTime>;
  fn changed(&self) -> io::Result<SystemTime>;
  fn modified(&self) -> io::Result<SystemTime>;
  fn dev(&self) -> io::Result<u64>;
  fn ino(&self) -> io::Result<u64>;
  fn mode(&self) -> io::Result<u32>;
  fn nlink(&self) -> io::Result<u64>;
  fn uid(&self) -> io::Result<u32>;
  fn gid(&self) -> io::Result<u32>;
  fn rdev(&self) -> io::Result<u64>;
  fn blksize(&self) -> io::Result<u64>;
  fn blocks(&self) -> io::Result<u64>;
  fn is_block_device(&self) -> io::Result<bool>;
  fn is_char_device(&self) -> io::Result<bool>;
  fn is_fifo(&self) -> io::Result<bool>;
  fn is_socket(&self) -> io::Result<bool>;
  fn file_attributes(&self) -> io::Result<u32>;
}

pub trait BaseFsMetadata {
  type Metadata: FsMetadataValue;

  #[doc(hidden)]
  fn base_fs_metadata(&self, path: &Path) -> io::Result<Self::Metadata>;

  #[doc(hidden)]
  fn base_fs_symlink_metadata(&self, path: &Path)
    -> io::Result<Self::Metadata>;

  #[doc(hidden)]
  fn base_fs_exists(&self, path: &Path) -> io::Result<bool> {
    match self.base_fs_symlink_metadata(path) {
      Ok(_) => Ok(true),
      Err(err) => {
        if err.kind() == ErrorKind::NotFound {
          Ok(false)
        } else {
          Err(err)
        }
      }
    }
  }

  #[doc(hidden)]
  fn base_fs_exists_no_err(&self, path: &Path) -> bool {
    self.base_fs_exists(path).unwrap_or(false)
  }
}

/// These two functions are so cloesly related that it becomes verbose to
/// separate them out into two traits.
pub trait FsMetadata: BaseFsMetadata {
  #[inline]
  fn fs_metadata(&self, path: impl AsRef<Path>) -> io::Result<Self::Metadata> {
    self.base_fs_metadata(path.as_ref())
  }

  #[inline]
  fn fs_symlink_metadata(
    &self,
    path: impl AsRef<Path>,
  ) -> io::Result<Self::Metadata> {
    self.base_fs_symlink_metadata(path.as_ref())
  }

  #[inline]
  fn fs_is_file(&self, path: impl AsRef<Path>) -> io::Result<bool> {
    Ok(self.fs_metadata(path)?.file_type() == FileType::File)
  }

  #[inline]
  fn fs_is_file_no_err(&self, path: impl AsRef<Path>) -> bool {
    self.fs_is_file(path).unwrap_or(false)
  }

  #[inline]
  fn fs_is_dir(&self, path: impl AsRef<Path>) -> io::Result<bool> {
    Ok(self.fs_metadata(path)?.file_type() == FileType::Dir)
  }

  #[inline]
  fn fs_is_dir_no_err(&self, path: impl AsRef<Path>) -> bool {
    self.fs_is_dir(path).unwrap_or(false)
  }

  #[inline]
  fn fs_exists(&self, path: impl AsRef<Path>) -> io::Result<bool> {
    self.base_fs_exists(path.as_ref())
  }

  #[inline]
  fn fs_exists_no_err(&self, path: impl AsRef<Path>) -> bool {
    self.base_fs_exists_no_err(path.as_ref())
  }

  #[inline]
  fn fs_is_symlink(&self, path: impl AsRef<Path>) -> io::Result<bool> {
    Ok(self.fs_symlink_metadata(path)?.file_type() == FileType::Symlink)
  }

  #[inline]
  fn fs_is_symlink_no_err(&self, path: impl AsRef<Path>) -> bool {
    self.fs_is_symlink(path).unwrap_or(false)
  }
}

impl<T: BaseFsMetadata> FsMetadata for T {}

// == FsOpen ==

pub trait FsFile:
  std::io::Read
  + std::io::Write
  + std::io::Seek
  + FsFileIsTerminal
  + FsFileLock
  + FsFileMetadata
  + FsFileSetPermissions
  + FsFileSetTimes
  + FsFileSetLen
  + FsFileSyncAll
  + FsFileSyncData
  + FsFileAsRaw
{
}

pub trait BoxableFsFile: Sized {
  fn into_boxed(self) -> BoxedFsFile;
}

impl<T> BoxableFsFile for T
where
  T: FsFile + Sized + 'static,
{
  fn into_boxed(self) -> BoxedFsFile {
    BoxedFsFile(Box::new(self))
  }
}

pub trait BaseFsOpen {
  // ideally this wouldn't be constrained, but by not doing
  // this then the type parameters get really out of hand
  type File: FsFile + Sized + 'static;

  #[doc(hidden)]
  fn base_fs_open(
    &self,
    path: &Path,
    options: &OpenOptions,
  ) -> io::Result<Self::File>;
}

pub trait FsOpen: BaseFsOpen {
  #[inline]
  fn fs_open(
    &self,
    path: impl AsRef<Path>,
    options: &OpenOptions,
  ) -> io::Result<Self::File> {
    self.base_fs_open(path.as_ref(), options)
  }
}

impl<T: BaseFsOpen> FsOpen for T {}

// == FsRead ==

pub trait BaseFsRead {
  #[doc(hidden)]
  fn base_fs_read(&self, path: &Path) -> io::Result<Cow<'static, [u8]>>;
}

pub trait FsRead: BaseFsRead {
  #[inline]
  fn fs_read(&self, path: impl AsRef<Path>) -> io::Result<Cow<'static, [u8]>> {
    self.base_fs_read(path.as_ref())
  }

  fn fs_read_to_string(
    &self,
    path: impl AsRef<Path>,
  ) -> io::Result<Cow<'static, str>> {
    let bytes = self.fs_read(path)?;
    match bytes {
      Cow::Borrowed(bytes) => str::from_utf8(bytes)
        .map(Cow::Borrowed)
        .map_err(|e| e.to_string()),
      Cow::Owned(bytes) => String::from_utf8(bytes)
        .map(Cow::Owned)
        .map_err(|e| e.to_string()),
    }
    .map_err(|error_text| Error::new(ErrorKind::InvalidData, error_text))
  }

  fn fs_read_to_string_lossy(
    &self,
    path: impl AsRef<Path>,
  ) -> io::Result<Cow<'static, str>> {
    // Like String::from_utf8_lossy but operates on owned values
    #[inline(always)]
    fn string_from_utf8_lossy(buf: Vec<u8>) -> String {
      match String::from_utf8_lossy(&buf) {
        // buf contained non-utf8 chars than have been patched
        Cow::Owned(s) => s,
        // SAFETY: if Borrowed then the buf only contains utf8 chars,
        // we do this instead of .into_owned() to avoid copying the input buf
        Cow::Borrowed(_) => unsafe { String::from_utf8_unchecked(buf) },
      }
    }

    let bytes = self.fs_read(path)?;
    match bytes {
      Cow::Borrowed(bytes) => Ok(String::from_utf8_lossy(bytes)),
      Cow::Owned(bytes) => Ok(Cow::Owned(string_from_utf8_lossy(bytes))),
    }
  }
}

impl<T: BaseFsRead> FsRead for T {}

// == FsReadDir ==

pub trait FsDirEntry: std::fmt::Debug {
  type Metadata: FsMetadataValue;

  fn file_name(&self) -> Cow<'_, OsStr>;
  fn file_type(&self) -> io::Result<FileType>;
  fn metadata(&self) -> io::Result<Self::Metadata>;
  fn path(&self) -> Cow<'_, Path>;
}

pub trait BaseFsReadDir {
  type ReadDirEntry: FsDirEntry + 'static;

  #[doc(hidden)]
  fn base_fs_read_dir(
    &self,
    path: &Path,
  ) -> io::Result<Box<dyn Iterator<Item = io::Result<Self::ReadDirEntry>>>>;
}

pub trait FsReadDir: BaseFsReadDir {
  #[inline]
  fn fs_read_dir(
    &self,
    path: impl AsRef<Path>,
  ) -> io::Result<Box<dyn Iterator<Item = io::Result<Self::ReadDirEntry>>>> {
    self.base_fs_read_dir(path.as_ref())
  }
}

impl<T: BaseFsReadDir> FsReadDir for T {}

// == FsReadLink ==

pub trait BaseFsReadLink {
  #[doc(hidden)]
  fn base_fs_read_link(&self, path: &Path) -> io::Result<PathBuf>;
}

pub trait FsReadLink: BaseFsReadLink {
  #[inline]
  fn fs_read_link(&self, path: impl AsRef<Path>) -> io::Result<PathBuf> {
    self.base_fs_read_link(path.as_ref())
  }
}

impl<T: BaseFsReadLink> FsReadLink for T {}

// == FsRemoveDir ==

pub trait BaseFsRemoveDir {
  #[doc(hidden)]
  fn base_fs_remove_dir(&self, path: &Path) -> io::Result<()>;
}

pub trait FsRemoveDir: BaseFsRemoveDir {
  #[inline]
  fn fs_remove_dir(&self, path: impl AsRef<Path>) -> io::Result<()> {
    self.base_fs_remove_dir(path.as_ref())
  }
}

impl<T: BaseFsRemoveDir> FsRemoveDir for T {}

// == FsRemoveDirAll ==

pub trait BaseFsRemoveDirAll {
  #[doc(hidden)]
  fn base_fs_remove_dir_all(&self, path: &Path) -> io::Result<()>;
}

pub trait FsRemoveDirAll: BaseFsRemoveDirAll {
  #[inline]
  fn fs_remove_dir_all(&self, path: impl AsRef<Path>) -> io::Result<()> {
    self.base_fs_remove_dir_all(path.as_ref())
  }
}

impl<T: BaseFsRemoveDirAll> FsRemoveDirAll for T {}

// == FsRemoveFile ==

pub trait BaseFsRemoveFile {
  #[doc(hidden)]
  fn base_fs_remove_file(&self, path: &Path) -> io::Result<()>;
}

pub trait FsRemoveFile: BaseFsRemoveFile {
  #[inline]
  fn fs_remove_file(&self, path: impl AsRef<Path>) -> io::Result<()> {
    self.base_fs_remove_file(path.as_ref())
  }
}

impl<T: BaseFsRemoveFile> FsRemoveFile for T {}

// == FsRename ==

pub trait BaseFsRename {
  #[doc(hidden)]
  fn base_fs_rename(&self, from: &Path, to: &Path) -> io::Result<()>;
}

pub trait FsRename: BaseFsRename {
  #[inline]
  fn fs_rename(
    &self,
    from: impl AsRef<Path>,
    to: impl AsRef<Path>,
  ) -> io::Result<()> {
    self.base_fs_rename(from.as_ref(), to.as_ref())
  }
}

impl<T: BaseFsRename> FsRename for T {}

// == FsSetFileTimes ==

pub trait BaseFsSetFileTimes {
  #[doc(hidden)]
  fn base_fs_set_file_times(
    &self,
    path: &Path,
    atime: SystemTime,
    mtime: SystemTime,
  ) -> io::Result<()>;
}

pub trait FsSetFileTimes: BaseFsSetFileTimes {
  #[inline]
  fn fs_set_file_times(
    &self,
    path: impl AsRef<Path>,
    atime: SystemTime,
    mtime: SystemTime,
  ) -> io::Result<()> {
    self.base_fs_set_file_times(path.as_ref(), atime, mtime)
  }
}

impl<T: BaseFsSetFileTimes> FsSetFileTimes for T {}

// == FsSetSymlinkFileTimes ==

pub trait BaseFsSetSymlinkFileTimes {
  #[doc(hidden)]
  fn base_fs_set_symlink_file_times(
    &self,
    path: &Path,
    atime: SystemTime,
    mtime: SystemTime,
  ) -> io::Result<()>;
}

pub trait FsSetSymlinkFileTimes: BaseFsSetSymlinkFileTimes {
  #[inline]
  fn fs_set_symlink_file_times(
    &self,
    path: impl AsRef<Path>,
    atime: SystemTime,
    mtime: SystemTime,
  ) -> io::Result<()> {
    self.base_fs_set_symlink_file_times(path.as_ref(), atime, mtime)
  }
}

impl<T: BaseFsSetSymlinkFileTimes> FsSetSymlinkFileTimes for T {}

// == FsSetPermissions ==

pub trait BaseFsSetPermissions {
  #[doc(hidden)]
  fn base_fs_set_permissions(&self, path: &Path, mode: u32) -> io::Result<()>;
}

pub trait FsSetPermissions: BaseFsSetPermissions {
  fn fs_set_permissions(
    &self,
    path: impl AsRef<Path>,
    mode: u32,
  ) -> io::Result<()> {
    self.base_fs_set_permissions(path.as_ref(), mode)
  }
}

impl<T: BaseFsSetPermissions> FsSetPermissions for T {}

// == FsSymlinkDir ==

pub trait BaseFsSymlinkDir {
  #[doc(hidden)]
  fn base_fs_symlink_dir(&self, original: &Path, link: &Path)
    -> io::Result<()>;
}

pub trait FsSymlinkDir: BaseFsSymlinkDir {
  #[inline]
  fn fs_symlink_dir(
    &self,
    original: impl AsRef<Path>,
    link: impl AsRef<Path>,
  ) -> io::Result<()> {
    self.base_fs_symlink_dir(original.as_ref(), link.as_ref())
  }
}

impl<T: BaseFsSymlinkDir> FsSymlinkDir for T {}

// == FsSymlinkFile ==

pub trait BaseFsSymlinkFile {
  #[doc(hidden)]
  fn base_fs_symlink_file(
    &self,
    original: &Path,
    link: &Path,
  ) -> io::Result<()>;
}

pub trait FsSymlinkFile: BaseFsSymlinkFile {
  #[inline]
  fn fs_symlink_file(
    &self,
    original: impl AsRef<Path>,
    link: impl AsRef<Path>,
  ) -> io::Result<()> {
    self.base_fs_symlink_file(original.as_ref(), link.as_ref())
  }
}

impl<T: BaseFsSymlinkFile> FsSymlinkFile for T {}

// == FsWrite ==

pub trait BaseFsWrite {
  #[doc(hidden)]
  fn base_fs_write(&self, path: &Path, data: &[u8]) -> io::Result<()>;
}

pub trait FsWrite: BaseFsWrite {
  #[inline]
  fn fs_write(
    &self,
    path: impl AsRef<Path>,
    data: impl AsRef<[u8]>,
  ) -> io::Result<()> {
    self.base_fs_write(path.as_ref(), data.as_ref())
  }
}

impl<T: BaseFsWrite> FsWrite for T {}

// #### FILE SYSTEM FILE ####

pub trait FsFileAsRaw {
  /// Returns the raw handle for a file on Windows platforms only
  /// or `None` when the file doesn't support it (ex. in-memory file system).
  #[cfg(windows)]
  fn fs_file_as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle>;

  /// Returns the raw file descriptor on Unix platforms only
  /// or `None` when the file doesn't support it (ex. in-memory file system).
  #[cfg(unix)]
  fn fs_file_as_raw_fd(&self) -> Option<std::os::fd::RawFd>;
}

pub trait FsFileIsTerminal {
  fn fs_file_is_terminal(&self) -> bool;
}

pub enum FsFileLockMode {
  Shared,
  Exclusive,
}

pub trait FsFileLock {
  fn fs_file_lock(&mut self, mode: FsFileLockMode) -> io::Result<()>;
  fn fs_file_try_lock(&mut self, mode: FsFileLockMode) -> io::Result<()>;
  fn fs_file_unlock(&mut self) -> io::Result<()>;
}

pub trait FsFileMetadata {
  /// Gets the file metadata.
  ///
  /// This is boxed because I couldn't figure out how to do
  /// this well with type parameters.
  fn fs_file_metadata(&self) -> io::Result<BoxedFsMetadataValue>;
}

pub trait FsFileSetLen {
  fn fs_file_set_len(&mut self, size: u64) -> io::Result<()>;
}

pub trait FsFileSetPermissions {
  fn fs_file_set_permissions(&mut self, mode: u32) -> io::Result<()>;
}

#[derive(Debug, Clone, Default)]
pub struct FsFileTimes {
  pub accessed: Option<SystemTime>,
  pub modified: Option<SystemTime>,
}

impl FsFileTimes {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn accessed(&mut self, accessed: SystemTime) -> &mut Self {
    self.accessed = Some(accessed);
    self
  }

  pub fn modified(&mut self, accessed: SystemTime) -> &mut Self {
    self.modified = Some(accessed);
    self
  }
}

pub trait FsFileSetTimes {
  fn fs_file_set_times(&mut self, times: FsFileTimes) -> io::Result<()>;
}

pub trait FsFileSyncAll {
  fn fs_file_sync_all(&mut self) -> io::Result<()>;
}

pub trait FsFileSyncData {
  fn fs_file_sync_data(&mut self) -> io::Result<()>;
}

// #### SYSTEM ####

pub trait SystemTimeNow {
  fn sys_time_now(&self) -> std::time::SystemTime;
}

pub trait SystemRandom {
  fn sys_random(&self, buf: &mut [u8]) -> io::Result<()>;

  fn sys_random_u8(&self) -> io::Result<u8> {
    let mut buf = [0; 1];
    self.sys_random(&mut buf)?;
    Ok(buf[0])
  }

  fn sys_random_u32(&self) -> io::Result<u32> {
    let mut buf = [0; 4];
    self.sys_random(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
  }

  fn sys_random_u64(&self) -> io::Result<u64> {
    let mut buf = [0; 8];
    self.sys_random(&mut buf)?;
    Ok(u64::from_le_bytes(buf))
  }
}

pub trait ProcessExit {
  fn process_exit(&self, code: i32) -> !;
}

pub trait ThreadSleep {
  fn thread_sleep(&self, duration: std::time::Duration);
}
