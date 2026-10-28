//@check-pass
use assert_type_match::assert_type_match;

mod other {
    pub struct Test<T>(pub T, pub T);
}

#[assert_type_match(other::Test)]
struct Test<T>(T, T);

fn main() {}
