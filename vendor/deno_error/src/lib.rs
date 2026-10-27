// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
#![deny(clippy::unnecessary_wraps)]
#![deny(clippy::print_stderr)]
#![deny(clippy::print_stdout)]

//! Trait and macros to represent Rust errors in JavaScript.
//!
//! ## The [`JsError`] macro
//!
//! Macro to define the `JsErrorClass` trait on a struct or enum.
//!
//! The macro does not provide functionality to related to the `get_message`
//! function, as one can combine the [`thiserror`](https://crates.io/crates/thiserror) well with this macro.
//!
//! ### Attributes
//!
//! #### `#[class]`
//! This attribute accepts 3 possible kinds of value:
//!   1. `GENERIC`, `TYPE`, and a few more that are defined in the `builtin_classes`
//!      module, without the `_ERROR` suffix.
//!   2. A text value ie `"NotFound"`. If a text value is passed that is a valid
//!      builtin (see the previous point), it will error out as the special
//!      identifiers are preferred to avoid mistakes.
//!   3. `inherit`: this will inherit the class from whatever field is marked with
//!      the `#[inherit]` attribute. Alternatively, the `#[inherit]` attribute
//!      can be omitted if only one field is present in the enum variant or struct.
//!      This value is inferred if the class attribute is missing and only a single
//!      field is present on a struct, however for enums this inferring is not done.
//!
//! #### `#[property]`
//! This attribute allows defining fields as additional properties that should be
//! defined on the JavaScript error.
//!
//! The type of the field needs to implement a `.to_string()` function for it
//! being able to be inherited.
//!
//! #### `#[inherit]`
//! This attribute allows defining a field that should be used to inherit the class
//! and properties.
//!
//! This is inferred if only one field is present in the enum variant or struct.
//!
//! The class is only inherited if the `class` attribute is set to `inherit`.
//!
//! ### Examples
//!
//! #### Basic usage
//! ```
//! #[derive(Debug, thiserror::Error, deno_error::JsError)]
//! pub enum SomeError {
//!   #[class(generic)]
//!   #[error("Failure")]
//!   Failure,
//!   #[class(inherit)]
//!   #[error(transparent)]
//!   Io(#[inherit] std::io::Error),
//! }
//! ```
//!
//! #### Top-level class
//! ```
//! #[derive(Debug, thiserror::Error, deno_error::JsError)]
//! #[class(generic)]
//! pub enum SomeError {
//!   #[error("Failure")]
//!   Failure,
//!   #[class(inherit)] // overwrite the top-level
//!   #[error(transparent)]
//!   Io(#[inherit] std::io::Error),
//! }
//! ```
//!
//! #### Defining properties
//! ```
//! #[derive(Debug, thiserror::Error, deno_error::JsError)]
//! #[class(generic)]
//! pub enum SomeError {
//!   #[class(not_supported)]
//!   #[error("Failure")]
//!   Failure {
//!     #[property]
//!     code: u32,
//!   },
//!   #[error("Warning")]
//!   Warning(#[property = "code"] u32),
//!   #[class(inherit)] // inherit properties from `std::io::Error`
//!   #[error(transparent)]
//!   Io(#[inherit] std::io::Error),
//! }
//! ```
//!
//! ##### Defining external properties
//! ```
//! #[derive(Debug, thiserror::Error, deno_error::JsError)]
//! #[property("code" = 10)]
//! #[property("kind" = self.get_name())]
//! #[class(generic)]
//! #[error(transparent)]
//! pub struct SomeError(std::io::Error);
//!
//! impl SomeError {
//!   fn get_name(&self) -> String {
//!     self.0.kind().to_string()
//!   }
//! }
//! ```
//!
//! #### Explicit property inheritance
//!
//! ```
//! #[derive(Debug, thiserror::Error, deno_error::JsError)]
//! #[class(generic)]
//! #[properties(inherit)]
//! #[error(transparent)]
//! pub struct SomeError(std::io::Error);
//! ```
//!
//! ```
//! #[derive(Debug, thiserror::Error, deno_error::JsError)]
//! #[class(inherit)]
//! #[properties(no_inherit)]
//! #[error(transparent)]
//! pub struct SomeError(std::io::Error);
//! ```
//!
//! #### Inferred inheritance
//! ```
//! #[derive(Debug, thiserror::Error, deno_error::JsError)]
//! #[error("My io error")]
//! pub struct SomeError(std::io::Error);
//! ```
//!
//! ```
//! #[derive(Debug, thiserror::Error, deno_error::JsError)]
//! #[class(inherit)]
//! #[error("My io error")]
//! pub struct SomeError(std::io::Error);
//! ```
//!
//! ```
//! #[derive(Debug, thiserror::Error, deno_error::JsError)]
//! #[class(generic)] // don't inherit the error
//! #[error("My io error")]
//! pub struct SomeError(std::io::Error);
//! ```
//!
//! ```
//! #[derive(Debug, thiserror::Error, deno_error::JsError)]
//! #[class(type)]
//! pub enum SomeError {
//!   #[error("Failure")]
//!   Failure,
//!   #[class(inherit)]
//!   #[error(transparent)]
//!   Io(std::io::Error),
//! }
//! ```

