use std::borrow::Cow;
use std::io::Error;
use std::io::ErrorKind;
use std::io::Result;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use super::wasm_path_to_str;
use super::wasm_string_to_path;
use super::RealSys;
use crate::*;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;

#[wasm_bindgen(module = "node:fs")]
extern "C" {
  #[wasm_bindgen(js_name = Stats)]
  #[derive(Debug, Clone)]
  type Stats;
  #[wasm_bindgen(method, js_name = "isBlockDevice")]
  fn is_block_device(this: &Stats) -> bool;
  #[wasm_bindgen(method, js_name = "isCharacterDevice")]
  fn is_character_device(this: &Stats) -> bool;
  #[wasm_bindgen(method, js_name = "isDirectory")]
  fn is_directory(this: &Stats) -> bool;
  #[wasm_bindgen(method, js_name = "isFile")]
  fn is_file(this: &Stats) -> bool;
  #[wasm_bindgen(method, js_name = "isSocket")]
  fn is_socket(this: &Stats) -> bool;
  #[wasm_bindgen(method, js_name = "isSymbolicLink")]
  fn is_symbolic_link(this: &Stats) -> bool;
  #[wasm_bindgen(method, js_name = "isFIFO")]
  fn is_fifo(this: &Stats) -> bool;
  #[wasm_bindgen(js_name = chmodSync, catch)]
  fn node_chmod_sync(path: &str, mode: u32)
    -> std::result::Result<(), JsValue>;
  #[wasm_bindgen(js_name = copyFileSync, catch)]
  fn node_copy_file_sync(
    from: &str,
    to: &str,
  ) -> std::result::Result<(), JsValue>;
  #[wasm_bindgen(js_name = linkSync, catch)]
  fn node_link_sync(src: &str, dst: &str) -> std::result::Result<(), JsValue>;
  #[wasm_bindgen(js_name = lstatSync, catch)]
  fn node_lstat_sync(path: &str) -> std::result::Result<Stats, JsValue>;
  #[wasm_bindgen(js_name = mkdirSync, catch)]
  fn node_mkdir_sync(
    path: &str,
    options: JsValue,
  ) -> std::result::Result<(), JsValue>;
  #[wasm_bindgen(js_name = openSync, catch)]
  fn node_open_sync(
    path: &str,
    flags: &str,
    mode: Option<u32>,
  ) -> std::result::Result<i32, JsValue>;
  #[wasm_bindgen(js_name = readFileSync, catch)]
  fn node_read_file_sync(
    path: &str,
  ) -> std::result::Result<js_sys::Uint8Array, JsValue>;
  #[wasm_bindgen(js_name = readdirSync, catch)]
  fn node_readdir_sync(
    path: &str,
    options: &JsValue,
  ) -> std::result::Result<js_sys::Array, JsValue>;
  #[wasm_bindgen(js_name = readlinkSync, catch)]
  fn node_readlink_sync(path: &str) -> std::result::Result<String, JsValue>;
  #[wasm_bindgen(js_name = realpathSync, catch)]
  fn node_realpath_sync(path: &str) -> std::result::Result<String, JsValue>;
  #[wasm_bindgen(js_name = rmSync, catch)]
  fn node_rm_sync(
    path: &str,
    options: &JsValue,
  ) -> std::result::Result<(), JsValue>;
  #[wasm_bindgen(js_name = rmdirSync, catch)]
  fn node_rmdir_sync(
    path: &str,
    options: &JsValue,
  ) -> std::result::Result<(), JsValue>;
  #[wasm_bindgen(js_name = renameSync, catch)]
  fn node_rename_sync(
    oldpath: &str,
    newpath: &str,
  ) -> std::result::Result<(), JsValue>;
  #[wasm_bindgen(js_name = statSync, catch)]
  fn node_stat_sync(path: &str) -> std::result::Result<Stats, JsValue>;
  #[wasm_bindgen(js_name = symlinkSync, catch)]
  fn node_symlink_sync(
    target: &str,
    path: &str,
    type_: Option<&str>,
  ) -> std::result::Result<(), JsValue>;
  #[wasm_bindgen(js_name = unlinkSync, catch)]
  fn node_unlink_sync(path: &str) -> std::result::Result<(), JsValue>;
  #[wasm_bindgen(js_name = writeFileSync, catch)]
  fn node_write_file_sync(
    path: &str,
    data: &[u8],
  ) -> std::result::Result<(), JsValue>;
  #[wasm_bindgen(js_name = utimesSync, catch)]
  fn node_utimes_sync(
    path: &str,
    atime: f64,
    mtime: f64,
  ) -> std::result::Result<(), JsValue>;
  #[wasm_bindgen(js_name = closeSync, catch)]
  fn node_close_sync(fd: i32) -> std::result::Result<(), JsValue>;
  #[wasm_bindgen(js_name = readSync, catch)]
  fn node_read_sync(
    fd: i32,
    buffer: &mut [u8],
    offset: u32,
    length: u32,
    position: Option<i64>,
  ) -> std::result::Result<u32, JsValue>;
  #[wasm_bindgen(js_name = writeSync, catch)]
  fn node_write_sync(
    fd: i32,
    buffer: &[u8],
    offset: u32,
    length: u32,
    position: Option<i32>,
  ) -> std::result::Result<u32, JsValue>;

  #[wasm_bindgen(js_name = fstatSync, catch)]
  fn node_fstat_sync(fd: i32) -> std::result::Result<Stats, JsValue>;
  #[wasm_bindgen(js_name = ftruncateSync, catch)]
  fn node_ftruncate_sync(fd: i32, len: u32)
    -> std::result::Result<(), JsValue>;
  #[wasm_bindgen(js_name = fsyncSync, catch)]
  fn node_fsync_sync(fd: i32) -> std::result::Result<(), JsValue>;
  #[wasm_bindgen(js_name = fdatasyncSync, catch)]
  fn node_fdatasync_sync(fd: i32) -> std::result::Result<(), JsValue>;
  #[wasm_bindgen(js_name = futimesSync, catch)]
  fn node_futimes_sync(
    fd: i32,
    atime: f64,
    mtime: f64,
  ) -> std::result::Result<(), JsValue>;
  #[wasm_bindgen(js_name = fchmodSync, catch)]
  fn node_fchmod_sync(fd: i32, mode: u32) -> std::result::Result<(), JsValue>;
  #[wasm_bindgen(js_name = chownSync, catch)]
  fn node_chown_sync(
    path: &str,
    uid: u32,
    gid: u32,
  ) -> std::result::Result<(), JsValue>;
}

