// Copyright (C) 2024-2025 Takayuki Sato. All Rights Reserved.
// This program is free software under MIT License.
// See the file LICENSE in this distribution for more details.

use crate::options::Options;

/// Converts the input string to pascal case with the specified options.
///
/// ```rust
///     let opts = stringcase::Options{
///       separate_before_non_alphabets: true,
///       separate_after_non_alphabets: true,
///       separators: "",
///       keep: "",
///     };
///     let pascal = stringcase::pascal_case_with_options("foo_bar_100_baz", &opts);
///     assert_eq!(pascal, "FooBar100Baz");
/// ```
pub fn pascal_case_with_options(input: &str, opts: &Options) -> String {
    let mut result = String::with_capacity(input.len());
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
            if flag == ChIs::NextOfUpper
                || flag == ChIs::NextOfContdUpper
                || (!opts.separate_after_non_alphabets && flag == ChIs::NextOfKeptMark)
            {
                result.push(ch.to_ascii_lowercase());
                flag = ChIs::NextOfContdUpper;
            } else {
                result.push(ch);
                flag = ChIs::NextOfUpper;
            }
        } else if ch.is_ascii_lowercase() {
            if flag == ChIs::NextOfContdUpper {
                if let Some(prev) = result.pop() {
                    result.push(prev.to_ascii_uppercase());
                    result.push(ch);
                }
            } else if flag == ChIs::NextOfSepMark
                || flag == ChIs::FirstOfStr
                || (opts.separate_after_non_alphabets && flag == ChIs::NextOfKeptMark)
            {
                result.push(ch.to_ascii_uppercase());
            } else {
                result.push(ch);
            }
            flag = ChIs::Other;
        } else if ch.is_ascii_digit() {
            result.push(ch);
            flag = ChIs::NextOfKeptMark;
        } else if !opts.separators.is_empty() {
            if !opts.separators.contains(ch) {
                result.push(ch);
                flag = ChIs::NextOfKeptMark;
            } else if flag != ChIs::FirstOfStr {
                flag = ChIs::NextOfSepMark;
            }
        } else if !opts.keep.is_empty() {
            if opts.keep.contains(ch) {
                result.push(ch);
                flag = ChIs::NextOfKeptMark;
            } else if flag != ChIs::FirstOfStr {
                flag = ChIs::NextOfSepMark;
            }
        } else if flag != ChIs::FirstOfStr {
            flag = ChIs::NextOfSepMark;
        }
    }

    result
}

/// Converts the input string to pascal case.
///
/// It treats the end of a sequence of non-alphabetical characters as a word boundary, but not
/// the beginning.
///
/// ```rust
///     let pascal = stringcase::pascal_case("foo_bar100_baz");
///     assert_eq!(pascal, "FooBar100Baz");
/// ```
pub fn pascal_case(input: &str) -> String {
    let opts = Options {
        separate_before_non_alphabets: false,
        separate_after_non_alphabets: true,
        separators: "",
        keep: "",
    };
    pascal_case_with_options(input, &opts)
}

/// Converts the input string to pascal case with the specified separator characters.
#[deprecated(since = "0.4.0", note = "Should use pascal_case_with_options instead")]
pub fn pascal_case_with_sep(input: &str, seps: &str) -> String {
    let opts = Options {
        separate_before_non_alphabets: false,
        separate_after_non_alphabets: true,
        separators: seps,
        keep: "",
    };
    pascal_case_with_options(input, &opts)
}

/// Converts the input string to pascal case with the specified characters to be kept.
#[deprecated(since = "0.4.0", note = "Should use pascal_case_with_options instead")]
pub fn pascal_case_with_keep(input: &str, kept: &str) -> String {
    let opts = Options {
        separate_before_non_alphabets: false,
        separate_after_non_alphabets: true,
        separators: "",
        keep: kept,
    };
    pascal_case_with_options(input, &opts)
}

#[cfg(test)]
mod tests_of_pascal_case {
    use super::*;

