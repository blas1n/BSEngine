/// Lazily shortens a type name to remove all module paths.
///
/// The short name of a type is its full name as returned by
/// [`core::any::type_name`], but with the prefix of all paths removed. For
/// example, the short name of `alloc::vec::Vec<core::option::Option<u32>>`
/// would be `Vec<Option<u32>>`.
///
/// Shortening is performed lazily without allocation.
#[cfg_attr(
    feature = "alloc",
    doc = r#" To get a [`String`] from this type, use the [`to_string`](`alloc::string::ToString::to_string`) method."#
)]
///
/// # Examples
///
/// ```rust
/// # use disqualified::ShortName;
/// #
/// # mod foo {
/// #     pub mod bar {
/// #         pub struct Baz;
/// #     }
/// # }
/// let short_name = ShortName::of::<foo::bar::Baz>(); // Baz
/// ```
#[derive(Clone, Copy)]
pub struct ShortName<'a>(pub &'a str);

impl ShortName<'static> {
    /// Gets a shortened version of the name of the type `T`.
    pub fn of<T: ?Sized>() -> Self {
        Self(core::any::type_name::<T>())
    }
}

impl<'a> ShortName<'a> {
    /// Gets the original name before shortening.
    pub const fn original(&self) -> &'a str {
        self.0
    }
}

impl<'a> From<&'a str> for ShortName<'a> {
    fn from(value: &'a str) -> Self {
        Self(value)
    }
}

impl<'a> core::fmt::Debug for ShortName<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let &ShortName(full_name) = self;
        // Generics result in nested paths within <..> blocks.
        // Consider "bevy_render::camera::camera::extract_cameras<bevy_render::camera::bundle::Camera3d>".
        // To tackle this, we parse the string from left to right, collapsing as we go.
        let mut index: usize = 0;
        let end_of_string = full_name.len();

        while index < end_of_string {
            let rest_of_string = full_name.get(index..end_of_string).unwrap_or_default();

            // Collapse everything up to the next special character,
            // then skip over it
            if let Some(special_character_index) = rest_of_string.find(|c: char| {
                (c == ' ')
                    || (c == '<')
                    || (c == '>')
                    || (c == '(')
                    || (c == ')')
                    || (c == '[')
                    || (c == ']')
                    || (c == ',')
                    || (c == ';')
                    || (c == '&')
            }) {
                let segment_to_collapse = rest_of_string
                    .get(0..special_character_index)
                    .unwrap_or_default();

                f.write_str(collapse_type_name(segment_to_collapse))?;

                // Insert the special character
                let special_character =
                    &rest_of_string[special_character_index..=special_character_index];

                if special_character == "<" {
                    let mut end_lifetime_index = special_character_index + 1;
                    // Elide lifetime parameters, which always come first in generics.
                    while rest_of_string[end_lifetime_index..].starts_with("'_, ") {
                        end_lifetime_index += 4;
                    }
                    if rest_of_string[end_lifetime_index..].starts_with("'_>") {
                        // If all parameters are lifetime parameters,
                        // also elide the angle brackets.
                        end_lifetime_index += 3;
                    } else {
                        f.write_str("<")?;
                    }
                    index += end_lifetime_index;
                    continue;
                }

                f.write_str(special_character)?;

                match special_character {
                    ">" | ")" | "]"
                        if rest_of_string[special_character_index + 1..].starts_with("::") =>
                    {
                        f.write_str("::")?;
                        // Move the index past the "::"
                        index += special_character_index + 3;
                    }
                    // Move the index just past the special character
                    _ => index += special_character_index + 1,
                }
            } else {
                // If there are no special characters left, we're done!
                f.write_str(collapse_type_name(rest_of_string))?;
                index = end_of_string;
            }
        }

        Ok(())
    }
}

impl<'a> core::fmt::Display for ShortName<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        <Self as core::fmt::Debug>::fmt(self, f)
    }
}

#[inline(always)]
fn collapse_type_name(string: &str) -> &str {
    // Closures end with some number of `::{{closure}}`,
    // `::{{call_once}}`, or `::{{call_mut}` segments.
    // Take only the leftmost, if one exists.
    let mut closure_start = string.len();
    let mut closure_end = closure_start;
    if let Some(end_index) = string.find("}}") {
        if let Some(start_index) = string[..end_index].find("::{{") {
            closure_end = end_index + 2;
            closure_start = start_index;
        }
    }

    // Trim modules, but not type names.
    // If the next to last segment is uppercase, assume it is a type.
    // This covers both enum variants and methods.
    let mut segments = string[..closure_start].rsplit("::");
    let last = segments.next().unwrap();
    let second_last = segments.next();
    let type_len = second_last
        .filter(|second_last| second_last.starts_with(char::is_uppercase))
        .map_or(0, |second_last| second_last.len() + 2);
    let index = closure_start - last.len() - type_len;
    &string[index..closure_end]
}

#[cfg(all(test, feature = "alloc"))]
mod name_formatting_tests {
    use super::ShortName;

    #[test]
    fn trivial() {
        assert_eq!(ShortName("test_system").to_string(), "test_system");
    }

    #[test]
    fn empty() {
        assert_eq!(ShortName("").to_string(), "");
        assert_eq!(ShortName("::").to_string(), "");
    }

    #[test]
    fn path_separated() {
        assert_eq!(
            ShortName("bevy_prelude::make_fun_game").to_string(),
            "make_fun_game"
        );
    }

