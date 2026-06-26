use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Shape of the hitbox volume.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HitboxShape {
    /// Axis-aligned box with given half-extents.
    Box { half_extents: Vec3 },
    /// Sphere of the given radius.
    Sphere { radius: f32 },
    /// Upright capsule — radius and half-height along Y.
    Capsule { radius: f32, half_height: f32 },
}

/// A logical hit-detection volume separate from the physics collider.
/// Hitboxes define where the entity can be hit by attacks, projectiles, and line-of-sight checks.
/// Multiple `Hitbox` components can coexist on child entities for per-bone hitboxes.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Hitbox {
    pub shape: HitboxShape,
    /// Local-space offset from the entity's transform origin.
    pub offset: Vec3,
    /// Logical group tag, e.g. `"head"`, `"body"`, `"limb"`. Empty = default.
    pub group: String,
    /// Damage multiplier applied when this hitbox is struck (e.g. 2.0 = headshot).
    pub damage_multiplier: f32,
    pub enabled: bool,
}

impl Hitbox {
    pub fn new(shape: HitboxShape) -> Self {
        Self {
            shape,
            offset: Vec3::ZERO,
            group: String::new(),
            damage_multiplier: 1.0,
            enabled: true,
        }
    }

    pub fn r#box(half_extents: Vec3) -> Self {
        Self::new(HitboxShape::Box { half_extents })
    }

    pub fn sphere(radius: f32) -> Self {
        Self::new(HitboxShape::Sphere {
            radius: radius.max(0.0),
        })
    }

    pub fn capsule(radius: f32, half_height: f32) -> Self {
        Self::new(HitboxShape::Capsule {
            radius: radius.max(0.0),
            half_height: half_height.max(0.0),
        })
    }

    pub fn with_offset(mut self, offset: Vec3) -> Self {
        self.offset = offset;
        self
    }

    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        self.group = group.into();
        self
    }

    pub fn with_damage_multiplier(mut self, multiplier: f32) -> Self {
        self.damage_multiplier = multiplier.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Volume of the hitbox shape in cubic metres. Useful for priority sorting.
    pub fn volume(&self) -> f32 {
        match self.shape {
            HitboxShape::Box { half_extents } => {
                8.0 * half_extents.x * half_extents.y * half_extents.z
            }
            HitboxShape::Sphere { radius } => (4.0 / 3.0) * std::f32::consts::PI * radius.powi(3),
            HitboxShape::Capsule {
                radius,
                half_height,
            } => {
                (4.0 / 3.0) * std::f32::consts::PI * radius.powi(3)
                    + std::f32::consts::PI * radius.powi(2) * (2.0 * half_height)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hitbox_box_defaults() {
        let h = Hitbox::r#box(Vec3::ONE);
        assert_eq!(h.group, "");
        assert!((h.damage_multiplier - 1.0).abs() < 0.001);
        assert!(h.enabled);
    }

    #[test]
    fn hitbox_sphere_volume() {
        let h = Hitbox::sphere(1.0);
        let expected = (4.0 / 3.0) * std::f32::consts::PI;
        assert!((h.volume() - expected).abs() < 0.001);
    }

    #[test]
    fn hitbox_group_and_multiplier() {
        let h = Hitbox::sphere(0.3)
            .with_group("head")
            .with_damage_multiplier(2.0);
        assert_eq!(h.group, "head");
        assert!((h.damage_multiplier - 2.0).abs() < 0.001);
    }

    #[test]
    fn hitbox_box_volume() {
        let h = Hitbox::r#box(Vec3::new(1.0, 2.0, 0.5));
        assert!((h.volume() - 8.0).abs() < 0.001); // 8 * 1 * 2 * 0.5
    }

    #[test]
    fn hitbox_disabled() {
        let h = Hitbox::capsule(0.5, 1.0).disabled();
        assert!(!h.enabled);
    }
}
