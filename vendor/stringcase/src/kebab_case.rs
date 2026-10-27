// Copyright (C) 2024-2025 Takayuki Sato. All Rights Reserved.
// This program is free software under MIT License.
// See the file LICENSE in this distribution for more details.

use crate::options::Options;

/// Converts the input string to kebab case with the specified options.
///
/// ```rust
///     let opts = stringcase::Options{
///       separate_before_non_alphabets: true,
///       separate_after_non_alphabets: true,
///       separators: "",
///       keep: "",
///     };
///     let kebab = stringcase::kebab_case_with_options("fooBar123Baz", &opts);
///     assert_eq!(kebab, "foo-bar-123-baz");
/// ```
pub fn kebab_case_with_options(input: &str, opts: &Options) -> String {
    let mut result = String::with_capacity(input.len() + input.len() / 2);
    // .len returns byte count but ok in this case!

    #[derive(PartialEq)]
    enum ChIs {
        FirstOfStr,
        NextOfUpper,
        NextOfContdUpper,
        NextOfSepMark,
        NextOfKeptMark,
        Other,
    }

    let mut flag = ChIs::FirstOfStr;

    for ch in input.chars() {
        if ch.is_ascii_uppercase() {
            if flag == ChIs::FirstOfStr {
                result.push(ch.to_ascii_lowercase());
                flag = ChIs::NextOfUpper;
            } else if flag == ChIs::NextOfUpper
                || flag == ChIs::NextOfContdUpper
                || (!opts.separate_after_non_alphabets && flag == ChIs::NextOfKeptMark)
            {
                result.push(ch.to_ascii_lowercase());
                flag = ChIs::NextOfContdUpper;
            } else {
                result.push('-');
                result.push(ch.to_ascii_lowercase());
                flag = ChIs::NextOfUpper;
            }
        } else if ch.is_ascii_lowercase() {
            if flag == ChIs::NextOfContdUpper {
                if let Some(prev) = result.pop() {
                    result.push('-');
                    result.push(prev);
                    result.push(ch);
                }
            } else if flag == ChIs::NextOfSepMark
                || (opts.separate_after_non_alphabets && flag == ChIs::NextOfKeptMark)
            {
                result.push('-');
                result.push(ch);
            } else {
                result.push(ch);
            }
            flag = ChIs::Other;
        } else {
            let mut is_kept_char = false;
            if ch.is_ascii_digit() {
                is_kept_char = true;
            } else if !opts.separators.is_empty() {
                if !opts.separators.contains(ch) {
                    is_kept_char = true;
                }
            } else if !opts.keep.is_empty() {
                if opts.keep.contains(ch) {
                    is_kept_char = true;
                }
            }

            if is_kept_char {
                if opts.separate_before_non_alphabets {
                    if flag == ChIs::FirstOfStr || flag == ChIs::NextOfKeptMark {
                        result.push(ch);
                    } else {
                        result.push('-');
                        result.push(ch);
                    }
                } else {
                    if flag != ChIs::NextOfSepMark {
                        result.push(ch);
                    } else {
                        result.push('-');
                        result.push(ch);
                    }
                }
                flag = ChIs::NextOfKeptMark;
            } else {
                if flag != ChIs::FirstOfStr {
                    flag = ChIs::NextOfSepMark;
                }
            }
        }
    }

    result
}

/// Converts the input string to kebab case.
///
/// It treats the end of a sequence of non-alphabetical characters as a word boundary, but not
/// the beginning.
///
/// ```rust
///     let kebab = stringcase::kebab_case("fooBar123Baz");
///     assert_eq!(kebab, "foo-bar123-baz");
/// ```
pub fn kebab_case(input: &str) -> String {
    let opts = Options {
        separate_before_non_alphabets: false,
        separate_after_non_alphabets: true,
        separators: "",
        keep: "",
    };
    kebab_case_with_options(input, &opts)
}

/// Converts the input string to kebab case with the specified separator characters.
#[deprecated(since = "0.4.0", note = "Should use kebab_case_with_options instead")]
pub fn kebab_case_with_sep(input: &str, seps: &str) -> String {
    let opts = Options {
        separate_before_non_alphabets: false,
        separate_after_non_alphabets: true,
        separators: seps,
        keep: "",
    };
    kebab_case_with_options(input, &opts)
}

