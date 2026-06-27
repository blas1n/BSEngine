use bevy_ecs::prelude::Component;

/// Pure event-driven longing accumulator. Models tension that builds from
/// repeated failures or unfulfilled desires and persists until consciously
/// spent via `release()`.
///
/// Unlike time-based components (`Yowl` decays passively, `Worm` builds
/// passively), Wist has **no time-based logic at all** — `tick(dt)` only
/// clears one-frame flags. Wist must be built by explicit `yearn()` calls and
/// consumed by an explicit `release()` call. The tension is permanent until
/// spent, which gives players a reliable expectation.
///
/// `yearn(amount)` adds `amount` to `wist_level` (clamped to `max_wist`),
/// fires `just_yearned`. Fires `just_peaked` the first time `wist_level`
/// reaches `max_wist`. No-op when disabled or amount <= 0.
///
/// `release()` resets `wist_level` to 0 and fires `just_released`. No-op
/// when already at 0 or disabled.
///
/// `tick(dt)` clears all one-frame flags. No other state changes.
///
/// `is_peaked()` returns `wist_level >= max_wist && enabled`.
///
/// `wist_fraction()` returns `(wist_level / max_wist).clamp(0.0, 1.0)`.
///
/// `effective_longing(base)` returns `base * (1.0 + wist_fraction())` when
/// enabled — up to 2× at full wist; `base` unchanged when disabled.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wist {
    /// Accumulated longing [0, max_wist].
    pub wist_level: f32,
    /// Maximum longing capacity. Clamped >= 1.0.
    pub max_wist: f32,
    pub just_yearned: bool,
    pub just_peaked: bool,
    pub just_released: bool,
    pub enabled: bool,
}

impl Wist {
    pub fn new(max_wist: f32) -> Self {
        Self {
            wist_level: 0.0,
            max_wist: max_wist.max(1.0),
            just_yearned: false,
            just_peaked: false,
            just_released: false,
            enabled: true,
        }
    }

    /// Add `amount` of longing. Fires `just_yearned`. Fires `just_peaked`
    /// on first reaching max. No-op when disabled or `amount <= 0.0`.
    pub fn yearn(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let prev = self.wist_level;
        self.wist_level = (self.wist_level + amount).min(self.max_wist);
        self.just_yearned = true;
        if prev < self.max_wist && self.wist_level >= self.max_wist {
            self.just_peaked = true;
        }
    }

    /// Consume all longing: reset `wist_level` to 0 and fire `just_released`.
    /// No-op when already at 0 or disabled.
    pub fn release(&mut self) {
        if !self.enabled || self.wist_level == 0.0 {
            return;
        }
        self.wist_level = 0.0;
        self.just_released = true;
    }

    /// Advance one frame: clear one-frame flags only. No time-based changes.
    pub fn tick(&mut self, _dt: f32) {
        self.just_yearned = false;
        self.just_peaked = false;
        self.just_released = false;
    }

    /// `true` when wist is at maximum and component is enabled.
    pub fn is_peaked(&self) -> bool {
        self.wist_level >= self.max_wist && self.enabled
    }

    /// Longing as a fraction of maximum [0.0, 1.0].
    pub fn wist_fraction(&self) -> f32 {
        (self.wist_level / self.max_wist).clamp(0.0, 1.0)
    }

    /// Scale `base` by longing. Returns `base * (1.0 + wist_fraction())`
    /// when enabled (up to 2× at peak); `base` otherwise.
    pub fn effective_longing(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.wist_fraction())
    }
}

