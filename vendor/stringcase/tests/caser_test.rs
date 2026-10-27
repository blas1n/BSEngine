use stringcase::{Caser, Options};

#[cfg(test)]
mod test_of_to_camel_case {
    use super::*;

    #[test]
    fn it_should_convert_to_camel_case_by_method_of_string() {
        let converted = "foo_bar100%BAZQux".to_camel_case();
        assert_eq!(converted, "fooBar100BazQux");
    }

    mod tests_of_camel_case_with_options_by_method_of_string {
        use super::*;

        #[test]
        fn separate_before_non_alphabets() {
            let opts = Options::new(true, false, "", "%");
            let converted = "foo_bar100%BAZQux".to_camel_case_with_options(&opts);
            assert_eq!(converted, "fooBar100%bazQux");
        }

        #[test]
        fn separate_after_non_alphabets() {
            let opts = Options::new(false, true, "", "%");
            let converted = "foo_bar100%BAZQux".to_camel_case_with_options(&opts);
            assert_eq!(converted, "fooBar100%BazQux");
        }

        #[test]
        fn separate_before_and_after_non_alphabets() {
            let opts = Options::new(true, true, "", "%");
            let converted = "foo_bar100%BAZQux".to_camel_case_with_options(&opts);
            assert_eq!(converted, "fooBar100%BazQux");
        }

        #[test]
        fn not_separate_before_and_after_non_alphabets() {
            let opts = Options::new(false, false, "", "%");
            let converted = "foo_bar100%BAZQux".to_camel_case_with_options(&opts);
            assert_eq!(converted, "fooBar100%bazQux");
        }

        #[test]
        fn specify_separators() {
            let opts = Options {
                separators: "-_",
                ..Default::default()
            };
            let converted = "foo_bar100%BAZQux".to_camel_case_with_options(&opts);
            assert_eq!(converted, "fooBar100%BazQux");
        }

        #[test]
        fn specify_kept_characters() {
            let opts = Options {
                keep: "_$",
                ..Default::default()
            };
            let converted = "foo_bar100%BAZQux".to_camel_case_with_options(&opts);
            assert_eq!(converted, "foo_Bar100BazQux");
        }
    }

    #[allow(deprecated)]
    #[test]
    fn it_should_convert_to_camel_case_with_sep_by_method_of_string() {
        let converted = "foo_bar100%BAZQux".to_camel_case_with_sep("_-");
        assert_eq!(converted, "fooBar100%BazQux");
    }

    #[allow(deprecated)]
    #[test]
    fn it_should_convert_to_camel_case_with_keep_by_method_of_string() {
        let converted = "foo_bar100%BAZQux".to_camel_case_with_keep("_$");
        assert_eq!(converted, "foo_Bar100BazQux");
    }
}

#[cfg(test)]
mod test_of_to_cobol_case {
    use super::*;

    #[test]
    fn it_should_convert_to_cobol_case_by_method_of_string() {
        let converted = "foo_bar100%BAZQux".to_cobol_case();
        assert_eq!(converted, "FOO-BAR100-BAZ-QUX");
    }

    mod tests_of_cobol_case_with_options_by_method_of_string {
        use super::*;

        #[test]
        fn separate_before_non_alphabets() {
            let opts = Options::new(true, false, "", "%");
            let converted = "foo_bar100%BAZQux".to_cobol_case_with_options(&opts);
            assert_eq!(converted, "FOO-BAR-100%BAZ-QUX");
        }

        #[test]
        fn separate_after_non_alphabets() {
            let opts = Options::new(false, true, "", "%");
            let converted = "foo_bar100%BAZQux".to_cobol_case_with_options(&opts);
            assert_eq!(converted, "FOO-BAR100%-BAZ-QUX");
        }

        #[test]
        fn separate_before_and_after_non_alphabets() {
            let opts = Options::new(true, true, "", "%");
            let converted = "foo_bar100%BAZQux".to_cobol_case_with_options(&opts);
            assert_eq!(converted, "FOO-BAR-100%-BAZ-QUX");
        }

        #[test]
        fn not_separate_before_and_after_non_alphabets() {
            let opts = Options::new(false, false, "", "%");
            let converted = "foo_bar100%BAZQux".to_cobol_case_with_options(&opts);
            assert_eq!(converted, "FOO-BAR100%BAZ-QUX");
        }

        #[test]
        fn specify_separators() {
            let opts = Options {
                separators: "-_",
                ..Default::default()
            };
            let converted = "foo_bar100%BAZQux".to_cobol_case_with_options(&opts);
            assert_eq!(converted, "FOO-BAR100%-BAZ-QUX");
        }

