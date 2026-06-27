use bevy_ecs::prelude::Component;

/// Pack-size offensive bonus: the entity grows stronger as more allies fight
/// alongside it. `ally_count` is updated by the game system each frame via
/// `update(count)`, and `pack_bonus()` returns the resulting damage multiplier.
///
/// `update(count)` sets `ally_count` (clamped at `max_bonus_allies`). Fires
/// `just_peaked` on the first frame that `ally_count` reaches
/// `max_bonus_allies`, and `just_alone` on the first frame it drops to 0.
/// No-op when disabled or count is unchanged.
///
/// `tick()` clears `just_peaked` and `just_alone` each frame.
///
/// `pack_bonus()` returns `1.0 + bonus_per_ally * ally_count` when enabled;
/// returns `1.0` when disabled.
///
/// `is_alone()` returns `ally_count == 0 && enabled`.
///
/// `fill_fraction()` returns `(ally_count as f32 / max_bonus_allies as f32).clamp(0, 1)`.
///
/// Distinct from `Surround` (enemy encirclement giving *defensive* bonus),
/// `Morale` (group morale tracker driven by events), and `Aura` (stat emitter
/// for allies): Horde is a **pack-size offensive multiplier** — the entity
/// carrying this component personally benefits from having allies nearby, with
/// no concern for whose aura is emitting or what event caused the change.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Horde {
    /// Number of nearby allies currently counted (clamped at `max_bonus_allies`).
    pub ally_count: u32,
    /// Allies required for maximum bonus. Clamped ≥ 1.
    pub max_bonus_allies: u32,
    /// Damage multiplier increment per counted ally. Clamped ≥ 0.
    pub bonus_per_ally: f32,
    pub just_peaked: bool,
    pub just_alone: bool,
    pub enabled: bool,
}

impl Horde {
    pub fn new(max_bonus_allies: u32, bonus_per_ally: f32) -> Self {
        Self {
            ally_count: 0,
            max_bonus_allies: max_bonus_allies.max(1),
            bonus_per_ally: bonus_per_ally.max(0.0),
            just_peaked: false,
            just_alone: false,
            enabled: true,
        }
    }

    /// Update the ally count. Clamps `count` at `max_bonus_allies`. Fires
    /// `just_peaked` on the below-max → at-max transition and `just_alone` on
    /// the nonzero → zero transition. No-op when disabled or count is
    /// unchanged.
    pub fn update(&mut self, count: u32) {
        if !self.enabled {
            return;
        }
        let clamped = count.min(self.max_bonus_allies);
        if clamped == self.ally_count {
            return;
        }

        let was_at_max = self.ally_count >= self.max_bonus_allies;
        let was_alone = self.ally_count == 0;

        self.ally_count = clamped;

        if !was_at_max && self.ally_count >= self.max_bonus_allies {
            self.just_peaked = true;
        }
        if !was_alone && self.ally_count == 0 {
            self.just_alone = true;
        }
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_peaked = false;
        self.just_alone = false;
    }

    /// Cumulative outgoing damage multiplier from nearby allies.
    /// Returns `1.0 + bonus_per_ally * ally_count` when enabled; `1.0` when
    /// disabled.
    pub fn pack_bonus(&self) -> f32 {
        if !self.enabled {
            return 1.0;
        }
        1.0 + self.bonus_per_ally * self.ally_count as f32
    }

    /// `true` when no allies are nearby and the component is enabled.
    pub fn is_alone(&self) -> bool {
        self.ally_count == 0 && self.enabled
    }

    /// Pack fill fraction [0.0 = alone, 1.0 = max pack].
    pub fn fill_fraction(&self) -> f32 {
        (self.ally_count as f32 / self.max_bonus_allies as f32).clamp(0.0, 1.0)
    }
}

