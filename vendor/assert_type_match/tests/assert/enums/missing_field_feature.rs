use assert_type_match::assert_type_match;

mod other {
    pub enum Test {
        Unit,
        Tuple(i32, #[cfg(feature = "foo")] i32),
        Struct {
            x: i32,
            #[cfg(feature = "foo")]
            y: i32,
        },
    }
}

#[assert_type_match(other::Test)]
//~^ ERROR: variant `other::Test::Tuple` has no field named `1`
enum Test {
    Unit,
    Tuple(i32, i32),
    Struct { x: i32, y: i32 },
    //~^ ERROR: variant `other::Test::Struct` has no field named `y`
}

fn main() {}