        #[test]
        fn specify_kept_characters() {
            let opts = Options {
                keep: "_$",
                ..Default::default()
            };
            let converted = "foo_bar100%BAZQux".to_cobol_case_with_options(&opts);
            assert_eq!(converted, "FOO_-BAR100-BAZ-QUX");
        }
    }

    #[allow(deprecated)]
    #[test]
    fn it_should_convert_to_cobol_case_with_nums_as_word_by_method_of_string() {
        let converted = "foo_bar100%BAZQux".to_cobol_case_with_nums_as_word();
        assert_eq!(converted, "FOO-BAR-100-BAZ-QUX");
    }

    #[allow(deprecated)]
    #[test]
    fn it_should_convert_to_cobol_case_with_sep_by_method_of_string() {
        let converted = "foo_bar100BAZQux".to_cobol_case_with_sep("-_");
        assert_eq!(converted, "FOO-BAR100-BAZ-QUX");
    }

    #[allow(deprecated)]
    #[test]
    fn it_should_convert_to_cobol_case_with_keep_by_method_of_string() {
        let converted = "foo_bar100%BAZQux".to_cobol_case_with_keep("_$");
        assert_eq!(converted, "FOO_-BAR100-BAZ-QUX");
    }
}

#[cfg(test)]
mod test_of_to_kebab_case {
    use super::*;

    #[test]
    fn it_should_convert_to_kebab_case_by_method_of_string() {
        let converted = "foo_bar100%BAZQux".to_kebab_case();
        assert_eq!(converted, "foo-bar100-baz-qux");
    }

    mod tests_of_kebab_case_with_options_by_method_of_string {
        use super::*;

        #[test]
        fn separate_before_non_alphabets() {
            let opts = Options::new(true, false, "", "%");
            let converted = "foo_bar100%BAZQux".to_kebab_case_with_options(&opts);
            assert_eq!(converted, "foo-bar-100%baz-qux");
        }

        #[test]
        fn separate_after_non_alphabets() {
            let opts = Options::new(false, true, "", "%");
            let converted = "foo_bar100%BAZQux".to_kebab_case_with_options(&opts);
            assert_eq!(converted, "foo-bar100%-baz-qux");
        }

        #[test]
        fn separate_before_and_after_non_alphabets() {
            let opts = Options::new(true, true, "", "%");
            let converted = "foo_bar100%BAZQux".to_kebab_case_with_options(&opts);
            assert_eq!(converted, "foo-bar-100%-baz-qux");
        }

        #[test]
        fn not_separate_before_and_after_non_alphabets() {
            let opts = Options::new(false, false, "", "%");
            let converted = "foo_bar100%BAZQux".to_kebab_case_with_options(&opts);
            assert_eq!(converted, "foo-bar100%baz-qux");
        }

        #[test]
        fn specify_separators() {
            let opts = Options {
                separators: "-_",
                ..Default::default()
            };
            let converted = "foo_bar100%BAZQux".to_kebab_case_with_options(&opts);
            assert_eq!(converted, "foo-bar100%-baz-qux");
        }

        #[test]
        fn specify_kept_characters() {
            let opts = Options {
                keep: "_$",
                ..Default::default()
            };
            let converted = "foo_bar100%BAZQux".to_kebab_case_with_options(&opts);
            assert_eq!(converted, "foo_-bar100-baz-qux");
        }
    }

    #[allow(deprecated)]
    #[test]
    fn it_should_convert_to_kebab_case_with_nums_as_word_by_method_of_string() {
        let converted = "foo_bar100%BAZQux".to_kebab_case_with_sep("-_");
        assert_eq!(converted, "foo-bar100%-baz-qux");
    }

    #[allow(deprecated)]
    #[test]
    fn it_should_convert_to_kebab_case_with_sep_by_method_of_string() {
        let converted = "foo_bar100BAZQux".to_kebab_case_with_keep("_$");
        assert_eq!(converted, "foo_-bar100-baz-qux");
    }

    #[allow(deprecated)]
    #[test]
    fn it_should_convert_to_kebab_case_with_keep_by_method_of_string() {
        let converted = "foo_bar100BAZQux".to_kebab_case_with_nums_as_word();
        assert_eq!(converted, "foo-bar-100-baz-qux");
    }
}

#[cfg(test)]
mod test_of_to_macro_case {
    use super::*;

    #[test]
    fn it_should_convert_to_macro_case_by_method_of_string() {
        let converted = "foo_bar100%BAZQux".to_macro_case();
        assert_eq!(converted, "FOO_BAR100_BAZ_QUX");
    }

    mod tests_of_macro_case_with_options_by_method_of_string {
        use super::*;

