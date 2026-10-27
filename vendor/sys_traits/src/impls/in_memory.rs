use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Error;
use std::io::ErrorKind;
use std::io::Result;
use std::path::Component;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use parking_lot::RwLock;

use crate::*;

#[derive(Debug, Clone)]
pub struct InMemoryFile {
  sys: InMemorySys,
  path: PathBuf,
  inner: Arc<RwLock<FileInner>>,
  pos: usize,
}

impl FsFile for InMemoryFile {}

#[derive(Debug)]
struct FileInner {
  accessed: SystemTime,
  created: SystemTime,
  changed: SystemTime,
  modified: SystemTime,
  data: Vec<u8>,
  mode: u32,
}

#[derive(Debug)]
struct File {
  name: String,
  inner: Arc<RwLock<FileInner>>,
}

#[derive(Debug)]
enum DirectoryEntry {
  File(File),
  Directory(Directory),
  Symlink(Symlink),
}

impl DirectoryEntry {
  fn name(&self) -> &str {
    match self {
      DirectoryEntry::File(f) => &f.name,
      DirectoryEntry::Directory(d) => &d.name,
      DirectoryEntry::Symlink(s) => &s.name,
    }
  }

  fn file_type(&self) -> FileType {
    match self {
      DirectoryEntry::File(_) => FileType::File,
      DirectoryEntry::Directory(_) => FileType::Dir,
      DirectoryEntry::Symlink(_) => FileType::Symlink,
    }
  }

  fn len(&self) -> u64 {
    match self {
      DirectoryEntry::File(f) => f.inner.read().data.len() as u64,
      DirectoryEntry::Directory(_) => 0,
      DirectoryEntry::Symlink(_) => 0,
    }
  }

  fn accessed(&self) -> SystemTime {
    match self {
      DirectoryEntry::File(f) => f.inner.read().accessed,
      DirectoryEntry::Directory(d) => d.inner.read().accessed,
      DirectoryEntry::Symlink(s) => s.inner.read().accessed,
    }
  }

  fn created(&self) -> SystemTime {
    match self {
      DirectoryEntry::File(f) => f.inner.read().created,
      DirectoryEntry::Directory(d) => d.inner.read().created,
      DirectoryEntry::Symlink(s) => s.inner.read().created,
    }
  }

  fn changed(&self) -> SystemTime {
    match self {
      DirectoryEntry::File(f) => f.inner.read().changed,
      DirectoryEntry::Directory(d) => d.inner.read().changed,
      DirectoryEntry::Symlink(s) => s.inner.read().changed,
    }
  }

  fn modified(&self) -> SystemTime {
    match self {
      DirectoryEntry::File(f) => f.inner.read().modified,
      DirectoryEntry::Directory(d) => d.inner.read().modified,
      DirectoryEntry::Symlink(s) => s.inner.read().modified,
    }
  }

  fn mode(&self) -> u32 {
    match self {
      DirectoryEntry::File(f) => f.inner.read().mode,
      DirectoryEntry::Directory(d) => d.inner.read().mode,
      DirectoryEntry::Symlink(s) => s.inner.read().mode,
    }
  }

  fn set_filetimes(&self, atime: SystemTime, mtime: SystemTime) {
    match self {
      DirectoryEntry::Directory(entry) => {
        let mut entry = entry.inner.write();
        entry.accessed = atime;
        entry.changed = atime;
        entry.created = atime;
        entry.modified = mtime;
      }
      DirectoryEntry::File(entry) => {
        let mut entry = entry.inner.write();
        entry.accessed = atime;
        entry.changed = atime;
        entry.created = atime;
        entry.modified = mtime;
      }
      DirectoryEntry::Symlink(symlink) => {
        let mut inner = symlink.inner.write();
        inner.accessed = atime;
        inner.changed = atime;
        inner.created = atime;
        inner.modified = mtime;
      }
    }
  }
}

#[derive(Debug)]
struct SymlinkInner {
  accessed: SystemTime,
  created: SystemTime,
  changed: SystemTime,
  modified: SystemTime,
  mode: u32,
}

#[derive(Debug)]
struct Symlink {
  name: String,
  target: PathBuf,
  inner: RwLock<SymlinkInner>,
}

#[derive(Debug)]
struct DirectoryInner {
  accessed: SystemTime,
  created: SystemTime,
  changed: SystemTime,
  modified: SystemTime,
  mode: u32,
}

#[derive(Debug)]
struct Directory {
  name: String,
  inner: RwLock<DirectoryInner>,
  entries: Vec<DirectoryEntry>,
}

enum LookupEntry<'a> {
  NotFound(PathBuf),
  Found(PathBuf, &'a DirectoryEntry),
}

enum LookupNoFollowEntry<'a> {
  NotFound(PathBuf),
  Symlink {
    current_path: PathBuf,
    target_path: PathBuf,
    entry: &'a Symlink,
  },
  Found(PathBuf, &'a DirectoryEntry),
}

#[derive(Debug)]
struct InMemorySysInner {
  // Linux/Mac will always have one dir here, but Windows
  // may have multiple per drive.
  system_root: Vec<DirectoryEntry>,
  cwd: PathBuf,
  thread_sleep_enabled: bool,
  random_seed: Option<u64>,
  envs: HashMap<OsString, OsString>,
  time: Option<SystemTime>,
  umask: u32,
}

impl InMemorySysInner {
  fn to_absolute_path(&self, p: &Path) -> PathBuf {
    if p.is_absolute() {
      normalize_path(p)
    } else {
      normalize_path(&self.cwd.join(p))
    }
  }

  fn time_now(&self) -> SystemTime {
    self.time.unwrap_or_else(SystemTime::now)
  }

  fn lookup_entry<'a>(
    &'a self,
    path: &Path,
  ) -> Result<(PathBuf, &'a DirectoryEntry)> {
    match self.lookup_entry_detail(path)? {
      LookupEntry::Found(path, entry) => Ok((path, entry)),
      LookupEntry::NotFound(_) => Err(Error::new(
        ErrorKind::NotFound,
        format!("Path not found: '{}'", path.display()),
      )),
    }
  }

  fn lookup_entry_detail<'a>(&'a self, path: &Path) -> Result<LookupEntry<'a>> {
    let mut seen_entries = HashSet::new();
    let mut path = Cow::Borrowed(path);

    loop {
      match self.lookup_entry_detail_no_follow(&path)? {
        LookupNoFollowEntry::NotFound(path) => {
          return Ok(LookupEntry::NotFound(path));
        }
        LookupNoFollowEntry::Found(path, entry) => {
          return Ok(LookupEntry::Found(path, entry));
        }
        LookupNoFollowEntry::Symlink {
          current_path,
          target_path,
          ..
        } => {
          if seen_entries.is_empty() {
            // add the original path at this point in order to avoid allocating when we
            // don't have symlinks
            seen_entries.insert(current_path.clone());
          }
          if !seen_entries.insert(target_path.clone()) {
            return Err(Error::new(
              ErrorKind::Other,
              format!("Symlink loop detected resolving '{}'", path.display()),
            ));
          }
          path = Cow::Owned(target_path);
        }
      }
    }
  }

  fn lookup_entry_detail_no_follow<'a>(
    &'a self,
    path: &Path,
  ) -> Result<LookupNoFollowEntry<'a>> {
    let mut final_path = Vec::new();
    let mut comps = path.components().peekable();
    if comps.peek().is_none() {
      return Err(Error::new(ErrorKind::NotFound, "Empty path"));
    }

    let mut entries = &self.system_root;
    while let Some(comp) = comps.next() {
      final_path.push(comp);
      let comp = match comp {
        Component::RootDir => Cow::Borrowed(""),
        Component::Prefix(component) => {
          let component = component.as_os_str().to_string_lossy();
          if let Some(comp) = comps.next() {
            final_path.push(comp);
          }
          component
        }
        component => component.as_os_str().to_string_lossy(),
      };
      let pos = match entries.binary_search_by(|e| e.name().cmp(&comp)) {
        Ok(p) => p,
        Err(_) => {
          return Ok(LookupNoFollowEntry::NotFound(
            final_path.into_iter().chain(comps).collect(),
          ));
        }
      };

      match &entries[pos] {
        DirectoryEntry::Directory(dir) => {
          if comps.peek().is_none() {
            return Ok(LookupNoFollowEntry::Found(
              final_path.into_iter().collect(),
              &entries[pos],
            ));
          } else {
            entries = &dir.entries;
          }
        }
        DirectoryEntry::File(_) => {
          if comps.peek().is_none() {
            return Ok(LookupNoFollowEntry::Found(
              final_path.into_iter().collect(),
              &entries[pos],
            ));
          } else {
            return Err(Error::new(
              ErrorKind::Other,
              "Path leads into a file or symlink",
            ));
          }
        }
        DirectoryEntry::Symlink(symlink) => {
          let current_path = final_path.into_iter().collect::<PathBuf>();
          let target_path = normalize_path(&current_path.join(&symlink.target));
          return Ok(LookupNoFollowEntry::Symlink {
            current_path,
            target_path,
            entry: symlink,
          });
        }
      }
    }

    Ok(LookupNoFollowEntry::NotFound(
      final_path.into_iter().collect(),
    ))
  }

  fn find_directory_mut<'a>(
    &'a mut self,
    path: &Path,
    create_dirs: bool,
  ) -> Result<&'a mut Directory> {
    // ran into a lot of issues with the borrow checker... recommendation was to
    // resolve symlinks first then resolve the path
    let path = match self.lookup_entry_detail(path)? {
      LookupEntry::Found(path, _) => path,
      LookupEntry::NotFound(path) => path,
    };

    let time = self.time_now();
    let mut comps = path.components().peekable();
    if comps.peek().is_none() {
      return Err(Error::new(ErrorKind::NotFound, "Empty path"));
    }

    let mut entries = &mut self.system_root;
    while let Some(comp) = comps.next() {
      let comp = match comp {
        Component::RootDir => Cow::Borrowed(""),
        Component::Prefix(component) => {
          let component = component.as_os_str().to_string_lossy();
          comps.next();
          component
        }
        component => component.as_os_str().to_string_lossy(),
      };
      let pos = match entries.binary_search_by(|e| e.name().cmp(&comp)) {
        Ok(p) => p,
        Err(insert_pos) => {
          if create_dirs {
            let new_dir = Directory {
              name: comp.into_owned(),
              inner: RwLock::new(DirectoryInner {
                accessed: time,
                changed: time,
                created: time,
                modified: time,
                mode: 0o755,
              }),
              entries: vec![],
            };
            entries.insert(insert_pos, DirectoryEntry::Directory(new_dir));
            insert_pos
          } else {
            return Err(Error::new(ErrorKind::NotFound, "Path not found"));
          }
        }
      };

      match &mut entries[pos] {
        DirectoryEntry::Directory(dir) => {
          if comps.peek().is_none() {
            return Ok(dir);
          } else {
            entries = &mut dir.entries;
          }
        }
        DirectoryEntry::File(_) | DirectoryEntry::Symlink { .. } => {
          return Err(Error::new(
            ErrorKind::Other,
            "Path leads into a file or symlink",
          ));
        }
      }
    }

    Err(Error::new(ErrorKind::NotFound, "Path not found"))
  }
}

