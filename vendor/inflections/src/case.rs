//! Utilities to change the case of a string to another case. Supports “lower
//! case,” “UPPER CASE,” “sentence case,” “Title Case,” “camelCase,”
//! “PascalCase,” “kebab-case,” “Train-Case,” “snake_case,” and
//! “CONSTANT_CASE.”
//!
//! For more information [Wikipedia][1] has an interesting article on these
//! special case styles.
//!
//! [1]: https://en.wikipedia.org/wiki/Letter_case#Special_case_styles
//!
//! # Example
//! ```rust
//! use inflections::case::to_camel_case;
//!
//! assert_eq!(to_camel_case("Hello World"), "helloWorld".to_owned());
//! ```

use std::char::ToUppercase;
use std::iter::Peekable;

/// Converts any case into lower case ignoring separators.
///
/// # Example
/// ```rust
/// # use inflections::case::to_lower_case;
/// let lower = "hello world".to_owned();
/// assert_eq!(to_lower_case("hello world"), lower);
/// assert_eq!(to_lower_case("HELLO WORLD"), lower);
/// assert_eq!(to_lower_case("Hello World"), lower);
/// assert_eq!(to_lower_case("helloWorld"), "helloworld".to_owned());
/// assert_eq!(to_lower_case("HelloWorld"), "helloworld".to_owned());
/// assert_eq!(to_lower_case("hello-world"), "hello-world".to_owned());
/// assert_eq!(to_lower_case("Hello-World"), "hello-world".to_owned());
/// assert_eq!(to_lower_case("hello_world"), "hello_world".to_owned());
/// assert_eq!(to_lower_case("HELLO_WORLD"), "hello_world".to_owned());
/// ```
pub fn to_lower_case(string: &str) -> String {
  string
  .chars()
  .flat_map(char::to_lowercase)
  .collect()
}

/// Check to see if a string is completely lower case.
///
/// # Example
/// ```rust
/// # use inflections::case::is_lower_case;
/// assert_eq!(is_lower_case("hello world"), true);
/// assert_eq!(is_lower_case("HELLO WORLD"), false);
/// assert_eq!(is_lower_case("Hello World"), false);
/// assert_eq!(is_lower_case("helloWorld"), false);
/// assert_eq!(is_lower_case("HelloWorld"), false);
/// assert_eq!(is_lower_case("hello-world"), true);
/// assert_eq!(is_lower_case("Hello-World"), false);
/// assert_eq!(is_lower_case("hello_world"), true);
/// assert_eq!(is_lower_case("HELLO_WORLD"), false);
pub fn is_lower_case(string: &str) -> bool {
  string == to_lower_case(string)
}

/// Converts any case into UPPER CASE ignoring separators.
///
/// # Example
/// ```rust
/// # use inflections::case::to_upper_case;
/// let upper = "HELLO WORLD".to_owned();
/// assert_eq!(to_upper_case("hello world"), upper);
/// assert_eq!(to_upper_case("HELLO WORLD"), upper);
/// assert_eq!(to_upper_case("Hello World"), upper);
/// assert_eq!(to_upper_case("helloWorld"), "HELLOWORLD".to_owned());
/// assert_eq!(to_upper_case("HelloWorld"), "HELLOWORLD".to_owned());
/// assert_eq!(to_upper_case("hello-world"), "HELLO-WORLD".to_owned());
/// assert_eq!(to_upper_case("Hello-World"), "HELLO-WORLD".to_owned());
/// assert_eq!(to_upper_case("hello_world"), "HELLO_WORLD".to_owned());
/// assert_eq!(to_upper_case("HELLO_WORLD"), "HELLO_WORLD".to_owned());
/// ```
pub fn to_upper_case(string: &str) -> String {
  string
  .chars()
  .flat_map(char::to_uppercase)
  .collect()
}

