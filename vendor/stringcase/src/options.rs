// Copyright (C) 2025 Takayuki Sato. All Rights Reserved.
// This program is free software under MIT License.
// See the file LICENSE in this distribution for more details.

/// Is a struct that represents options for case conversion of strings.
///
/// The `separate_before_non_alphabets` field specifies whether to treat the
/// beginning of a sequence of non-alphabetic characters as a word boundary.
/// The `separate_after_non_alphabets` field specifies whether to treat the
/// end of a sequence of non-alphabetic characters as a word boundary.
/// The `separators` field specifies the set of characters to be treated as
/// word separators and removed from the result string.
/// The `keep` field specifies the set of characters not to be treated as word
/// separators and kept in the result string.
///
/// Alphanumeric characters specified in `separators` and `keep` are ignored.
/// If both `separators` and `keep` are specified, `separators` takes precedence
/// and `keep` is ignored.
pub struct Options<'a> {
    /// Specifies whether to treat the beginning of a sequence of non-alphabetic
    /// characters as a word boundary.
    pub separate_before_non_alphabets: bool,

    /// Specifies whether to treat the end of a sequence of non-alphabetic
    /// characters as a word boundary.
    pub separate_after_non_alphabets: bool,

    /// Specifies the set of characters to be treated as word separators and
    /// removed from the result string. Alphanumeric characters are ignored.
    pub separators: &'a str,

    /// Specifies the set of characters not to be treated as word separators
    /// and kept in the result string. Alphanumeric characters are ignored.
    pub keep: &'a str,
}

impl<'a> Options<'a> {
    pub fn new(
        separate_before_non_alphabets: bool,
        separate_after_non_alphabets: bool,
        separators: &'a str,
        keep: &'a str,
    ) -> Self {
        Self {
            separate_before_non_alphabets,
            separate_after_non_alphabets,
            separators,
            keep,
        }
    }
}

impl Default for Options<'_> {
    fn default() -> Self {
        Self {
            separate_before_non_alphabets: false,
            separate_after_non_alphabets: true,
            separators: "",
            keep: "",
        }
    }
}

#[cfg(test)]
mod tests_of_options {
    use super::*;

    #[test]
    fn test_of_new() {
        let opts = Options::new(true, true, "-_", "#@");
        assert_eq!(opts.separate_before_non_alphabets, true);
        assert_eq!(opts.separate_after_non_alphabets, true);
        assert_eq!(opts.separators, "-_");
        assert_eq!(opts.keep, "#@");
    }

    #[test]
    fn test_of_default() {
        let opts = Options::default();
        assert_eq!(opts.separate_before_non_alphabets, false);
        assert_eq!(opts.separate_after_non_alphabets, true);
        assert_eq!(opts.separators, "");
        assert_eq!(opts.keep, "");
    }

    #[test]
    fn test_of_default_by_fields() {
        let opts = Options {
            separate_before_non_alphabets: true,
            separators: "-#@",
            ..Default::default()
        };
        assert_eq!(opts.separate_before_non_alphabets, true);
        assert_eq!(opts.separate_after_non_alphabets, true);
        assert_eq!(opts.separators, "-#@");
        assert_eq!(opts.keep, "");
    }
}
