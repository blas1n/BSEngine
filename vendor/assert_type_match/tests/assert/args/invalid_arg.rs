use assert_type_match::assert_type_match;

mod other {
    pub struct Test {
        pub x: i32,
        pub y: i32,
    }
}

#[assert_type_match(other::Test, foo)]
//~^ ERROR: expected one of: `test_only`, `from`, `skip_name`, `skip_types`
struct Test {
    x: i32,
    y: i32,
}

fn main() {}
