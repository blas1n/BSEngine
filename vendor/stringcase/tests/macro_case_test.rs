use stringcase::{macro_case, macro_case_with_options, Options};

#[allow(deprecated)]
use stringcase::{macro_case_with_keep, macro_case_with_nums_as_word, macro_case_with_sep};

#[test]
fn it_should_convert_to_macro_case() {
    let converted = macro_case("foo-bar100%BAZQux");
    assert_eq!(converted, "FOO_BAR100_BAZ_QUX");
}

#[cfg(test)]
mod tests_of_macro_case_with_options {
    use super::*;

    #[test]
    fn separate_before_non_alphabets() {
        let opts = Options::new(true, false, "", "%");
        let converted = macro_case_with_options("foo-bar100%BAZQux", &opts);
        assert_eq!(converted, "FOO_BAR_100%BAZ_QUX");
    }

    #[test]
    fn separate_after_non_alphabets() {
        let opts = Options::new(false, true, "", "%");
        let converted = macro_case_with_options("foo-bar100%BAZQux", &opts);
        assert_eq!(converted, "FOO_BAR100%_BAZ_QUX");
    }

    #[test]
    fn separate_before_and_after_non_alphabets() {
        let opts = Options::new(true, true, "", "%");
        let converted = macro_case_with_options("foo-bar100%BAZQux", &opts);
        assert_eq!(converted, "FOO_BAR_100%_BAZ_QUX");
    }

    #[test]
    fn not_separate_before_and_after_non_alphabets() {
        let opts = Options::new(false, false, "", "%");
        let converted = macro_case_with_options("foo-bar100%BAZQux", &opts);
        assert_eq!(converted, "FOO_BAR100%BAZ_QUX");
    }

    #[test]
    fn specify_separators() {
        let opts = Options {
            separators: "-_",
            ..Default::default()
        };
        let converted = macro_case_with_options("foo-bar100%BAZQux", &opts);
        assert_eq!(converted, "FOO_BAR100%_BAZ_QUX");
    }

    #[test]
    fn specify_kept_characters() {
        let opts = Options {
            keep: "-$",
            ..Default::default()
        };
        let converted = macro_case_with_options("foo-bar100%BAZQux", &opts);
        assert_eq!(converted, "FOO-_BAR100_BAZ_QUX");
    }
}

#[allow(deprecated)]
#[test]
fn it_should_convert_to_macro_case_with_sep() {
    let converted = macro_case_with_sep("foo_bar100%BAZQux", "_");
    assert_eq!(converted, "FOO_BAR100%_BAZ_QUX");
}

#[allow(deprecated)]
#[test]
fn it_should_convert_to_macro_case_with_keep() {
    let converted = macro_case_with_keep("foo_bar100%BAZQux", "%");
    assert_eq!(converted, "FOO_BAR100%_BAZ_QUX");
}

#[allow(deprecated)]
#[test]
fn it_should_convert_to_macro_case_with_nums_as_word() {
    let converted = macro_case_with_nums_as_word("foo-bar100%-baz-qux");
    assert_eq!(converted, "FOO_BAR_100_BAZ_QUX");
}
