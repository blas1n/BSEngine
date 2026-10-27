use stringcase::{snake_case, snake_case_with_options, Options};

#[allow(deprecated)]
use stringcase::{snake_case_with_keep, snake_case_with_nums_as_word, snake_case_with_sep};

#[test]
fn it_should_convert_to_snake_case() {
    let converted = snake_case("fooBar100%BAZQux");
    assert_eq!(converted, "foo_bar100_baz_qux");
}

#[cfg(test)]
mod tests_of_snake_case_with_options {
    use super::*;

    #[test]
    fn separate_before_non_alphabets() {
        let opts = Options::new(true, false, "", "%");
        let converted = snake_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "foo_bar_100%baz_qux");
    }

    #[test]
    fn separate_after_non_alphabets() {
        let opts = Options::new(false, true, "", "%");
        let converted = snake_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "foo_bar100%_baz_qux");
    }

    #[test]
    fn separate_before_and_after_non_alphabets() {
        let opts = Options::new(true, true, "", "%");
        let converted = snake_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "foo_bar_100%_baz_qux");
    }

    #[test]
    fn not_separate_before_and_after_non_alphabets() {
        let opts = Options::new(false, false, "", "%");
        let converted = snake_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "foo_bar100%baz_qux");
    }

    #[test]
    fn specify_separators() {
        let opts = Options {
            separators: "-_",
            ..Default::default()
        };
        let converted = snake_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "foo_bar100%_baz_qux");
    }

    #[test]
    fn specify_kept_characters() {
        let opts = Options {
            keep: "_$",
            ..Default::default()
        };
        let converted = snake_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "foo__bar100_baz_qux");
    }
}

#[allow(deprecated)]
#[test]
fn it_should_convert_to_snake_case_with_sep() {
    let converted = snake_case_with_sep("foo_bar100%BAZQux", "_");
    assert_eq!(converted, "foo_bar100%_baz_qux");
}

#[allow(deprecated)]
#[test]
fn it_should_convert_to_snake_case_with_keep() {
    let converted = snake_case_with_keep("foo_bar100%BAZQux", "%");
    assert_eq!(converted, "foo_bar100%_baz_qux");
}

#[allow(deprecated)]
#[test]
fn it_should_convert_to_snake_case_with_nums_as_word() {
    let converted = snake_case_with_nums_as_word("foo-bar100%BazQux");
    assert_eq!(converted, "foo_bar_100_baz_qux");
}
