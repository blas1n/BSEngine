use bevy_ecs::prelude::Component;

/// Temperature-recovery tracker that eases freeze/chill penalties as an
/// entity warms back up. `thaw_fraction` starts at 1.0 (fully warm); each
/// call to `apply_freeze(intensity)` drains it toward 0.0; `tick(dt)`
/// recovers it at `thaw_rate` per second. Movement penalties scale linearly
/// with `freeze_depth()` (= 1.0 − `thaw_fraction`).
///
/// `apply_freeze(intensity)` reduces `thaw_fraction` by `intensity`
/// (floored at 0.0). Fires `just_frozen` on the first tick the fraction
/// reaches 0.0. No-op when disabled or `intensity ≤ 0`.
///
/// `tick(dt)` clears one-frame flags at start; increments `thaw_fraction`
/// by `thaw_rate * dt` (capped at 1.0); fires `just_thawed` on the first
/// tick that `thaw_fraction` reaches 1.0 from below. No-op when disabled.
///
/// `is_frozen()` returns `thaw_fraction == 0.0 && enabled`.
///
/// `is_thawed()` returns `thaw_fraction >= 1.0` (pure query, no enabled check
/// — used to ask whether the entity has fully warmed up).
///
/// `freeze_depth()` returns `(1.0 - thaw_fraction).clamp(0.0, 1.0)`.
///
/// `effective_move_speed(base)` returns
/// `base * (1.0 - freeze_penalty * freeze_depth())` when enabled, floored at
/// 0.0; returns `base` when disabled.
///
/// Distinct from `Freeze` (binary freeze/stun that interrupts actions),
/// `Chill` (progressive chill-stack buildup), and
/// `Frostbite` (cold damage over time): Thaw is the **temperature recovery
/// curve** — it models how quickly an entity warms up after being chilled or
/// frozen, with a continuous penalty rather than a binary on/off state.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Thaw {
    /// Current warmth fraction [0.0 = frozen solid, 1.0 = fully warm].
    pub thaw_fraction: f32,
    /// Warmth recovered per second while not being frozen. Clamped >= 0.0.
    pub thaw_rate: f32,
    /// Maximum movement speed reduction at full freeze (thaw_fraction = 0).
    /// Clamped [0.0, 1.0].
    pub freeze_penalty: f32,
    pub just_thawed: bool,
    pub just_frozen: bool,
    pub enabled: bool,
}

impl Thaw {
    pub fn new(thaw_rate: f32, freeze_penalty: f32) -> Self {
        Self {
            thaw_fraction: 1.0,
            thaw_rate: thaw_rate.max(0.0),
            freeze_penalty: freeze_penalty.clamp(0.0, 1.0),
            just_thawed: false,
            just_frozen: false,
            enabled: true,
        }
    }

    /// Cool the entity by `intensity` (drains `thaw_fraction` toward 0.0).
    /// Fires `just_frozen` on the first drain that reaches exactly 0.0.
    /// No-op when disabled or `intensity ≤ 0`.
    pub fn apply_freeze(&mut self, intensity: f32) {
        if !self.enabled || intensity <= 0.0 {
            return;
        }
        let prev = self.thaw_fraction;
        self.thaw_fraction = (self.thaw_fraction - intensity).max(0.0);
        if prev > 0.0 && self.thaw_fraction == 0.0 {
            self.just_frozen = true;
        }
    }

    /// Advance temperature recovery. Clears one-frame flags first; increments
    /// `thaw_fraction` by `thaw_rate * dt`; fires `just_thawed` on first
    /// reach of 1.0. No-op when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_thawed = false;
        self.just_frozen = false;

        if !self.enabled {
            return;
        }

        if self.thaw_fraction < 1.0 {
            let prev = self.thaw_fraction;
            self.thaw_fraction = (self.thaw_fraction + self.thaw_rate * dt).min(1.0);
            if prev < 1.0 && self.thaw_fraction >= 1.0 {
                self.just_thawed = true;
            }
        }
    }

    /// `true` when `thaw_fraction` is exactly 0.0 and the component is enabled.
    pub fn is_frozen(&self) -> bool {
        self.thaw_fraction == 0.0 && self.enabled
    }

    /// `true` when `thaw_fraction` has reached 1.0 (fully warm).
    /// Independent of enabled state.
    pub fn is_thawed(&self) -> bool {
        self.thaw_fraction >= 1.0
    }

    /// Depth of freeze: `(1.0 - thaw_fraction).clamp(0.0, 1.0)`.
    pub fn freeze_depth(&self) -> f32 {
        (1.0 - self.thaw_fraction).clamp(0.0, 1.0)
    }

    /// Movement speed reduced by remaining freeze depth. Returns
    /// `base * (1.0 - freeze_penalty * freeze_depth())` when enabled, floored
    /// at 0.0. Returns `base` when disabled.
    pub fn effective_move_speed(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        (base * (1.0 - self.freeze_penalty * self.freeze_depth())).max(0.0)
    }
}

