use assert_type_match::assert_type_match;

mod other {
    pub enum Test {
        #[cfg(feature = "foo")]
        Unit,
        Tuple(i32, i32),
        Struct {
            x: i32,
            y: i32,
        },
    }
}

#[assert_type_match(other::Test)]
enum Test {
    Unit,
    //~^ ERROR: no variant named `Unit` found for enum `other::Test`
    Tuple(i32, i32),
    Struct { x: i32, y: i32 },
}

fn main() {}