    #[test]
    fn convert_camel_case() {
        let result = pascal_case("abcDefGHIjk");
        assert_eq!(result, "AbcDefGhIjk");
    }

    #[test]
    fn convert_pascal_case() {
        let result = pascal_case("AbcDefGHIjk");
        assert_eq!(result, "AbcDefGhIjk");
    }

    #[test]
    fn convert_snake_case() {
        let result = pascal_case("abc_def_ghi");
        assert_eq!(result, "AbcDefGhi");
    }

    #[test]
    fn convert_kebab_case() {
        let result = pascal_case("abc-def-ghi");
        assert_eq!(result, "AbcDefGhi");
    }

    #[test]
    fn convert_train_case() {
        let result = pascal_case("Abc-Def-Ghi");
        assert_eq!(result, "AbcDefGhi");
    }

    #[test]
    fn convert_macro_case() {
        let result = pascal_case("ABC_DEF_GHI");
        assert_eq!(result, "AbcDefGhi");
    }

    #[test]
    fn convert_cobol_case() {
        let result = pascal_case("ABC-DEF-GHI");
        assert_eq!(result, "AbcDefGhi");
    }

    #[test]
    fn convert_with_keeping_digits() {
        let result = pascal_case("abc123-456defG89HIJklMN12");
        assert_eq!(result, "Abc123456DefG89HiJklMn12");
    }

    #[test]
    fn convert_with_symbols_as_separators() {
        let result = pascal_case(":.abc~!@def#$ghi%&jk(lm)no/?");
        assert_eq!(result, "AbcDefGhiJkLmNo");
    }

    #[test]
    fn convert_when_starting_with_digit() {
        let result = pascal_case("123abc456def");
        assert_eq!(result, "123Abc456Def");

        let result = pascal_case("123ABC456DEF");
        assert_eq!(result, "123Abc456Def");

        let result = pascal_case("123Abc456Def");
        assert_eq!(result, "123Abc456Def");
    }

    #[test]
    fn convert_empty_string() {
        let result = pascal_case("");
        assert_eq!(result, "");
    }
}

#[cfg(test)]
mod tests_of_pascal_case_with_options {
    use super::*;

    mod non_alphabets_as_head_of_a_word {
        use super::*;