/// Check to see if a string is completely UPPER CASE.
///
/// # Example
/// ```rust
/// # use inflections::case::is_upper_case;
/// assert_eq!(is_upper_case("hello world"), false);
/// assert_eq!(is_upper_case("HELLO WORLD"), true);
/// assert_eq!(is_upper_case("Hello World"), false);
/// assert_eq!(is_upper_case("helloWorld"), false);
/// assert_eq!(is_upper_case("HelloWorld"), false);
/// assert_eq!(is_upper_case("hello-world"), false);
/// assert_eq!(is_upper_case("Hello-World"), false);
/// assert_eq!(is_upper_case("hello_world"), false);
/// assert_eq!(is_upper_case("HELLO_WORLD"), true);
pub fn is_upper_case(string: &str) -> bool {
  string == to_upper_case(string)
}

/// Converts any case into traditional sentence case without capitalizing the
/// first letter.
///
/// # Example
/// ```rust
/// # use inflections::case::to_sentence_case;
/// let sentence = "hello world".to_owned();
/// assert_eq!(to_sentence_case("hello world"), sentence);
/// assert_eq!(to_sentence_case("HELLO WORLD"), sentence);
/// assert_eq!(to_sentence_case("Hello World"), sentence);
/// assert_eq!(to_sentence_case("helloWorld"), sentence);
/// assert_eq!(to_sentence_case("HelloWorld"), sentence);
/// assert_eq!(to_sentence_case("hello-world"), sentence);
/// assert_eq!(to_sentence_case("Hello-World"), sentence);
/// assert_eq!(to_sentence_case("hello_world"), sentence);
/// assert_eq!(to_sentence_case("HELLO_WORLD"), sentence);
/// ```
pub fn to_sentence_case(string: &str) -> String {
  string
  .chars()
  .map(|c| swap_separator(c, ' '))
  .break_camel(' ')
  .flat_map(char::to_lowercase)
  .collect()
}

/// Check to see if a string is sentence case.
///
/// # Example
/// ```rust
/// # use inflections::case::is_sentence_case;
/// assert_eq!(is_sentence_case("hello world"), true);
/// assert_eq!(is_sentence_case("HELLO WORLD"), false);
/// assert_eq!(is_sentence_case("Hello World"), false);
/// assert_eq!(is_sentence_case("helloWorld"), false);
/// assert_eq!(is_sentence_case("HelloWorld"), false);
/// assert_eq!(is_sentence_case("hello-world"), false);
/// assert_eq!(is_sentence_case("Hello-World"), false);
/// assert_eq!(is_sentence_case("hello_world"), false);
/// assert_eq!(is_sentence_case("HELLO_WORLD"), false);
pub fn is_sentence_case(string: &str) -> bool {
  string == to_sentence_case(string)
}

/// Converts any case into title case where *every* word is capitalized.
///
/// # Example
/// ```rust
/// # use inflections::case::to_title_case;
/// let title = "Hello World".to_owned();
/// assert_eq!(to_title_case("hello world"), title);
/// assert_eq!(to_title_case("HELLO WORLD"), title);
/// assert_eq!(to_title_case("Hello World"), title);
/// assert_eq!(to_title_case("helloWorld"), title);
/// assert_eq!(to_title_case("HelloWorld"), title);
/// assert_eq!(to_title_case("hello-world"), title);
/// assert_eq!(to_title_case("Hello-World"), title);
/// assert_eq!(to_title_case("hello_world"), title);
/// assert_eq!(to_title_case("HELLO_WORLD"), title);
/// ```
pub fn to_title_case(string: &str) -> String {
  string
  .chars()
  .map(|c| swap_separator(c, ' '))
  .break_camel(' ')
  .flat_map(char::to_lowercase)
  .capitalize_words()
  .collect()
}

/// Check to see if a string is Title Case.
///
/// # Example
/// ```rust
/// # use inflections::case::is_title_case;
/// assert_eq!(is_title_case("Hello World"), true);
/// assert_eq!(is_title_case("hello world"), false);
/// assert_eq!(is_title_case("HELLO WORLD"), false);
/// assert_eq!(is_title_case("helloWorld"), false);
/// assert_eq!(is_title_case("HelloWorld"), false);
/// assert_eq!(is_title_case("hello-world"), false);
/// assert_eq!(is_title_case("Hello-World"), false);
/// assert_eq!(is_title_case("hello_world"), false);
/// assert_eq!(is_title_case("HELLO_WORLD"), false);
pub fn is_title_case(string: &str) -> bool {
  string == to_title_case(string)
}

