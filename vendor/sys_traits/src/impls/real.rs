use std::borrow::Cow;
use std::env;
use std::fs;
use std::io::Result;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use io::IsTerminal;

use super::strip_unc_prefix;
use super::RealSys;

use crate::*;

// ==== Environment ====

impl EnvCurrentDir for RealSys {
  #[inline]
  fn env_current_dir(&self) -> std::io::Result<PathBuf> {
    env::current_dir()
  }
}

impl BaseEnvSetCurrentDir for RealSys {
  #[inline]
  fn base_env_set_current_dir(&self, path: &Path) -> std::io::Result<()> {
    env::set_current_dir(path)
  }
}

impl BaseEnvVar for RealSys {
  #[inline]
  fn base_env_var_os(&self, key: &OsStr) -> Option<OsString> {
    env::var_os(key)
  }
}

impl EnvVars for RealSys {
  type EnvVarsOs = env::VarsOs;

  #[inline]
  fn env_vars_os(&self) -> Self::EnvVarsOs {
    env::vars_os()
  }
}

impl BaseEnvRemoveVar for RealSys {
  #[inline]
  fn base_env_remove_var(&self, key: &OsStr) {
    env::remove_var(key);
  }
}

impl BaseEnvSetVar for RealSys {
  #[inline]
  fn base_env_set_var(&self, key: &OsStr, value: &OsStr) {
    env::set_var(key, value);
  }
}

#[cfg(all(unix, feature = "libc"))]
impl EnvUmask for RealSys {
  fn env_umask(&self) -> std::io::Result<u32> {
    use libc::mode_t;
    use libc::umask;

    // SAFETY: libc calls
    unsafe {
      // unfortuantely there's no way to get the umask without setting it
      // temporarily... so we set the value then restore it after
      let current_umask = umask(0o000 as mode_t);
      umask(current_umask);
      Ok(current_umask as u32)
    }
  }
}

#[cfg(not(unix))]
impl EnvUmask for RealSys {
  fn env_umask(&self) -> std::io::Result<u32> {
    Err(std::io::Error::new(
      ErrorKind::Unsupported,
      "umask is not supported on this platform",
    ))
  }
}

#[cfg(all(unix, feature = "libc"))]
impl EnvSetUmask for RealSys {
  fn env_set_umask(&self, value: u32) -> std::io::Result<u32> {
    // SAFETY: libc calls
    unsafe {
      use libc::mode_t;
      use libc::umask;

      let current_umask = umask(value as mode_t);
      Ok(current_umask as u32)
    }
  }
}

#[cfg(not(unix))]
impl EnvSetUmask for RealSys {
  fn env_set_umask(&self, _umask: u32) -> std::io::Result<u32> {
    Err(std::io::Error::new(
      ErrorKind::Unsupported,
      "umask is not supported on this platform",
    ))
  }
}

#[cfg(any(
  all(target_os = "windows", feature = "winapi"),
  all(unix, feature = "libc")
))]
impl EnvCacheDir for RealSys {
  #[inline]
  fn env_cache_dir(&self) -> Option<PathBuf> {
    real_cache_dir_with_env(self)
  }
}

/// Uses the provided env for environment variables and the home
/// directory, but falls back to real sys calls.
#[cfg(any(
  all(target_os = "windows", feature = "winapi"),
  all(unix, feature = "libc")
))]
pub fn real_cache_dir_with_env(
  env: &(impl EnvVar + EnvHomeDir),
) -> Option<PathBuf> {
  #[cfg(all(target_os = "windows", feature = "winapi"))]
  {
    let _ = env;
    known_folder(&windows_sys::Win32::UI::Shell::FOLDERID_LocalAppData)
  }
  #[cfg(all(unix, feature = "libc"))]
  {
    if cfg!(target_os = "macos") {
      env.env_home_dir().map(|h| h.join("Library/Caches"))
    } else {
      env
        .env_var_path("XDG_CACHE_HOME")
        .or_else(|| env.env_home_dir().map(|home| home.join(".cache")))
    }
  }
}

#[cfg(any(
  all(target_os = "windows", feature = "winapi"),
  all(unix, feature = "libc")
))]
impl EnvHomeDir for RealSys {
  fn env_home_dir(&self) -> Option<PathBuf> {
    real_home_dir_with_env(self)
  }
}

