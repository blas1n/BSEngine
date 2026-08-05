//! Convex decomposition of walkable space into axis-aligned rectangles.
//!
//! The navigation mesh is the free area of a level: a walkable bounds with the
//! footprints of obstacles removed. Decomposing what remains into convex pieces
//! is what makes pathfinding tractable — inside a convex region any two points
//! are joined by a straight line, so a path only has to decide *which regions*
//! to cross, and the geometry between region boundaries takes care of itself.
//!
//! Rectangles rather than arbitrary polygons because every collider in this
//! engine is a box, sphere or capsule, all of which project to an axis-aligned
//! footprint. Nothing here would survive triangle-soup input, and nothing here
//! pretends it would.

use glam::Vec3;

/// An axis-aligned rectangle in the XZ plane.
///
/// `min` is inclusive and `max` exclusive in the sense that touching edges do
/// not overlap — two rectangles sharing a boundary are adjacent, not
/// intersecting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Lower X bound.
    pub min_x: f32,
    /// Upper X bound.
    pub max_x: f32,
    /// Lower Z bound.
    pub min_z: f32,
    /// Upper Z bound.
    pub max_z: f32,
}

impl Rect {
    /// Builds a rectangle, ordering the bounds so `min` is never above `max`.
    pub fn new(min_x: f32, max_x: f32, min_z: f32, max_z: f32) -> Self {
        Self {
            min_x: min_x.min(max_x),
            max_x: min_x.max(max_x),
            min_z: min_z.min(max_z),
            max_z: min_z.max(max_z),
        }
    }

    /// Whether the rectangle encloses zero area, and so is not walkable.
    pub fn is_empty(&self) -> bool {
        self.max_x - self.min_x <= f32::EPSILON || self.max_z - self.min_z <= f32::EPSILON
    }

    /// Whether `point`'s XZ position lies inside. The Y coordinate is ignored:
    /// this navigation mesh is planar.
    pub fn contains(&self, point: Vec3) -> bool {
        point.x >= self.min_x
            && point.x <= self.max_x
            && point.z >= self.min_z
            && point.z <= self.max_z
    }

    /// The centre, at the given height.
    pub fn center(&self, y: f32) -> Vec3 {
        Vec3::new(
            (self.min_x + self.max_x) * 0.5,
            y,
            (self.min_z + self.max_z) * 0.5,
        )
    }

    /// Whether the two rectangles share more than a point of boundary.
    ///
    /// Touching at a single corner is not adjacency — an agent cannot walk
    /// through a corner, and treating it as a connection produces paths that
    /// clip obstacles diagonally, which is one of the artifacts the grid
    /// implementation had.
    pub fn adjacent_to(&self, other: &Rect) -> bool {
        let x_overlap = self.min_x.max(other.min_x) < self.max_x.min(other.max_x);
        let z_overlap = self.min_z.max(other.min_z) < self.max_z.min(other.max_z);
        let touch_x =
            (self.max_x - other.min_x).abs() < EPS || (other.max_x - self.min_x).abs() < EPS;
        let touch_z =
            (self.max_z - other.min_z).abs() < EPS || (other.max_z - self.min_z).abs() < EPS;
        (touch_x && z_overlap) || (touch_z && x_overlap)
    }

    /// The shared boundary segment with an adjacent rectangle, as its two
    /// endpoints. Used as the portal a path passes through.
    pub fn portal_to(&self, other: &Rect, y: f32) -> Option<(Vec3, Vec3)> {
        if !self.adjacent_to(other) {
            return None;
        }
        let z_lo = self.min_z.max(other.min_z);
        let z_hi = self.max_z.min(other.max_z);
        if z_hi > z_lo {
            let x = if (self.max_x - other.min_x).abs() < EPS {
                self.max_x
            } else {
                self.min_x
            };
            return Some((Vec3::new(x, y, z_lo), Vec3::new(x, y, z_hi)));
        }
        let x_lo = self.min_x.max(other.min_x);
        let x_hi = self.max_x.min(other.max_x);
        let z = if (self.max_z - other.min_z).abs() < EPS {
            self.max_z
        } else {
            self.min_z
        };
        Some((Vec3::new(x_lo, y, z), Vec3::new(x_hi, y, z)))
    }
}

const EPS: f32 = 1e-4;

