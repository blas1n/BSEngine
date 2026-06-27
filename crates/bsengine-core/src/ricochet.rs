use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Projectile bounce component — allows a projectile to ricochet off surfaces.
///
/// The projectile system calls `bounce(surface_normal)` when the projectile
/// hits a surface. If bounces remain, the method returns the reflected velocity
/// direction and decrements `bounces_remaining`. When no bounces remain,
/// `bounce()` returns `None` and the projectile should be destroyed.
///
/// `energy_retention` (0.0–1.0) scales the speed after each bounce to simulate
/// energy loss. `min_dot` filters out grazing impacts: if `dot(-velocity, normal)
/// < min_dot` the bounce is skipped (the projectile passes through at near-parallel
/// angles rather than sticking or inverting).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Ricochet {
    /// Maximum number of bounces before the projectile is spent.
    pub max_bounces: u32,
    /// Bounces remaining before the projectile is spent.
    pub bounces_remaining: u32,
    /// Fraction of speed retained after each bounce (0.0 = dead stop, 1.0 = elastic).
    pub energy_retention: f32,
    /// Minimum dot product of (-velocity, normal) to register a bounce (0.0 = any angle).
    pub min_dot: f32,
    /// True on the frame a bounce was registered.
    pub just_bounced: bool,
    pub enabled: bool,
}

impl Ricochet {
    pub fn new(max_bounces: u32, energy_retention: f32) -> Self {
        Self {
            max_bounces,
            bounces_remaining: max_bounces,
            energy_retention: energy_retention.clamp(0.0, 1.0),
            min_dot: 0.0,
            just_bounced: false,
            enabled: true,
        }
    }

    pub fn with_min_dot(mut self, dot: f32) -> Self {
        self.min_dot = dot.clamp(0.0, 1.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Attempt a bounce given the incoming `velocity` and surface `normal`.
    ///
    /// Returns the reflected velocity (magnitude scaled by `energy_retention`)
    /// if the bounce is valid, or `None` if out of bounces / disabled / grazing.
    pub fn bounce(&mut self, velocity: Vec3, normal: Vec3) -> Option<Vec3> {
        self.just_bounced = false;

        if !self.enabled || self.bounces_remaining == 0 {
            return None;
        }

        let speed = velocity.length();
        if speed < 1e-6 {
            return None;
        }

        let dir = velocity / speed;
        // dot(-dir, normal): how head-on the impact is.
        let dot = (-dir).dot(normal);
        if dot < self.min_dot {
            return None;
        }

        // Reflect: v' = v - 2(v·n)n
        let reflected = velocity - 2.0 * velocity.dot(normal) * normal;
        let new_velocity = reflected * self.energy_retention;

        self.bounces_remaining -= 1;
        self.just_bounced = true;
        Some(new_velocity)
    }

    pub fn reset(&mut self) {
        self.bounces_remaining = self.max_bounces;
        self.just_bounced = false;
    }

    pub fn can_bounce(&self) -> bool {
        self.enabled && self.bounces_remaining > 0
    }

    /// Fraction of bounces remaining (1.0 = full, 0.0 = spent).
    pub fn bounce_fraction(&self) -> f32 {
        if self.max_bounces > 0 {
            self.bounces_remaining as f32 / self.max_bounces as f32
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounce_reflects_velocity() {
        let mut r = Ricochet::new(3, 1.0);
        let v = Vec3::new(0.0, -1.0, 0.0);
        let normal = Vec3::Y;
        let result = r.bounce(v, normal).unwrap();
        assert!((result.y - 1.0).abs() < 1e-5);
    }

    #[test]
    fn bounce_decrements_count() {
        let mut r = Ricochet::new(2, 1.0);
        r.bounce(Vec3::new(0.0, -1.0, 0.0), Vec3::Y);
        assert_eq!(r.bounces_remaining, 1);
    }

    #[test]
    fn no_bounce_when_spent() {
        let mut r = Ricochet::new(1, 1.0);
        r.bounce(Vec3::new(0.0, -1.0, 0.0), Vec3::Y);
        let result = r.bounce(Vec3::new(0.0, -1.0, 0.0), Vec3::Y);
        assert!(result.is_none());
    }

    #[test]
    fn energy_retention_scales_speed() {
        let mut r = Ricochet::new(3, 0.5);
        let v = Vec3::new(0.0, -2.0, 0.0);
        let result = r.bounce(v, Vec3::Y).unwrap();
        assert!((result.length() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn min_dot_filters_grazing() {
        let mut r = Ricochet::new(3, 1.0).with_min_dot(0.9);
        // Grazing: velocity is nearly parallel to the wall (small head-on component)
        let v = Vec3::new(1.0, -0.01, 0.0); // mostly horizontal, tiny downward
        let result = r.bounce(v, Vec3::Y);
        assert!(result.is_none());
    }

    #[test]
    fn reset_restores_full_bounces() {
        let mut r = Ricochet::new(3, 1.0);
        r.bounce(Vec3::new(0.0, -1.0, 0.0), Vec3::Y);
        r.reset();
        assert_eq!(r.bounces_remaining, 3);
    }
}
