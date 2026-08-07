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
use bevy_reflect::{
    impl_reflect_value, prelude::ReflectDefault, ReflectDeserialize, ReflectSerialize,
};

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

/// Serialised as three floats, so a scene writes `(1.0, 0.85, 0.4)`.
///
/// Hand-written rather than derived so that `glam`'s optional `serde` feature
/// stays off: the wrapper is three f32s and nothing about the representation
/// needs `Vec3` to know how to serialise itself.
impl serde::Serialize for ReflectColor {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeTuple;
        let mut tuple = serializer.serialize_tuple(3)?;
        tuple.serialize_element(&self.0.x)?;
        tuple.serialize_element(&self.0.y)?;
        tuple.serialize_element(&self.0.z)?;
        tuple.end()
    }
}

impl<'de> serde::Deserialize<'de> for ReflectColor {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let [r, g, b] = <[f32; 3]>::deserialize(deserializer)?;
        Ok(Self(glam::Vec3::new(r, g, b)))
    }
}

// `Serialize`/`Deserialize` in this list are what register `ReflectSerialize`
// and `ReflectDeserialize` for the type, and those are what a scene needs.
//
// `impl_reflect_value!` makes this an *opaque* type as far as reflection is
// concerned, and an opaque type can only be deserialised through
// `ReflectDeserialize`. Structural types -- structs, tuples, arrays -- are
// walked field by field and need none of this, which is why a component made
// of plain f32s always worked and one holding a colour did not: the component
// came back absent, with a warning and no error, exactly the failure mode
// item 29 recorded.
//
// The sibling wrappers in `reflect_glam.rs` and `reflect_degrees.rs` still
// have the gap. Nothing authors them from a scene today, so this fixes the one
// that a scene needed rather than all of them at once.
impl_reflect_value!((in bsengine_core::reflect_color) ReflectColor(
    Debug,
    PartialEq,
    Default,
    Serialize,
    Deserialize
));

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