/// An in-memory system implementation useful for testing.
///
/// This is extremely untested and sloppily implemented. Use with extreme caution
/// and only for testing. You will encounter bugs. Please submit fixes. I implemented
/// this lazily and quickly.
#[derive(Debug, Clone)]
pub struct InMemorySys(Arc<RwLock<InMemorySysInner>>);

impl Default for InMemorySys {
  fn default() -> Self {
    Self(Arc::new(RwLock::new(InMemorySysInner {
      envs: Default::default(),
      system_root: vec![],
      cwd: PathBuf::from("/"),
      thread_sleep_enabled: true,
      random_seed: None,
      time: None,
      umask: 0o666,
    })))
  }
}

impl InMemorySys {
  pub fn new_with_cwd(cwd: impl AsRef<Path>) -> Self {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all(cwd.as_ref()).unwrap();
    sys.env_set_current_dir(cwd.as_ref()).unwrap();
    sys
  }

  pub fn set_seed(&self, seed: Option<u64>) {
    self.0.write().random_seed = seed;
  }

  pub fn set_time(&self, time: Option<SystemTime>) {
    self.0.write().time = time;
  }

  /// Makes thread sleeping a no-op.
  pub fn disable_thread_sleep(&self) {
    self.0.write().thread_sleep_enabled = false;
  }

  pub fn fs_insert(&self, path: impl AsRef<Path>, data: impl AsRef<[u8]>) {
    self
      .fs_create_dir_all(path.as_ref().parent().unwrap())
      .unwrap();
    self.fs_write(path, data).unwrap();
  }

  /// Helper method for inserting json into the in-memory file system.
  #[cfg(feature = "serde_json")]
  pub fn fs_insert_json(
    &self,
    path: impl AsRef<Path>,
    json: serde_json::Value,
  ) {
    self
      .fs_create_dir_all(path.as_ref().parent().unwrap())
      .unwrap();
    self
      .fs_write(path, serde_json::to_string(&json).unwrap())
      .unwrap();
  }
}

impl EnvCurrentDir for InMemorySys {
  fn env_current_dir(&self) -> std::io::Result<PathBuf> {
    Ok(self.0.read().cwd.clone())
  }
}

impl BaseEnvSetCurrentDir for InMemorySys {
  fn base_env_set_current_dir(&self, path: &Path) -> std::io::Result<()> {
    let path = self.fs_canonicalize(path)?; // cause an error if not exists
    self.0.write().cwd = path;
    Ok(())
  }
}

impl BaseEnvVar for InMemorySys {
  fn base_env_var_os(&self, key: &OsStr) -> Option<OsString> {
    self.0.read().envs.get(key).cloned()
  }
}

impl EnvVars for InMemorySys {
  type EnvVarsOs = std::collections::hash_map::IntoIter<OsString, OsString>;

  fn env_vars_os(&self) -> Self::EnvVarsOs {
    self.0.read().envs.clone().into_iter()
  }
}

impl BaseEnvRemoveVar for InMemorySys {
  fn base_env_remove_var(&self, key: &OsStr) {
    self.0.write().envs.remove(key);
  }
}

impl BaseEnvSetVar for InMemorySys {
  fn base_env_set_var(&self, key: &OsStr, value: &OsStr) {
    self
      .0
      .write()
      .envs
      .insert(key.to_os_string(), value.to_os_string());
  }
}

impl EnvCacheDir for InMemorySys {
  fn env_cache_dir(&self) -> Option<PathBuf> {
    self.env_home_dir().map(|h| h.join(".cache"))
  }
}

impl EnvHomeDir for InMemorySys {
  fn env_home_dir(&self) -> Option<PathBuf> {
    self.env_var("HOME").ok().map(PathBuf::from)
  }
}

impl EnvTempDir for InMemorySys {
  fn env_temp_dir(&self) -> std::io::Result<PathBuf> {
    let inner = self.0.read();
    if let Some(first_dir) = inner.system_root.first() {
      let name = first_dir.name();
      let name = if name.is_empty() { "/" } else { name };
      Ok(PathBuf::from(name).join("tmp"))
    } else {
      Err(std::io::Error::new(ErrorKind::Other, "Create a root for the InMemorySys file system before getting the temp dir."))
    }
  }
}

impl EnvUmask for InMemorySys {
  fn env_umask(&self) -> std::io::Result<u32> {
    Ok(self.0.read().umask)
  }
}

impl EnvSetUmask for InMemorySys {
  fn env_set_umask(&self, umask: u32) -> std::io::Result<u32> {
    let mut inner = self.0.write();
    let old = inner.umask;
    inner.umask = umask;
    Ok(old)
  }
}

// File System

impl BaseFsCanonicalize for InMemorySys {
  fn base_fs_canonicalize(&self, path: &Path) -> Result<PathBuf> {
    if path.as_os_str().is_empty() {
      return Err(Error::new(ErrorKind::NotFound, "No such file or directory"));
    }
    let inner = self.0.read();
    let path = inner.to_absolute_path(path);
    let (path, _) = inner.lookup_entry(&path)?;
    Ok(path)
  }
}

impl BaseFsChown for InMemorySys {
  fn base_fs_chown(
    &self,
    _path: &Path,
    _uid: Option<u32>,
    _gid: Option<u32>,
  ) -> Result<()> {
    Err(Error::new(
      ErrorKind::Other,
      "chown is not supported on this platform",
    ))
  }
}

impl BaseFsSymlinkChown for InMemorySys {
  fn base_fs_symlink_chown(
    &self,
    _path: &Path,
    _uid: Option<u32>,
    _gid: Option<u32>,
  ) -> Result<()> {
    Err(Error::new(
      ErrorKind::Other,
      "lchown is not supported on this platform",
    ))
  }
}

