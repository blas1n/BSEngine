use bevy_ecs::prelude::Component;

/// Consumable protective coating that intercepts incoming damage before it
/// reaches the entity's health or other resources. Systems apply wax via
/// `apply(amount)` and route damage through `absorb(damage)`, which returns
/// the overflow that was not intercepted.
///
/// `apply(amount)` increases `wax_level` by `amount` (capped at `max_wax`).
/// Fires `just_applied` on the first positive transition from 0.0. No-op
/// when disabled or `amount <= 0`.
///
/// `absorb(damage)` reduces `wax_level` by up to `damage` and returns the
/// remaining (overflow) damage that was not absorbed. Fires `just_stripped`
/// when `wax_level` reaches 0.0 from positive. Returns `damage` unchanged
/// when disabled or wax_level is already 0.0.
///
/// `tick(dt)` clears `just_applied` and `just_stripped`.
///
/// `is_coated()` returns `wax_level > 0.0 && enabled`.
///
/// `wax_fraction()` returns `(wax_level / max_wax).clamp(0.0, 1.0)`.
///
/// Distinct from `Overshield` (regenerating extra-HP layer), `Shield` (fixed
/// HP pool for general incoming damage), `Barrier` (one-time directional
/// block), and `Absorb` (passive damage-type conversion): Wax models an
/// **applied consumable coating** — it is placed deliberately by spells,
/// environment, or crafting, is strictly consumed without regeneration, and
/// returns overflow so callers can route the remainder correctly.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wax {
    /// Current coating thickness [0.0, max_wax].
    pub wax_level: f32,
    /// Maximum coating that can be applied. Clamped >= 1.0.
    pub max_wax: f32,
    pub just_applied: bool,
    pub just_stripped: bool,
    pub enabled: bool,
}

impl Wax {
    pub fn new(max_wax: f32) -> Self {
        Self {
            wax_level: 0.0,
            max_wax: max_wax.max(1.0),
            just_applied: false,
            just_stripped: false,
            enabled: true,
        }
    }

    /// Add protective coating. Fires `just_applied` on first positive
    /// transition from 0.0. No-op when disabled or `amount <= 0`.
    pub fn apply(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_zero = self.wax_level == 0.0;
        self.wax_level = (self.wax_level + amount).min(self.max_wax);
        if was_zero && self.wax_level > 0.0 {
            self.just_applied = true;
        }
    }

    /// Intercept incoming `damage`. Reduces wax by up to `damage` and returns
    /// the overflow not absorbed. Fires `just_stripped` when wax reaches 0.0.
    /// Returns `damage` unchanged when disabled or wax is already at 0.0.
    pub fn absorb(&mut self, damage: f32) -> f32 {
        if !self.enabled || self.wax_level == 0.0 {
            return damage;
        }
        if damage <= 0.0 {
            return 0.0;
        }
        let absorbed = damage.min(self.wax_level);
        let was_positive = self.wax_level > 0.0;
        self.wax_level -= absorbed;
        if was_positive && self.wax_level == 0.0 {
            self.just_stripped = true;
        }
        damage - absorbed
    }

    /// Advance one frame: clear one-frame flags. No-op when disabled.
    pub fn tick(&mut self, _dt: f32) {
        self.just_applied = false;
        self.just_stripped = false;
    }

    /// `true` when any coating remains and the component is enabled.
    pub fn is_coated(&self) -> bool {
        self.wax_level > 0.0 && self.enabled
    }

    /// Coating thickness as a fraction of the maximum [0.0, 1.0].
    pub fn wax_fraction(&self) -> f32 {
        (self.wax_level / self.max_wax).clamp(0.0, 1.0)
    }
}

