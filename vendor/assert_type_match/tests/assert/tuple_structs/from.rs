//@check-pass
use assert_type_match::assert_type_match;

mod other {
    pub struct Test(pub i32, pub i32, #[cfg(feature = "foo")] pub i32);
}

#[assert_type_match(other::Test, from)]
struct Test(i32, i32, #[cfg(feature = "foo")] i32);

fn convert_this(test: Test) -> other::Test {
    test.into()
}

fn convert_that(test: other::Test) -> Test {
    test.into()
}

fn main() {}
