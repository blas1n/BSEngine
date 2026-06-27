use bevy_ecs::prelude::Component;

/// Devotion-threshold accumulator. Builds via `inspire()` and decays
/// passively **only while below `threshold`** — once devoted
/// (`zeal_level >= threshold`), decay pauses and level holds until the game
/// system reduces it externally.
///
/// Distinct from `Zest` (always decays regardless of level) and `Yen` (grows
/// autonomously without a threshold gate): Zeal models committed investment —
/// hard to break once established, but easy to lose if never reinforced.
///
/// `inspire(amount)` adds `amount` to `zeal_level` (clamped to `max_zeal`)
/// when enabled and `amount > 0`. Fires `just_devoted` the first frame
/// `zeal_level` crosses `threshold` from below. No-op when disabled or
/// `amount <= 0`.
///
/// `tick(dt)` clears one-frame flags first. If enabled and
/// `0 < zeal_level < threshold`: subtracts `decay_rate * dt`, floors at 0.
/// Fires `just_lapsed` when reaching 0. No-op (beyond flag clear) when
/// disabled or already devoted.
///
/// `is_devoted()` returns `zeal_level >= threshold && enabled`.
///
/// `is_fading()` returns `zeal_level > 0.0 && zeal_level < threshold &&
/// enabled`.
///
/// `zeal_fraction()` returns `(zeal_level / max_zeal).clamp(0.0, 1.0)`.
///
/// `effective_motivation(base)` returns `base * (1.0 + zeal_fraction())`
/// when enabled — 1× at 0, 2× at max; `base` when disabled.
///
/// Default: `new(10.0, 5.0, 1.0)` — max 10, devoted at 5, decay 1/s below.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zeal {
    /// Current zeal level [0, max_zeal].
    pub zeal_level: f32,
    /// Maximum zeal. Clamped >= 1.0.
    pub max_zeal: f32,
    /// Level at which devotion activates. Clamped to (0, max_zeal].
    pub threshold: f32,
    /// Decay rate in units/second applied only below threshold. Clamped >= 0.
    pub decay_rate: f32,
    pub just_devoted: bool,
    pub just_lapsed: bool,
    pub enabled: bool,
}

impl Zeal {
    pub fn new(max_zeal: f32, threshold: f32, decay_rate: f32) -> Self {
        let max_zeal = max_zeal.max(1.0);
        let threshold = threshold.clamp(0.01, max_zeal);
        Self {
            zeal_level: 0.0,
            max_zeal,
            threshold,
            decay_rate: decay_rate.max(0.0),
            just_devoted: false,
            just_lapsed: false,
            enabled: true,
        }
    }

    /// Add `amount` to `zeal_level`. Fires `just_devoted` on first crossing
    /// threshold. No-op when `amount <= 0` or disabled.
    pub fn inspire(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_devoted = self.zeal_level >= self.threshold;
        self.zeal_level = (self.zeal_level + amount).min(self.max_zeal);
        if !was_devoted && self.zeal_level >= self.threshold {
            self.just_devoted = true;
        }
    }

    /// Advance one frame: clear flags, then decay below threshold. Fires
    /// `just_lapsed` on reaching 0. No-op (beyond flag clear) when disabled
    /// or devoted.
    pub fn tick(&mut self, dt: f32) {
        self.just_devoted = false;
        self.just_lapsed = false;

        if !self.enabled {
            return;
        }

        if self.zeal_level > 0.0 && self.zeal_level < self.threshold {
            self.zeal_level = (self.zeal_level - self.decay_rate * dt).max(0.0);
            if self.zeal_level == 0.0 {
                self.just_lapsed = true;
            }
        }
    }

    /// `true` when committed: `zeal_level >= threshold` and enabled.
    pub fn is_devoted(&self) -> bool {
        self.zeal_level >= self.threshold && self.enabled
    }

    /// `true` when zeal is present but below threshold (actively decaying).
    pub fn is_fading(&self) -> bool {
        self.zeal_level > 0.0 && self.zeal_level < self.threshold && self.enabled
    }

    /// Zeal as a fraction of maximum [0.0, 1.0].
    pub fn zeal_fraction(&self) -> f32 {
        (self.zeal_level / self.max_zeal).clamp(0.0, 1.0)
    }