#[wasm_bindgen(module = "node:process")]
extern "C" {
  #[wasm_bindgen(js_name = cwd, catch)]
  fn node_process_cwd() -> std::result::Result<String, JsValue>;
  #[wasm_bindgen(js_name = chdir, catch)]
  fn node_process_chdir(path: &str) -> std::result::Result<(), JsValue>;
  #[wasm_bindgen(js_name = umask, catch)]
  fn node_process_umask(mask: Option<u32>)
    -> std::result::Result<u32, JsValue>;
  #[wasm_bindgen(js_name = env, thread_local_v2)]
  static NODE_PROCESS_ENV: JsValue;
  #[wasm_bindgen(js_name = platform, thread_local_v2)]
  static NODE_PROCESS_PLATFORM: String;
  #[wasm_bindgen(js_name = exit)]
  fn node_process_exit(code: i32);
}

#[wasm_bindgen(module = "node:tty")]
extern "C" {
  #[wasm_bindgen(js_name = isatty)]
  fn node_tty_isatty(fd: i32) -> bool;
}

#[wasm_bindgen]
extern "C" {
  #[wasm_bindgen(js_namespace = ["globalThis", "Date"], js_name = now)]
  fn date_now() -> f64;
  #[wasm_bindgen(js_namespace = ["globalThis", "crypto"], js_name = getRandomValues, catch)]
  fn get_random_values(buf: &mut [u8]) -> std::result::Result<(), JsValue>;
  #[wasm_bindgen(js_namespace = Atomics, js_name = wait)]
  fn atomics_wait(
    i32array: &js_sys::Int32Array,
    index: u32,
    value: i32,
    timeout: f64,
  ) -> String;

}

#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
#[wasm_bindgen(module = "node:os")]
extern "C" {
  #[wasm_bindgen(js_name = tmpdir, catch)]
  fn node_tmpdir() -> std::result::Result<String, JsValue>;
}

#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
#[wasm_bindgen(
  inline_js = "export function copy_bytes(from, to, dst) { new Uint8Array(to.buffer).set(from, dst) }"
)]
extern "C" {
  fn copy_bytes(from: JsValue, to: JsValue, ptr: *mut u8);
}

// ==== Environment ====

impl EnvCurrentDir for RealSys {
  fn env_current_dir(&self) -> std::io::Result<PathBuf> {
    node_process_cwd()
      .map(wasm_string_to_path)
      .map_err(|err| js_value_to_io_error(err))
  }
}

impl BaseEnvSetCurrentDir for RealSys {
  fn base_env_set_current_dir(&self, path: &Path) -> std::io::Result<()> {
    node_process_chdir(&wasm_path_to_str(path)).map_err(js_value_to_io_error)
  }
}

impl BaseEnvVar for RealSys {
  fn base_env_var_os(&self, key: &OsStr) -> Option<OsString> {
    let key = key.to_str()?;
    NODE_PROCESS_ENV.with(|env_obj| {
      let js_key = JsValue::from_str(key);
      let value = js_sys::Reflect::get(env_obj, &js_key).ok()?;
      value.as_string().map(OsString::from)
    })
  }
}

impl EnvVars for RealSys {
  type EnvVarsOs = std::vec::IntoIter<(OsString, OsString)>;

  fn env_vars_os(&self) -> Self::EnvVarsOs {
    let obj = NODE_PROCESS_ENV.with(|env| env.clone());
    let entries = js_sys::Object::entries(&obj.into());
    let mut vars = Vec::with_capacity(entries.length() as usize);
    for entry in entries.iter() {
      let pair = js_sys::Array::from(&entry);
      if let (Some(k), Some(v)) =
        (pair.get(0).as_string(), pair.get(1).as_string())
      {
        vars.push((OsString::from(k), OsString::from(v)));
      }
    }
    vars.into_iter()
  }
}

impl BaseEnvRemoveVar for RealSys {
  fn base_env_remove_var(&self, key: &OsStr) {
    let key = key.to_str().unwrap();
    let js_key = JsValue::from_str(key);
    NODE_PROCESS_ENV.with(|env| {
      let _ = js_sys::Reflect::delete_property(&env.clone().into(), &js_key);
    });
  }
}

impl BaseEnvSetVar for RealSys {
  fn base_env_set_var(&self, key: &OsStr, value: &OsStr) {
    let key = key.to_str().unwrap();
    let value = value.to_str().unwrap();
    NODE_PROCESS_ENV.with(|env_obj| {
      let js_key = JsValue::from_str(key);
      let js_value = JsValue::from_str(value);
      js_sys::Reflect::set(env_obj, &js_key, &js_value).unwrap();
    })
  }
}

