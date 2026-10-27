use std::borrow::Cow;
use std::collections::TryReserveError;
use std::fmt::Write;

#[cfg(feature = "ecow")]
pub mod ecow;
#[cfg(feature = "hipstr")]
pub mod hipstr;

pub use capacity_builder_macros::CapacityDisplay;

macro_rules! count_digits {
  ($value:expr) => {{
    let mut value = $value;
    if value == 0 {
      1
    } else {
      let mut count = 0;
      while value > 0 {
        value /= 10;
        count += 1;
      }
      count
    }
  }};
}

macro_rules! impl_appendable_for_int {
  ($($t:ty),*) => {
    $(
      impl EndianBytesAppendable for $t {
        fn byte_len(&self) -> usize {
          std::mem::size_of::<$t>()
        }

        fn push_le_to<TBytes: BytesTypeMut>(&self, bytes: &mut TBytes) {
          bytes.extend_from_slice(&self.to_le_bytes());
        }

        fn push_be_to<TBytes: BytesTypeMut>(&self, bytes: &mut TBytes) {
          bytes.extend_from_slice(&self.to_be_bytes());
        }
      }

      impl StringAppendableValue for $t {
        fn byte_len(&self) -> usize {
          count_digits!(*self)
        }

        fn push_to<TString: StringTypeMut>(&self, text: &mut TString) {
          // no need to reuse buffers as per the documentation
          // and as found in my benchmarks
          let mut buffer = itoa::Buffer::new();
          let s = buffer.format(*self);
          text.push_str(s);
        }

        #[inline(always)]
        fn write_to_formatter(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
          let mut buffer = itoa::Buffer::new();
          let s = buffer.format(*self);
          fmt.write_str(s)
        }
      }
    )*
  };
}

pub trait StringAppendable<'a> {
  fn append_to_builder<TString: StringType>(
    self,
    builder: &mut StringBuilder<'a, TString>,
  );
}

impl<'a, T> StringAppendable<'a> for T
where
  T: StringAppendableValue,
{
  fn append_to_builder<TString: StringType>(
    self,
    builder: &mut StringBuilder<'a, TString>,
  ) {
    builder.append_value(self);
  }
}

pub trait StringType: Sized {
  type MutType: StringTypeMut;

  fn with_capacity(size: usize) -> Result<Self::MutType, TryReserveError>;
  fn from_mut(inner: Self::MutType) -> Self;
}

#[allow(clippy::len_without_is_empty)]
pub trait StringTypeMut {
  fn push(&mut self, c: char);
  fn push_str(&mut self, str: &str);
  fn len(&self) -> usize;
}

impl StringType for String {
  type MutType = String;

  #[inline(always)]
  fn with_capacity(size: usize) -> Result<Self::MutType, TryReserveError> {
    let mut text = String::new();
    text.try_reserve_exact(size)?;
    Ok(text)
  }

  #[inline(always)]
  fn from_mut(inner: Self::MutType) -> Self {
    inner
  }
}

impl StringTypeMut for String {
  #[inline(always)]
  fn push(&mut self, c: char) {
    String::push(self, c)
  }

  #[inline(always)]
  fn push_str(&mut self, str: &str) {
    String::push_str(self, str)
  }

  #[inline(always)]
  fn len(&self) -> usize {
    String::len(self)
  }
}

impl StringType for Box<str> {
  type MutType = String;

  #[inline(always)]
  fn with_capacity(size: usize) -> Result<Self::MutType, TryReserveError> {
    let mut text = String::new();
    text.try_reserve_exact(size)?;
    Ok(text)
  }

  #[inline(always)]
  fn from_mut(inner: Self::MutType) -> Self {
    inner.into_boxed_str()
  }
}

impl<'a> StringAppendable<'a> for &'a Box<str> {
  #[inline(always)]
  fn append_to_builder<TString: StringType>(
    self,
    builder: &mut StringBuilder<'a, TString>,
  ) {
    builder.append(self.as_ref());
  }
}

pub trait StringAppendableValue {
  fn byte_len(&self) -> usize;
  fn push_to<TString: StringTypeMut>(&self, text: &mut TString);
  fn write_to_formatter(
    &self,
    fmt: &mut std::fmt::Formatter<'_>,
  ) -> std::fmt::Result;
}

pub trait BytesType: Sized {
  type MutType: BytesTypeMut;

  fn with_capacity(size: usize) -> Result<Self::MutType, TryReserveError>;
  fn from_mut(inner: Self::MutType) -> Self;
}

