use bevy_ecs::prelude::Component;

/// Encirclement tracker: counts adjacent enemies and provides a defense bonus
/// when the entity is surrounded. Caller systems set `adjacent_count` via
/// `update()` each frame; this component fires transition events and exposes
/// damage-mitigation helpers.
///
/// `update(count)` sets `adjacent_count` to `count`. Fires `just_encircled`
/// on the transition from below to at-or-above `encircle_threshold`. Fires
/// `just_cleared` on the transition from at-or-above to below. No-op when
/// disabled or `count == adjacent_count`.
///
/// `tick()` clears `just_encircled` and `just_cleared` each frame.
///
/// `is_surrounded()` returns `adjacent_count >= encircle_threshold && enabled`.
///
/// `effective_damage(incoming)` returns `incoming * (1.0 - defense_bonus)`
/// when the entity is surrounded; returns `incoming` otherwise.
///
/// Distinct from `Armor` (static, always-active damage reduction), `Guard`
/// (player-activated block that reduces a single hit), and `Protect`
/// (redirects damage from an ally to self): Surround is a **situational
/// encirclement defense** — the bonus only applies when a threshold number
/// of enemies are adjacent, rewarding tactical awareness of positioning.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Surround {
    /// Current count of adjacent enemies (set by caller systems).
    pub adjacent_count: u32,
    /// Minimum adjacent enemies to be considered surrounded. Clamped ≥ 1.
    pub encircle_threshold: u32,
    /// Incoming damage reduction fraction when surrounded. Clamped [0.0, 1.0].
    pub defense_bonus: f32,
    pub just_encircled: bool,
    pub just_cleared: bool,
    pub enabled: bool,
}

impl Surround {
    pub fn new(encircle_threshold: u32, defense_bonus: f32) -> Self {
        Self {
            adjacent_count: 0,
            encircle_threshold: encircle_threshold.max(1),
            defense_bonus: defense_bonus.clamp(0.0, 1.0),
            just_encircled: false,
            just_cleared: false,
            enabled: true,
        }
    }

    /// Set the current adjacent enemy count. Fires `just_encircled` on the
    /// transition to surrounded and `just_cleared` on the transition away.
    /// No-op when disabled or when `count` equals `adjacent_count`.
    pub fn update(&mut self, count: u32) {
        if !self.enabled || count == self.adjacent_count {
            return;
        }
        let was_surrounded = self.is_surrounded();
        self.adjacent_count = count;
        let now_surrounded = self.is_surrounded();

        if !was_surrounded && now_surrounded {
            self.just_encircled = true;
        } else if was_surrounded && !now_surrounded {
            self.just_cleared = true;
        }
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_encircled = false;
        self.just_cleared = false;
    }

    /// `true` when enough enemies are adjacent and the component is enabled.
    pub fn is_surrounded(&self) -> bool {
        self.adjacent_count >= self.encircle_threshold && self.enabled
    }

    /// Incoming damage after applying the encirclement defense bonus.
    /// Returns `incoming * (1.0 - defense_bonus)` when surrounded; returns
    /// `incoming` otherwise.
    pub fn effective_damage(&self, incoming: f32) -> f32 {
        if self.is_surrounded() {
            (incoming * (1.0 - self.defense_bonus)).max(0.0)
        } else {
            incoming
        }
    }

    /// Fraction of slots filled relative to threshold [0.0 = none, 1.0+ = surrounded].
    /// Useful for UI fill bars. Clamped to [0.0, 1.0].
    pub fn fill_fraction(&self) -> f32 {
        (self.adjacent_count as f32 / self.encircle_threshold as f32).clamp(0.0, 1.0)
    }
}

