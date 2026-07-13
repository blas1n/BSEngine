//! Opaque `Reflect` impls for `glam` vector/quaternion types.
//!
//! `glam::{Vec2, Vec3, Vec4, Quat}` cannot implement `bevy_reflect::Reflect` directly: both the
//! trait (defined in `bevy_reflect`) and the types (defined in `glam`) are foreign to this crate,
//! which Rust's orphan rule forbids (confirmed directly: `impl_reflect_value!(::glam::Vec3(..))`
//! fails to compile with E0117, "only traits defined in the current crate can be implemented for
//! types defined outside of the crate"). `bevy_reflect` ships its own optional `glam` feature
//! with built-in impls, but it pins `glam = "0.27"` while this workspace uses `glam = "0.29"` —
//! those are different, non-interchangeable types to the compiler, so that feature does not help.
//!
//! Instead we define local newtype wrappers and give *those* opaque `Reflect` impls, which is
//! legal since the wrapper types are local to this crate. Callers convert at the boundary via
//! `From`/`Into`/`Deref`.
//!
//! Note on `impl_reflect_value!` syntax: a path with no leading `::` (e.g. bare `Foo`, or even a
//! multi-segment local path like `crate::reflect_glam::Foo`) is parsed as a "primitive" name and
//! its module path is discarded, which fails to resolve for anything but actual language
//! primitives. A leading-`::` absolute path can't refer back to this crate from within itself.
//! The macro's custom-path form, `impl_reflect_value!((in crate_name::module) Type(...))`, is the
//! documented way to reflect a local type: the bare `Type` resolves normally in this scope, while
//! `(in ...)` only supplies the string used for `reflect_type_path()`.
use bevy_reflect::{impl_reflect_value, prelude::ReflectDefault};

macro_rules! reflect_glam_wrapper {
    ($wrapper:ident, $inner:ty) => {
        #[derive(Debug, Clone, Copy, PartialEq, Default)]
        pub struct $wrapper(pub $inner);

        impl std::ops::Deref for $wrapper {
            type Target = $inner;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl std::ops::DerefMut for $wrapper {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        impl From<$inner> for $wrapper {
            fn from(value: $inner) -> Self {
                Self(value)
            }
        }

        impl From<$wrapper> for $inner {
            fn from(value: $wrapper) -> Self {
                value.0
            }
        }
    };
}

reflect_glam_wrapper!(ReflectVec2, glam::Vec2);
reflect_glam_wrapper!(ReflectVec3, glam::Vec3);
reflect_glam_wrapper!(ReflectVec4, glam::Vec4);
reflect_glam_wrapper!(ReflectQuat, glam::Quat);

impl_reflect_value!((in bsengine_core::reflect_glam) ReflectVec2(Debug, PartialEq, Default));
impl_reflect_value!((in bsengine_core::reflect_glam) ReflectVec3(Debug, PartialEq, Default));
impl_reflect_value!((in bsengine_core::reflect_glam) ReflectVec4(Debug, PartialEq, Default));
impl_reflect_value!((in bsengine_core::reflect_glam) ReflectQuat(Debug, PartialEq, Default));

#[cfg(test)]
mod tests {
    use super::ReflectVec3;
    use bevy_reflect::Reflect;
    use glam::Vec3;

    #[test]
    fn reflect_vec3_reports_correct_reflect_type_path() {
        let v: ReflectVec3 = Vec3::new(1.0, 2.0, 3.0).into();
        let reflected: &dyn Reflect = &v;
        assert_eq!(
            reflected.reflect_type_path(),
            "bsengine_core::reflect_glam::ReflectVec3"
        );
    }

    #[test]
    fn reflect_vec3_downcasts_back_to_concrete_type() {
        let v: ReflectVec3 = Vec3::new(1.0, 2.0, 3.0).into();
        let boxed: Box<dyn Reflect> = Box::new(v);
        let back = boxed.downcast::<ReflectVec3>().expect("downcast failed");
        assert_eq!(*back, v);
        assert_eq!(back.0, Vec3::new(1.0, 2.0, 3.0));
    }
}
