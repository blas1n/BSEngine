use bevy_ecs::prelude::Component;

/// Role of this node in the power network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerRole {
    /// Generates power (engine, solar panel, generator).
    Producer,
    /// Consumes power (light, motor, trap).
    Consumer,
    /// Can both produce and consume (battery, capacitor).
    Storage,
}

/// Electrical/energy power node component.
///
/// Attach to generators, batteries, and devices that participate in a power grid.
/// The power system sums all producers' `output_watts` and allocates to consumers
/// in priority order, then calls `tick(dt)` to update stored charge and `powered` flag.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Power {
    pub role: PowerRole,
    /// Maximum power this node can deliver per second (watts). Relevant for Producer/Storage.
    pub output_watts: f32,
    /// Power this node requires per second (watts). Relevant for Consumer/Storage (charging).
    pub draw_watts: f32,
    /// Current stored charge (joules). Producers may set this to `f32::INFINITY`.
    pub charge: f32,
    /// Maximum stored charge (joules). Irrelevant for pure producers.
    pub max_charge: f32,
    /// Charge / discharge efficiency [0, 1].
    pub efficiency: f32,
    /// True when the node received enough power (or produced it) this frame.
    pub powered: bool,
    /// Priority for load-shedding — higher priority consumers get power first.
    pub priority: u32,
    pub enabled: bool,
}

impl Power {
    pub fn producer(output_watts: f32) -> Self {
        Self {
            role: PowerRole::Producer,
            output_watts: output_watts.max(0.0),
            draw_watts: 0.0,
            charge: f32::INFINITY,
            max_charge: f32::INFINITY,
            efficiency: 1.0,
            powered: true,
            priority: 0,
            enabled: true,
        }
    }

    pub fn consumer(draw_watts: f32) -> Self {
        Self {
            role: PowerRole::Consumer,
            output_watts: 0.0,
            draw_watts: draw_watts.max(0.0),
            charge: 0.0,
            max_charge: 0.0,
            efficiency: 1.0,
            powered: false,
            priority: 0,
            enabled: true,
        }
    }

    pub fn storage(capacity_joules: f32, output_watts: f32, draw_watts: f32) -> Self {
        Self {
            role: PowerRole::Storage,
            output_watts: output_watts.max(0.0),
            draw_watts: draw_watts.max(0.0),
            charge: 0.0,
            max_charge: capacity_joules.max(0.0),
            efficiency: 0.9,
            powered: false,
            priority: 0,
            enabled: true,
        }
    }

    pub fn with_priority(mut self, p: u32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_efficiency(mut self, e: f32) -> Self {
        self.efficiency = e.clamp(0.0, 1.0);
        self
    }

    pub fn with_initial_charge(mut self, joules: f32) -> Self {
        self.charge = joules.clamp(0.0, self.max_charge);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Deposit `joules` into storage (respects efficiency and capacity).
    /// Returns actual joules stored.
    pub fn deposit(&mut self, joules: f32) -> f32 {
        if self.role != PowerRole::Storage {
            return 0.0;
        }
        let storable = (self.max_charge - self.charge).max(0.0);
        let actual = (joules * self.efficiency).min(storable);
        self.charge += actual;
        actual
    }

    /// Withdraw `joules` from storage.
    /// Returns actual joules available.
    pub fn withdraw(&mut self, joules: f32) -> f32 {
        let available = self.charge.min(joules);
        self.charge -= available;
        available
    }

    pub fn charge_fraction(&self) -> f32 {
        if self.max_charge <= 0.0 || self.max_charge.is_infinite() {
            1.0
        } else {
            (self.charge / self.max_charge).clamp(0.0, 1.0)
        }
    }

    pub fn is_full(&self) -> bool {
        self.charge >= self.max_charge
    }

    pub fn is_empty(&self) -> bool {
        self.charge <= 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn producer_defaults_powered_and_infinite_charge() {
        let p = Power::producer(100.0);
        assert!(p.powered);
        assert!(p.charge.is_infinite());
    }

    #[test]
    fn consumer_defaults_unpowered() {
        let c = Power::consumer(50.0);
        assert!(!c.powered);
    }

    #[test]
    fn deposit_respects_efficiency() {
        let mut s = Power::storage(100.0, 10.0, 10.0).with_efficiency(0.8);
        let stored = s.deposit(10.0);
        assert!((stored - 8.0).abs() < 1e-5);
        assert!((s.charge - 8.0).abs() < 1e-5);
    }

    #[test]
    fn deposit_capped_at_capacity() {
        let mut s = Power::storage(10.0, 5.0, 5.0)
            .with_efficiency(1.0)
            .with_initial_charge(8.0);
        s.deposit(5.0); // only 2 joules fit
        assert!((s.charge - 10.0).abs() < 1e-5);
    }

    #[test]
    fn withdraw_reduces_charge() {
        let mut s = Power::storage(100.0, 10.0, 10.0).with_initial_charge(50.0);
        let got = s.withdraw(20.0);
        assert!((got - 20.0).abs() < 1e-5);
        assert!((s.charge - 30.0).abs() < 1e-5);
    }

    #[test]
    fn withdraw_capped_at_available() {
        let mut s = Power::storage(100.0, 10.0, 10.0).with_initial_charge(5.0);
        let got = s.withdraw(20.0);
        assert!((got - 5.0).abs() < 1e-5);
        assert!(s.is_empty());
    }
}
