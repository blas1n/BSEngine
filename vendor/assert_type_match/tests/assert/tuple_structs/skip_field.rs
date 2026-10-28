//@check-pass
use assert_type_match::assert_type_match;

mod other {
    pub struct Test(pub i32, pub i32);
}

#[assert_type_match(other::Test)]
struct Test(i32, i32, #[assert_type_match(skip)] i32);

fn main() {}
