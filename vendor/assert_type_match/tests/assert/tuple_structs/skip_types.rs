//@check-pass
use assert_type_match::assert_type_match;

mod other {
    pub struct Test(pub i32, pub i32);
}

#[assert_type_match(other::Test, skip_types)]
struct Test(f32, f32);

fn main() {}
