use bevy_ecs::prelude::Component;

/// A single numeric stat — health pool, attack damage, movement speed, etc.
/// Final value = clamp((base + bonus) * multiplier, min, max).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Stat {
    /// Intrinsic value before any modifiers.
    pub base: f32,
    /// Sum of all additive bonuses (buffs, equipment, etc.).
    pub bonus: f32,
    /// Multiplicative scale applied after the additive sum.
    pub multiplier: f32,
    /// Optional inclusive lower bound for the final value.
    pub min: Option<f32>,
    /// Optional inclusive upper bound for the final value.
    pub max: Option<f32>,
}

impl Stat {
    pub fn new(base: f32) -> Self {
        Self {
            base,
            bonus: 0.0,
            multiplier: 1.0,
            min: None,
            max: None,
        }
    }

    pub fn with_min(mut self, min: f32) -> Self {
        self.min = Some(min);
        self
    }

    pub fn with_max(mut self, max: f32) -> Self {
        self.max = Some(max);
        self
    }

    pub fn with_range(mut self, min: f32, max: f32) -> Self {
        self.min = Some(min);
        self.max = Some(max.max(min));
        self
    }

    /// Final clamped value.
    pub fn value(&self) -> f32 {
        let raw = (self.base + self.bonus) * self.multiplier;
        let low = self.min.map_or(raw, |m| raw.max(m));
        self.max.map_or(low, |m| low.min(m))
    }

    /// Add a flat bonus.
    pub fn add_bonus(&mut self, amount: f32) {
        self.bonus += amount;
    }

    /// Remove a flat bonus (negative `amount` to reverse an `add_bonus` call).
    pub fn remove_bonus(&mut self, amount: f32) {
        self.bonus -= amount;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_value() {
        let s = Stat::new(100.0);
        assert!((s.value() - 100.0).abs() < 0.001);
    }

    #[test]
    fn bonus_and_multiplier() {
        let mut s = Stat::new(100.0);
        s.multiplier = 1.5;
        s.add_bonus(20.0);
        assert!((s.value() - 180.0).abs() < 0.001); // (100+20)*1.5
    }

    #[test]
    fn clamped_to_min() {
        let s = Stat::new(-10.0).with_min(0.0);
        assert_eq!(s.value(), 0.0);
    }

    #[test]
    fn clamped_to_max() {
        let s = Stat::new(200.0).with_max(100.0);
        assert_eq!(s.value(), 100.0);
    }

    #[test]
    fn remove_bonus() {
        let mut s = Stat::new(50.0);
        s.add_bonus(30.0);
        s.remove_bonus(30.0);
        assert!((s.value() - 50.0).abs() < 0.001);
    }
}
