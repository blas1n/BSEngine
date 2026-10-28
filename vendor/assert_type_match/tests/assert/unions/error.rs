use assert_type_match::assert_type_match;

mod other {
    pub union Test {
        x: i32,
        y: i32,
    }
}

#[assert_type_match(other::Test)]
//~v ERROR: unions are not supported
union Test {
    x: i32,
    y: i32,
}

fn main() {}
