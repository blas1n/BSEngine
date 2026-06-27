use bevy_ecs::prelude::Component;

/// Operational state of a fuel tank.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuelState {
    /// Fuel level is healthy.
    Full,
    /// Fuel is being consumed normally.
    Running,
    /// Fuel has dropped below `low_threshold`.
    Low,
    /// Fuel depleted; powered system cannot operate.
    Empty,
}

/// Generic liquid-fuel tank component for vehicles, generators, torches, etc.
///
/// Distinct from `Jetpack` (which bundles fuel tracking with flight physics).
/// `Fuel` is a reusable resource component that any fuel-burning system can
/// attach to an entity.
///
/// Each frame the owning system calls `consume(rate, dt)` to burn fuel.
/// `tick()` updates state transitions. On depletion `just_emptied` fires for
/// one frame. Call `refuel(amount)` for pickups or refill stations.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Fuel {
    pub state: FuelState,
    pub fuel: f32,
    pub max_fuel: f32,
    /// Fraction of max_fuel at which state transitions to Low.
    pub low_threshold: f32,
    /// True on the exact frame the tank runs dry.
    pub just_emptied: bool,
    pub enabled: bool,
}

impl Fuel {
    pub fn new(max_fuel: f32) -> Self {
        Self {
            state: FuelState::Full,
            fuel: max_fuel,
            max_fuel: max_fuel.max(0.0),
            low_threshold: 0.25,
            just_emptied: false,
            enabled: true,
        }
    }

    pub fn with_low_threshold(mut self, fraction: f32) -> Self {
        self.low_threshold = fraction.clamp(0.0, 1.0);
        self
    }

    /// Start with a partial fill (0.0 to 1.0).
    pub fn with_fill(mut self, fraction: f32) -> Self {
        self.fuel = (self.max_fuel * fraction.clamp(0.0, 1.0)).min(self.max_fuel);
        self.update_state_no_event();
        self
    }

    pub fn empty(max_fuel: f32) -> Self {
        Self::new(max_fuel).with_fill(0.0)
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Consume `rate * dt` fuel. Returns how much was actually consumed.
    /// Call this each frame from the powered-system's update.
    pub fn consume(&mut self, rate: f32, dt: f32) -> f32 {
        if !self.enabled || self.state == FuelState::Empty {
            return 0.0;
        }
        let amount = (rate * dt).max(0.0);
        let consumed = amount.min(self.fuel);
        self.fuel -= consumed;
        consumed
    }

    /// Add fuel (pickup, refill station). Clamped to max_fuel.
    pub fn refuel(&mut self, amount: f32) {
        self.fuel = (self.fuel + amount.max(0.0)).min(self.max_fuel);
    }

    /// Advance state transitions. Call once per frame, after `consume()`.
    pub fn tick(&mut self) {
        self.just_emptied = false;

        let prev_empty = self.state == FuelState::Empty;
        self.update_state_no_event();

        if !prev_empty && self.state == FuelState::Empty {
            self.just_emptied = true;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.fuel <= 0.0
    }

    pub fn is_low(&self) -> bool {
        matches!(self.state, FuelState::Low | FuelState::Empty)
    }

    /// [0, 1] fill level.
    pub fn fuel_fraction(&self) -> f32 {
        if self.max_fuel > 0.0 {
            self.fuel / self.max_fuel
        } else {
            0.0
        }
    }

    fn update_state_no_event(&mut self) {
        self.state = if self.fuel <= 0.0 {
            self.fuel = 0.0;
            FuelState::Empty
        } else if self.fuel_fraction() <= self.low_threshold {
            FuelState::Low
        } else if (self.fuel - self.max_fuel).abs() < f32::EPSILON {
            FuelState::Full
        } else {
            FuelState::Running
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consume_decrements_fuel() {
        let mut f = Fuel::new(100.0);
        let consumed = f.consume(10.0, 1.0);
        assert!((consumed - 10.0).abs() < 1e-5);
        assert!((f.fuel - 90.0).abs() < 1e-5);
    }

    #[test]
    fn consume_clamps_at_zero() {
        let mut f = Fuel::new(5.0);
        let consumed = f.consume(100.0, 1.0);
        assert!((consumed - 5.0).abs() < 1e-5);
        assert_eq!(f.fuel, 0.0);
    }

    #[test]
    fn state_transitions_to_low() {
        let mut f = Fuel::new(100.0).with_low_threshold(0.25);
        f.consume(80.0, 1.0);
        f.tick();
        assert_eq!(f.state, FuelState::Low);
    }

    #[test]
    fn just_emptied_fires_once() {
        let mut f = Fuel::new(10.0);
        f.consume(10.0, 1.0);
        f.tick();
        assert!(f.just_emptied);
        f.tick();
        assert!(!f.just_emptied);
    }

    #[test]
    fn refuel_restores_fuel() {
        let mut f = Fuel::empty(100.0);
        f.refuel(50.0);
        f.tick();
        assert!((f.fuel - 50.0).abs() < 1e-5);
        assert_eq!(f.state, FuelState::Running); // 50/100 = 0.5 > 0.25 low threshold
    }

    #[test]
    fn refuel_clamps_at_max() {
        let mut f = Fuel::new(100.0);
        f.refuel(200.0);
        assert!((f.fuel - 100.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_blocks_consume() {
        let mut f = Fuel::new(100.0).disabled();
        let consumed = f.consume(10.0, 1.0);
        assert_eq!(consumed, 0.0);
        assert!((f.fuel - 100.0).abs() < 1e-5);
    }
}