#[allow(clippy::len_without_is_empty)]
pub trait BytesTypeMut: Sized {
  fn push(&mut self, c: u8);
  fn extend_from_slice(&mut self, bytes: &[u8]);
  fn len(&self) -> usize;
}

impl BytesType for Vec<u8> {
  type MutType = Vec<u8>;

  #[inline(always)]
  fn with_capacity(size: usize) -> Result<Self::MutType, TryReserveError> {
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(size)?;
    Ok(bytes)
  }

  #[inline(always)]
  fn from_mut(inner: Self::MutType) -> Self {
    inner
  }
}

impl BytesType for Box<[u8]> {
  type MutType = Vec<u8>;

  #[inline(always)]
  fn with_capacity(size: usize) -> Result<Self::MutType, TryReserveError> {
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(size)?;
    Ok(bytes)
  }

  #[inline(always)]
  fn from_mut(inner: Self::MutType) -> Self {
    inner.into_boxed_slice()
  }
}

impl<'a> BytesAppendable<'a> for &'a Box<[u8]> {
  fn append_to_builder<TBytes: BytesType>(
    self,
    builder: &mut BytesBuilder<'a, TBytes>,
  ) {
    builder.append(self.as_ref());
  }
}

impl BytesTypeMut for Vec<u8> {
  #[inline(always)]
  fn push(&mut self, c: u8) {
    self.push(c)
  }

  #[inline(always)]
  fn extend_from_slice(&mut self, bytes: &[u8]) {
    self.extend_from_slice(bytes);
  }

  #[inline(always)]
  fn len(&self) -> usize {
    self.len()
  }
}

pub trait BytesAppendable<'a> {
  fn append_to_builder<TBytes: BytesType>(
    self,
    builder: &mut BytesBuilder<'a, TBytes>,
  );
}

pub trait BytesAppendableValue {
  fn byte_len(&self) -> usize;
  fn push_to<TBytes: BytesTypeMut>(&self, bytes: &mut TBytes);
}

impl<'a, T: BytesAppendableValue> BytesAppendable<'a> for T {
  fn append_to_builder<TBytes: BytesType>(
    self,
    builder: &mut BytesBuilder<'a, TBytes>,
  ) {
    match &mut builder.bytes {
      Some(b) => self.push_to(*b),
      None => builder.capacity += self.byte_len(),
    }
  }
}

pub trait EndianBytesAppendable {
  fn byte_len(&self) -> usize;
  fn push_le_to<TBytes: BytesTypeMut>(&self, bytes: &mut TBytes);
  fn push_be_to<TBytes: BytesTypeMut>(&self, bytes: &mut TBytes);
}

impl StringAppendableValue for &str {
  #[inline(always)]
  fn byte_len(&self) -> usize {
    self.len()
  }

  #[inline(always)]
  fn push_to<TString: StringTypeMut>(&self, text: &mut TString) {
    text.push_str(self);
  }

  #[inline(always)]
  fn write_to_formatter(
    &self,
    fmt: &mut std::fmt::Formatter<'_>,
  ) -> std::fmt::Result {
    fmt.write_str(self)
  }
}

impl BytesAppendableValue for &str {
  #[inline(always)]
  fn byte_len(&self) -> usize {
    self.len()
  }

  #[inline(always)]
  fn push_to<TBytes: BytesTypeMut>(&self, bytes: &mut TBytes) {
    bytes.extend_from_slice(self.as_bytes());
  }
}

impl StringAppendableValue for &String {
  #[inline(always)]
  fn byte_len(&self) -> usize {
    self.len()
  }

  #[inline(always)]
  fn push_to<TString: StringTypeMut>(&self, text: &mut TString) {
    text.push_str(self);
  }

  #[inline(always)]
  fn write_to_formatter(
    &self,
    fmt: &mut std::fmt::Formatter<'_>,
  ) -> std::fmt::Result {
    fmt.write_str(self)
  }
}

impl BytesAppendableValue for &String {
  #[inline(always)]
  fn byte_len(&self) -> usize {
    self.len()
  }

  #[inline(always)]
  fn push_to<TBytes: BytesTypeMut>(&self, bytes: &mut TBytes) {
    bytes.extend_from_slice(self.as_bytes());
  }
}

impl<'a> StringAppendableValue for &'a Cow<'a, str> {
  #[inline(always)]
  fn byte_len(&self) -> usize {
    self.len()
  }

  #[inline(always)]
  fn push_to<TString: StringTypeMut>(&self, text: &mut TString) {
    text.push_str(self);
  }

  #[inline(always)]
  fn write_to_formatter(
    &self,
    fmt: &mut std::fmt::Formatter<'_>,
  ) -> std::fmt::Result {
    fmt.write_str(self)
  }
}

