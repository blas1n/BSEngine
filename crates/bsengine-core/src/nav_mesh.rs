use std::sync::atomic::{AtomicU64, Ordering};

use bevy_ecs::prelude::Resource;
use glam::Vec3;

use crate::nav_poly::{build_from_footprints_for_agent, NavPolys, Rect};

/// Source of [`NavMesh::generation`]. Process-wide rather than per mesh so a
/// mesh that *replaces* another as the resource -- a script's `navmesh.init`,
/// a bake -- still reads as a different generation to anyone holding a path
/// computed on the old one.
static GENERATION: AtomicU64 = AtomicU64::new(1);

fn next_generation() -> u64 {
    GENERATION.fetch_add(1, Ordering::Relaxed)
}

/// Navigation mesh for pathfinding on the XZ plane.
///
/// Two ways to author it, both ending in the same convex decomposition:
///
/// * a uniform grid ([`NavMesh::new`] + [`set_walkable`](NavMesh::set_walkable)),
///   the original scripting surface, where blocked cells become obstacles; or
/// * a bake from the level's static collision geometry
///   ([`NavMesh::bake_from_aabbs`]), which has no grid at all -- `width` and
///   `depth` are 0 and endpoint queries go straight to the polygons.
#[derive(Resource, Debug, Clone)]
pub struct NavMesh {
    /// Number of cells along the X axis. Zero for a baked mesh.
    pub width: u32,
    /// Number of cells along the Z axis. Zero for a baked mesh.
    pub depth: u32,
    /// World-space size of one grid cell, along both axes.
    pub cell_size: f32,
    /// World-space position of the grid's cell (0, 0) corner. For a baked
    /// mesh only its `y` is meaningful: the plane the mesh lies on.
    pub origin: Vec3,
    walkable: Vec<bool>,
    /// The convex decomposition the search actually runs on.
    polys: NavPolys,
    /// See [`generation`](NavMesh::generation).
    generation: u64,
}

impl Default for NavMesh {
    fn default() -> Self {
        Self::new(0, 0, 1.0, Vec3::ZERO)
    }
}

/// What a bake needs to know about the agents that will walk its result.
///
/// The same three numbers every reference engine asks for on its bake
/// settings (Unity's agent type, Unreal's agent radius/height/step, Godot's
/// `NavigationMesh` agent properties), plus where the floor is -- which those
/// engines find by slope analysis over arbitrary geometry, and this planar
/// mesh finds by picking one plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NavBakeParams {
    /// Half the width of the agents. Everything they must keep clear of is
    /// grown by this, and the walkable area shrinks by it at its edges.
    pub agent_radius: f32,
    /// How tall the agents are. Geometry whose underside is at least this
    /// far above the floor is a ceiling, not an obstacle.
    pub agent_height: f32,
    /// The highest rise an agent simply steps over. Geometry that does not
    /// protrude above the floor by more than this is ground, not a wall.
    pub step_height: f32,
    /// The plane to bake, as the height of the walkable floor. `None` picks
    /// the top face of the static collider with the largest footprint -- in
    /// every level under `games/`, the ground.
    pub floor_y: Option<f32>,
}

impl Default for NavBakeParams {
    fn default() -> Self {
        // `NavMeshAgent`'s own radius/height defaults, and Unity's default
        // step height.
        Self {
            agent_radius: 0.4,
            agent_height: 1.8,
            step_height: 0.4,
            floor_y: None,
        }
    }
}

impl NavMesh {
    /// Creates a `width` x `depth` grid of the given cell size at `origin`, all cells walkable.
    pub fn new(width: u32, depth: u32, cell_size: f32, origin: Vec3) -> Self {
        let total = (width as usize).saturating_mul(depth as usize);
        let mut mesh = Self {
            width,
            depth,
            cell_size: cell_size.max(f32::EPSILON),
            origin,
            walkable: vec![true; total],
            polys: NavPolys::default(),
            generation: next_generation(),
        };
        mesh.rebuild();
        mesh
    }

