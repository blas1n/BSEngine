use assert_type_match::assert_type_match;

mod other {
    pub enum Test {
        Unit,
        Tuple(i32, i32),
        Struct { x: i32, y: i32 },
    }
}

#[assert_type_match(other::Test)]
//~^ ERROR: non-exhaustive patterns: `other::Test::Unit` not covered
enum Test {
    Tuple(i32, i32),
    Struct { x: i32, y: i32 },
}

fn main() {}
