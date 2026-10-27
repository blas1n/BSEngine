use stringcase::{pascal_case, pascal_case_with_options, Options};

#[allow(deprecated)]
use stringcase::{pascal_case_with_keep, pascal_case_with_sep};

#[test]
fn it_should_convert_to_pascal_case() {
    let converted = pascal_case("foo_bar100%BAZQux");
    assert_eq!(converted, "FooBar100BazQux");
}

#[cfg(test)]
mod tests_of_pascal_case_with_options {
    use super::*;

    #[test]
    fn separate_before_non_alphabets() {
        let opts = Options::new(true, false, "", "%");
        let converted = pascal_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "FooBar100%bazQux");
    }

    #[test]
    fn separate_after_non_alphabets() {
        let opts = Options::new(false, true, "", "%");
        let converted = pascal_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "FooBar100%BazQux");
    }

    #[test]
    fn separate_before_and_after_non_alphabets() {
        let opts = Options::new(true, true, "", "%");
        let converted = pascal_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "FooBar100%BazQux");
    }

    #[test]
    fn not_separate_before_and_after_non_alphabets() {
        let opts = Options::new(false, false, "", "%");
        let converted = pascal_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "FooBar100%bazQux");
    }

    #[test]
    fn specify_separators() {
        let opts = Options {
            separators: "-_",
            ..Default::default()
        };
        let converted = pascal_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "FooBar100%BazQux");
    }

    #[test]
    fn specify_kept_characters() {
        let opts = Options {
            keep: "_$",
            ..Default::default()
        };
        let converted = pascal_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "Foo_Bar100BazQux");
    }
}

#[allow(deprecated)]
#[test]
fn it_should_convert_to_pascal_case_with_sep() {
    let converted = pascal_case_with_sep("foo_bar100%BAZQux", "_");
    assert_eq!(converted, "FooBar100%BazQux");
}

#[allow(deprecated)]
#[test]
fn it_should_convert_to_pascal_case_with_keep() {
    let converted = pascal_case_with_keep("foo_bar100%BAZQux", "%");
    assert_eq!(converted, "FooBar100%BazQux");
}