    /// Bakes a mesh from the world-space bounding boxes of a level's static
    /// colliders, as `(min, max)` corners.
    ///
    /// Sorting the boxes is the whole job, and it is done against one plane:
    ///
    /// * a box whose top face is within `step_height` of the floor is a piece
    ///   of *floor* (the ground itself, a slightly raised slab, a low curb);
    /// * a box that protrudes above the floor by more than `step_height` and
    ///   whose underside is below `agent_height` is an *obstacle* (a wall, a
    ///   pillar, a table); an agent walks neither through nor over it;
    /// * everything else is ignored -- a ceiling above head height, a pit
    ///   below the floor. Note a raised platform counts as an obstacle: this
    ///   mesh is planar, and a second walkable level is what item 26 left out
    ///   of scope, deliberately.
    ///
    /// The result has no grid: `width`/`depth` are 0, and
    /// [`find_path`](NavMesh::find_path) snaps an endpoint that is off the
    /// mesh to the nearest walkable point, the way Unity's `SetDestination`
    /// samples the navmesh rather than refusing. A grid mesh keeps its
    /// stricter "you cannot path out of a wall" rule; see `find_path`.
    ///
    /// Takes plain boxes rather than colliders so this crate stays free of
    /// any physics dependency: `bsengine_physics::PhysicsWorld` projects its
    /// colliders, and this decides what is walkable.
    pub fn bake_from_aabbs(aabbs: &[(Vec3, Vec3)], params: &NavBakeParams) -> Self {
        let mut mesh = Self {
            width: 0,
            depth: 0,
            cell_size: 1.0,
            origin: Vec3::ZERO,
            walkable: Vec::new(),
            polys: NavPolys::default(),
            generation: next_generation(),
        };
        let Some(floor_y) = params.floor_y.or_else(|| {
            aabbs
                .iter()
                .max_by(|a, b| {
                    let area = |(min, max): &&(Vec3, Vec3)| (max.x - min.x) * (max.z - min.z);
                    area(a)
                        .partial_cmp(&area(b))
                        .expect("collider bounds are finite")
                })
                .map(|(_, max)| max.y)
        }) else {
            return mesh;
        };
        mesh.origin.y = floor_y;

        let step = params.step_height.max(0.0);
        let head = floor_y + params.agent_height.max(0.0);
        let mut surfaces = Vec::new();
        let mut obstacles = Vec::new();
        for (min, max) in aabbs {
            let footprint = Rect::new(min.x, max.x, min.z, max.z);
            if footprint.is_empty() {
                continue;
            }
            if (max.y - floor_y).abs() <= step {
                surfaces.push(footprint);
            } else if max.y > floor_y + step && min.y < head {
                obstacles.push(footprint);
            }
        }
        mesh.polys =
            build_from_footprints_for_agent(&surfaces, &obstacles, floor_y, params.agent_radius);
        mesh
    }

    /// Changes every time the walkable area does: on every rebuild of a grid
    /// mesh and for every freshly baked or constructed one.
    ///
    /// A path is only valid for the mesh it was computed on. The agent system
    /// caches paths per destination, and before this existed a mesh swapped
    /// under it -- by a script's `navmesh.init`, now by a re-bake -- left every
    /// agent walking its old route until its destination happened to change.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Whether the walkable area covers this position (Y ignored).
    ///
    /// The polygon answer, so it means the same for a grid mesh and a baked
    /// one; [`is_walkable`](NavMesh::is_walkable) is the grid cell's answer
    /// and is always `false` on a baked mesh, which has no cells.
    pub fn is_point_walkable(&self, point: Vec3) -> bool {
        self.polys.is_walkable(point)
    }

    /// Marks a grid cell as walkable or blocked. Out-of-bounds coordinates are ignored.
    pub fn set_walkable(&mut self, x: u32, z: u32, walkable: bool) {
        if x < self.width && z < self.depth {
            self.walkable[(z * self.width + x) as usize] = walkable;
            self.rebuild();
        }
    }

    /// Returns whether the given cell is walkable; out-of-bounds coordinates are never walkable.
    pub fn is_walkable(&self, x: i32, z: i32) -> bool {
        if x < 0 || z < 0 || x as u32 >= self.width || z as u32 >= self.depth {
            return false;
        }
        self.walkable[(z as u32 * self.width + x as u32) as usize]
    }

