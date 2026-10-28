use assert_type_match::assert_type_match;

mod other {
    pub struct Test {
        pub x: i32,
        pub y: i32,
    }
}

#[assert_type_match(other::Test)]
struct Test {
    x: i32,
    z: i32,
    //~^ ERROR: struct `other::Test` has no field named `z`
}

fn main() {}
