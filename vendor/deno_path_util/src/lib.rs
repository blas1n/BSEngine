// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(clippy::print_stderr)]
#![deny(clippy::print_stdout)]
#![deny(clippy::unused_async)]
#![deny(clippy::unnecessary_wraps)]

use deno_error::JsError;
use std::borrow::Cow;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use sys_traits::SystemRandom;
use sys_traits::impls::is_windows;
use thiserror::Error;
use url::Url;

pub mod fs;

/// Gets the parent of this url.
pub fn url_parent(url: &Url) -> Url {
  let mut url = url.clone();
  // don't use url.segments() because it will strip the leading slash
  let mut segments = url.path().split('/').collect::<Vec<_>>();
  if segments.iter().all(|s| s.is_empty()) {
    return url;
  }
  if let Some(last) = segments.last() {
    if last.is_empty() {
      segments.pop();
    }
    segments.pop();
    let new_path = format!("{}/", segments.join("/"));
    url.set_path(&new_path);
  }
  url
}

#[derive(Debug, Error, deno_error::JsError)]
#[class(uri)]
#[error("Could not convert URL to file path.\n  URL: {0}")]
pub struct UrlToFilePathError(pub Url);

/// Attempts to convert a url to a file path. By default, uses the Url
/// crate's `to_file_path()` method, but falls back to try and resolve unix-style
/// paths on Windows.
pub fn url_to_file_path(url: &Url) -> Result<PathBuf, UrlToFilePathError> {
  let result = if url.scheme() != "file" {
    Err(())
  } else {
    url_to_file_path_inner(url)
  };
  match result {
    Ok(path) => Ok(path),
    Err(()) => Err(UrlToFilePathError(url.clone())),
  }
}

fn url_to_file_path_inner(url: &Url) -> Result<PathBuf, ()> {
  #[cfg(any(unix, windows, target_os = "redox", target_os = "wasi"))]
  return url_to_file_path_real(url);
  #[cfg(not(any(unix, windows, target_os = "redox", target_os = "wasi")))]
  url_to_file_path_wasm(url)
}

#[cfg(any(unix, windows, target_os = "redox", target_os = "wasi"))]
fn url_to_file_path_real(url: &Url) -> Result<PathBuf, ()> {
  if cfg!(windows) {
    match url.to_file_path() {
      Ok(path) => Ok(path),
      Err(()) => {
        // This might be a unix-style path which is used in the tests even on Windows.
        // Attempt to see if we can convert it to a `PathBuf`. This code should be removed
        // once/if https://github.com/servo/rust-url/issues/730 is implemented.
        if url.scheme() == "file"
          && url.host().is_none()
          && url.port().is_none()
          && url.path_segments().is_some()
        {
          let path_str = url.path();
          match String::from_utf8(
            percent_encoding::percent_decode(path_str.as_bytes()).collect(),
          ) {
            Ok(path_str) => Ok(PathBuf::from(path_str)),
            Err(_) => Err(()),
          }
        } else {
          Err(())
        }
      }
    }
  } else {
    url.to_file_path()
  }
}

#[cfg(any(
  test,
  not(any(unix, windows, target_os = "redox", target_os = "wasi"))
))]
#[allow(clippy::unnecessary_wraps)]
fn url_to_file_path_wasm(url: &Url) -> Result<PathBuf, ()> {
  fn is_windows_path_segment(url: &str) -> bool {
    let mut chars = url.chars();

    let first_char = chars.next();
    if first_char.is_none() || !first_char.unwrap().is_ascii_alphabetic() {
      return false;
    }

    if chars.next() != Some(':') {
      return false;
    }

    chars.next().is_none()
  }

  let path_segments = url.path_segments().unwrap().collect::<Vec<_>>();
  let mut final_text = String::new();
  let mut is_windows_share = false;
  if let Some(host) = url.host_str() {
    final_text.push_str("\\\\");
    final_text.push_str(host);
    is_windows_share = true;
  }
  for segment in path_segments.iter() {
    if is_windows_share {
      final_text.push('\\');
    } else if !final_text.is_empty() {
      final_text.push('/');
    }
    final_text.push_str(
      &percent_encoding::percent_decode_str(segment).decode_utf8_lossy(),
    );
  }
  if !is_windows_share && !is_windows_path_segment(path_segments[0]) {
    final_text = format!("/{}", final_text);
  }
  Ok(PathBuf::from(final_text))
}

