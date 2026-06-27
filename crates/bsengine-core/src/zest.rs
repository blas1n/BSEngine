use bevy_ecs::prelude::Component;

/// Enthusiasm accumulator with passive decay. Zest builds through
/// `inspire()` events and fades each tick unless renewed — models
/// motivation, momentum, or drive that must be actively maintained.
///
/// Unlike `Wiz` (permanent ratchet, no decay) and `Woo` (social persuasion
/// with rebuff reset), Zest is **purely self-driven** with no hard reset
/// method — enthusiasm simply fades silently when un-renewed. This models
/// internal energy states rather than social or skill states.
///
/// `inspire(amount)` adds enthusiasm. If enabled and `amount > 0`: clamps
/// `zest_level` to `max_zest`. Fires `just_enthused`. Fires `just_peaked`
/// the first time `zest_level` reaches `max_zest`. No-op when disabled.
///
/// `tick(dt)` clears one-frame flags first, then if enabled and
/// `zest_level > 0`: subtracts `decay_rate * dt`, floors at 0. No event
/// is fired on reaching 0 — level fades silently. No-op (beyond flag
/// clear) when disabled.
///
/// `is_enthused()` returns `zest_level > 0.0 && enabled`.
///
/// `is_peaked()` returns `zest_level >= max_zest && enabled`.
///
/// `zest_fraction()` returns `(zest_level / max_zest).clamp(0.0, 1.0)`.
///
/// `effective_zest(base)` returns `base * (1.0 + zest_fraction())` when
/// enabled — up to 2× at full enthusiasm; returns `base` when disabled.
///
/// Default: `new(10.0, 1.0)` — max zest 10, decays 1 unit/second.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zest {
    /// Current enthusiasm level [0, max_zest].
    pub zest_level: f32,
    /// Maximum enthusiasm. Clamped >= 1.0.
    pub max_zest: f32,
    /// Passive decay rate in enthusiasm-units/second. Clamped >= 0.0.
    pub decay_rate: f32,
    pub just_enthused: bool,
    pub just_peaked: bool,
    pub enabled: bool,
}

impl Zest {
    pub fn new(max_zest: f32, decay_rate: f32) -> Self {
        Self {
            zest_level: 0.0,
            max_zest: max_zest.max(1.0),
            decay_rate: decay_rate.max(0.0),
            just_enthused: false,
            just_peaked: false,
            enabled: true,
        }
    }

    /// Increase enthusiasm by `amount`. Fires `just_enthused`. Fires
    /// `just_peaked` on first reaching `max_zest`. No-op when disabled or
    /// `amount <= 0`.
    pub fn inspire(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let prev = self.zest_level;
        self.zest_level = (self.zest_level + amount).min(self.max_zest);
        self.just_enthused = true;
        if prev < self.max_zest && self.zest_level >= self.max_zest {
            self.just_peaked = true;
        }
    }

    /// Advance one frame: clear one-frame flags, then decay passively.
    /// Level fades silently — no event fires when reaching 0. No-op (beyond
    /// flag clear) when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_enthused = false;
        self.just_peaked = false;

        if !self.enabled {
            return;
        }

        if self.zest_level > 0.0 {
            self.zest_level = (self.zest_level - self.decay_rate * dt).max(0.0);
        }
    }

    /// `true` when enthusiasm is above 0 and component is enabled.
    pub fn is_enthused(&self) -> bool {
        self.zest_level > 0.0 && self.enabled
    }

    /// `true` when enthusiasm is at maximum and component is enabled.
    pub fn is_peaked(&self) -> bool {
        self.zest_level >= self.max_zest && self.enabled
    }

    /// Enthusiasm level as a fraction of maximum [0.0, 1.0].
    pub fn zest_fraction(&self) -> f32 {
        (self.zest_level / self.max_zest).clamp(0.0, 1.0)
    }

    /// Scale `base` by enthusiasm. Returns `base * (1.0 + zest_fraction())`
    /// when enabled — up to 2× at peak; `base` when disabled.
    pub fn effective_zest(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.zest_fraction())
    }
}

