//@check-pass
use assert_type_match::assert_type_match;

mod other {
    pub struct Test;
}

#[assert_type_match(other::Test, from)]
struct Test;

fn convert_this(test: Test) -> other::Test {
    test.into()
}

fn convert_that(test: other::Test) -> Test {
    test.into()
}

fn main() {}