        #[test]
        fn separate_before_non_alphabets() {
            let opts = Options::new(true, false, "", "%");
            let converted = "foo_bar100%BAZQux".to_macro_case_with_options(&opts);
            assert_eq!(converted, "FOO_BAR_100%BAZ_QUX");
        }

        #[test]
        fn separate_after_non_alphabets() {
            let opts = Options::new(false, true, "", "%");
            let converted = "foo_bar100%BAZQux".to_macro_case_with_options(&opts);
            assert_eq!(converted, "FOO_BAR100%_BAZ_QUX");
        }

        #[test]
        fn separate_before_and_after_non_alphabets() {
            let opts = Options::new(true, true, "", "%");
            let converted = "foo_bar100%BAZQux".to_macro_case_with_options(&opts);
            assert_eq!(converted, "FOO_BAR_100%_BAZ_QUX");
        }

        #[test]
        fn not_separate_before_and_after_non_alphabets() {
            let opts = Options::new(false, false, "", "%");
            let converted = "foo_bar100%BAZQux".to_macro_case_with_options(&opts);
            assert_eq!(converted, "FOO_BAR100%BAZ_QUX");
        }

        #[test]
        fn specify_separators() {
            let opts = Options {
                separators: "-_",
                ..Default::default()
            };
            let converted = "foo_bar100%BAZQux".to_macro_case_with_options(&opts);
            assert_eq!(converted, "FOO_BAR100%_BAZ_QUX");
        }

        #[test]
        fn specify_kept_characters() {
            let opts = Options {
                keep: "_$",
                ..Default::default()
            };
            let converted = "foo_bar100%BAZQux".to_macro_case_with_options(&opts);
            assert_eq!(converted, "FOO__BAR100_BAZ_QUX");
        }
    }

    #[allow(deprecated)]
    #[test]
    fn it_should_convert_to_macro_case_with_nums_as_word_by_method_of_string() {
        let converted = "foo_bar100%BAZQux".to_macro_case_with_nums_as_word();
        assert_eq!(converted, "FOO_BAR_100_BAZ_QUX");
    }

    #[allow(deprecated)]
    #[test]
    fn it_should_convert_to_macro_case_with_sep_by_method_of_string() {
        let converted = "foo_bar100%BAZQux".to_macro_case_with_sep("_-");
        assert_eq!(converted, "FOO_BAR100%_BAZ_QUX");
    }

    #[allow(deprecated)]
    #[test]
    fn it_should_convert_to_macro_case_with_keep_by_method_of_string() {
        let converted = "foo_bar100BAZQux".to_macro_case_with_keep("_$");
        assert_eq!(converted, "FOO__BAR100_BAZ_QUX");
    }
}

#[cfg(test)]
mod test_of_to_pascal_case {
    use super::*;

    #[test]
    fn it_should_convert_to_pascal_case_by_method_of_string() {
        let converted = "foo_bar100%BAZQux".to_pascal_case();
        assert_eq!(converted, "FooBar100BazQux");
    }

    mod tests_of_pascal_case_with_options_by_method_of_string {
        use super::*;

        #[test]
        fn separate_before_non_alphabets() {
            let opts = Options::new(true, false, "", "%");
            let converted = "foo_bar100%BAZQux".to_pascal_case_with_options(&opts);
            assert_eq!(converted, "FooBar100%bazQux");
        }

        #[test]
        fn separate_after_non_alphabets() {
            let opts = Options::new(false, true, "", "%");
            let converted = "foo_bar100%BAZQux".to_pascal_case_with_options(&opts);
            assert_eq!(converted, "FooBar100%BazQux");
        }

        #[test]
        fn separate_before_and_after_non_alphabets() {
            let opts = Options::new(true, true, "", "%");
            let converted = "foo_bar100%BAZQux".to_pascal_case_with_options(&opts);
            assert_eq!(converted, "FooBar100%BazQux");
        }

        #[test]
        fn not_separate_before_and_after_non_alphabets() {
            let opts = Options::new(false, false, "", "%");
            let converted = "foo_bar100%BAZQux".to_pascal_case_with_options(&opts);
            assert_eq!(converted, "FooBar100%bazQux");
        }

        #[test]
        fn specify_separators() {
            let opts = Options {
                separators: "-_",
                ..Default::default()
            };
            let converted = "foo_bar100%BAZQux".to_pascal_case_with_options(&opts);
            assert_eq!(converted, "FooBar100%BazQux");
        }

        #[test]
        fn specify_kept_characters() {
            let opts = Options {
                keep: "_$",
                ..Default::default()
            };
            let converted = "foo_bar100%BAZQux".to_pascal_case_with_options(&opts);
            assert_eq!(converted, "Foo_Bar100BazQux");
        }
    }

