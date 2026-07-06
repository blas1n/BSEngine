use std::collections::HashMap;

use bevy_ecs::prelude::Resource;
use glam::Vec3;

/// Uniform-grid navigation mesh for A* pathfinding. Cells lie in the XZ plane.
#[derive(Resource, Debug, Clone)]
pub struct NavMesh {
    pub width: u32,
    pub depth: u32,
    pub cell_size: f32,
    pub origin: Vec3,
    walkable: Vec<bool>,
}

impl Default for NavMesh {
    fn default() -> Self {
        Self::new(0, 0, 1.0, Vec3::ZERO)
    }
}

impl NavMesh {
    pub fn new(width: u32, depth: u32, cell_size: f32, origin: Vec3) -> Self {
        let total = (width as usize).saturating_mul(depth as usize);
        Self {
            width,
            depth,
            cell_size: cell_size.max(f32::EPSILON),
            origin,
            walkable: vec![true; total],
        }
    }

    pub fn set_walkable(&mut self, x: u32, z: u32, walkable: bool) {
        if x < self.width && z < self.depth {
            self.walkable[(z * self.width + x) as usize] = walkable;
        }
    }

    pub fn is_walkable(&self, x: i32, z: i32) -> bool {
        if x < 0 || z < 0 || x as u32 >= self.width || z as u32 >= self.depth {
            return false;
        }
        self.walkable[(z as u32 * self.width + x as u32) as usize]
    }

    pub fn world_to_cell(&self, pos: Vec3) -> (i32, i32) {
        let dx = pos.x - self.origin.x;
        let dz = pos.z - self.origin.z;
        (
            (dx / self.cell_size).floor() as i32,
            (dz / self.cell_size).floor() as i32,
        )
    }

    pub fn cell_center(&self, x: i32, z: i32) -> Vec3 {
        Vec3::new(
            self.origin.x + x as f32 * self.cell_size + self.cell_size * 0.5,
            self.origin.y,
            self.origin.z + z as f32 * self.cell_size + self.cell_size * 0.5,
        )
    }

    /// A* pathfinding from `from` to `to` in world space.
    /// Returns waypoints excluding the start, ending at `to`. None if no path exists.
    pub fn find_path(&self, from: Vec3, to: Vec3) -> Option<Vec<Vec3>> {
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;

        if self.width == 0 || self.depth == 0 {
            return None;
        }

        let (sx, sz) = self.world_to_cell(from);
        let (ex, ez) = self.world_to_cell(to);

        if !self.is_walkable(sx, sz) || !self.is_walkable(ex, ez) {
            return None;
        }

        if sx == ex && sz == ez {
            return Some(vec![to]);
        }

        let heuristic = |x: i32, z: i32| -> u32 {
            let dx = (x - ex).unsigned_abs();
            let dz = (z - ez).unsigned_abs();
            10 * dx.max(dz)
        };

        let mut open: BinaryHeap<Reverse<(u32, i32, i32)>> = BinaryHeap::new();
        let mut g_score: HashMap<(i32, i32), u32> = HashMap::new();
        let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();

        open.push(Reverse((0, sx, sz)));
        g_score.insert((sx, sz), 0);

        while let Some(Reverse((_, x, z))) = open.pop() {
            if x == ex && z == ez {
                let mut cells = vec![(ex, ez)];
                let mut cur = (ex, ez);
                while let Some(&prev) = came_from.get(&cur) {
                    cells.push(prev);
                    cur = prev;
                }
                cells.reverse();
                let waypoints = cells
                    .into_iter()
                    .skip(1)
                    .map(|(px, pz)| {
                        if px == ex && pz == ez {
                            to
                        } else {
                            self.cell_center(px, pz)
                        }
                    })
                    .collect();
                return Some(waypoints);
            }

            let g = *g_score.get(&(x, z)).unwrap_or(&u32::MAX);
            let neighbors = [
                (x - 1, z, 10u32),
                (x + 1, z, 10),
                (x, z - 1, 10),
                (x, z + 1, 10),
                (x - 1, z - 1, 14),
                (x + 1, z - 1, 14),
                (x - 1, z + 1, 14),
                (x + 1, z + 1, 14),
            ];
            for (nx, nz, step_cost) in neighbors {
                if !self.is_walkable(nx, nz) {
                    continue;
                }
                let new_g = g.saturating_add(step_cost);
                if new_g < *g_score.get(&(nx, nz)).unwrap_or(&u32::MAX) {
                    g_score.insert((nx, nz), new_g);
                    came_from.insert((nx, nz), (x, z));
                    open.push(Reverse((new_g + heuristic(nx, nz), nx, nz)));
                }
            }
        }

        None
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
