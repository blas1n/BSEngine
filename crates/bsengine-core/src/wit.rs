use bevy_ecs::prelude::Component;

/// Quickness-acuity accumulation tracker named after wit, the
/// noun meaning mental sharpness and inventiveness; the ability
/// to perceive and express things in an amusing and clever way;
/// (archaic) mind, intelligence, or understanding — from the
/// Old English witt (mind, intelligence, understanding, sense),
/// from the Proto-Germanic witją (knowledge, understanding),
/// from the Proto-Indo-European root weid- (to see, to know).
/// Wit shares its root with wise, witness, vision, and idea —
/// all words about knowing through perceiving. In its oldest
/// English uses, wit was simply mind or intelligence: the
/// five wits were the five senses, and to have one's wits
/// about one was to be mentally present and alert. The
/// narrowing to the specifically quick and verbal kind of
/// intelligence — the ability to make unexpected connections
/// rapidly and express them in a form that produces laughter
/// or surprise — happened gradually, as the word distinguished
/// itself from wisdom (slow, experiential) and cleverness
/// (practical skill). Wit became the lightning variety of
/// intelligence, the kind that strikes suddenly and is gone:
/// a witty remark is precisely timed; a person of wit has
/// responses that arrive before others have framed the
/// question. In game mechanics, a wit mechanic models the
/// accumulation of mental quickness — the build of acuity,
/// reaction speed, verbal sharpness, or quick-thinking
/// capacity that eventually reaches the threshold at which
/// a character gains access to wit-based abilities, resists
/// mental effects, or delivers decisive responses. `acuity`
/// builds via `sharpen(amount)` and accumulates passively
/// at `quick_rate` per second in `tick(dt)` or dulls via
/// `dull(amount)`.
///
/// Models wit-acuity fill levels, sharpness-saturation bars,
/// mental-speed accumulation trackers, quickness-build gauges,
/// repartee fill levels, verbal-saturation indicators,
/// quick-thinking accumulation bars, acuity meters, sharp-
/// mind completion fill levels, or any mechanic where a
/// character slowly accumulates the mental quickness, verbal
/// precision, or acuity required to deliver decisive responses,
/// resist confusion effects, or reach the threshold of fully
/// sharpened wit.
///
/// `sharpen(amount)` adds acuity; fires `just_sharp` when
/// first reaching `max_acuity`. No-op when disabled.
///
/// `dull(amount)` reduces acuity immediately; fires
/// `just_dulled` when reaching 0. No-op when disabled or
/// already dulled.
///
/// `tick(dt)` clears both flags, then increases acuity by
/// `quick_rate * dt` (capped at `max_acuity`). Fires
/// `just_sharp` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_sharp()` returns `acuity >= max_acuity && enabled`.
///
/// `is_dulled()` returns `acuity == 0.0` (not gated by
/// `enabled`).
///
/// `acuity_fraction()` returns
/// `(acuity / max_acuity).clamp(0, 1)`.
///
/// `effective_quip(scale)` returns `scale * acuity_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — sharpens at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wit {
    pub acuity: f32,
    pub max_acuity: f32,
    pub quick_rate: f32,
    pub just_sharp: bool,
    pub just_dulled: bool,
    pub enabled: bool,
}

impl Wit {
    pub fn new(max_acuity: f32, quick_rate: f32) -> Self {
        Self {
            acuity: 0.0,
            max_acuity: max_acuity.max(0.1),
            quick_rate: quick_rate.max(0.0),
            just_sharp: false,
            just_dulled: false,
            enabled: true,
        }
    }

    /// Add acuity; fires `just_sharp` when first reaching max.
    /// No-op when disabled.
    pub fn sharpen(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.acuity < self.max_acuity;
        self.acuity = (self.acuity + amount).min(self.max_acuity);
        if was_below && self.acuity >= self.max_acuity {
            self.just_sharp = true;
        }
    }

    /// Reduce acuity; fires `just_dulled` when reaching 0.
    /// No-op when disabled or already dulled.
    pub fn dull(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.acuity <= 0.0 {
            return;
        }
        self.acuity = (self.acuity - amount).max(0.0);
        if self.acuity <= 0.0 {
            self.just_dulled = true;
        }
    }

    /// Clear flags, then increase acuity by `quick_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_sharp = false;
        self.just_dulled = false;
        if self.enabled && self.quick_rate > 0.0 && self.acuity < self.max_acuity {
            let was_below = self.acuity < self.max_acuity;
            self.acuity = (self.acuity + self.quick_rate * dt).min(self.max_acuity);
            if was_below && self.acuity >= self.max_acuity {
                self.just_sharp = true;
            }
        }
    }

    /// `true` when acuity is at maximum and component is enabled.
    pub fn is_sharp(&self) -> bool {
        self.acuity >= self.max_acuity && self.enabled
    }

    /// `true` when acuity is 0 (not gated by `enabled`).
    pub fn is_dulled(&self) -> bool {
        self.acuity == 0.0
    }

    /// Fraction of maximum acuity [0.0, 1.0].
    pub fn acuity_fraction(&self) -> f32 {
        (self.acuity / self.max_acuity).clamp(0.0, 1.0)
    }

    /// Returns `scale * acuity_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_quip(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.acuity_fraction()
    }
}