    /// Scale `base` by zeal level. Returns `base * (1.0 + zeal_fraction())`
    /// when enabled — 1× at 0, 2× at max; `base` when disabled.
    pub fn effective_motivation(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.zeal_fraction())
    }
}

impl Default for Zeal {
    fn default() -> Self {
        Self::new(10.0, 5.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zeal {
        Zeal::new(10.0, 5.0, 1.0) // max=10, threshold=5, decay=1/s
    }

    // --- construction ---

    #[test]
    fn new_starts_at_zero() {
        let z = z();
        assert_eq!(z.zeal_level, 0.0);
        assert!(!z.just_devoted);
        assert!(!z.just_lapsed);
        assert!(!z.is_devoted());
        assert!(!z.is_fading());
    }

    #[test]
    fn max_zeal_clamped_to_one() {
        let z = Zeal::new(0.0, 0.5, 1.0);
        assert!((z.max_zeal - 1.0).abs() < 1e-5);
    }

    #[test]
    fn threshold_clamped_to_max() {
        let z = Zeal::new(10.0, 20.0, 1.0);
        assert!((z.threshold - 10.0).abs() < 1e-5);
    }

    #[test]
    fn threshold_clamped_above_zero() {
        let z = Zeal::new(10.0, -1.0, 1.0);
        assert!(z.threshold > 0.0);
    }

    #[test]
    fn decay_rate_clamped_to_zero() {
        let z = Zeal::new(10.0, 5.0, -1.0);
        assert_eq!(z.decay_rate, 0.0);
    }

    // --- inspire ---

    #[test]
    fn inspire_adds_zeal() {
        let mut z = z();
        z.inspire(3.0);
        assert!((z.zeal_level - 3.0).abs() < 1e-5);
    }

    #[test]
    fn inspire_clamps_to_max() {
        let mut z = z();
        z.inspire(100.0);
        assert!((z.zeal_level - 10.0).abs() < 1e-5);
    }

    #[test]
    fn inspire_fires_just_devoted_on_threshold_crossing() {
        let mut z = z(); // threshold=5
        z.inspire(5.0);
        assert!(z.just_devoted);
        assert!(z.is_devoted());
    }

    #[test]
    fn inspire_does_not_refire_just_devoted_when_already_devoted() {
        let mut z = z();
        z.inspire(5.0); // crosses threshold
        z.tick(0.016); // clear flags
        z.inspire(2.0); // already devoted — no refire
        assert!(!z.just_devoted);
    }

    #[test]
    fn inspire_no_op_at_zero_amount() {
        let mut z = z();
        z.inspire(0.0);
        assert_eq!(z.zeal_level, 0.0);
        assert!(!z.just_devoted);
    }

    #[test]
    fn inspire_no_op_at_negative_amount() {
        let mut z = z();
        z.inspire(-3.0);
        assert_eq!(z.zeal_level, 0.0);
    }

    #[test]
    fn inspire_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.inspire(5.0);
        assert_eq!(z.zeal_level, 0.0);
        assert!(!z.just_devoted);
    }

    // --- tick: decay below threshold ---

    #[test]
    fn tick_decays_below_threshold() {
        let mut z = z(); // decay=1/s, threshold=5
        z.zeal_level = 3.0; // below threshold
        z.tick(1.0);
        assert!((z.zeal_level - 2.0).abs() < 1e-5);
    }

    #[test]
    fn tick_fires_just_lapsed_at_zero() {
        let mut z = z();
        z.zeal_level = 0.5;
        z.tick(1.0); // crosses 0
        assert!(z.just_lapsed);
        assert_eq!(z.zeal_level, 0.0);
    }

    #[test]
    fn tick_just_lapsed_clears_next_frame() {
        let mut z = z();
        z.zeal_level = 0.5;
        z.tick(1.0); // just_lapsed=true
        z.tick(0.016);
        assert!(!z.just_lapsed);
    }

    #[test]
    fn tick_pauses_decay_when_devoted() {
        let mut z = z(); // threshold=5
        z.zeal_level = 7.0; // above threshold — devoted
        z.tick(10.0); // long tick — should NOT decay
        assert!((z.zeal_level - 7.0).abs() < 1e-5);
    }

    #[test]
    fn tick_no_op_at_exactly_threshold() {
        let mut z = z();
        z.zeal_level = 5.0; // exactly at threshold
        z.tick(10.0); // devoted — no decay
        assert!((z.zeal_level - 5.0).abs() < 1e-5);
    }

    #[test]
    fn tick_no_op_at_zero() {
        let mut z = z();
        z.tick(10.0); // already 0
        assert_eq!(z.zeal_level, 0.0);
        assert!(!z.just_lapsed);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut z = z();
        z.zeal_level = 3.0;
        z.enabled = false;
        z.tick(1.0);
        assert!((z.zeal_level - 3.0).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_flags_when_disabled() {
        let mut z = z();
        z.just_devoted = true;
        z.just_lapsed = true;
        z.enabled = false;
        z.tick(0.016);
        assert!(!z.just_devoted);
        assert!(!z.just_lapsed);
    }

    #[test]
    fn tick_clears_just_devoted() {
        let mut z = z();
        z.inspire(5.0); // just_devoted=true
        z.tick(0.016);
        assert!(!z.just_devoted);
    }

    // --- is_devoted / is_fading ---

    #[test]
    fn is_devoted_false_below_threshold() {
        let mut z = z();
        z.zeal_level = 4.9;
        assert!(!z.is_devoted());
    }

    #[test]
    fn is_devoted_true_at_threshold() {
        let mut z = z();
        z.zeal_level = 5.0;
        assert!(z.is_devoted());
    }

    #[test]
    fn is_devoted_true_above_threshold() {
        let mut z = z();
        z.inspire(8.0);
        assert!(z.is_devoted());
    }

    #[test]
    fn is_devoted_false_when_disabled() {
        let mut z = z();
        z.inspire(8.0);
        z.enabled = false;
        assert!(!z.is_devoted());
    }

    #[test]
    fn is_fading_true_between_zero_and_threshold() {
        let mut z = z();
        z.zeal_level = 3.0;
        assert!(z.is_fading());
    }

    #[test]
    fn is_fading_false_at_zero() {
        let z = z();
        assert!(!z.is_fading());
    }

    #[test]
    fn is_fading_false_when_devoted() {
        let mut z = z();
        z.inspire(5.0);
        assert!(!z.is_fading());
    }

    #[test]
    fn is_fading_false_when_disabled() {
        let mut z = z();
        z.zeal_level = 3.0;
        z.enabled = false;
        assert!(!z.is_fading());
    }

    // --- zeal_fraction ---

    #[test]
    fn zeal_fraction_zero_at_start() {
        let z = z();
        assert_eq!(z.zeal_fraction(), 0.0);
    }

    #[test]
    fn zeal_fraction_half_at_mid() {
        let mut z = z(); // max=10
        z.inspire(5.0);
        assert!((z.zeal_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn zeal_fraction_one_at_max() {
        let mut z = z();
        z.inspire(10.0);
        assert!((z.zeal_fraction() - 1.0).abs() < 1e-5);
    }

    // --- effective_motivation ---

    #[test]
    fn effective_motivation_passthrough_at_zero() {
        let z = z(); // fraction=0 → 100*(1+0)=100
        assert!((z.effective_motivation(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_motivation_at_half() {
        let mut z = z();
        z.inspire(5.0); // fraction=0.5 → 100*(1+0.5)=150
        assert!((z.effective_motivation(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_motivation_doubled_at_max() {
        let mut z = z();
        z.inspire(10.0); // fraction=1.0 → 100*(1+1)=200
        assert!((z.effective_motivation(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn effective_motivation_passthrough_when_disabled() {
        let mut z = z();
        z.inspire(10.0);
        z.enabled = false;
        assert!((z.effective_motivation(100.0) - 100.0).abs() < 1e-4);
    }

    // --- devotion/decay cycle ---

    #[test]
    fn devotion_holds_then_decays_after_dip() {
        let mut z = z(); // threshold=5, decay=1/s
        z.inspire(7.0); // devoted at 7
        z.zeal_level = 3.0; // simulate external reduction below threshold
        z.tick(1.0); // now decays: 3-1=2
        assert!((z.zeal_level - 2.0).abs() < 1e-5);
        assert!(!z.is_devoted());
        assert!(z.is_fading());
    }

    #[test]
    fn re_inspiring_restores_devotion() {
        let mut z = z();
        z.inspire(7.0); // devoted
        z.zeal_level = 3.0; // dipped below
        z.inspire(4.0); // back to 7 → devoted again
        assert!(z.is_devoted());
        assert!(z.just_devoted);
    }
}
