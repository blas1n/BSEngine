use stringcase::{camel_case, camel_case_with_options, Options};

#[allow(deprecated)]
use stringcase::{camel_case_with_keep, camel_case_with_sep};

#[test]
fn it_should_convert_to_camel_case() {
    let converted = camel_case("foo_bar100%BAZQux");
    assert_eq!(converted, "fooBar100BazQux");
}

#[cfg(test)]
mod tests_of_camel_case_with_options {
    use super::*;

    #[test]
    fn separate_before_non_alphabets() {
        let opts = Options::new(true, false, "", "%");
        let converted = camel_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "fooBar100%bazQux");
    }

    #[test]
    fn separate_after_non_alphabets() {
        let opts = Options::new(false, true, "", "%");
        let converted = camel_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "fooBar100%BazQux");
    }

    #[test]
    fn separate_before_and_after_non_alphabets() {
        let opts = Options::new(true, true, "", "%");
        let converted = camel_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "fooBar100%BazQux");
    }

    #[test]
    fn not_separate_before_and_after_non_alphabets() {
        let opts = Options::new(false, false, "", "%");
        let converted = camel_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "fooBar100%bazQux");
    }

    #[test]
    fn specify_separators() {
        let opts = Options {
            separators: "-_",
            ..Default::default()
        };
        let converted = camel_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "fooBar100%BazQux");
    }

    #[test]
    fn specify_kept_characters() {
        let opts = Options {
            keep: "_$",
            ..Default::default()
        };
        let converted = camel_case_with_options("foo_bar100BAZQux", &opts);
        assert_eq!(converted, "foo_Bar100BazQux");
    }
}

#[allow(deprecated)]
#[test]
fn it_should_convert_to_camel_case_with_sep() {
    let converted = camel_case_with_sep("foo_bar100%BAZQux", "_");
    assert_eq!(converted, "fooBar100%BazQux");
}

#[allow(deprecated)]
#[test]
fn it_should_convert_to_camel_case_with_keep() {
    let converted = camel_case_with_keep("foo_bar100%BAZQux", "%");
    assert_eq!(converted, "fooBar100%BazQux");
}
