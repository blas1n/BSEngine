use stringcase::{cobol_case, cobol_case_with_options, Options};

#[allow(deprecated)]
use stringcase::{cobol_case_with_keep, cobol_case_with_nums_as_word, cobol_case_with_sep};

#[test]
fn it_should_convert_to_cobol_case() {
    let converted = cobol_case("foo_bar100%BAZQux");
    assert_eq!(converted, "FOO-BAR100-BAZ-QUX");
}

#[cfg(test)]
mod tests_of_cobol_case_with_options {
    use super::*;

    #[test]
    fn separate_before_non_alphabets() {
        let opts = Options::new(true, false, "", "%");
        let converted = cobol_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "FOO-BAR-100%BAZ-QUX");
    }

    #[test]
    fn separate_after_non_alphabets() {
        let opts = Options::new(false, true, "", "%");
        let converted = cobol_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "FOO-BAR100%-BAZ-QUX");
    }

    #[test]
    fn separate_before_and_after_non_alphabets() {
        let opts = Options::new(true, true, "", "%");
        let converted = cobol_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "FOO-BAR-100%-BAZ-QUX");
    }

    #[test]
    fn not_separate_before_and_after_non_alphabets() {
        let opts = Options::new(false, false, "", "%");
        let converted = cobol_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "FOO-BAR100%BAZ-QUX");
    }

    #[test]
    fn specify_separators() {
        let opts = Options {
            separators: "-_",
            ..Default::default()
        };
        let converted = cobol_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "FOO-BAR100%-BAZ-QUX");
    }

    #[test]
    fn specify_kept_characters() {
        let opts = Options {
            keep: "_$",
            ..Default::default()
        };
        let converted = cobol_case_with_options("foo_bar100%BAZQux", &opts);
        assert_eq!(converted, "FOO_-BAR100-BAZ-QUX");
    }
}

#[allow(deprecated)]
#[test]
fn it_should_convert_to_cobol_case_with_sep() {
    let converted = cobol_case_with_sep("foo_bar100%BAZQux", "_");
    assert_eq!(converted, "FOO-BAR100%-BAZ-QUX");
}

#[allow(deprecated)]
#[test]
fn it_should_convert_to_cobol_case_with_keep() {
    let converted = cobol_case_with_keep("foo_bar100%BAZQux", "%");
    assert_eq!(converted, "FOO-BAR100%-BAZ-QUX");
}

#[allow(deprecated)]
#[test]
fn it_should_convert_to_cobol_case_with_nums_as_word() {
    let converted = cobol_case_with_nums_as_word("foo_bar100%BAZQux");
    assert_eq!(converted, "FOO-BAR-100-BAZ-QUX");
}