impl EnvUmask for RealSys {
  fn env_umask(&self) -> std::io::Result<u32> {
    if is_windows() {
      Err(not_supported_windows("umask"))
    } else {
      node_process_umask(None).map_err(js_value_to_io_error)
    }
  }
}

impl EnvSetUmask for RealSys {
  fn env_set_umask(&self, umask: u32) -> std::io::Result<u32> {
    if is_windows() {
      Err(not_supported_windows("set_umask"))
    } else {
      node_process_umask(Some(umask)).map_err(js_value_to_io_error)
    }
  }
}

impl EnvCacheDir for RealSys {
  fn env_cache_dir(&self) -> Option<PathBuf> {
    if is_windows() {
      self
        .env_var_path("USERPROFILE")
        .map(|dir| dir.join("AppData/Local"))
    } else if NODE_PROCESS_PLATFORM.with(|env| env == "darwin") {
      self.env_home_dir().map(|h| h.join("Library/Caches"))
    } else {
      self
        .env_var_path("XDG_CACHE_HOME")
        .or_else(|| self.env_home_dir().map(|home| home.join(".cache")))
    }
  }
}

impl EnvHomeDir for RealSys {
  fn env_home_dir(&self) -> Option<PathBuf> {
    if is_windows() {
      self.env_var_path("USERPROFILE")
    } else {
      self.env_var_path("HOME")
    }
  }
}

impl EnvProgramsDir for RealSys {
  fn env_programs_dir(&self) -> Option<PathBuf> {
    if is_windows() {
      self
        .env_var_path("LOCALAPPDATA")
        .map(|dir| dir.join("Programs"))
    } else {
      None
    }
  }
}

impl EnvTempDir for RealSys {
  fn env_temp_dir(&self) -> std::io::Result<PathBuf> {
    node_tmpdir()
      .map(wasm_string_to_path)
      .map_err(js_value_to_io_error)
  }
}

// ==== File System ====

impl BaseFsCanonicalize for RealSys {
  fn base_fs_canonicalize(&self, path: &Path) -> Result<PathBuf> {
    node_realpath_sync(&wasm_path_to_str(path))
      .map(wasm_string_to_path)
      .map_err(js_value_to_io_error)
  }
}

impl BaseFsChown for RealSys {
  fn base_fs_chown(
    &self,
    path: &Path,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> Result<()> {
    if is_windows() {
      return Err(not_supported_windows("fs::chown"));
    }

    let (uid, gid) = match (uid, gid) {
      (Some(uid), Some(gid)) => (uid, gid),
      (None, None) => {
        return Ok(());
      }
      (None, Some(_)) | (Some(_), None) => {
        let stats = self.base_fs_metadata(path)?;
        (uid.unwrap_or(stats.uid()?), gid.unwrap_or(stats.gid()?))
      }
    };

    node_chown_sync(&wasm_path_to_str(path), uid, gid)
      .map_err(js_value_to_io_error)
  }
}

impl BaseFsSymlinkChown for RealSys {
  fn base_fs_symlink_chown(
    &self,
    _path: &Path,
    _uid: Option<u32>,
    _gid: Option<u32>,
  ) -> Result<()> {
    Err(Error::new(
      ErrorKind::Unsupported,
      "fs_symlink_chown is not supported in Wasm",
    ))
  }
}

impl BaseFsCopy for RealSys {
  #[inline]
  fn base_fs_copy(&self, from: &Path, to: &Path) -> std::io::Result<u64> {
    node_copy_file_sync(&wasm_path_to_str(from), &wasm_path_to_str(to))
      .map(|()| 0) // this is fine, nobody uses this return value
      .map_err(js_value_to_io_error)
  }
}

impl BaseFsCloneFile for RealSys {
  #[inline]
  fn base_fs_clone_file(
    &self,
    _from: &Path,
    _to: &Path,
  ) -> std::io::Result<()> {
    Err(Error::new(
      ErrorKind::Unsupported,
      "fs_clone_file is not supported in Wasm",
    ))
  }
}

impl BaseFsCreateDir for RealSys {
  fn base_fs_create_dir(
    &self,
    path: &Path,
    options: &CreateDirOptions,
  ) -> Result<()> {
    let path_str = wasm_path_to_str(path);

    let wasm_options = ObjectBuilder::new()
      .field_from("recursive", options.recursive)
      .field_from("mode", options.mode.unwrap_or(0o777))
      .build();

    // Call the Node.js fs.mkdirSync function
    node_mkdir_sync(&path_str, wasm_options)
      .map_err(|e| js_value_to_io_error(e))
  }
}

impl BaseFsHardLink for RealSys {
  fn base_fs_hard_link(&self, src: &Path, dst: &Path) -> std::io::Result<()> {
    let src_str = wasm_path_to_str(src);
    let dst_str = wasm_path_to_str(dst);

    node_link_sync(&src_str, &dst_str).map_err(js_value_to_io_error)
  }
}

impl BaseFsCreateJunction for RealSys {
  fn base_fs_create_junction(
    &self,
    original: &Path,
    junction: &Path,
  ) -> io::Result<()> {
    node_symlink_sync_with_type(original, junction, "junction")
  }
}

impl From<&Stats> for FileType {
  fn from(value: &Stats) -> Self {
    if value.is_file() {
      return FileType::File;
    }

    if value.is_directory() {
      return FileType::Dir;
    }

    if value.is_symbolic_link() {
      return FileType::Symlink;
    }

    FileType::Unknown
  }
}

#[derive(Debug, Clone)]
pub struct WasmMetadata(Stats);

impl FsMetadataValue for WasmMetadata {
  fn file_type(&self) -> FileType {
    (&self.0).into()
  }