impl<'a> BytesAppendableValue for &'a Cow<'a, str> {
  #[inline(always)]
  fn byte_len(&self) -> usize {
    self.len()
  }

  #[inline(always)]
  fn push_to<TBytes: BytesTypeMut>(&self, bytes: &mut TBytes) {
    bytes.extend_from_slice(self.as_bytes());
  }
}

impl_appendable_for_int!(
  i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize
);

impl StringAppendableValue for char {
  #[inline(always)]
  fn byte_len(&self) -> usize {
    self.len_utf8()
  }

  #[inline(always)]
  fn push_to<TString: StringTypeMut>(&self, text: &mut TString) {
    text.push(*self);
  }

  #[inline(always)]
  fn write_to_formatter(
    &self,
    fmt: &mut std::fmt::Formatter<'_>,
  ) -> std::fmt::Result {
    fmt.write_char(*self)
  }
}

impl BytesAppendableValue for char {
  #[inline(always)]
  fn byte_len(&self) -> usize {
    self.len_utf8()
  }

  fn push_to<TBytes: BytesTypeMut>(&self, bytes: &mut TBytes) {
    let mut buffer = [0; 4];
    let encoded = self.encode_utf8(&mut buffer);
    bytes.extend_from_slice(encoded.as_bytes())
  }
}

impl<T: StringAppendableValue> StringAppendableValue for Option<T> {
  #[inline(always)]
  fn byte_len(&self) -> usize {
    match self {
      Some(value) => value.byte_len(),
      None => 0,
    }
  }

  #[inline(always)]
  fn push_to<TString: StringTypeMut>(&self, text: &mut TString) {
    if let Some(value) = self {
      value.push_to(text);
    }
  }

  #[inline(always)]
  fn write_to_formatter(
    &self,
    fmt: &mut std::fmt::Formatter<'_>,
  ) -> std::fmt::Result {
    if let Some(value) = self {
      value.write_to_formatter(fmt)
    } else {
      Ok(())
    }
  }
}

impl<T: BytesAppendableValue> BytesAppendableValue for Option<T> {
  #[inline(always)]
  fn byte_len(&self) -> usize {
    match self {
      Some(value) => value.byte_len(),
      None => 0,
    }
  }

  #[inline(always)]
  fn push_to<TBytes: BytesTypeMut>(&self, bytes: &mut TBytes) {
    if let Some(value) = self {
      value.push_to(bytes);
    }
  }
}

impl BytesAppendableValue for &Vec<u8> {
  #[inline(always)]
  fn byte_len(&self) -> usize {
    self.len()
  }

  #[inline(always)]
  fn push_to<TBytes: BytesTypeMut>(&self, bytes: &mut TBytes) {
    bytes.extend_from_slice(self)
  }
}

impl BytesAppendableValue for u8 {
  #[inline(always)]
  fn byte_len(&self) -> usize {
    1
  }

  #[inline(always)]
  fn push_to<TBytes: BytesTypeMut>(&self, bytes: &mut TBytes) {
    bytes.push(*self)
  }
}

impl BytesAppendableValue for [u8] {
  #[inline(always)]
  fn byte_len(&self) -> usize {
    self.len()
  }

  #[inline(always)]
  fn push_to<TBytes: BytesTypeMut>(&self, bytes: &mut TBytes) {
    bytes.extend_from_slice(self)
  }
}

impl BytesAppendableValue for &[u8] {
  #[inline(always)]
  fn byte_len(&self) -> usize {
    self.len()
  }

  #[inline(always)]
  fn push_to<TBytes: BytesTypeMut>(&self, bytes: &mut TBytes) {
    bytes.extend_from_slice(self)
  }
}

impl<const N: usize> BytesAppendableValue for [u8; N] {
  #[inline(always)]
  fn byte_len(&self) -> usize {
    N
  }

  #[inline(always)]
  fn push_to<TBytes: BytesTypeMut>(&self, bytes: &mut TBytes) {
    bytes.extend_from_slice(self);
  }
}

impl<const N: usize> BytesAppendableValue for &[u8; N] {
  #[inline(always)]
  fn byte_len(&self) -> usize {
    N
  }

  #[inline(always)]
  fn push_to<TBytes: BytesTypeMut>(&self, bytes: &mut TBytes) {
    bytes.extend_from_slice(*self);
  }
}

enum Mode<'a, TStringMut> {
  Capacity,
  Text(&'a mut TStringMut),
  Format(&'a mut std::fmt::Formatter<'a>),
  FormatError(std::fmt::Error),
}

