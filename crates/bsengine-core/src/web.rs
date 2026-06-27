use bevy_ecs::prelude::Component;

/// Sticky projectile residue that progressively restricts an entity's
/// movement. Systems apply web material via `apply(amount)` and allow the
/// entity to break free via `struggle(amount)`. Movement cost scales with
/// current web coverage.
///
/// `apply(amount)` increases `web_strength` by `amount` (capped at
/// `max_strength`). Fires `just_caught` on the first positive transition from
/// 0.0. No-op when disabled or `amount <= 0`.
///
/// `struggle(amount)` reduces `web_strength` by `amount` (floored 0). Fires
/// `just_broken` when `web_strength` reaches 0.0 from positive. No-op when
/// disabled or `amount <= 0`.
///
/// `tick(dt)` clears `just_caught` and `just_broken`. No-op when disabled.
///
/// `is_caught()` returns `web_strength > 0.0 && enabled`.
///
/// `web_fraction()` returns `(web_strength / max_strength).clamp(0.0, 1.0)`.
///
/// `effective_speed(base)` returns
/// `(base * (1.0 - movement_penalty * web_fraction())).max(0.0)` when
/// enabled; returns `base` unchanged otherwise.
///
/// Distinct from `Entangle` (plant-root immobilisation), `Snare`
/// (ground-based trigger trap), `Root` (instant-full-stop effect), and
/// `Hobble` (leg-wound limping penalty): Web is a **projectile-deposited
/// adhesive** that stacks with repeated hits, degrades via explicit struggle,
/// and scales speed proportionally to coverage rather than imposing a fixed
/// stun or root.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Web {
    /// Current adhesive coverage [0.0, max_strength].
    pub web_strength: f32,
    /// Maximum web that can accumulate before full entrapment. Clamped >= 1.0.
    pub max_strength: f32,
    /// Movement reduction at full coverage [0.0, 1.0]. Clamped.
    pub movement_penalty: f32,
    pub just_caught: bool,
    pub just_broken: bool,
    pub enabled: bool,
}

impl Web {
    pub fn new(max_strength: f32, movement_penalty: f32) -> Self {
        Self {
            web_strength: 0.0,
            max_strength: max_strength.max(1.0),
            movement_penalty: movement_penalty.clamp(0.0, 1.0),
            just_caught: false,
            just_broken: false,
            enabled: true,
        }
    }

    /// Apply web material. Fires `just_caught` on first positive transition
    /// from 0.0. No-op when disabled or `amount <= 0`.
    pub fn apply(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_zero = self.web_strength == 0.0;
        self.web_strength = (self.web_strength + amount).min(self.max_strength);
        if was_zero && self.web_strength > 0.0 {
            self.just_caught = true;
        }
    }

    /// Break free from web coverage. Fires `just_broken` when `web_strength`
    /// reaches 0.0 from positive. No-op when disabled or `amount <= 0`.
    pub fn struggle(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_positive = self.web_strength > 0.0;
        self.web_strength = (self.web_strength - amount).max(0.0);
        if was_positive && self.web_strength == 0.0 {
            self.just_broken = true;
        }
    }

    /// Advance one frame: clear one-frame flags. No-op when disabled.
    pub fn tick(&mut self, _dt: f32) {
        self.just_caught = false;
        self.just_broken = false;
    }

    /// `true` when any web coverage remains and the component is enabled.
    pub fn is_caught(&self) -> bool {
        self.web_strength > 0.0 && self.enabled
    }

    /// Web coverage as a fraction of maximum [0.0, 1.0].
    pub fn web_fraction(&self) -> f32 {
        (self.web_strength / self.max_strength).clamp(0.0, 1.0)
    }

    /// Scale movement `base` by remaining mobility. Returns
    /// `(base * (1 - penalty * fraction)).max(0)` when enabled; `base`
    /// otherwise.
    pub fn effective_speed(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        (base * (1.0 - self.movement_penalty * self.web_fraction())).max(0.0)
    }
}