    /// Converts a world-space position to its containing grid cell coordinates.
    pub fn world_to_cell(&self, pos: Vec3) -> (i32, i32) {
        let dx = pos.x - self.origin.x;
        let dz = pos.z - self.origin.z;
        (
            (dx / self.cell_size).floor() as i32,
            (dz / self.cell_size).floor() as i32,
        )
    }

    /// Returns the world-space center of the given grid cell.
    pub fn cell_center(&self, x: i32, z: i32) -> Vec3 {
        Vec3::new(
            self.origin.x + x as f32 * self.cell_size + self.cell_size * 0.5,
            self.origin.y,
            self.origin.z + z as f32 * self.cell_size + self.cell_size * 0.5,
        )
    }

    /// Finds a path from `from` to `to` in world space.
    ///
    /// Returns waypoints excluding the start and ending at `to`, or `None`
    /// when either endpoint is on blocked ground or no route exists.
    ///
    /// The search itself runs on the convex decomposition, not the grid: the
    /// cells are the *authoring* surface and the polygons are what is walked.
    /// That is why a path across open ground now comes back as a single
    /// waypoint instead of a staircase of cell centres.
    pub fn find_path(&self, from: Vec3, to: Vec3) -> Option<Vec<Vec3>> {
        if self.width == 0 || self.depth == 0 {
            // No grid: a baked mesh, or an empty one (whose polygons are
            // empty too, so this is `None`). Endpoints are the polygon
            // locator's business, which snaps an off-mesh point to the
            // nearest piece -- what a baked level wants, since a destination
            // a hair past the eroded edge is still a place to walk toward.
            return self.polys.find_path(from, to);
        }
        // Endpoint walkability is still decided by the grid, so "you cannot
        // path out of a wall" keeps meaning exactly what it did. The polygon
        // locator deliberately snaps an off-mesh point to the nearest piece,
        // which is right for an agent knocked out of bounds mid-game and wrong
        // as an answer to "is this square blocked".
        let (sx, sz) = self.world_to_cell(from);
        let (ex, ez) = self.world_to_cell(to);
        if !self.is_walkable(sx, sz) || !self.is_walkable(ex, ez) {
            return None;
        }
        self.polys.find_path(from, to)
    }

