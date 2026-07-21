//! Opaque `Reflect` wrapper for `glam::Mat4`.
//!
//! Structurally identical to `ReflectVec2/3/4`/`ReflectQuat`
//! (`reflect_glam.rs`) and `ReflectColor`/`ReflectDegrees` — a plain
//! `Deref<Target = glam::Mat4>` newtype, needed for the same orphan-rule
//! reason documented in `reflect_glam.rs`: neither `Reflect` nor `glam::Mat4`
//! is local to this crate, so `Reflect` can't be implemented for `Mat4`
//! directly.
use bevy_reflect::{impl_reflect_value, prelude::ReflectDefault};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ReflectMat4(pub glam::Mat4);

impl std::ops::Deref for ReflectMat4 {
    type Target = glam::Mat4;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for ReflectMat4 {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<glam::Mat4> for ReflectMat4 {
    fn from(value: glam::Mat4) -> Self {
        Self(value)
    }
}

impl From<ReflectMat4> for glam::Mat4 {
    fn from(value: ReflectMat4) -> Self {
        value.0
    }
}

impl_reflect_value!((in bsengine_core::reflect_mat4) ReflectMat4(Debug, PartialEq, Default));

#[cfg(test)]
mod tests {
    use super::ReflectMat4;
    use bevy_reflect::Reflect;

    #[test]
    fn reflect_mat4_reports_correct_reflect_type_path() {
        let v: ReflectMat4 = glam::Mat4::IDENTITY.into();
        let reflected: &dyn Reflect = &v;
        assert_eq!(
            reflected.reflect_type_path(),
            "bsengine_core::reflect_mat4::ReflectMat4"
        );
    }

    #[test]
    fn reflect_mat4_downcasts_back_to_concrete_type() {
        let v: ReflectMat4 = glam::Mat4::IDENTITY.into();
        let boxed: Box<dyn Reflect> = Box::new(v);
        let back = boxed.downcast::<ReflectMat4>().expect("downcast failed");
        assert_eq!(*back, v);
        assert_eq!(back.0, glam::Mat4::IDENTITY);
    }

    #[test]
    fn reflect_mat4_clone_value_round_trips() {
        let v: ReflectMat4 = glam::Mat4::from_scale(glam::Vec3::new(2.0, 3.0, 4.0)).into();
        let reflected: &dyn Reflect = &v;
        let cloned = reflected.clone_value();
        let back = cloned.downcast::<ReflectMat4>().expect("downcast failed");
        assert_eq!(*back, v);
    }
}