/// Normalize all intermediate components of the path (ie. remove "./" and "../" components).
/// Similar to `fs::canonicalize()` but doesn't resolve symlinks.
///
/// Adapted from Cargo
/// <https://github.com/rust-lang/cargo/blob/af307a38c20a753ec60f0ad18be5abed3db3c9ac/src/cargo/util/paths.rs#L60-L85>
#[inline]
pub fn normalize_path(path: Cow<Path>) -> Cow<Path> {
  fn should_normalize(path: &Path) -> bool {
    if path_has_trailing_separator(path) {
      return true;
    }

    let mut last_part = None;
    for component in path.components() {
      match component {
        Component::CurDir | Component::ParentDir => {
          return true;
        }
        Component::Prefix(..) | Component::RootDir => {
          // ok
        }
        Component::Normal(component) => {
          last_part = Some(component);
        }
      }
    }

    if is_windows()
      && let Some(last_part) = last_part
    {
      let bytes = last_part.as_encoded_bytes();
      if bytes.ends_with(b".") || bytes.ends_with(b" ") {
        return true;
      }
    }

    path_has_cur_dir_separator(path)
  }

  fn path_has_trailing_separator(path: &Path) -> bool {
    #[cfg(unix)]
    let raw = std::os::unix::ffi::OsStrExt::as_bytes(path.as_os_str());
    #[cfg(windows)]
    let raw = path.as_os_str().as_encoded_bytes();
    #[cfg(target_arch = "wasm32")]
    let raw = path.to_string_lossy();
    #[cfg(target_arch = "wasm32")]
    let raw = raw.as_bytes();

    if sys_traits::impls::is_windows() {
      raw.contains(&b'/') || raw.ends_with(b"\\")
    } else {
      raw.ends_with(b"/")
    }
  }

  // Rust normalizes away `Component::CurDir` most of the time
  // so we need to explicitly check for it in the bytes
  fn path_has_cur_dir_separator(path: &Path) -> bool {
    #[cfg(unix)]
    let raw = std::os::unix::ffi::OsStrExt::as_bytes(path.as_os_str());
    #[cfg(windows)]
    let raw = path.as_os_str().as_encoded_bytes();
    #[cfg(target_arch = "wasm32")]
    let raw = path.to_string_lossy();
    #[cfg(target_arch = "wasm32")]
    let raw = raw.as_bytes();

    if raw.ends_with(b"\\.") || raw.ends_with(b"/.") {
      return true;
    }

    if sys_traits::impls::is_windows() {
      for window in raw.windows(3) {
        if matches!(window, [b'\\', b'.', b'\\']) {
          return true;
        }
      }
    } else {
      for window in raw.windows(3) {
        if matches!(window, [b'/', b'.', b'/']) {
          return true;
        }
      }
    }

    false
  }

  fn inner(path: &Path) -> PathBuf {
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
          if is_windows() {
            let bytes = c.as_encoded_bytes();
            // Strip trailing dots and spaces on Windows
            let mut end = bytes.len();
            while end > 0 && (bytes[end - 1] == b'.' || bytes[end - 1] == b' ')
            {
              end -= 1;
            }
            if end == bytes.len() {
              ret.push(c);
            } else if end > 0 {
              #[cfg(windows)]
              {
                use std::os::windows::ffi::{OsStrExt, OsStringExt};
                let wide: Vec<u16> = c.encode_wide().collect();
                let trimmed = std::ffi::OsString::from_wide(&wide[..end]);
                ret.push(trimmed);
              }
              // SAFETY: trimmed spaces and dots only
              #[cfg(not(windows))]
              unsafe {
                let trimmed =
                  std::ffi::OsStr::from_encoded_bytes_unchecked(&bytes[..end]);
                ret.push(trimmed);
              }
            }
          } else {
            ret.push(c);
          }
        }
      }
    }
    ret
  }

  if should_normalize(&path) {
    Cow::Owned(inner(&path))
  } else {
    path
  }
}

#[derive(Debug, Clone, Error, deno_error::JsError, PartialEq, Eq)]
#[class(uri)]
#[error("Could not convert path to URL.\n  Path: {0}")]
pub struct PathToUrlError(pub PathBuf);

