use assert_type_match::assert_type_match;

mod other {
    pub struct Test {
        pub x: i32,
        pub y: i32,
    }
}

#[assert_type_match(other::Test)]
//~^ ERROR: missing field `y` in initializer of `other::Test`
struct Test {
    x: i32,
    #[cfg(feature = "foo")]
    y: i32,
}

fn main() {}