impl Default for Wist {
    fn default() -> Self {
        Self::new(10.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wist {
        Wist::new(10.0)
    }

    #[test]
    fn new_starts_empty() {
        let w = w();
        assert_eq!(w.wist_level, 0.0);
        assert!(!w.just_yearned);
        assert!(!w.just_peaked);
        assert!(!w.just_released);
        assert!(!w.is_peaked());
    }

    #[test]
    fn yearn_increases_level() {
        let mut w = w();
        w.yearn(4.0);
        assert!((w.wist_level - 4.0).abs() < 1e-4);
    }

    #[test]
    fn yearn_fires_just_yearned() {
        let mut w = w();
        w.yearn(1.0);
        assert!(w.just_yearned);
    }

    #[test]
    fn yearn_clamps_to_max() {
        let mut w = w(); // max=10
        w.yearn(15.0);
        assert!((w.wist_level - 10.0).abs() < 1e-4);
    }

    #[test]
    fn yearn_fires_just_peaked_at_max() {
        let mut w = w();
        w.yearn(10.0);
        assert!(w.just_peaked);
    }

    #[test]
    fn yearn_fires_just_peaked_crossing_max() {
        let mut w = w();
        w.yearn(7.0);
        w.tick(0.016); // clear flags
        w.yearn(5.0); // crosses 10.0
        assert!(w.just_peaked);
    }

    #[test]
    fn yearn_does_not_fire_just_peaked_when_already_at_max() {
        let mut w = w();
        w.yearn(10.0); // peaks
        w.tick(0.016); // clear flags
        w.yearn(1.0); // already at max, no new crossing
        assert!(!w.just_peaked);
        assert!(w.just_yearned); // yearned still fires (amount > 0, enabled)
    }

    #[test]
    fn yearn_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.yearn(5.0);
        assert_eq!(w.wist_level, 0.0);
        assert!(!w.just_yearned);
    }

    #[test]
    fn yearn_no_op_when_amount_zero() {
        let mut w = w();
        w.yearn(0.0);
        assert_eq!(w.wist_level, 0.0);
        assert!(!w.just_yearned);
    }

    #[test]
    fn yearn_no_op_when_amount_negative() {
        let mut w = w();
        w.yearn(-5.0);
        assert_eq!(w.wist_level, 0.0);
        assert!(!w.just_yearned);
    }

    #[test]
    fn release_resets_level() {
        let mut w = w();
        w.yearn(7.0);
        w.release();
        assert_eq!(w.wist_level, 0.0);
    }

    #[test]
    fn release_fires_just_released() {
        let mut w = w();
        w.yearn(5.0);
        w.release();
        assert!(w.just_released);
    }

    #[test]
    fn release_no_op_when_already_zero() {
        let mut w = w();
        w.release();
        assert!(!w.just_released);
    }

    #[test]
    fn release_no_op_when_disabled() {
        let mut w = w();
        w.yearn(5.0);
        w.enabled = false;
        w.release();
        assert!((w.wist_level - 5.0).abs() < 1e-4);
        assert!(!w.just_released);
    }

    #[test]
    fn tick_clears_just_yearned() {
        let mut w = w();
        w.yearn(1.0);
        w.tick(0.016);
        assert!(!w.just_yearned);
    }

    #[test]
    fn tick_clears_just_peaked() {
        let mut w = w();
        w.yearn(10.0);
        w.tick(0.016);
        assert!(!w.just_peaked);
    }

    #[test]
    fn tick_clears_just_released() {
        let mut w = w();
        w.yearn(5.0);
        w.release();
        w.tick(0.016);
        assert!(!w.just_released);
    }

    #[test]
    fn tick_does_not_decay_level() {
        let mut w = w();
        w.yearn(8.0);
        w.tick(1000.0); // no time-based decay
        assert!((w.wist_level - 8.0).abs() < 1e-4);
    }

    #[test]
    fn tick_does_not_decay_when_disabled() {
        let mut w = w();
        w.yearn(8.0);
        w.enabled = false;
        w.tick(1000.0);
        assert!((w.wist_level - 8.0).abs() < 1e-4);
    }

    #[test]
    fn is_peaked_true_at_max() {
        let mut w = w();
        w.yearn(10.0);
        assert!(w.is_peaked());
    }

    #[test]
    fn is_peaked_false_below_max() {
        let mut w = w();
        w.yearn(9.9);
        assert!(!w.is_peaked());
    }

    #[test]
    fn is_peaked_false_when_disabled() {
        let mut w = w();
        w.yearn(10.0);
        w.enabled = false;
        assert!(!w.is_peaked());
    }

    #[test]
    fn wist_fraction_zero_when_empty() {
        let w = w();
        assert_eq!(w.wist_fraction(), 0.0);
    }

    #[test]
    fn wist_fraction_half_at_midpoint() {
        let mut w = w(); // max=10
        w.yearn(5.0); // 5/10=0.5
        assert!((w.wist_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn wist_fraction_one_at_max() {
        let mut w = w();
        w.yearn(10.0);
        assert!((w.wist_fraction() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn effective_longing_passthrough_when_empty() {
        let w = w();
        assert!((w.effective_longing(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_longing_scaled_at_half() {
        let mut w = w();
        w.yearn(5.0); // fraction=0.5 → 100*(1+0.5)=150
        assert!((w.effective_longing(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_longing_doubled_at_max() {
        let mut w = w();
        w.yearn(10.0); // fraction=1.0 → 100*(1+1.0)=200
        assert!((w.effective_longing(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn effective_longing_passthrough_when_disabled() {
        let mut w = w();
        w.yearn(10.0);
        w.enabled = false;
        assert!((w.effective_longing(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn max_wist_clamped_to_one() {
        let w = Wist::new(0.0);
        assert!((w.max_wist - 1.0).abs() < 1e-5);
    }

    #[test]
    fn yearn_release_cycle() {
        let mut w = w();
        w.yearn(6.0);
        w.release();
        assert_eq!(w.wist_level, 0.0);
        w.yearn(3.0);
        assert!((w.wist_level - 3.0).abs() < 1e-4);
    }

    #[test]
    fn multiple_yearns_accumulate() {
        let mut w = w();
        w.yearn(3.0);
        w.yearn(2.0); // 5
        w.yearn(2.0); // 7
        assert!((w.wist_level - 7.0).abs() < 1e-4);
    }

    #[test]
    fn wist_persists_across_many_ticks() {
        let mut w = w();
        w.yearn(5.0);
        for _ in 0..100 {
            w.tick(1.0);
        }
        assert!((w.wist_level - 5.0).abs() < 1e-4);
    }
}
