//@check-pass
use assert_type_match::assert_type_match;

mod other {
    pub struct Test {
        pub x: i32,
        pub y: i32,
        #[cfg(feature = "foo")]
        pub z: i32,
    }
}

#[assert_type_match(other::Test, from)]
struct Test {
    x: i32,
    y: i32,
    #[cfg(feature = "foo")]
    pub z: i32,
}

fn convert_this(test: Test) -> other::Test {
    test.into()
}

fn convert_that(test: other::Test) -> Test {
    test.into()
}

fn main() {}