mod error_codes;

pub use deno_error_macro::*;
pub use error_codes::*;
use std::any::Any;
use std::borrow::Cow;

/// Various built-in error classes, mainly related to the JavaScript specification.
/// May include some error classes that are non-standard.
pub mod builtin_classes {
  // keep in sync with macros/lib.rs
  pub const GENERIC_ERROR: &str = "Error";
  pub const RANGE_ERROR: &str = "RangeError";
  pub const TYPE_ERROR: &str = "TypeError";
  pub const SYNTAX_ERROR: &str = "SyntaxError";
  pub const URI_ERROR: &str = "URIError";
  pub const REFERENCE_ERROR: &str = "ReferenceError";

  /// Non-standard
  pub const NOT_SUPPORTED_ERROR: &str = "NotSupported";
}
use builtin_classes::*;

/// Represents a property value that can be either a string or a number
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
  String(Cow<'static, str>),
  Number(f64),
}

impl std::fmt::Display for PropertyValue {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      PropertyValue::String(s) => write!(f, "{}", s),
      PropertyValue::Number(n) => write!(f, "{}", n),
    }
  }
}

impl From<String> for PropertyValue {
  fn from(s: String) -> Self {
    PropertyValue::String(Cow::Owned(s))
  }
}

impl From<&'static str> for PropertyValue {
  fn from(s: &'static str) -> Self {
    PropertyValue::String(Cow::Borrowed(s))
  }
}

impl From<f64> for PropertyValue {
  fn from(n: f64) -> Self {
    PropertyValue::Number(n)
  }
}

impl From<&f64> for PropertyValue {
  fn from(n: &f64) -> Self {
    PropertyValue::Number(*n)
  }
}

impl From<i32> for PropertyValue {
  fn from(n: i32) -> Self {
    PropertyValue::Number(n as f64)
  }
}

impl From<&i32> for PropertyValue {
  fn from(n: &i32) -> Self {
    PropertyValue::Number(*n as f64)
  }
}

impl From<u32> for PropertyValue {
  fn from(n: u32) -> Self {
    PropertyValue::Number(n as f64)
  }
}

impl From<&u32> for PropertyValue {
  fn from(n: &u32) -> Self {
    PropertyValue::Number(*n as f64)
  }
}