/// Converts any case into camelCase.
///
/// # Example
/// ```rust
/// # use inflections::case::to_camel_case;
/// let camel = "helloWorld".to_owned();
/// assert_eq!(to_camel_case("hello world"), camel);
/// assert_eq!(to_camel_case("HELLO WORLD"), camel);
/// assert_eq!(to_camel_case("Hello World"), camel);
/// assert_eq!(to_camel_case("helloWorld"), camel);
/// assert_eq!(to_camel_case("HelloWorld"), camel);
/// assert_eq!(to_camel_case("hello-world"), camel);
/// assert_eq!(to_camel_case("Hello-World"), camel);
/// assert_eq!(to_camel_case("hello_world"), camel);
/// assert_eq!(to_camel_case("HELLO_WORLD"), camel);
/// ```
pub fn to_camel_case(string: &str) -> String {
  string
  .chars()
  .scan((false, None), scan_to_camel)
  .collect()
}

/// Check to see if a string is camelCase.
///
/// # Example
/// ```rust
/// # use inflections::case::is_camel_case;
/// assert_eq!(is_camel_case("helloWorld"), true);
/// assert_eq!(is_camel_case("hello world"), false);
/// assert_eq!(is_camel_case("HELLO WORLD"), false);
/// assert_eq!(is_camel_case("Hello World"), false);
/// assert_eq!(is_camel_case("HelloWorld"), false);
/// assert_eq!(is_camel_case("hello-world"), false);
/// assert_eq!(is_camel_case("Hello-World"), false);
/// assert_eq!(is_camel_case("hello_world"), false);
/// assert_eq!(is_camel_case("HELLO_WORLD"), false);
pub fn is_camel_case(string: &str) -> bool {
  string == to_camel_case(string)
}

/// Converts any case into PascalCase.
///
/// # Example
/// ```rust
/// # use inflections::case::to_pascal_case;
/// let pascal = "HelloWorld".to_owned();
/// assert_eq!(to_pascal_case("hello world"), pascal);
/// assert_eq!(to_pascal_case("HELLO WORLD"), pascal);
/// assert_eq!(to_pascal_case("Hello World"), pascal);
/// assert_eq!(to_pascal_case("helloWorld"), pascal);
/// assert_eq!(to_pascal_case("HelloWorld"), pascal);
/// assert_eq!(to_pascal_case("hello-world"), pascal);
/// assert_eq!(to_pascal_case("Hello-World"), pascal);
/// assert_eq!(to_pascal_case("hello_world"), pascal);
/// assert_eq!(to_pascal_case("HELLO_WORLD"), pascal);
/// ```
pub fn to_pascal_case(string: &str) -> String {
  string
  .chars()
  .scan((true, None), scan_to_camel)
  .collect()
}

/// Check to see if a string is PascalCase.
///
/// # Example
/// ```rust
/// # use inflections::case::is_pascal_case;
/// assert_eq!(is_pascal_case("HelloWorld"), true);
/// assert_eq!(is_pascal_case("hello world"), false);
/// assert_eq!(is_pascal_case("HELLO WORLD"), false);
/// assert_eq!(is_pascal_case("Hello World"), false);
/// assert_eq!(is_pascal_case("helloWorld"), false);
/// assert_eq!(is_pascal_case("hello-world"), false);
/// assert_eq!(is_pascal_case("Hello-World"), false);
/// assert_eq!(is_pascal_case("hello_world"), false);
/// assert_eq!(is_pascal_case("HELLO_WORLD"), false);
pub fn is_pascal_case(string: &str) -> bool {
  string == to_pascal_case(string)
}