impl BaseFsCopy for InMemorySys {
  fn base_fs_copy(&self, from: &Path, to: &Path) -> Result<u64> {
    let data = self.fs_read(from)?;
    let len = data.len();
    self.fs_write(to, data)?;
    Ok(len as u64)
  }
}

impl BaseFsCloneFile for InMemorySys {
  fn base_fs_clone_file(&self, from: &Path, to: &Path) -> Result<()> {
    self.base_fs_copy(from, to).map(|_| ())
  }
}

impl BaseFsCreateDir for InMemorySys {
  fn base_fs_create_dir(
    &self,
    path: &Path,
    // todo: implement this properly
    _options: &CreateDirOptions,
  ) -> Result<()> {
    let mut inner = self.0.write();
    let abs = inner.to_absolute_path(path);
    inner.find_directory_mut(&abs, true)?;
    Ok(())
  }
}

impl BaseFsHardLink for InMemorySys {
  fn base_fs_hard_link(&self, src: &Path, dst: &Path) -> Result<()> {
    let inner = self.0.read();
    let src = inner.to_absolute_path(src.as_ref());
    let dst = inner.to_absolute_path(dst.as_ref());
    let (_, entry) = inner.lookup_entry(&src)?;
    match entry {
      DirectoryEntry::File(file) => {
        let data = {
          let inner = file.inner.read();
          inner.data.clone()
        };
        drop(inner);
        self.fs_write(&dst, data)?;
      }
      DirectoryEntry::Directory(_) | DirectoryEntry::Symlink(_) => {
        return Err(Error::new(
          ErrorKind::Other,
          "Cannot hard link directories or symlinks",
        ));
      }
    }
    Ok(())
  }
}

impl BaseFsCreateJunction for InMemorySys {
  fn base_fs_create_junction(
    &self,
    _original: &Path,
    _junction: &Path,
  ) -> io::Result<()> {
    Err(std::io::Error::new(
      ErrorKind::Unsupported,
      "Junctions are not supported in an in-memory sys.",
    ))
  }
}

#[derive(Debug, Clone)]
pub struct InMemoryMetadata {
  file_type: FileType,
  len: u64,
  accessed: SystemTime,
  changed: SystemTime,
  created: SystemTime,
  modified: SystemTime,
  mode: u32,
}

macro_rules! not_supported_metadata_prop {
  ($id:ident, $type:ident) => {
    #[inline]
    fn $id(&self) -> Result<$type> {
      Err(Error::new(
        ErrorKind::Unsupported,
        concat!(stringify!($id), " is not supported on this platform"),
      ))
    }
  };
}

impl FsMetadataValue for InMemoryMetadata {
  #[inline]
  fn file_type(&self) -> FileType {
    self.file_type
  }

  #[inline]
  fn len(&self) -> u64 {
    self.len
  }

  #[inline]
  fn accessed(&self) -> Result<SystemTime> {
    Ok(self.accessed)
  }

  #[inline]
  fn changed(&self) -> Result<SystemTime> {
    Ok(self.changed)
  }

  #[inline]
  fn created(&self) -> Result<SystemTime> {
    Ok(self.created)
  }

  #[inline]
  fn modified(&self) -> Result<SystemTime> {
    Ok(self.modified)
  }

  #[inline]
  fn mode(&self) -> Result<u32> {
    Ok(self.mode)
  }

  not_supported_metadata_prop!(dev, u64);
  not_supported_metadata_prop!(ino, u64);
  not_supported_metadata_prop!(nlink, u64);
  not_supported_metadata_prop!(uid, u32);
  not_supported_metadata_prop!(gid, u32);
  not_supported_metadata_prop!(rdev, u64);
  not_supported_metadata_prop!(blksize, u64);
  not_supported_metadata_prop!(blocks, u64);
  not_supported_metadata_prop!(is_block_device, bool);
  not_supported_metadata_prop!(is_char_device, bool);
  not_supported_metadata_prop!(is_fifo, bool);
  not_supported_metadata_prop!(is_socket, bool);
  not_supported_metadata_prop!(file_attributes, u32);
}

impl BaseFsMetadata for InMemorySys {
  type Metadata = InMemoryMetadata;

  fn base_fs_metadata(&self, path: &Path) -> std::io::Result<InMemoryMetadata> {
    let inner = self.0.read();
    let path = inner.to_absolute_path(path);
    let (_, entry) = inner.lookup_entry(&path)?;
    Ok(InMemoryMetadata {
      file_type: entry.file_type(),
      len: entry.len(),
      accessed: entry.accessed(),
      changed: entry.changed(),
      created: entry.created(),
      modified: entry.modified(),
      mode: entry.mode(),
    })
  }

  fn base_fs_symlink_metadata(
    &self,
    path: &Path,
  ) -> std::io::Result<InMemoryMetadata> {
    let inner = self.0.read();
    let path = inner.to_absolute_path(path);
    let detail = inner.lookup_entry_detail_no_follow(&path)?;
    match detail {
      LookupNoFollowEntry::NotFound(path) => Err(Error::new(
        ErrorKind::NotFound,
        format!("Path not found: '{}'", path.display()),
      )),
      LookupNoFollowEntry::Symlink { entry, .. } => {
        let inner = entry.inner.read();
        Ok(InMemoryMetadata {
          file_type: FileType::Symlink,
          len: 0,
          accessed: inner.accessed,
          changed: inner.changed,
          created: inner.created,
          modified: inner.modified,
          mode: inner.mode,
        })
      }
      LookupNoFollowEntry::Found(_, entry) => Ok(InMemoryMetadata {
        file_type: entry.file_type(),
        len: entry.len(),
        accessed: entry.accessed(),
        changed: entry.changed(),
        created: entry.created(),
        modified: entry.modified(),
        mode: entry.mode(),
      }),
    }
  }
}

impl BaseFsOpen for InMemorySys {
  type File = InMemoryFile;

  fn base_fs_open(
    &self,
    path: &Path,
    options: &OpenOptions,
  ) -> std::io::Result<InMemoryFile> {
    let mut inner = self.0.write();
    let time_now = inner.time_now();
    let umask = inner.umask;
    let path = inner.to_absolute_path(path);

    // Edge case: If `parent()` is None, path might be root or invalid
    // The minimal fix is to check for that scenario
    let parent_path = match path.parent() {
      Some(p) if !p.as_os_str().is_empty() => p,
      _ => {
        return Err(Error::new(
          ErrorKind::Other,
          "Cannot open root or invalid path",
        ));
      }
    };

    let parent = inner.find_directory_mut(parent_path, false)?;
    let file_name = match path.file_name() {
      Some(n) => n.to_string_lossy(),
      None => {
        return Err(Error::new(ErrorKind::Other, "No file name found"));
      }
    };

    match parent
      .entries
      .binary_search_by(|e| e.name().cmp(&file_name))
    {
      Ok(pos) => match &mut parent.entries[pos] {
        DirectoryEntry::File(f) => {
          if options.create_new {
            return Err(Error::new(
              ErrorKind::AlreadyExists,
              "File already exists (create_new=true)",
            ));
          }
          if options.truncate {
            let mut fi = f.inner.write();
            fi.data.clear();
            fi.modified = time_now;
          }
          Ok(InMemoryFile {
            sys: self.clone(),
            inner: f.inner.clone(),
            path,
            pos: if options.append {
              f.inner.read().data.len()
            } else {
              0
            },
          })
        }
        _ => Err(Error::new(ErrorKind::Other, "Path is not a file")),
      },
      Err(insert_pos) => {
        if !options.create {
          return Err(Error::new(ErrorKind::NotFound, "File not found"));
        }
        let new_file = File {
          name: file_name.into_owned(),
          inner: Arc::new(RwLock::new(FileInner {
            accessed: time_now,
            changed: time_now,
            created: time_now,
            modified: time_now,
            data: vec![],
            mode: options.mode.unwrap_or(umask),
          })),
        };
        let result = InMemoryFile {
          sys: self.clone(),
          inner: new_file.inner.clone(),
          path,
          pos: if options.append {
            new_file.inner.read().data.len()
          } else {
            0
          },
        };
        parent
          .entries
          .insert(insert_pos, DirectoryEntry::File(new_file));
        Ok(result)
      }
    }
  }
}