    /// Rebuilds the convex decomposition from the current grid.
    ///
    /// Every blocked cell becomes an obstacle rectangle; the sweep merges runs
    /// of them, so a wall of fifty cells costs the same as one wall.
    fn rebuild(&mut self) {
        self.generation = next_generation();
        if self.width == 0 || self.depth == 0 {
            self.polys = NavPolys::default();
            return;
        }
        let bounds = Rect::new(
            self.origin.x,
            self.origin.x + self.width as f32 * self.cell_size,
            self.origin.z,
            self.origin.z + self.depth as f32 * self.cell_size,
        );
        let mut obstacles = Vec::new();
        for z in 0..self.depth {
            for x in 0..self.width {
                if !self.walkable[(z * self.width + x) as usize] {
                    let x0 = self.origin.x + x as f32 * self.cell_size;
                    let z0 = self.origin.z + z as f32 * self.cell_size;
                    obstacles.push(Rect::new(x0, x0 + self.cell_size, z0, z0 + self.cell_size));
                }
            }
        }
        self.polys = NavPolys::build(bounds, &obstacles, self.origin.y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(w: u32, d: u32) -> NavMesh {
        NavMesh::new(w, d, 1.0, Vec3::ZERO)
    }

    #[test]
    fn all_cells_walkable_by_default() {
        let nm = flat(4, 4);
        for x in 0..4i32 {
            for z in 0..4i32 {
                assert!(nm.is_walkable(x, z));
            }
        }
    }

    #[test]
    fn out_of_bounds_not_walkable() {
        let nm = flat(4, 4);
        assert!(!nm.is_walkable(-1, 0));
        assert!(!nm.is_walkable(4, 0));
        assert!(!nm.is_walkable(0, -1));
        assert!(!nm.is_walkable(0, 4));
    }

    #[test]
    fn set_walkable_false_blocks_cell() {
        let mut nm = flat(4, 4);
        nm.set_walkable(2, 2, false);
        assert!(!nm.is_walkable(2, 2));
        assert!(nm.is_walkable(1, 1));
    }

    #[test]
    fn world_to_cell_maps_correctly() {
        let nm = flat(10, 10);
        assert_eq!(nm.world_to_cell(Vec3::new(1.5, 0.0, 2.9)), (1, 2));
        assert_eq!(nm.world_to_cell(Vec3::new(0.0, 0.0, 0.0)), (0, 0));
        assert_eq!(nm.world_to_cell(Vec3::new(9.99, 0.0, 9.99)), (9, 9));
    }

    #[test]
    fn find_path_open_grid() {
        let nm = flat(10, 10);
        let path = nm
            .find_path(Vec3::new(0.5, 0.0, 0.5), Vec3::new(5.5, 0.0, 0.5))
            .expect("path must exist on open grid");
        assert!(!path.is_empty());
        let last = *path.last().unwrap();
        assert!((last.x - 5.5).abs() < 0.01, "last waypoint x ≈ 5.5");
        assert!((last.z - 0.5).abs() < 0.01, "last waypoint z ≈ 0.5");
    }

    #[test]
    fn find_path_around_partial_wall() {
        let mut nm = flat(10, 10);
        for z in 0..5u32 {
            nm.set_walkable(4, z, false);
        }
        let path = nm.find_path(Vec3::new(0.5, 0.0, 0.5), Vec3::new(7.5, 0.0, 0.5));
        assert!(path.is_some(), "should route around partial wall");
    }

    #[test]
    fn find_path_none_through_full_wall() {
        let mut nm = flat(10, 10);
        for z in 0..10u32 {
            nm.set_walkable(4, z, false);
        }
        let path = nm.find_path(Vec3::new(0.5, 0.0, 0.5), Vec3::new(7.5, 0.0, 0.5));
        assert!(path.is_none(), "full wall blocks all paths");
    }

    #[test]
    fn same_cell_returns_exact_destination() {
        let nm = flat(10, 10);
        let dest = Vec3::new(0.7, 0.0, 0.3);
        let path = nm
            .find_path(Vec3::new(0.1, 0.0, 0.2), dest)
            .expect("same-cell path");
        assert_eq!(path.len(), 1);
        assert_eq!(path[0], dest);
    }

    #[test]
    fn empty_grid_returns_none() {
        let nm = NavMesh::new(0, 0, 1.0, Vec3::ZERO);
        assert!(nm.find_path(Vec3::ZERO, Vec3::X).is_none());
    }

    #[test]
    fn default_is_empty_grid() {
        let nm = NavMesh::default();
        assert_eq!(nm.width, 0);
        assert_eq!(nm.depth, 0);
    }

    // ---- baking from static collider boxes ----

    /// A 20x20 floor slab whose top face is at y = 0.
    fn floor() -> (Vec3, Vec3) {
        (Vec3::new(-10.0, -1.0, -10.0), Vec3::new(10.0, 0.0, 10.0))
    }

    /// The smallest distance from any point of the polyline `path` (starting
    /// at `from`) to the rectangle `rect`, sampled finely along each segment.
    fn path_clearance(from: Vec3, path: &[Vec3], rect: &Rect) -> f32 {
        let mut prev = from;
        let mut best = f32::INFINITY;
        for &wp in path {
            for i in 0..=200 {
                let t = i as f32 / 200.0;
                let p = prev.lerp(wp, t);
                let cx = p.x.clamp(rect.min_x, rect.max_x);
                let cz = p.z.clamp(rect.min_z, rect.max_z);
                best = best.min(((p.x - cx).powi(2) + (p.z - cz).powi(2)).sqrt());
            }
            prev = wp;
        }
        best
    }

    /// The property the radius exists for: the path an agent is handed keeps
    /// its centre a full radius away from the obstacle it goes around. With
    /// radius 0 the funnel pulls the route taut against the pillar's corner
    /// (clearance 0), which is exactly the clipping the erosion prevents.
    #[test]
    fn a_baked_path_keeps_the_agent_radius_clear_of_obstacles() {
        let pillar = (Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 2.0, 1.0));
        let pillar_rect = Rect::new(-1.0, 1.0, -1.0, 1.0);
        let params = NavBakeParams {
            agent_radius: 0.5,
            ..Default::default()
        };
        let mesh = NavMesh::bake_from_aabbs(&[floor(), pillar], &params);

        // Premise: the straight line from start to goal runs through the
        // pillar, so any path found had to go around it.
        assert!(
            !mesh.is_point_walkable(Vec3::ZERO),
            "the pillar blocks its footprint"
        );
        let from = Vec3::new(-5.0, 0.0, 0.0);
        let to = Vec3::new(5.0, 0.0, 0.0);
        let path = mesh
            .find_path(from, to)
            .expect("a route around the pillar exists");
        assert_eq!(*path.last().unwrap(), Vec3::new(5.0, 0.0, 0.0));

        let clearance = path_clearance(from, &path, &pillar_rect);
        assert!(
            clearance < 1.5,
            "premise: the detour hugs the pillar rather than wandering off; clearance {clearance}"
        );
        assert!(
            clearance >= 0.5 - 1e-3,
            "the path must stay one agent radius (0.5) from the pillar, got {clearance}"
        );

        // And the same level at radius 0 does clip the corner -- the erosion
        // is what made the difference above, not the decomposition.
        let sharp = NavMesh::bake_from_aabbs(
            &[floor(), pillar],
            &NavBakeParams {
                agent_radius: 0.0,
                ..params
            },
        );
        let sharp_path = sharp.find_path(from, to).unwrap();
        assert!(
            path_clearance(from, &sharp_path, &pillar_rect) < 1e-3,
            "premise: with no radius the funnel touches the corner"
        );
    }

    /// Step height is what separates ground from wall: a curb lower than it is
    /// walked over, the same slab made taller is a wall. Both are static
    /// colliders standing on the floor; only their height differs.
    #[test]
    fn a_curb_below_step_height_is_ground_and_a_wall_above_it_blocks() {
        let params = NavBakeParams {
            step_height: 0.4,
            ..Default::default()
        };
        let from = Vec3::new(-5.0, 0.0, 0.0);
        let to = Vec3::new(5.0, 0.0, 0.0);

        let curb = (Vec3::new(-1.0, 0.0, -10.0), Vec3::new(1.0, 0.2, 10.0));
        let with_curb = NavMesh::bake_from_aabbs(&[floor(), curb], &params);
        assert!(
            with_curb.is_point_walkable(Vec3::ZERO),
            "a 0.2 rise is stepped over, so the curb's footprint stays walkable"
        );
        assert!(with_curb.find_path(from, to).is_some());

        let wall = (Vec3::new(-1.0, 0.0, -10.0), Vec3::new(1.0, 1.0, 10.0));
        let with_wall = NavMesh::bake_from_aabbs(&[floor(), wall], &params);
        assert!(
            !with_wall.is_point_walkable(Vec3::ZERO),
            "a 1.0 rise is a wall"
        );
        assert!(
            with_wall.find_path(from, to).is_none(),
            "the wall spans the whole floor, so there is no way around it"
        );
    }

    /// Agent height is what separates a ceiling from an obstacle: the same
    /// slab spanning the floor is walked under when its underside clears the
    /// agent's head, and blocks when the agent is taller than the gap.
    #[test]
    fn geometry_above_agent_height_is_a_ceiling_not_an_obstacle() {
        let bridge = (Vec3::new(-1.0, 2.0, -10.0), Vec3::new(1.0, 2.5, 10.0));
        let from = Vec3::new(-5.0, 0.0, 0.0);
        let to = Vec3::new(5.0, 0.0, 0.0);

        let short_agent = NavMesh::bake_from_aabbs(
            &[floor(), bridge],
            &NavBakeParams {
                agent_height: 1.8,
                ..Default::default()
            },
        );
        assert!(
            short_agent.is_point_walkable(Vec3::ZERO),
            "2.0 clears a 1.8 agent"
        );
        assert!(short_agent.find_path(from, to).is_some());

        let tall_agent = NavMesh::bake_from_aabbs(
            &[floor(), bridge],
            &NavBakeParams {
                agent_height: 3.0,
                ..Default::default()
            },
        );
        assert!(
            !tall_agent.is_point_walkable(Vec3::ZERO),
            "premise: the same slab is an obstacle to a 3.0 agent, so the height was consulted"
        );
        assert!(tall_agent.find_path(from, to).is_none());
    }

    /// With no explicit floor, the plane is the top of the largest footprint.
    /// The table is listed first so that "take the first box" -- the obvious
    /// wrong rule -- picks the wrong plane and fails this test.
    #[test]
    fn the_floor_is_the_largest_footprint_unless_told_otherwise() {
        let table = (Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
        let boxes = [table, floor()];

        let auto = NavMesh::bake_from_aabbs(&boxes, &NavBakeParams::default());
        assert_eq!(auto.origin.y, 0.0, "the floor's top face, not the table's");
        assert!(
            auto.is_point_walkable(Vec3::new(5.0, 0.0, 5.0)),
            "on the floor"
        );
        assert!(
            !auto.is_point_walkable(Vec3::ZERO),
            "the table stands on it"
        );

        let explicit = NavMesh::bake_from_aabbs(
            &boxes,
            &NavBakeParams {
                floor_y: Some(1.0),
                ..Default::default()
            },
        );
        assert_eq!(explicit.origin.y, 1.0);
        assert!(
            explicit.is_point_walkable(Vec3::ZERO),
            "at floor_y = 1 the table top is the (only) floor"
        );
        assert!(
            !explicit.is_point_walkable(Vec3::new(5.0, 0.0, 5.0)),
            "and the ground, a metre below it, is not"
        );
    }

    /// A baked mesh has no grid, and `find_path` used to refuse any mesh
    /// without one. It must route on the polygons instead -- and, unlike a
    /// grid mesh, snap an endpoint just past the eroded edge onto the mesh.
    #[test]
    fn a_baked_mesh_has_no_grid_and_paths_on_its_polygons() {
        let mesh = NavMesh::bake_from_aabbs(&[floor()], &NavBakeParams::default());
        assert_eq!((mesh.width, mesh.depth), (0, 0));
        assert!(!mesh.is_walkable(0, 0), "there are no cells to be walkable");

        let path = mesh
            .find_path(Vec3::new(-5.0, 0.0, -5.0), Vec3::new(5.0, 0.0, 5.0))
            .expect("open floor");
        assert_eq!(path, vec![Vec3::new(5.0, 0.0, 5.0)], "one straight hop");

        // 9.8 is inside the eroded edge (10 - 0.4 = 9.6)? No: it is past it.
        let past_edge = Vec3::new(9.8, 0.0, 0.0);
        assert!(
            !mesh.is_point_walkable(past_edge),
            "premise: outside the eroded mesh"
        );
        assert!(
            mesh.find_path(Vec3::ZERO, past_edge).is_some(),
            "an off-mesh destination is snapped, not refused"
        );
    }

    /// Every change to the walkable area must be visible as a new generation,
    /// because that is the only thing that tells a cached path it is stale.
    #[test]
    fn every_rebuild_and_bake_is_a_new_generation() {
        let mut grid = NavMesh::new(4, 4, 1.0, Vec3::ZERO);
        let g0 = grid.generation();
        grid.set_walkable(1, 1, false);
        let g1 = grid.generation();
        assert!(g1 > g0, "blocking a cell changed the mesh");

        let baked = NavMesh::bake_from_aabbs(&[floor()], &NavBakeParams::default());
        assert!(
            baked.generation() > g1,
            "a fresh bake is newer than anything before it"
        );

        let replacement = NavMesh::new(4, 4, 1.0, Vec3::ZERO);
        assert_ne!(
            replacement.generation(),
            g0,
            "an identical grid built later is still a different mesh to a cached path"
        );
    }
}