        #[test]
        fn convert_camel_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "AbcDefGhIjk");
        }

        #[test]
        fn convert_pascal_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "AbcDefGhIjk");
        }

        #[test]
        fn convert_snake_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "AbcDefGhi");
        }

        #[test]
        fn convert_kebab_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "AbcDefGhi");
        }

        #[test]
        fn convert_train_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "AbcDefGhi");
        }

        #[test]
        fn convert_macro_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "AbcDefGhi");
        }

        #[test]
        fn convert_cobol_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "AbcDefGhi");
        }

        #[test]
        fn convert_with_keeping_digits_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "Abc123456defG89hiJklMn12");
        }

        #[test]
        fn convert_with_symbols_as_separators_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, "AbcDefGhiJkLmNo");
        }

        #[test]
        fn convert_when_starting_with_digit_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123abc456def");

            let result = pascal_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123abc456def");

            let result = pascal_case_with_options("123Abc456Def", &opts);
            assert_eq!(result, "123Abc456Def");
        }

        #[test]
        fn convert_empty_string_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("", &opts);
            assert_eq!(result, "");
        }
    }

    mod non_alphabets_as_tail_of_a_word {
        use super::*;

        #[test]
        fn convert_camel_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "AbcDefGhIjk");
        }

        #[test]
        fn convert_pascal_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "AbcDefGhIjk");
        }

        #[test]
        fn convert_snake_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "AbcDefGhi");
        }

        #[test]
        fn convert_kebab_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "AbcDefGhi");
        }

        #[test]
        fn convert_train_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "AbcDefGhi");
        }

        #[test]
        fn convert_macro_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "AbcDefGhi");
        }

        #[test]
        fn convert_cobol_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "AbcDefGhi");
        }

        #[test]
        fn convert_with_keeping_digits_with_options() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "Abc123456DefG89HiJklMn12");
        }

        #[test]
        fn convert_with_symbols_as_separators_with_options() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, "AbcDefGhiJkLmNo");
        }

        #[test]
        fn convert_when_starting_with_digit_with_options() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123Abc456Def");

            let result = pascal_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123Abc456Def");

            let result = pascal_case_with_options("123Abc456Def", &opts);
            assert_eq!(result, "123Abc456Def");
        }

        #[test]
        fn convert_empty_string_with_options() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("", &opts);
            assert_eq!(result, "");
        }
    }

    mod non_alphabets_as_a_word {
        use super::*;

        #[test]
        fn convert_camel_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "AbcDefGhIjk");
        }

        #[test]
        fn convert_pascal_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "AbcDefGhIjk");
        }

        #[test]
        fn convert_snake_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "AbcDefGhi");
        }

        #[test]
        fn convert_kebab_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "AbcDefGhi");
        }

        #[test]
        fn convert_train_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "AbcDefGhi");
        }

        #[test]
        fn convert_macro_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "AbcDefGhi");
        }

        #[test]
        fn convert_cobol_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "AbcDefGhi");
        }

        #[test]
        fn convert_with_keeping_digits_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "Abc123456DefG89HiJklMn12");
        }

        #[test]
        fn convert_with_symbols_as_separators_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, "AbcDefGhiJkLmNo");
        }

        #[test]
        fn convert_when_starting_with_digit_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123Abc456Def");

            let result = pascal_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123Abc456Def");

            let result = pascal_case_with_options("123Abc456Def", &opts);
            assert_eq!(result, "123Abc456Def");
        }

        #[test]
        fn convert_empty_string_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("", &opts);
            assert_eq!(result, "");
        }
    }

    mod non_alphabets_as_part_a_word {
        use super::*;

        #[test]
        fn convert_camel_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "AbcDefGhIjk");
        }

        #[test]
        fn convert_pascal_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "AbcDefGhIjk");
        }

        #[test]
        fn convert_snake_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "AbcDefGhi");
        }

        #[test]
        fn convert_kebab_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "AbcDefGhi");
        }

        #[test]
        fn convert_train_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "AbcDefGhi");
        }

        #[test]
        fn convert_macro_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "AbcDefGhi");
        }

        #[test]
        fn convert_cobol_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "AbcDefGhi");
        }

        #[test]
        fn convert_with_keeping_digits_with_options() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "Abc123456defG89hiJklMn12");
        }

        #[test]
        fn convert_with_symbols_as_separators_with_options() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, "AbcDefGhiJkLmNo");
        }

        #[test]
        fn convert_when_starting_with_digit_with_options() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123abc456def");

            let result = pascal_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123abc456def");

            let result = pascal_case_with_options("123Abc456Def", &opts);
            assert_eq!(result, "123Abc456Def");
        }

        #[test]
        fn convert_empty_string_with_options() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let result = pascal_case_with_options("", &opts);
            assert_eq!(result, "");
        }
    }

    mod non_alphabets_as_head_a_word_with_separators {
        use super::*;

        #[test]
        fn convert_camel_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-_",
                keep: "",
            };

            let result = pascal_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "AbcDefGhIjk");
        }

        #[test]
        fn convert_pascal_case_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-_",
                keep: "",
            };

            let result = pascal_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "AbcDefGhIjk");
        }

        #[test]
        fn convert_snake_case_with_options() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "_",
                keep: "",
            };

            let result = pascal_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.separators = "-";
            let result = pascal_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "Abc_def_ghi");
        }

        #[test]
        fn convert_kebab_case_with_options() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };

            let result = pascal_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.separators = "_";
            let result = pascal_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "Abc-def-ghi");
        }

        #[test]
        fn convert_train_case_with_options() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };

            let result = pascal_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.separators = "_";
            let result = pascal_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "Abc-Def-Ghi");
        }

        #[test]
        fn convert_macro_case_with_options() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "_",
                keep: "",
            };

            let result = pascal_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.separators = "-";
            let result = pascal_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "Abc_def_ghi");
        }

        #[test]
        fn convert_cobol_case_with_options() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };

            let result = pascal_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.separators = "_";
            let result = pascal_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "Abc-def-ghi");
        }

        #[test]
        fn convert_with_keeping_digits_with_options() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };

            let result = pascal_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "Abc123456defG89hiJklMn12");

            opts.separators = "_";
            let result = pascal_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "Abc123-456defG89hiJklMn12");
        }

        #[test]
        fn convert_with_symbols_as_separators_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: ":@$&()/",
                keep: "",
            };

            let result = pascal_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, ".abc~!Def#Ghi%JkLmNo?");
        }

        #[test]
        fn convert_when_starting_with_digit_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };

            let result = pascal_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123abc456def");

            let result = pascal_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123abc456def");

            let result = pascal_case_with_options("123Abc456Def", &opts);
            assert_eq!(result, "123Abc456Def");
        }

        #[test]
        fn convert_empty_string_with_options() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-_",
                keep: "",
            };

            let result = pascal_case_with_options("", &opts);
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

            let result = pascal_case_with_options("abc123def", &opts);
            assert_eq!(result, "Abc123def");
        }
    }

    mod non_alphabets_as_tail_a_word_with_separators {
        use super::*;

        #[test]
        fn convert_camel_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-_",
                keep: "",
            };
            let result = pascal_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "AbcDefGhIjk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-_",
                keep: "",
            };
            let result = pascal_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "AbcDefGhIjk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "_",
                keep: "",
            };
            let result = pascal_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.separators = "-";
            let result = pascal_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "Abc_Def_Ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };
            let result = pascal_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.separators = "_";
            let result = pascal_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "Abc-Def-Ghi");
        }

        #[test]
        fn convert_train_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };
            let result = pascal_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.separators = "_";
            let result = pascal_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "Abc-Def-Ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "_",
                keep: "",
            };
            let result = pascal_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.separators = "-";
            let result = pascal_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "Abc_Def_Ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };
            let result = pascal_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.separators = "_";
            let result = pascal_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "Abc-Def-Ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };
            let result = pascal_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "Abc123456DefG89HiJklMn12");

            opts.separators = "_";
            let result = pascal_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "Abc123-456DefG89HiJklMn12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: ":@$&()/",
                keep: "",
            };
            let result = pascal_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, ".Abc~!Def#Ghi%JkLmNo?");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };
            let result = pascal_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123Abc456Def");

            let result = pascal_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123Abc456Def");

            let result = pascal_case_with_options("123Abc456Def", &opts);
            assert_eq!(result, "123Abc456Def");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-_",
                keep: "",
            };
            let result = pascal_case_with_options("", &opts);
            assert_eq!(result, "");
        }

        #[test]
        fn alphabets_and_numbers_in_separators_have_no_effect() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-b2",
                keep: "",
            };
            let result = pascal_case_with_options("abc123def", &opts);
            assert_eq!(result, "Abc123Def");
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
            let result = pascal_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "AbcDefGhIjk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "-_",
                keep: "",
            };
            let result = pascal_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "AbcDefGhIjk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "_",
                keep: "",
            };
            let result = pascal_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.separators = "-";
            let result = pascal_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "Abc_Def_Ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };
            let result = pascal_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.separators = "_";
            let result = pascal_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "Abc-Def-Ghi");
        }

        #[test]
        fn convert_train_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };
            let result = pascal_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.separators = "_";
            let result = pascal_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "Abc-Def-Ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "_",
                keep: "",
            };
            let result = pascal_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.separators = "-";
            let result = pascal_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "Abc_Def_Ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };
            let result = pascal_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.separators = "_";
            let result = pascal_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "Abc-Def-Ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };
            let result = pascal_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "Abc123456DefG89HiJklMn12");

            opts.separators = "_";
            let result = pascal_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "Abc123-456DefG89HiJklMn12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: ":@$&()/",
                keep: "",
            };
            let result = pascal_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, ".Abc~!Def#Ghi%JkLmNo?");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "-_",
                keep: "",
            };
            let result = pascal_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123Abc456Def");

            let result = pascal_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123Abc456Def");

            let result = pascal_case_with_options("123Abc456Def", &opts);
            assert_eq!(result, "123Abc456Def");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "-_",
                keep: "",
            };
            let result = pascal_case_with_options("", &opts);
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
            let result = pascal_case_with_options("abc123def", &opts);
            assert_eq!(result, "Abc123Def");
        }
    }

    mod non_alphabets_as_part_a_word_with_separators {
        use super::*;

        #[test]
        fn convert_camel_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "-_",
                keep: "",
            };
            let result = pascal_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "AbcDefGhIjk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "-_",
                keep: "",
            };
            let result = pascal_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "AbcDefGhIjk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "_",
                keep: "",
            };
            let result = pascal_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.separators = "-";
            let result = pascal_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "Abc_def_ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };
            let result = pascal_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.separators = "_";
            let result = pascal_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "Abc-def-ghi");
        }

        #[test]
        fn convert_train_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };
            let result = pascal_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.separators = "_";
            let result = pascal_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "Abc-Def-Ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "_",
                keep: "",
            };
            let result = pascal_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.separators = "-";
            let result = pascal_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "Abc_def_ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };
            let result = pascal_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.separators = "_";
            let result = pascal_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "Abc-def-ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };
            let result = pascal_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "Abc123456defG89hiJklMn12");

            opts.separators = "_";
            let result = pascal_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "Abc123-456defG89hiJklMn12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: ":@$&()/",
                keep: "",
            };
            let result = pascal_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, ".abc~!Def#Ghi%JkLmNo?");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "-_",
                keep: "",
            };
            let result = pascal_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123abc456def");

            let result = pascal_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123abc456def");

            let result = pascal_case_with_options("123Abc456Def", &opts);
            assert_eq!(result, "123Abc456Def");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "-_",
                keep: "",
            };
            let result = pascal_case_with_options("", &opts);
            assert_eq!(result, "");
        }

        #[test]
        fn alphabets_and_numbers_in_separators_have_no_effect() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "-b2",
                keep: "",
            };
            let result = pascal_case_with_options("abc123def", &opts);
            assert_eq!(result, "Abc123def");
        }
    }

    mod non_alphabets_as_head_a_word_with_kept_characters {
        use super::*;

        #[test]
        fn convert_camel_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-_",
            };
            let result = pascal_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "AbcDefGhIjk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-_",
            };
            let result = pascal_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "AbcDefGhIjk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-",
            };
            let result = pascal_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.keep = "_";
            let result = pascal_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "Abc_def_ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "_",
            };
            let result = pascal_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.keep = "-";
            let result = pascal_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "Abc-def-ghi");
        }

        #[test]
        fn convert_train_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "_",
            };
            let result = pascal_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.keep = "-";
            let result = pascal_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "Abc-Def-Ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-",
            };
            let result = pascal_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.keep = "_";
            let result = pascal_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "Abc_def_ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "_",
            };
            let result = pascal_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.keep = "-";
            let result = pascal_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "Abc-def-ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "_",
            };
            let result = pascal_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "Abc123456defG89hiJklMn12");

            opts.keep = "-";
            let result = pascal_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "Abc123-456defG89hiJklMn12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: ".~!#%?",
            };
            let result = pascal_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, ".abc~!Def#Ghi%JkLmNo?");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-",
            };
            let result = pascal_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123abc456def");

            let result = pascal_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123abc456def");

            let result = pascal_case_with_options("123Abc456Def", &opts);
            assert_eq!(result, "123Abc456Def");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-_",
            };
            let result = pascal_case_with_options("", &opts);
            assert_eq!(result, "");
        }
    }

    mod non_alphabets_as_tail_a_word_with_kept_characters {
        use super::*;

        #[test]
        fn convert_camel_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-_",
            };
            let result = pascal_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "AbcDefGhIjk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-_",
            };
            let result = pascal_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "AbcDefGhIjk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-",
            };
            let result = pascal_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.keep = "_";
            let result = pascal_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "Abc_Def_Ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };
            let result = pascal_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.keep = "-";
            let result = pascal_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "Abc-Def-Ghi");
        }

        #[test]
        fn convert_train_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };
            let result = pascal_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.keep = "-";
            let result = pascal_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "Abc-Def-Ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-",
            };
            let result = pascal_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.keep = "_";
            let result = pascal_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "Abc_Def_Ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };
            let result = pascal_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.keep = "-";
            let result = pascal_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "Abc-Def-Ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };
            let result = pascal_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "Abc123456DefG89HiJklMn12");

            opts.keep = "-";
            let result = pascal_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "Abc123-456DefG89HiJklMn12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: ".~!#%?",
            };
            let result = pascal_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, ".Abc~!Def#Ghi%JkLmNo?");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-",
            };
            let result = pascal_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123Abc456Def");

            let result = pascal_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123Abc456Def");

            let result = pascal_case_with_options("123Abc456Def", &opts);
            assert_eq!(result, "123Abc456Def");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-_",
                keep: "",
            };
            let result = pascal_case_with_options("", &opts);
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
            let result = pascal_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "AbcDefGhIjk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-_",
            };
            let result = pascal_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "AbcDefGhIjk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-",
            };
            let result = pascal_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.keep = "_";
            let result = pascal_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "Abc_Def_Ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };
            let result = pascal_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.keep = "-";
            let result = pascal_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "Abc-Def-Ghi");
        }

        #[test]
        fn convert_train_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };
            let result = pascal_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.keep = "-";
            let result = pascal_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "Abc-Def-Ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-",
            };
            let result = pascal_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.keep = "_";
            let result = pascal_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "Abc_Def_Ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };
            let result = pascal_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.keep = "-";
            let result = pascal_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "Abc-Def-Ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };
            let result = pascal_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "Abc123456DefG89HiJklMn12");

            opts.keep = "-";
            let result = pascal_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "Abc123-456DefG89HiJklMn12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: ".~!#%?",
            };
            let result = pascal_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, ".Abc~!Def#Ghi%JkLmNo?");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-_",
            };
            let result = pascal_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123Abc456Def");

            let result = pascal_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123Abc456Def");

            let result = pascal_case_with_options("123Abc456Def", &opts);
            assert_eq!(result, "123Abc456Def");
        }

        #[test]
        fn convert_an_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-_",
            };
            let result = pascal_case_with_options("", &opts);
            assert_eq!(result, "");
        }
    }

    mod non_alphabets_as_part_a_word_with_kept_characters {
        use super::*;

        #[test]
        fn convert_camel_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-_",
            };
            let result = pascal_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "AbcDefGhIjk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-_",
            };
            let result = pascal_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "AbcDefGhIjk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-",
            };
            let result = pascal_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.keep = "_";
            let result = pascal_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "Abc_def_ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "_",
            };
            let result = pascal_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.keep = "-";
            let result = pascal_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "Abc-def-ghi");
        }

        #[test]
        fn convert_train_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "_",
            };
            let result = pascal_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.keep = "-";
            let result = pascal_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "Abc-Def-Ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-",
            };
            let result = pascal_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.keep = "_";
            let result = pascal_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "Abc_def_ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "_",
            };
            let result = pascal_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "AbcDefGhi");

            opts.keep = "-";
            let result = pascal_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "Abc-def-ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "_",
            };
            let result = pascal_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "Abc123456defG89hiJklMn12");

            opts.keep = "-";
            let result = pascal_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "Abc123-456defG89hiJklMn12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: ".~!#%?",
            };
            let result = pascal_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, ".abc~!Def#Ghi%JkLmNo?");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-_",
            };
            let result = pascal_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123abc456def");

            let result = pascal_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123abc456def");

            let result = pascal_case_with_options("123Abc456Def", &opts);
            assert_eq!(result, "123Abc456Def");
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-_",
            };
            let result = pascal_case_with_options("", &opts);
            assert_eq!(result, "");
        }
    }
}