impl Default for Wax {
    fn default() -> Self {
        Self::new(10.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_uncoated() {
        let w = Wax::new(10.0);
        assert_eq!(w.wax_level, 0.0);
        assert!(!w.is_coated());
        assert!(!w.just_applied);
    }

    #[test]
    fn apply_adds_coating() {
        let mut w = Wax::new(10.0);
        w.apply(4.0);
        assert!((w.wax_level - 4.0).abs() < 1e-5);
    }

    #[test]
    fn apply_caps_at_max_wax() {
        let mut w = Wax::new(10.0);
        w.apply(20.0);
        assert!((w.wax_level - 10.0).abs() < 1e-5);
    }

    #[test]
    fn apply_fires_just_applied_from_zero() {
        let mut w = Wax::new(10.0);
        w.apply(3.0);
        assert!(w.just_applied);
        assert!(w.is_coated());
    }

    #[test]
    fn apply_no_just_applied_when_already_coated() {
        let mut w = Wax::new(10.0);
        w.apply(3.0); // just_applied fires
        w.tick(0.016); // clear
        w.apply(2.0); // already coated, no re-fire
        assert!(!w.just_applied);
    }

    #[test]
    fn apply_no_op_when_disabled() {
        let mut w = Wax::new(10.0);
        w.enabled = false;
        w.apply(5.0);
        assert_eq!(w.wax_level, 0.0);
    }

    #[test]
    fn apply_no_op_when_amount_zero() {
        let mut w = Wax::new(10.0);
        w.apply(0.0);
        assert_eq!(w.wax_level, 0.0);
    }

    #[test]
    fn apply_no_op_when_amount_negative() {
        let mut w = Wax::new(10.0);
        w.apply(-1.0);
        assert_eq!(w.wax_level, 0.0);
    }

    #[test]
    fn absorb_reduces_wax() {
        let mut w = Wax::new(10.0);
        w.apply(8.0);
        let overflow = w.absorb(3.0);
        assert!((w.wax_level - 5.0).abs() < 1e-5);
        assert_eq!(overflow, 0.0);
    }

    #[test]
    fn absorb_returns_overflow_when_wax_depleted() {
        let mut w = Wax::new(10.0);
        w.apply(4.0);
        let overflow = w.absorb(6.0); // 4 absorbed, 2 overflow
        assert_eq!(w.wax_level, 0.0);
        assert!((overflow - 2.0).abs() < 1e-5);
    }

    #[test]
    fn absorb_returns_full_damage_when_exactly_depleted() {
        let mut w = Wax::new(10.0);
        w.apply(5.0);
        let overflow = w.absorb(5.0); // exact wipe
        assert_eq!(w.wax_level, 0.0);
        assert_eq!(overflow, 0.0);
    }

    #[test]
    fn absorb_fires_just_stripped_when_wax_reaches_zero() {
        let mut w = Wax::new(10.0);
        w.apply(5.0);
        w.tick(0.016); // clear just_applied
        let _overflow = w.absorb(10.0); // strip all
        assert!(w.just_stripped);
        assert!(!w.is_coated());
    }

    #[test]
    fn absorb_no_just_stripped_when_wax_remains() {
        let mut w = Wax::new(10.0);
        w.apply(8.0);
        w.tick(0.016);
        w.absorb(3.0); // 8 → 5, still coated
        assert!(!w.just_stripped);
    }

    #[test]
    fn absorb_returns_damage_unchanged_when_disabled() {
        let mut w = Wax::new(10.0);
        w.apply(8.0);
        w.enabled = false;
        let returned = w.absorb(5.0);
        assert!((returned - 5.0).abs() < 1e-5);
        assert!((w.wax_level - 8.0).abs() < 1e-5); // unchanged
    }

    #[test]
    fn absorb_returns_damage_when_uncoated() {
        let mut w = Wax::new(10.0);
        let returned = w.absorb(7.0);
        assert!((returned - 7.0).abs() < 1e-5);
    }

    #[test]
    fn absorb_returns_zero_for_nonpositive_damage() {
        let mut w = Wax::new(10.0);
        w.apply(5.0);
        let r1 = w.absorb(0.0);
        let r2 = w.absorb(-3.0);
        assert_eq!(r1, 0.0);
        assert_eq!(r2, 0.0);
        assert!((w.wax_level - 5.0).abs() < 1e-5); // unchanged
    }

    #[test]
    fn tick_clears_just_applied() {
        let mut w = Wax::new(10.0);
        w.apply(4.0); // just_applied = true
        w.tick(0.016);
        assert!(!w.just_applied);
    }

    #[test]
    fn tick_clears_just_stripped() {
        let mut w = Wax::new(10.0);
        w.apply(3.0);
        w.tick(0.016);
        w.absorb(3.0); // just_stripped = true
        w.tick(0.016);
        assert!(!w.just_stripped);
    }

    #[test]
    fn tick_clears_flags_even_when_disabled() {
        let mut w = Wax::new(10.0);
        w.apply(4.0); // just_applied = true
        w.enabled = false;
        w.tick(0.016); // flags cleared regardless
        assert!(!w.just_applied);
    }

    #[test]
    fn is_coated_true_when_positive() {
        let mut w = Wax::new(10.0);
        w.apply(1.0);
        assert!(w.is_coated());
    }

    #[test]
    fn is_coated_false_when_empty() {
        let w = Wax::new(10.0);
        assert!(!w.is_coated());
    }

    #[test]
    fn is_coated_false_when_disabled() {
        let mut w = Wax::new(10.0);
        w.apply(5.0);
        w.enabled = false;
        assert!(!w.is_coated());
    }

    #[test]
    fn wax_fraction_zero_when_empty() {
        let w = Wax::new(10.0);
        assert_eq!(w.wax_fraction(), 0.0);
    }

    #[test]
    fn wax_fraction_half_at_midpoint() {
        let mut w = Wax::new(10.0);
        w.apply(5.0);
        assert!((w.wax_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn wax_fraction_one_at_max() {
        let mut w = Wax::new(10.0);
        w.apply(10.0);
        assert!((w.wax_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn max_wax_clamped_to_one() {
        let w = Wax::new(0.0);
        assert!((w.max_wax - 1.0).abs() < 1e-5);
    }

    #[test]
    fn multiple_applies_stack() {
        let mut w = Wax::new(10.0);
        w.apply(2.0);
        w.apply(3.0);
        w.apply(1.5);
        assert!((w.wax_level - 6.5).abs() < 1e-4);
    }

    #[test]
    fn apply_absorb_reapply_cycle() {
        let mut w = Wax::new(10.0);
        w.apply(8.0); // just_applied
        w.tick(0.016);
        let overflow = w.absorb(10.0); // stripped, 2 overflow
        assert!(w.just_stripped);
        assert!((overflow - 2.0).abs() < 1e-4);
        w.tick(0.016);
        w.apply(5.0); // just_applied again
        assert!(w.just_applied);
        assert!(w.is_coated());
    }

    #[test]
    fn absorb_partial_multiple_hits() {
        let mut w = Wax::new(10.0);
        w.apply(10.0);
        let o1 = w.absorb(3.0); // 10 → 7
        let o2 = w.absorb(3.0); // 7 → 4
        let o3 = w.absorb(3.0); // 4 → 1
        let o4 = w.absorb(3.0); // 1 → 0, overflow = 2
        assert_eq!(o1, 0.0);
        assert_eq!(o2, 0.0);
        assert_eq!(o3, 0.0);
        assert!((o4 - 2.0).abs() < 1e-4);
        assert!(!w.is_coated());
    }
}