  fn len(&self) -> u64 {
    let Ok(m) = js_sys::Reflect::get(&self.0, &JsValue::from_str("size"))
    else {
      return 0;
    };
    m.as_f64().unwrap_or(0.0) as u64
  }

  fn accessed(&self) -> Result<SystemTime> {
    parse_date_prop(&self.0, "atime")
  }

  fn created(&self) -> Result<SystemTime> {
    parse_date_prop(&self.0, "birthtime")
  }

  fn changed(&self) -> Result<SystemTime> {
    parse_date_prop(&self.0, "ctime")
  }

  fn modified(&self) -> Result<SystemTime> {
    parse_date_prop(&self.0, "mtime")
  }

  fn dev(&self) -> Result<u64> {
    parse_u64_prop(&self.0, "dev")
  }

  fn ino(&self) -> Result<u64> {
    parse_u64_prop(&self.0, "ino")
  }

  fn mode(&self) -> Result<u32> {
    parse_u32_prop(&self.0, "mode")
  }

  fn nlink(&self) -> Result<u64> {
    parse_u64_prop(&self.0, "nlink")
  }

  fn uid(&self) -> Result<u32> {
    parse_u32_prop(&self.0, "uid")
  }

  fn gid(&self) -> Result<u32> {
    parse_u32_prop(&self.0, "gid")
  }

  fn rdev(&self) -> Result<u64> {
    parse_u64_prop(&self.0, "rdev")
  }

  fn blksize(&self) -> Result<u64> {
    parse_u64_prop(&self.0, "blksize")
  }

  fn blocks(&self) -> Result<u64> {
    parse_u64_prop(&self.0, "blocks")
  }

  fn is_block_device(&self) -> Result<bool> {
    if is_windows() {
      Err(not_supported_windows("is_block_device"))
    } else {
      Ok(self.0.is_block_device())
    }
  }

  fn is_char_device(&self) -> Result<bool> {
    if is_windows() {
      Err(not_supported_windows("is_char_device"))
    } else {
      Ok(self.0.is_character_device())
    }
  }

  fn is_fifo(&self) -> Result<bool> {
    if is_windows() {
      Err(not_supported_windows("is_fifo"))
    } else {
      Ok(self.0.is_fifo())
    }
  }

  fn is_socket(&self) -> Result<bool> {
    if is_windows() {
      Err(not_supported_windows("is_socket"))
    } else {
      Ok(self.0.is_socket())
    }
  }

  fn file_attributes(&self) -> Result<u32> {
    Err(Error::new(
      ErrorKind::Unsupported,
      "file_attributes is not supported in Wasm",
    ))
  }
}

fn not_supported_windows(name: &str) -> std::io::Error {
  Error::new(
    ErrorKind::Unsupported,
    format!("{} is not supported on Windows", name),
  )
}

fn parse_date_prop(value: &JsValue, prop: &'static str) -> Result<SystemTime> {
  let m = get_prop(value, prop)?;
  if let Some(date) = m.dyn_ref::<js_sys::Date>() {
    let ms = date.get_time() as u64;
    Ok(SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(ms))
  } else if m.is_null() {
    Err(Error::new(
      ErrorKind::Unsupported,
      format!("{} not supported", prop),
    ))
  } else if m.is_undefined() {
    Err(Error::new(ErrorKind::Other, format!("{} not found", prop)))
  } else {
    Err(Error::new(ErrorKind::Other, format!("{} not a date", prop)))
  }
}

fn parse_u32_prop(value: &JsValue, prop: &'static str) -> Result<u32> {
  if is_windows() {
    return Err(not_supported_windows(prop));
  }

  let m = get_prop(value, prop)?;
  if let Some(num) = m.as_f64() {
    if num >= 0.0 && num.fract() == 0.0 && num <= u32::MAX as f64 {
      Ok(num as u32)
    } else {
      Err(Error::new(
        ErrorKind::Other,
        format!("{} is out of range for u32", prop),
      ))
    }
  } else if m.is_null() {
    Err(Error::new(
      ErrorKind::Unsupported,
      format!("{} not supported", prop),
    ))
  } else {
    Err(Error::new(
      ErrorKind::Other,
      format!("{} is not a valid number", prop),
    ))
  }
}

fn parse_u64_prop(value: &JsValue, prop: &'static str) -> Result<u64> {
  if is_windows() {
    return Err(not_supported_windows(prop));
  }
  let m = get_prop(value, prop)?;
  if let Some(bigint) = m.dyn_ref::<js_sys::BigInt>() {
    if let Some(bigint_f64) = bigint.as_f64() {
      if bigint_f64 >= 0.0
        && bigint_f64 <= u64::MAX as f64
        && bigint_f64.fract() == 0.0
      {
        Ok(bigint_f64 as u64)
      } else {
        Err(Error::new(
          ErrorKind::Other,
          format!("{} is out of range for u64", prop),
        ))
      }
    } else {
      Err(Error::new(
        ErrorKind::Other,
        format!("{} is not a valid u64", prop),
      ))
    }
  } else if let Some(num) = m.as_f64() {
    if num >= 0.0 && num.fract() == 0.0 && num <= u64::MAX as f64 {
      Ok(num as u64)
    } else {
      Err(Error::new(
        ErrorKind::Other,
        format!("{} is out of range for u64", prop),
      ))
    }
  } else if m.is_null() {
    Err(Error::new(
      ErrorKind::Unsupported,
      format!("{} not supported", prop),
    ))
  } else if m.is_undefined() {
    Err(Error::new(ErrorKind::Other, format!("{} not found", prop)))
  } else {
    Err(Error::new(
      ErrorKind::Other,
      format!("{} is not a number or bigint", prop),
    ))
  }
}

fn get_prop(value: &JsValue, prop: &'static str) -> Result<JsValue> {
  js_sys::Reflect::get(value, &JsValue::from_str(prop)).map_err(|_| {
    std::io::Error::new(
      std::io::ErrorKind::Other,
      format!("Failed to access {}", prop),
    )
  })
}

impl BaseFsMetadata for RealSys {
  type Metadata = WasmMetadata;