/// Converts any case into kebab-case.
///
/// # Example
/// ```rust
/// # use inflections::case::to_kebab_case;
/// let kebab = "hello-world".to_owned();
/// assert_eq!(to_kebab_case("hello world"), kebab);
/// assert_eq!(to_kebab_case("HELLO WORLD"), kebab);
/// assert_eq!(to_kebab_case("Hello World"), kebab);
/// assert_eq!(to_kebab_case("helloWorld"), kebab);
/// assert_eq!(to_kebab_case("HelloWorld"), kebab);
/// assert_eq!(to_kebab_case("hello-world"), kebab);
/// assert_eq!(to_kebab_case("Hello-World"), kebab);
/// assert_eq!(to_kebab_case("hello_world"), kebab);
/// assert_eq!(to_kebab_case("HELLO_WORLD"), kebab);
/// ```
pub fn to_kebab_case(string: &str) -> String {
  string
  .chars()
  .map(|c| swap_separator(c, '-'))
  .break_camel('-')
  .flat_map(char::to_lowercase)
  .collect()
}

/// Check to see if a string is kebab-case.
///
/// # Example
/// ```rust
/// # use inflections::case::is_kebab_case;
/// assert_eq!(is_kebab_case("hello-world"), true);
/// assert_eq!(is_kebab_case("hello world"), false);
/// assert_eq!(is_kebab_case("HELLO WORLD"), false);
/// assert_eq!(is_kebab_case("Hello World"), false);
/// assert_eq!(is_kebab_case("helloWorld"), false);
/// assert_eq!(is_kebab_case("HelloWorld"), false);
/// assert_eq!(is_kebab_case("Hello-World"), false);
/// assert_eq!(is_kebab_case("hello_world"), false);
/// assert_eq!(is_kebab_case("HELLO_WORLD"), false);
pub fn is_kebab_case(string: &str) -> bool {
  string == to_kebab_case(string)
}

/// Converts any case into Train-Case.
///
/// # Example
/// ```rust
/// # use inflections::case::to_train_case;
/// let train = "Hello-World".to_owned();
/// assert_eq!(to_train_case("hello world"), train);
/// assert_eq!(to_train_case("HELLO WORLD"), train);
/// assert_eq!(to_train_case("Hello World"), train);
/// assert_eq!(to_train_case("helloWorld"), train);
/// assert_eq!(to_train_case("HelloWorld"), train);
/// assert_eq!(to_train_case("hello-world"), train);
/// assert_eq!(to_train_case("Hello-World"), train);
/// assert_eq!(to_train_case("hello_world"), train);
/// assert_eq!(to_train_case("HELLO_WORLD"), train);
/// ```
pub fn to_train_case(string: &str) -> String {
  string
  .chars()
  .map(|c| swap_separator(c, '-'))
  .break_camel('-')
  .flat_map(char::to_lowercase)
  .capitalize_words()
  .collect()
}

/// Check to see if a string is Train-Case.
///
/// # Example
/// ```rust
/// # use inflections::case::is_train_case;
/// assert_eq!(is_train_case("Hello-World"), true);
/// assert_eq!(is_train_case("hello world"), false);
/// assert_eq!(is_train_case("HELLO WORLD"), false);
/// assert_eq!(is_train_case("Hello World"), false);
/// assert_eq!(is_train_case("helloWorld"), false);
/// assert_eq!(is_train_case("HelloWorld"), false);
/// assert_eq!(is_train_case("hello-world"), false);
/// assert_eq!(is_train_case("hello_world"), false);
/// assert_eq!(is_train_case("HELLO_WORLD"), false);
pub fn is_train_case(string: &str) -> bool {
  string == to_train_case(string)
}

/// Converts any case into snake_case.
///
/// # Example
/// ```rust
/// # use inflections::case::to_snake_case;
/// let snake = "hello_world".to_owned();
/// assert_eq!(to_snake_case("hello world"), snake);
/// assert_eq!(to_snake_case("HELLO WORLD"), snake);
/// assert_eq!(to_snake_case("Hello World"), snake);
/// assert_eq!(to_snake_case("helloWorld"), snake);
/// assert_eq!(to_snake_case("HelloWorld"), snake);
/// assert_eq!(to_snake_case("hello-world"), snake);
/// assert_eq!(to_snake_case("Hello-World"), snake);
/// assert_eq!(to_snake_case("hello_world"), snake);
/// assert_eq!(to_snake_case("HELLO_WORLD"), snake);
/// ```
pub fn to_snake_case(string: &str) -> String {
  string
  .chars()
  .map(|c| swap_separator(c, '_'))
  .break_camel('_')
  .flat_map(char::to_lowercase)
  .collect()
}