    #[test]
    fn tuple_type() {
        assert_eq!(
            ShortName("(String, String)").to_string(),
            "(String, String)"
        );
    }

    #[test]
    fn array_type() {
        assert_eq!(ShortName("[i32; 3]").to_string(), "[i32; 3]");
    }

    #[test]
    fn trivial_generics() {
        assert_eq!(ShortName("a<B>").to_string(), "a<B>");
    }

    #[test]
    fn multiple_type_parameters() {
        assert_eq!(ShortName("a<B, C>").to_string(), "a<B, C>");
    }

    #[test]
    fn enums() {
        assert_eq!(ShortName("Option::None").to_string(), "Option::None");
        assert_eq!(ShortName("Option::Some(2)").to_string(), "Option::Some(2)");
        assert_eq!(
            ShortName("bevy_render::RenderSet::Prepare").to_string(),
            "RenderSet::Prepare"
        );
    }

    #[test]
    fn generics() {
        assert_eq!(
            ShortName("bevy_render::camera::camera::extract_cameras<bevy_render::camera::bundle::Camera3d>").to_string(),
            "extract_cameras<Camera3d>"
        );
    }

    #[test]
    fn nested_generics() {
        assert_eq!(
            ShortName("bevy::mad_science::do_mad_science<mad_science::Test<mad_science::Tube>, bavy::TypeSystemAbuse>").to_string(),
            "do_mad_science<Test<Tube>, TypeSystemAbuse>"
        );
    }

    #[test]
    fn sub_path_after_closing_bracket() {
        assert_eq!(
            ShortName("bevy_asset::assets::Assets<bevy_scene::dynamic_scene::DynamicScene>::asset_event_system").to_string(),
            "Assets<DynamicScene>::asset_event_system"
        );
        assert_eq!(
            ShortName("(String, String)::default").to_string(),
            "(String, String)::default"
        );
        assert_eq!(
            ShortName("[i32; 16]::default").to_string(),
            "[i32; 16]::default"
        );
    }

    #[test]
    fn references() {
        assert_eq!(ShortName("&T").to_string(), "&T");
        assert_eq!(ShortName("&mut T").to_string(), "&mut T");
        assert_eq!(ShortName("&t::T<&u::U>").to_string(), "&T<&U>");
        assert_eq!(
            ShortName("&mut t::T<&mut u::U>").to_string(),
            "&mut T<&mut U>"
        );
    }

    #[test]
    fn pointers() {
        assert_eq!(ShortName("*const T").to_string(), "*const T");
        assert_eq!(ShortName("*mut T").to_string(), "*mut T");
        assert_eq!(
            ShortName("*const t::T<*const u::U>").to_string(),
            "*const T<*const U>"
        );
        assert_eq!(
            ShortName("*mut t::T<*mut u::U>").to_string(),
            "*mut T<*mut U>"
        );
    }

    #[test]
    fn lifetimes() {
        assert_eq!(ShortName("t::T<'_>").to_string(), "T");
        assert_eq!(ShortName("t::T<'_, '_>").to_string(), "T");
        assert_eq!(ShortName("t::T<'_, '_, '_>").to_string(), "T");
        assert_eq!(ShortName("t::T<'_, u::U>").to_string(), "T<U>");
        assert_eq!(ShortName("t::T<'_, '_, u::U>").to_string(), "T<U>");
        assert_eq!(ShortName("t::T<'_, '_, '_, u::U>").to_string(), "T<U>");
    }

    #[test]
    fn functions() {
        assert_eq!(ShortName("m::f").to_string(), "f");
        assert_eq!(ShortName("m::f::{{closure}}").to_string(), "f::{{closure}}");
        assert_eq!(ShortName("m::Ty::f").to_string(), "Ty::f");
        assert_eq!(
            ShortName("m::Ty::f::{{closure}}").to_string(),
            "Ty::f::{{closure}}"
        );
        assert_eq!(ShortName("m::Ty<t::T>::f").to_string(), "Ty<T>::f");
        assert_eq!(
            ShortName("m::Ty<t::T>::f::{{closure}}").to_string(),
            "Ty<T>::f::{{closure}}"
        );

        assert_eq!(ShortName("m::f<t::T>").to_string(), "f<T>");
        assert_eq!(
            ShortName("m::f<t::T>::{{closure}}").to_string(),
            "f<T>::{{closure}}"
        );
        assert_eq!(ShortName("m::Ty::f<t::T>").to_string(), "Ty::f<T>");
        assert_eq!(
            ShortName("m::Ty::f<t::T>::{{closure}}").to_string(),
            "Ty::f<T>::{{closure}}"
        );
        assert_eq!(ShortName("m::Ty<t::T>::f<t::T>").to_string(), "Ty<T>::f<T>");
        assert_eq!(
            ShortName("m::Ty<t::T>::f<t::T>::{{closure}}").to_string(),
            "Ty<T>::f<T>::{{closure}}"
        );

        assert_eq!(
            ShortName("m::f::{{closure}}::{{closure}}").to_string(),
            "f::{{closure}}"
        );
        assert_eq!(
            ShortName("m::f::{{closure}}::{{closure}}::{{call_once}}").to_string(),
            "f::{{closure}}"
        );
        assert_eq!(
            ShortName("m::Ty::f::{{closure}}::{{closure}}").to_string(),
            "Ty::f::{{closure}}"
        );
        assert_eq!(
            ShortName("m::Ty::f::{{closure}}::{{closure}}::{{call_once}}").to_string(),
            "Ty::f::{{closure}}"
        );
    }
}
