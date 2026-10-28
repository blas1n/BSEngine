use assert_type_match::assert_type_match;

mod other {
    pub struct Test(pub i32, pub i32);
}

#[assert_type_match(other::Test)]
//~^ ERROR: struct `other::Test` has no field named `2`
struct Test(i32, i32, i32);

fn main() {}
