//! This is a library which allows anyone to change various properties of their
//! strings with a heavy emphasis on performance. Allows programmers to
//! manipulate a single programatic name consistently in multiple contexts.
//!
//! # Example
//! ```rust
//! // Remember to import the `Inflect` trait!
//! use inflections::Inflect;
//!
//! assert_eq!("Hello World".to_camel_case(), "helloWorld".to_owned());
//! ```

pub mod case;

/// An extension trait to make the functions in the `case` module available as
/// methods on the `str` type.
///
/// # Example
///
/// ```rust
/// // Remember to import the `Inflect` trait!
/// use inflections::Inflect;
///
/// assert_eq!("Hello World".to_camel_case(), "helloWorld".to_owned());
/// ```
///
/// # Stability
///
/// This trait is *not* meant to be used for generic programming. We reserve
/// the right to add more methods to this trait and to change the
/// implementations of this trait for primitive types *without making a major
/// version release* as long as we don't break existing method calls.
pub trait Inflect {
  fn to_upper_case(&self) -> String;
  fn is_upper_case(&self) -> bool;
  fn to_lower_case(&self) -> String;
  fn is_lower_case(&self) -> bool;
  fn to_sentence_case(&self) -> String;
  fn is_sentence_case(&self) -> bool;
  fn to_title_case(&self) -> String;
  fn is_title_case(&self) -> bool;
  fn to_camel_case(&self) -> String;
  fn is_camel_case(&self) -> bool;
  fn to_pascal_case(&self) -> String;
  fn is_pascal_case(&self) -> bool;
  fn to_kebab_case(&self) -> String;
  fn is_kebab_case(&self) -> bool;
  fn to_train_case(&self) -> String;
  fn is_train_case(&self) -> bool;
  fn to_snake_case(&self) -> String;
  fn is_snake_case(&self) -> bool;
  fn to_constant_case(&self) -> String;
  fn is_constant_case(&self) -> bool;
}

impl Inflect for str {
  #[inline] fn to_upper_case(&self) -> String { case::to_upper_case(self) }
  #[inline] fn is_upper_case(&self) -> bool { case::is_upper_case(self) }
  #[inline] fn to_lower_case(&self) -> String { case::to_lower_case(self) }
  #[inline] fn is_lower_case(&self) -> bool { case::is_lower_case(self) }
  #[inline] fn to_sentence_case(&self) -> String { case::to_sentence_case(self) }
  #[inline] fn is_sentence_case(&self) -> bool { case::is_sentence_case(self) }
  #[inline] fn to_title_case(&self) -> String { case::to_title_case(self) }
  #[inline] fn is_title_case(&self) -> bool { case::is_title_case(self) }
  #[inline] fn to_camel_case(&self) -> String { case::to_camel_case(self) }
  #[inline] fn is_camel_case(&self) -> bool { case::is_camel_case(self) }
  #[inline] fn to_pascal_case(&self) -> String { case::to_pascal_case(self) }
  #[inline] fn is_pascal_case(&self) -> bool { case::is_pascal_case(self) }
  #[inline] fn to_kebab_case(&self) -> String { case::to_kebab_case(self) }
  #[inline] fn is_kebab_case(&self) -> bool { case::is_kebab_case(self) }
  #[inline] fn to_train_case(&self) -> String { case::to_train_case(self) }
  #[inline] fn is_train_case(&self) -> bool { case::is_train_case(self) }
  #[inline] fn to_snake_case(&self) -> String { case::to_snake_case(self) }
  #[inline] fn is_snake_case(&self) -> bool { case::is_snake_case(self) }
  #[inline] fn to_constant_case(&self) -> String { case::to_constant_case(self) }
  #[inline] fn is_constant_case(&self) -> bool { case::is_constant_case(self) }
}

#[cfg(test)]
mod test {
  use super::Inflect;

  #[test]
  fn test_str() {
    assert_eq!("foo".to_title_case(), "Foo".to_owned());
  }

  #[test]
  fn test_string() {
    assert_eq!("foo".to_owned().to_title_case(), "Foo".to_owned());
  }
}
