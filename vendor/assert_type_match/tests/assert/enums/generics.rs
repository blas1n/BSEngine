//@check-pass
use assert_type_match::assert_type_match;

mod other {
    pub enum Test<T> {
        Unit,
        Tuple(T, T),
        Struct { x: T, y: T },
    }
}

#[assert_type_match(other::Test)]
enum Test<T> {
    Unit,
    Tuple(T, T),
    Struct { x: T, y: T },
}

fn main() {}