impl Default for Surround {
    fn default() -> Self {
        Self::new(3, 0.25)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_zero() {
        let s = Surround::new(3, 0.25);
        assert_eq!(s.adjacent_count, 0);
        assert!(!s.is_surrounded());
    }

    #[test]
    fn update_sets_count() {
        let mut s = Surround::new(3, 0.25);
        s.update(2);
        assert_eq!(s.adjacent_count, 2);
    }

    #[test]
    fn update_fires_just_encircled_on_transition() {
        let mut s = Surround::new(3, 0.25);
        s.update(2);
        assert!(!s.just_encircled);
        s.update(3); // hits threshold
        assert!(s.just_encircled);
        assert!(s.is_surrounded());
    }

    #[test]
    fn update_fires_just_cleared_on_drop() {
        let mut s = Surround::new(3, 0.25);
        s.update(3); // encircled
        s.tick();
        s.update(2); // drops below
        assert!(s.just_cleared);
        assert!(!s.is_surrounded());
    }

    #[test]
    fn update_no_just_encircled_when_already_surrounded() {
        let mut s = Surround::new(3, 0.25);
        s.update(3); // encircled
        s.tick();
        s.update(5); // still surrounded
        assert!(!s.just_encircled);
        assert!(!s.just_cleared);
    }

    #[test]
    fn update_no_just_cleared_when_not_surrounded() {
        let mut s = Surround::new(3, 0.25);
        s.update(1);
        s.tick();
        s.update(2); // still below threshold
        assert!(!s.just_cleared);
    }

    #[test]
    fn update_no_op_when_same_count() {
        let mut s = Surround::new(3, 0.25);
        s.update(3); // encircled
        s.tick();
        s.update(3); // same — no-op
        assert!(!s.just_encircled);
        assert!(!s.just_cleared);
    }

    #[test]
    fn update_no_op_when_disabled() {
        let mut s = Surround::new(3, 0.25);
        s.enabled = false;
        s.update(5);
        assert_eq!(s.adjacent_count, 0);
    }

    #[test]
    fn tick_clears_just_encircled() {
        let mut s = Surround::new(3, 0.25);
        s.update(3);
        s.tick();
        assert!(!s.just_encircled);
    }

    #[test]
    fn tick_clears_just_cleared() {
        let mut s = Surround::new(3, 0.25);
        s.update(3);
        s.tick();
        s.update(1);
        s.tick();
        assert!(!s.just_cleared);
    }

    #[test]
    fn is_surrounded_false_when_disabled() {
        let mut s = Surround::new(3, 0.25);
        s.adjacent_count = 5;
        s.enabled = false;
        assert!(!s.is_surrounded());
    }

    #[test]
    fn is_surrounded_true_at_exact_threshold() {
        let mut s = Surround::new(3, 0.25);
        s.update(3);
        assert!(s.is_surrounded());
    }

    #[test]
    fn effective_damage_reduced_when_surrounded() {
        let mut s = Surround::new(3, 0.25);
        s.update(3);
        // 100 * (1 - 0.25) = 75
        assert!((s.effective_damage(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_damage_full_when_not_surrounded() {
        let s = Surround::new(3, 0.25);
        assert!((s.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_damage_full_when_disabled() {
        let mut s = Surround::new(3, 0.25);
        s.adjacent_count = 5;
        s.enabled = false;
        assert!((s.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_damage_floored_at_zero() {
        let mut s = Surround::new(3, 1.0);
        s.update(3);
        assert_eq!(s.effective_damage(100.0), 0.0);
    }

    #[test]
    fn fill_fraction_at_zero() {
        let s = Surround::new(4, 0.25);
        assert!(s.fill_fraction().abs() < 1e-5);
    }

    #[test]
    fn fill_fraction_at_half() {
        let mut s = Surround::new(4, 0.25);
        s.adjacent_count = 2;
        assert!((s.fill_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn fill_fraction_clamped_at_one() {
        let mut s = Surround::new(3, 0.25);
        s.adjacent_count = 10;
        assert!((s.fill_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn encircle_threshold_clamped_to_one() {
        let s = Surround::new(0, 0.25);
        assert_eq!(s.encircle_threshold, 1);
    }

    #[test]
    fn defense_bonus_clamped_at_one() {
        let s = Surround::new(3, 2.0);
        assert!((s.defense_bonus - 1.0).abs() < 1e-5);
    }

    #[test]
    fn defense_bonus_clamped_at_zero() {
        let s = Surround::new(3, -0.5);
        assert_eq!(s.defense_bonus, 0.0);
    }

    #[test]
    fn re_encircle_after_clear() {
        let mut s = Surround::new(3, 0.25);
        s.update(3); // encircled
        s.tick();
        s.update(1); // cleared
        s.tick();
        s.update(4); // re-encircled
        assert!(s.just_encircled);
    }

    #[test]
    fn zero_defense_bonus_no_reduction() {
        let mut s = Surround::new(3, 0.0);
        s.update(3);
        assert!((s.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }
}