impl BaseFsRead for InMemorySys {
  fn base_fs_read(&self, path: &Path) -> std::io::Result<Cow<'static, [u8]>> {
    let arc_file = self.fs_open(path, &OpenOptions::new_read())?;
    let inner = arc_file.inner.read();
    Ok(Cow::Owned(inner.data.clone()))
  }
}

impl BaseFsReadDir for InMemorySys {
  type ReadDirEntry = InMemoryDirEntry;

  fn base_fs_read_dir(
    &self,
    path: &Path,
  ) -> std::io::Result<
    Box<dyn Iterator<Item = std::io::Result<Self::ReadDirEntry>>>,
  > {
    let inner = self.0.read();
    let abs_path = inner.to_absolute_path(path);

    let (_, entry) = inner.lookup_entry(&abs_path)?;
    match entry {
      DirectoryEntry::Directory(dir) => Ok(Box::new(
        dir
          .entries
          .iter()
          .map(|entry| Ok(InMemoryDirEntry::new(path, entry)))
          .collect::<Vec<_>>()
          .into_iter(),
      )),
      _ => Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "Path is not a directory",
      )),
    }
  }
}

impl BaseFsReadLink for InMemorySys {
  fn base_fs_read_link(&self, path: &Path) -> io::Result<PathBuf> {
    let inner = self.0.read();
    let detail = inner.lookup_entry_detail_no_follow(path)?;
    match detail {
      LookupNoFollowEntry::NotFound(path) => Err(Error::new(
        ErrorKind::NotFound,
        format!("Path not found: '{}'", path.display()),
      )),
      LookupNoFollowEntry::Found { .. } => Err(Error::new(
        ErrorKind::InvalidInput,
        format!("Path is not a symlink: '{}'", path.display()),
      )),
      LookupNoFollowEntry::Symlink { target_path, .. } => Ok(target_path),
    }
  }
}

#[derive(Debug)]
pub struct InMemoryDirEntry {
  name: String,
  path: PathBuf,
  file_type: FileType,
  len: u64,
  accessed: SystemTime,
  created: SystemTime,
  changed: SystemTime,
  modified: SystemTime,
  mode: u32,
}

impl InMemoryDirEntry {
  fn new(initial_path: &Path, entry: &DirectoryEntry) -> Self {
    Self {
      name: entry.name().to_string(),
      path: initial_path.join(entry.name()),
      file_type: entry.file_type(),
      len: entry.len(),
      accessed: entry.accessed(),
      changed: entry.changed(),
      created: entry.created(),
      modified: entry.modified(),
      mode: entry.mode(),
    }
  }
}

impl FsDirEntry for InMemoryDirEntry {
  type Metadata = InMemoryMetadata;

  fn file_name(&self) -> std::borrow::Cow<'_, std::ffi::OsStr> {
    std::borrow::Cow::Owned(self.name.clone().into())
  }

  fn file_type(&self) -> std::io::Result<FileType> {
    Ok(self.file_type)
  }

  fn metadata(&self) -> std::io::Result<Self::Metadata> {
    Ok(InMemoryMetadata {
      file_type: self.file_type,
      len: self.len,
      accessed: self.accessed,
      created: self.created,
      changed: self.changed,
      modified: self.modified,
      mode: self.mode,
    })
  }

  fn path(&self) -> std::borrow::Cow<'_, std::path::Path> {
    std::borrow::Cow::Borrowed(self.path.as_ref())
  }
}

impl BaseFsRemoveDir for InMemorySys {
  fn base_fs_remove_dir(&self, path: &Path) -> io::Result<()> {
    let mut inner = self.0.write();
    let abs_path = inner.to_absolute_path(path);
    let parent_path = match abs_path.parent() {
      Some(p) if !p.as_os_str().is_empty() => p,
      _ => {
        return Err(Error::new(
          ErrorKind::Other,
          "Cannot remove root or invalid path",
        ));
      }
    };
    let parent = inner.find_directory_mut(parent_path, false)?;
    let dir_name = match abs_path.file_name() {
      Some(n) => n.to_string_lossy(),
      None => {
        return Err(Error::new(ErrorKind::Other, "Directory not found"));
      }
    };

    match parent.entries.binary_search_by(|e| e.name().cmp(&dir_name)) {
      Ok(pos) => match &parent.entries[pos] {
        DirectoryEntry::Directory(dir) => {
          if !dir.entries.is_empty() {
            return Err(Error::new(ErrorKind::Other, "Directory is not empty"));
          }
          parent.entries.remove(pos);
          Ok(())
        }
        _ => Err(Error::new(ErrorKind::Other, "Not a directory")),
      },
      Err(_) => Err(Error::new(ErrorKind::NotFound, "Directory not found")),
    }
  }
}

impl BaseFsRemoveDirAll for InMemorySys {
  fn base_fs_remove_dir_all(&self, path: &Path) -> io::Result<()> {
    let mut inner = self.0.write();
    let abs_path = inner.to_absolute_path(path);
    let parent_path = match abs_path.parent() {
      Some(p) if !p.as_os_str().is_empty() => p,
      _ => {
        return Err(Error::new(
          ErrorKind::Other,
          "Cannot remove root or invalid path",
        ));
      }
    };
    let parent = inner.find_directory_mut(parent_path, false)?;
    let dir_name = match abs_path.file_name() {
      Some(n) => n.to_string_lossy(),
      None => {
        return Ok(()); // previously deleted
      }
    };

    match parent.entries.binary_search_by(|e| e.name().cmp(&dir_name)) {
      Ok(pos) => match &parent.entries[pos] {
        DirectoryEntry::Directory(_) => {
          parent.entries.remove(pos);
          Ok(())
        }
        _ => Err(Error::new(ErrorKind::Other, "Not a directory")),
      },
      Err(_) => Ok(()), // previously deleted
    }
  }
}

impl BaseFsRemoveFile for InMemorySys {
  fn base_fs_remove_file(&self, path: &Path) -> std::io::Result<()> {
    let mut inner = self.0.write();
    let path = inner.to_absolute_path(path);
    let parent_path = match path.parent() {
      Some(p) if !p.as_os_str().is_empty() => p,
      _ => {
        return Err(Error::new(
          ErrorKind::Other,
          "Cannot remove root or invalid path",
        ));
      }
    };
    let parent = inner.find_directory_mut(parent_path, false)?;
    let file_name = match path.file_name() {
      Some(n) => n.to_string_lossy(),
      None => {
        return Err(Error::new(ErrorKind::Other, "No file name found"));
      }
    };

    match parent
      .entries
      .binary_search_by(|e| e.name().cmp(&file_name))
    {
      Ok(pos) => match &parent.entries[pos] {
        DirectoryEntry::File(_) => {
          parent.entries.remove(pos);
          Ok(())
        }
        _ => Err(Error::new(ErrorKind::Other, "Not a file")),
      },
      Err(_) => Err(Error::new(ErrorKind::NotFound, "File not found")),
    }
  }
}

