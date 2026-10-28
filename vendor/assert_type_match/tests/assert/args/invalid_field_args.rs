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
    y: i32,
    #[assert_type_match(foo)]
    //~^ ERROR: expected `skip` or `skip_type`
    z: i32,
}

fn main() {}
