//@check-pass
use assert_type_match::assert_type_match;

mod other {
    pub enum Test {
        Unit,
        Tuple(i32, i32),
        Struct { x: i32, y: i32 },
    }
}

#[assert_type_match(other::Test, skip_types)]
enum Test {
    Unit,
    Tuple(f32, f32),
    Struct { x: f32, y: f32 },
}

fn main() {}
