use bevy_ecs::prelude::Component;

/// Encumbrance tracker: when `current_load` exceeds `max_load`, the entity
/// is overladen and movement systems should apply a speed penalty proportional
/// to how much it is over capacity.
///
/// `add_load(amount)` and `remove_load(amount)` update `current_load`.
/// `overload_ratio()` returns how far beyond capacity the entity is (clamped
/// to [0.0, 1.0]). `effective_speed(base)` applies the penalty:
/// `base * (1 - speed_penalty * overload_ratio())` while overladen and enabled.
///
/// Distinct from `Slow` (applied CC debuff), `Exhaustion` (stamina depletion),
/// and `Hobble` (leg injury): Laden is a **physics-grounded weight penalty** —
/// the entity is carrying too much, slowing it in proportion to the excess load.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Laden {
    pub current_load: f32,
    /// Maximum load before speed penalties apply. Clamped ≥ 0.0.
    pub max_load: f32,
    /// Fraction of base speed lost per unit of overload ratio. Clamped [0.0, 1.0].
    /// e.g. 0.5 means fully double-loaded → 50% speed penalty.
    pub speed_penalty: f32,
    pub enabled: bool,
}

impl Laden {
    pub fn new(max_load: f32, speed_penalty: f32) -> Self {
        Self {
            current_load: 0.0,
            max_load: max_load.max(0.0),
            speed_penalty: speed_penalty.clamp(0.0, 1.0),
            enabled: true,
        }
    }

    /// Add weight to the carried load. No-op for non-positive amounts.
    pub fn add_load(&mut self, amount: f32) {
        if amount > 0.0 {
            self.current_load += amount;
        }
    }

    /// Remove weight from the carried load; floors at 0.0.
    pub fn remove_load(&mut self, amount: f32) {
        if amount > 0.0 {
            self.current_load = (self.current_load - amount).max(0.0);
        }
    }

    /// True when the entity is carrying more than its capacity and enabled.
    pub fn is_overladen(&self) -> bool {
        self.enabled && self.current_load > self.max_load
    }

    /// Fraction of capacity filled [0.0 = empty, 1.0 = at limit].
    /// Clamped to [0.0, 1.0]; use `overload_ratio()` to measure excess.
    pub fn load_fraction(&self) -> f32 {
        if self.max_load <= 0.0 {
            return 1.0;
        }
        (self.current_load / self.max_load).clamp(0.0, 1.0)
    }

    /// How far over capacity the entity is, expressed as a fraction of `max_load`,
    /// clamped to [0.0, 1.0]. 0.0 means at or under capacity; 1.0 means carrying
    /// double the maximum load.
    pub fn overload_ratio(&self) -> f32 {
        if self.max_load <= 0.0 {
            return 1.0;
        }
        ((self.current_load - self.max_load) / self.max_load).clamp(0.0, 1.0)
    }

    /// Effective movement speed after encumbrance penalty.
    /// Returns `base * (1 - speed_penalty * overload_ratio())` while overladen
    /// and enabled, floored at 0.0. Returns `base` otherwise.
    pub fn effective_speed(&self, base: f32) -> f32 {
        if self.is_overladen() {
            (base * (1.0 - self.speed_penalty * self.overload_ratio())).max(0.0)
        } else {
            base
        }
    }
}

impl Default for Laden {
    fn default() -> Self {
        Self::new(50.0, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_load_increases_current_load() {
        let mut l = Laden::new(50.0, 0.5);
        l.add_load(30.0);
        assert!((l.current_load - 30.0).abs() < 1e-5);
    }

    #[test]
    fn add_load_zero_no_op() {
        let mut l = Laden::new(50.0, 0.5);
        l.add_load(0.0);
        assert!((l.current_load).abs() < 1e-5);
    }

    #[test]
    fn remove_load_decreases_current_load() {
        let mut l = Laden::new(50.0, 0.5);
        l.add_load(30.0);
        l.remove_load(10.0);
        assert!((l.current_load - 20.0).abs() < 1e-5);
    }

    #[test]
    fn remove_load_floors_at_zero() {
        let mut l = Laden::new(50.0, 0.5);
        l.add_load(10.0);
        l.remove_load(50.0);
        assert!((l.current_load).abs() < 1e-5);
    }

    #[test]
    fn is_overladen_false_under_capacity() {
        let mut l = Laden::new(50.0, 0.5);
        l.add_load(40.0);
        assert!(!l.is_overladen());
    }

    #[test]
    fn is_overladen_true_over_capacity() {
        let mut l = Laden::new(50.0, 0.5);
        l.add_load(60.0);
        assert!(l.is_overladen());
    }

    #[test]
    fn load_fraction_at_half() {
        let mut l = Laden::new(50.0, 0.5);
        l.add_load(25.0);
        assert!((l.load_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn load_fraction_capped_at_one_when_over() {
        let mut l = Laden::new(50.0, 0.5);
        l.add_load(100.0);
        assert!((l.load_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn overload_ratio_zero_when_at_capacity() {
        let mut l = Laden::new(50.0, 0.5);
        l.add_load(50.0);
        assert!((l.overload_ratio()).abs() < 1e-5);
    }

    #[test]
    fn overload_ratio_half_at_one_and_half_capacity() {
        let mut l = Laden::new(50.0, 0.5);
        l.add_load(75.0); // 25 over max, max=50 → ratio = 25/50 = 0.5
        assert!((l.overload_ratio() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn overload_ratio_capped_at_one() {
        let mut l = Laden::new(50.0, 0.5);
        l.add_load(200.0); // way over cap
        assert!((l.overload_ratio() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_speed_penalized_when_overladen() {
        let mut l = Laden::new(50.0, 0.5);
        l.add_load(75.0); // overload_ratio = 0.5; speed = 100 * (1 - 0.5*0.5) = 75
        assert!((l.effective_speed(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_speed_unaffected_under_capacity() {
        let mut l = Laden::new(50.0, 0.5);
        l.add_load(40.0);
        assert!((l.effective_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_speed_floored_at_zero() {
        let mut l = Laden::new(50.0, 1.0); // full penalty
        l.add_load(200.0); // ratio = 1.0; speed = 100 * (1 - 1.0*1.0) = 0
        assert!((l.effective_speed(100.0)).abs() < 1e-3);
    }

    #[test]
    fn disabled_is_not_overladen() {
        let mut l = Laden::new(50.0, 0.5);
        l.add_load(60.0);
        l.enabled = false;
        assert!(!l.is_overladen());
    }

    #[test]
    fn disabled_effective_speed_unaffected() {
        let mut l = Laden::new(50.0, 0.5);
        l.add_load(60.0);
        l.enabled = false;
        assert!((l.effective_speed(100.0) - 100.0).abs() < 1e-5);
    }
}
