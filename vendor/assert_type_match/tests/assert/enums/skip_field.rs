//@check-pass
use assert_type_match::assert_type_match;

mod other {
    pub enum Test {
        Unit,
        Tuple(i32, i32),
        Struct { x: i32, y: i32 },
    }
}

#[assert_type_match(other::Test)]
pub enum Test {
    Unit,
    Tuple(i32, i32, #[assert_type_match(skip)] i32),
    Struct {
        x: i32,
        y: i32,
        #[assert_type_match(skip)]
        z: i32,
    },
}

fn main() {}