pub struct StringBuilder<'a, TString: StringType = String> {
  capacity: usize,
  mode: Mode<'a, TString::MutType>,
}

impl<'a> StringBuilder<'a, String> {
  /// Formats the string using the provided formatter.
  ///
  /// If an error occurs, the error is stored and surfaced
  /// at the end. The remaining `append` calls are then ignored.
  #[inline(always)]
  pub fn fmt(
    fmt: &mut std::fmt::Formatter<'_>,
    build: impl FnOnce(&mut StringBuilder<'a, String>),
  ) -> std::fmt::Result {
    let mut state = StringBuilder {
      // SAFETY: mutable interior whose lifetimes we don't want to expose in the public API
      mode: Mode::Format(unsafe {
        std::mem::transmute::<
          &mut std::fmt::Formatter<'_>,
          &mut std::fmt::Formatter<'_>,
        >(fmt)
      }),
      capacity: 0,
    };
    build(&mut state);
    match state.mode {
      Mode::Format(_) => Ok(()),
      Mode::FormatError(error) => Err(error),
      Mode::Capacity | Mode::Text(_) => unreachable!(),
    }
  }
}

impl<'a, TString: StringType> StringBuilder<'a, TString> {
  #[inline(always)]
  pub fn build(
    build: impl Fn(&mut StringBuilder<'a, TString>),
  ) -> Result<TString, TryReserveError> {
    let mut state = StringBuilder {
      mode: Mode::Capacity,
      capacity: 0,
    };
    build(&mut state);
    let mut text = TString::with_capacity(state.capacity)?;
    // SAFETY: mutable interior whose lifetimes we don't want to expose in the public API
    state.mode = Mode::Text(unsafe {
      std::mem::transmute::<
        &mut <TString as StringType>::MutType,
        &mut <TString as StringType>::MutType,
      >(&mut text)
    });
    build(&mut state);
    debug_assert_eq!(state.capacity, text.len());
    Ok(TString::from_mut(text))
  }

  /// Gets the current length of the builder.
  ///
  /// On the first pass this will be the current calculated capacity and
  /// on the second pass it will be the current length of the string.
  #[allow(clippy::len_without_is_empty)]
  pub fn len(&self) -> usize {
    match &self.mode {
      Mode::Text(t) => t.len(),
      Mode::Capacity | Mode::Format(_) | Mode::FormatError(_) => self.capacity,
    }
  }

  #[inline(always)]
  pub fn append(&mut self, value: impl StringAppendable<'a> + 'a) {
    value.append_to_builder(self);
  }

  pub fn append_with_replace(&mut self, value: &'a str, from: &str, to: &str) {
    fn calculate_capacity(value: &str, from: &str, to: &str) -> usize {
      if from.len() == to.len() {
        value.len()
      } else {
        let count = value.match_indices(value).count();
        if to.len() > from.len() {
          value.len() + count * (to.len() - from.len())
        } else {
          value.len() - count * (from.len() - to.len())
        }
      }
    }

    fn format_with_replace(
      formatter: &mut std::fmt::Formatter<'_>,
      value: &str,
      from: &str,
      to: &str,
    ) -> Result<usize, std::fmt::Error> {
      let mut start = 0;
      let mut size = 0;
      while let Some(pos) = value[start..].find(from) {
        let end_pos = start + pos;
        formatter.write_str(&value[start..end_pos])?;
        formatter.write_str(to)?;
        size += pos + to.len();
        start += pos + from.len();
      }
      let remaining = &value[start..];
      formatter.write_str(remaining)?;
      size += remaining.len();
      Ok(size)
    }

    match &mut self.mode {
      Mode::Text(buffer) => {
        let mut start = 0;
        while let Some(pos) = value[start..].find(from) {
          buffer.push_str(&value[start..start + pos]);
          buffer.push_str(to);
          start += pos + from.len();
        }
        buffer.push_str(&value[start..]);
      }
      Mode::Format(formatter) => {
        match format_with_replace(formatter, value, from, to) {
          Ok(size) => self.capacity += size,
          Err(e) => {
            // this is very rare, so if it happens we transition
            // to an error state, storing the error to be surfaced
            // later and don't bother formatting the remaining bytes
            self.mode = Mode::FormatError(e);
            self.capacity = calculate_capacity(value, from, to);
          }
        }
      }
      Mode::Capacity | Mode::FormatError(_) => {
        self.capacity += calculate_capacity(value, from, to);
      }
    }
  }

  /// Appends an owned value whose size is known on the first pass.
  ///
  /// WARNING: Be very careful using this as you might accidentally cause
  /// a reallocation. In debug mode this will panic when the size does not
  /// equal the built value.
  pub fn append_owned_unsafe<TStringRef: AsRef<str>>(
    &mut self,
    size: usize,
    build: impl FnOnce() -> TStringRef,
  ) {
    match &mut self.mode {
      Mode::Text(t) => {
        let text = build();
        debug_assert_eq!(text.as_ref().len(), size, "append_owned used where size was not equal! This will cause a reallocation in release mode.");
        t.push_str(text.as_ref());
      }
      Mode::Capacity => self.capacity += size,
      Mode::Format(formatter) => {
        let text = build();
        let result = formatter.write_str(text.as_ref());
        if let Err(e) = result {
          // this is very rare, so if it happens we transition
          // to an error state, storing the error to be surfaced
          // later and don't bother formatting the remaining bytes
          self.mode = Mode::FormatError(e);
        }
        self.capacity += size;
      }
      Mode::FormatError(_) => {
        // keep setting the capacity in case the remaining
        // code relies on this
        self.capacity += size;
      }
    }
  }

  fn append_value(&mut self, value: impl StringAppendableValue) {
    match &mut self.mode {
      Mode::Text(t) => value.push_to(*t),
      Mode::Capacity => self.capacity += value.byte_len(),
      Mode::Format(formatter) => {
        let result = value.write_to_formatter(formatter);
        if let Err(e) = result {
          // this is very rare, so if it happens we transition
          // to an error state, storing the error to be surfaced
          // later and don't bother formatting the remaining bytes
          self.mode = Mode::FormatError(e);
        }
        self.capacity += value.byte_len();
      }
      Mode::FormatError(_) => {
        // keep setting the capacity in case the remaining
        // code relies on this
        self.capacity += value.byte_len();
      }
    }
  }
}

/// Helper method for converting an appendable value to a string.
pub fn appendable_to_string<'a, TString: StringType>(
  value: impl StringAppendable<'a> + Copy + 'a,
) -> TString
where
  <TString as StringType>::MutType: 'a,
{
  StringBuilder::<TString>::build(|builder| builder.append(value)).unwrap()
}

pub struct BytesBuilder<'a, TBytes: BytesType> {
  capacity: usize,
  bytes: Option<&'a mut TBytes::MutType>,
}