pub type AdditionalProperties =
  Box<dyn Iterator<Item = (Cow<'static, str>, PropertyValue)>>;

/// Trait to implement how an error should be represented in JavaScript.
///
/// **Note**:
/// it is not recommended to manually implement this type, but instead
/// rather use the [`JsError`] macro.
pub trait JsErrorClass:
  std::error::Error + Send + Sync + Any + 'static
{
  /// Represents the error class used in JavaScript side.
  fn get_class(&self) -> Cow<'static, str>;

  /// Represents the error message used in JavaScript side.
  fn get_message(&self) -> Cow<'static, str>;

  /// Additional properties that should be defined on the error in JavaScript side.
  fn get_additional_properties(&self) -> AdditionalProperties;

  fn get_ref(&self) -> &(dyn std::error::Error + Send + Sync + 'static);
}

/// Macro which lets you wrap an existing error in a new error that implements
/// the [`JsErrorClass`] trait. This macro however does currently not support
/// the special identifiers that the [`JsError`] macro supports.
///
/// ## Examples
///
/// ```rust
/// # use deno_error::js_error_wrapper;
/// js_error_wrapper!(std::net::AddrParseError, JsAddrParseError, "TypeError");
/// ```
///
/// ```rust
/// # use deno_error::js_error_wrapper;
/// js_error_wrapper!(std::net::AddrParseError, JsAddrParseError, |err| {
///   // match or do some logic to get the error class
///   "TypeError"
/// });
/// ```
#[macro_export]
macro_rules! js_error_wrapper {
  ($err_path:path, $err_name:ident, $js_err_type:tt) => {
    deno_error::js_error_wrapper!($err_path, $err_name, |_error| $js_err_type);
  };
  ($err_path:path, $err_name:ident, |$inner:ident| $js_err_type:tt) => {
    #[derive(Debug)]
    pub struct $err_name(pub $err_path);
    impl From<$err_path> for $err_name {
      fn from(err: $err_path) -> Self {
        Self(err)
      }
    }
    impl $err_name {
      pub fn get_error_class(
        $inner: &$err_path,
      ) -> impl Into<std::borrow::Cow<'static, str>> {
        $js_err_type
      }
    }
    impl std::error::Error for $err_name {
      fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        std::error::Error::source(&self.0)
      }
    }
    impl std::fmt::Display for $err_name {
      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
      }
    }
    impl deno_error::JsErrorClass for $err_name {
      fn get_class(&self) -> std::borrow::Cow<'static, str> {
        Self::get_error_class(&self.0).into()
      }
      fn get_message(&self) -> std::borrow::Cow<'static, str> {
        self.to_string().into()
      }
      fn get_additional_properties(&self) -> deno_error::AdditionalProperties {
        Box::new(std::iter::empty())
      }
      fn get_ref(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
        self
      }
    }
    impl std::ops::Deref for $err_name {
      type Target = $err_path;

      fn deref(&self) -> &Self::Target {
        &self.0
      }
    }
  };
}

impl<T: JsErrorClass> JsErrorClass for Box<T> {
  fn get_class(&self) -> Cow<'static, str> {
    (**self).get_class()
  }

  fn get_message(&self) -> Cow<'static, str> {
    (**self).get_message()
  }

  fn get_additional_properties(&self) -> AdditionalProperties {
    (**self).get_additional_properties()
  }

  fn get_ref(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
    self
  }
}

impl JsErrorClass for std::io::Error {
  fn get_class(&self) -> Cow<'static, str> {
    use std::io::ErrorKind::*;

    let class = match self.kind() {
      NotFound => "NotFound",
      PermissionDenied => "PermissionDenied",
      ConnectionRefused => "ConnectionRefused",
      ConnectionReset => "ConnectionReset",
      ConnectionAborted => "ConnectionAborted",
      NotConnected => "NotConnected",
      AddrInUse => "AddrInUse",
      AddrNotAvailable => "AddrNotAvailable",
      BrokenPipe => "BrokenPipe",
      AlreadyExists => "AlreadyExists",
      InvalidInput => TYPE_ERROR,
      InvalidData => "InvalidData",
      TimedOut => "TimedOut",
      Interrupted => "Interrupted",
      WriteZero => "WriteZero",
      UnexpectedEof => "UnexpectedEof",
      Other => GENERIC_ERROR,
      WouldBlock => "WouldBlock",
      IsADirectory => "IsADirectory",
      NetworkUnreachable => "NetworkUnreachable",
      NotADirectory => "NotADirectory",
      kind => match format!("{kind:?}").as_str() {
        "FilesystemLoop" => "FilesystemLoop",
        _ => GENERIC_ERROR,
      },
    };

    Cow::Borrowed(class)
  }

  fn get_message(&self) -> Cow<'static, str> {
    self.to_string().into()
  }

  fn get_additional_properties(&self) -> AdditionalProperties {
    if let Some(code) = get_error_code(self) {
      Box::new(std::iter::once((
        "code".into(),
        PropertyValue::String(code.into()),
      )))
    } else {
      Box::new(Box::new(std::iter::empty()))
    }
  }

  fn get_ref(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
    self
  }
}

impl JsErrorClass for std::env::VarError {
  fn get_class(&self) -> Cow<'static, str> {
    Cow::Borrowed(match self {
      std::env::VarError::NotPresent => "NotFound",
      std::env::VarError::NotUnicode(..) => "InvalidData",
    })
  }

  fn get_message(&self) -> Cow<'static, str> {
    self.to_string().into()
  }

  fn get_additional_properties(&self) -> AdditionalProperties {
    Box::new(std::iter::empty())
  }

  fn get_ref(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
    self
  }
}

