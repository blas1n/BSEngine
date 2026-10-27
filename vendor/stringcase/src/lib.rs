// Copyright (C) 2024-2025 Takayuki Sato. All Rights Reserved.
// This program is free software under MIT License.
// See the file LICENSE in this distribution for more details.

//! This library provides some functions that convert string cases between camelCase, COBOL-CASE,
//! kebab-case, MACRO_CASE, PascalCase, snake_case and Train-Case.
//! And this library also provides a trait `Caser` which enables strings to convert themselves
//! to their cases by their own methods.
//!
//! Basically, these functions only target ASCII uppercase and lowercase letters for
//! capitalization. All characters other than ASCII uppercase and lowercase letters and ASCII
//! numbers are removed as word separators.
//!
//! If you want to use some symbols as separators, specify those symbols in the `separators` field
//! of `Options` struct and use the `〜case_with_options` function for the desired case.
//! If you want to retain certain symbols and use everything else as separators, specify those
//! symbols in `keep` field of `Options` struct and use the `〜case_with_options` function for the
//! desired case.
//!
//! Additionally, you can specify whether to place word boundaries before and/or after
//! non-alphabetic characters with conversion options.
//! This can be set using the `separate_before_non_alphabets` and `separate_after_non_alphabets`
//! fields in the `Options` struct.
//!
//! The `〜_case` functions that do not take `Options` as an argument only place word boundaries
//! after non-alphabetic characters.
//! In other words, they behave as if `separate_before_non_alphabets = false` and
//! `separate_after_non_alphabets = true`.
//!
//! ## Install
//!
//! In `Cargo.toml`, write this crate as a dependency.
//!
//! ```toml
//! [dependencies]
//! stringcase = "0.4.0"
//! ```
//!
//! ## Usage
//!
//! The functions in this crate can be used as follows:
//!
//! ```rust
//! use stringcase::snake_case;
//!
//! fn main() {
//!     let input = "fooBar123Baz";
//!     let snake = snake_case(input);
//!     assert_eq!(snake, "foo_bar123_baz");
//! }
//! ```
//!
//! If you want the conversion to behave differently, use `〜_case_with_options`.
//!
//! ```rust
//! use stringcase::{snake_case_with_options, Options};
//!
//! fn main() {
//!     let opts = Options{separate_before_non_alphabets: true, ..Default::default()};
//!     let input = "fooBar123Baz";
//!     let snake = snake_case_with_options(input, &opts);
//!     assert_eq!(snake, "foo_bar_123_baz");
//! }
//! ```
//!
//! And by bringing `Caser` with `use` declaration, it will be able to execute
//! methods of strings, `String` or `&str`, to convert to their cases.
//!
//! ```rust
//! use stringcase::{Caser, Options};
//!
//! fn main() {
//!     let input = "fooBar123Baz";
//!     let snake = input.to_snake_case();
//!     assert_eq!(snake, "foo_bar123_baz");
//!
//!     let opts = Options{separate_before_non_alphabets: true, ..Default::default()};
//!     let snake = input.to_snake_case_with_options(&opts);
//!     assert_eq!(snake, "foo_bar_123_baz");
//! }
//! ```

mod camel_case;
mod caser;
mod cobol_case;
mod kebab_case;
mod macro_case;
mod options;
mod pascal_case;
mod snake_case;
mod train_case;

pub use camel_case::*;
pub use caser::*;
pub use cobol_case::*;
pub use kebab_case::*;
pub use macro_case::*;
pub use options::Options;
pub use pascal_case::*;
pub use snake_case::*;
pub use train_case::*;