    #[allow(deprecated)]
    #[test]
    fn it_should_convert_to_pascal_case_with_sep_by_method_of_string() {
        let converted = "foo_bar100%BAZQux".to_pascal_case_with_sep("_");
        assert_eq!(converted, "FooBar100%BazQux");
    }

    #[allow(deprecated)]
    #[test]
    fn it_should_convert_to_pascal_case_with_keep_by_method_of_string() {
        let converted = "foo_bar100BAZQux".to_pascal_case_with_keep("_$");
        assert_eq!(converted, "Foo_Bar100BazQux");
    }
}

#[cfg(test)]
mod test_of_to_snake_case {
    use super::*;

    #[test]
    fn it_should_convert_to_snake_case_by_method_of_string() {
        let converted = "foo_bar100%BAZQux".to_snake_case();
        assert_eq!(converted, "foo_bar100_baz_qux");
    }

    #[allow(deprecated)]
    #[test]
    fn it_should_convert_to_snake_case_with_nums_as_word_by_method_of_string() {
        let converted = "foo_bar100%BAZQux".to_snake_case_with_nums_as_word();
        assert_eq!(converted, "foo_bar_100_baz_qux");
    }

    #[allow(deprecated)]
    #[test]
    fn it_should_convert_to_snake_case_with_sep_by_method_of_string() {
        let converted = "foo_bar100%BAZQux".to_snake_case_with_sep("_-");
        assert_eq!(converted, "foo_bar100%_baz_qux");
    }

    #[allow(deprecated)]
    #[test]
    fn it_should_convert_to_snake_case_with_keep_by_method_of_string() {
        let converted = "foo_bar100%BAZQux".to_snake_case_with_keep("_$");
        assert_eq!(converted, "foo__bar100_baz_qux");
    }
}

#[cfg(test)]
mod test_of_to_train_case {
    use super::*;

    #[test]
    fn it_should_convert_to_train_case_by_method_of_string() {
        let converted = "foo_bar100%BAZQux".to_train_case();
        assert_eq!(converted, "Foo-Bar100-Baz-Qux");
    }

    mod tests_of_train_case_with_options_by_method_of_string {
        use super::*;

        #[test]
        fn separate_before_non_alphabets() {
            let opts = Options::new(true, false, "", "%");
            let converted = "foo_bar100%BAZQux".to_train_case_with_options(&opts);
            assert_eq!(converted, "Foo-Bar-100%baz-Qux");
        }

        #[test]
        fn separate_after_non_alphabets() {
            let opts = Options::new(false, true, "", "%");
            let converted = "foo_bar100%BAZQux".to_train_case_with_options(&opts);
            assert_eq!(converted, "Foo-Bar100%-Baz-Qux");
        }

        #[test]
        fn separate_before_and_after_non_alphabets() {
            let opts = Options::new(true, true, "", "%");
            let converted = "foo_bar100%BAZQux".to_train_case_with_options(&opts);
            assert_eq!(converted, "Foo-Bar-100%-Baz-Qux");
        }

        #[test]
        fn not_separate_before_and_after_non_alphabets() {
            let opts = Options::new(false, false, "", "%");
            let converted = "foo_bar100%BAZQux".to_train_case_with_options(&opts);
            assert_eq!(converted, "Foo-Bar100%baz-Qux");
        }

        #[test]
        fn specify_separators() {
            let opts = Options {
                separators: "-_",
                ..Default::default()
            };
            let converted = "foo_bar100%BAZQux".to_train_case_with_options(&opts);
            assert_eq!(converted, "Foo-Bar100%-Baz-Qux");
        }

        #[test]
        fn specify_kept_characters() {
            let opts = Options {
                keep: "_$",
                ..Default::default()
            };
            let converted = "foo_bar100%BAZQux".to_train_case_with_options(&opts);
            assert_eq!(converted, "Foo_-Bar100-Baz-Qux");
        }
    }

    #[allow(deprecated)]
    #[test]
    fn it_should_convert_to_train_case_with_nums_as_word_by_method_of_string() {
        let converted = "foo_bar100%BAZQux".to_train_case_with_nums_as_word();
        assert_eq!(converted, "Foo-Bar-100-Baz-Qux");
    }

    #[allow(deprecated)]
    #[test]
    fn it_should_convert_to_train_case_with_sep_by_method_of_string() {
        let converted = "foo_bar100%BAZQux".to_train_case_with_sep("-_");
        assert_eq!(converted, "Foo-Bar100%-Baz-Qux");
    }

    #[allow(deprecated)]
    #[test]
    fn it_should_convert_to_train_case_with_keep_by_method_of_string() {
        let converted = "foo_bar100%BAZQux".to_train_case_with_keep("_$");
        assert_eq!(converted, "Foo_-Bar100-Baz-Qux");
    }
}