impl JsErrorClass for std::sync::mpsc::RecvError {
  fn get_class(&self) -> Cow<'static, str> {
    Cow::Borrowed(GENERIC_ERROR)
  }

  fn get_message(&self) -> Cow<'static, str> {
    self.to_string().into()
  }

  fn get_additional_properties(&self) -> AdditionalProperties {
    Box::new(std::iter::empty())
  }

  fn get_ref(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
    self
  }
}

impl JsErrorClass for std::str::Utf8Error {
  fn get_class(&self) -> Cow<'static, str> {
    Cow::Borrowed(GENERIC_ERROR)
  }

  fn get_message(&self) -> Cow<'static, str> {
    self.to_string().into()
  }

  fn get_additional_properties(&self) -> AdditionalProperties {
    Box::new(std::iter::empty())
  }

  fn get_ref(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
    self
  }
}

impl JsErrorClass for std::num::TryFromIntError {
  fn get_class(&self) -> Cow<'static, str> {
    Cow::Borrowed(TYPE_ERROR)
  }

  fn get_message(&self) -> Cow<'static, str> {
    self.to_string().into()
  }

  fn get_additional_properties(&self) -> AdditionalProperties {
    Box::new(std::iter::empty())
  }

  fn get_ref(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
    self
  }
}

impl JsErrorClass for std::convert::Infallible {
  fn get_class(&self) -> Cow<'static, str> {
    unreachable!()
  }

  fn get_message(&self) -> Cow<'static, str> {
    unreachable!()
  }

  fn get_additional_properties(&self) -> AdditionalProperties {
    unreachable!();
  }

  fn get_ref(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
    unreachable!()
  }
}

#[cfg(all(feature = "serde", feature = "serde_json"))]
impl JsErrorClass for serde_json::Error {
  fn get_class(&self) -> Cow<'static, str> {
    use serde::de::StdError;
    use serde_json::error::*;

    match self.classify() {
      Category::Io => self
        .source()
        .and_then(|e| e.downcast_ref::<std::io::Error>())
        .map(|e| e.get_class())
        .unwrap_or_else(|| Cow::Borrowed("Error")),
      Category::Syntax => Cow::Borrowed(SYNTAX_ERROR),
      Category::Data => Cow::Borrowed("InvalidData"),
      Category::Eof => Cow::Borrowed("UnexpectedEof"),
    }
  }

  fn get_message(&self) -> Cow<'static, str> {
    self.to_string().into()
  }

  fn get_additional_properties(&self) -> AdditionalProperties {
    Box::new(std::iter::empty()) // TODO: could be io error code
  }

  fn get_ref(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
    self
  }
}

#[cfg(feature = "url")]
impl JsErrorClass for url::ParseError {
  fn get_class(&self) -> Cow<'static, str> {
    Cow::Borrowed(URI_ERROR)
  }

  fn get_message(&self) -> Cow<'static, str> {
    self.to_string().into()
  }

  fn get_additional_properties(&self) -> AdditionalProperties {
    Box::new(std::iter::empty())
  }

  fn get_ref(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
    self
  }
}

#[cfg(feature = "tokio")]
impl<T: Send + Sync + 'static> JsErrorClass
  for tokio::sync::mpsc::error::SendError<T>
{
  fn get_class(&self) -> Cow<'static, str> {
    Cow::Borrowed(GENERIC_ERROR)
  }

  fn get_message(&self) -> Cow<'static, str> {
    self.to_string().into()
  }

  fn get_additional_properties(&self) -> AdditionalProperties {
    Box::new(std::iter::empty())
  }

  fn get_ref(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
    self
  }
}

#[cfg(feature = "tokio")]
impl JsErrorClass for tokio::task::JoinError {
  fn get_class(&self) -> Cow<'static, str> {
    Cow::Borrowed(GENERIC_ERROR)
  }

  fn get_message(&self) -> Cow<'static, str> {
    self.to_string().into()
  }

  fn get_additional_properties(&self) -> AdditionalProperties {
    Box::new(std::iter::empty())
  }

  fn get_ref(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
    self
  }
}

