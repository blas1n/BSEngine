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
    Unit,
    Tuple(i32, i32),
    Struct { x: i32, y: i32 },
    Extra,
    //~^ ERROR: no variant named `Extra` found for enum `other::Test`
}

fn main() {}
