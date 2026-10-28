use assert_type_match::assert_type_match;

mod other {
    pub enum Test {
        Unit,
        Tuple(i32, i32),
        Struct { x: i32, y: i32 },
    }
}

#[assert_type_match(other::Test)]
//~^ ERROR: missing field `1` in initializer of `other::Test`
//~| ERROR: missing field `y` in initializer of `other::Test`
enum Test {
    Unit,
    Tuple(i32),
    Struct { x: i32 },
}

fn main() {}
