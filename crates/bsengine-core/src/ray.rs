use glam::Vec3;

use crate::Aabb;

/// A ray in 3D space — used for raycasting, picking, and line-of-sight tests.
/// Direction is always normalized on construction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    pub origin: Vec3,
    /// Unit vector. Use `Ray::new()` to construct with auto-normalization.
    pub direction: Vec3,
}

impl Ray {
    /// Construct a ray. `direction` is normalized automatically.
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
        }
    }

    /// Point along the ray at parameter `t`. Negative `t` goes behind the origin.
    pub fn at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }

    /// Slab-method AABB intersection. Returns `Some(t)` at the entry point,
    /// where `t >= 0` means the intersection is in front of the origin.
    /// Returns `None` if the ray misses or the box is fully behind the origin.
    pub fn intersect_aabb(&self, aabb: &Aabb) -> Option<f32> {
        let inv_dir = Vec3::new(
            if self.direction.x != 0.0 {
                1.0 / self.direction.x
            } else {
                f32::INFINITY
            },
            if self.direction.y != 0.0 {
                1.0 / self.direction.y
            } else {
                f32::INFINITY
            },
            if self.direction.z != 0.0 {
                1.0 / self.direction.z
            } else {
                f32::INFINITY
            },
        );

        let t1 = (aabb.min - self.origin) * inv_dir;
        let t2 = (aabb.max - self.origin) * inv_dir;

        let t_min = t1.min(t2);
        let t_max = t1.max(t2);

        let entry = t_min.x.max(t_min.y).max(t_min.z);
        let exit = t_max.x.min(t_max.y).min(t_max.z);

        if entry > exit || exit < 0.0 {
            return None;
        }

        let t = if entry >= 0.0 { entry } else { exit };
        Some(t)
    }

    /// Sphere intersection. Returns `Some(t)` at the nearest surface hit
    /// in front of the origin, or `None` if the ray misses.
    pub fn intersect_sphere(&self, center: Vec3, radius: f32) -> Option<f32> {
        let l = center - self.origin;
        let tca = l.dot(self.direction);
        let d2 = l.dot(l) - tca * tca;
        let r2 = radius * radius;

        if d2 > r2 {
            return None;
        }

        let thc = (r2 - d2).sqrt();
        let t0 = tca - thc;
        let t1 = tca + thc;

        if t1 < 0.0 {
            return None; // both intersections behind origin
        }

        Some(if t0 >= 0.0 { t0 } else { t1 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    fn unit_aabb() -> Aabb {
        Aabb::new(Vec3::NEG_ONE, Vec3::ONE)
    }

    #[test]
    fn direction_is_normalized() {
        let r = Ray::new(Vec3::ZERO, Vec3::new(0.0, 0.0, 2.0));
        assert!((r.direction.length() - 1.0).abs() < 0.001);
    }

    #[test]
    fn at_returns_point_along_ray() {
        let r = Ray::new(Vec3::ZERO, Vec3::Z);
        assert_eq!(r.at(3.0), Vec3::new(0.0, 0.0, 3.0));
    }

    #[test]
    fn ray_hits_aabb_head_on() {
        let r = Ray::new(Vec3::new(0.0, 0.0, -5.0), Vec3::Z);
        let t = r.intersect_aabb(&unit_aabb());
        assert!(t.is_some());
        assert!((t.unwrap() - 4.0).abs() < 0.001); // hits at z = -1, so t = 4
    }

    #[test]
    fn ray_misses_aabb() {
        let r = Ray::new(Vec3::new(5.0, 0.0, -5.0), Vec3::Z);
        assert!(r.intersect_aabb(&unit_aabb()).is_none());
    }

    #[test]
    fn ray_behind_aabb_returns_none() {
        let r = Ray::new(Vec3::new(0.0, 0.0, 5.0), Vec3::Z); // pointing away
        assert!(r.intersect_aabb(&unit_aabb()).is_none());
    }

    #[test]
    fn ray_inside_aabb_hits_exit() {
        let r = Ray::new(Vec3::ZERO, Vec3::Z); // inside unit_aabb
        let t = r.intersect_aabb(&unit_aabb());
        assert!(t.is_some());
        assert!(t.unwrap() >= 0.0);
    }

    #[test]
    fn ray_hits_sphere() {
        let r = Ray::new(Vec3::new(0.0, 0.0, -5.0), Vec3::Z);
        let t = r.intersect_sphere(Vec3::ZERO, 1.0);
        assert!(t.is_some());
        assert!((t.unwrap() - 4.0).abs() < 0.001);
    }

    #[test]
    fn ray_misses_sphere() {
        let r = Ray::new(Vec3::new(5.0, 0.0, -5.0), Vec3::Z);
        assert!(r.intersect_sphere(Vec3::ZERO, 1.0).is_none());
    }

    #[test]
    fn ray_behind_sphere_returns_none() {
        let r = Ray::new(Vec3::new(0.0, 0.0, 5.0), Vec3::Z);
        assert!(r.intersect_sphere(Vec3::ZERO, 1.0).is_none());
    }

    #[test]
    fn ray_inside_sphere_returns_exit_point() {
        let r = Ray::new(Vec3::ZERO, Vec3::Z); // inside sphere of radius 2
        let t = r.intersect_sphere(Vec3::ZERO, 2.0);
        assert!(t.is_some());
        assert!((t.unwrap() - 2.0).abs() < 0.001);
    }
}
