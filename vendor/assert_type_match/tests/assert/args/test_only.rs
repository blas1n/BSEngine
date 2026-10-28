use assert_type_match::assert_type_match;

pub struct Test {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[assert_type_match(Test, test_only)]
//~^ ERROR: missing field `z` in initializer of `Test`
struct Test {
    x: i32,
    y: i32,
}

fn main() {}