#[cfg(all(target_os = "windows", feature = "winapi"))]
impl EnvProgramsDir for RealSys {
  fn env_programs_dir(&self) -> Option<PathBuf> {
    known_folder(&windows_sys::Win32::UI::Shell::FOLDERID_UserProgramFiles)
  }
}

/// Uses the provided env for environment variables, but falls
/// back to real sys calls.
#[cfg(any(
  all(target_os = "windows", feature = "winapi"),
  all(unix, feature = "libc")
))]
pub fn real_home_dir_with_env(env: &impl EnvVar) -> Option<PathBuf> {
  #[cfg(all(target_os = "windows", feature = "winapi"))]
  {
    env.env_var_path("USERPROFILE").or_else(|| {
      known_folder(&windows_sys::Win32::UI::Shell::FOLDERID_Profile)
    })
  }
  #[cfg(all(unix, feature = "libc"))]
  {
    // This piece of code was taken from the deprecated home_dir() function in Rust's standard library
    unsafe fn fallback() -> Option<std::ffi::OsString> {
      let amt = match libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) {
        n if n < 0 => 512_usize,
        n => n as usize,
      };
      let mut buf = Vec::with_capacity(amt);
      let mut passwd: libc::passwd = std::mem::zeroed();
      let mut result = std::ptr::null_mut();
      match libc::getpwuid_r(
        libc::getuid(),
        &mut passwd,
        buf.as_mut_ptr(),
        buf.capacity(),
        &mut result,
      ) {
        0 if !result.is_null() => {
          let ptr = passwd.pw_dir as *const _;
          let bytes = std::ffi::CStr::from_ptr(ptr).to_bytes().to_vec();
          Some(std::os::unix::ffi::OsStringExt::from_vec(bytes))
        }
        _ => None,
      }
    }

    env.env_var_path("HOME").or_else(|| {
      // SAFETY: libc
      unsafe { fallback().map(PathBuf::from) }
    })
  }
}

impl EnvTempDir for RealSys {
  #[inline]
  fn env_temp_dir(&self) -> std::io::Result<PathBuf> {
    Ok(env::temp_dir())
  }
}

// ==== File System ====

impl BaseFsCanonicalize for RealSys {
  #[inline]
  fn base_fs_canonicalize(&self, path: &Path) -> Result<PathBuf> {
    fs::canonicalize(path).map(strip_unc_prefix)
  }
}

