use bevy_ecs::prelude::Component;
use glam::Vec3;

use crate::Color;

/// Projects a texture onto nearby geometry within an oriented bounding box.
/// The entity's `Transform` defines the projection direction and position.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Decal {
    /// Asset path to the decal texture.
    pub texture_path: String,
    /// Tint color multiplied with the texture. Use `Color::WHITE` for no tint.
    pub color: Color,
    /// Half-extents of the projection box in local space.
    pub half_extents: Vec3,
    /// Fade the decal to transparent as the surface angle approaches 90°.
    /// Value in degrees — surfaces angled beyond this are fully transparent.
    pub fade_angle: f32,
    /// Opacity of the decal (0.0 = invisible, 1.0 = fully opaque).
    pub opacity: f32,
}

impl Decal {
    pub fn new(texture_path: impl Into<String>, half_extents: Vec3) -> Self {
        Self {
            texture_path: texture_path.into(),
            color: Color::WHITE,
            half_extents: half_extents.max(Vec3::ZERO),
            fade_angle: 45.0,
            opacity: 1.0,
        }
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn with_fade_angle(mut self, degrees: f32) -> Self {
        self.fade_angle = degrees.clamp(0.0, 90.0);
        self
    }

    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn is_visible(&self) -> bool {
        self.opacity > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decal_defaults() {
        let d = Decal::new("decals/blood.png", Vec3::splat(0.5));
        assert_eq!(d.texture_path, "decals/blood.png");
        assert_eq!(d.color, Color::WHITE);
        assert!((d.fade_angle - 45.0).abs() < 0.001);
        assert!((d.opacity - 1.0).abs() < 0.001);
        assert!(d.is_visible());
    }

    #[test]
    fn decal_negative_extents_clamped() {
        let d = Decal::new("t.png", Vec3::splat(-1.0));
        assert_eq!(d.half_extents, Vec3::ZERO);
    }

    #[test]
    fn decal_fade_angle_clamped() {
        let d = Decal::new("t.png", Vec3::ONE).with_fade_angle(120.0);
        assert!((d.fade_angle - 90.0).abs() < 0.001);
    }

    #[test]
    fn decal_opacity_zero_not_visible() {
        let d = Decal::new("t.png", Vec3::ONE).with_opacity(0.0);
        assert!(!d.is_visible());
    }

    #[test]
    fn decal_opacity_clamped() {
        let d = Decal::new("t.png", Vec3::ONE).with_opacity(2.0);
        assert!((d.opacity - 1.0).abs() < 0.001);
    }
}