impl BaseFsRename for InMemorySys {
  fn base_fs_rename(&self, from: &Path, to: &Path) -> std::io::Result<()> {
    let mut inner = self.0.write();
    let from = inner.to_absolute_path(from.as_ref());
    let to = inner.to_absolute_path(to.as_ref());

    if from == to {
      return Ok(());
    }

    let from_parent_path = match from.parent() {
      Some(p) if !p.as_os_str().is_empty() => p,
      _ => {
        return Err(Error::new(
          ErrorKind::Other,
          "Cannot rename root or invalid path",
        ));
      }
    };
    let from_file_name = match from.file_name() {
      Some(n) => n.to_string_lossy(),
      None => {
        return Err(Error::new(ErrorKind::Other, "No source file name found"));
      }
    };
    let to_parent_path = match to.parent() {
      Some(p) if !p.as_os_str().is_empty() => p,
      _ => {
        return Err(Error::new(
          ErrorKind::Other,
          "Cannot rename to root or invalid path",
        ));
      }
    };
    let to_file_name = match to.file_name() {
      Some(n) => n.to_string_lossy(),
      None => {
        return Err(Error::new(
          ErrorKind::Other,
          "No destination file name found",
        ));
      }
    };

    // gather source/destination kinds up front so we can validate without
    // having to roll back a partial mutation
    let source_is_dir = match inner.lookup_entry_detail(&from)? {
      LookupEntry::Found(_, entry) => {
        matches!(entry, DirectoryEntry::Directory(_))
      }
      LookupEntry::NotFound(_) => {
        return Err(Error::new(ErrorKind::NotFound, "Source not found"));
      }
    };

    // prevent moving a directory into itself or one of its descendants
    if source_is_dir && to.starts_with(&from) {
      return Err(Error::new(
        ErrorKind::Other,
        "Cannot rename a directory into itself or a subdirectory",
      ));
    }

    let dest_state = match inner.lookup_entry_detail(&to)? {
      LookupEntry::Found(_, entry) => {
        let is_dir = matches!(entry, DirectoryEntry::Directory(_));
        let dir_empty = match entry {
          DirectoryEntry::Directory(d) => d.entries.is_empty(),
          _ => false,
        };
        Some((is_dir, dir_empty))
      }
      LookupEntry::NotFound(_) => None,
    };

    if let Some((dest_is_dir, dest_dir_empty)) = dest_state {
      if source_is_dir && !dest_is_dir {
        return Err(Error::new(
          ErrorKind::Other,
          "Cannot overwrite a non-directory with a directory",
        ));
      }
      if !source_is_dir && dest_is_dir {
        return Err(Error::new(
          ErrorKind::Other,
          "Cannot overwrite a directory with a non-directory",
        ));
      }
      if dest_is_dir && !dest_dir_empty {
        return Err(Error::new(
          ErrorKind::AlreadyExists,
          "Destination directory is not empty",
        ));
      }
    }

    // remove the source entry
    let from_parent = inner.find_directory_mut(from_parent_path, false)?;
    let from_idx = match from_parent
      .entries
      .binary_search_by(|e| e.name().cmp(&from_file_name))
    {
      Ok(pos) => pos,
      Err(_) => {
        return Err(Error::new(ErrorKind::NotFound, "Source not found"));
      }
    };
    let mut entry = from_parent.entries.remove(from_idx);

    // insert into the destination parent, replacing any existing entry
    let to_parent = inner.find_directory_mut(to_parent_path, true)?;
    let pos = to_parent
      .entries
      .binary_search_by(|e| e.name().cmp(&to_file_name));
    set_entry_name(&mut entry, to_file_name.into_owned());
    match pos {
      Ok(pos) => {
        to_parent.entries[pos] = entry;
      }
      Err(insert_pos) => {
        to_parent.entries.insert(insert_pos, entry);
      }
    }

    Ok(())
  }
}

fn set_entry_name(entry: &mut DirectoryEntry, name: String) {
  match entry {
    DirectoryEntry::File(f) => f.name = name,
    DirectoryEntry::Directory(d) => d.name = name,
    DirectoryEntry::Symlink(s) => s.name = name,
  }
}

impl BaseFsSetFileTimes for InMemorySys {
  fn base_fs_set_file_times(
    &self,
    path: &Path,
    atime: SystemTime,
    mtime: SystemTime,
  ) -> io::Result<()> {
    let inner = self.0.read();
    let entry = inner.lookup_entry_detail(path)?;
    match entry {
      LookupEntry::NotFound(path_buf) => Err(Error::new(
        ErrorKind::NotFound,
        format!("Path not found: '{}'", path_buf.display()),
      )),
      LookupEntry::Found(_, directory_entry) => {
        directory_entry.set_filetimes(atime, mtime);
        Ok(())
      }
    }
  }
}

impl BaseFsSetSymlinkFileTimes for InMemorySys {
  fn base_fs_set_symlink_file_times(
    &self,
    path: &Path,
    atime: SystemTime,
    mtime: SystemTime,
  ) -> io::Result<()> {
    let inner = self.0.read();
    let entry = inner.lookup_entry_detail_no_follow(path)?;
    match entry {
      LookupNoFollowEntry::Symlink { entry, .. } => {
        let mut inner = entry.inner.write();
        inner.accessed = atime;
        inner.changed = atime;
        inner.created = atime;
        inner.modified = mtime;
        Ok(())
      }
      LookupNoFollowEntry::NotFound(path) => Err(Error::new(
        ErrorKind::NotFound,
        format!("Path not found: '{}'", path.display()),
      )),
      LookupNoFollowEntry::Found(_, directory_entry) => {
        directory_entry.set_filetimes(atime, mtime);
        Ok(())
      }
    }
  }
}

impl BaseFsSetPermissions for InMemorySys {
  fn base_fs_set_permissions(
    &self,
    path: &Path,
    mode: u32,
  ) -> std::io::Result<()> {
    let inner = self.0.read();
    let path = inner.to_absolute_path(path);
    let (_, entry) = inner.lookup_entry(&path)?;

    match entry {
      DirectoryEntry::File(f) => {
        let mut inner = f.inner.write();
        inner.mode = mode;
      }
      DirectoryEntry::Directory(d) => {
        let mut inner = d.inner.write();
        inner.mode = mode;
      }
      DirectoryEntry::Symlink(s) => {
        let mut inner = s.inner.write();
        inner.mode = mode;
      }
    }
    Ok(())
  }
}

impl BaseFsSymlinkDir for InMemorySys {
  fn base_fs_symlink_dir(
    &self,
    original: &Path,
    link: &Path,
  ) -> std::io::Result<()> {
    self.base_fs_symlink_file(original, link)
  }
}

impl BaseFsSymlinkFile for InMemorySys {
  fn base_fs_symlink_file(
    &self,
    original: &Path,
    link: &Path,
  ) -> std::io::Result<()> {
    let mut inner = self.0.write();
    let time = inner.time_now();
    let link = inner.to_absolute_path(link.as_ref());
    let parent = inner.find_directory_mut(link.parent().unwrap(), false)?;
    let file_name = link.file_name().unwrap().to_string_lossy();
    match parent
      .entries
      .binary_search_by(|e| e.name().cmp(&file_name))
    {
      Ok(overwrite_pos) => {
        match &parent.entries[overwrite_pos] {
          DirectoryEntry::Directory(directory) => {
            return Err(Error::new(
              ErrorKind::AlreadyExists,
              format!("Directory already exists: '{}'", directory.name),
            ));
          }
          DirectoryEntry::File(_) | DirectoryEntry::Symlink(_) => {
            // do nothing
          }
        }

        parent.entries[overwrite_pos] = DirectoryEntry::Symlink(Symlink {
          name: file_name.into_owned(),
          target: original.to_path_buf(),
          inner: RwLock::new(SymlinkInner {
            accessed: time,
            changed: time,
            created: time,
            modified: time,
            mode: 0o777,
          }),
        });
        Ok(())
      }
      Err(insert_index) => {
        parent.entries.insert(
          insert_index,
          DirectoryEntry::Symlink(Symlink {
            name: file_name.into_owned(),
            target: original.to_path_buf(),
            inner: RwLock::new(SymlinkInner {
              accessed: time,
              changed: time,
              created: time,
              modified: time,
              mode: 0o777,
            }),
          }),
        );
        Ok(())
      }
    }
  }
}

impl BaseFsWrite for InMemorySys {
  fn base_fs_write(&self, path: &Path, data: &[u8]) -> std::io::Result<()> {
    let opts = OpenOptions {
      write: true,
      create: true,
      truncate: true,
      append: false,
      read: false,
      create_new: false,
      ..Default::default()
    };
    let time_now = self.sys_time_now();
    let file = self.fs_open(path, &opts)?;
    let mut inner = file.inner.write();
    inner.data.clear();
    inner.data.extend_from_slice(data.as_ref());
    inner.modified = time_now;
    Ok(())
  }
}

// File System File

impl FsFileAsRaw for InMemoryFile {
  #[cfg(windows)]
  #[inline]
  fn fs_file_as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> {
    None
  }

  #[cfg(unix)]
  #[inline]
  fn fs_file_as_raw_fd(&self) -> Option<std::os::fd::RawFd> {
    None
  }
}

impl FsFileLock for InMemoryFile {
  #[inline]
  fn fs_file_lock(&mut self, _mode: FsFileLockMode) -> io::Result<()> {
    Ok(())
  }
  #[inline]
  fn fs_file_try_lock(&mut self, _mode: FsFileLockMode) -> io::Result<()> {
    Ok(())
  }
  #[inline]
  fn fs_file_unlock(&mut self) -> io::Result<()> {
    Ok(())
  }
}

impl FsFileIsTerminal for InMemoryFile {
  #[inline]
  fn fs_file_is_terminal(&self) -> bool {
    false
  }
}

impl FsFileSetLen for InMemoryFile {
  fn fs_file_set_len(&mut self, size: u64) -> std::io::Result<()> {
    let mut inner = self.inner.write();
    inner.data.resize(size as usize, 0);
    Ok(())
  }
}