  #[inline]
  fn base_fs_metadata(&self, path: &Path) -> Result<WasmMetadata> {
    let s = wasm_path_to_str(path);
    match node_stat_sync(&s) {
      Ok(v) => Ok(WasmMetadata(v)),
      Err(e) => Err(js_value_to_io_error(e)),
    }
  }

  #[inline]
  fn base_fs_symlink_metadata(&self, path: &Path) -> Result<WasmMetadata> {
    let s = wasm_path_to_str(path);
    match node_lstat_sync(&s) {
      Ok(v) => Ok(WasmMetadata(v)),
      Err(e) => Err(js_value_to_io_error(e)),
    }
  }
}

impl BaseFsOpen for RealSys {
  type File = WasmFile;

  fn base_fs_open(
    &self,
    path: &Path,
    options: &OpenOptions,
  ) -> std::io::Result<WasmFile> {
    let s = wasm_path_to_str(path).into_owned();

    // Convert OpenOptions to Node.js flags
    let flags = if options.create_new {
      "wx" // This should produce EEXIST if file exists
    } else if options.append {
      "a"
    } else if options.write && options.create && options.truncate {
      "w"
    } else if options.write && options.create {
      // TODO: this should really create the file, but
      // i don't think there's a mode that does write + create + no append + no truncate.
      "r+"
    } else if options.write {
      "r+"
    } else {
      "r"
    };

    let mode = options.mode;
    let fd = node_open_sync(&s, flags, mode).map_err(js_value_to_io_error)?;

    // Set initial position based on flags
    let initial_position = if options.append {
      // For append mode, start at the end of the file
      let metadata = node_fstat_sync(fd).map_err(js_value_to_io_error)?;
      js_sys::Reflect::get(&metadata, &JsValue::from_str("size"))
        .map_err(js_value_to_io_error)?
        .as_f64()
        .unwrap_or(0.0) as u64
    } else {
      0
    };

    Ok(WasmFile {
      fd,
      path: s,
      position: initial_position,
    })
  }
}

impl BaseFsRead for RealSys {
  fn base_fs_read(&self, path: &Path) -> Result<Cow<'static, [u8]>> {
    let s = wasm_path_to_str(path);
    let ua = node_read_file_sync(&s).map_err(js_value_to_io_error)?;

    // manually construct a vec to work around bug: https://github.com/dsherret/sys_traits/pull/58
    let len = ua.byte_length() as usize;
    let mut vec = Vec::with_capacity(len);
    copy_bytes(ua.into(), wasm_bindgen::memory(), vec.as_mut_ptr());
    unsafe {
      vec.set_len(len);
    }

    Ok(Cow::Owned(vec))
  }
}

#[derive(Debug, Clone)]
pub struct ObjectBuilder {
  object: js_sys::Object,
}

impl ObjectBuilder {
  pub fn new() -> Self {
    Self {
      object: js_sys::Object::new(),
    }
  }

  pub fn field_from<V: Into<JsValue>>(self, name: &str, value: V) -> Self {
    js_sys::Reflect::set(&self.object, &JsValue::from_str(name), &value.into())
      .unwrap();
    self
  }

  pub fn build(self) -> JsValue {
    self.object.into()
  }
}

impl BaseFsReadDir for RealSys {
  type ReadDirEntry = WasmFsDirEntry;

  fn base_fs_read_dir(
    &self,
    path: &Path,
  ) -> std::io::Result<
    Box<dyn Iterator<Item = std::io::Result<Self::ReadDirEntry>>>,
  > {
    let path_str = wasm_path_to_str(path);

    let wasm_options = ObjectBuilder::new()
      .field_from("withFileTypes", true)
      .build();

    let entries = node_readdir_sync(&path_str, &JsValue::from(wasm_options))
      .map_err(js_value_to_io_error)?;

    let path = path.to_path_buf();
    let entries_vec: Vec<JsValue> = js_sys::Array::from(&entries).to_vec();

    Ok(Box::new(entries_vec.into_iter().map(move |entry| {
      Ok(WasmFsDirEntry {
        value: entry,
        parent_path: path.clone(),
      })
    })))
  }
}

impl BaseFsReadLink for RealSys {
  fn base_fs_read_link(&self, path: &Path) -> io::Result<PathBuf> {
    let s = wasm_path_to_str(path);
    node_readlink_sync(&s)
      .map(wasm_string_to_path)
      .map_err(js_value_to_io_error)
  }
}

#[derive(Debug)]
pub struct WasmFsDirEntry {
  parent_path: PathBuf,
  value: JsValue,
}

impl FsDirEntry for WasmFsDirEntry {
  type Metadata = WasmMetadata;

  fn file_name(&self) -> Cow<OsStr> {
    let name = js_sys::Reflect::get(&self.value, &JsValue::from_str("name"))
      .ok()
      .and_then(|v| v.as_string())
      .unwrap_or_default();
    Cow::Owned(OsString::from(name))
  }

