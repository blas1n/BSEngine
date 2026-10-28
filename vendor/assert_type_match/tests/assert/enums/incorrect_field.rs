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
    Struct { x: i32, z: i32 },
    //~^ ERROR: variant `other::Test::Struct` has no field named `z`
}

fn main() {}