#[cfg(unix)]
impl BaseFsChown for RealSys {
  #[inline]
  fn base_fs_chown(
    &self,
    path: &Path,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> io::Result<()> {
    std::os::unix::fs::chown(path, uid, gid)
  }
}

#[cfg(not(unix))]
impl BaseFsChown for RealSys {
  #[inline]
  fn base_fs_chown(
    &self,
    _path: &Path,
    _uid: Option<u32>,
    _gid: Option<u32>,
  ) -> io::Result<()> {
    Err(Error::new(
      ErrorKind::Unsupported,
      "chown is not supported on this platform",
    ))
  }
}

#[cfg(unix)]
impl BaseFsSymlinkChown for RealSys {
  #[inline]
  fn base_fs_symlink_chown(
    &self,
    path: &Path,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> io::Result<()> {
    std::os::unix::fs::lchown(path, uid, gid)
  }
}

#[cfg(not(unix))]
impl BaseFsSymlinkChown for RealSys {
  #[inline]
  fn base_fs_symlink_chown(
    &self,
    _path: &Path,
    _uid: Option<u32>,
    _gid: Option<u32>,
  ) -> io::Result<()> {
    Err(Error::new(
      ErrorKind::Unsupported,
      "lchown is not supported on this platform",
    ))
  }
}

#[cfg(all(target_vendor = "apple", feature = "libc"))]
impl BaseFsCloneFile for RealSys {
  #[inline]
  fn base_fs_clone_file(&self, from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let from = std::ffi::CString::new(from.as_os_str().as_bytes())?;
    let to = std::ffi::CString::new(to.as_os_str().as_bytes())?;
    // SAFETY: `from` and `to` are valid C strings.
    let ret = unsafe { libc::clonefile(from.as_ptr(), to.as_ptr(), 0) };
    if ret != 0 {
      return Err(std::io::Error::last_os_error());
    }
    Ok(())
  }
}

#[cfg(not(all(target_vendor = "apple", feature = "libc")))]
impl BaseFsCloneFile for RealSys {
  fn base_fs_clone_file(&self, _from: &Path, _to: &Path) -> io::Result<()> {
    Err(std::io::Error::new(
      ErrorKind::Unsupported,
      "clonefile is not supported on this platform or the libc feature in sys_traits is not enabled",
    ))
  }
}

impl BaseFsCopy for RealSys {
  #[inline]
  fn base_fs_copy(&self, from: &Path, to: &Path) -> std::io::Result<u64> {
    fs::copy(from, to)
  }
}

impl BaseFsCreateDir for RealSys {
  fn base_fs_create_dir(
    &self,
    path: &Path,
    options: &CreateDirOptions,
  ) -> Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(options.recursive);
    #[cfg(unix)]
    {
      use std::os::unix::fs::DirBuilderExt;
      if let Some(mode) = options.mode {
        builder.mode(mode);
      }
    }
    builder.create(path)
  }
}

impl BaseFsHardLink for RealSys {
  #[inline]
  fn base_fs_hard_link(&self, src: &Path, dst: &Path) -> Result<()> {
    fs::hard_link(src, dst)
  }
}

impl BaseFsCreateJunction for RealSys {
  fn base_fs_create_junction(
    &self,
    original: &Path,
    junction: &Path,
  ) -> io::Result<()> {
    #[cfg(all(target_os = "windows", feature = "winapi"))]
    {
      junction::create(original, junction)
    }
    #[cfg(all(target_os = "windows", not(feature = "winapi")))]
    {
      _ = (original, junction);
      Err(Error::new(
        ErrorKind::Unsupported,
        "Enable the 'winapi' feature in sys_traits for junction support.",
      ))
    }
    #[cfg(unix)]
    {
      _ = (original, junction);
      Err(Error::new(
        ErrorKind::Unsupported,
        "Creating NTFS junctions is not supported on this platform",
      ))
    }
  }
}

macro_rules! unix_metadata_prop {
  ($id:ident, $type:ident) => {
    #[inline]
    fn $id(&self) -> Result<$type> {
      #[cfg(unix)]
      {
        use std::os::unix::fs::MetadataExt;
        Ok(self.0.$id())
      }
      #[cfg(not(unix))]
      {
        Err(Error::new(
          ErrorKind::Unsupported,
          concat!(stringify!($id), " is not supported on this platform"),
        ))
      }
    }
  };
}

macro_rules! unix_metadata_file_type_prop {
  ($id:ident, $type:ident) => {
    #[inline]
    fn $id(&self) -> Result<$type> {
      #[cfg(unix)]
      {
        use std::os::unix::fs::FileTypeExt;
        Ok(self.0.file_type().$id())
      }
      #[cfg(not(unix))]
      {
        Err(Error::new(
          ErrorKind::Unsupported,
          concat!(stringify!($id), " is not supported on this platform"),
        ))
      }
    }
  };
}

/// A wrapper type is used in order to force usages to
/// `use sys_traits::FsMetadataValue` so that the code
/// compiles under Wasm.
#[derive(Debug, Clone)]
pub struct RealFsMetadata(fs::Metadata);

impl FsMetadataValue for RealFsMetadata {
  #[inline]
  fn file_type(&self) -> FileType {
    self.0.file_type().into()
  }

  #[inline]
  fn len(&self) -> u64 {
    self.0.len()
  }

  #[inline]
  fn accessed(&self) -> Result<SystemTime> {
    self.0.accessed()
  }

  #[inline]
  fn changed(&self) -> Result<SystemTime> {
    #[cfg(unix)]
    {
      use std::os::unix::fs::MetadataExt;
      let changed = self.0.ctime();
      Ok(
        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(changed as u64),
      )
    }
    #[cfg(not(unix))]
    {
      Err(Error::new(
        ErrorKind::Unsupported,
        "ctime is not supported on this platform",
      ))
    }
  }

  #[inline]
  fn created(&self) -> Result<SystemTime> {
    self.0.created()
  }

  #[inline]
  fn modified(&self) -> Result<SystemTime> {
    self.0.modified()
  }

