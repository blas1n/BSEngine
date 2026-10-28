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
    Tuple(i32, i32),
    #[assert_type_match(foo)]
    //~^ ERROR: expected `skip`
    Struct {
        x: i32,
        y: i32,
    },
}

fn main() {}