impl Default for Thaw {
    fn default() -> Self {
        Self::new(0.5, 0.6)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_fully_warm() {
        let t = Thaw::new(0.5, 0.6);
        assert!((t.thaw_fraction - 1.0).abs() < 1e-5);
        assert!(!t.is_frozen());
        assert!(t.is_thawed());
    }

    #[test]
    fn apply_freeze_drains_fraction() {
        let mut t = Thaw::new(0.5, 0.6);
        t.apply_freeze(0.4);
        assert!((t.thaw_fraction - 0.6).abs() < 1e-5);
    }

    #[test]
    fn apply_freeze_floors_at_zero() {
        let mut t = Thaw::new(0.5, 0.6);
        t.apply_freeze(5.0);
        assert_eq!(t.thaw_fraction, 0.0);
    }

    #[test]
    fn apply_freeze_fires_just_frozen_on_reaching_zero() {
        let mut t = Thaw::new(0.5, 0.6);
        t.apply_freeze(1.0);
        assert!(t.just_frozen);
        assert!(t.is_frozen());
    }

    #[test]
    fn apply_freeze_no_just_frozen_if_partial_drain() {
        let mut t = Thaw::new(0.5, 0.6);
        t.apply_freeze(0.5);
        assert!(!t.just_frozen);
    }

    #[test]
    fn apply_freeze_no_just_frozen_if_already_frozen() {
        let mut t = Thaw::new(0.0, 0.6); // thaw_rate=0 so stays frozen
        t.apply_freeze(1.0); // fully freeze
        t.tick(0.0); // clear flags
        t.apply_freeze(0.1); // already at 0; no transition
        assert!(!t.just_frozen);
    }

    #[test]
    fn apply_freeze_no_op_when_disabled() {
        let mut t = Thaw::new(0.5, 0.6);
        t.enabled = false;
        t.apply_freeze(0.5);
        assert!((t.thaw_fraction - 1.0).abs() < 1e-5);
    }

    #[test]
    fn apply_freeze_no_op_when_intensity_zero() {
        let mut t = Thaw::new(0.5, 0.6);
        t.apply_freeze(0.0);
        assert!((t.thaw_fraction - 1.0).abs() < 1e-5);
    }

    #[test]
    fn apply_freeze_no_op_when_intensity_negative() {
        let mut t = Thaw::new(0.5, 0.6);
        t.apply_freeze(-0.5);
        assert!((t.thaw_fraction - 1.0).abs() < 1e-5);
    }

    #[test]
    fn tick_recovers_fraction() {
        let mut t = Thaw::new(0.5, 0.6);
        t.apply_freeze(0.8); // fraction = 0.2
        t.tick(1.0); // +0.5 → 0.7
        assert!((t.thaw_fraction - 0.7).abs() < 1e-5);
    }

    #[test]
    fn tick_caps_at_one() {
        let mut t = Thaw::new(2.0, 0.6);
        t.apply_freeze(0.5); // fraction = 0.5
        t.tick(1.0); // +2.0 → capped at 1.0
        assert!((t.thaw_fraction - 1.0).abs() < 1e-5);
    }

    #[test]
    fn tick_fires_just_thawed_on_full_recovery() {
        let mut t = Thaw::new(1.0, 0.6);
        t.apply_freeze(0.5); // fraction = 0.5
        t.tick(0.5); // +0.5 → 1.0
        assert!(t.just_thawed);
    }

    #[test]
    fn tick_no_just_thawed_while_warming_but_not_full() {
        let mut t = Thaw::new(0.2, 0.6);
        t.apply_freeze(0.8);
        t.tick(1.0); // +0.2 → 0.4, not yet 1.0
        assert!(!t.just_thawed);
    }

    #[test]
    fn tick_no_just_thawed_when_already_at_one() {
        let mut t = Thaw::new(0.5, 0.6);
        // thaw_fraction = 1.0 already
        t.tick(1.0);
        assert!(!t.just_thawed);
    }

    #[test]
    fn tick_clears_just_frozen_next_frame() {
        let mut t = Thaw::new(0.0, 0.6);
        t.apply_freeze(1.0);
        assert!(t.just_frozen);
        t.tick(0.016); // cleared
        assert!(!t.just_frozen);
    }

    #[test]
    fn tick_clears_just_thawed_next_frame() {
        let mut t = Thaw::new(1.0, 0.6);
        t.apply_freeze(1.0); // fraction = 0.0
        t.tick(1.0); // just_thawed = true
        t.tick(0.016); // cleared
        assert!(!t.just_thawed);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut t = Thaw::new(0.5, 0.6);
        t.apply_freeze(1.0);
        t.enabled = false;
        t.tick(1.0);
        assert_eq!(t.thaw_fraction, 0.0); // no recovery
    }

    #[test]
    fn is_frozen_false_when_partial() {
        let mut t = Thaw::new(0.5, 0.6);
        t.apply_freeze(0.5);
        assert!(!t.is_frozen());
    }

    #[test]
    fn is_frozen_false_when_disabled() {
        let mut t = Thaw::new(0.5, 0.6);
        t.thaw_fraction = 0.0;
        t.enabled = false;
        assert!(!t.is_frozen());
    }

    #[test]
    fn is_thawed_true_when_at_one() {
        let t = Thaw::new(0.5, 0.6);
        assert!(t.is_thawed());
    }

    #[test]
    fn is_thawed_false_when_frozen() {
        let mut t = Thaw::new(0.0, 0.6);
        t.apply_freeze(1.0);
        assert!(!t.is_thawed());
    }

    #[test]
    fn is_thawed_independent_of_enabled() {
        let mut t = Thaw::new(0.5, 0.6);
        t.enabled = false;
        assert!(t.is_thawed()); // fraction=1.0
    }

    #[test]
    fn freeze_depth_at_zero_when_warm() {
        let t = Thaw::new(0.5, 0.6);
        assert_eq!(t.freeze_depth(), 0.0);
    }

    #[test]
    fn freeze_depth_at_one_when_frozen() {
        let mut t = Thaw::new(0.5, 0.6);
        t.apply_freeze(1.0);
        assert!((t.freeze_depth() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn freeze_depth_at_half() {
        let mut t = Thaw::new(0.5, 0.6);
        t.apply_freeze(0.5); // thaw = 0.5, depth = 0.5
        assert!((t.freeze_depth() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn effective_move_speed_full_when_warm() {
        let t = Thaw::new(0.5, 0.6);
        assert!((t.effective_move_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_move_speed_reduced_when_frozen() {
        let mut t = Thaw::new(0.5, 0.6);
        t.apply_freeze(1.0); // depth = 1.0
                             // 100 * (1 - 0.6 * 1.0) = 40
        assert!((t.effective_move_speed(100.0) - 40.0).abs() < 1e-3);
    }

    #[test]
    fn effective_move_speed_partial_freeze() {
        let mut t = Thaw::new(0.5, 0.4);
        t.apply_freeze(0.5); // depth = 0.5
                             // 100 * (1 - 0.4 * 0.5) = 80
        assert!((t.effective_move_speed(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_move_speed_base_when_disabled() {
        let mut t = Thaw::new(0.5, 0.6);
        t.apply_freeze(1.0);
        t.enabled = false;
        assert!((t.effective_move_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_move_speed_floored_at_zero() {
        let mut t = Thaw::new(0.5, 1.0);
        t.apply_freeze(1.0); // full freeze, full penalty
        assert_eq!(t.effective_move_speed(100.0), 0.0);
    }

    #[test]
    fn thaw_rate_clamped_to_zero() {
        let t = Thaw::new(-0.5, 0.6);
        assert_eq!(t.thaw_rate, 0.0);
    }

    #[test]
    fn freeze_penalty_clamped_to_one() {
        let t = Thaw::new(0.5, 2.0);
        assert!((t.freeze_penalty - 1.0).abs() < 1e-5);
    }

    #[test]
    fn freeze_penalty_clamped_to_zero() {
        let t = Thaw::new(0.5, -0.5);
        assert_eq!(t.freeze_penalty, 0.0);
    }

    #[test]
    fn repeated_freeze_and_thaw_cycle() {
        let mut t = Thaw::new(1.0, 0.6);
        t.apply_freeze(1.0); // freeze
        t.tick(1.0); // thaw → just_thawed
        assert!(t.is_thawed());
        assert!(t.just_thawed);
        t.apply_freeze(1.0); // freeze again
        assert!(t.is_frozen());
    }
}