  unix_metadata_prop!(dev, u64);
  unix_metadata_prop!(ino, u64);
  unix_metadata_prop!(mode, u32);
  unix_metadata_prop!(nlink, u64);
  unix_metadata_prop!(uid, u32);
  unix_metadata_prop!(gid, u32);
  unix_metadata_prop!(rdev, u64);
  unix_metadata_prop!(blksize, u64);
  unix_metadata_prop!(blocks, u64);
  unix_metadata_file_type_prop!(is_block_device, bool);
  unix_metadata_file_type_prop!(is_char_device, bool);
  unix_metadata_file_type_prop!(is_fifo, bool);
  unix_metadata_file_type_prop!(is_socket, bool);

  fn file_attributes(&self) -> io::Result<u32> {
    #[cfg(windows)]
    {
      use std::os::windows::prelude::MetadataExt;
      Ok(self.0.file_attributes())
    }
    #[cfg(not(windows))]
    {
      Err(Error::new(
        ErrorKind::Unsupported,
        "file_attributes is not supported on this platform",
      ))
    }
  }
}

impl BaseFsMetadata for RealSys {
  type Metadata = RealFsMetadata;

  #[inline]
  fn base_fs_metadata(&self, path: &Path) -> Result<Self::Metadata> {
    fs::metadata(path).map(RealFsMetadata)
  }

  #[inline]
  fn base_fs_symlink_metadata(&self, path: &Path) -> Result<Self::Metadata> {
    fs::symlink_metadata(path).map(RealFsMetadata)
  }

  #[cfg(any(all(unix, feature = "libc"), all(windows, feature = "winapi")))]
  #[inline]
  fn base_fs_exists_no_err(&self, path: &Path) -> bool {
    #[cfg(unix)]
    {
      use libc::access;
      use libc::F_OK;
      use std::os::unix::ffi::OsStrExt;

      let Ok(c_path) = std::ffi::CString::new(path.as_os_str().as_bytes())
      else {
        return false;
      };

      // Safety: `access` is a system call and we ensure `c_path` is a valid C string.
      unsafe { access(c_path.as_ptr(), F_OK) == 0 }
    }

    #[cfg(windows)]
    {
      use std::os::windows::ffi::OsStrExt;
      use windows_sys::Win32::Storage::FileSystem::GetFileAttributesW;
      use windows_sys::Win32::Storage::FileSystem::INVALID_FILE_ATTRIBUTES;

      let wide_path: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

      // Safety: `GetFileAttributesW` is a Windows API call, and `wide_path` is null-terminated.
      unsafe {
        GetFileAttributesW(wide_path.as_ptr()) != INVALID_FILE_ATTRIBUTES
      }
    }
  }
}

impl BaseFsOpen for RealSys {
  type File = RealFsFile;

  fn base_fs_open(
    &self,
    path: &Path,
    options: &OpenOptions,
  ) -> std::io::Result<Self::File> {
    let mut builder = fs::OpenOptions::new();
    if let Some(mode) = options.mode {
      #[cfg(unix)]
      {
        use std::os::unix::fs::OpenOptionsExt;
        builder.mode(mode);
      }
      #[cfg(not(unix))]
      let _ = mode;
    }
    if let Some(flags) = options.custom_flags {
      #[cfg(unix)]
      {
        use std::os::unix::fs::OpenOptionsExt;
        builder.custom_flags(flags);
      }
      #[cfg(windows)]
      {
        use std::os::windows::fs::OpenOptionsExt;
        builder.custom_flags(flags);
      }
      #[cfg(all(not(windows), not(unix)))]
      let _ = flags;
    }
    if let Some(value) = options.access_mode {
      #[cfg(windows)]
      {
        use std::os::windows::fs::OpenOptionsExt;
        builder.access_mode(value);
      }
      #[cfg(not(windows))]
      let _ = value;
    }
    if let Some(value) = options.share_mode {
      #[cfg(windows)]
      {
        use std::os::windows::fs::OpenOptionsExt;
        builder.share_mode(value);
      }
      #[cfg(not(windows))]
      let _ = value;
    }
    if let Some(value) = options.attributes {
      #[cfg(windows)]
      {
        use std::os::windows::fs::OpenOptionsExt;
        builder.attributes(value);
      }
      #[cfg(not(windows))]
      let _ = value;
    }
    if let Some(value) = options.security_qos_flags {
      #[cfg(windows)]
      {
        use std::os::windows::fs::OpenOptionsExt;
        builder.security_qos_flags(value);
      }
      #[cfg(not(windows))]
      let _ = value;
    }
    builder
      .read(options.read)
      .write(options.write)
      .create(options.create)
      .truncate(options.truncate)
      .append(options.append)
      .create_new(options.create_new)
      .open(path)
      .map(RealFsFile)
  }
}

impl BaseFsRead for RealSys {
  #[inline]
  fn base_fs_read(&self, path: &Path) -> Result<Cow<'static, [u8]>> {
    fs::read(path).map(Cow::Owned)
  }
}

