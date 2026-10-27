// Copyright (C) 2024-2025 Takayuki Sato. All Rights Reserved.
// This program is free software under MIT License.
// See the file LICENSE in this distribution for more details.

use crate::options::Options;

/// Converts the input string to snake case with the specified options.
///
/// ```rust
///     let opts = stringcase::Options{
///       separate_before_non_alphabets: true,
///       separate_after_non_alphabets: true,
///       separators: "",
///       keep: "",
///     };
///     let snake = stringcase::snake_case_with_options("fooBar123Baz", &opts);
///     assert_eq!(snake, "foo_bar_123_baz");
/// ```
pub fn snake_case_with_options(input: &str, opts: &Options) -> String {
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
                result.push('_');
                result.push(ch.to_ascii_lowercase());
                flag = ChIs::NextOfUpper;
            }
        } else if ch.is_ascii_lowercase() {
            if flag == ChIs::NextOfContdUpper {
                if let Some(prev) = result.pop() {
                    result.push('_');
                    result.push(prev);
                    result.push(ch);
                }
            } else if flag == ChIs::NextOfSepMark
                || (opts.separate_after_non_alphabets && flag == ChIs::NextOfKeptMark)
            {
                result.push('_');
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
                        result.push('_');
                        result.push(ch);
                    }
                } else {
                    if flag != ChIs::NextOfSepMark {
                        result.push(ch);
                    } else {
                        result.push('_');
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

/// Converts the input string to snake case.
///
/// It treats the end of a sequence of non-alphabetical characters as a word boundary, but not
/// the beginning.
///
/// ```rust
///     let snake = stringcase::snake_case("fooBar123Baz");
///     assert_eq!(snake, "foo_bar123_baz");
/// ```
pub fn snake_case(input: &str) -> String {
    let opts = Options {
        separate_before_non_alphabets: false,
        separate_after_non_alphabets: true,
        separators: "",
        keep: "",
    };
    snake_case_with_options(input, &opts)
}

/// Converts the input string to snake case with the specified separator characters.
#[deprecated(since = "0.4.0", note = "Should use snake_case_with_options instead")]
pub fn snake_case_with_sep(input: &str, seps: &str) -> String {
    let opts = Options {
        separate_before_non_alphabets: false,
        separate_after_non_alphabets: true,
        separators: seps,
        keep: "",
    };
    snake_case_with_options(input, &opts)
}

/// Converts the input string to snake case with the specified characters to be kept.
#[deprecated(since = "0.4.0", note = "Should use snake_case_with_options instead")]
pub fn snake_case_with_keep(input: &str, kept: &str) -> String {
    let opts = Options {
        separate_before_non_alphabets: false,
        separate_after_non_alphabets: true,
        separators: "",
        keep: kept,
    };
    snake_case_with_options(input, &opts)
}

/// Converts the input string to snake case.
///
/// It treats the beginning and the end of a sequence of non-alphabetical characters as a word
/// boundary.
#[deprecated(since = "0.4.0", note = "Should use snake_case_with_options instead")]
pub fn snake_case_with_nums_as_word(input: &str) -> String {
    let opts = Options {
        separate_before_non_alphabets: true,
        separate_after_non_alphabets: true,
        separators: "",
        keep: "",
    };
    snake_case_with_options(input, &opts)
}

#[cfg(test)]
mod tests_of_snake_case {
    use super::*;

    #[test]
    fn convert_camel_case() {
        let result = snake_case("abcDefGHIjk");
        assert_eq!(result, "abc_def_gh_ijk");
    }

    #[test]
    fn convert_pascal_case() {
        let result = snake_case("AbcDefGHIjk");
        assert_eq!(result, "abc_def_gh_ijk");
    }

    #[test]
    fn convert_snake_case() {
        let result = snake_case("abc_def_ghi");
        assert_eq!(result, "abc_def_ghi");
    }

    #[test]
    fn convert_kebab_case() {
        let result = snake_case("abc-def-ghi");
        assert_eq!(result, "abc_def_ghi");
    }

    #[test]
    fn convert_train_case() {
        let result = snake_case("Abc-Def-Ghi");
        assert_eq!(result, "abc_def_ghi");
    }

    #[test]
    fn convert_macro_case() {
        let result = snake_case("ABC_DEF_GHI");
        assert_eq!(result, "abc_def_ghi");
    }

    #[test]
    fn convert_cobol_case() {
        let result = snake_case("ABC-DEF-GHI");
        assert_eq!(result, "abc_def_ghi");
    }

    #[test]
    fn convert_with_keeping_digits() {
        let result = snake_case("abc123-456defG89HIJklMN12");
        assert_eq!(result, "abc123_456_def_g89_hi_jkl_mn12");
    }

    #[test]
    fn convert_with_symbols_as_separators() {
        let result = snake_case(":.abc~!@def#$ghi%&jk(lm)no/?");
        assert_eq!(result, "abc_def_ghi_jk_lm_no");
    }

    #[test]
    fn convert_when_starting_with_digit() {
        let result = snake_case("123abc456def");
        assert_eq!(result, "123_abc456_def");

        let result = snake_case("123ABC456DEF");
        assert_eq!(result, "123_abc456_def");

        let result = snake_case("123Abc456Def");
        assert_eq!(result, "123_abc456_def");
    }

    #[test]
    fn convert_empty_string() {
        let result = snake_case("");
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
            let result = snake_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abc_def_gh_ijk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abc_def_gh_ijk");
        }

        #[test]
        fn convert_snake_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_train_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_macro_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc_123_456def_g_89hi_jkl_mn_12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, "abc_def_ghi_jk_lm_no");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123abc_456def");

            let result = snake_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123abc_456def");

            let result = snake_case_with_options("123Abc456Def", &opts);
            assert_eq!(result, "123_abc_456_def");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("", &opts);
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
            let result = snake_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abc_def_gh_ijk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abc_def_gh_ijk");
        }

        #[test]
        fn convert_snake_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_train_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_macro_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123_456_def_g89_hi_jkl_mn12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, "abc_def_ghi_jk_lm_no");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123_abc456_def");

            let result = snake_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123_abc456_def");

            let result = snake_case_with_options("123Abc456Def", &opts);
            assert_eq!(result, "123_abc456_def");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("", &opts);
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
            let result = snake_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abc_def_gh_ijk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abc_def_gh_ijk");
        }

        #[test]
        fn convert_snake_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_train_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_macro_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc_123_456_def_g_89_hi_jkl_mn_12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, "abc_def_ghi_jk_lm_no");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123_abc_456_def");

            let result = snake_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123_abc_456_def");

            let result = snake_case_with_options("123Abc456Def", &opts);
            assert_eq!(result, "123_abc_456_def");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("", &opts);
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
            let result = snake_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abc_def_gh_ijk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abc_def_gh_ijk");
        }

        #[test]
        fn convert_snake_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_train_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_macro_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123_456def_g89hi_jkl_mn12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, "abc_def_ghi_jk_lm_no");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123abc456def");

            let result = snake_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123abc456def");

            let result = snake_case_with_options("123Abc456Def", &opts);
            assert_eq!(result, "123_abc456_def");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };
            let result = snake_case_with_options("", &opts);
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
            let result = snake_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abc_def_gh_ijk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-_",
                keep: "",
            };
            let result = snake_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abc_def_gh_ijk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "_",
                keep: "",
            };
            let result = snake_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.separators = "-";
            let result = snake_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc__def__ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };
            let result = snake_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.separators = "_";
            let result = snake_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc_-def_-ghi");
        }

        #[test]
        fn convert_train_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };
            let result = snake_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.separators = "_";
            let result = snake_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc_-_def_-_ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "_",
                keep: "",
            };
            let result = snake_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.separators = "-";
            let result = snake_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc__def__ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };
            let result = snake_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.separators = "_";
            let result = snake_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc_-def_-ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };
            let result = snake_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc_123_456def_g_89hi_jkl_mn_12");

            opts.separators = "_";
            let result = snake_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc_123-456def_g_89hi_jkl_mn_12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: ":@$&()/",
                keep: "",
            };
            let result = snake_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, ".abc_~!_def_#_ghi_%_jk_lm_no_?");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };
            let result = snake_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123abc_456def");

            let result = snake_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123abc_456def");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-_",
                keep: "",
            };
            let result = snake_case_with_options("", &opts);
            assert_eq!(result, "");
        }

        #[test]
        fn alphabets_and_numbers_in_separators_have_no_effect() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-b2",
                keep: "",
            };
            let result = snake_case_with_options("abc123def", &opts);
            assert_eq!(result, "abc_123def");
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
            let result = snake_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abc_def_gh_ijk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-_",
                keep: "",
            };
            let result = snake_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abc_def_gh_ijk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "_",
                keep: "",
            };
            let result = snake_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.separators = "-";
            let result = snake_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc__def__ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };
            let result = snake_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.separators = "_";
            let result = snake_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc-_def-_ghi");
        }

        #[test]
        fn convert_train_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };
            let result = snake_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.separators = "_";
            let result = snake_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc-_def-_ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "_",
                keep: "",
            };
            let result = snake_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.separators = "-";
            let result = snake_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc__def__ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };
            let result = snake_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.separators = "_";
            let result = snake_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc-_def-_ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };
            let result = snake_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123_456_def_g89_hi_jkl_mn12");

            opts.separators = "_";
            let result = snake_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123-456_def_g89_hi_jkl_mn12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: ":@$&()/",
                keep: "",
            };
            let result = snake_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, "._abc~!_def#_ghi%_jk_lm_no_?");
        }

        #[test]
        fn convert_with_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };
            let result = snake_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123_abc456_def");

            let result = snake_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123_abc456_def");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-_",
                keep: "",
            };
            let result = snake_case_with_options("", &opts);
            assert_eq!(result, "");
        }

        #[test]
        fn alphabets_and_numbers_in_separators_no_effect() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-b2",
                keep: "",
            };
            let result = snake_case_with_options("abc123def", &opts);
            assert_eq!(result, "abc123_def");
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
            let result = snake_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abc_def_gh_ijk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "-_",
                keep: "",
            };
            let result = snake_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abc_def_gh_ijk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "_",
                keep: "",
            };
            let result = snake_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.separators = "-";
            let result = snake_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc___def___ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };
            let result = snake_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.separators = "_";
            let result = snake_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc_-_def_-_ghi");
        }

        #[test]
        fn convert_train_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };
            let result = snake_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.separators = "_";
            let result = snake_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc_-_def_-_ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "_",
                keep: "",
            };
            let result = snake_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.separators = "-";
            let result = snake_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc___def___ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };
            let result = snake_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.separators = "_";
            let result = snake_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc_-_def_-_ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };
            let result = snake_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc_123_456_def_g_89_hi_jkl_mn_12");

            opts.separators = "_";
            let result = snake_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc_123-456_def_g_89_hi_jkl_mn_12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: ":@$&()/",
                keep: "",
            };
            let result = snake_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, "._abc_~!_def_#_ghi_%_jk_lm_no_?");
        }

        #[test]
        fn convert_with_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };
            let result = snake_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123_abc_456_def");

            let result = snake_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123_abc_456_def");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "-_",
                keep: "",
            };
            let result = snake_case_with_options("", &opts);
            assert_eq!(result, "");
        }

        #[test]
        fn alphabets_and_numbers_in_separators_have_no_effect() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "-b2",
                keep: "",
            };
            let result = snake_case_with_options("abc123def", &opts);
            assert_eq!(result, "abc_123_def");
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
            let result = snake_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abc_def_gh_ijk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "-_",
                keep: "",
            };
            let result = snake_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abc_def_gh_ijk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "_",
                keep: "",
            };
            let result = snake_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.separators = "-";
            let result = snake_case_with_options("abc_def_ghi", &opts);
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
            let result = snake_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.separators = "_";
            let result = snake_case_with_options("abc-def-ghi", &opts);
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
            let result = snake_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.separators = "_";
            let result = snake_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc-_def-_ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "_",
                keep: "",
            };
            let result = snake_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.separators = "-";
            let result = snake_case_with_options("ABC_DEF_GHI", &opts);
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
            let result = snake_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.separators = "_";
            let result = snake_case_with_options("ABC-DEF-GHI", &opts);
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
            let result = snake_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123_456def_g89hi_jkl_mn12");

            opts.separators = "_";
            let result = snake_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123-456def_g89hi_jkl_mn12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: ":@$&()/",
                keep: "",
            };
            let result = snake_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, ".abc~!_def#_ghi%_jk_lm_no_?");
        }

        #[test]
        fn convert_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };
            let result = snake_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123abc456def");

            let result = snake_case_with_options("123ABC456DEF", &opts);
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
            let result = snake_case_with_options("", &opts);
            assert_eq!(result, "");
        }

        #[test]
        fn alphabets_and_numbers_in_separators_no_effect() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "-b2",
                keep: "",
            };
            let result = snake_case_with_options("abc123def", &opts);
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
            let result = snake_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abc_def_gh_ijk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-_",
            };
            let result = snake_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abc_def_gh_ijk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-",
            };
            let result = snake_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.keep = "_";
            let result = snake_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc__def__ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "_",
            };
            let result = snake_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.keep = "-";
            let result = snake_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc_-def_-ghi");
        }

        #[test]
        fn convert_train_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "_",
            };
            let result = snake_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.keep = "-";
            let result = snake_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc_-_def_-_ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-",
            };
            let result = snake_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.keep = "_";
            let result = snake_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc__def__ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "_",
            };
            let result = snake_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.keep = "-";
            let result = snake_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc_-def_-ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "_",
            };
            let result = snake_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc_123_456def_g_89hi_jkl_mn_12");

            opts.keep = "-";
            let result = snake_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc_123-456def_g_89hi_jkl_mn_12");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-",
            };
            let result = snake_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123abc_456def");

            let result = snake_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123abc_456def");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: ".~!#%?",
            };
            let result = snake_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, ".abc_~!_def_#_ghi_%_jk_lm_no_?");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-_",
            };
            let result = snake_case_with_options("", &opts);
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
            let result = snake_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abc_def_gh_ijk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-_",
            };
            let result = snake_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abc_def_gh_ijk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-",
            };
            let result = snake_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.keep = "_";
            let result = snake_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc__def__ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };
            let result = snake_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.keep = "-";
            let result = snake_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc-_def-_ghi");
        }

        #[test]
        fn convert_train_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };
            let result = snake_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.keep = "-";
            let result = snake_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc-_def-_ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-",
            };
            let result = snake_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.keep = "_";
            let result = snake_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc__def__ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };
            let result = snake_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.keep = "-";
            let result = snake_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc-_def-_ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };
            let result = snake_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123_456_def_g89_hi_jkl_mn12");

            opts.keep = "-";
            let result = snake_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123-456_def_g89_hi_jkl_mn12");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-",
            };
            let result = snake_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123_abc456_def");

            opts.keep = "_";
            let result = snake_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123_abc456_def");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: ".~!#%?",
            };
            let result = snake_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, "._abc~!_def#_ghi%_jk_lm_no_?");
        }

        #[test]
        fn convert_an_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-_",
            };
            let result = snake_case_with_options("", &opts);
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
            let result = snake_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abc_def_gh_ijk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-_",
            };
            let result = snake_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abc_def_gh_ijk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-",
            };
            let result = snake_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.keep = "_";
            let result = snake_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc___def___ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };
            let result = snake_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.keep = "-";
            let result = snake_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc_-_def_-_ghi");
        }

        #[test]
        fn convert_train_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };
            let result = snake_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.keep = "-";
            let result = snake_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc_-_def_-_ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-",
            };
            let result = snake_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.keep = "_";
            let result = snake_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc___def___ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };
            let result = snake_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.keep = "-";
            let result = snake_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc_-_def_-_ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };
            let result = snake_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc_123_456_def_g_89_hi_jkl_mn_12");

            opts.keep = "-";
            let result = snake_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc_123-456_def_g_89_hi_jkl_mn_12");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-",
            };
            let result = snake_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123_abc_456_def");

            let result = snake_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123_abc_456_def");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: ".~!#%?",
            };
            let result = snake_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, "._abc_~!_def_#_ghi_%_jk_lm_no_?");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-_",
            };
            let result = snake_case_with_options("", &opts);
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
            let result = snake_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abc_def_gh_ijk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-_",
            };
            let result = snake_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abc_def_gh_ijk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-",
            };
            let result = snake_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.keep = "_";
            let result = snake_case_with_options("abc_def_ghi", &opts);
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
            let result = snake_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.keep = "-";
            let result = snake_case_with_options("abc-def-ghi", &opts);
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
            let result = snake_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.keep = "-";
            let result = snake_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc-_def-_ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-",
            };
            let result = snake_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.keep = "_";
            let result = snake_case_with_options("ABC_DEF_GHI", &opts);
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
            let result = snake_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc_def_ghi");

            opts.keep = "-";
            let result = snake_case_with_options("ABC-DEF-GHI", &opts);
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
            let result = snake_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123_456def_g89hi_jkl_mn12");

            opts.keep = "-";
            let result = snake_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123-456def_g89hi_jkl_mn12");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-",
            };
            let result = snake_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123abc456def");

            let result = snake_case_with_options("123ABC456DEF", &opts);
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
            let result = snake_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, ".abc~!_def#_ghi%_jk_lm_no_?");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-_",
            };
            let result = snake_case_with_options("", &opts);
            assert_eq!(result, "");
        }
    }
}
