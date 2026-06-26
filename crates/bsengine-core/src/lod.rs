use bevy_ecs::prelude::Component;

/// One LOD level: a mesh to use when the camera is within `max_distance` units.
/// Levels should be ordered from nearest (highest detail) to farthest (lowest detail).
#[derive(Debug, Clone, PartialEq)]
pub struct LodLevel {
    /// Asset path of the mesh to use at this LOD level.
    pub mesh_path: String,
    /// Maximum camera distance at which this level is displayed.
    /// The last (farthest) level should use `f32::INFINITY` or a large value.
    pub max_distance: f32,
}

impl LodLevel {
    pub fn new(mesh_path: impl Into<String>, max_distance: f32) -> Self {
        Self {
            mesh_path: mesh_path.into(),
            max_distance: max_distance.max(0.0),
        }
    }
}

/// Level-of-Detail control for a mesh entity.
/// The render system picks the first level whose `max_distance` >= camera distance.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Lod {
    pub levels: Vec<LodLevel>,
}

impl Lod {
    pub fn new(levels: Vec<LodLevel>) -> Self {
        Self { levels }
    }

    /// Returns the index of the active LOD level for the given camera distance.
    /// Returns `None` if there are no levels.
    pub fn active_level(&self, distance: f32) -> Option<usize> {
        if self.levels.is_empty() {
            return None;
        }
        for (i, level) in self.levels.iter().enumerate() {
            if distance <= level.max_distance {
                return Some(i);
            }
        }
        Some(self.levels.len() - 1)
    }

    pub fn level_count(&self) -> usize {
        self.levels.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_lod() -> Lod {
        Lod::new(vec![
            LodLevel::new("models/high.glb", 20.0),
            LodLevel::new("models/mid.glb", 60.0),
            LodLevel::new("models/low.glb", f32::INFINITY),
        ])
    }

    #[test]
    fn lod_selects_first_level_when_close() {
        let lod = make_lod();
        assert_eq!(lod.active_level(5.0), Some(0));
        assert_eq!(lod.active_level(20.0), Some(0));
    }

    #[test]
    fn lod_selects_mid_level() {
        let lod = make_lod();
        assert_eq!(lod.active_level(21.0), Some(1));
        assert_eq!(lod.active_level(60.0), Some(1));
    }

    #[test]
    fn lod_falls_back_to_last_level() {
        let lod = make_lod();
        assert_eq!(lod.active_level(1000.0), Some(2));
    }

    #[test]
    fn lod_empty_returns_none() {
        let lod = Lod::new(vec![]);
        assert_eq!(lod.active_level(10.0), None);
    }

    #[test]
    fn lod_level_negative_distance_clamped() {
        let level = LodLevel::new("model.glb", -5.0);
        assert_eq!(level.max_distance, 0.0);
    }
}