impl Default for Web {
    fn default() -> Self {
        Self::new(10.0, 0.8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Web {
        Web::new(10.0, 0.5)
    }

    #[test]
    fn new_starts_free() {
        let w = Web::new(10.0, 0.5);
        assert_eq!(w.web_strength, 0.0);
        assert!(!w.is_caught());
        assert!(!w.just_caught);
    }

    #[test]
    fn apply_increases_web_strength() {
        let mut w = w();
        w.apply(4.0);
        assert!((w.web_strength - 4.0).abs() < 1e-5);
    }

    #[test]
    fn apply_caps_at_max_strength() {
        let mut w = w();
        w.apply(20.0);
        assert!((w.web_strength - 10.0).abs() < 1e-5);
    }

    #[test]
    fn apply_fires_just_caught_from_zero() {
        let mut w = w();
        w.apply(3.0);
        assert!(w.just_caught);
        assert!(w.is_caught());
    }

    #[test]
    fn apply_no_just_caught_when_already_caught() {
        let mut w = w();
        w.apply(3.0); // just_caught fires
        w.tick(0.016); // clear
        w.apply(2.0); // already caught, no re-fire
        assert!(!w.just_caught);
    }

    #[test]
    fn apply_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.apply(5.0);
        assert_eq!(w.web_strength, 0.0);
    }

    #[test]
    fn apply_no_op_when_amount_zero() {
        let mut w = w();
        w.apply(0.0);
        assert_eq!(w.web_strength, 0.0);
    }

    #[test]
    fn apply_no_op_when_amount_negative() {
        let mut w = w();
        w.apply(-1.0);
        assert_eq!(w.web_strength, 0.0);
    }

    #[test]
    fn struggle_reduces_web_strength() {
        let mut w = w();
        w.apply(8.0);
        w.struggle(3.0);
        assert!((w.web_strength - 5.0).abs() < 1e-5);
    }

    #[test]
    fn struggle_floors_at_zero() {
        let mut w = w();
        w.apply(3.0);
        w.struggle(10.0);
        assert_eq!(w.web_strength, 0.0);
    }

    #[test]
    fn struggle_fires_just_broken_at_zero() {
        let mut w = w();
        w.apply(5.0);
        w.tick(0.016); // clear just_caught
        w.struggle(5.0); // reaches exactly 0
        assert!(w.just_broken);
        assert!(!w.is_caught());
    }

    #[test]
    fn struggle_fires_just_broken_when_overshot() {
        let mut w = w();
        w.apply(3.0);
        w.tick(0.016);
        w.struggle(10.0); // overshoots, floors at 0
        assert!(w.just_broken);
    }

    #[test]
    fn struggle_no_just_broken_when_strength_remains() {
        let mut w = w();
        w.apply(8.0);
        w.tick(0.016);
        w.struggle(3.0); // 8 → 5, still caught
        assert!(!w.just_broken);
    }

    #[test]
    fn struggle_no_op_when_disabled() {
        let mut w = w();
        w.apply(6.0);
        w.enabled = false;
        w.struggle(3.0);
        assert!((w.web_strength - 6.0).abs() < 1e-5);
    }

    #[test]
    fn struggle_no_op_when_amount_zero() {
        let mut w = w();
        w.apply(5.0);
        w.struggle(0.0);
        assert!((w.web_strength - 5.0).abs() < 1e-5);
    }

    #[test]
    fn struggle_no_op_when_amount_negative() {
        let mut w = w();
        w.apply(5.0);
        w.struggle(-1.0);
        assert!((w.web_strength - 5.0).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_just_caught() {
        let mut w = w();
        w.apply(4.0);
        w.tick(0.016);
        assert!(!w.just_caught);
    }

    #[test]
    fn tick_clears_just_broken() {
        let mut w = w();
        w.apply(3.0);
        w.tick(0.016);
        w.struggle(3.0);
        w.tick(0.016);
        assert!(!w.just_broken);
    }

    #[test]
    fn tick_clears_flags_even_when_disabled() {
        let mut w = w();
        w.apply(4.0);
        w.enabled = false;
        w.tick(0.016);
        assert!(!w.just_caught);
    }

    #[test]
    fn is_caught_true_when_strength_positive() {
        let mut w = w();
        w.apply(1.0);
        assert!(w.is_caught());
    }

    #[test]
    fn is_caught_false_when_no_web() {
        let w = w();
        assert!(!w.is_caught());
    }

    #[test]
    fn is_caught_false_when_disabled() {
        let mut w = w();
        w.apply(5.0);
        w.enabled = false;
        assert!(!w.is_caught());
    }

    #[test]
    fn web_fraction_zero_when_free() {
        let w = w();
        assert_eq!(w.web_fraction(), 0.0);
    }

    #[test]
    fn web_fraction_half_at_midpoint() {
        let mut w = w();
        w.apply(5.0);
        assert!((w.web_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn web_fraction_one_at_max() {
        let mut w = w();
        w.apply(10.0);
        assert!((w.web_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_speed_unpenalized_when_free() {
        let w = Web::new(10.0, 0.5);
        assert!((w.effective_speed(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_speed_half_penalty_at_full_web_with_0_5_penalty() {
        let mut w = Web::new(10.0, 0.5);
        w.apply(10.0);
        assert!((w.effective_speed(100.0) - 50.0).abs() < 1e-4);
    }

    #[test]
    fn effective_speed_fully_stopped_at_full_web_with_1_0_penalty() {
        let mut w = Web::new(10.0, 1.0);
        w.apply(10.0);
        assert!((w.effective_speed(100.0) - 0.0).abs() < 1e-4);
    }

    #[test]
    fn effective_speed_partial_at_half_coverage() {
        let mut w = Web::new(10.0, 0.5);
        w.apply(5.0); // fraction = 0.5
                      // 100 * (1 - 0.5*0.5) = 75
        assert!((w.effective_speed(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_speed_passthrough_when_disabled() {
        let mut w = Web::new(10.0, 1.0);
        w.apply(10.0);
        w.enabled = false;
        assert!((w.effective_speed(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_speed_floored_at_zero() {
        let mut w = Web::new(10.0, 1.0);
        w.apply(10.0);
        assert!(w.effective_speed(100.0) >= 0.0);
    }

    #[test]
    fn max_strength_clamped_to_one() {
        let w = Web::new(0.0, 0.5);
        assert!((w.max_strength - 1.0).abs() < 1e-5);
    }

    #[test]
    fn movement_penalty_clamped_high() {
        let w = Web::new(10.0, 2.0);
        assert!((w.movement_penalty - 1.0).abs() < 1e-5);
    }

    #[test]
    fn movement_penalty_clamped_low() {
        let w = Web::new(10.0, -1.0);
        assert_eq!(w.movement_penalty, 0.0);
    }

    #[test]
    fn multiple_applies_stack() {
        let mut w = w();
        w.apply(2.0);
        w.apply(3.0);
        w.apply(1.5);
        assert!((w.web_strength - 6.5).abs() < 1e-4);
    }

    #[test]
    fn apply_struggle_reapply_cycle() {
        let mut w = w();
        w.apply(8.0); // just_caught
        w.tick(0.016);
        w.struggle(8.0); // just_broken
        assert!(w.just_broken);
        w.tick(0.016);
        w.apply(5.0); // just_caught again
        assert!(w.just_caught);
        assert!(w.is_caught());
    }

    #[test]
    fn struggle_multiple_partial() {
        let mut w = w();
        w.apply(10.0);
        w.struggle(3.0); // 10 → 7
        w.struggle(3.0); // 7 → 4
        w.struggle(3.0); // 4 → 1
        w.struggle(3.0); // 1 → 0, just_broken
        assert!(w.just_broken);
        assert!(!w.is_caught());
    }
}