  fn file_type(&self) -> std::io::Result<FileType> {
    use wasm_bindgen::JsCast;

    let is_file_fn =
      js_sys::Reflect::get(&self.value, &JsValue::from_str("isFile"))
        .map_err(js_value_to_io_error)?
        .dyn_into::<js_sys::Function>()
        .map_err(js_value_to_io_error)?;
    let is_file =
      js_sys::Reflect::apply(&is_file_fn, &self.value, &js_sys::Array::new())
        .map_err(js_value_to_io_error)?;

    if is_file.as_bool().unwrap_or(false) {
      return Ok(FileType::File);
    }

    let is_directory_fn =
      js_sys::Reflect::get(&self.value, &JsValue::from_str("isDirectory"))
        .map_err(js_value_to_io_error)?
        .dyn_into::<js_sys::Function>()
        .map_err(js_value_to_io_error)?;
    let is_directory = js_sys::Reflect::apply(
      &is_directory_fn,
      &self.value,
      &js_sys::Array::new(),
    )
    .map_err(js_value_to_io_error)?;

    if is_directory.as_bool().unwrap_or(false) {
      return Ok(FileType::Dir);
    }

    let is_symlink_fn =
      js_sys::Reflect::get(&self.value, &JsValue::from_str("isSymbolicLink"))
        .map_err(js_value_to_io_error)?
        .dyn_into::<js_sys::Function>()
        .map_err(js_value_to_io_error)?;
    let is_symlink = js_sys::Reflect::apply(
      &is_symlink_fn,
      &self.value,
      &js_sys::Array::new(),
    )
    .map_err(js_value_to_io_error)?;

    if is_symlink.as_bool().unwrap_or(false) {
      return Ok(FileType::Symlink);
    }

    Ok(FileType::Unknown)
  }

  fn metadata(&self) -> std::io::Result<Self::Metadata> {
    let path = self.path();
    let s = wasm_path_to_str(&path);
    match node_stat_sync(&s) {
      Ok(v) => Ok(WasmMetadata(v)),
      Err(e) => Err(js_value_to_io_error(e)),
    }
  }

  fn path(&self) -> Cow<Path> {
    let name = js_sys::Reflect::get(&self.value, &JsValue::from_str("name"))
      .ok()
      .and_then(|v| v.as_string())
      .unwrap_or_default();
    Cow::Owned(self.parent_path.join(name))
  }
}

impl BaseFsRemoveDir for RealSys {
  fn base_fs_remove_dir(&self, path: &Path) -> std::io::Result<()> {
    let s = wasm_path_to_str(path);
    let options = ObjectBuilder::new().build();
    node_rmdir_sync(&s, &JsValue::from(options)).map_err(js_value_to_io_error)
  }
}

impl BaseFsRemoveDirAll for RealSys {
  fn base_fs_remove_dir_all(&self, path: &Path) -> std::io::Result<()> {
    let s = wasm_path_to_str(path);
    let options = ObjectBuilder::new()
      .field_from("recursive", true)
      .field_from("force", true)
      .build();
    node_rm_sync(&s, &JsValue::from(options)).map_err(js_value_to_io_error)
  }
}

impl BaseFsRemoveFile for RealSys {
  fn base_fs_remove_file(&self, path: &Path) -> std::io::Result<()> {
    let s = wasm_path_to_str(path);
    node_unlink_sync(&s).map_err(js_value_to_io_error)
  }
}

impl BaseFsRename for RealSys {
  fn base_fs_rename(&self, from: &Path, to: &Path) -> std::io::Result<()> {
    let f = wasm_path_to_str(from);
    let t = wasm_path_to_str(to);
    node_rename_sync(&f, &t).map_err(js_value_to_io_error)
  }
}

impl BaseFsSetFileTimes for RealSys {
  #[inline]
  fn base_fs_set_file_times(
    &self,
    path: &Path,
    atime: SystemTime,
    mtime: SystemTime,
  ) -> Result<()> {
    let atime_secs = system_time_to_secs(atime)?;
    let mtime_secs = system_time_to_secs(mtime)?;
    node_utimes_sync(&wasm_path_to_str(path), atime_secs, mtime_secs)
      .map_err(js_value_to_io_error)
  }
}

fn system_time_to_secs(system_time: SystemTime) -> Result<f64> {
  let duration_since_epoch = system_time
    .duration_since(SystemTime::UNIX_EPOCH)
    .map_err(|_| {
      Error::new(ErrorKind::InvalidInput, "SystemTime before UNIX EPOCH")
    })?;
  let secs = duration_since_epoch.as_secs() as f64
    + duration_since_epoch.subsec_nanos() as f64 / 1_000_000_000.0;

  Ok(secs)
}

impl BaseFsSetSymlinkFileTimes for RealSys {
  #[inline]
  fn base_fs_set_symlink_file_times(
    &self,
    _path: &Path,
    _atime: SystemTime,
    _mtime: SystemTime,
  ) -> Result<()> {
    Err(Error::new(
      ErrorKind::Unsupported,
      "fs_set_symlink_file_times is not supported in Wasm",
    ))
  }
}

impl BaseFsSetPermissions for RealSys {
  fn base_fs_set_permissions(
    &self,
    path: &Path,
    mode: u32,
  ) -> std::io::Result<()> {
    if is_windows() {
      Err(std::io::Error::new(
        ErrorKind::Unsupported,
        "cannot set path permissions on this platform",
      ))
    } else {
      let path = wasm_path_to_str(path);
      node_chmod_sync(&path, mode).map_err(js_value_to_io_error)
    }
  }
}