#[derive(Debug)]
pub struct RealFsDirEntry(fs::DirEntry);

impl FsDirEntry for RealFsDirEntry {
  type Metadata = RealFsMetadata;

  #[inline]
  fn file_name(&self) -> Cow<'_, OsStr> {
    Cow::Owned(self.0.file_name())
  }

  #[inline]
  fn file_type(&self) -> std::io::Result<FileType> {
    self.0.file_type().map(FileType::from)
  }

  #[inline]
  fn metadata(&self) -> std::io::Result<Self::Metadata> {
    self.0.metadata().map(RealFsMetadata)
  }

  #[inline]
  fn path(&self) -> Cow<'_, Path> {
    Cow::Owned(self.0.path())
  }
}

impl BaseFsReadDir for RealSys {
  type ReadDirEntry = RealFsDirEntry;

  #[inline]
  fn base_fs_read_dir(
    &self,
    path: &Path,
  ) -> std::io::Result<
    Box<dyn Iterator<Item = std::io::Result<Self::ReadDirEntry>>>,
  > {
    let iterator = fs::read_dir(path)?;
    Ok(Box::new(iterator.map(|result| result.map(RealFsDirEntry))))
  }
}

impl BaseFsReadLink for RealSys {
  fn base_fs_read_link(&self, path: &Path) -> io::Result<PathBuf> {
    fs::read_link(path)
  }
}

impl BaseFsRemoveDir for RealSys {
  #[inline]
  fn base_fs_remove_dir(&self, path: &Path) -> std::io::Result<()> {
    fs::remove_dir(path)
  }
}

impl BaseFsRemoveDirAll for RealSys {
  #[inline]
  fn base_fs_remove_dir_all(&self, path: &Path) -> std::io::Result<()> {
    fs::remove_dir_all(path)
  }
}

impl BaseFsRemoveFile for RealSys {
  #[inline]
  fn base_fs_remove_file(&self, path: &Path) -> std::io::Result<()> {
    fs::remove_file(path)
  }
}

impl BaseFsRename for RealSys {
  #[inline]
  fn base_fs_rename(&self, from: &Path, to: &Path) -> std::io::Result<()> {
    fs::rename(from, to)
  }
}

#[cfg(feature = "filetime")]
impl BaseFsSetFileTimes for RealSys {
  #[inline]
  fn base_fs_set_file_times(
    &self,
    path: &Path,
    atime: SystemTime,
    mtime: SystemTime,
  ) -> Result<()> {
    let atime = filetime::FileTime::from_system_time(atime);
    let mtime = filetime::FileTime::from_system_time(mtime);
    filetime::set_file_times(path, atime, mtime)
  }
}

#[cfg(feature = "filetime")]
impl BaseFsSetSymlinkFileTimes for RealSys {
  #[inline]
  fn base_fs_set_symlink_file_times(
    &self,
    path: &Path,
    atime: SystemTime,
    mtime: SystemTime,
  ) -> Result<()> {
    let atime = filetime::FileTime::from_system_time(atime);
    let mtime = filetime::FileTime::from_system_time(mtime);
    filetime::set_symlink_file_times(path, atime, mtime)
  }
}

#[cfg(unix)]
impl BaseFsSetPermissions for RealSys {
  #[inline]
  fn base_fs_set_permissions(
    &self,
    path: &Path,
    mode: u32,
  ) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let permissions = fs::Permissions::from_mode(mode);
    fs::set_permissions(path, permissions)
  }
}

#[cfg(not(unix))]
impl BaseFsSetPermissions for RealSys {
  fn base_fs_set_permissions(
    &self,
    _path: &Path,
    _mode: u32,
  ) -> std::io::Result<()> {
    Err(std::io::Error::new(
      ErrorKind::Unsupported,
      "cannot set path permissions on this platform",
    ))
  }
}

