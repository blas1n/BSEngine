use bevy_ecs::prelude::Component;
use glam::Vec3;
use std::f32::consts::PI;

/// Capsule bounding volume in local space — a cylinder with hemispherical caps,
/// centered at the origin and aligned to the Y axis.
/// Pair with `GlobalTransform` to get the world-space center.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Capsule {
    /// Half the height of the cylindrical shaft (not including caps).
    pub half_height: f32,
    pub radius: f32,
}

impl Capsule {
    pub fn new(half_height: f32, radius: f32) -> Self {
        Self {
            half_height: half_height.max(0.0),
            radius: radius.max(0.0),
        }
    }

    /// Total height including hemispherical caps (2 * half_height + 2 * radius).
    pub fn total_height(&self) -> f32 {
        2.0 * self.half_height + 2.0 * self.radius
    }

    /// Volume of the cylindrical shaft plus the full sphere formed by the two caps.
    pub fn volume(&self) -> f32 {
        let cylinder = PI * self.radius * self.radius * (2.0 * self.half_height);
        let sphere = (4.0 / 3.0) * PI * self.radius * self.radius * self.radius;
        cylinder + sphere
    }

    /// Surface area of the cylindrical wall plus the full sphere formed by the two caps.
    pub fn surface_area(&self) -> f32 {
        let cylinder_wall = 2.0 * PI * self.radius * (2.0 * self.half_height);
        let sphere = 4.0 * PI * self.radius * self.radius;
        cylinder_wall + sphere
    }

    /// Closest point on the capsule's central segment (the Y-axis shaft) to `point`.
    pub fn closest_point_on_segment(&self, point: Vec3) -> Vec3 {
        let clamped_y = point.y.clamp(-self.half_height, self.half_height);
        Vec3::new(0.0, clamped_y, 0.0)
    }

    /// True if `point` (in local space, capsule centered at origin) is inside this capsule.
    pub fn contains_point(&self, point: Vec3) -> bool {
        let closest = self.closest_point_on_segment(point);
        point.distance_squared(closest) <= self.radius * self.radius
    }

    /// True if this capsule (centered at `pos_a`) overlaps `other` (centered at `pos_b`).
    /// Both capsules are Y-axis aligned in their local spaces.
    pub fn intersects(&self, pos_a: Vec3, other: &Capsule, pos_b: Vec3) -> bool {
        // Build segment endpoints in world space
        let a0 = Vec3::new(pos_a.x, pos_a.y - self.half_height, pos_a.z);
        let a1 = Vec3::new(pos_a.x, pos_a.y + self.half_height, pos_a.z);
        let b0 = Vec3::new(pos_b.x, pos_b.y - other.half_height, pos_b.z);
        let b1 = Vec3::new(pos_b.x, pos_b.y + other.half_height, pos_b.z);

        let combined_radius = self.radius + other.radius;
        segment_segment_sq_dist(a0, a1, b0, b1) <= combined_radius * combined_radius
    }
}

/// Minimum squared distance between two line segments (Ericson, Real-Time Collision Detection §5.1.9).
fn segment_segment_sq_dist(p0: Vec3, p1: Vec3, q0: Vec3, q1: Vec3) -> f32 {
    let d1 = p1 - p0;
    let d2 = q1 - q0;
    let r = p0 - q0;

    let a = d1.dot(d1);
    let e = d2.dot(d2);
    let f = d2.dot(r);

    let (mut s, mut t) = if a <= f32::EPSILON {
        if e <= f32::EPSILON {
            (0.0_f32, 0.0_f32)
        } else {
            (0.0, (f / e).clamp(0.0, 1.0))
        }
    } else {
        let c = d1.dot(r);
        if e <= f32::EPSILON {
            ((-c / a).clamp(0.0, 1.0), 0.0)
        } else {
            let b = d1.dot(d2);
            let denom = a * e - b * b;

            let s_unclamped = if denom > f32::EPSILON {
                ((b * f - c * e) / denom).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let t_unclamped = (b * s_unclamped + f) / e;
            if t_unclamped < 0.0 {
                (-c / a, 0.0_f32)
            } else if t_unclamped > 1.0 {
                (((b - c) / a).clamp(0.0, 1.0), 1.0)
            } else {
                (s_unclamped, t_unclamped)
            }
        }
    };

    s = s.clamp(0.0, 1.0);
    t = t.clamp(0.0, 1.0);

    let closest_p = p0 + d1 * s;
    let closest_q = q0 + d2 * t;
    (closest_p - closest_q).length_squared()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_values_clamped() {
        let c = Capsule::new(-1.0, -2.0);
        assert_eq!(c.half_height, 0.0);
        assert_eq!(c.radius, 0.0);
    }

    #[test]
    fn total_height_includes_caps() {
        let c = Capsule::new(1.0, 0.5);
        assert!((c.total_height() - 3.0).abs() < 0.001);
    }

    #[test]
    fn center_point_is_inside() {
        let c = Capsule::new(1.0, 0.5);
        assert!(c.contains_point(Vec3::ZERO));
    }

    #[test]
    fn point_in_top_cap_is_inside() {
        let c = Capsule::new(1.0, 0.5);
        // Top segment endpoint is (0, 1, 0), so (0, 1.4, 0) is inside cap
        assert!(c.contains_point(Vec3::new(0.0, 1.4, 0.0)));
    }

    #[test]
    fn point_above_top_cap_is_outside() {
        let c = Capsule::new(1.0, 0.5);
        assert!(!c.contains_point(Vec3::new(0.0, 2.0, 0.0)));
    }

    #[test]
    fn point_on_side_inside_radius() {
        let c = Capsule::new(1.0, 0.5);
        assert!(c.contains_point(Vec3::new(0.4, 0.0, 0.0)));
    }

    #[test]
    fn point_on_side_outside_radius() {
        let c = Capsule::new(1.0, 0.5);
        assert!(!c.contains_point(Vec3::new(0.6, 0.0, 0.0)));
    }

    #[test]
    fn overlapping_capsules_intersect() {
        let a = Capsule::new(1.0, 0.5);
        let b = Capsule::new(1.0, 0.5);
        assert!(a.intersects(Vec3::ZERO, &b, Vec3::new(0.8, 0.0, 0.0)));
    }

    #[test]
    fn separated_capsules_do_not_intersect() {
        let a = Capsule::new(1.0, 0.5);
        let b = Capsule::new(1.0, 0.5);
        assert!(!a.intersects(Vec3::ZERO, &b, Vec3::new(3.0, 0.0, 0.0)));
    }

    #[test]
    fn stacked_capsules_intersect() {
        // One directly above the other, caps touching
        let a = Capsule::new(1.0, 0.5);
        let b = Capsule::new(1.0, 0.5);
        // a top cap at y=1.5, b bottom cap at y = 2.0 - 1.5 = 0.5 → they overlap
        assert!(a.intersects(Vec3::ZERO, &b, Vec3::new(0.0, 2.5, 0.0)));
    }

    #[test]
    fn volume_greater_than_sphere() {
        let c = Capsule::new(1.0, 1.0); // half_height > 0, so more than just sphere
        let sphere_vol = (4.0 / 3.0) * std::f32::consts::PI;
        assert!(c.volume() > sphere_vol);
    }
}
