use bevy_ecs::prelude::Component;
use glam::{Quat, Vec3};

/// A named socket on an entity where child entities or effects can be attached.
/// Used by the attachment system to position child entities in bone or local space.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct AttachPoint {
    /// Logical name for this socket, e.g. `"hand_r"`, `"muzzle"`, `"spine_01"`.
    pub name: String,
    /// Name of the skeleton bone this point tracks. `None` = entity local space.
    pub bone_name: Option<String>,
    /// Position offset relative to the bone (or entity origin).
    pub local_offset: Vec3,
    /// Rotation applied on top of the bone rotation.
    pub local_rotation: Quat,
}

impl AttachPoint {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            bone_name: None,
            local_offset: Vec3::ZERO,
            local_rotation: Quat::IDENTITY,
        }
    }

    pub fn on_bone(mut self, bone_name: impl Into<String>) -> Self {
        self.bone_name = Some(bone_name.into());
        self
    }

    pub fn with_offset(mut self, offset: Vec3) -> Self {
        self.local_offset = offset;
        self
    }

    pub fn with_rotation(mut self, rotation: Quat) -> Self {
        self.local_rotation = rotation.normalize();
        self
    }

    /// Returns `true` if this attach point is bound to a named bone.
    pub fn is_bone_relative(&self) -> bool {
        self.bone_name.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attach_point_defaults() {
        let ap = AttachPoint::new("hand_r");
        assert_eq!(ap.name, "hand_r");
        assert!(ap.bone_name.is_none());
        assert_eq!(ap.local_offset, Vec3::ZERO);
        assert_eq!(ap.local_rotation, Quat::IDENTITY);
        assert!(!ap.is_bone_relative());
    }

    #[test]
    fn on_bone_sets_bone_name() {
        let ap = AttachPoint::new("muzzle").on_bone("weapon_r");
        assert_eq!(ap.bone_name.as_deref(), Some("weapon_r"));
        assert!(ap.is_bone_relative());
    }

    #[test]
    fn with_offset() {
        let ap = AttachPoint::new("tip").with_offset(Vec3::new(0.0, 0.5, 0.0));
        assert_eq!(ap.local_offset, Vec3::new(0.0, 0.5, 0.0));
    }

    #[test]
    fn with_rotation_normalizes() {
        let non_unit = Quat::from_xyzw(0.0, 1.0, 0.0, 0.0);
        let ap = AttachPoint::new("spine").with_rotation(non_unit);
        let len = (ap.local_rotation.x.powi(2)
            + ap.local_rotation.y.powi(2)
            + ap.local_rotation.z.powi(2)
            + ap.local_rotation.w.powi(2))
        .sqrt();
        assert!((len - 1.0).abs() < 0.001);
    }
}