/// Check to see if a string is snake_case.
///
/// # Example
/// ```rust
/// # use inflections::case::is_snake_case;
/// assert_eq!(is_snake_case("hello_world"), true);
/// assert_eq!(is_snake_case("hello world"), false);
/// assert_eq!(is_snake_case("HELLO WORLD"), false);
/// assert_eq!(is_snake_case("Hello World"), false);
/// assert_eq!(is_snake_case("helloWorld"), false);
/// assert_eq!(is_snake_case("HelloWorld"), false);
/// assert_eq!(is_snake_case("hello-world"), false);
/// assert_eq!(is_snake_case("Hello-World"), false);
/// assert_eq!(is_snake_case("HELLO_WORLD"), false);
pub fn is_snake_case(string: &str) -> bool {
  string == to_snake_case(string)
}

/// Converts any case into CONSTANT_CASE.
///
/// # Example
/// ```rust
/// # use inflections::case::to_constant_case;
/// let constant = "HELLO_WORLD".to_owned();
/// assert_eq!(to_constant_case("hello world"), constant);
/// assert_eq!(to_constant_case("HELLO WORLD"), constant);
/// assert_eq!(to_constant_case("Hello World"), constant);
/// assert_eq!(to_constant_case("helloWorld"), constant);
/// assert_eq!(to_constant_case("HelloWorld"), constant);
/// assert_eq!(to_constant_case("hello-world"), constant);
/// assert_eq!(to_constant_case("Hello-World"), constant);
/// assert_eq!(to_constant_case("hello_world"), constant);
/// assert_eq!(to_constant_case("HELLO_WORLD"), constant);
/// ```
pub fn to_constant_case(string: &str) -> String {
  string
  .chars()
  .map(|c| swap_separator(c, '_'))
  .break_camel('_')
  .flat_map(char::to_uppercase)
  .collect()
}

/// Check to see if a string is CONSTANT_CASE.
///
/// # Example
/// ```rust
/// # use inflections::case::is_constant_case;
/// assert_eq!(is_constant_case("HELLO_WORLD"), true);
/// assert_eq!(is_constant_case("hello world"), false);
/// assert_eq!(is_constant_case("HELLO WORLD"), false);
/// assert_eq!(is_constant_case("Hello World"), false);
/// assert_eq!(is_constant_case("helloWorld"), false);
/// assert_eq!(is_constant_case("HelloWorld"), false);
/// assert_eq!(is_constant_case("hello-world"), false);
/// assert_eq!(is_constant_case("Hello-World"), false);
/// assert_eq!(is_constant_case("hello_world"), false);
pub fn is_constant_case(string: &str) -> bool {
  string == to_constant_case(string)
}

/// Checks if a character is a separator.
#[inline]
fn is_separator(c: char) -> bool {
  c == ' ' || c == '-' || c == '_'
}

/// Swaps the current character (which may be a separator), with the separator
/// of choice. Currently ' ', '-', and '_' are considered separators which will
/// be swapped.
#[inline]
fn swap_separator(c: char, sep: char) -> char {
  if is_separator(c) {
    sep
  } else {
    c
  }
}

/// The function to be used with the iterator `scan` method which converts a
/// char iterator into a string iterator which has
/// removed/uppercased/lowercased the bits which need for the conversion to be
/// successful. This would work best with a `flat_scan`.
#[inline]
fn scan_to_camel(state: &mut (bool, Option<char>), curr: char) -> Option<String> {
  // Store the last character in the scope and update the state to use the
  // current character.
  let last = state.1;
  state.1 = Some(curr);

  if state.0 {
    // If the state has signaled the next character must be capitalized,
    // capitalize it and mark the state as finished.
    state.0 = false;
    Some(curr.to_uppercase().collect())
  } else if is_separator(curr) {
    // If the current character is a separator, mark the state to capitalize
    // the next character and remove the separator.
    state.0 = true;
    Some("".to_owned())
  } else if !last.map_or(false, char::is_lowercase) {
    // If the last character was not lowercase, this character should be
    // lower cased. This magic preserves camelCase strings while lowercasing
    // cases like CONSTANT_CASE.
    Some(curr.to_lowercase().collect())
  } else {
    // Otherwise, just return the character.
    let mut string = String::with_capacity(1);
    string.push(curr);
    Some(string)
  }
}

