use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Snaps the entity's translation to a 3D grid each frame.
/// The transform system rounds the position to the nearest multiple of each
/// cell size axis. Useful for tile-based editors and RTS grid placement.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct GridSnap {
    /// Grid cell size per axis. Use `Vec3::splat(1.0)` for a uniform 1-unit grid.
    /// Each component must be > 0; zero is treated as "no snapping on that axis".
    pub cell_size: Vec3,
    /// World-space origin offset so the grid doesn't have to start at (0,0,0).
    pub offset: Vec3,
    pub enabled: bool,
}

impl GridSnap {
    pub fn new(cell_size: Vec3) -> Self {
        Self {
            cell_size: cell_size.max(Vec3::ZERO),
            offset: Vec3::ZERO,
            enabled: true,
        }
    }

    /// Uniform grid: same cell size on all three axes.
    pub fn uniform(size: f32) -> Self {
        Self::new(Vec3::splat(size.max(0.0)))
    }

    pub fn with_offset(mut self, offset: Vec3) -> Self {
        self.offset = offset;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Snap a position to this grid.
    /// Axes where `cell_size == 0` pass through unchanged.
    pub fn snap(&self, position: Vec3) -> Vec3 {
        let p = position - self.offset;
        Vec3::new(
            snap_axis(p.x, self.cell_size.x),
            snap_axis(p.y, self.cell_size.y),
            snap_axis(p.z, self.cell_size.z),
        ) + self.offset
    }
}

fn snap_axis(value: f32, cell: f32) -> f32 {
    if cell <= 0.0 {
        value
    } else {
        (value / cell).round() * cell
    }
}

impl Default for GridSnap {
    fn default() -> Self {
        Self::uniform(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_snap_defaults() {
        let gs = GridSnap::default();
        assert_eq!(gs.cell_size, Vec3::ONE);
        assert_eq!(gs.offset, Vec3::ZERO);
        assert!(gs.enabled);
    }

    #[test]
    fn snap_rounds_to_nearest_cell() {
        let gs = GridSnap::uniform(2.0);
        let snapped = gs.snap(Vec3::new(1.3, 3.7, -1.1));
        assert!((snapped.x - 2.0).abs() < 0.001);
        assert!((snapped.y - 4.0).abs() < 0.001);
        assert!((snapped.z - -2.0).abs() < 0.001);
    }

    #[test]
    fn zero_cell_size_passes_through() {
        let gs = GridSnap::new(Vec3::new(1.0, 0.0, 1.0));
        let pos = Vec3::new(0.5, 99.7, 0.5);
        let snapped = gs.snap(pos);
        assert!((snapped.y - 99.7).abs() < 0.001);
    }

    #[test]
    fn offset_shifts_grid() {
        let gs = GridSnap::uniform(1.0).with_offset(Vec3::new(0.5, 0.0, 0.0));
        let snapped = gs.snap(Vec3::new(1.0, 0.0, 0.0));
        assert!((snapped.x - 1.5).abs() < 0.001);
    }

    #[test]
    fn disabled_flag() {
        let gs = GridSnap::default().disabled();
        assert!(!gs.enabled);
    }
}
