use assert_type_match::assert_type_match;

mod other {
    pub struct Test(pub i32, pub i32);
}

#[assert_type_match(other::Test)]
//~^ ERROR: missing field `1` in initializer of `other::Test`
struct Test(i32);

fn main() {}