impl BaseFsSymlinkDir for RealSys {
  fn base_fs_symlink_dir(
    &self,
    original: &Path,
    link: &Path,
  ) -> std::io::Result<()> {
    #[cfg(windows)]
    {
      std::os::windows::fs::symlink_dir(original, link)
    }
    #[cfg(not(windows))]
    {
      std::os::unix::fs::symlink(original, link)
    }
  }
}

impl BaseFsSymlinkFile for RealSys {
  fn base_fs_symlink_file(
    &self,
    original: &Path,
    link: &Path,
  ) -> std::io::Result<()> {
    #[cfg(windows)]
    {
      std::os::windows::fs::symlink_file(original, link)
    }
    #[cfg(not(windows))]
    {
      std::os::unix::fs::symlink(original, link)
    }
  }
}

impl BaseFsWrite for RealSys {
  #[inline]
  fn base_fs_write(&self, path: &Path, data: &[u8]) -> std::io::Result<()> {
    fs::write(path, data)
  }
}

// ==== File System File ====

/// A wrapper type is used in order to force usages to
/// `use sys_traits::FsFile` so that the code
/// compiles under Wasm.
#[derive(Debug)]
pub struct RealFsFile(fs::File);

impl RealFsFile {
  pub fn from_raw(file: fs::File) -> Self {
    Self(file)
  }
}

impl FsFile for RealFsFile {}

impl FsFileAsRaw for RealFsFile {
  #[cfg(windows)]
  #[inline]
  fn fs_file_as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> {
    use std::os::windows::io::AsRawHandle;
    Some(self.0.as_raw_handle())
  }

  #[cfg(unix)]
  #[inline]
  fn fs_file_as_raw_fd(&self) -> Option<std::os::fd::RawFd> {
    use std::os::fd::AsRawFd;
    Some(self.0.as_raw_fd())
  }
}

impl FsFileIsTerminal for RealFsFile {
  #[inline]
  fn fs_file_is_terminal(&self) -> bool {
    self.0.is_terminal()
  }
}

impl FsFileLock for RealFsFile {
  fn fs_file_lock(&mut self, mode: FsFileLockMode) -> io::Result<()> {
    lock_file(&self.0, mode, false)
  }

  fn fs_file_try_lock(&mut self, mode: FsFileLockMode) -> io::Result<()> {
    lock_file(&self.0, mode, true)
  }

  fn fs_file_unlock(&mut self) -> io::Result<()> {
    unlock_file(&self.0)
  }
}

#[cfg(all(unix, feature = "libc"))]
fn lock_file(
  file: &fs::File,
  mode: FsFileLockMode,
  try_lock: bool,
) -> Result<()> {
  let operation = match mode {
    FsFileLockMode::Shared => libc::LOCK_SH,
    FsFileLockMode::Exclusive => libc::LOCK_EX,
  } | if try_lock { libc::LOCK_NB } else { 0 };

  flock(file, operation)
}

#[cfg(all(unix, feature = "libc"))]
#[inline]
fn unlock_file(file: &fs::File) -> Result<()> {
  flock(file, libc::LOCK_UN)
}

#[cfg(all(unix, feature = "libc"))]
fn flock(file: &fs::File, operation: i32) -> Result<()> {
  use std::os::unix::io::AsRawFd;

  // SAFETY: libc calls
  unsafe {
    let fd = file.as_raw_fd();
    let result = libc::flock(fd, operation);
    if result < 0 {
      Err(Error::last_os_error())
    } else {
      Ok(())
    }
  }
}

#[cfg(all(windows, feature = "winapi"))]
fn lock_file(
  file: &fs::File,
  mode: FsFileLockMode,
  try_lock: bool,
) -> Result<()> {
  use std::os::windows::io::AsRawHandle;

  use windows_sys::Win32::Foundation::FALSE;
  use windows_sys::Win32::Storage::FileSystem::LockFileEx;
  use windows_sys::Win32::Storage::FileSystem::LOCKFILE_EXCLUSIVE_LOCK;
  use windows_sys::Win32::Storage::FileSystem::LOCKFILE_FAIL_IMMEDIATELY;

  let flags = match mode {
    FsFileLockMode::Shared => 0,
    FsFileLockMode::Exclusive => LOCKFILE_EXCLUSIVE_LOCK,
  } | if try_lock {
    LOCKFILE_FAIL_IMMEDIATELY
  } else {
    0
  };

  // SAFETY: winapi calls
  unsafe {
    let mut overlapped = std::mem::zeroed();
    let success =
      LockFileEx(file.as_raw_handle(), flags, 0, !0, !0, &mut overlapped);
    if success == FALSE {
      Err(Error::last_os_error())
    } else {
      Ok(())
    }
  }
}