/// Trait with some extra methods for the iterators we use.
trait Extras: Iterator<Item=char> {
  /// Uses the `BreakCamel` type to break apart camel case strings, i.e.
  /// strings like `helloWorld` to `hello world` using the `sep` argument to
  /// seperate the new words.
  #[inline]
  fn break_camel(self, sep: char) -> BreakCamel<Self> where Self: Sized {
    BreakCamel {
      iter: self.peekable(),
      sep: sep,
      br: false
    }
  }

  /// Uses the `CapitalizeWords` type to capitilize individual words seperated
  /// by a separator (as defined by `is_separator`).
  #[inline]
  fn capitalize_words(self) -> CapitalizeWords<Self> where Self: Sized {
    let mut iter = self.peekable();
    // If the first character is not a separator, we want to capitilize it.
    let cap = !iter.peek().cloned().map_or(false, is_separator);
    CapitalizeWords {
      iter: iter,
      cap: cap,
      upper: None
    }
  }
}

/// Add the extra methods to the iterators.
impl<I> Extras for I where I: Iterator<Item=char> {}

/// Utility for breaking apart camelCased strings.
struct BreakCamel<I> where I: Iterator<Item=char> {
  /// The source iterator.
  iter: Peekable<I>,
  /// Character to use when breaking apart camelCase strings.
  sep: char,
  /// Iterator state representing whether the iterator should insert a break.
  br: bool
}

impl<I> Iterator for BreakCamel<I> where I: Iterator<Item=char> {
  type Item = char;

  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    // If we have been signaled to break, the next item is a separator and we
    // should disable break mode.
    if self.br {
      self.br = false;
      return Some(self.sep);
    }

    match (self.iter.next(), self.iter.peek()) {
      // If we have a current character that is lowercase and we have a next
      // character that is uppercase, we need to break the string apart next
      // time `next` is called.
      (Some(curr), Some(next)) if curr.is_lowercase() && next.is_uppercase() => {
        self.br = true;
        Some(curr)
      },
      // Otherwise behave as normal.
      (Some(curr), _) => Some(curr),
      (None, _) => None
    }
  }
}

/// Utility for capitalizing words.
struct CapitalizeWords<I> where I: Iterator<Item=char> {
  /// The source iterator.
  iter: Peekable<I>,
  /// Iteration state indicating whether or not the next letter should be
  /// capitalized.
  cap: bool,
  /// An iterator for the letter being upper cased.
  upper: Option<ToUppercase>
}

impl<I> Iterator for CapitalizeWords<I> where I: Iterator<Item=char> {
  type Item = char;

  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    // Note: only one control path should allow this loop to continue.
    loop {
      // If there is an upper casing still in progress…
      if let Some(ref mut upper) = self.upper {
        if let Some(next) = upper.next() {
          // If there was a character in the uppercase iterator, return it.
          return Some(next);
        }
      }

      // If we are here this means we have exhausted the upper case iterator,
      // therefore we reset the upper iterator state.
      if self.upper.is_some() {
        self.upper = None;
      }

      if let Some(c) = self.iter.next() {
        // If there is a character…
        if self.cap {
          // If we have been signaled to capitalize the next character, disable
          // the signal, and enable the capitalization iterator.
          self.cap = false;
          self.upper = Some(c.to_uppercase());
          // We want it to loop back here…
        } else {
          // Otherwise return the character, but if it is a separator, and the
          // next character is not a separator—signal the next character should
          // be capitalized.
          if is_separator(c) && !self.iter.peek().cloned().map_or(false, is_separator) {
            self.cap = true;
          }
          return Some(c);
        }
      } else {
        // End the iterator.
        return None;
      }
    }
  }
}
