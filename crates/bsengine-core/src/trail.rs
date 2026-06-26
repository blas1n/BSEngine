use bevy_ecs::prelude::Component;
use glam::Vec3;

/// A single point in the trail ribbon.
#[derive(Debug, Clone, PartialEq)]
pub struct TrailPoint {
    pub position: Vec3,
    /// Normalised age [0 = just spawned, 1 = about to expire].
    pub age: f32,
}

/// Movement ribbon trail for VFX (speedlines, ghost trail, weapon arc, etc.).
///
/// Each frame the system should call `tick(position, dt)` which appends a new
/// point at the entity's position and ages/removes expired points.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Trail {
    /// Ordered trail points, newest last.
    pub points: Vec<TrailPoint>,
    /// Maximum number of points retained in the trail.
    pub max_points: usize,
    /// How long each point lives before being removed (seconds).
    pub point_lifetime: f32,
    /// Minimum distance the entity must move before a new point is recorded.
    pub min_distance: f32,
    /// Width of the ribbon at the newest point (units).
    pub width_start: f32,
    /// Width of the ribbon at the oldest point (tapers to this).
    pub width_end: f32,
    /// Whether to emit new points this frame.
    pub emitting: bool,
    pub enabled: bool,
}

impl Trail {
    pub fn new(point_lifetime: f32) -> Self {
        Self {
            points: Vec::new(),
            max_points: 64,
            point_lifetime: point_lifetime.max(0.01),
            min_distance: 0.05,
            width_start: 0.1,
            width_end: 0.0,
            emitting: true,
            enabled: true,
        }
    }

    pub fn with_max_points(mut self, n: usize) -> Self {
        self.max_points = n.max(2);
        self
    }

    pub fn with_min_distance(mut self, dist: f32) -> Self {
        self.min_distance = dist.max(0.0);
        self
    }

    pub fn with_width(mut self, start: f32, end: f32) -> Self {
        self.width_start = start.max(0.0);
        self.width_end = end.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Update the trail each frame.
    /// Appends a new point at `position` if `emitting` and the entity moved far enough.
    /// Ages existing points and removes expired ones.
    pub fn tick(&mut self, position: Vec3, dt: f32) {
        if !self.enabled {
            return;
        }

        // Age points and remove expired.
        let lifetime = self.point_lifetime;
        for p in &mut self.points {
            p.age += dt / lifetime;
        }
        self.points.retain(|p| p.age < 1.0);

        // Emit a new point if far enough from the last one.
        if self.emitting {
            let should_emit = self.points.last().map_or(true, |last| {
                last.position.distance(position) >= self.min_distance
            });
            if should_emit {
                self.points.push(TrailPoint { position, age: 0.0 });
                if self.points.len() > self.max_points {
                    self.points.remove(0);
                }
            }
        }
    }

    pub fn clear(&mut self) {
        self.points.clear();
    }

    pub fn point_count(&self) -> usize {
        self.points.len()
    }

    /// Width at a given normalised trail position [0 = newest, 1 = oldest].
    pub fn width_at(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        self.width_start + (self.width_end - self.width_start) * t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_adds_point() {
        let mut t = Trail::new(2.0);
        t.tick(Vec3::ZERO, 0.016);
        assert_eq!(t.point_count(), 1);
    }

    #[test]
    fn min_distance_prevents_duplicate_points() {
        let mut t = Trail::new(2.0).with_min_distance(1.0);
        t.tick(Vec3::ZERO, 0.016);
        t.tick(Vec3::new(0.1, 0.0, 0.0), 0.016);
        assert_eq!(t.point_count(), 1);
        t.tick(Vec3::new(1.5, 0.0, 0.0), 0.016);
        assert_eq!(t.point_count(), 2);
    }

    #[test]
    fn expired_points_removed() {
        let mut t = Trail::new(0.1);
        t.tick(Vec3::ZERO, 0.016);
        t.tick(Vec3::new(2.0, 0.0, 0.0), 0.12);
        assert_eq!(t.point_count(), 1);
    }

    #[test]
    fn max_points_capped() {
        let mut t = Trail::new(60.0).with_max_points(3).with_min_distance(0.0);
        for i in 0..5 {
            t.tick(Vec3::new(i as f32 * 2.0, 0.0, 0.0), 0.016);
        }
        assert_eq!(t.point_count(), 3);
    }

    #[test]
    fn width_at_interpolates() {
        let t = Trail::new(1.0).with_width(0.4, 0.0);
        assert!((t.width_at(0.0) - 0.4).abs() < 0.001);
        assert!((t.width_at(1.0)).abs() < 0.001);
        assert!((t.width_at(0.5) - 0.2).abs() < 0.001);
    }
}
