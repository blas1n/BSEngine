use stringcase::{kebab_case, kebab_case_with_options, Options};

#[allow(deprecated)]
use stringcase::{kebab_case_with_keep, kebab_case_with_nums_as_word, kebab_case_with_sep};

#[test]
fn it_should_convert_to_kebab_case() {
    let converted = kebab_case("fooBar100%BazQux");
    assert_eq!(converted, "foo-bar100-baz-qux");
}

#[cfg(test)]
mod tests_of_kebab_case_with_options {
    use super::*;

    #[test]
    fn separate_before_non_alphabets() {
        let opts = Options::new(true, false, "", "%");
        let converted = kebab_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "foo-bar-100%baz-qux");
    }

    #[test]
    fn separate_after_non_alphabets() {
        let opts = Options::new(false, true, "", "%");
        let converted = kebab_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "foo-bar100%-baz-qux");
    }

    #[test]
    fn separate_before_and_after_non_alphabets() {
        let opts = Options::new(true, true, "", "%");
        let converted = kebab_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "foo-bar-100%-baz-qux");
    }

    #[test]
    fn not_separate_before_and_after_non_alphabets() {
        let opts = Options::new(false, false, "", "%");
        let converted = kebab_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "foo-bar100%baz-qux");
    }

    #[test]
    fn specify_separators() {
        let opts = Options {
            separators: "-_",
            ..Default::default()
        };
        let converted = kebab_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "foo-bar100%-baz-qux");
    }

    #[test]
    fn specify_kept_characters() {
        let opts = Options {
            keep: "_$",
            ..Default::default()
        };
        let converted = kebab_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "foo_-bar100-baz-qux");
    }
}

#[allow(deprecated)]
#[test]
fn it_should_convert_to_kebab_case_with_sep() {
    let converted = kebab_case_with_sep("foo_bar100%BAZQux", "-_");
    assert_eq!(converted, "foo-bar100%-baz-qux");
}

#[allow(deprecated)]
#[test]
fn it_should_convert_to_kebab_case_with_keep() {
    let converted = kebab_case_with_keep("foo_bar100%BAZQux", "%");
    assert_eq!(converted, "foo-bar100%-baz-qux");
}

#[allow(deprecated)]
#[test]
fn it_should_convert_to_kebab_case_with_nums_as_word() {
    let converted = kebab_case_with_nums_as_word("foo_bar100%BAZQux");
    assert_eq!(converted, "foo-bar-100-baz-qux");
}