/// Converts the input string to kebab case with the specified characters to be kept.
#[deprecated(since = "0.4.0", note = "Should use kebab_case_with_options instead")]
pub fn kebab_case_with_keep(input: &str, kept: &str) -> String {
    let opts = Options {
        separate_before_non_alphabets: false,
        separate_after_non_alphabets: true,
        separators: "",
        keep: kept,
    };
    kebab_case_with_options(input, &opts)
}

/// Converts the input string to kebab case.
///
/// It treats the beginning and the end of a sequence of non-alphabetical characters as a word
/// boundary.
#[deprecated(since = "0.4.0", note = "Should use kebab_case_with_options instead")]
pub fn kebab_case_with_nums_as_word(input: &str) -> String {
    let opts = Options {
        separate_before_non_alphabets: true,
        separate_after_non_alphabets: true,
        separators: "",
        keep: "",
    };
    kebab_case_with_options(input, &opts)
}

#[cfg(test)]
mod tests_of_kebab_case {
    use super::*;

    #[test]
    fn convert_camel_case() {
        let result = kebab_case("abcDefGHIjk");
        assert_eq!(result, "abc-def-gh-ijk");
    }

    #[test]
    fn convert_pascal_case() {
        let result = kebab_case("AbcDefGHIjk");
        assert_eq!(result, "abc-def-gh-ijk");
    }

    #[test]
    fn convert_snake_case() {
        let result = kebab_case("abc_def_ghi");
        assert_eq!(result, "abc-def-ghi");
    }

    #[test]
    fn convert_kebab_case() {
        let result = kebab_case("abc-def-ghi");
        assert_eq!(result, "abc-def-ghi");
    }

    #[test]
    fn convert_train_case() {
        let result = kebab_case("Abc-Def-Ghi");
        assert_eq!(result, "abc-def-ghi");
    }

    #[test]
    fn convert_macro_case() {
        let result = kebab_case("ABC_DEF_GHI");
        assert_eq!(result, "abc-def-ghi");
    }

    #[test]
    fn convert_cobol_case() {
        let result = kebab_case("ABC-DEF-GHI");
        assert_eq!(result, "abc-def-ghi");
    }

    #[test]
    fn convert_with_keeping_digits() {
        let result = kebab_case("abc123-456defG89HIJklMN12");
        assert_eq!(result, "abc123-456-def-g89-hi-jkl-mn12");
    }

    #[test]
    fn convert_with_symbols_as_separators() {
        let result = kebab_case(":.abc~!@def#$ghi%&jk(lm)no/?");
        assert_eq!(result, "abc-def-ghi-jk-lm-no");
    }

    #[test]
    fn convert_when_starting_with_digit() {
        let result = kebab_case("123abc456def");
        assert_eq!(result, "123-abc456-def");

        let result = kebab_case("123ABC456DEF");
        assert_eq!(result, "123-abc456-def");

        let result = kebab_case("123Abc456Def");
        assert_eq!(result, "123-abc456-def");
    }

    #[test]
    fn convert_empty_string() {
        let result = kebab_case("");
        assert_eq!(result, "");
    }
}

#[cfg(test)]
mod tests_of_cobol_case_with_options {
    use super::*;

    mod non_alphabets_as_head_of_a_word {
        use super::*;

        #[test]
        fn convert_camel_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abc-def-gh-ijk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abc-def-gh-ijk");
        }