#[allow(clippy::result_unit_err)]
pub fn url_from_file_path(path: &Path) -> Result<Url, PathToUrlError> {
  let path = normalize_path(Cow::Borrowed(path));
  #[cfg(any(unix, windows, target_os = "redox", target_os = "wasi"))]
  return Url::from_file_path(&path)
    .map_err(|()| PathToUrlError(path.to_path_buf()));
  #[cfg(not(any(unix, windows, target_os = "redox", target_os = "wasi")))]
  url_from_file_path_wasm(&path)
    .map_err(|()| PathToUrlError(path.to_path_buf()))
}

#[allow(clippy::result_unit_err)]
pub fn url_from_directory_path(path: &Path) -> Result<Url, PathToUrlError> {
  let path = normalize_path(Cow::Borrowed(path));
  #[cfg(any(unix, windows, target_os = "redox", target_os = "wasi"))]
  return Url::from_directory_path(&path)
    .map_err(|()| PathToUrlError(path.to_path_buf()));
  #[cfg(not(any(unix, windows, target_os = "redox", target_os = "wasi")))]
  url_from_directory_path_wasm(&path)
    .map_err(|()| PathToUrlError(path.to_path_buf()))
}

#[cfg(any(
  test,
  not(any(unix, windows, target_os = "redox", target_os = "wasi"))
))]
fn url_from_directory_path_wasm(path: &Path) -> Result<Url, ()> {
  let mut url = url_from_file_path_wasm(path)?;
  url.path_segments_mut().unwrap().push("");
  Ok(url)
}

