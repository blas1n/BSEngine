//@check-pass
use assert_type_match::assert_type_match;

mod other {
    pub struct Test<T> {
        pub x: T,
        pub y: T,
    }
}

#[assert_type_match(other::Test)]
struct Test<T> {
    x: T,
    y: T,
}

fn main() {}
