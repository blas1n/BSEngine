use bevy_ecs::prelude::Component;

/// Thermal state of an entity — used by fire, lava, frost, and environmental damage zones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThermalState {
    /// Temperature within the comfortable baseline band.
    Normal,
    /// Temperature above the heat threshold — fire/burning effects active.
    Overheated,
    /// Temperature below the cold threshold — frost/freezing effects active.
    Frozen,
}

/// Entity temperature and thermal resistance component.
///
/// Environmental systems call `apply_heat(rate, dt)` or `apply_cold(rate, dt)`.
/// `tick(dt)` decays temperature back toward `resting_temp` each frame.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Heat {
    /// Current temperature in arbitrary units (0 = cold, 100 = very hot, 50 = neutral).
    pub temperature: f32,
    /// Comfortable baseline temperature the entity passively returns to.
    pub resting_temp: f32,
    /// Temperature above which state is `Overheated`.
    pub heat_threshold: f32,
    /// Temperature below which state is `Frozen`.
    pub cold_threshold: f32,
    /// How quickly temperature decays toward `resting_temp` per second (units/s).
    pub decay_rate: f32,
    /// Multiplier applied to incoming heat changes (0 = immune, 2 = doubly sensitive).
    pub resistance: f32,
    pub state: ThermalState,
    pub enabled: bool,
}

impl Heat {
    pub fn new(resting_temp: f32) -> Self {
        Self {
            temperature: resting_temp,
            resting_temp,
            heat_threshold: 80.0,
            cold_threshold: 20.0,
            decay_rate: 5.0,
            resistance: 1.0,
            state: ThermalState::Normal,
            enabled: true,
        }
    }

    pub fn with_thresholds(mut self, cold: f32, hot: f32) -> Self {
        self.cold_threshold = cold;
        self.heat_threshold = hot;
        self
    }

    pub fn with_decay_rate(mut self, rate: f32) -> Self {
        self.decay_rate = rate.max(0.0);
        self
    }

    pub fn with_resistance(mut self, resistance: f32) -> Self {
        self.resistance = resistance.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Raise temperature by `rate * dt`, scaled by `resistance`.
    pub fn apply_heat(&mut self, rate: f32, dt: f32) {
        if !self.enabled {
            return;
        }
        self.temperature += rate * dt * self.resistance;
        self.update_state();
    }

    /// Lower temperature by `rate * dt`, scaled by `resistance`.
    pub fn apply_cold(&mut self, rate: f32, dt: f32) {
        if !self.enabled {
            return;
        }
        self.temperature -= rate * dt * self.resistance;
        self.update_state();
    }

    /// Passively decay temperature toward `resting_temp` each frame.
    pub fn tick(&mut self, dt: f32) {
        if !self.enabled {
            return;
        }
        let diff = self.resting_temp - self.temperature;
        if diff.abs() < 0.001 {
            return;
        }
        let step = self.decay_rate * dt;
        if diff.abs() <= step {
            self.temperature = self.resting_temp;
        } else {
            self.temperature += diff.signum() * step;
        }
        self.update_state();
    }

    fn update_state(&mut self) {
        self.state = if self.temperature >= self.heat_threshold {
            ThermalState::Overheated
        } else if self.temperature <= self.cold_threshold {
            ThermalState::Frozen
        } else {
            ThermalState::Normal
        };
    }

    pub fn is_overheated(&self) -> bool {
        self.state == ThermalState::Overheated
    }

    pub fn is_frozen(&self) -> bool {
        self.state == ThermalState::Frozen
    }

    /// Normalised temperature [0, 1] between cold and heat thresholds.
    pub fn fraction(&self) -> f32 {
        let range = self.heat_threshold - self.cold_threshold;
        if range <= 0.0 {
            return 0.5;
        }
        ((self.temperature - self.cold_threshold) / range).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_heat_transitions_to_overheated() {
        let mut h = Heat::new(50.0).with_thresholds(20.0, 80.0);
        h.apply_heat(35.0, 1.0);
        assert!(h.is_overheated());
    }

    #[test]
    fn apply_cold_transitions_to_frozen() {
        let mut h = Heat::new(50.0).with_thresholds(20.0, 80.0);
        h.apply_cold(35.0, 1.0);
        assert!(h.is_frozen());
    }

    #[test]
    fn tick_decays_to_resting() {
        let mut h = Heat::new(50.0).with_decay_rate(10.0);
        h.temperature = 90.0;
        h.tick(4.0);
        assert!((h.temperature - 50.0).abs() < 0.001);
    }

    #[test]
    fn resistance_scales_heat_gain() {
        let mut h = Heat::new(50.0).with_resistance(0.5);
        h.apply_heat(20.0, 1.0);
        assert!((h.temperature - 60.0).abs() < 0.001);
    }

    #[test]
    fn fraction_midpoint() {
        let h = Heat::new(50.0).with_thresholds(0.0, 100.0);
        assert!((h.fraction() - 0.5).abs() < 0.001);
    }
}