#[cfg(any(
  test,
  not(any(unix, windows, target_os = "redox", target_os = "wasi"))
))]
fn url_from_file_path_wasm(path: &Path) -> Result<Url, ()> {
  use std::path::Component;

  let original_path = path.to_string_lossy();
  let mut path_str = original_path;
  // assume paths containing backslashes are windows paths
  if path_str.contains('\\') {
    let mut url = Url::parse("file://").unwrap();
    if let Some(next) = path_str.strip_prefix(r#"\\?\UNC\"#) {
      if let Some((host, rest)) = next.split_once('\\')
        && url.set_host(Some(host)).is_ok()
      {
        path_str = rest.to_string().into();
      }
    } else if let Some(next) = path_str.strip_prefix(r#"\\?\"#) {
      path_str = next.to_string().into();
    } else if let Some(next) = path_str.strip_prefix(r#"\\"#)
      && let Some((host, rest)) = next.split_once('\\')
      && url.set_host(Some(host)).is_ok()
    {
      path_str = rest.to_string().into();
    }

    for component in path_str.split('\\') {
      url.path_segments_mut().unwrap().push(component);
    }

    Ok(url)
  } else {
    let mut url = Url::parse("file://").unwrap();
    for component in path.components() {
      match component {
        Component::RootDir => {
          url.path_segments_mut().unwrap().push("");
        }
        Component::Normal(segment) => {
          url
            .path_segments_mut()
            .unwrap()
            .push(&segment.to_string_lossy());
        }
        Component::Prefix(_) | Component::CurDir | Component::ParentDir => {
          return Err(());
        }
      }
    }

    Ok(url)
  }
}

#[cfg(not(windows))]
#[inline]
pub fn strip_unc_prefix(path: PathBuf) -> PathBuf {
  path
}

/// Strips the unc prefix (ex. \\?\) from Windows paths.
#[cfg(windows)]
pub fn strip_unc_prefix(path: PathBuf) -> PathBuf {
  use std::path::Component;
  use std::path::Prefix;

  let mut components = path.components();
  match components.next() {
    Some(Component::Prefix(prefix)) => {
      match prefix.kind() {
        // \\?\device
        Prefix::Verbatim(device) => {
          let mut path = PathBuf::new();
          path.push(format!(r"\\{}\", device.to_string_lossy()));
          path.extend(components.filter(|c| !matches!(c, Component::RootDir)));
          path
        }
        // \\?\c:\path
        Prefix::VerbatimDisk(_) => {
          let mut path = PathBuf::new();
          path.push(prefix.as_os_str().to_string_lossy().replace(r"\\?\", ""));
          path.extend(components);
          path
        }
        // \\?\UNC\hostname\share_name\path
        Prefix::VerbatimUNC(hostname, share_name) => {
          let mut path = PathBuf::new();
          path.push(format!(
            r"\\{}\{}\",
            hostname.to_string_lossy(),
            share_name.to_string_lossy()
          ));
          path.extend(components.filter(|c| !matches!(c, Component::RootDir)));
          path
        }
        _ => path,
      }
    }
    _ => path,
  }
}

/// Returns true if the input string starts with a sequence of characters
/// that could be a valid URI scheme, like 'https:', 'git+ssh:' or 'data:'.
///
/// According to RFC 3986 (https://tools.ietf.org/html/rfc3986#section-3.1),
/// a valid scheme has the following format:
///   scheme = ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )
///
/// We additionally require the scheme to be at least 2 characters long,
/// because otherwise a windows path like c:/foo would be treated as a URL,
/// while no schemes with a one-letter name actually exist.
pub fn specifier_has_uri_scheme(specifier: &str) -> bool {
  let mut chars = specifier.chars();
  let mut len = 0usize;
  // The first character must be a letter.
  match chars.next() {
    Some(c) if c.is_ascii_alphabetic() => len += 1,
    _ => return false,
  }
  // Second and following characters must be either a letter, number,
  // plus sign, minus sign, or dot.
  loop {
    match chars.next() {
      Some(c) if c.is_ascii_alphanumeric() || "+-.".contains(c) => len += 1,
      Some(':') if len >= 2 => return true,
      _ => return false,
    }
  }
}

#[derive(Debug, Clone, Error, JsError, PartialEq, Eq)]
pub enum ResolveUrlOrPathError {
  #[error(transparent)]
  #[class(inherit)]
  UrlParse(url::ParseError),
  #[error(transparent)]
  #[class(inherit)]
  PathToUrl(PathToUrlError),
}

/// Takes a string representing either an absolute URL or a file path,
/// as it may be passed to deno as a command line argument.
/// The string is interpreted as a URL if it starts with a valid URI scheme,
/// e.g. 'http:' or 'file:' or 'git+ssh:'. If not, it's interpreted as a
/// file path; if it is a relative path it's resolved relative to passed
/// `current_dir`.
pub fn resolve_url_or_path(
  specifier: &str,
  current_dir: &Path,
) -> Result<Url, ResolveUrlOrPathError> {
  if specifier_has_uri_scheme(specifier) {
    Url::parse(specifier).map_err(ResolveUrlOrPathError::UrlParse)
  } else {
    resolve_path(specifier, current_dir)
      .map_err(ResolveUrlOrPathError::PathToUrl)
  }
}

/// Converts a string representing a relative or absolute path into a
/// ModuleSpecifier. A relative path is considered relative to the passed
/// `current_dir`.
pub fn resolve_path(
  path_str: &str,
  current_dir: &Path,
) -> Result<Url, PathToUrlError> {
  let path = current_dir.join(path_str);
  url_from_file_path(&path)
}

#[derive(Debug, Error, Clone, PartialEq, Eq, deno_error::JsError)]
pub enum SpecifierError {
  // don't make this error a source because it's short
  // and that causes unnecessary verbosity
  #[class(inherit)]
  #[error("invalid URL: {0}")]
  InvalidUrl(url::ParseError),
  #[class(type)]
  #[error("Import \"{specifier}\" not a dependency")]
  ImportPrefixMissing { specifier: String },
}

/// Given a specifier string and a referring module specifier, try to resolve
/// the target module specifier, erroring if it cannot be resolved.
///
/// This function is useful for resolving specifiers in situations without an
/// import map.
pub fn resolve_import(
  specifier: &str,
  referrer: &Url,
) -> Result<Url, SpecifierError> {
  match Url::parse(specifier) {
    // 1. Apply the URL parser to specifier.
    //    If the result is not failure, return the result.
    Ok(url) => Ok(url),

    // 2. If specifier does not start with the character U+002F SOLIDUS (/),
    //    the two-character sequence U+002E FULL STOP, U+002F SOLIDUS (./),
    //    or the three-character sequence U+002E FULL STOP, U+002E FULL STOP,
    //    U+002F SOLIDUS (../), return failure.
    Err(url::ParseError::RelativeUrlWithoutBase)
      if !(specifier.starts_with('/')
        || specifier.starts_with("./")
        || specifier.starts_with("../")) =>
    {
      Err(SpecifierError::ImportPrefixMissing {
        specifier: specifier.to_string(),
      })
    }

    // 3. Return the result of applying the URL parser to specifier with base
    //    URL as the base URL.
    Err(url::ParseError::RelativeUrlWithoutBase) => {
      referrer.join(specifier).map_err(SpecifierError::InvalidUrl)
    }

    // If parsing the specifier as a URL failed for a different reason than
    // it being relative, always return the original error. We don't want to
    // return `ImportPrefixMissing` or `InvalidBaseUrl` if the real
    // problem lies somewhere else.
    Err(err) => Err(SpecifierError::InvalidUrl(err)),
  }
}

pub fn get_atomic_path(sys: &impl SystemRandom, path: &Path) -> PathBuf {
  let rand = gen_rand_path_component(sys);
  let extension = format!("{rand}.tmp");
  path.with_extension(extension)
}

fn gen_rand_path_component(sys: &impl SystemRandom) -> String {
  use std::fmt::Write;
  (0..4).fold(String::with_capacity(8), |mut output, _| {
    write!(&mut output, "{:02x}", sys.sys_random_u8().unwrap()).unwrap();
    output
  })
}

pub fn is_relative_specifier(specifier: &str) -> bool {
  let mut specifier_chars = specifier.chars();
  let Some(first_char) = specifier_chars.next() else {
    return false;
  };
  if first_char != '.' {
    return false;
  }
  let Some(second_char) = specifier_chars.next() else {
    return true;
  };
  if second_char == '/' {
    return true;
  }
  let Some(third_char) = specifier_chars.next() else {
    return second_char == '.';
  };
  second_char == '.' && third_char == '/'
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_url_parent() {
    run_test("file:///", "file:///");
    run_test("file:///test", "file:///");
    run_test("file:///test/", "file:///");
    run_test("file:///test/other", "file:///test/");
    run_test("file:///test/other.txt", "file:///test/");
    run_test("file:///test/other/", "file:///test/");

    fn run_test(url: &str, expected: &str) {
      let result = url_parent(&Url::parse(url).unwrap());
      assert_eq!(result.to_string(), expected);
    }
  }

  #[test]
  fn test_url_to_file_path() {
    run_success_test("file:///", "/");
    run_success_test("file:///test", "/test");
    run_success_test("file:///dir/test/test.txt", "/dir/test/test.txt");
    #[cfg(not(debug_assertions))]
    run_success_test(
      "file:///dir/test%20test/test.txt",
      "/dir/test test/test.txt",
    );

    assert_no_panic_url_to_file_path("file:/");
    assert_no_panic_url_to_file_path("file://");
    #[cfg(not(debug_assertions))]
    assert_no_panic_url_to_file_path("file://asdf/");
    assert_no_panic_url_to_file_path("file://asdf/66666/a.ts");

    #[track_caller]
    fn run_success_test(url: &str, expected_path: &str) {
      let result = url_to_file_path(&Url::parse(url).unwrap()).unwrap();
      assert_eq!(result, PathBuf::from(expected_path));
    }

    #[track_caller]
    fn assert_no_panic_url_to_file_path(url: &str) {
      let _result = url_to_file_path(&Url::parse(url).unwrap());
    }
  }

  #[test]
  fn test_url_to_file_path_wasm() {
    #[track_caller]
    fn convert(path: &str) -> String {
      url_to_file_path_wasm(&Url::parse(path).unwrap())
        .unwrap()
        .to_string_lossy()
        .into_owned()
    }

    assert_eq!(convert("file:///a/b/c.json"), "/a/b/c.json");
    assert_eq!(convert("file:///D:/test/other.json"), "D:/test/other.json");
    assert_eq!(
      convert("file:///path%20with%20spaces/and%23special%25chars!.json"),
      "/path with spaces/and#special%chars!.json",
    );
    assert_eq!(
      convert("file:///C:/My%20Documents/file.txt"),
      "C:/My Documents/file.txt"
    );
    assert_eq!(
      convert("file:///a/b/%D0%BF%D1%80%D0%B8%D0%BC%D0%B5%D1%80.txt"),
      "/a/b/пример.txt"
    );
    assert_eq!(
      convert("file://server/share/folder/file.txt"),
      "\\\\server\\share\\folder\\file.txt"
    );
  }

  #[test]
  fn test_url_from_file_path_wasm() {
    #[track_caller]
    fn convert(path: &str) -> String {
      url_from_file_path_wasm(Path::new(path))
        .unwrap()
        .to_string()
    }

    assert_eq!(convert("/a/b/c.json"), "file:///a/b/c.json");
    assert_eq!(
      convert("D:\\test\\other.json"),
      "file:///D:/test/other.json"
    );
    assert_eq!(
      convert("/path with spaces/and#special%chars!.json"),
      "file:///path%20with%20spaces/and%23special%25chars!.json"
    );
    assert_eq!(
      convert("C:\\My Documents\\file.txt"),
      "file:///C:/My%20Documents/file.txt"
    );
    assert_eq!(
      convert("/a/b/пример.txt"),
      "file:///a/b/%D0%BF%D1%80%D0%B8%D0%BC%D0%B5%D1%80.txt"
    );
    assert_eq!(
      convert("\\\\server\\share\\folder\\file.txt"),
      "file://server/share/folder/file.txt"
    );
    assert_eq!(convert(r#"\\?\UNC\server\share"#), "file://server/share");
    assert_eq!(
      convert(r"\\?\cat_pics\subfolder\file.jpg"),
      "file:///cat_pics/subfolder/file.jpg"
    );
    assert_eq!(convert(r"\\?\cat_pics"), "file:///cat_pics");
  }

  #[test]
  fn test_url_from_directory_path_wasm() {
    #[track_caller]
    fn convert(path: &str) -> String {
      url_from_directory_path_wasm(Path::new(path))
        .unwrap()
        .to_string()
    }

    assert_eq!(convert("/a/b/c"), "file:///a/b/c/");
    assert_eq!(convert("D:\\test\\other"), "file:///D:/test/other/");
  }

  #[cfg(windows)]
  #[test]
  fn test_strip_unc_prefix() {
    use std::path::PathBuf;

    run_test(r"C:\", r"C:\");
    run_test(r"C:\test\file.txt", r"C:\test\file.txt");

    run_test(r"\\?\C:\", r"C:\");
    run_test(r"\\?\C:\test\file.txt", r"C:\test\file.txt");

    run_test(r"\\.\C:\", r"\\.\C:\");
    run_test(r"\\.\C:\Test\file.txt", r"\\.\C:\Test\file.txt");

    run_test(r"\\?\UNC\localhost\", r"\\localhost");
    run_test(r"\\?\UNC\localhost\c$\", r"\\localhost\c$");
    run_test(
      r"\\?\UNC\localhost\c$\Windows\file.txt",
      r"\\localhost\c$\Windows\file.txt",
    );
    run_test(r"\\?\UNC\wsl$\deno.json", r"\\wsl$\deno.json");

    run_test(r"\\?\server1", r"\\server1");
    run_test(r"\\?\server1\e$\", r"\\server1\e$\");
    run_test(
      r"\\?\server1\e$\test\file.txt",
      r"\\server1\e$\test\file.txt",
    );

    fn run_test(input: &str, expected: &str) {
      assert_eq!(
        super::strip_unc_prefix(PathBuf::from(input)),
        PathBuf::from(expected)
      );
    }
  }

  #[test]
  fn test_normalize_path_basic() {
    let run_test = run_normalize_path_test;
    run_test("a/../b", "b");
    run_test("a/./b/", &PathBuf::from("a").join("b").to_string_lossy());
    run_test(
      "a/./b/../c",
      &PathBuf::from("a").join("c").to_string_lossy(),
    );
  }

  #[cfg(windows)]
  #[test]
  fn test_normalize_path_win() {
    let run_test = run_normalize_path_test;

    run_test("C:\\test\\file.txt", "C:\\test\\file.txt");
    run_test("C:\\test\\./file.txt", "C:\\test\\file.txt");
    run_test("C:\\test\\../other/file.txt", "C:\\other\\file.txt");
    run_test("C:\\test\\../other\\file.txt", "C:\\other\\file.txt");
    run_test(
      "C:\\test\\removes_trailing_slash\\",
      "C:\\test\\removes_trailing_slash",
    );
    run_test("C:\\a\\.\\b\\..\\c", "C:\\a\\c");
    run_test("C:\\test\\.", "C:\\test");
    run_test("C:\\test\\test...", "C:\\test\\test");
    run_test("C:\\test\\test    ", "C:\\test\\test");
  }

  #[track_caller]
  fn run_normalize_path_test(input: &str, expected: &str) {
    assert_eq!(
      normalize_path(Cow::Owned(PathBuf::from(input))).to_string_lossy(),
      expected
    );
  }

  #[test]
  fn test_atomic_path() {
    let sys = sys_traits::impls::InMemorySys::default();
    sys.set_seed(Some(10));
    let path = Path::new("/a/b/c.txt");
    let atomic_path = get_atomic_path(&sys, path);
    assert_eq!(atomic_path.parent().unwrap(), path.parent().unwrap());
    assert_eq!(atomic_path.file_name().unwrap(), "c.3d3d3d3d.tmp");
  }

  #[test]
  fn test_specifier_has_uri_scheme() {
    let tests = vec![
      ("http://foo.bar/etc", true),
      ("HTTP://foo.bar/etc", true),
      ("http:ftp:", true),
      ("http:", true),
      ("hTtP:", true),
      ("ftp:", true),
      ("mailto:spam@please.me", true),
      ("git+ssh://git@github.com/denoland/deno", true),
      ("blob:https://whatwg.org/mumbojumbo", true),
      ("abc.123+DEF-ghi:", true),
      ("abc.123+def-ghi:@", true),
      ("", false),
      (":not", false),
      ("http", false),
      ("c:dir", false),
      ("X:", false),
      ("./http://not", false),
      ("1abc://kinda/but/no", false),
      ("schluẞ://no/more", false),
    ];

    for (specifier, expected) in tests {
      let result = specifier_has_uri_scheme(specifier);
      assert_eq!(result, expected);
    }
  }

  #[test]
  fn test_resolve_url_or_path() {
    // Absolute URL.
    let mut tests: Vec<(&str, String)> = vec![
      (
        "http://deno.land/core/tests/006_url_imports.ts",
        "http://deno.land/core/tests/006_url_imports.ts".to_string(),
      ),
      (
        "https://deno.land/core/tests/006_url_imports.ts",
        "https://deno.land/core/tests/006_url_imports.ts".to_string(),
      ),
    ];

    // The local path tests assume that the cwd is the deno repo root. Note
    // that we can't use `cwd` in miri tests, so we just use `/miri` instead.
    let cwd = if cfg!(miri) {
      PathBuf::from("/miri")
    } else {
      std::env::current_dir().unwrap()
    };
    let cwd_str = cwd.to_str().unwrap();

    if cfg!(target_os = "windows") {
      // Absolute local path.
      let expected_url = "file:///C:/deno/tests/006_url_imports.ts";
      tests.extend(vec![
        (
          r"C:/deno/tests/006_url_imports.ts",
          expected_url.to_string(),
        ),
        (
          r"C:\deno\tests\006_url_imports.ts",
          expected_url.to_string(),
        ),
        (
          r"\\?\C:\deno\tests\006_url_imports.ts",
          expected_url.to_string(),
        ),
        // Not supported: `Url::from_file_path()` fails.
        // (r"\\.\C:\deno\tests\006_url_imports.ts", expected_url.to_string()),
        // Not supported: `Url::from_file_path()` performs the wrong conversion.
        // (r"//./C:/deno/tests/006_url_imports.ts", expected_url.to_string()),
      ]);

      // Rooted local path without drive letter.
      let expected_url = format!(
        "file:///{}:/deno/tests/006_url_imports.ts",
        cwd_str.get(..1).unwrap(),
      );
      tests.extend(vec![
        (r"/deno/tests/006_url_imports.ts", expected_url.to_string()),
        (r"\deno\tests\006_url_imports.ts", expected_url.to_string()),
        (
          r"\deno\..\deno\tests\006_url_imports.ts",
          expected_url.to_string(),
        ),
        (r"\deno\.\tests\006_url_imports.ts", expected_url),
      ]);

      // Relative local path.
      let expected_url = format!(
        "file:///{}/tests/006_url_imports.ts",
        cwd_str.replace('\\', "/")
      );
      tests.extend(vec![
        (r"tests/006_url_imports.ts", expected_url.to_string()),
        (r"tests\006_url_imports.ts", expected_url.to_string()),
        (r"./tests/006_url_imports.ts", (*expected_url).to_string()),
        (r".\tests\006_url_imports.ts", (*expected_url).to_string()),
      ]);

      // UNC network path.
      let expected_url = "file://server/share/deno/cool";
      tests.extend(vec![
        (r"\\server\share\deno\cool", expected_url.to_string()),
        (r"\\server/share/deno/cool", expected_url.to_string()),
        // Not supported: `Url::from_file_path()` performs the wrong conversion.
        // (r"//server/share/deno/cool", expected_url.to_string()),
      ]);
    } else {
      // Absolute local path.
      let expected_url = "file:///deno/tests/006_url_imports.ts";
      tests.extend(vec![
        ("/deno/tests/006_url_imports.ts", expected_url.to_string()),
        ("//deno/tests/006_url_imports.ts", expected_url.to_string()),
      ]);

      // Relative local path.
      let expected_url = format!("file://{cwd_str}/tests/006_url_imports.ts");
      tests.extend(vec![
        ("tests/006_url_imports.ts", expected_url.to_string()),
        ("./tests/006_url_imports.ts", expected_url.to_string()),
        (
          "tests/../tests/006_url_imports.ts",
          expected_url.to_string(),
        ),
        ("tests/./006_url_imports.ts", expected_url),
      ]);
    }

    for (specifier, expected_url) in tests {
      let url = resolve_url_or_path(specifier, &cwd).unwrap().to_string();
      assert_eq!(url, expected_url);
    }
  }

  #[test]
  fn test_resolve_url_or_path_deprecated_error() {
    use ResolveUrlOrPathError::*;
    use url::ParseError::*;

    let mut tests = vec![
      ("https://eggplant:b/c", UrlParse(InvalidPort)),
      ("https://:8080/a/b/c", UrlParse(EmptyHost)),
    ];
    if cfg!(target_os = "windows") {
      let p = r"\\.\c:/stuff/deno/script.ts";
      tests.push((p, PathToUrl(PathToUrlError(PathBuf::from(p)))));
    }

    for (specifier, expected_err) in tests {
      let err =
        resolve_url_or_path(specifier, &PathBuf::from("/")).unwrap_err();
      assert_eq!(err, expected_err);
    }
  }

  #[test]
  fn test_is_relative_specifier() {
    assert!(is_relative_specifier("."));
    assert!(is_relative_specifier(".."));
    assert!(is_relative_specifier("../"));
    assert!(is_relative_specifier("../test"));
    assert!(is_relative_specifier("./"));
    assert!(is_relative_specifier("./test"));
    assert!(!is_relative_specifier(""));
    assert!(!is_relative_specifier("a"));
    assert!(!is_relative_specifier(".test"));
    assert!(!is_relative_specifier("..test"));
    assert!(!is_relative_specifier("/test"));
    assert!(!is_relative_specifier("test"));
  }

  #[test]
  fn test_resolve_import() {
    fn run_test(specifier: &str, base: &str, expected: &str) {
      let actual =
        resolve_import(specifier, &Url::parse(base).unwrap()).unwrap();
      assert_eq!(actual.as_str(), expected);
    }

    run_test(
      "./test.js",
      "https://example.com",
      "https://example.com/test.js",
    );
    run_test(
      "https://deno.land/x/mod.ts",
      "https://example.com",
      "https://deno.land/x/mod.ts",
    );
    run_test(
      "../test.js",
      "https://example.com/sub",
      "https://example.com/test.js",
    );
    run_test(
      "/test.js",
      "https://example.com/sub/dir/test",
      "https://example.com/test.js",
    );

    match resolve_import("test.js", &Url::parse("https://example.com").unwrap())
    {
      Err(SpecifierError::ImportPrefixMissing { specifier }) => {
        assert_eq!(specifier, "test.js");
      }
      _ => unreachable!(),
    }
  }
}