impl BaseFsSymlinkDir for RealSys {
  fn base_fs_symlink_dir(
    &self,
    original: &Path,
    link: &Path,
  ) -> std::io::Result<()> {
    node_symlink_sync_with_type(original, link, "dir")
  }
}

impl BaseFsSymlinkFile for RealSys {
  fn base_fs_symlink_file(
    &self,
    original: &Path,
    link: &Path,
  ) -> std::io::Result<()> {
    node_symlink_sync_with_type(original, link, "file")
  }
}

fn node_symlink_sync_with_type(
  original: &Path,
  link: &Path,
  type_str: &'static str,
) -> std::io::Result<()> {
  let target = wasm_path_to_str(original);
  let path = wasm_path_to_str(link);

  node_symlink_sync(&target, &path, Some(type_str))
    .map_err(js_value_to_io_error)
}

impl BaseFsWrite for RealSys {
  fn base_fs_write(&self, path: &Path, data: &[u8]) -> std::io::Result<()> {
    let s = wasm_path_to_str(path);
    node_write_file_sync(&s, data).map_err(js_value_to_io_error)
  }
}

// ==== File System File ====

#[derive(Debug)]
pub struct WasmFile {
  fd: i32,
  #[allow(dead_code)]
  path: String,
  position: u64,
}

impl Drop for WasmFile {
  fn drop(&mut self) {
    let _ = node_close_sync(self.fd);
  }
}

impl FsFile for WasmFile {}

impl FsFileAsRaw for WasmFile {}

impl FsFileIsTerminal for WasmFile {
  #[inline]
  fn fs_file_is_terminal(&self) -> bool {
    node_tty_isatty(self.fd)
  }
}

impl FsFileLock for WasmFile {
  fn fs_file_lock(&mut self, _mode: FsFileLockMode) -> io::Result<()> {
    Err(io::Error::new(
      io::ErrorKind::Unsupported,
      "file locking is not supported",
    ))
  }

  fn fs_file_try_lock(&mut self, _mode: FsFileLockMode) -> io::Result<()> {
    Err(io::Error::new(
      io::ErrorKind::Unsupported,
      "file locking is not supported",
    ))
  }

  fn fs_file_unlock(&mut self) -> io::Result<()> {
    Err(io::Error::new(
      io::ErrorKind::Unsupported,
      "file locking is not supported",
    ))
  }
}

impl FsFileSetLen for WasmFile {
  fn fs_file_set_len(&mut self, size: u64) -> std::io::Result<()> {
    node_ftruncate_sync(self.fd, size as u32).map_err(js_value_to_io_error)
  }
}

impl FsFileMetadata for WasmFile {
  fn fs_file_metadata(&self) -> io::Result<BoxedFsMetadataValue> {
    node_fstat_sync(self.fd)
      .map(|m| BoxedFsMetadataValue::new(WasmMetadata(m)))
      .map_err(js_value_to_io_error)
  }
}

impl FsFileSetPermissions for WasmFile {
  fn fs_file_set_permissions(&mut self, mode: u32) -> std::io::Result<()> {
    if is_windows() {
      return Ok(()); // ignore
    }
    node_fchmod_sync(self.fd, mode).map_err(js_value_to_io_error)
  }
}

impl FsFileSetTimes for WasmFile {
  fn fs_file_set_times(
    &mut self,
    file_times: FsFileTimes,
  ) -> std::io::Result<()> {
    let FsFileTimes { accessed, modified } = file_times;
    let (atime, mtime) = match (accessed, modified) {
      (Some(atime), Some(mtime)) => (atime, mtime),
      (Some(atime), None) => {
        let metadata = self.fs_file_metadata()?;
        (atime, metadata.modified()?)
      }
      (None, Some(mtime)) => {
        let metadata = self.fs_file_metadata()?;
        (metadata.accessed()?, mtime)
      }
      (None, None) => {
        return Ok(());
      }
    };
    let atime_secs = system_time_to_secs(atime)?;
    let mtime_secs = system_time_to_secs(mtime)?;
    node_futimes_sync(self.fd, atime_secs, mtime_secs)
      .map_err(js_value_to_io_error)
  }
}

impl FsFileSyncAll for WasmFile {
  #[inline]
  fn fs_file_sync_all(&mut self) -> io::Result<()> {
    node_fsync_sync(self.fd).map_err(js_value_to_io_error)
  }
}

impl FsFileSyncData for WasmFile {
  #[inline]
  fn fs_file_sync_data(&mut self) -> io::Result<()> {
    node_fdatasync_sync(self.fd).map_err(js_value_to_io_error)
  }
}

impl std::io::Seek for WasmFile {
  fn seek(&mut self, pos: std::io::SeekFrom) -> Result<u64> {
    let new_position = match pos {
      std::io::SeekFrom::Start(offset) => offset,
      std::io::SeekFrom::End(offset) => {
        // We need to get file size first
        let metadata =
          node_fstat_sync(self.fd).map_err(js_value_to_io_error)?;
        let size = js_sys::Reflect::get(&metadata, &JsValue::from_str("size"))
          .map_err(js_value_to_io_error)?
          .as_f64()
          .unwrap_or(0.0) as u64;
        (size as i64 + offset) as u64
      }
      std::io::SeekFrom::Current(offset) => {
        (self.position as i64 + offset) as u64
      }
    };
    self.position = new_position;
    Ok(new_position)
  }
}