impl Default for Horde {
    fn default() -> Self {
        Self::new(5, 0.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_alone() {
        let h = Horde::new(5, 0.1);
        assert_eq!(h.ally_count, 0);
        assert!(h.is_alone());
    }

    #[test]
    fn update_sets_ally_count() {
        let mut h = Horde::new(5, 0.1);
        h.update(3);
        assert_eq!(h.ally_count, 3);
    }

    #[test]
    fn update_clamps_at_max_bonus_allies() {
        let mut h = Horde::new(5, 0.1);
        h.update(10);
        assert_eq!(h.ally_count, 5);
    }

    #[test]
    fn update_fires_just_peaked_at_max() {
        let mut h = Horde::new(3, 0.1);
        h.update(3);
        assert!(h.just_peaked);
    }

    #[test]
    fn update_no_just_peaked_when_already_at_max() {
        let mut h = Horde::new(3, 0.1);
        h.update(3); // peaks
        h.tick();
        h.update(4); // still at max (clamped to 3)
        assert!(!h.just_peaked);
    }

    #[test]
    fn update_no_just_peaked_below_max() {
        let mut h = Horde::new(5, 0.1);
        h.update(3);
        assert!(!h.just_peaked);
    }

    #[test]
    fn update_fires_just_alone_on_drop_to_zero() {
        let mut h = Horde::new(5, 0.1);
        h.update(3);
        h.tick();
        h.update(0);
        assert!(h.just_alone);
    }

    #[test]
    fn update_no_just_alone_when_already_at_zero() {
        let mut h = Horde::new(5, 0.1);
        h.update(0); // already 0
        assert!(!h.just_alone);
    }

    #[test]
    fn update_no_op_when_disabled() {
        let mut h = Horde::new(5, 0.1);
        h.enabled = false;
        h.update(3);
        assert_eq!(h.ally_count, 0);
    }

    #[test]
    fn update_no_op_when_count_unchanged() {
        let mut h = Horde::new(5, 0.1);
        h.update(3);
        h.tick();
        h.update(3); // same count
        assert!(!h.just_peaked);
        assert!(!h.just_alone);
    }

    #[test]
    fn tick_clears_just_peaked() {
        let mut h = Horde::new(3, 0.1);
        h.update(3); // peaks
        h.tick();
        assert!(!h.just_peaked);
    }

    #[test]
    fn tick_clears_just_alone() {
        let mut h = Horde::new(5, 0.1);
        h.update(2);
        h.tick();
        h.update(0); // alone
        h.tick();
        assert!(!h.just_alone);
    }

    #[test]
    fn pack_bonus_at_zero_allies() {
        let h = Horde::new(5, 0.1);
        assert!((h.pack_bonus() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn pack_bonus_scales_with_allies() {
        let mut h = Horde::new(5, 0.1);
        h.update(3);
        // 1 + 0.1 * 3 = 1.3
        assert!((h.pack_bonus() - 1.3).abs() < 1e-5);
    }

    #[test]
    fn pack_bonus_at_max_allies() {
        let mut h = Horde::new(5, 0.2);
        h.update(5);
        // 1 + 0.2 * 5 = 2.0
        assert!((h.pack_bonus() - 2.0).abs() < 1e-5);
    }

    #[test]
    fn pack_bonus_one_when_disabled() {
        let mut h = Horde::new(5, 0.1);
        h.update(5);
        h.enabled = false;
        assert!((h.pack_bonus() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn is_alone_true_at_zero() {
        let h = Horde::new(5, 0.1);
        assert!(h.is_alone());
    }

    #[test]
    fn is_alone_false_with_allies() {
        let mut h = Horde::new(5, 0.1);
        h.update(1);
        assert!(!h.is_alone());
    }

    #[test]
    fn is_alone_false_when_disabled() {
        let mut h = Horde::new(5, 0.1);
        h.enabled = false;
        assert!(!h.is_alone());
    }

    #[test]
    fn fill_fraction_at_zero() {
        let h = Horde::new(5, 0.1);
        assert_eq!(h.fill_fraction(), 0.0);
    }

    #[test]
    fn fill_fraction_at_half() {
        let mut h = Horde::new(4, 0.1);
        h.update(2);
        assert!((h.fill_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn fill_fraction_at_full() {
        let mut h = Horde::new(5, 0.1);
        h.update(5);
        assert!((h.fill_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn max_bonus_allies_clamped_to_one() {
        let h = Horde::new(0, 0.1);
        assert_eq!(h.max_bonus_allies, 1);
    }

    #[test]
    fn bonus_per_ally_clamped_non_negative() {
        let h = Horde::new(5, -0.5);
        assert_eq!(h.bonus_per_ally, 0.0);
    }

    #[test]
    fn zero_bonus_per_ally_always_returns_one() {
        let mut h = Horde::new(5, 0.0);
        h.update(5);
        assert!((h.pack_bonus() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn peaked_then_dropped_then_peaks_again() {
        let mut h = Horde::new(3, 0.1);
        h.update(3); // peaks
        h.tick();
        h.update(1); // drops
        h.tick();
        h.update(3); // peaks again
        assert!(h.just_peaked);
    }
}
