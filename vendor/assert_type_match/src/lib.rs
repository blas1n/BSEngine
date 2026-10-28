mod args;
mod enums;
mod fields;
mod flags;
mod structs;
mod utils;
mod variants;
mod wrap;

use crate::args::Args;
use crate::enums::enum_assert;
use crate::structs::struct_assert;
use crate::wrap::wrap_assertions;
use proc_macro::TokenStream;
use syn::{parse_macro_input, Data, DeriveInput};

const ATTRIBUTE: &str = "assert_type_match";

/// An attribute macro that can be used to statically verify that the annotated struct or enum
/// matches the structure of a foreign type.
///
/// For structs, it will verify that the fields of the annotated struct match the fields of the
/// foreign struct.
///
/// For enums, it will verify that the variants of the annotated enum match the variants of the
/// foreign enum, and that the fields of each variant match the fields of the corresponding variant
/// in the foreign enum.
///
/// This will also output the original annotated struct or enum,
/// unless the `test_only` argument is set to `true`.
///
/// # Arguments
///
/// This macro accepts arguments to control its behavior.
/// These arguments are passed as a comma-separated list after the foreign type.
///
/// All boolean arguments may be set to true by using either the `foo` or `foo = true` syntax.
///
/// ## `test_only`
///
/// Type: `bool`
///
/// Controls whether to output the annotated struct or enum in the generated code.
///
/// ## `from`
///
/// Type: `bool`
///
/// Controls whether a `From` implementation should be generated.
///
/// If true, two `From` implementations will be generated:
/// one for converting from the annotated type to the foreign type,
/// and one for converting from the foreign type to the annotated type.
///
/// ## `skip_name`
///
/// Type: `bool`
///
/// Controls whether checking that the name of the annotated struct or enum matches
/// the name of the foreign type should be skipped.
///
/// For example, comparing `struct Foo(u32)` to `struct Bar(u32)` would pass
/// when this argument is set to `true`.
///
/// ## `skip_types`
///
/// Type: `bool`
///
/// Controls whether checking field types should be skipped.
///
/// For example, comparing `struct Foo(i32)` to `struct Foo(f32)` would pass
/// when this argument is set to `true`.
///
/// ## Field Arguments
///
/// This macro also supports field attributes.
///
/// These are also defined with the `#[assert_type_match(...)]` attribute.
///
/// ### `skip`
///
/// Type: `bool`
///
/// Controls whether the field should be skipped.
///
/// This allows you to skip fields that are not present on the foreign type.
///
/// ### `skip_type`
///
/// Type: `bool`
///
/// Controls whether checking the field type should be skipped.
///
/// ## Variant Arguments
///
/// This macro also supports variant attributes.
///
/// These are also defined with the `#[assert_type_match(...)]` attribute.
///
/// ### `skip`
///
/// Type: `bool`
///
/// Controls whether the variant should be skipped.
///
/// This allows you to skip variants that are not present on the foreign type.
///
/// # Example
///
/// A passing example:
///
/// ```
/// # use assert_type_match::assert_type_match;
/// mod other {
///     pub struct Test {
///         pub x: i32,
///         pub y: i32,
///     }
/// }
///
/// #[assert_type_match(other::Test)]
/// struct Test {
///     x: i32,
///     y: i32,
/// }
/// ```
///
/// A failing example:
///
/// ```compile_fail
/// # use assert_type_match::assert_type_match;
/// mod other {
///     pub struct Test {
///         pub x: i32,
///         pub y: i32,
///     }
/// }
///
/// #[assert_type_match(other::Test)]
/// struct Test {
///     x: i32,
///     z: i32, // Error: struct `other::Test` has no field named `z`
/// }
/// ```
///
#[proc_macro_attribute]
pub fn assert_type_match(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let args = parse_macro_input!(args as Args);

    wrap_assertions(input, args, |input, args| match &input.data {
        Data::Struct(data) => struct_assert(data, input, args),
        Data::Enum(data) => enum_assert(data, input, args),
        Data::Union(data) => Err(syn::Error::new(
            data.union_token.span,
            "unions are not supported",
        )),
    })
    .into()
}