impl std::io::Write for WasmFile {
  fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
    let bytes_written = node_write_sync(
      self.fd,
      buf,
      0,
      buf.len() as u32,
      Some(self.position as i32),
    )
    .map_err(js_value_to_io_error)? as usize;
    self.position += bytes_written as u64;
    Ok(bytes_written)
  }

  fn flush(&mut self) -> std::io::Result<()> {
    node_fsync_sync(self.fd).map_err(js_value_to_io_error)
  }
}

impl std::io::Read for WasmFile {
  fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
    let bytes_read = node_read_sync(
      self.fd,
      buf,
      0,
      buf.len() as u32,
      Some(self.position as i64),
    )
    .map_err(js_value_to_io_error)? as usize;
    self.position += bytes_read as u64;
    Ok(bytes_read)
  }
}

// ==== System ====

impl SystemTimeNow for RealSys {
  #[inline]
  fn sys_time_now(&self) -> SystemTime {
    SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(date_now() as u64)
  }
}

impl crate::SystemRandom for RealSys {
  #[inline]
  fn sys_random(&self, buf: &mut [u8]) -> Result<()> {
    const MAX_BUFFER_SIZE: usize = 65536; // max buffer size for WebCrypto

    for chunk in buf.chunks_mut(MAX_BUFFER_SIZE) {
      if let Err(err) = get_random_values(chunk) {
        return Err(js_value_to_io_error(err));
      }
    }
    Ok(())
  }
}

impl crate::ProcessExit for RealSys {
  fn process_exit(&self, code: i32) -> ! {
    node_process_exit(code);
    unreachable!()
  }
}

impl crate::ThreadSleep for RealSys {
  fn thread_sleep(&self, duration: std::time::Duration) {
    use js_sys::Int32Array;
    use js_sys::SharedArrayBuffer;

    let sab = SharedArrayBuffer::new(4);
    let int32_array = Int32Array::new(&sab);
    int32_array.set_index(0, 0);
    let timeout = duration.as_millis() as f64;
    let _result = atomics_wait(&int32_array, 0, 0, timeout);
  }
}

#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
#[inline]
pub fn is_windows() -> bool {
  NODE_PROCESS_PLATFORM.with(|env| env == "win32")
}

fn js_value_to_io_error(js_value: wasm_bindgen::JsValue) -> Error {
  use wasm_bindgen::JsCast;

  // Check if the error is a Node.js Error object
  if let Some(error_obj) = js_value.dyn_ref::<js_sys::Error>() {
    let error_name = error_obj.name();
    let message = error_obj
      .message()
      .as_string()
      .unwrap_or_else(|| "Unknown error".to_string());

    // Check for Node.js error codes in the error object
    let error_code =
      js_sys::Reflect::get(&js_value, &JsValue::from_str("code"))
        .ok()
        .and_then(|v| v.as_string())
        .or_else(|| {
          // Also try 'errno' property
          js_sys::Reflect::get(&js_value, &JsValue::from_str("errno"))
            .ok()
            .and_then(|v| v.as_string())
        });

    let maybe_kind = if let Some(code) = error_code {
      match code.as_str() {
        "ENOENT" => Some(ErrorKind::NotFound),
        "EEXIST" => Some(ErrorKind::AlreadyExists),
        "EACCES" | "EPERM" => Some(ErrorKind::PermissionDenied),
        "EISDIR" => Some(ErrorKind::InvalidInput),
        "ENOTDIR" => Some(ErrorKind::NotFound),
        "ENOSPC" => Some(ErrorKind::StorageFull),
        "EMFILE" | "ENFILE" => Some(ErrorKind::Other), // Too many open files
        "ENOTSUP" | "EOPNOTSUPP" => Some(ErrorKind::Unsupported),
        "ETIMEDOUT" => Some(ErrorKind::TimedOut),
        "ECONNREFUSED" => Some(ErrorKind::ConnectionRefused),
        "ECONNRESET" => Some(ErrorKind::ConnectionReset),
        "ECONNABORTED" => Some(ErrorKind::ConnectionAborted),
        "EADDRINUSE" => Some(ErrorKind::AddrInUse),
        "EADDRNOTAVAIL" => Some(ErrorKind::AddrNotAvailable),
        "EBADF" => Some(ErrorKind::InvalidInput),
        "EINVAL" => Some(ErrorKind::InvalidInput),
        "ELOOP" => Some(ErrorKind::InvalidInput),
        "ENAMETOOLONG" => Some(ErrorKind::InvalidInput),
        "EROFS" => Some(ErrorKind::PermissionDenied),
        _ => None,
      }
    } else if error_name == "NotFound" {
      Some(ErrorKind::NotFound)
    } else if error_name == "AlreadyExists" {
      Some(ErrorKind::AlreadyExists)
    } else if error_name == "NotSupported" {
      Some(ErrorKind::Unsupported)
    } else {
      // If no error code, try to infer from message
      if message.contains("file already exists") || message.contains("EEXIST") {
        Some(ErrorKind::AlreadyExists)
      } else if message.contains("no such file") || message.contains("ENOENT") {
        Some(ErrorKind::NotFound)
      } else if message.contains("permission denied")
        || message.contains("EACCES")
        || message.contains("EPERM")
      {
        Some(ErrorKind::PermissionDenied)
      } else {
        None
      }
    };

    if let Some(error_kind) = maybe_kind {
      return Error::new(error_kind, message);
    } else {
      return Error::new(ErrorKind::Other, message);
    }
  }

  // Fallback for unknown error types
  if let Some(err_msg) = js_value.as_string() {
    Error::new(ErrorKind::Other, err_msg)
  } else {
    Error::new(ErrorKind::Other, "An unknown JavaScript error occurred")
  }
}
