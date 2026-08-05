use bevy_ecs::prelude::Resource;
use glam::Vec3;

use crate::nav_poly::{NavPolys, Rect};

/// Uniform-grid navigation mesh for A* pathfinding. Cells lie in the XZ plane.
#[derive(Resource, Debug, Clone)]
pub struct NavMesh {
    /// Number of cells along the X axis.
    pub width: u32,
    /// Number of cells along the Z axis.
    pub depth: u32,
    /// World-space size of one grid cell, along both axes.
    pub cell_size: f32,
    /// World-space position of the grid's cell (0, 0) corner.
    pub origin: Vec3,
    walkable: Vec<bool>,
    /// The convex decomposition the search actually runs on.
    polys: NavPolys,
}

impl Default for NavMesh {
    fn default() -> Self {
        Self::new(0, 0, 1.0, Vec3::ZERO)
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
        };
        mesh.rebuild();
        mesh
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
            return None;
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
}
