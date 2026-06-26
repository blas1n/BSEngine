use std::f32::consts::PI;

use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Unified collision shape attached to an entity.
/// Describes geometry for physics and overlap queries.
#[derive(Component, Debug, Clone, PartialEq)]
pub enum Collider {
    /// Axis-aligned box defined by half-extents from the entity's origin.
    Aabb { half_extents: Vec3 },
    /// Sphere with a given radius.
    Sphere { radius: f32 },
    /// Capsule aligned on the local Y axis with spherical caps.
    Capsule { half_height: f32, radius: f32 },
}

impl Collider {
    pub fn aabb(half_extents: Vec3) -> Self {
        Self::Aabb {
            half_extents: half_extents.max(Vec3::ZERO),
        }
    }

    pub fn sphere(radius: f32) -> Self {
        Self::Sphere {
            radius: radius.max(0.0),
        }
    }

    pub fn capsule(half_height: f32, radius: f32) -> Self {
        Self::Capsule {
            half_height: half_height.max(0.0),
            radius: radius.max(0.0),
        }
    }

    pub fn volume(&self) -> f32 {
        match self {
            Self::Aabb { half_extents } => 8.0 * half_extents.x * half_extents.y * half_extents.z,
            Self::Sphere { radius } => (4.0 / 3.0) * PI * radius * radius * radius,
            Self::Capsule {
                half_height,
                radius,
            } => {
                PI * radius * radius * (2.0 * half_height)
                    + (4.0 / 3.0) * PI * radius * radius * radius
            }
        }
    }

    pub fn surface_area(&self) -> f32 {
        match self {
            Self::Aabb { half_extents } => {
                let e = half_extents * 2.0;
                2.0 * (e.x * e.y + e.y * e.z + e.z * e.x)
            }
            Self::Sphere { radius } => 4.0 * PI * radius * radius,
            Self::Capsule {
                half_height,
                radius,
            } => 2.0 * PI * radius * (2.0 * half_height) + 4.0 * PI * radius * radius,
        }
    }

    /// Test if `point` (in the shape's local space, relative to `center`) is inside.
    pub fn contains_point(&self, center: Vec3, point: Vec3) -> bool {
        let local = point - center;
        match self {
            Self::Aabb { half_extents } => {
                local.x.abs() <= half_extents.x
                    && local.y.abs() <= half_extents.y
                    && local.z.abs() <= half_extents.z
            }
            Self::Sphere { radius } => local.length_squared() <= radius * radius,
            Self::Capsule {
                half_height,
                radius,
            } => {
                let clamped_y = local.y.clamp(-half_height, *half_height);
                let closest = Vec3::new(0.0, clamped_y, 0.0);
                (local - closest).length_squared() <= radius * radius
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aabb_volume() {
        let c = Collider::aabb(Vec3::new(1.0, 2.0, 3.0));
        assert!((c.volume() - 48.0).abs() < 0.001);
    }

    #[test]
    fn sphere_volume() {
        let c = Collider::sphere(1.0);
        let expected = (4.0 / 3.0) * PI;
        assert!((c.volume() - expected).abs() < 0.001);
    }

    #[test]
    fn aabb_contains_center() {
        let c = Collider::aabb(Vec3::splat(1.0));
        assert!(c.contains_point(Vec3::ZERO, Vec3::ZERO));
    }

    #[test]
    fn aabb_excludes_outside() {
        let c = Collider::aabb(Vec3::splat(1.0));
        assert!(!c.contains_point(Vec3::ZERO, Vec3::new(2.0, 0.0, 0.0)));
    }

    #[test]
    fn sphere_contains_point_on_surface() {
        let c = Collider::sphere(5.0);
        assert!(c.contains_point(Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0)));
    }

    #[test]
    fn capsule_contains_point_in_cylinder() {
        let c = Collider::capsule(1.0, 0.5);
        assert!(c.contains_point(Vec3::ZERO, Vec3::new(0.4, 0.5, 0.0)));
    }

    #[test]
    fn capsule_excludes_far_point() {
        let c = Collider::capsule(1.0, 0.5);
        assert!(!c.contains_point(Vec3::ZERO, Vec3::new(2.0, 0.0, 0.0)));
    }

    #[test]
    fn negative_dimensions_clamped() {
        let c = Collider::sphere(-5.0);
        if let Collider::Sphere { radius } = c {
            assert_eq!(radius, 0.0);
        }
    }
}