#[cfg(all(windows, feature = "winapi"))]
fn unlock_file(file: &fs::File) -> Result<()> {
  use std::os::windows::io::AsRawHandle;

  use windows_sys::Win32::Foundation::FALSE;
  use windows_sys::Win32::Storage::FileSystem::UnlockFile;

  // SAFETY: winapi calls
  unsafe {
    let success = UnlockFile(file.as_raw_handle(), 0, 0, !0, !0);
    if success == FALSE {
      Err(Error::last_os_error())
    } else {
      Ok(())
    }
  }
}

#[cfg(not(any(
  all(unix, feature = "libc"),
  all(windows, feature = "winapi")
)))]
fn lock_file(
  _file: &fs::File,
  _mode: FsFileLockMode,
  _try_lock: bool,
) -> Result<()> {
  Err(Error::new(
    ErrorKind::Unsupported,
    "file locking is not supported on this platform or the libc/winapi feature is not enabled",
  ))
}

#[cfg(not(any(
  all(unix, feature = "libc"),
  all(windows, feature = "winapi")
)))]
fn unlock_file(_file: &fs::File) -> Result<()> {
  Err(Error::new(
    ErrorKind::Unsupported,
    "file locking is not supported on this platform or the libc/winapi feature is not enabled",
  ))
}

impl FsFileSetLen for RealFsFile {
  #[inline]
  fn fs_file_set_len(&mut self, size: u64) -> std::io::Result<()> {
    self.0.set_len(size)
  }
}

impl FsFileMetadata for RealFsFile {
  #[inline]
  fn fs_file_metadata(&self) -> io::Result<BoxedFsMetadataValue> {
    self
      .0
      .metadata()
      .map(|m| BoxedFsMetadataValue::new(RealFsMetadata(m)))
  }
}

impl FsFileSetPermissions for RealFsFile {
  #[inline]
  fn fs_file_set_permissions(&mut self, mode: u32) -> Result<()> {
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      let permissions = fs::Permissions::from_mode(mode);
      self.0.set_permissions(permissions)
    }
    #[cfg(not(unix))]
    {
      let _ = mode;
      Ok(())
    }
  }
}

impl FsFileSetTimes for RealFsFile {
  fn fs_file_set_times(&mut self, times: FsFileTimes) -> io::Result<()> {
    let mut std_times = std::fs::FileTimes::new();
    if let Some(atime) = times.accessed {
      std_times = std_times.set_accessed(atime);
    }
    if let Some(mtime) = times.modified {
      std_times = std_times.set_modified(mtime);
    }
    self.0.set_times(std_times)
  }
}

impl FsFileSyncAll for RealFsFile {
  #[inline]
  fn fs_file_sync_all(&mut self) -> io::Result<()> {
    self.0.sync_all()
  }
}

impl FsFileSyncData for RealFsFile {
  #[inline]
  fn fs_file_sync_data(&mut self) -> io::Result<()> {
    self.0.sync_data()
  }
}

impl std::io::Seek for RealFsFile {
  #[inline]
  fn seek(&mut self, pos: std::io::SeekFrom) -> Result<u64> {
    self.0.seek(pos)
  }
}

impl std::io::Write for RealFsFile {
  #[inline]
  fn write(&mut self, buf: &[u8]) -> Result<usize> {
    self.0.write(buf)
  }

  #[inline]
  fn flush(&mut self) -> Result<()> {
    self.0.flush()
  }
}

impl std::io::Read for RealFsFile {
  #[inline]
  fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
    self.0.read(buf)
  }
}

// ==== System ====

impl SystemTimeNow for RealSys {
  #[inline]
  fn sys_time_now(&self) -> SystemTime {
    SystemTime::now()
  }
}

#[cfg(feature = "getrandom")]
impl crate::SystemRandom for RealSys {
  #[inline]
  fn sys_random(&self, buf: &mut [u8]) -> Result<()> {
    getrandom::getrandom(buf)
      .map_err(|err| Error::new(ErrorKind::Other, err.to_string()))
  }
}

