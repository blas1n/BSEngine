use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Transform {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
        }
    }
}

impl Transform {
    pub fn from_translation(x: f32, y: f32, z: f32) -> Self {
        Self {
            translation: [x, y, z],
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Transform;

    #[test]
    fn default_is_identity() {
        let t = Transform::default();
        assert_eq!(t.translation, [0.0, 0.0, 0.0]);
        assert_eq!(t.rotation, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(t.scale, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn from_translation_sets_position() {
        let t = Transform::from_translation(1.0, 2.0, 3.0);
        assert_eq!(t.translation, [1.0, 2.0, 3.0]);
        assert_eq!(t.scale, [1.0, 1.0, 1.0]);
    }
}