impl FsFileMetadata for InMemoryFile {
  fn fs_file_metadata(&self) -> std::io::Result<BoxedFsMetadataValue> {
    self
      .sys
      .base_fs_metadata(&self.path)
      .map(BoxedFsMetadataValue::new)
  }
}

impl FsFileSetPermissions for InMemoryFile {
  fn fs_file_set_permissions(&mut self, mode: u32) -> std::io::Result<()> {
    let mut inner = self.inner.write();
    inner.mode = mode;
    Ok(())
  }
}

impl FsFileSetTimes for InMemoryFile {
  fn fs_file_set_times(&mut self, times: FsFileTimes) -> std::io::Result<()> {
    let mut inner = self.inner.write();
    if let Some(accessed) = times.accessed {
      inner.accessed = accessed;
    }
    if let Some(modified) = times.modified {
      inner.modified = modified;
    }
    Ok(())
  }
}

impl FsFileSyncAll for InMemoryFile {
  #[inline]
  fn fs_file_sync_all(&mut self) -> io::Result<()> {
    Ok(())
  }
}

impl FsFileSyncData for InMemoryFile {
  #[inline]
  fn fs_file_sync_data(&mut self) -> io::Result<()> {
    Ok(())
  }
}

impl std::io::Seek for InMemoryFile {
  fn seek(&mut self, pos: std::io::SeekFrom) -> Result<u64> {
    match pos {
      std::io::SeekFrom::Start(n) => {
        self.pos = n as usize;
      }
      std::io::SeekFrom::End(n) => {
        let inner = self.inner.read();
        if -n > inner.data.len() as i64 {
          return Err(Error::new(
            ErrorKind::InvalidInput,
            "Seeking before start of file",
          ));
        }
        self.pos = (inner.data.len() as i64 + n) as usize;
      }
      std::io::SeekFrom::Current(n) => {
        self.pos = self.pos.wrapping_add(n as usize);
      }
    }
    Ok(self.pos as u64)
  }
}

impl std::io::Write for InMemoryFile {
  fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
    let time = self.sys.sys_time_now();
    let mut inner = self.inner.write();
    if self.pos > inner.data.len() {
      inner.data.resize(self.pos, 0);
    }
    inner.data.splice(self.pos.., buf.as_ref().iter().cloned());
    inner.modified = time;
    self.pos += buf.as_ref().len();
    Ok(buf.len())
  }

  fn flush(&mut self) -> std::io::Result<()> {
    Ok(())
  }
}

impl std::io::Read for InMemoryFile {
  fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
    let inner = self.inner.read();
    if self.pos > inner.data.len() {
      return Ok(0);
    }
    let data = &inner.data[self.pos..];
    let len = std::cmp::min(data.len(), buf.len());
    buf[..len].copy_from_slice(&data[..len]);
    self.pos += len;
    Ok(len)
  }
}

// System

impl SystemTimeNow for InMemorySys {
  fn sys_time_now(&self) -> SystemTime {
    self.0.read().time_now()
  }
}

impl SystemRandom for InMemorySys {
  fn sys_random(&self, buf: &mut [u8]) -> std::io::Result<()> {
    fn random_with_seed(seed: u64, buf: &mut [u8]) {
      // not the best, but good enough for now
      let mut state = seed;
      for byte in buf.iter_mut() {
        // simple linear congruential generator
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        *byte = (state >> 24) as u8; // use the top 8 bits
      }
    }

    match self.0.read().random_seed {
      Some(seed) => {
        random_with_seed(seed, buf);
        Ok(())
      }
      None => {
        #[cfg(feature = "getrandom")]
        {
          getrandom::getrandom(buf)
            .map_err(|err| Error::new(ErrorKind::Other, err.to_string()))
        }
        #[cfg(not(feature = "getrandom"))]
        {
          random_with_seed(0, buf);
          Ok(())
        }
      }
    }
  }
}

impl ProcessExit for InMemorySys {
  fn process_exit(&self, code: i32) -> ! {
    panic!("process exited with code {code}");
  }
}

impl ThreadSleep for InMemorySys {
  fn thread_sleep(&self, dur: std::time::Duration) {
    if self.0.read().thread_sleep_enabled {
      std::thread::sleep(dur);
    }
  }
}

/// Normalize all intermediate components of the path (ie. remove "./" and "../" components).
/// Similar to `fs::canonicalize()` but doesn't resolve symlinks.
///
/// Taken from Cargo
/// <https://github.com/rust-lang/cargo/blob/af307a38c20a753ec60f0ad18be5abed3db3c9ac/src/cargo/util/paths.rs#L60-L85>
#[inline]
fn normalize_path(path: &Path) -> PathBuf {
  let mut components = path.components().peekable();
  let mut ret =
    if let Some(c @ Component::Prefix(..)) = components.peek().cloned() {
      components.next();
      PathBuf::from(c.as_os_str())
    } else {
      PathBuf::new()
    };

  for component in components {
    match component {
      Component::Prefix(..) => unreachable!(),
      Component::RootDir => {
        ret.push(component.as_os_str());
      }
      Component::CurDir => {}
      Component::ParentDir => {
        ret.pop();
      }
      Component::Normal(c) => {
        ret.push(c);
      }
    }
  }
  ret
}

// most of these tests were lazily created with ChatGPT
#[cfg(test)]
mod tests {
  use super::*;
  use std::io::Seek;
  use std::io::Write;
  use std::path::Path;
  use std::time::Duration;
  use std::time::SystemTime;

  #[test]
  fn test_env_vars() {
    let sys = InMemorySys::default();
    sys.env_set_var("VALUE", "other");
    assert_eq!(sys.env_var_os("VALUE"), Some("other".into()));
  }

  #[test]
  fn test_create_dir_all() {
    let sys = InMemorySys::default();
    let dir_path = Path::new("/rootDir/subDir");
    assert!(!sys.fs_exists(dir_path).unwrap());
    sys.fs_create_dir_all(dir_path).unwrap();
    assert!(sys.fs_exists(dir_path).unwrap());
    assert!(sys.fs_is_dir(dir_path).unwrap());
  }

  #[test]
  fn test_write_read_file() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/rootDir").unwrap();

    let file_path = "/rootDir/test.txt";
    sys.fs_write(file_path, b"Hello World!").unwrap();
    assert!(sys.fs_exists(file_path).unwrap());
    assert!(sys.fs_is_file(file_path).unwrap());

