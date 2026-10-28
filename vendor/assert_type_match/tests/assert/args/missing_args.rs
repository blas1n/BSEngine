use assert_type_match::assert_type_match;

#[assert_type_match]
//~^ ERROR: expected the type path to a foreign type (e.g. `#[assert_type_match(path::to::ForeignType)]`)
struct Test {
    x: i32,
    y: i32,
}

fn main() {}
