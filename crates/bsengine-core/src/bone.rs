use bevy_ecs::prelude::Component;
use glam::Mat4;

/// A joint in a skeletal hierarchy.
/// Bone entities are typically children of a skeleton root entity.
/// The skinning system reads all bones referenced by a `SkinnedMesh` each frame.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Bone {
    /// Unique name of this bone within its skeleton (e.g. "spine", "head").
    pub name: String,
    /// Index within the skeleton's bone array. Must match the order in `SkinnedMesh::bones`.
    pub index: usize,
    /// Transforms vertices from bind-pose space into this bone's local space.
    /// Typically loaded from the skeleton asset.
    pub inverse_bind_pose: Mat4,
}

impl Bone {
    pub fn new(name: impl Into<String>, index: usize) -> Self {
        Self {
            name: name.into(),
            index,
            inverse_bind_pose: Mat4::IDENTITY,
        }
    }

    pub fn with_inverse_bind_pose(mut self, matrix: Mat4) -> Self {
        self.inverse_bind_pose = matrix;
        self
    }

    /// Computes the final skinning matrix: `global_transform * inverse_bind_pose`.
    /// Call this with the bone entity's current global transform each frame.
    pub fn skinning_matrix(&self, global_transform: Mat4) -> Mat4 {
        global_transform * self.inverse_bind_pose
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn bone_defaults() {
        let b = Bone::new("spine", 0);
        assert_eq!(b.name, "spine");
        assert_eq!(b.index, 0);
        assert_eq!(b.inverse_bind_pose, Mat4::IDENTITY);
    }

    #[test]
    fn bone_with_custom_inverse_bind() {
        let ibp = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
        let b = Bone::new("head", 1).with_inverse_bind_pose(ibp);
        assert_eq!(b.inverse_bind_pose, ibp);
    }

    #[test]
    fn skinning_matrix_identity_global() {
        let b = Bone::new("arm", 2);
        let skinning = b.skinning_matrix(Mat4::IDENTITY);
        assert_eq!(skinning, Mat4::IDENTITY);
    }

    #[test]
    fn skinning_matrix_combines_transforms() {
        let global = Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0));
        let ibp = Mat4::from_translation(Vec3::new(-5.0, 0.0, 0.0));
        let b = Bone::new("foot", 3).with_inverse_bind_pose(ibp);
        let skinning = b.skinning_matrix(global);
        // global * ibp should be close to identity (translate by 5, then by -5)
        let expected = global * ibp;
        assert_eq!(skinning, expected);
    }
}