/// Decomposes `bounds` minus `obstacles` into convex rectangles.
///
/// Sweeps the X axis at every obstacle edge, producing vertical strips in which
/// the set of blocking obstacles does not change. Within a strip the free space
/// is a set of Z intervals, each of which is a rectangle. The result covers the
/// walkable area exactly, with no overlaps.
pub fn decompose(bounds: Rect, obstacles: &[Rect]) -> Vec<Rect> {
    if bounds.is_empty() {
        return Vec::new();
    }

    // Every X at which the blocking set can change.
    let mut xs = vec![bounds.min_x, bounds.max_x];
    for o in obstacles {
        for x in [o.min_x, o.max_x] {
            if x > bounds.min_x && x < bounds.max_x {
                xs.push(x);
            }
        }
    }
    xs.sort_by(|a, b| a.partial_cmp(b).expect("navmesh bounds must be finite"));
    xs.dedup_by(|a, b| (*a - *b).abs() < EPS);

    let mut out = Vec::new();
    for pair in xs.windows(2) {
        let (x0, x1) = (pair[0], pair[1]);
        if x1 - x0 <= EPS {
            continue;
        }
        let mid_x = (x0 + x1) * 0.5;

        // Z intervals blocked somewhere in this strip.
        let mut blocked: Vec<(f32, f32)> = obstacles
            .iter()
            .filter(|o| o.min_x < mid_x && o.max_x > mid_x)
            .map(|o| (o.min_z.max(bounds.min_z), o.max_z.min(bounds.max_z)))
            .filter(|(lo, hi)| hi > lo)
            .collect();
        blocked.sort_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .expect("obstacle bounds must be finite")
        });

        // Walk the gaps between blocked intervals.
        let mut z = bounds.min_z;
        for (lo, hi) in blocked {
            if lo > z {
                out.push(Rect::new(x0, x1, z, lo));
            }
            z = z.max(hi);
        }
        if bounds.max_z > z {
            out.push(Rect::new(x0, x1, z, bounds.max_z));
        }
    }
    out.retain(|r| !r.is_empty());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_level_decomposes_to_one_rectangle() {
        let bounds = Rect::new(0.0, 10.0, 0.0, 10.0);
        let polys = decompose(bounds, &[]);
        assert_eq!(polys.len(), 1);
        assert_eq!(polys[0], bounds);
    }

    #[test]
    fn an_obstacle_splits_the_space_around_it() {
        // A pillar in the middle leaves free space on all four sides, and the
        // sweep produces it as three strips: everything left of the pillar,
        // the two slivers above and below it, and everything right.
        let bounds = Rect::new(0.0, 10.0, 0.0, 10.0);
        let polys = decompose(bounds, &[Rect::new(4.0, 6.0, 4.0, 6.0)]);

        assert_eq!(polys.len(), 4, "left, below, above, right — got {polys:?}");
        let total: f32 = polys
            .iter()
            .map(|r| (r.max_x - r.min_x) * (r.max_z - r.min_z))
            .sum();
        assert!(
            (total - (100.0 - 4.0)).abs() < 1e-3,
            "the pieces should cover the level minus the obstacle, got {total}"
        );
        assert!(
            polys.iter().all(|r| !r.contains(Vec3::new(5.0, 0.0, 5.0))),
            "no piece may contain the obstacle's centre"
        );
    }

    #[test]
    fn a_wall_with_a_gap_leaves_the_gap_walkable() {
        // The case a uniform grid gets wrong when the gap is narrower than a
        // cell: here the gap is 0.3 wide, far below any sane cell size, and it
        // survives decomposition exactly.
        let bounds = Rect::new(0.0, 10.0, 0.0, 10.0);
        let wall_lower = Rect::new(4.0, 5.0, 0.0, 4.85);
        let wall_upper = Rect::new(4.0, 5.0, 5.15, 10.0);
        let polys = decompose(bounds, &[wall_lower, wall_upper]);

        let gap = polys
            .iter()
            .find(|r| r.contains(Vec3::new(4.5, 0.0, 5.0)))
            .expect("the gap in the wall must be walkable");
        assert!(
            (gap.max_z - gap.min_z - 0.3).abs() < 1e-3,
            "the gap should keep its exact width, got {}",
            gap.max_z - gap.min_z
        );
    }

    #[test]
    fn pieces_that_share_an_edge_are_adjacent() {
        let left = Rect::new(0.0, 5.0, 0.0, 10.0);
        let right = Rect::new(5.0, 10.0, 0.0, 10.0);
        assert!(left.adjacent_to(&right));
        assert!(right.adjacent_to(&left));
    }

    #[test]
    fn pieces_that_meet_only_at_a_corner_are_not_adjacent() {
        // Diagonal corner-cutting is one of the grid implementation's
        // artifacts; an agent cannot squeeze through a point.
        let lower_left = Rect::new(0.0, 5.0, 0.0, 5.0);
        let upper_right = Rect::new(5.0, 10.0, 5.0, 10.0);
        assert!(!lower_left.adjacent_to(&upper_right));
    }

    #[test]
    fn a_portal_is_the_shared_edge() {
        let left = Rect::new(0.0, 5.0, 2.0, 8.0);
        let right = Rect::new(5.0, 10.0, 0.0, 6.0);
        let (a, b) = left.portal_to(&right, 0.0).expect("they are adjacent");
        assert_eq!(a.x, 5.0);
        assert_eq!(b.x, 5.0);
        // The portal is the overlap of their Z ranges, not either one's whole edge.
        assert_eq!((a.z, b.z), (2.0, 6.0));
    }
}