#[cfg(feature = "tokio")]
impl JsErrorClass for tokio::sync::broadcast::error::RecvError {
  fn get_class(&self) -> Cow<'static, str> {
    Cow::Borrowed(GENERIC_ERROR)
  }

  fn get_message(&self) -> Cow<'static, str> {
    self.to_string().into()
  }

  fn get_additional_properties(&self) -> AdditionalProperties {
    Box::new(std::iter::empty())
  }

  fn get_ref(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
    self
  }
}

enum JsErrorBoxInner {
  Standalone {
    class: Cow<'static, str>,
    message: Cow<'static, str>,
  },
  Wrap(Box<dyn JsErrorClass>),
}

pub struct JsErrorBox(JsErrorBoxInner);

impl std::fmt::Debug for JsErrorBox {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let mut debug = f.debug_struct("JsErrorBox");

    match &self.0 {
      JsErrorBoxInner::Standalone { class, message } => {
        debug.field("class", class);
        debug.field("message", message);
      }
      JsErrorBoxInner::Wrap(inner) => {
        debug.field("inner", inner);
      }
    }

    debug.finish()
  }
}

impl std::fmt::Display for JsErrorBox {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.get_message())
  }
}

impl std::error::Error for JsErrorBox {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match &self.0 {
      JsErrorBoxInner::Standalone { .. } => None,
      JsErrorBoxInner::Wrap(inner) => inner.source(),
    }
  }
}

impl JsErrorClass for JsErrorBox {
  fn get_class(&self) -> Cow<'static, str> {
    match &self.0 {
      JsErrorBoxInner::Standalone { class, .. } => class.clone(),
      JsErrorBoxInner::Wrap(inner) => inner.get_class(),
    }
  }

  fn get_message(&self) -> Cow<'static, str> {
    match &self.0 {
      JsErrorBoxInner::Standalone { message, .. } => message.clone(),
      JsErrorBoxInner::Wrap(inner) => inner.get_message(),
    }
  }

  fn get_additional_properties(&self) -> AdditionalProperties {
    match &self.0 {
      JsErrorBoxInner::Standalone { .. } => Box::new(std::iter::empty()),
      JsErrorBoxInner::Wrap(inner) => inner.get_additional_properties(),
    }
  }

  fn get_ref(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
    match &self.0 {
      JsErrorBoxInner::Standalone { .. } => self,
      JsErrorBoxInner::Wrap(inner) => inner.get_ref(),
    }
  }
}

impl JsErrorBox {
  pub fn new(
    class: impl Into<Cow<'static, str>>,
    message: impl Into<Cow<'static, str>>,
  ) -> Self {
    Self(JsErrorBoxInner::Standalone {
      class: class.into(),
      message: message.into(),
    })
  }

  pub fn from_err<T: JsErrorClass>(err: T) -> Self {
    Self(JsErrorBoxInner::Wrap(Box::new(err)))
  }

  pub fn generic(message: impl Into<Cow<'static, str>>) -> JsErrorBox {
    Self::new(GENERIC_ERROR, message)
  }

  pub fn type_error(message: impl Into<Cow<'static, str>>) -> JsErrorBox {
    Self::new(TYPE_ERROR, message)
  }

  pub fn range_error(message: impl Into<Cow<'static, str>>) -> JsErrorBox {
    Self::new(RANGE_ERROR, message)
  }

  pub fn uri_error(message: impl Into<Cow<'static, str>>) -> JsErrorBox {
    Self::new(URI_ERROR, message)
  }

  // Non-standard errors
  pub fn not_supported() -> JsErrorBox {
    Self::new(NOT_SUPPORTED_ERROR, "The operation is not supported")
  }

  pub fn get_inner_ref(
    &self,
  ) -> Option<&(dyn std::error::Error + Send + Sync + 'static)> {
    match &self.0 {
      JsErrorBoxInner::Standalone { .. } => None,
      JsErrorBoxInner::Wrap(inner) => Some(inner.get_ref()),
    }
  }
}

#[cfg(test)]
mod tests {
  use std::io;

  use super::JsErrorClass;

  #[test]
  fn test_io_error_class_stable() {
    assert_eq!(
      io::Error::new(io::ErrorKind::NotFound, "").get_class(),
      "NotFound",
    );
  }

  #[test]
  #[cfg(unix)]
  fn test_io_error_class_unstable() {
    assert_eq!(
      io::Error::from_raw_os_error(libc::ELOOP).get_class(),
      "FilesystemLoop",
    );
  }
}