impl<'a, TBytes: BytesType> BytesBuilder<'a, TBytes> {
  #[inline(always)]
  pub fn build(
    build: impl Fn(&mut BytesBuilder<'a, TBytes>),
  ) -> Result<TBytes, TryReserveError> {
    let mut builder = BytesBuilder {
      bytes: None,
      capacity: 0,
    };
    build(&mut builder);
    let mut bytes = TBytes::with_capacity(builder.capacity)?;
    // SAFETY: mutable interior whose lifetimes we don't want to expose in the public API
    builder.bytes = Some(unsafe {
      std::mem::transmute::<
        &mut <TBytes as BytesType>::MutType,
        &mut <TBytes as BytesType>::MutType,
      >(&mut bytes)
    });
    build(&mut builder);
    debug_assert_eq!(builder.capacity, builder.bytes.as_ref().unwrap().len());
    Ok(TBytes::from_mut(bytes))
  }

  /// Gets the current length of the builder.
  ///
  /// On the first pass this will be the current calculated capacity and
  /// on the second pass it will be the current length of the bytes.
  #[allow(clippy::len_without_is_empty)]
  pub fn len(&self) -> usize {
    self
      .bytes
      .as_ref()
      .map(|t| t.len())
      .unwrap_or(self.capacity)
  }

  #[inline(always)]
  pub fn append(&mut self, value: impl BytesAppendable<'a> + 'a) {
    value.append_to_builder(self);
  }

  /// Appends a number in big-endian byte order.
  ///
  /// WARNING: Rust defaults to i32 for integer literals. It's probably
  /// best to always specify the type of number.
  #[inline(always)]
  pub fn append_be<T: EndianBytesAppendable + 'a>(&mut self, value: T) {
    match &mut self.bytes {
      Some(b) => value.push_be_to(*b),
      None => self.capacity += value.byte_len(),
    }
  }

  /// Appends a number in little-endian byte order.
  ///
  /// WARNING: Rust defaults to i32 for integer literals. It's probably
  /// best to always specify the type of number.
  #[inline(always)]
  pub fn append_le<T: EndianBytesAppendable + 'a>(&mut self, value: T) {
    match &mut self.bytes {
      Some(b) => value.push_le_to(*b),
      None => self.capacity += value.byte_len(),
    }
  }
}
