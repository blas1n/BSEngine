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
    #[assert_type_match]
    //~^ ERROR: expected attribute arguments in parentheses: #[assert_type_match(...)]
    Struct {
        x: i32,
        y: i32,
    },
}

fn main() {}
