//@check-pass
use assert_type_match::assert_type_match;

mod other {
    pub struct Test;
}

#[assert_type_match(other::Test)]
struct Test;

fn main() {}
