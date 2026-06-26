use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Current operational state of the jetpack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JetpackState {
    /// Thrusters off; no fuel consumed.
    Idle,
    /// Thrusters firing; fuel drains, force applied.
    Thrusting,
    /// Fuel depleted; no thrust available until refuelled.
    Depleted,
}

/// Thruster / jetpack component for sustained flight or directional boosts.
///
/// Each frame, if `wants_thrust` is true and fuel remains, the movement system:
///   1. Checks `state` and `fuel`.
///   2. Calls `tick(dt)` to drain fuel and apply `thrust_force` in `thrust_direction`.
///   3. Reads `current_thrust()` to add the impulse to the rigidbody.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Jetpack {
    pub state: JetpackState,
    /// World-space thrust direction (normalised by the movement system).
    pub thrust_direction: Vec3,
    /// Force magnitude applied per second while thrusting (N).
    pub thrust_force: f32,
    /// Remaining fuel (arbitrary units).
    pub fuel: f32,
    /// Maximum fuel capacity.
    pub max_fuel: f32,
    /// Fuel consumed per second while thrusting.
    pub fuel_drain_rate: f32,
    /// Fuel recovered per second while idle and grounded.
    pub fuel_regen_rate: f32,
    /// True when the player is holding the thrust input.
    pub wants_thrust: bool,
    /// Whether the regen tick runs even while airborne.
    pub regen_in_air: bool,
    pub enabled: bool,
}

impl Jetpack {
    pub fn new(thrust_force: f32, max_fuel: f32, fuel_drain_rate: f32) -> Self {
        Self {
            state: JetpackState::Idle,
            thrust_direction: Vec3::Y,
            thrust_force: thrust_force.max(0.0),
            fuel: max_fuel,
            max_fuel: max_fuel.max(0.0),
            fuel_drain_rate: fuel_drain_rate.max(0.0),
            fuel_regen_rate: 0.0,
            wants_thrust: false,
            regen_in_air: false,
            enabled: true,
        }
    }

    pub fn with_regen(mut self, rate: f32, in_air: bool) -> Self {
        self.fuel_regen_rate = rate.max(0.0);
        self.regen_in_air = in_air;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Advance the jetpack. `is_grounded` controls regen eligibility.
    /// Returns the thrust impulse vector to add to the rigidbody this frame.
    pub fn tick(&mut self, dt: f32, is_grounded: bool) -> Vec3 {
        if !self.enabled {
            return Vec3::ZERO;
        }

        let can_regen = is_grounded || self.regen_in_air;
        let thrusting = self.wants_thrust && self.fuel > 0.0;

        if thrusting {
            self.fuel = (self.fuel - self.fuel_drain_rate * dt).max(0.0);
            // Transition to Depleted immediately if fuel ran out this frame.
            self.state = if self.fuel <= 0.0 {
                JetpackState::Depleted
            } else {
                JetpackState::Thrusting
            };
        } else {
            self.state = if self.fuel <= 0.0 {
                JetpackState::Depleted
            } else {
                JetpackState::Idle
            };
            if can_regen && self.fuel_regen_rate > 0.0 {
                self.fuel = (self.fuel + self.fuel_regen_rate * dt).min(self.max_fuel);
                if self.fuel > 0.0 {
                    self.state = JetpackState::Idle;
                }
            }
        }

        if thrusting {
            self.thrust_direction.normalize_or_zero() * self.thrust_force
        } else {
            Vec3::ZERO
        }
    }

    pub fn fuel_fraction(&self) -> f32 {
        if self.max_fuel > 0.0 {
            self.fuel / self.max_fuel
        } else {
            0.0
        }
    }

    pub fn is_thrusting(&self) -> bool {
        self.state == JetpackState::Thrusting
    }

    pub fn is_depleted(&self) -> bool {
        self.state == JetpackState::Depleted
    }

    /// Instantly refuel to max capacity.
    pub fn refuel(&mut self) {
        self.fuel = self.max_fuel;
        self.state = JetpackState::Idle;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thrust_produces_force() {
        let mut j = Jetpack::new(10.0, 100.0, 5.0);
        j.wants_thrust = true;
        let force = j.tick(0.016, false);
        assert!(force.length() > 0.0);
        assert!(j.is_thrusting());
    }

    #[test]
    fn fuel_drains_while_thrusting() {
        let mut j = Jetpack::new(10.0, 100.0, 10.0);
        j.wants_thrust = true;
        j.tick(1.0, false);
        assert!((j.fuel - 90.0).abs() < 0.01);
    }

    #[test]
    fn depletes_when_fuel_exhausted() {
        let mut j = Jetpack::new(10.0, 1.0, 10.0);
        j.wants_thrust = true;
        j.tick(1.0, false);
        assert!(j.is_depleted());
    }

    #[test]
    fn no_thrust_when_depleted() {
        let mut j = Jetpack::new(10.0, 0.1, 10.0);
        j.wants_thrust = true;
        j.tick(1.0, false); // depletes
        let force = j.tick(0.1, false);
        assert_eq!(force, Vec3::ZERO);
    }

    #[test]
    fn regen_refills_fuel_when_grounded() {
        let mut j = Jetpack::new(10.0, 100.0, 20.0).with_regen(10.0, false);
        j.wants_thrust = true;
        j.tick(1.0, false); // drain 20
        j.wants_thrust = false;
        j.tick(1.0, true); // regen 10
        assert!((j.fuel - 90.0).abs() < 0.01);
    }

    #[test]
    fn regen_skipped_when_airborne_and_not_allowed() {
        let mut j = Jetpack::new(10.0, 100.0, 20.0).with_regen(10.0, false);
        j.wants_thrust = true;
        j.tick(1.0, false); // drain 20
        let before = j.fuel;
        j.wants_thrust = false;
        j.tick(1.0, false); // airborne, no regen allowed
        assert!((j.fuel - before).abs() < 1e-5);
    }
}
