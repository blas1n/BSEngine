use assert_type_match::assert_type_match;

mod other {
    pub struct Test {
        pub x: i32,
        #[cfg(feature = "foo")]
        pub y: i32,
    }
}

#[assert_type_match(other::Test)]
struct Test {
    x: i32,
    y: i32,
    //~^ ERROR: struct `other::Test` has no field named `y`
}

fn main() {}
