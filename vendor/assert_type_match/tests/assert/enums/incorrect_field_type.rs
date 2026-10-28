use assert_type_match::assert_type_match;

mod other {
    pub enum Test {
        Unit,
        Tuple(i32, i32),
        Struct { x: i32, y: i32 },
    }
}

#[assert_type_match(other::Test)]
//~^ ERROR: mismatched types
enum Test {
    Unit,
    Tuple(i32, f32),
    Struct { x: i32, y: f32 },
    //~^ ERROR: mismatched types
}

fn main() {}
