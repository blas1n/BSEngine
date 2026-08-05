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

/// A decomposed level: convex pieces plus who borders whom.
#[derive(Debug, Clone, Default)]
pub struct NavPolys {
    /// The convex pieces covering the walkable area.
    pub rects: Vec<Rect>,
    /// `neighbours[i]` lists the indices sharing an edge with `rects[i]`.
    neighbours: Vec<Vec<usize>>,
    /// The plane this navigation mesh lies on.
    y: f32,
}

impl NavPolys {
    /// Builds the adjacency graph over a decomposition.
    pub fn new(rects: Vec<Rect>, y: f32) -> Self {
        let neighbours = (0..rects.len())
            .map(|i| {
                (0..rects.len())
                    .filter(|&j| j != i && rects[i].adjacent_to(&rects[j]))
                    .collect()
            })
            .collect();
        Self {
            rects,
            neighbours,
            y,
        }
    }

    /// Decomposes `bounds` minus `obstacles` and builds the graph.
    pub fn build(bounds: Rect, obstacles: &[Rect], y: f32) -> Self {
        Self::new(decompose(bounds, obstacles), y)
    }

    /// Whether any piece covers this position.
    pub fn is_walkable(&self, point: Vec3) -> bool {
        self.rects.iter().any(|r| r.contains(point))
    }

    /// The piece containing `point`, or the nearest one if it is off the mesh.
    ///
    /// Falling back to the nearest keeps an agent that has been nudged just
    /// outside the mesh — by a knockback impulse, say — able to path home
    /// instead of freezing with "no path".
    fn locate(&self, point: Vec3) -> Option<usize> {
        if let Some(i) = self.rects.iter().position(|r| r.contains(point)) {
            return Some(i);
        }
        self.rects
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let cx = point.x.clamp(r.min_x, r.max_x);
                let cz = point.z.clamp(r.min_z, r.max_z);
                let d = (point.x - cx).powi(2) + (point.z - cz).powi(2);
                (i, d)
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).expect("distances are finite"))
            .map(|(i, _)| i)
    }

    /// Finds a path from `from` to `to`, as waypoints excluding the start.
    ///
    /// A* over the piece graph gives the corridor to cross; the funnel then
    /// pulls the route taut inside it, so the result turns only where an
    /// obstacle actually forces it to. A uniform grid cannot do that — its
    /// waypoints sit on cell centres and its turns are limited to eight
    /// directions, which is what made grid paths stair-step across open space.
    pub fn find_path(&self, from: Vec3, to: Vec3) -> Option<Vec<Vec3>> {
        let start = self.locate(from)?;
        let goal = self.locate(to)?;
        if start == goal {
            return Some(vec![Vec3::new(to.x, self.y, to.z)]);
        }
        let corridor = self.astar(start, goal, from, to)?;
        Some(self.funnel(&corridor, from, to))
    }

    /// A* over piece centres, returning the sequence of piece indices.
    fn astar(&self, start: usize, goal: usize, from: Vec3, to: Vec3) -> Option<Vec<usize>> {
        use std::cmp::Ordering;
        use std::collections::BinaryHeap;

        /// Ordered by cost, smallest first — `BinaryHeap` is a max-heap.
        struct Node {
            cost: f32,
            index: usize,
        }
        impl PartialEq for Node {
            fn eq(&self, other: &Self) -> bool {
                self.cost == other.cost
            }
        }
        impl Eq for Node {}
        impl PartialOrd for Node {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }
        impl Ord for Node {
            fn cmp(&self, other: &Self) -> Ordering {
                other
                    .cost
                    .partial_cmp(&self.cost)
                    .unwrap_or(Ordering::Equal)
            }
        }

        let n = self.rects.len();
        let mut best = vec![f32::INFINITY; n];
        let mut came_from = vec![usize::MAX; n];
        let mut heap = BinaryHeap::new();

        best[start] = 0.0;
        heap.push(Node {
            cost: 0.0,
            index: start,
        });

        while let Some(Node { index, .. }) = heap.pop() {
            if index == goal {
                let mut path = vec![goal];
                let mut at = goal;
                while at != start {
                    at = came_from[at];
                    path.push(at);
                }
                path.reverse();
                return Some(path);
            }
            for &next in &self.neighbours[index] {
                let a = if index == start {
                    from
                } else {
                    self.rects[index].center(self.y)
                };
                let b = if next == goal {
                    to
                } else {
                    self.rects[next].center(self.y)
                };
                let step = ((a.x - b.x).powi(2) + (a.z - b.z).powi(2)).sqrt();
                let candidate = best[index] + step;
                if candidate < best[next] {
                    best[next] = candidate;
                    came_from[next] = index;
                    let goal_c = self.rects[goal].center(self.y);
                    let h = ((b.x - goal_c.x).powi(2) + (b.z - goal_c.z).powi(2)).sqrt();
                    heap.push(Node {
                        cost: candidate + h,
                        index: next,
                    });
                }
            }
        }
        None
    }

    /// Pulls a corridor taut — the "simple stupid funnel" algorithm.
    ///
    /// Walks the portals between consecutive pieces, narrowing a left/right
    /// wedge from the current corner. When the wedge would invert, the side
    /// that crossed becomes the next corner. The result is the shortest route
    /// through that sequence of portals.
    fn funnel(&self, corridor: &[usize], from: Vec3, to: Vec3) -> Vec<Vec3> {
        let mut portals: Vec<(Vec3, Vec3)> = vec![(from, from)];
        for pair in corridor.windows(2) {
            if let Some(p) = self.rects[pair[0]].portal_to(&self.rects[pair[1]], self.y) {
                portals.push(p);
            }
        }
        portals.push((to, to));

        // Positive when c is left of the line a->b, in the XZ plane.
        fn cross(a: Vec3, b: Vec3, c: Vec3) -> f32 {
            (b.x - a.x) * (c.z - a.z) - (b.z - a.z) * (c.x - a.x)
        }

        let mut out = Vec::new();
        let mut apex = portals[0].0;
        let (mut left, mut right) = (portals[0].0, portals[0].1);
        let (mut left_i, mut right_i) = (0usize, 0usize);

        let mut i = 1;
        while i < portals.len() {
            let (p_left, p_right) = portals[i];

            if cross(apex, right, p_right) <= 0.0 {
                if apex == right || cross(apex, left, p_right) > 0.0 {
                    right = p_right;
                    right_i = i;
                } else {
                    // The right side crossed the left: the left vertex is a corner.
                    out.push(left);
                    apex = left;
                    right = apex;
                    i = left_i + 1;
                    right_i = left_i;
                    continue;
                }
            }
            if cross(apex, left, p_left) >= 0.0 {
                if apex == left || cross(apex, right, p_left) < 0.0 {
                    left = p_left;
                    left_i = i;
                } else {
                    out.push(right);
                    apex = right;
                    left = apex;
                    i = right_i + 1;
                    left_i = right_i;
                    continue;
                }
            }
            i += 1;
        }
        // The loop can already have emitted the goal, when the last corner it
        // turned was the final portal — which collapses to the goal point
        // itself. Pushing again would hand the agent the same waypoint twice
        // and make it think it had one more step to walk than it does.
        let goal = Vec3::new(to.x, self.y, to.z);
        if out.last().map(|last| *last != goal).unwrap_or(true) {
            out.push(goal);
        }
        out
    }
}