        #[test]
        fn convert_snake_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_train_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_macro_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc-123-456def-g-89hi-jkl-mn-12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, "abc-def-ghi-jk-lm-no");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123abc-456def");

            let result = kebab_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123abc-456def");

            let result = kebab_case_with_options("123Abc456Def", &opts);
            assert_eq!(result, "123-abc-456-def");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("", &opts);
            assert_eq!(result, "");
        }
    }

    mod non_alphabets_as_tail_of_a_word {
        use super::*;

        #[test]
        fn convert_camel_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abc-def-gh-ijk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abc-def-gh-ijk");
        }

        #[test]
        fn convert_snake_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_train_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_macro_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123-456-def-g89-hi-jkl-mn12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, "abc-def-ghi-jk-lm-no");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123-abc456-def");

            let result = kebab_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123-abc456-def");

            let result = kebab_case_with_options("123Abc456Def", &opts);
            assert_eq!(result, "123-abc456-def");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("", &opts);
            assert_eq!(result, "");
        }
    }

    mod non_alphabets_as_a_word {
        use super::*;

        #[test]
        fn convert_camel_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abc-def-gh-ijk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abc-def-gh-ijk");
        }

        #[test]
        fn convert_snake_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_train_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_macro_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc-123-456-def-g-89-hi-jkl-mn-12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, "abc-def-ghi-jk-lm-no");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123-abc-456-def");

            let result = kebab_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123-abc-456-def");

            let result = kebab_case_with_options("123Abc456Def", &opts);
            assert_eq!(result, "123-abc-456-def");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("", &opts);
            assert_eq!(result, "");
        }
    }

    mod non_alphabets_as_part_of_a_word {
        use super::*;

        #[test]
        fn convert_camel_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abc-def-gh-ijk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abc-def-gh-ijk");
        }

        #[test]
        fn convert_snake_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_train_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_macro_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123-456def-g89hi-jkl-mn12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, "abc-def-ghi-jk-lm-no");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123abc456def");

            let result = kebab_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123abc456def");

            let result = kebab_case_with_options("123Abc456Def", &opts);
            assert_eq!(result, "123-abc456-def");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = kebab_case_with_options("", &opts);
            assert_eq!(result, "");
        }
    }

    mod non_alphabets_as_head_of_a_word_with_separators {
        use super::*;

        #[test]
        fn convert_camel_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-_",
                keep: "",
            };

            let result = kebab_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abc-def-gh-ijk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-_",
                keep: "",
            };

            let result = kebab_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abc-def-gh-ijk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "_",
                keep: "",
            };

            let mut result = kebab_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.separators = "-";
            result = kebab_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc-_def-_ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };

            let mut result = kebab_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.separators = "_";
            result = kebab_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc--def--ghi");
        }

        #[test]
        fn convert_train_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };

            let mut result = kebab_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.separators = "_";
            result = kebab_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc---def---ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "_",
                keep: "",
            };

            let mut result = kebab_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.separators = "-";
            result = kebab_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc-_def-_ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };

            let mut result = kebab_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.separators = "_";
            result = kebab_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc--def--ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };

            let mut result = kebab_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc-123-456def-g-89hi-jkl-mn-12");

            opts.separators = "_";
            result = kebab_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc-123-456def-g-89hi-jkl-mn-12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: ":@$&()/",
                keep: "",
            };

            let result = kebab_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, ".abc-~!-def-#-ghi-%-jk-lm-no-?");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };

            let result = kebab_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123abc-456def");

            let result = kebab_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123abc-456def");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-_",
                keep: "",
            };

            let result = kebab_case_with_options("", &opts);
            assert_eq!(result, "");
        }

        #[test]
        fn alphabets_and_numbers_in_separators_are_no_effect() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-b2",
                keep: "",
            };

            let result = kebab_case_with_options("abc123def", &opts);
            assert_eq!(result, "abc-123def");
        }
    }

    mod non_alphabets_as_tail_of_a_word_with_separators {
        use super::*;

        #[test]
        fn convert_camel_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-_",
                keep: "",
            };

            let result = kebab_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abc-def-gh-ijk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-_",
                keep: "",
            };

            let result = kebab_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abc-def-gh-ijk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "_",
                keep: "",
            };

            let mut result = kebab_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.separators = "-";
            result = kebab_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc_-def_-ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };

            let mut result = kebab_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.separators = "_";
            result = kebab_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc--def--ghi");
        }

        #[test]
        fn convert_train_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };

            let mut result = kebab_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.separators = "_";
            result = kebab_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc--def--ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "_",
                keep: "",
            };

            let mut result = kebab_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.separators = "-";
            result = kebab_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc_-def_-ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };

            let mut result = kebab_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.separators = "_";
            result = kebab_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc--def--ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };

            let mut result = kebab_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123-456-def-g89-hi-jkl-mn12");

            opts.separators = "_";
            result = kebab_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123-456-def-g89-hi-jkl-mn12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: ":@$&()/",
                keep: "",
            };

            let result = kebab_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, ".-abc~!-def#-ghi%-jk-lm-no-?");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };

            let result = kebab_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123-abc456-def");

            let result = kebab_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123-abc456-def");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-_",
                keep: "",
            };

            let result = kebab_case_with_options("", &opts);
            assert_eq!(result, "");
        }

        #[test]
        fn alphabets_and_numbers_in_separators_are_no_effect() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-b2",
                keep: "",
            };

            let result = kebab_case_with_options("abc123def", &opts);
            assert_eq!(result, "abc123-def");
        }
    }

    mod non_alphabets_as_a_word_with_separators {
        use super::*;

        #[test]
        fn convert_camel_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "-_",
                keep: "",
            };

            let result = kebab_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abc-def-gh-ijk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "-_",
                keep: "",
            };

            let result = kebab_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abc-def-gh-ijk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "_",
                keep: "",
            };

            let mut result = kebab_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.separators = "-";
            result = kebab_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc-_-def-_-ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };

            let mut result = kebab_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.separators = "_";
            result = kebab_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc---def---ghi");
        }

        #[test]
        fn convert_train_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };

            let mut result = kebab_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.separators = "_";
            result = kebab_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc---def---ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "_",
                keep: "",
            };

            let mut result = kebab_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.separators = "-";
            result = kebab_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc-_-def-_-ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };

            let mut result = kebab_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.separators = "_";
            result = kebab_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc---def---ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };

            let mut result = kebab_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc-123-456-def-g-89-hi-jkl-mn-12");

            opts.separators = "_";
            result = kebab_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc-123-456-def-g-89-hi-jkl-mn-12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: ":@$&()/",
                keep: "",
            };

            let result = kebab_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, ".-abc-~!-def-#-ghi-%-jk-lm-no-?");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };

            let result = kebab_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123-abc-456-def");

            let result = kebab_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123-abc-456-def");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "-_",
                keep: "",
            };

            let result = kebab_case_with_options("", &opts);
            assert_eq!(result, "");
        }

        #[test]
        fn alphabets_and_numbers_in_separators_are_no_effect() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "-b2",
                keep: "",
            };

            let result = kebab_case_with_options("abc123def", &opts);
            assert_eq!(result, "abc-123-def");
        }
    }

    mod non_alphabets_as_part_of_a_word_with_separators {
        use super::*;

        #[test]
        fn convert_camel_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "-_",
                keep: "",
            };

            let result = kebab_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abc-def-gh-ijk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "-_",
                keep: "",
            };

            let result = kebab_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abc-def-gh-ijk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "_",
                keep: "",
            };

            let mut result = kebab_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.separators = "-";
            result = kebab_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };

            let mut result = kebab_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.separators = "_";
            result = kebab_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_train_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };

            let mut result = kebab_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.separators = "_";
            result = kebab_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc--def--ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "_",
                keep: "",
            };

            let mut result = kebab_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.separators = "-";
            result = kebab_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };

            let mut result = kebab_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.separators = "_";
            result = kebab_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };

            let mut result = kebab_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123-456def-g89hi-jkl-mn12");

            opts.separators = "_";
            result = kebab_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123-456def-g89hi-jkl-mn12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: ":@$&()/",
                keep: "",
            };

            let result = kebab_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, ".abc~!-def#-ghi%-jk-lm-no-?");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };

            let result = kebab_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123abc456def");

            let result = kebab_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123abc456def");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "-_",
                keep: "",
            };

            let result = kebab_case_with_options("", &opts);
            assert_eq!(result, "");
        }

        #[test]
        fn alphabets_and_numbers_in_separators_are_no_effect() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "-b2",
                keep: "",
            };

            let result = kebab_case_with_options("abc123def", &opts);
            assert_eq!(result, "abc123def");
        }
    }

    mod non_alphabets_as_head_of_a_word_with_kept_characters {
        use super::*;

        #[test]
        fn convert_camel_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-_",
            };

            let result = kebab_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abc-def-gh-ijk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-_",
            };

            let result = kebab_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abc-def-gh-ijk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-",
            };

            let mut result = kebab_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.keep = "_";
            result = kebab_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc-_def-_ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "_",
            };

            let mut result = kebab_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.keep = "-";
            result = kebab_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc--def--ghi");
        }

        #[test]
        fn convert_train_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "_",
            };

            let mut result = kebab_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.keep = "-";
            result = kebab_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc---def---ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-",
            };

            let mut result = kebab_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.keep = "_";
            result = kebab_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc-_def-_ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "_",
            };

            let mut result = kebab_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.keep = "-";
            result = kebab_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc--def--ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "_",
            };

            let mut result = kebab_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc-123-456def-g-89hi-jkl-mn-12");

            opts.keep = "-";
            result = kebab_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc-123-456def-g-89hi-jkl-mn-12");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-",
            };

            let result = kebab_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123abc-456def");

            let result = kebab_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123abc-456def");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: ".~!#%?",
            };

            let result = kebab_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, ".abc-~!-def-#-ghi-%-jk-lm-no-?");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-_",
            };

            let result = kebab_case_with_options("", &opts);
            assert_eq!(result, "");
        }
    }

    mod non_alphabets_as_tail_of_a_word_with_kept_characters {
        use super::*;

        #[test]
        fn convert_camel_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-_",
            };

            let result = kebab_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abc-def-gh-ijk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-_",
            };

            let result = kebab_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abc-def-gh-ijk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-",
            };

            let mut result = kebab_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.keep = "_";
            result = kebab_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc_-def_-ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };

            let mut result = kebab_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.keep = "-";
            result = kebab_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc--def--ghi");
        }

        #[test]
        fn convert_train_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };

            let mut result = kebab_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.keep = "-";
            result = kebab_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc--def--ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-",
            };

            let mut result = kebab_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.keep = "_";
            result = kebab_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc_-def_-ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };

            let mut result = kebab_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.keep = "-";
            result = kebab_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc--def--ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };

            let mut result = kebab_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123-456-def-g89-hi-jkl-mn12");

            opts.keep = "-";
            result = kebab_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123-456-def-g89-hi-jkl-mn12");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-",
            };

            let result = kebab_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123-abc456-def");

            opts.keep = "_";
            let result = kebab_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123-abc456-def");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: ".~!#%?",
            };

            let result = kebab_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, ".-abc~!-def#-ghi%-jk-lm-no-?");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-_",
            };

            let result = kebab_case_with_options("", &opts);
            assert_eq!(result, "");
        }
    }

    mod non_alphabets_as_a_word_with_kept_characters {
        use super::*;

        #[test]
        fn convert_camel_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-_",
            };

            let result = kebab_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abc-def-gh-ijk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-_",
            };

            let result = kebab_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abc-def-gh-ijk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-",
            };

            let mut result = kebab_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.keep = "_";
            result = kebab_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc-_-def-_-ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };

            let mut result = kebab_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.keep = "-";
            result = kebab_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc---def---ghi");
        }

        #[test]
        fn convert_train_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };

            let mut result = kebab_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.keep = "-";
            result = kebab_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc---def---ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-",
            };

            let mut result = kebab_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.keep = "_";
            result = kebab_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc-_-def-_-ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };

            let mut result = kebab_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.keep = "-";
            result = kebab_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc---def---ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };

            let mut result = kebab_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc-123-456-def-g-89-hi-jkl-mn-12");

            opts.keep = "-";
            result = kebab_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc-123-456-def-g-89-hi-jkl-mn-12");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-",
            };

            let result = kebab_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123-abc-456-def");

            let result = kebab_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123-abc-456-def");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: ".~!#%?",
            };

            let result = kebab_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, ".-abc-~!-def-#-ghi-%-jk-lm-no-?");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-_",
            };

            let result = kebab_case_with_options("", &opts);
            assert_eq!(result, "");
        }
    }

    mod non_alphabets_as_part_of_a_word_with_kept_characters {
        use super::*;

        #[test]
        fn convert_camel_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-_",
            };

            let result = kebab_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abc-def-gh-ijk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-_",
            };

            let result = kebab_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abc-def-gh-ijk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-",
            };

            let mut result = kebab_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.keep = "_";
            result = kebab_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "_",
            };

            let mut result = kebab_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.keep = "-";
            result = kebab_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_train_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "_",
            };

            let mut result = kebab_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.keep = "-";
            result = kebab_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc--def--ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-",
            };

            let mut result = kebab_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.keep = "_";
            result = kebab_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "_",
            };

            let mut result = kebab_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc-def-ghi");

            opts.keep = "-";
            result = kebab_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "_",
            };

            let mut result = kebab_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123-456def-g89hi-jkl-mn12");

            opts.keep = "-";
            result = kebab_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123-456def-g89hi-jkl-mn12");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-",
            };

            let result = kebab_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123abc456def");

            let result = kebab_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123abc456def");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: ".~!#%?",
            };

            let result = kebab_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, ".abc~!-def#-ghi%-jk-lm-no-?");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-_",
            };

            let result = kebab_case_with_options("", &opts);
            assert_eq!(result, "");
        }
    }
}