impl crate::ProcessExit for RealSys {
  #[inline]
  fn process_exit(&self, code: i32) -> ! {
    std::process::exit(code)
  }
}

impl crate::ThreadSleep for RealSys {
  #[inline]
  fn thread_sleep(&self, duration: std::time::Duration) {
    std::thread::sleep(duration);
  }
}

#[cfg(all(windows, feature = "winapi"))]
fn known_folder(folder_id: *const windows_sys::core::GUID) -> Option<PathBuf> {
  use std::ffi::c_void;
  use std::os::windows::ffi::OsStringExt;
  use windows_sys::Win32::Foundation::S_OK;
  use windows_sys::Win32::Globalization::lstrlenW;
  use windows_sys::Win32::System::Com::CoTaskMemFree;
  use windows_sys::Win32::UI::Shell::SHGetKnownFolderPath;

  // SAFETY: winapi calls
  unsafe {
    let mut path_ptr = std::ptr::null_mut();
    let result =
      SHGetKnownFolderPath(folder_id, 0, std::ptr::null_mut(), &mut path_ptr);
    if result != S_OK {
      return None;
    }
    let len = lstrlenW(path_ptr) as usize;
    let path = std::slice::from_raw_parts(path_ptr, len);
    let ostr: OsString = OsStringExt::from_wide(path);
    CoTaskMemFree(path_ptr as *mut c_void);
    Some(PathBuf::from(ostr))
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[cfg(any(feature = "winapi", feature = "libc"))]
  #[test]
  fn test_known_folders() {
    assert!(RealSys.env_cache_dir().is_some());
    assert!(RealSys.env_home_dir().is_some());
  }

  #[cfg(all(target_os = "windows", feature = "winapi"))]
  #[test]
  fn test_known_folder_programs_dir() {
    // was failing on gh actions for some reason
    if std::env::var_os("CI").is_none() {
      assert!(RealSys.env_programs_dir().is_some());
    }
  }

  #[cfg(all(unix, feature = "libc"))]
  #[test]
  fn test_umask() {
    let original_umask = RealSys.env_umask().unwrap();
    assert_eq!(RealSys.env_set_umask(0o777).unwrap(), original_umask);
    assert_eq!(RealSys.env_set_umask(original_umask).unwrap(), 0o777);
  }

  #[cfg(target_os = "windows")]
  #[test]
  fn test_umask() {
    let err = RealSys.env_umask().unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Unsupported);
    let err = RealSys.env_set_umask(0o000).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Unsupported);
  }

  #[test]
  fn test_general() {
    assert!(RealSys.sys_time_now().elapsed().is_ok());
  }

  #[cfg(any(feature = "winapi", feature = "libc"))]
  #[test]
  fn lock_file() {
    let sys = RealSys;
    let mut file = sys.fs_open("Cargo.toml", &OpenOptions::new_read()).unwrap();
    file.fs_file_lock(FsFileLockMode::Shared).unwrap();
    file.fs_file_unlock().unwrap();
    file.fs_file_try_lock(FsFileLockMode::Shared).unwrap();
    file.fs_file_unlock().unwrap();
    file.fs_file_lock(FsFileLockMode::Exclusive).unwrap();
    file.fs_file_unlock().unwrap();
    file.fs_file_try_lock(FsFileLockMode::Exclusive).unwrap();
    file.fs_file_unlock().unwrap();
  }

  #[test]
  fn test_exists_no_err() {
    assert!(RealSys.fs_exists_no_err("Cargo.toml"));
    assert!(!RealSys.fs_exists_no_err("Cargo2.toml"));
  }

  #[test]
  fn test_clone_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path();
    RealSys.fs_write(path.join("file.txt"), "data").unwrap();
    let result =
      RealSys.fs_clone_file(path.join("file.txt"), path.join("cloned.txt"));
    if cfg!(target_vendor = "apple") {
      assert!(result.is_ok());
      assert_eq!(
        RealSys.fs_read_to_string(path.join("cloned.txt")).unwrap(),
        "data"
      );
    } else {
      assert_eq!(result.unwrap_err().kind(), ErrorKind::Unsupported);
    }
  }

  #[test]
  fn test_fs_canonicalize_empty() {
    let result = RealSys.fs_canonicalize("");
    assert_eq!(result.unwrap_err().kind(), ErrorKind::NotFound);
  }
}
