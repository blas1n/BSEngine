//! Opaque `Reflect` wrapper for `f32` values that represent an angle in
//! degrees.
//!
//! Unlike `ReflectVec2/3/4`/`ReflectQuat` (see `reflect_glam.rs`), there's no
//! orphan-rule obstacle here — `f32` already implements `Reflect` (a
//! `bevy_reflect` built-in), so a plain `#[derive(Reflect)]` on a tuple
//! struct wrapping it would compile fine. `ReflectDegrees` uses
//! `impl_reflect_value!` anyway, to keep it an *opaque* leaf value (like
//! `ReflectVecN`) rather than a `TupleStruct`-shaped one — that's what lets
//! `draw_reflect_ui`'s existing `Struct`/`Enum`-only top-level dispatch pick
//! it up via `draw_leaf_ui`'s downcast-based handling, with no changes
//! needed to `draw_reflect_ui` itself.
//!
//! This type exists purely as a *display* signal for the generic reflected-
//! field editor: a struct field's *type* (not its name, not a comment)
//! declares "this number means degrees", so `draw_reflect_ui` can render it
//! without per-field naming conventions or hints. `Camera.fov_y_degrees`
//! ("PR C-2") was the first real field wired to this type;
//! `SpotLight.inner_angle_degrees`/`outer_angle_degrees` ("PR C-3") is the
//! second. Both Camera and SpotLight now store their angle fields in
//! degrees internally.
use bevy_reflect::{impl_reflect_value, prelude::ReflectDefault};

/// Reflectable `f32` wrapper representing an angle in degrees, used to signal
/// to the generic reflected-field editor that a field should render as an angle.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ReflectDegrees(pub f32);

impl std::ops::Deref for ReflectDegrees {
    type Target = f32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for ReflectDegrees {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<f32> for ReflectDegrees {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

impl From<ReflectDegrees> for f32 {
    fn from(value: ReflectDegrees) -> Self {
        value.0
    }
}

impl_reflect_value!((in bsengine_core::reflect_degrees) ReflectDegrees(Debug, PartialEq, Default));

#[cfg(test)]
mod tests {
    use super::ReflectDegrees;
    use bevy_reflect::Reflect;

    #[test]
    fn reflect_degrees_reports_correct_reflect_type_path() {
        let v: ReflectDegrees = 45.0_f32.into();
        let reflected: &dyn Reflect = &v;
        assert_eq!(
            reflected.reflect_type_path(),
            "bsengine_core::reflect_degrees::ReflectDegrees"
        );
    }

    #[test]
    fn reflect_degrees_downcasts_back_to_concrete_type() {
        let v: ReflectDegrees = 45.0_f32.into();
        let boxed: Box<dyn Reflect> = Box::new(v);
        let back = boxed.downcast::<ReflectDegrees>().expect("downcast failed");
        assert_eq!(*back, v);
        assert_eq!(back.0, 45.0_f32);
    }

    #[test]
    fn reflect_degrees_clone_value_round_trips() {
        let v: ReflectDegrees = 90.0_f32.into();
        let reflected: &dyn Reflect = &v;
        let cloned = reflected.clone_value();
        let back = cloned
            .downcast::<ReflectDegrees>()
            .expect("downcast failed");
        assert_eq!(*back, v);
    }
}
