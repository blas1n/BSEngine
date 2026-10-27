// Copyright (C) 2024-2025 Takayuki Sato. All Rights Reserved.
// This program is free software under MIT License.
// See the file LICENSE in this distribution for more details.

use crate::options::Options;

/// Converts the input string to camel case with the specified options.
///
/// ```rust
///     let opts = stringcase::Options{
///       separate_before_non_alphabets: true,
///       separate_after_non_alphabets: true,
///       separators: "",
///       keep: "",
///     };
///     let camel = stringcase::camel_case_with_options("foo_bar_100_baz", &opts);
///     assert_eq!(camel, "fooBar100Baz");
/// ```
pub fn camel_case_with_options(input: &str, opts: &Options) -> String {
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

/// Converts the input string to camel case.
///
/// It treats the end of a sequence of non-alphabetical characters as a word boundary, but not
/// the beginning.
///
/// ```rust
///     let camel = stringcase::camel_case("foo_bar100_baz");
///     assert_eq!(camel, "fooBar100Baz");
/// ```
pub fn camel_case(input: &str) -> String {
    let opts = Options {
        separate_before_non_alphabets: false,
        separate_after_non_alphabets: true,
        separators: "",
        keep: "",
    };
    camel_case_with_options(input, &opts)
}

/// Converts the input string to camel case with the specified separator characters.
#[deprecated(since = "0.4.0", note = "Should use camel_case_with_options instead")]
pub fn camel_case_with_sep(input: &str, seps: &str) -> String {
    let opts = Options {
        separate_before_non_alphabets: false,
        separate_after_non_alphabets: true,
        separators: seps,
        keep: "",
    };
    camel_case_with_options(input, &opts)
}

/// Converts the input string to camel case with the specified characters to be kept.
#[deprecated(since = "0.4.0", note = "Should use camel_case_with_options instead")]
pub fn camel_case_with_keep(input: &str, kept: &str) -> String {
    let opts = Options {
        separate_before_non_alphabets: false,
        separate_after_non_alphabets: true,
        separators: "",
        keep: kept,
    };
    camel_case_with_options(input, &opts)
}

#[cfg(test)]
mod tests_of_camel_case {
    use super::*;

    #[test]
    fn convert_camel_case() {
        assert_eq!(camel_case("abcDefGHIjk"), "abcDefGhIjk");
    }

    #[test]
    fn convert_pascal_case() {
        assert_eq!(camel_case("AbcDefGHIjk"), "abcDefGhIjk");
    }

    #[test]
    fn convert_snake_case() {
        assert_eq!(camel_case("abc_def_ghi"), "abcDefGhi");
    }

    #[test]
    fn convert_kebab_case() {
        assert_eq!(camel_case("abc-def-ghi"), "abcDefGhi");
    }

    #[test]
    fn convert_train_case() {
        assert_eq!(camel_case("Abc-Def-Ghi"), "abcDefGhi");
    }

    #[test]
    fn convert_macro_case() {
        assert_eq!(camel_case("ABC_DEF_GHI"), "abcDefGhi");
    }

    #[test]
    fn convert_cobol_case() {
        assert_eq!(camel_case("ABC-DEF-GHI"), "abcDefGhi");
    }

    #[test]
    fn convert_with_digits() {
        assert_eq!(
            camel_case("abc123-456defG89HIJklMN12"),
            "abc123456DefG89HiJklMn12"
        );
    }

    #[test]
    fn convert_with_symbols_as_separators() {
        assert_eq!(
            camel_case(":.abc~!@def#$ghi%&jk(lm)no/?"),
            "abcDefGhiJkLmNo"
        );
    }

    #[test]
    fn convert_starting_with_digit() {
        assert_eq!(camel_case("123abc456def"), "123Abc456Def");
        assert_eq!(camel_case("123ABC456DEF"), "123Abc456Def");
        assert_eq!(camel_case("123Abc456Def"), "123Abc456Def");
    }

    #[test]
    fn convert_empty_string() {
        assert_eq!(camel_case(""), "");
    }
}

#[cfg(test)]
mod tests_of_camel_case_with_options {
    use super::*;

    mod non_alphabets_as_head_of_a_word {
        use super::*;

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("AbcDefGHIjk", &opts), "abcDefGhIjk");
        }

        #[test]
        fn convert_snake_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("abc_def_ghi", &opts), "abcDefGhi");
        }

        #[test]
        fn convert_kebab_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("abc-def-ghi", &opts), "abcDefGhi");
        }

        #[test]
        fn convert_train_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("Abc-Def-Ghi", &opts), "abcDefGhi");
        }

        #[test]
        fn convert_macro_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("ABC_DEF_GHI", &opts), "abcDefGhi");
        }

        #[test]
        fn convert_cobol_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("ABC-DEF-GHI", &opts), "abcDefGhi");
        }

        #[test]
        fn convert_with_digits() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            assert_eq!(
                camel_case_with_options("abc123-456defG89HIJklMN12", &opts),
                "abc123456defG89hiJklMn12"
            );
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            assert_eq!(
                camel_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts),
                "abcDefGhiJkLmNo"
            );
        }

        #[test]
        fn convert_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            assert_eq!(
                camel_case_with_options("123abc456def", &opts),
                "123abc456def"
            );
            assert_eq!(
                camel_case_with_options("123ABC456DEF", &opts),
                "123abc456def"
            );
            assert_eq!(
                camel_case_with_options("123Abc456Def", &opts),
                "123Abc456Def"
            );
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("", &opts), "");
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

            assert_eq!(camel_case_with_options("abcDefGHIjk", &opts), "abcDefGhIjk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("AbcDefGHIjk", &opts), "abcDefGhIjk");
        }

        #[test]
        fn convert_snake_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("abc_def_ghi", &opts), "abcDefGhi");
        }

        #[test]
        fn convert_kebab_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("abc-def-ghi", &opts), "abcDefGhi");
        }

        #[test]
        fn convert_train_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("Abc-Def-Ghi", &opts), "abcDefGhi");
        }

        #[test]
        fn convert_macro_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("ABC_DEF_GHI", &opts), "abcDefGhi");
        }

        #[test]
        fn convert_cobol_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("ABC-DEF-GHI", &opts), "abcDefGhi");
        }

        #[test]
        fn convert_with_digits() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            assert_eq!(
                camel_case_with_options("abc123-456defG89HIJklMN12", &opts),
                "abc123456DefG89HiJklMn12"
            );
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            assert_eq!(
                camel_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts),
                "abcDefGhiJkLmNo"
            );
        }

        #[test]
        fn convert_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            assert_eq!(
                camel_case_with_options("123abc456def", &opts),
                "123Abc456Def"
            );
            assert_eq!(
                camel_case_with_options("123ABC456DEF", &opts),
                "123Abc456Def"
            );
            assert_eq!(
                camel_case_with_options("123Abc456Def", &opts),
                "123Abc456Def"
            );
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("", &opts), "");
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

            assert_eq!(camel_case_with_options("abcDefGHIjk", &opts), "abcDefGhIjk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("AbcDefGHIjk", &opts), "abcDefGhIjk");
        }

        #[test]
        fn convert_snake_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("abc_def_ghi", &opts), "abcDefGhi");
        }

        #[test]
        fn convert_kebab_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("abc-def-ghi", &opts), "abcDefGhi");
        }

        #[test]
        fn convert_train_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("Abc-Def-Ghi", &opts), "abcDefGhi");
        }

        #[test]
        fn convert_macro_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("ABC_DEF_GHI", &opts), "abcDefGhi");
        }

        #[test]
        fn convert_cobol_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("ABC-DEF-GHI", &opts), "abcDefGhi");
        }

        #[test]
        fn convert_with_digits() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            assert_eq!(
                camel_case_with_options("abc123-456defG89HIJklMN12", &opts),
                "abc123456DefG89HiJklMn12"
            );
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            assert_eq!(
                camel_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts),
                "abcDefGhiJkLmNo"
            );
        }

        #[test]
        fn convert_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            assert_eq!(
                camel_case_with_options("123abc456def", &opts),
                "123Abc456Def"
            );
            assert_eq!(
                camel_case_with_options("123ABC456DEF", &opts),
                "123Abc456Def"
            );
            assert_eq!(
                camel_case_with_options("123Abc456Def", &opts),
                "123Abc456Def"
            );
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("", &opts), "");
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

            assert_eq!(camel_case_with_options("abcDefGHIjk", &opts), "abcDefGhIjk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("AbcDefGHIjk", &opts), "abcDefGhIjk");
        }

        #[test]
        fn convert_snake_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("abc_def_ghi", &opts), "abcDefGhi");
        }

        #[test]
        fn convert_kebab_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("abc-def-ghi", &opts), "abcDefGhi");
        }

        #[test]
        fn convert_train_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("Abc-Def-Ghi", &opts), "abcDefGhi");
        }

        #[test]
        fn convert_macro_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("ABC_DEF_GHI", &opts), "abcDefGhi");
        }

        #[test]
        fn convert_cobol_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("ABC-DEF-GHI", &opts), "abcDefGhi");
        }

        #[test]
        fn convert_with_digits() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            assert_eq!(
                camel_case_with_options("abc123-456defG89HIJklMN12", &opts),
                "abc123456defG89hiJklMn12"
            );
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            assert_eq!(
                camel_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts),
                "abcDefGhiJkLmNo"
            );
        }

        #[test]
        fn convert_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            assert_eq!(
                camel_case_with_options("123abc456def", &opts),
                "123abc456def"
            );
            assert_eq!(
                camel_case_with_options("123ABC456DEF", &opts),
                "123abc456def"
            );
            assert_eq!(
                camel_case_with_options("123Abc456Def", &opts),
                "123Abc456Def"
            );
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            assert_eq!(camel_case_with_options("", &opts), "");
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
            assert_eq!(camel_case_with_options("abcDefGHIjk", &opts), "abcDefGhIjk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-_",
                keep: "",
            };
            assert_eq!(camel_case_with_options("AbcDefGHIjk", &opts), "abcDefGhIjk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "_",
                keep: "",
            };

            assert_eq!(camel_case_with_options("abc_def_ghi", &opts), "abcDefGhi");

            opts.separators = "-";
            assert_eq!(camel_case_with_options("abc_def_ghi", &opts), "abc_def_ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };

            assert_eq!(camel_case_with_options("abc-def-ghi", &opts), "abcDefGhi");

            opts.separators = "_";
            assert_eq!(camel_case_with_options("abc-def-ghi", &opts), "abc-def-ghi");
        }

        #[test]
        fn convert_train_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };

            assert_eq!(camel_case_with_options("Abc-Def-Ghi", &opts), "abcDefGhi");

            opts.separators = "_";
            assert_eq!(camel_case_with_options("Abc-Def-Ghi", &opts), "abc-Def-Ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "_",
                keep: "",
            };

            assert_eq!(camel_case_with_options("ABC_DEF_GHI", &opts), "abcDefGhi");

            opts.separators = "-";
            assert_eq!(camel_case_with_options("ABC_DEF_GHI", &opts), "abc_def_ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };

            assert_eq!(camel_case_with_options("ABC-DEF-GHI", &opts), "abcDefGhi");

            opts.separators = "_";
            assert_eq!(camel_case_with_options("ABC-DEF-GHI", &opts), "abc-def-ghi");
        }

        #[test]
        fn convert_with_digits() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };

            assert_eq!(
                camel_case_with_options("abc123-456defG89HIJklMN12", &opts),
                "abc123456defG89hiJklMn12"
            );

            opts.separators = "_";
            assert_eq!(
                camel_case_with_options("abc123-456defG89HIJklMN12", &opts),
                "abc123-456defG89hiJklMn12"
            );
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: ":@$&()/",
                keep: "",
            };

            assert_eq!(
                camel_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts),
                ".abc~!Def#Ghi%JkLmNo?"
            );
        }

        #[test]
        fn convert_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-",
                keep: "",
            };

            assert_eq!(
                camel_case_with_options("123abc456def", &opts),
                "123abc456def"
            );
            assert_eq!(
                camel_case_with_options("123ABC456DEF", &opts),
                "123abc456def"
            );
            assert_eq!(
                camel_case_with_options("123Abc456Def", &opts),
                "123Abc456Def"
            );
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-_",
                keep: "",
            };

            assert_eq!(camel_case_with_options("", &opts), "");
        }

        #[test]
        fn alphabets_and_numbers_in_separators_have_no_effect() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "-b2",
                keep: "",
            };

            assert_eq!(camel_case_with_options("abc123def", &opts), "abc123def");
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
            assert_eq!(camel_case_with_options("abcDefGHIjk", &opts), "abcDefGhIjk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-_",
                keep: "",
            };
            assert_eq!(camel_case_with_options("AbcDefGHIjk", &opts), "abcDefGhIjk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "_",
                keep: "",
            };

            assert_eq!(camel_case_with_options("abc_def_ghi", &opts), "abcDefGhi");

            opts.separators = "-";
            assert_eq!(camel_case_with_options("abc_def_ghi", &opts), "abc_Def_Ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };

            assert_eq!(camel_case_with_options("abc-def-ghi", &opts), "abcDefGhi");

            opts.separators = "_";
            assert_eq!(camel_case_with_options("abc-def-ghi", &opts), "abc-Def-Ghi");
        }

        #[test]
        fn convert_train_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };

            assert_eq!(camel_case_with_options("Abc-Def-Ghi", &opts), "abcDefGhi");

            opts.separators = "_";
            assert_eq!(camel_case_with_options("Abc-Def-Ghi", &opts), "abc-Def-Ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "_",
                keep: "",
            };

            assert_eq!(camel_case_with_options("ABC_DEF_GHI", &opts), "abcDefGhi");

            opts.separators = "-";
            assert_eq!(camel_case_with_options("ABC_DEF_GHI", &opts), "abc_Def_Ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };

            assert_eq!(camel_case_with_options("ABC-DEF-GHI", &opts), "abcDefGhi");

            opts.separators = "_";
            assert_eq!(camel_case_with_options("ABC-DEF-GHI", &opts), "abc-Def-Ghi");
        }

        #[test]
        fn convert_with_digits() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };

            assert_eq!(
                camel_case_with_options("abc123-456defG89HIJklMN12", &opts),
                "abc123456DefG89HiJklMn12"
            );

            opts.separators = "_";
            assert_eq!(
                camel_case_with_options("abc123-456defG89HIJklMN12", &opts),
                "abc123-456DefG89HiJklMn12"
            );
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: ":@$&()/",
                keep: "",
            };

            assert_eq!(
                camel_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts),
                ".Abc~!Def#Ghi%JkLmNo?"
            );
        }

        #[test]
        fn convert_starting_with_digit() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-",
                keep: "",
            };

            assert_eq!(
                camel_case_with_options("123abc456def", &opts),
                "123Abc456Def"
            );
            assert_eq!(
                camel_case_with_options("123ABC456DEF", &opts),
                "123Abc456Def"
            );
            assert_eq!(
                camel_case_with_options("123Abc456Def", &opts),
                "123Abc456Def"
            );
        }

        #[test]
        fn convert_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-_",
                keep: "",
            };

            assert_eq!(camel_case_with_options("", &opts), "");
        }

        #[test]
        fn alphabets_and_numbers_in_separators_have_no_effect() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "-b2",
                keep: "",
            };

            assert_eq!(camel_case_with_options("abc123def", &opts), "abc123Def");
        }
    }

    mod non_alphabets_as_a_word_with_separators {
        use super::*;

        #[test]
        fn convert_camel_case() {
            let orig_opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let mut opts = orig_opts;
            opts.separators = "-_";
            let result = camel_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abcDefGhIjk");
        }

        #[test]
        fn convert_pascal_case() {
            let orig_opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let mut opts = orig_opts;
            opts.separators = "-_";
            let result = camel_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abcDefGhIjk");
        }

        #[test]
        fn convert_snake_case() {
            let orig_opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let mut opts = orig_opts;
            opts.separators = "_";
            let result = camel_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.separators = "-";
            let result = camel_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc_Def_Ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let orig_opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let mut opts = orig_opts;
            opts.separators = "-";
            let result = camel_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.separators = "_";
            let result = camel_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc-Def-Ghi");
        }

        #[test]
        fn convert_train_case() {
            let orig_opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let mut opts = orig_opts;
            opts.separators = "-";
            let result = camel_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.separators = "_";
            let result = camel_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc-Def-Ghi");
        }

        #[test]
        fn convert_macro_case() {
            let orig_opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let mut opts = orig_opts;

            opts.separators = "_";
            let result = camel_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.separators = "-";
            let result = camel_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc_Def_Ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let orig_opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let mut opts = orig_opts;
            opts.separators = "-";
            let result = camel_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.separators = "_";
            let result = camel_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc-Def-Ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let orig_opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let mut opts = orig_opts;
            opts.separators = "-";
            let result = camel_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123456DefG89HiJklMn12");

            opts.separators = "_";
            let result = camel_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123-456DefG89HiJklMn12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let orig_opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let mut opts = orig_opts;
            opts.separators = ":@$&()/";
            let result = camel_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, ".Abc~!Def#Ghi%JkLmNo?");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let orig_opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let mut opts = orig_opts;
            opts.separators = "-_";
            let result = camel_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123Abc456Def");

            let result = camel_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123Abc456Def");

            let result = camel_case_with_options("123Abc456Def", &opts);
            assert_eq!(result, "123Abc456Def");
        }

        #[test]
        fn convert_an_empty_string() {
            let orig_opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let mut opts = orig_opts;
            opts.separators = "-_";
            let result = camel_case_with_options("", &opts);
            assert_eq!(result, "");
        }

        #[test]
        fn alphabets_and_numbers_in_separators_are_no_effect() {
            let orig_opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "",
            };

            let mut opts = orig_opts;
            opts.separators = "-b2";
            let result = camel_case_with_options("abc123def", &opts);
            assert_eq!(result, "abc123Def");
        }
    }

    mod non_alphabets_as_part_of_a_word_with_separators {
        use super::*;

        #[test]
        fn convert_camel_case() {
            let orig_opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let mut opts = orig_opts;
            opts.separators = "-_";
            let result = camel_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abcDefGhIjk");
        }

        #[test]
        fn convert_pascal_case() {
            let orig_opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let mut opts = orig_opts;
            opts.separators = "-_";
            let result = camel_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abcDefGhIjk");
        }

        #[test]
        fn convert_snake_case() {
            let orig_opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let mut opts = orig_opts;
            opts.separators = "_";
            let result = camel_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.separators = "-";
            let result = camel_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let orig_opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let mut opts = orig_opts;
            opts.separators = "-";
            let result = camel_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.separators = "_";
            let result = camel_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_train_case() {
            let orig_opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let mut opts = orig_opts;
            opts.separators = "-";
            let result = camel_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.separators = "_";
            let result = camel_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc-Def-Ghi");
        }

        #[test]
        fn convert_macro_case() {
            let orig_opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let mut opts = orig_opts;
            opts.separators = "_";
            let result = camel_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.separators = "-";
            let result = camel_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let orig_opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let mut opts = orig_opts;
            opts.separators = "-";
            let result = camel_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.separators = "_";
            let result = camel_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let orig_opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let mut opts = orig_opts;
            opts.separators = "-";
            let result = camel_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123456defG89hiJklMn12");

            opts.separators = "_";
            let result = camel_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123-456defG89hiJklMn12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let orig_opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let mut opts = orig_opts;
            opts.separators = ":@$&()/";
            let result = camel_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
            assert_eq!(result, ".abc~!Def#Ghi%JkLmNo?");
        }

        #[test]
        fn convert_when_starting_with_digit() {
            let orig_opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let mut opts = orig_opts;
            opts.separators = "-_";
            let result = camel_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123abc456def");

            let result = camel_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123abc456def");

            let result = camel_case_with_options("123Abc456Def", &opts);
            assert_eq!(result, "123Abc456Def");
        }

        #[test]
        fn convert_an_empty_string() {
            let orig_opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let mut opts = orig_opts;
            opts.separators = "-_";
            let result = camel_case_with_options("", &opts);
            assert_eq!(result, "");
        }

        #[test]
        fn alphabets_and_numbers_in_separators_are_no_effect() {
            let orig_opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "",
            };

            let mut opts = orig_opts;
            opts.separators = "-b2";
            let result = camel_case_with_options("abc123def", &opts);
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
            let result = camel_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abcDefGhIjk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-_",
            };
            let result = camel_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abcDefGhIjk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-",
            };
            let result = camel_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.keep = "_";
            let result = camel_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "_",
            };
            let result = camel_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.keep = "-";
            let result = camel_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_train_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "_",
            };
            let result = camel_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.keep = "-";
            let result = camel_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc-Def-Ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-",
            };
            let result = camel_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.keep = "_";
            let result = camel_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc_def_ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "_",
            };
            let result = camel_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.keep = "-";
            let result = camel_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc-def-ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "_",
            };
            let result = camel_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123456defG89hiJklMn12");

            opts.keep = "-";
            let result = camel_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123-456defG89hiJklMn12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: ".~!#%?",
            };
            let result = camel_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
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
            let result = camel_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123abc456def");

            let result = camel_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123abc456def");

            let result = camel_case_with_options("123Abc456Def", &opts);
            assert_eq!(result, "123Abc456Def");
        }

        #[test]
        fn convert_an_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-_",
            };
            let result = camel_case_with_options("", &opts);
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
            let result = camel_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abcDefGhIjk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-_",
            };
            let result = camel_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abcDefGhIjk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-",
            };
            let result = camel_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.keep = "_";
            let result = camel_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc_Def_Ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };
            let result = camel_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.keep = "-";
            let result = camel_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc-Def-Ghi");
        }

        #[test]
        fn convert_train_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };
            let result = camel_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.keep = "-";
            let result = camel_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc-Def-Ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-",
            };
            let result = camel_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.keep = "_";
            let result = camel_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc_Def_Ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };
            let result = camel_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.keep = "-";
            let result = camel_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc-Def-Ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };
            let result = camel_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123456DefG89HiJklMn12");

            opts.keep = "-";
            let result = camel_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123-456DefG89HiJklMn12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: ".~!#%?",
            };
            let result = camel_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
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
            let result = camel_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123Abc456Def");

            let result = camel_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123Abc456Def");

            let result = camel_case_with_options("123Abc456Def", &opts);
            assert_eq!(result, "123Abc456Def");
        }

        #[test]
        fn convert_an_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-_",
            };
            let result = camel_case_with_options("", &opts);
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
            let result = camel_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abcDefGhIjk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-_",
            };
            let result = camel_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abcDefGhIjk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-",
            };
            let result = camel_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.keep = "_";
            let result = camel_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abc_Def_Ghi");
        }

        #[test]
        fn convert_kebab_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };
            let result = camel_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.keep = "-";
            let result = camel_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abc-Def-Ghi");
        }

        #[test]
        fn convert_train_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };
            let result = camel_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.keep = "-";
            let result = camel_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc-Def-Ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "-",
            };
            let result = camel_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.keep = "_";
            let result = camel_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abc_Def_Ghi");
        }

        #[test]
        fn convert_cobol_case() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };
            let result = camel_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.keep = "-";
            let result = camel_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abc-Def-Ghi");
        }

        #[test]
        fn convert_with_keeping_digits() {
            let mut opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: "_",
            };
            let result = camel_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123456DefG89HiJklMn12");

            opts.keep = "-";
            let result = camel_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123-456DefG89HiJklMn12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: true,
                separate_after_non_alphabets: true,
                separators: "",
                keep: ".~!#%?",
            };
            let result = camel_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
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
            let result = camel_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123Abc456Def");

            let result = camel_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123Abc456Def");

            let result = camel_case_with_options("123Abc456Def", &opts);
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
            let result = camel_case_with_options("", &opts);
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
            let result = camel_case_with_options("abcDefGHIjk", &opts);
            assert_eq!(result, "abcDefGhIjk");
        }

        #[test]
        fn convert_pascal_case() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-_",
            };
            let result = camel_case_with_options("AbcDefGHIjk", &opts);
            assert_eq!(result, "abcDefGhIjk");
        }

        #[test]
        fn convert_snake_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-",
            };
            let result = camel_case_with_options("abc_def_ghi", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.keep = "_";
            let result = camel_case_with_options("abc_def_ghi", &opts);
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
            let result = camel_case_with_options("abc-def-ghi", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.keep = "-";
            let result = camel_case_with_options("abc-def-ghi", &opts);
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
            let result = camel_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.keep = "-";
            let result = camel_case_with_options("Abc-Def-Ghi", &opts);
            assert_eq!(result, "abc-Def-Ghi");
        }

        #[test]
        fn convert_macro_case() {
            let mut opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-",
            };
            let result = camel_case_with_options("ABC_DEF_GHI", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.keep = "_";
            let result = camel_case_with_options("ABC_DEF_GHI", &opts);
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
            let result = camel_case_with_options("ABC-DEF-GHI", &opts);
            assert_eq!(result, "abcDefGhi");

            opts.keep = "-";
            let result = camel_case_with_options("ABC-DEF-GHI", &opts);
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
            let result = camel_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123456defG89hiJklMn12");

            opts.keep = "-";
            let result = camel_case_with_options("abc123-456defG89HIJklMN12", &opts);
            assert_eq!(result, "abc123-456defG89hiJklMn12");
        }

        #[test]
        fn convert_with_symbols_as_separators() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: ".~!#%?",
            };
            let result = camel_case_with_options(":.abc~!@def#$ghi%&jk(lm)no/?", &opts);
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
            let result = camel_case_with_options("123abc456def", &opts);
            assert_eq!(result, "123abc456def");

            let result = camel_case_with_options("123ABC456DEF", &opts);
            assert_eq!(result, "123abc456def");

            let result = camel_case_with_options("123Abc456Def", &opts);
            assert_eq!(result, "123Abc456Def");
        }

        #[test]
        fn convert_an_empty_string() {
            let opts = Options {
                separate_before_non_alphabets: false,
                separate_after_non_alphabets: false,
                separators: "",
                keep: "-_",
            };
            let result = camel_case_with_options("", &opts);
            assert_eq!(result, "");
        }
    }
}