impl Default for Zest {
    fn default() -> Self {
        Self::new(10.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zest {
        Zest::new(10.0, 1.0) // max=10, decay 1/s
    }

    #[test]
    fn new_starts_flat() {
        let z = z();
        assert_eq!(z.zest_level, 0.0);
        assert!(!z.just_enthused);
        assert!(!z.just_peaked);
        assert!(!z.is_enthused());
        assert!(!z.is_peaked());
    }

    // --- inspire ---

    #[test]
    fn inspire_increases_level() {
        let mut z = z();
        z.inspire(4.0);
        assert!((z.zest_level - 4.0).abs() < 1e-5);
    }

    #[test]
    fn inspire_fires_just_enthused() {
        let mut z = z();
        z.inspire(2.0);
        assert!(z.just_enthused);
    }

    #[test]
    fn inspire_clamps_at_max() {
        let mut z = z();
        z.inspire(20.0);
        assert!((z.zest_level - 10.0).abs() < 1e-5);
    }

    #[test]
    fn inspire_fires_just_peaked_at_max() {
        let mut z = z();
        z.inspire(10.0); // exactly at max
        assert!(z.just_peaked);
    }

    #[test]
    fn inspire_fires_just_peaked_crossing_max() {
        let mut z = z();
        z.inspire(7.0);
        z.inspire(5.0); // crosses 10
        assert!(z.just_peaked);
    }

    #[test]
    fn inspire_does_not_refire_just_peaked_at_cap() {
        let mut z = z();
        z.inspire(10.0); // just_peaked=true
        z.just_peaked = false;
        z.inspire(1.0); // already at cap, no re-fire
        assert!(!z.just_peaked);
    }

    #[test]
    fn inspire_no_op_for_zero_amount() {
        let mut z = z();
        z.inspire(0.0);
        assert_eq!(z.zest_level, 0.0);
        assert!(!z.just_enthused);
    }

    #[test]
    fn inspire_no_op_for_negative_amount() {
        let mut z = z();
        z.inspire(-3.0);
        assert_eq!(z.zest_level, 0.0);
    }

    #[test]
    fn inspire_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.inspire(5.0);
        assert_eq!(z.zest_level, 0.0);
        assert!(!z.just_enthused);
    }

    // --- tick ---

    #[test]
    fn tick_decays_passively() {
        let mut z = z(); // decay=1/s
        z.inspire(6.0);
        z.tick(2.0); // 6 - 2 = 4
        assert!((z.zest_level - 4.0).abs() < 1e-4);
    }

    #[test]
    fn tick_floors_at_zero() {
        let mut z = z();
        z.inspire(2.0);
        z.tick(10.0); // 2 - 10 → 0
        assert_eq!(z.zest_level, 0.0);
    }

    #[test]
    fn tick_fades_silently_no_event() {
        let mut z = z();
        z.inspire(5.0);
        z.tick(2.0);
        assert!(!z.just_enthused);
        assert!(!z.just_peaked);
    }

    #[test]
    fn tick_clears_just_enthused() {
        let mut z = z();
        z.inspire(3.0); // just_enthused=true
        z.tick(0.016);
        assert!(!z.just_enthused);
    }

    #[test]
    fn tick_clears_just_peaked() {
        let mut z = z();
        z.inspire(10.0); // just_peaked=true
        z.tick(0.016);
        assert!(!z.just_peaked);
    }

    #[test]
    fn tick_no_op_when_at_zero() {
        let mut z = z();
        z.tick(10.0); // nothing to decay
        assert_eq!(z.zest_level, 0.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut z = z();
        z.inspire(5.0);
        z.enabled = false;
        z.tick(10.0);
        assert!((z.zest_level - 5.0).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_flags_even_when_disabled() {
        let mut z = z();
        z.just_enthused = true;
        z.just_peaked = true;
        z.enabled = false;
        z.tick(0.016);
        assert!(!z.just_enthused);
        assert!(!z.just_peaked);
    }

    // --- is_enthused / is_peaked ---

    #[test]
    fn is_enthused_false_when_flat() {
        let z = z();
        assert!(!z.is_enthused());
    }

    #[test]
    fn is_enthused_true_with_level() {
        let mut z = z();
        z.inspire(2.0);
        assert!(z.is_enthused());
    }

    #[test]
    fn is_enthused_false_when_disabled() {
        let mut z = z();
        z.inspire(5.0);
        z.enabled = false;
        assert!(!z.is_enthused());
    }

    #[test]
    fn is_peaked_false_below_max() {
        let mut z = z();
        z.inspire(7.0);
        assert!(!z.is_peaked());
    }

    #[test]
    fn is_peaked_true_at_max() {
        let mut z = z();
        z.inspire(10.0);
        assert!(z.is_peaked());
    }

    #[test]
    fn is_peaked_false_when_disabled() {
        let mut z = z();
        z.inspire(10.0);
        z.enabled = false;
        assert!(!z.is_peaked());
    }

    // --- zest_fraction ---

    #[test]
    fn zest_fraction_zero_when_flat() {
        let z = z();
        assert_eq!(z.zest_fraction(), 0.0);
    }

    #[test]
    fn zest_fraction_at_half() {
        let mut z = z(); // max=10
        z.inspire(5.0); // 5/10=0.5
        assert!((z.zest_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn zest_fraction_one_at_max() {
        let mut z = z();
        z.inspire(10.0);
        assert!((z.zest_fraction() - 1.0).abs() < 1e-4);
    }

    // --- effective_zest ---

    #[test]
    fn effective_zest_passthrough_when_flat() {
        let z = z();
        assert!((z.effective_zest(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_zest_scaled_at_half() {
        let mut z = z();
        z.inspire(5.0); // fraction=0.5 → 100*(1+0.5)=150
        assert!((z.effective_zest(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_zest_doubled_at_peak() {
        let mut z = z();
        z.inspire(10.0); // fraction=1.0 → 100*(1+1)=200
        assert!((z.effective_zest(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn effective_zest_passthrough_when_disabled() {
        let mut z = z();
        z.inspire(10.0);
        z.enabled = false;
        assert!((z.effective_zest(100.0) - 100.0).abs() < 1e-4);
    }

    // --- constructor clamping ---

    #[test]
    fn max_zest_clamped_to_one() {
        let z = Zest::new(0.0, 1.0);
        assert!((z.max_zest - 1.0).abs() < 1e-5);
    }

    #[test]
    fn decay_rate_clamped_to_zero() {
        let z = Zest::new(10.0, -1.0);
        assert_eq!(z.decay_rate, 0.0);
    }

    #[test]
    fn zero_decay_keeps_level() {
        let mut z = Zest::new(10.0, 0.0);
        z.inspire(7.0);
        z.tick(100.0);
        assert!((z.zest_level - 7.0).abs() < 1e-4);
    }

    // --- inspire-decay cycle ---

    #[test]
    fn inspire_decay_cycle() {
        let mut z = z(); // decay=1/s
        z.inspire(8.0);
        z.tick(3.0); // 5.0
        z.inspire(3.0); // 8.0
        assert!((z.zest_level - 8.0).abs() < 1e-4);
    }

    #[test]
    fn decay_to_zero_then_reinspire() {
        let mut z = z();
        z.inspire(2.0);
        z.tick(5.0); // fully faded
        assert_eq!(z.zest_level, 0.0);
        z.inspire(6.0);
        assert!((z.zest_level - 6.0).abs() < 1e-5);
        assert!(z.just_enthused);
    }

    #[test]
    fn accumulator_carries_over_ticks() {
        let mut z = z(); // decay=1/s
        z.inspire(5.0);
        z.tick(1.0); // 4.0
        z.tick(1.0); // 3.0
        z.tick(1.0); // 2.0
        assert!((z.zest_level - 2.0).abs() < 1e-4);
    }
}