    let contents = sys.fs_read_to_string(file_path).unwrap();
    assert_eq!(&*contents, "Hello World!");
  }

  #[test]
  fn test_truncate_existing_file() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/rootDir").unwrap();

    let file_path = "/rootDir/data.bin";
    sys.fs_write(file_path, b"abcdef").unwrap();

    let opts = OpenOptions {
      write: true,
      truncate: true,
      ..Default::default()
    };
    let file = sys.fs_open(file_path, &opts).unwrap();
    // file is truncated at open, so should be empty
    let guard = file.inner.read();
    assert!(guard.data.is_empty());
  }

  #[test]
  fn test_rename_file() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/rootDir").unwrap();

    let old_path = "/rootDir/old.txt";
    let new_path = "/rootDir/new.txt";
    sys.fs_write(old_path, b"some data").unwrap();

    sys.fs_rename(old_path, new_path).unwrap();
    assert!(!sys.fs_exists(old_path).unwrap());
    assert!(sys.fs_exists(new_path).unwrap());

    let data = sys.fs_read_to_string(new_path).unwrap();
    assert_eq!(&*data, "some data");
  }

  #[test]
  fn test_remove_file() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/rootDir").unwrap();

    let file_path = "/rootDir/remove_me.txt";
    sys.fs_write(file_path, b"Bye!").unwrap();
    assert!(sys.fs_exists(file_path).unwrap());

    sys.fs_remove_file(file_path).unwrap();
    assert!(!sys.fs_exists(file_path).unwrap());
  }

  #[test]
  fn test_modified_time() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/rootDir").unwrap();

    let file_path = "/rootDir/hello.txt";
    sys.fs_write(file_path, b"Hi!").unwrap();

    // First check if we can get a valid modified time
    let modified = sys.fs_metadata(file_path).unwrap().modified;

    // Since we can't easily freeze or manipulate real time,
    // we'll just assert it's no earlier than the current system time minus some buffer.
    let now = SystemTime::now();
    let duration = now.duration_since(modified);
    assert!(duration.is_ok());

    let opened_file = sys.fs_open(file_path, &OpenOptions::new_read()).unwrap();
    assert_eq!(
      opened_file.fs_file_metadata().unwrap().modified().unwrap(),
      modified
    );
  }

  #[test]
  fn test_exists_no_err() {
    let sys = InMemorySys::default();
    assert!(!sys.fs_exists_no_err("/does/not/exist"));
    sys.fs_create_dir_all("/exists").unwrap();
    assert!(sys.fs_exists_no_err("/exists"));
  }

  #[test]
  fn test_is_file_no_err() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/dir").unwrap();
    sys.fs_write("/dir/file.txt", b"contents").unwrap();
    assert!(!sys.fs_is_file_no_err("/no/file"));
    assert!(!sys.fs_is_file_no_err("/dir"));
    assert!(sys.fs_is_file_no_err("/dir/file.txt"));
  }

  #[test]
  fn test_is_dir_no_err() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/dir").unwrap();
    sys.fs_write("/dir/file.txt", b"contents").unwrap();
    assert!(!sys.fs_is_dir_no_err("/no/dir"));
    assert!(sys.fs_is_dir_no_err("/dir"));
    assert!(!sys.fs_is_dir_no_err("/dir/file.txt"));
  }

  #[test]
  fn test_file_permissions() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/dir").unwrap();

    let file_path = "/dir/perm_test.txt";
    sys.fs_write(file_path, b"Testing perms").unwrap();
    let mut file = sys.fs_open(file_path, &OpenOptions::new_read()).unwrap();
    file.fs_file_set_permissions(0o755).unwrap();

    let guard = file.inner.read();
    assert_eq!(guard.mode, 0o755);
  }

  #[test]
  fn test_file_append() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/dir").unwrap();

    let file_path = "/dir/append_test.txt";
    let mut opts = OpenOptions {
      write: true,
      create: true,
      ..Default::default()
    };
    // Not truncate
    sys.fs_open(file_path, &opts).unwrap(); // creates empty file
                                            // Now open again with append
    opts.append = true;
    let mut file = sys.fs_open(file_path, &opts).unwrap();
    // Should start at position 0 in the code, but let's test manually
    _ = file.write(b"Appending ").unwrap();
    _ = file.write(b"more data").unwrap();

    let contents = sys.fs_read_to_string(file_path).unwrap();
    assert_eq!(&*contents, "Appending more data");
  }

  #[test]
  fn test_create_dir_that_already_exists() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/already/exists").unwrap();
    let result = sys.fs_create_dir_all("/already/exists");
    assert!(
      result.is_ok(),
      "Creating a directory that already exists should succeed"
    );
  }

  #[test]
  fn test_remove_non_empty_directory_fails() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/dir").unwrap();
    sys.fs_write("/dir/file.txt", b"data").unwrap();
    let result = sys.fs_remove_file("/dir");
    assert!(
            result.is_err(),
            "Removing a non-empty directory (treated as a directory, not a file) should fail"
        );
  }

  #[test]
  fn test_fs_canonicalize_relative_path() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/test/relative").unwrap();
    {
      let mut inner = sys.0.write();
      inner.cwd = PathBuf::from("/test");
    }
    let abs = sys.fs_canonicalize("relative").unwrap();
    assert_eq!(abs, PathBuf::from("/test/relative"));
  }

  #[test]
  fn test_fs_canonicalize_absolute_path() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/absolute").unwrap();
    let abs = sys.fs_canonicalize("/absolute").unwrap();
    assert_eq!(abs, PathBuf::from("/absolute"));
  }

  #[test]
  fn test_fs_canonicalize_empty() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/a/b/c").unwrap();
    sys.env_set_current_dir("/a/b").unwrap();
    let result = sys.fs_canonicalize("");
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::NotFound);
  }

  #[test]
  fn test_sys_random_no_seed() {
    let sys = InMemorySys::default();
    let mut buf1 = [0u8; 8];
    let mut buf2 = [0u8; 8];
    sys.sys_random(&mut buf1).unwrap();
    sys.sys_random(&mut buf2).unwrap();
    // There's no guarantee on exact values without a seed, but it should succeed
    assert_ne!(buf1, [0u8; 8]);
    assert_ne!(buf2, [0u8; 8]);
  }

  #[test]
  fn test_thread_sleep_no_op() {
    let sys = InMemorySys::default();
    sys.disable_thread_sleep();
    let start = SystemTime::now();
    sys.thread_sleep(Duration::from_secs(1));
    // Should be effectively no-op, so the elapsed time should be tiny
    let elapsed = start.elapsed().unwrap();
    assert!(
      elapsed < Duration::from_millis(100),
      "Sleep should be disabled"
    );
  }

  #[test]
  fn test_rename_file_to_existing_file() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/dir").unwrap();
    sys.fs_write("/dir/file1.txt", b"111").unwrap();
    sys.fs_write("/dir/file2.txt", b"222").unwrap();
    let result = sys.fs_rename("/dir/file1.txt", "/dir/file2.txt");
    assert!(result.is_ok() || result.is_err());
    let file1_exists = sys.fs_exists_no_err("/dir/file1.txt");
    let file2_exists = sys.fs_exists_no_err("/dir/file2.txt");
    assert!(!file1_exists && file2_exists);
  }

  #[test]
  fn test_rename_directory() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/a/old").unwrap();
    sys.fs_write("/a/old/inner.txt", b"hi").unwrap();
    sys.fs_create_dir_all("/a/old/nested").unwrap();
    sys.fs_write("/a/old/nested/deep.txt", b"deep").unwrap();

    sys.fs_rename("/a/old", "/a/new").unwrap();

    assert!(!sys.fs_exists_no_err("/a/old"));
    assert!(sys.fs_is_dir_no_err("/a/new"));
    assert_eq!(&*sys.fs_read_to_string("/a/new/inner.txt").unwrap(), "hi");
    assert_eq!(
      &*sys.fs_read_to_string("/a/new/nested/deep.txt").unwrap(),
      "deep"
    );
  }

  #[test]
  fn test_rename_directory_across_parents() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/src/inner").unwrap();
    sys.fs_write("/src/inner/x.txt", b"x").unwrap();
    sys.fs_create_dir_all("/dst").unwrap();

    sys.fs_rename("/src/inner", "/dst/inner").unwrap();

    assert!(!sys.fs_exists_no_err("/src/inner"));
    assert!(sys.fs_is_dir_no_err("/dst/inner"));
    assert_eq!(&*sys.fs_read_to_string("/dst/inner/x.txt").unwrap(), "x");
  }

  #[test]
  fn test_rename_directory_onto_empty_directory() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/from").unwrap();
    sys.fs_write("/from/keep.txt", b"keep").unwrap();
    sys.fs_create_dir_all("/to").unwrap();

    sys.fs_rename("/from", "/to").unwrap();

    assert!(!sys.fs_exists_no_err("/from"));
    assert_eq!(&*sys.fs_read_to_string("/to/keep.txt").unwrap(), "keep");
  }

  #[test]
  fn test_rename_directory_onto_non_empty_directory_fails() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/from").unwrap();
    sys.fs_create_dir_all("/to").unwrap();
    sys.fs_write("/to/keeper.txt", b"keep").unwrap();

    let err = sys.fs_rename("/from", "/to").unwrap_err();
    assert_eq!(err.kind(), ErrorKind::AlreadyExists);
    assert!(sys.fs_is_dir_no_err("/from"));
    assert!(sys.fs_exists_no_err("/to/keeper.txt"));
  }

  #[test]
  fn test_rename_directory_into_itself_fails() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/a").unwrap();
    let err = sys.fs_rename("/a", "/a/b").unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Other);
    assert!(sys.fs_is_dir_no_err("/a"));
  }

  #[test]
  fn test_rename_directory_onto_file_fails() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/dir").unwrap();
    sys.fs_write("/file", b"data").unwrap();
    let err = sys.fs_rename("/dir", "/file").unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Other);
    assert!(sys.fs_is_dir_no_err("/dir"));
    assert!(sys.fs_is_file_no_err("/file"));
  }

  #[test]
  fn test_rename_file_onto_directory_fails() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/dir").unwrap();
    sys.fs_write("/file", b"data").unwrap();
    let err = sys.fs_rename("/file", "/dir").unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Other);
    assert!(sys.fs_is_file_no_err("/file"));
    assert!(sys.fs_is_dir_no_err("/dir"));
  }

  #[test]
  fn test_fs_write_into_non_existent_subdir_fails() {
    let sys = InMemorySys::default();
    let result = sys.fs_write("/no-such-subdir/myfile.txt", b"content");
    assert!(
      result.is_err(),
      "Should fail because /no-such-subdir does not exist"
    );
  }

  #[test]
  fn test_fs_read_dir_with_files() {
    let sys = InMemorySys::default();
    let root_dir = "/test";

    // Setup directories and files
    sys.fs_create_dir_all(root_dir).unwrap();
    sys
      .fs_write(format!("{}/file1.txt", root_dir), b"Content 1")
      .unwrap();
    sys
      .fs_write(format!("{}/file2.txt", root_dir), b"Content 2")
      .unwrap();

    // Read directory
    let entries: Vec<_> = sys
      .fs_read_dir(root_dir)
      .unwrap()
      .map(|res| res.unwrap().file_name().to_string_lossy().to_string())
      .collect();

    assert_eq!(entries.len(), 2);
    assert!(entries.contains(&"file1.txt".to_string()));
    assert!(entries.contains(&"file2.txt".to_string()));
  }

  #[test]
  fn test_fs_read_dir_with_subdirectories() {
    let sys = InMemorySys::default();
    let root_dir = "/test";

    // Setup directories and files
    sys
      .fs_create_dir_all(format!("{}/subdir", root_dir))
      .unwrap();
    sys
      .fs_write(format!("{}/subdir/file.txt", root_dir), b"Content")
      .unwrap();

    // Read root directory
    let entries: Vec<_> = sys
      .fs_read_dir(root_dir)
      .unwrap()
      .map(|res| res.unwrap().file_name().to_string_lossy().to_string())
      .collect();

    assert_eq!(entries.len(), 1);
    assert!(entries.contains(&"subdir".to_string()));
  }

  #[test]
  fn test_fs_read_dir_not_a_directory() {
    let sys = InMemorySys::default();
    let file_path = "/file.txt";

    // Create a file
    sys.fs_create_dir_all("/").unwrap();
    sys.fs_write(file_path, b"Content").unwrap();

    // Attempt to read as directory
    let result = sys.fs_read_dir(file_path);
    assert!(result.is_err());
    match result {
      Err(err) => {
        assert_eq!(err.kind(), std::io::ErrorKind::Other);
      }
      _ => panic!("Expected an error"),
    }
  }

  #[test]
  fn test_fs_read_dir_empty_directory() {
    let sys = InMemorySys::default();
    let empty_dir = "/empty";
    sys.fs_create_dir_all(empty_dir).unwrap();

    let entries: Vec<_> = sys
      .fs_read_dir(empty_dir)
      .unwrap()
      .map(|res| res.unwrap().file_name().to_string_lossy().to_string())
      .collect();

    assert!(entries.is_empty());
  }

  #[test]
  fn test_fs_exists() {
    let sys = InMemorySys::default();
    sys.fs_insert("hello.txt", "hello\n");
    assert!(sys.fs_exists("hello.txt").unwrap());
  }

  #[test]
  fn test_hard_link_sync() {
    let sys = InMemorySys::default();
    let empty_dir = "/empty";
    sys.fs_create_dir_all(empty_dir).unwrap();

    sys.fs_write("/empty/file.txt", b"Content").unwrap();
    sys
      .fs_hard_link("/empty/file.txt", "/empty/file2.txt")
      .unwrap();
    assert_eq!(
      sys.fs_read("/empty/file2.txt").unwrap().as_ref(),
      b"Content"
    );
  }

  #[test]
  fn test_seek_start() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/test").unwrap();
    let file_path = "/test/seek.txt";
    sys.fs_write(file_path, b"abcdef").unwrap();

    let mut file = sys.fs_open(file_path, &OpenOptions::new_write()).unwrap();

    // Seek to the start of the file
    let new_pos = file.seek(std::io::SeekFrom::Start(0)).unwrap();
    assert_eq!(new_pos, 0);
    assert_eq!(file.pos, 0);
  }

  #[test]
  fn test_seek_end() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/test").unwrap();
    let file_path = "/test/seek.txt";
    sys.fs_write(file_path, b"abcdef").unwrap();

    let mut file = sys.fs_open(file_path, &OpenOptions::new_read()).unwrap();

    // Seek to the end of the file
    let new_pos = file.seek(std::io::SeekFrom::End(0)).unwrap();
    assert_eq!(new_pos, 6);
    assert_eq!(file.pos, 6);

    // Seek 2 bytes before the end
    let new_pos = file.seek(std::io::SeekFrom::End(-2)).unwrap();
    assert_eq!(new_pos, 4);
    assert_eq!(file.pos, 4);
  }

  #[test]
  fn test_seek_current() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/test").unwrap();
    let file_path = "/test/seek.txt";
    sys.fs_write(file_path, b"abcdef").unwrap();

    let mut file = sys.fs_open(file_path, &OpenOptions::new_write()).unwrap();

    // Seek 2 bytes forward from the start
    let new_pos = file.seek(std::io::SeekFrom::Current(2)).unwrap();
    assert_eq!(new_pos, 2);
    assert_eq!(file.pos, 2);

    // Seek 1 byte backward from the current position
    let new_pos = file.seek(std::io::SeekFrom::Current(-1)).unwrap();
    assert_eq!(new_pos, 1);
    assert_eq!(file.pos, 1);
  }

  #[test]
  fn test_seek_before_start_fails() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/test").unwrap();
    let file_path = "/test/seek.txt";
    sys.fs_write(file_path, b"abcdef").unwrap();

    let mut file = sys.fs_open(file_path, &OpenOptions::new_write()).unwrap();

    // Attempt to seek before the start of the file
    let result = file.seek(std::io::SeekFrom::End(-1000));
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
  }

  #[test]
  fn test_seek_write_position() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/test").unwrap();
    let file_path = "/test/seek_write.txt";
    sys.fs_write(file_path, b"abcdef").unwrap();

    let mut file = sys
      .fs_open(
        file_path,
        &OpenOptions {
          truncate: false,
          write: false,
          ..Default::default()
        },
      )
      .unwrap();

    // Seek to position 3 and write data
    file.seek(std::io::SeekFrom::Start(3)).unwrap();
    file.write_all(b"XYZ").unwrap();
    // Seek then write past the end
    file.seek(std::io::SeekFrom::End(2)).unwrap();
    file.write_all(b"a").unwrap();

    let contents = sys.fs_read_to_string(file_path).unwrap();
    assert_eq!(&*contents, "abcXYZ\0\0a");
  }

  #[test]
  fn test_temp_dir() {
    let sys = InMemorySys::default();
    assert!(sys.env_temp_dir().is_err());
    sys.fs_create_dir_all("/test").unwrap();
    assert_eq!(sys.env_temp_dir().unwrap(), PathBuf::from("/tmp"));
  }

  #[test]
  fn test_remove_dir() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/test/test").unwrap();
    assert!(sys.fs_remove_dir("/test").is_err());
    assert!(sys.fs_remove_dir("/test/test").is_ok());
    sys.fs_create_dir_all("/test/test").unwrap();
    assert!(sys.fs_remove_dir_all("/test").is_ok());
    assert!(!sys.fs_exists_no_err("/test"));
  }

  #[test]
  fn test_new_with_cwd() {
    let cwd = if cfg!(windows) { "C:\\dir" } else { "/dir" };

    let sys = InMemorySys::new_with_cwd(cwd);
    assert_eq!(sys.env_current_dir().unwrap(), PathBuf::from(cwd));
    assert!(sys.fs_is_dir(cwd).unwrap());
  }

  #[test]
  fn test_set_permissions_all_types() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/test").unwrap();

    // Test file permissions
    sys.fs_write("/test/file.txt", b"content").unwrap();
    sys.fs_set_permissions("/test/file.txt", 0o600).unwrap();
    let metadata = sys.fs_metadata("/test/file.txt").unwrap();
    assert_eq!(metadata.mode().unwrap(), 0o600);

    // Test directory permissions
    sys.fs_set_permissions("/test", 0o700).unwrap();
    let dir_metadata = sys.fs_metadata("/test").unwrap();
    assert_eq!(dir_metadata.mode().unwrap(), 0o700);

    // Test symlink permissions
    sys
      .fs_symlink_file("/test/file.txt", "/test/link.txt")
      .unwrap();
    sys.fs_set_permissions("/test/link.txt", 0o755).unwrap();
    // This follows the symlink, so it changes the target file's mode
    let file_metadata = sys.fs_metadata("/test/file.txt").unwrap();
    assert_eq!(file_metadata.mode().unwrap(), 0o755);
  }
}