/// Builds a navigation mesh from footprints taken off scene geometry.
///
/// `surfaces` are the walkable ground; `obstacles` are what stands on it. The
/// bounds are the union of the surfaces, so a level made of several floor
/// pieces still produces one mesh.
///
/// Footprints rather than colliders so this stays free of any physics
/// dependency: whoever has the colliders projects them, and this decides what
/// is walkable.
pub fn build_from_footprints(surfaces: &[Rect], obstacles: &[Rect], y: f32) -> NavPolys {
    let Some(first) = surfaces.first() else {
        return NavPolys::default();
    };
    let bounds = surfaces.iter().fold(*first, |acc, r| Rect {
        min_x: acc.min_x.min(r.min_x),
        max_x: acc.max_x.max(r.max_x),
        min_z: acc.min_z.min(r.min_z),
        max_z: acc.max_z.max(r.max_z),
    });

    // Anything the surfaces do not cover is as unwalkable as an obstacle. A
    // union of two floor slabs that do not meet leaves a hole between them, and
    // an agent must not path across it.
    let mut blocking = obstacles.to_vec();
    for strip in decompose(bounds, surfaces) {
        blocking.push(strip);
    }
    NavPolys::build(bounds, &blocking, y)
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
    fn crossing_an_open_room_is_a_straight_line() {
        // The grid's signature artifact: with 8-directional movement between
        // cell centres, an unobstructed diagonal came out as a staircase of
        // waypoints.
        //
        // The obstacles are off in a corner, well clear of the route, but they
        // split the room into several pieces so the path genuinely crosses
        // portals. Without that the whole room is one piece, `find_path`
        // short-circuits, and this would pass without the funnel ever running —
        // which is what a first version of this test did.
        let polys = NavPolys::build(
            Rect::new(0.0, 20.0, 0.0, 20.0),
            &[
                Rect::new(2.0, 4.0, 16.0, 18.0),
                Rect::new(16.0, 18.0, 2.0, 4.0),
            ],
            0.0,
        );
        assert!(polys.rects.len() > 3, "the room must really be subdivided");

        let path = polys
            .find_path(Vec3::new(1.0, 0.0, 1.0), Vec3::new(19.0, 0.0, 19.0))
            .expect("the diagonal is clear");
        assert_eq!(
            path.len(),
            1,
            "nothing blocks the diagonal, so there is nothing to turn around: {path:?}"
        );
        assert_eq!((path[0].x, path[0].z), (19.0, 19.0));
    }

    #[test]
    fn a_path_around_a_pillar_turns_only_at_its_corner() {
        // Taut, not stair-stepped: one corner to round, so one intermediate
        // waypoint, and it sits on the obstacle's corner rather than on some
        // cell centre nearby.
        let polys = NavPolys::build(
            Rect::new(0.0, 20.0, 0.0, 20.0),
            &[Rect::new(8.0, 12.0, 0.0, 12.0)],
            0.0,
        );
        let path = polys
            .find_path(Vec3::new(4.0, 0.0, 4.0), Vec3::new(16.0, 0.0, 4.0))
            .expect("there is a way round the pillar");

        // The pillar runs from the south wall up to z = 12, so the way past is
        // around its top. Taut, that is exactly two corners — the pillar's two
        // top vertices — and then a straight run to the goal. A grid would
        // emit a waypoint per cell along the same route.
        assert_eq!(path.len(), 3, "two corners and the goal: {path:?}");
        assert_eq!((path[0].x, path[0].z), (8.0, 12.0), "near top corner");
        assert_eq!((path[1].x, path[1].z), (12.0, 12.0), "far top corner");
        assert_eq!(
            (path[2].x, path[2].z),
            (16.0, 4.0),
            "then straight to the goal"
        );
    }

    #[test]
    fn a_path_through_a_narrow_gap_uses_it() {
        // 0.3 units wide — narrower than any cell a grid would use for a
        // 20-unit room, so the grid either loses the gap or has to be built at
        // a resolution that makes the whole level expensive.
        let polys = NavPolys::build(
            Rect::new(0.0, 20.0, 0.0, 20.0),
            &[
                Rect::new(9.0, 11.0, 0.0, 9.85),
                Rect::new(9.0, 11.0, 10.15, 20.0),
            ],
            0.0,
        );
        let path = polys
            .find_path(Vec3::new(2.0, 0.0, 10.0), Vec3::new(18.0, 0.0, 10.0))
            .expect("the gap is walkable, so a path exists");
        assert!(
            path.iter().all(|p| polys.is_walkable(*p)),
            "every waypoint must be on the mesh: {path:?}"
        );
    }

    #[test]
    fn a_fully_walled_off_goal_has_no_path() {
        // The negative: a wall with no gap must not produce a path, or the
        // agent walks through it.
        let polys = NavPolys::build(
            Rect::new(0.0, 20.0, 0.0, 20.0),
            &[Rect::new(9.0, 11.0, 0.0, 20.0)],
            0.0,
        );
        assert!(polys
            .find_path(Vec3::new(2.0, 0.0, 10.0), Vec3::new(18.0, 0.0, 10.0))
            .is_none());
    }

    #[test]
    fn a_navmesh_built_from_one_floor_slab_covers_it() {
        let polys = build_from_footprints(&[Rect::new(0.0, 10.0, 0.0, 10.0)], &[], 0.0);
        assert!(polys.is_walkable(Vec3::new(5.0, 0.0, 5.0)));
        assert!(!polys.is_walkable(Vec3::new(15.0, 0.0, 5.0)));
    }

    #[test]
    fn an_obstacle_on_the_floor_is_not_walkable() {
        let polys = build_from_footprints(
            &[Rect::new(0.0, 10.0, 0.0, 10.0)],
            &[Rect::new(4.0, 6.0, 4.0, 6.0)],
            0.0,
        );
        assert!(
            !polys.is_walkable(Vec3::new(5.0, 0.0, 5.0)),
            "inside the obstacle"
        );
        assert!(polys.is_walkable(Vec3::new(1.0, 0.0, 1.0)), "beside it");
    }

    #[test]
    fn a_gap_between_two_floor_slabs_is_not_walkable() {
        // The bounds are the union of the slabs, so without subtracting what
        // the slabs do not cover, the hole between them would silently become
        // walkable and agents would path across thin air.
        let polys = build_from_footprints(
            &[
                Rect::new(0.0, 4.0, 0.0, 10.0),
                Rect::new(6.0, 10.0, 0.0, 10.0),
            ],
            &[],
            0.0,
        );
        assert!(polys.is_walkable(Vec3::new(2.0, 0.0, 5.0)), "left slab");
        assert!(polys.is_walkable(Vec3::new(8.0, 0.0, 5.0)), "right slab");
        assert!(
            !polys.is_walkable(Vec3::new(5.0, 0.0, 5.0)),
            "the gap between them is not floor"
        );
        assert!(
            polys
                .find_path(Vec3::new(2.0, 0.0, 5.0), Vec3::new(8.0, 0.0, 5.0))
                .is_none(),
            "and nothing may path across it"
        );
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
