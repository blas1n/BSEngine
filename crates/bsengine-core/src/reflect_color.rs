//! Opaque `Reflect` wrapper for `glam::Vec3` values that represent an RGB
//! color (linear space, 0.0-1.0 per channel by convention, matching this
//! codebase's existing color fields).
//!
//! Structurally identical to `ReflectVec2/3/4`/`ReflectQuat`
//! (`reflect_glam.rs`) — a plain `Deref<Target = glam::Vec3>` newtype — but
//! kept as its own distinct type rather than reusing `ReflectVec3`, for the
//! same reason `ReflectDegrees` is distinct from plain `f32`: the generic
//! reflected-field editor (`draw_reflect_ui`) dispatches purely on a field's
//! *type*, with no per-field naming conventions or hints (see
//! `reflect_degrees.rs`'s doc comment). A struct field typed `ReflectColor`
//! renders as a color-swatch picker; a field typed `ReflectVec3` renders as
//! three raw XYZ `DragValue`s. Both wrap the same underlying `glam::Vec3` —
//! the type alone is what tells the UI which widget to use.
use bevy_reflect::{impl_reflect_value, prelude::ReflectDefault};

/// Reflectable RGB color wrapper around a `glam::Vec3` (linear space, 0.0-1.0 per channel).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ReflectColor(pub glam::Vec3);

impl std::ops::Deref for ReflectColor {
    type Target = glam::Vec3;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for ReflectColor {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<glam::Vec3> for ReflectColor {
    fn from(value: glam::Vec3) -> Self {
        Self(value)
    }
}

impl From<ReflectColor> for glam::Vec3 {
    fn from(value: ReflectColor) -> Self {
        value.0
    }
}

impl_reflect_value!((in bsengine_core::reflect_color) ReflectColor(Debug, PartialEq, Default));

#[cfg(test)]
mod tests {
    use super::ReflectColor;
    use bevy_reflect::Reflect;

    #[test]
    fn reflect_color_reports_correct_reflect_type_path() {
        let v: ReflectColor = glam::Vec3::new(1.0, 0.5, 0.0).into();
        let reflected: &dyn Reflect = &v;
        assert_eq!(
            reflected.reflect_type_path(),
            "bsengine_core::reflect_color::ReflectColor"
        );
    }

    #[test]
    fn reflect_color_downcasts_back_to_concrete_type() {
        let v: ReflectColor = glam::Vec3::new(1.0, 0.5, 0.0).into();
        let boxed: Box<dyn Reflect> = Box::new(v);
        let back = boxed.downcast::<ReflectColor>().expect("downcast failed");
        assert_eq!(*back, v);
        assert_eq!(back.0, glam::Vec3::new(1.0, 0.5, 0.0));
    }

    #[test]
    fn reflect_color_clone_value_round_trips() {
        let v: ReflectColor = glam::Vec3::new(0.2, 0.8, 1.0).into();
        let reflected: &dyn Reflect = &v;
        let cloned = reflected.clone_value();
        let back = cloned.downcast::<ReflectColor>().expect("downcast failed");
        assert_eq!(*back, v);
    }
}
