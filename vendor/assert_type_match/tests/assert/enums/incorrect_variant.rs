use assert_type_match::assert_type_match;

mod other {
    pub enum Test {
        Unit,
        Tuple(i32, i32),
        Struct { x: i32, y: i32 },
    }
}

#[assert_type_match(other::Test)]
enum Test {
    Empty,
    //~^ ERROR: no variant named `Empty` found for enum `other::Test`
    Tuple(i32, i32),
    Struct { x: i32, y: i32 },
}

fn main() {}
