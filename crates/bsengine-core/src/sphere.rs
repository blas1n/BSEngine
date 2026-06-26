use bevy_ecs::prelude::Component;
use glam::Vec3;
use std::f32::consts::PI;

/// Spherical bounding volume in local space.
/// Pair with `GlobalTransform` to derive the world-space center for queries.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Sphere {
    pub radius: f32,
}

impl Sphere {
    pub fn new(radius: f32) -> Self {
        Self {
            radius: radius.max(0.0),
        }
    }

    /// True if `point` is inside or on the surface of this sphere at `center`.
    pub fn contains_point(&self, center: Vec3, point: Vec3) -> bool {
        center.distance_squared(point) <= self.radius * self.radius
    }

    /// True if this sphere (at `center_a`) overlaps `other` (at `center_b`).
    pub fn intersects(&self, center_a: Vec3, other: &Sphere, center_b: Vec3) -> bool {
        let combined_radius = self.radius + other.radius;
        center_a.distance_squared(center_b) <= combined_radius * combined_radius
    }

    pub fn surface_area(&self) -> f32 {
        4.0 * PI * self.radius * self.radius
    }

    pub fn volume(&self) -> f32 {
        (4.0 / 3.0) * PI * self.radius * self.radius * self.radius
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_radius_clamped_to_zero() {
        assert_eq!(Sphere::new(-1.0).radius, 0.0);
    }

    #[test]
    fn center_is_inside() {
        let s = Sphere::new(1.0);
        assert!(s.contains_point(Vec3::ZERO, Vec3::ZERO));
    }

    #[test]
    fn surface_point_is_inside() {
        let s = Sphere::new(1.0);
        assert!(s.contains_point(Vec3::ZERO, Vec3::X));
    }

    #[test]
    fn exterior_point_is_outside() {
        let s = Sphere::new(1.0);
        assert!(!s.contains_point(Vec3::ZERO, Vec3::new(2.0, 0.0, 0.0)));
    }

    #[test]
    fn overlapping_spheres_intersect() {
        let a = Sphere::new(1.0);
        let b = Sphere::new(1.0);
        assert!(a.intersects(Vec3::ZERO, &b, Vec3::new(1.5, 0.0, 0.0)));
    }

    #[test]
    fn touching_spheres_intersect() {
        let a = Sphere::new(1.0);
        let b = Sphere::new(1.0);
        assert!(a.intersects(Vec3::ZERO, &b, Vec3::new(2.0, 0.0, 0.0)));
    }

    #[test]
    fn separated_spheres_do_not_intersect() {
        let a = Sphere::new(1.0);
        let b = Sphere::new(1.0);
        assert!(!a.intersects(Vec3::ZERO, &b, Vec3::new(3.0, 0.0, 0.0)));
    }

    #[test]
    fn surface_area_of_unit_sphere() {
        let s = Sphere::new(1.0);
        assert!((s.surface_area() - 4.0 * std::f32::consts::PI).abs() < 0.001);
    }

    #[test]
    fn volume_of_unit_sphere() {
        let s = Sphere::new(1.0);
        let expected = (4.0 / 3.0) * std::f32::consts::PI;
        assert!((s.volume() - expected).abs() < 0.001);
    }
}
