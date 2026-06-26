use bevy_ecs::prelude::Component;

/// Fired projectile data — controls motion, lifetime, and hit behaviour.
/// The projectile movement system reads this component to advance position,
/// apply gravity, and despawn when `range` is exceeded.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Projectile {
    /// Movement speed in world units per second.
    pub speed: f32,
    /// Gravity multiplier. 0 = no gravity (laser), 1 = full gravity (grenade).
    pub gravity_scale: f32,
    /// How many targets the projectile can pierce before stopping.
    /// 0 = stops on the first hit.
    pub piercing: u32,
    /// Maximum travel distance in world units before the projectile is despawned.
    /// 0 = unlimited range.
    pub range: f32,
    /// Accumulated travel distance — written by the movement system each frame.
    pub distance_traveled: f32,
}

impl Projectile {
    pub fn new(speed: f32) -> Self {
        Self {
            speed: speed.max(0.0),
            gravity_scale: 0.0,
            piercing: 0,
            range: 0.0,
            distance_traveled: 0.0,
        }
    }

    pub fn with_gravity(mut self, scale: f32) -> Self {
        self.gravity_scale = scale.max(0.0);
        self
    }

    pub fn with_piercing(mut self, count: u32) -> Self {
        self.piercing = count;
        self
    }

    pub fn with_range(mut self, range: f32) -> Self {
        self.range = range.max(0.0);
        self
    }

    /// Returns `true` if the projectile has exceeded its maximum range.
    /// Always returns `false` when `range == 0` (unlimited).
    pub fn out_of_range(&self) -> bool {
        self.range > 0.0 && self.distance_traveled >= self.range
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projectile_defaults() {
        let p = Projectile::new(20.0);
        assert!((p.speed - 20.0).abs() < 0.001);
        assert_eq!(p.gravity_scale, 0.0);
        assert_eq!(p.piercing, 0);
        assert_eq!(p.range, 0.0);
        assert_eq!(p.distance_traveled, 0.0);
    }

    #[test]
    fn speed_clamped() {
        let p = Projectile::new(-5.0);
        assert_eq!(p.speed, 0.0);
    }

    #[test]
    fn unlimited_range_never_expires() {
        let mut p = Projectile::new(10.0); // range = 0 = unlimited
        p.distance_traveled = 1_000_000.0;
        assert!(!p.out_of_range());
    }

    #[test]
    fn out_of_range_triggers() {
        let mut p = Projectile::new(10.0).with_range(50.0);
        p.distance_traveled = 50.0;
        assert!(p.out_of_range());
    }

    #[test]
    fn piercing_stored() {
        let p = Projectile::new(15.0).with_piercing(3);
        assert_eq!(p.piercing, 3);
    }
}
