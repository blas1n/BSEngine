use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Axis-aligned bounding box in local space.
/// Used for broad-phase collision detection and frustum culling.
/// Combine with `GlobalTransform` to get world-space bounds.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// Create from center and half-extents (positive values for each axis).
    pub fn from_center_half_extents(center: Vec3, half: Vec3) -> Self {
        Self {
            min: center - half,
            max: center + half,
        }
    }

    /// Create a unit cube centered at origin.
    pub fn unit() -> Self {
        Self::from_center_half_extents(Vec3::ZERO, Vec3::ONE * 0.5)
    }

    pub fn center(self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn size(self) -> Vec3 {
        self.max - self.min
    }

    pub fn half_extents(self) -> Vec3 {
        self.size() * 0.5
    }

    pub fn contains_point(self, point: Vec3) -> bool {
        point.cmpge(self.min).all() && point.cmple(self.max).all()
    }

    /// Returns true if this AABB overlaps with `other` (inclusive on boundary).
    pub fn intersects(self, other: Self) -> bool {
        self.min.cmple(other.max).all() && self.max.cmpge(other.min).all()
    }

    /// Expand to include the given point.
    pub fn expanded_to_include(self, point: Vec3) -> Self {
        Self {
            min: self.min.min(point),
            max: self.max.max(point),
        }
    }

    /// Union of two AABBs.
    pub fn union(self, other: Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    /// Intersection of two AABBs. Returns None if they do not overlap.
    pub fn intersection(self, other: Self) -> Option<Self> {
        let min = self.min.max(other.min);
        let max = self.max.min(other.max);
        if min.cmple(max).all() {
            Some(Self { min, max })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit() -> Aabb {
        Aabb::new(Vec3::NEG_ONE, Vec3::ONE)
    }

    #[test]
    fn center_of_unit_cube() {
        assert_eq!(unit().center(), Vec3::ZERO);
    }

    #[test]
    fn size_of_unit_cube() {
        assert_eq!(unit().size(), Vec3::ONE * 2.0);
    }

    #[test]
    fn half_extents() {
        assert_eq!(unit().half_extents(), Vec3::ONE);
    }

    #[test]
    fn from_center_half_extents() {
        let a = Aabb::from_center_half_extents(Vec3::ZERO, Vec3::ONE);
        assert_eq!(a.min, Vec3::NEG_ONE);
        assert_eq!(a.max, Vec3::ONE);
    }

    #[test]
    fn contains_interior_point() {
        assert!(unit().contains_point(Vec3::ZERO));
    }

    #[test]
    fn contains_boundary_point() {
        assert!(unit().contains_point(Vec3::ONE));
    }

    #[test]
    fn does_not_contain_exterior() {
        assert!(!unit().contains_point(Vec3::new(2.0, 0.0, 0.0)));
    }

    #[test]
    fn overlapping_aabbs_intersect() {
        let a = Aabb::new(Vec3::ZERO, Vec3::ONE * 2.0);
        let b = Aabb::new(Vec3::ONE, Vec3::ONE * 3.0);
        assert!(a.intersects(b));
        assert!(b.intersects(a));
    }

    #[test]
    fn non_overlapping_aabbs_do_not_intersect() {
        let a = Aabb::new(Vec3::ZERO, Vec3::ONE);
        let b = Aabb::new(Vec3::new(2.0, 0.0, 0.0), Vec3::new(3.0, 1.0, 1.0));
        assert!(!a.intersects(b));
    }

    #[test]
    fn touching_aabbs_intersect() {
        let a = Aabb::new(Vec3::ZERO, Vec3::ONE);
        let b = Aabb::new(Vec3::ONE, Vec3::ONE * 2.0);
        assert!(a.intersects(b));
    }

    #[test]
    fn union_enclosing_box() {
        let a = Aabb::new(Vec3::NEG_ONE, Vec3::ZERO);
        let b = Aabb::new(Vec3::ZERO, Vec3::ONE);
        let u = a.union(b);
        assert_eq!(u.min, Vec3::NEG_ONE);
        assert_eq!(u.max, Vec3::ONE);
    }

    #[test]
    fn intersection_returns_overlap() {
        let a = Aabb::new(Vec3::ZERO, Vec3::ONE * 2.0);
        let b = Aabb::new(Vec3::ONE, Vec3::ONE * 3.0);
        let i = a.intersection(b).unwrap();
        assert_eq!(i.min, Vec3::ONE);
        assert_eq!(i.max, Vec3::ONE * 2.0);
    }

    #[test]
    fn intersection_none_when_no_overlap() {
        let a = Aabb::new(Vec3::ZERO, Vec3::ONE);
        let b = Aabb::new(Vec3::new(2.0, 0.0, 0.0), Vec3::new(3.0, 1.0, 1.0));
        assert!(a.intersection(b).is_none());
    }

    #[test]
    fn expand_to_include_point() {
        let a = Aabb::new(Vec3::ZERO, Vec3::ONE);
        let expanded = a.expanded_to_include(Vec3::new(2.0, 0.0, 0.0));
        assert_eq!(expanded.max.x, 2.0);
    }
}