impl Default for Wit {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wit {
        Wit::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_dulled() {
        let w = w();
        assert_eq!(w.acuity, 0.0);
        assert!(w.is_dulled());
        assert!(!w.is_sharp());
    }

    #[test]
    fn new_clamps_max_acuity() {
        let w = Wit::new(-5.0, 1.5);
        assert!((w.max_acuity - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_quick_rate() {
        let w = Wit::new(100.0, -1.5);
        assert_eq!(w.quick_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Wit::default();
        assert!((w.max_acuity - 100.0).abs() < 1e-5);
        assert!((w.quick_rate - 1.5).abs() < 1e-5);
    }

    // --- sharpen ---

    #[test]
    fn sharpen_adds_acuity() {
        let mut w = w();
        w.sharpen(40.0);
        assert!((w.acuity - 40.0).abs() < 1e-3);
    }

    #[test]
    fn sharpen_clamps_at_max() {
        let mut w = w();
        w.sharpen(200.0);
        assert!((w.acuity - 100.0).abs() < 1e-3);
    }

    #[test]
    fn sharpen_fires_just_sharp_at_max() {
        let mut w = w();
        w.sharpen(100.0);
        assert!(w.just_sharp);
        assert!(w.is_sharp());
    }

    #[test]
    fn sharpen_no_just_sharp_when_already_at_max() {
        let mut w = w();
        w.acuity = 100.0;
        w.sharpen(10.0);
        assert!(!w.just_sharp);
    }

    #[test]
    fn sharpen_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.sharpen(50.0);
        assert_eq!(w.acuity, 0.0);
    }

    #[test]
    fn sharpen_no_op_when_amount_zero() {
        let mut w = w();
        w.sharpen(0.0);
        assert_eq!(w.acuity, 0.0);
    }

    // --- dull ---

    #[test]
    fn dull_reduces_acuity() {
        let mut w = w();
        w.acuity = 60.0;
        w.dull(20.0);
        assert!((w.acuity - 40.0).abs() < 1e-3);
    }

    #[test]
    fn dull_clamps_at_zero() {
        let mut w = w();
        w.acuity = 30.0;
        w.dull(200.0);
        assert_eq!(w.acuity, 0.0);
    }

    #[test]
    fn dull_fires_just_dulled_at_zero() {
        let mut w = w();
        w.acuity = 30.0;
        w.dull(30.0);
        assert!(w.just_dulled);
    }

    #[test]
    fn dull_no_op_when_already_dulled() {
        let mut w = w();
        w.dull(10.0);
        assert!(!w.just_dulled);
    }

    #[test]
    fn dull_no_op_when_disabled() {
        let mut w = w();
        w.acuity = 50.0;
        w.enabled = false;
        w.dull(50.0);
        assert!((w.acuity - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_acuity() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.acuity - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_sharp_on_acuity_to_max() {
        let mut w = Wit::new(100.0, 200.0);
        w.acuity = 95.0;
        w.tick(1.0);
        assert!(w.just_sharp);
        assert!(w.is_sharp());
    }

    #[test]
    fn tick_no_build_when_already_sharp() {
        let mut w = w();
        w.acuity = 100.0;
        w.tick(1.0);
        assert!(!w.just_sharp);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Wit::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.acuity, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.acuity, 0.0);
    }

    #[test]
    fn tick_clears_just_sharp() {
        let mut w = Wit::new(100.0, 200.0);
        w.acuity = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_sharp);
    }

    #[test]
    fn tick_clears_just_dulled() {
        let mut w = w();
        w.acuity = 10.0;
        w.dull(10.0);
        w.tick(0.016);
        assert!(!w.just_dulled);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.acuity - 9.0).abs() < 1e-3);
    }

    // --- is_sharp / is_dulled ---

    #[test]
    fn is_sharp_false_when_disabled() {
        let mut w = w();
        w.acuity = 100.0;
        w.enabled = false;
        assert!(!w.is_sharp());
    }

    #[test]
    fn is_dulled_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_dulled());
    }

    // --- acuity_fraction / effective_quip ---

    #[test]
    fn acuity_fraction_zero_when_dulled() {
        assert_eq!(w().acuity_fraction(), 0.0);
    }

    #[test]
    fn acuity_fraction_half_at_midpoint() {
        let mut w = w();
        w.acuity = 50.0;
        assert!((w.acuity_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_quip_zero_when_dulled() {
        assert_eq!(w().effective_quip(100.0), 0.0);
    }

    #[test]
    fn effective_quip_scales_with_acuity() {
        let mut w = w();
        w.acuity = 75.0;
        assert!((w.effective_quip(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_quip_zero_when_disabled() {
        let mut w = w();
        w.acuity = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_quip(100.0), 0.0);
    }
}
