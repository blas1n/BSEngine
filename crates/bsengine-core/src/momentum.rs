use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Accumulated impulse/momentum for physics-gameplay systems.
///
/// Unlike `Velocity` (which stores the final movement vector), `Momentum`
/// accumulates impulses over time and decays them. Useful for:
/// - Momentum-based abilities (skating, heavy objects, drift)
/// - Combo multipliers scaled by movement speed
/// - Environmental momentum from conveyors or explosions
///
/// Call `add(impulse)` to inject velocity. `tick(dt)` decays `current` by
/// `damping` (fraction-per-second retained, 0.0 = instant stop, 1.0 = no decay).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Momentum {
    /// Accumulated momentum vector (world space, units/s).
    pub current: Vec3,
    /// Fraction of momentum retained per second (0.0–1.0).
    pub damping: f32,
    /// Maximum speed the accumulated momentum can reach (0.0 = uncapped).
    pub max_speed: f32,
    pub enabled: bool,
}

impl Momentum {
    pub fn new(damping: f32) -> Self {
        Self {
            current: Vec3::ZERO,
            damping: damping.clamp(0.0, 1.0),
            max_speed: 0.0,
            enabled: true,
        }
    }

    pub fn with_max_speed(mut self, speed: f32) -> Self {
        self.max_speed = speed.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Add an impulse vector to the accumulated momentum.
    pub fn add(&mut self, impulse: Vec3) {
        if !self.enabled {
            return;
        }
        self.current += impulse;
        self.clamp_to_max();
    }

    /// Decay momentum by `damping` and return the current momentum vector.
    pub fn tick(&mut self, dt: f32) -> Vec3 {
        if !self.enabled {
            return Vec3::ZERO;
        }
        let retain = self.damping.powf(dt);
        self.current *= retain;
        if self.current.length_squared() < 1e-6 {
            self.current = Vec3::ZERO;
        }
        self.current
    }

    /// Instantly halt all momentum.
    pub fn stop(&mut self) {
        self.current = Vec3::ZERO;
    }

    /// Current speed (magnitude of momentum vector).
    pub fn speed(&self) -> f32 {
        self.current.length()
    }

    /// Fraction of `max_speed` currently reached (0.0–1.0). Always 0.0 if uncapped.
    pub fn speed_fraction(&self) -> f32 {
        if self.max_speed > 0.0 {
            (self.speed() / self.max_speed).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    fn clamp_to_max(&mut self) {
        if self.max_speed > 0.0 {
            let spd = self.speed();
            if spd > self.max_speed {
                self.current = self.current / spd * self.max_speed;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_accumulates_impulses() {
        let mut m = Momentum::new(0.9);
        m.add(Vec3::new(1.0, 0.0, 0.0));
        m.add(Vec3::new(2.0, 0.0, 0.0));
        assert!((m.current.x - 3.0).abs() < 1e-5);
    }

    #[test]
    fn tick_decays_momentum() {
        let mut m = Momentum::new(0.0); // instant stop
        m.add(Vec3::new(5.0, 0.0, 0.0));
        let result = m.tick(1.0);
        assert!(result.length() < 1e-5);
    }

    #[test]
    fn tick_with_full_damping_retains_momentum() {
        let mut m = Momentum::new(1.0); // no decay
        m.add(Vec3::new(3.0, 0.0, 0.0));
        let result = m.tick(1.0);
        assert!((result.x - 3.0).abs() < 1e-4);
    }

    #[test]
    fn max_speed_clamps_impulse() {
        let mut m = Momentum::new(0.9).with_max_speed(5.0);
        m.add(Vec3::new(10.0, 0.0, 0.0));
        assert!((m.speed() - 5.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_ignores_add() {
        let mut m = Momentum::new(0.9).disabled();
        m.add(Vec3::new(10.0, 0.0, 0.0));
        assert_eq!(m.current, Vec3::ZERO);
    }

    #[test]
    fn stop_zeroes_current() {
        let mut m = Momentum::new(0.9);
        m.add(Vec3::new(5.0, 2.0, 1.0));
        m.stop();
        assert_eq!(m.current, Vec3::ZERO);
    }
}
