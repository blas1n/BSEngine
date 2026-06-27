use bevy_ecs::prelude::Component;

/// Persuasion accumulator with passive decay. Models sustained social
/// effort: charm builds via `sway()` events, fades automatically each
/// tick, and collapses instantly on `rebuff()`.
///
/// Unlike `Wist` (event-only longing, no time logic whatsoever) and
/// `Charm` (different mechanic), Woo requires **continuous investment**
/// to maintain — `decay_rate` drains the level each tick, so attention
/// that stops will eventually reset the state on its own. `rebuff()`
/// provides a hard instant reset for hostile reactions.
///
/// `sway(amount)` increases affection. If enabled and `amount > 0`:
/// clamps `woo_level` to `max_woo`. Fires `just_charmed` on any increase.
/// Fires `just_smitten` the first time `woo_level` reaches `max_woo`.
/// No-op when disabled.
///
/// `rebuff()` instantly resets `woo_level` to 0 and fires `just_rejected`.
/// No-op when already at 0 or disabled.
///
/// `tick(dt)` clears one-frame flags first, then if enabled and
/// `woo_level > 0`: subtracts `decay_rate * dt`, floors at 0. No passive
/// event is fired when draining (unlike `Wok`'s `just_cooled`) — the
/// level simply fades silently. No-op (beyond flag clear) when disabled
/// or already at 0.
///
/// `is_charmed()` returns `woo_level > 0.0 && enabled`.
///
/// `is_smitten()` returns `woo_level >= max_woo && enabled`.
///
/// `woo_fraction()` returns `(woo_level / max_woo).clamp(0.0, 1.0)`.
///
/// `effective_appeal(base)` returns `base * (1.0 + woo_fraction())` when
/// enabled — up to 2× when fully smitten; returns `base` when disabled.
///
/// Default: `new(10.0, 1.0)` — max affection 10, decays 1 unit/second.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Woo {
    /// Current affection level [0, max_woo].
    pub woo_level: f32,
    /// Maximum affection. Clamped >= 1.0.
    pub max_woo: f32,
    /// Passive decay rate in affection-units/second. Clamped >= 0.0.
    pub decay_rate: f32,
    pub just_charmed: bool,
    pub just_smitten: bool,
    pub just_rejected: bool,
    pub enabled: bool,
}

impl Woo {
    pub fn new(max_woo: f32, decay_rate: f32) -> Self {
        Self {
            woo_level: 0.0,
            max_woo: max_woo.max(1.0),
            decay_rate: decay_rate.max(0.0),
            just_charmed: false,
            just_smitten: false,
            just_rejected: false,
            enabled: true,
        }
    }

    /// Increase affection by `amount`. Fires `just_charmed`. Fires
    /// `just_smitten` on first reaching `max_woo`. No-op when disabled
    /// or `amount <= 0`.
    pub fn sway(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let prev = self.woo_level;
        self.woo_level = (self.woo_level + amount).min(self.max_woo);
        self.just_charmed = true;
        if prev < self.max_woo && self.woo_level >= self.max_woo {
            self.just_smitten = true;
        }
    }

    /// Instantly clear all affection. Fires `just_rejected`. No-op when
    /// already at 0 or disabled.
    pub fn rebuff(&mut self) {
        if !self.enabled || self.woo_level == 0.0 {
            return;
        }
        self.woo_level = 0.0;
        self.just_rejected = true;
    }

    /// Advance one frame: clear one-frame flags, then decay passively. No
    /// passive event is fired when draining (level fades silently). No-op
    /// (beyond flag clear) when disabled or already at 0.
    pub fn tick(&mut self, dt: f32) {
        self.just_charmed = false;
        self.just_smitten = false;
        self.just_rejected = false;

        if !self.enabled || self.woo_level == 0.0 {
            return;
        }

        self.woo_level = (self.woo_level - self.decay_rate * dt).max(0.0);
    }

    /// `true` when affection is above 0 and component is enabled.
    pub fn is_charmed(&self) -> bool {
        self.woo_level > 0.0 && self.enabled
    }

    /// `true` when affection is at maximum and component is enabled.
    pub fn is_smitten(&self) -> bool {
        self.woo_level >= self.max_woo && self.enabled
    }

    /// Affection level as a fraction of maximum [0.0, 1.0].
    pub fn woo_fraction(&self) -> f32 {
        (self.woo_level / self.max_woo).clamp(0.0, 1.0)
    }

    /// Scale `base` by affection. Returns `base * (1.0 + woo_fraction())`
    /// when enabled — up to 2× at max affection; `base` when disabled.
    pub fn effective_appeal(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.woo_fraction())
    }
}

impl Default for Woo {
    fn default() -> Self {
        Self::new(10.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Woo {
        Woo::new(10.0, 1.0) // max=10, decay 1/s
    }

    #[test]
    fn new_starts_neutral() {
        let w = w();
        assert_eq!(w.woo_level, 0.0);
        assert!(!w.just_charmed);
        assert!(!w.just_smitten);
        assert!(!w.just_rejected);
        assert!(!w.is_charmed());
        assert!(!w.is_smitten());
    }

    // --- sway ---

    #[test]
    fn sway_increases_level() {
        let mut w = w();
        w.sway(3.0);
        assert!((w.woo_level - 3.0).abs() < 1e-5);
    }

    #[test]
    fn sway_fires_just_charmed() {
        let mut w = w();
        w.sway(2.0);
        assert!(w.just_charmed);
    }

    #[test]
    fn sway_clamps_at_max() {
        let mut w = w();
        w.sway(20.0);
        assert!((w.woo_level - 10.0).abs() < 1e-5);
    }

    #[test]
    fn sway_fires_just_smitten_at_max() {
        let mut w = w();
        w.sway(10.0); // exactly at max
        assert!(w.just_smitten);
    }

    #[test]
    fn sway_fires_just_smitten_crossing_max() {
        let mut w = w();
        w.sway(7.0);
        w.sway(5.0); // crosses 10
        assert!(w.just_smitten);
    }

    #[test]
    fn sway_does_not_refire_just_smitten_at_cap() {
        let mut w = w();
        w.sway(10.0); // just_smitten=true
        w.just_smitten = false;
        w.sway(1.0); // already at cap
        assert!(!w.just_smitten);
    }

    #[test]
    fn sway_no_op_for_zero_amount() {
        let mut w = w();
        w.sway(0.0);
        assert_eq!(w.woo_level, 0.0);
        assert!(!w.just_charmed);
    }

    #[test]
    fn sway_no_op_for_negative_amount() {
        let mut w = w();
        w.sway(-3.0);
        assert_eq!(w.woo_level, 0.0);
        assert!(!w.just_charmed);
    }

    #[test]
    fn sway_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.sway(5.0);
        assert_eq!(w.woo_level, 0.0);
        assert!(!w.just_charmed);
    }

    // --- rebuff ---

    #[test]
    fn rebuff_resets_level() {
        let mut w = w();
        w.sway(6.0);
        w.rebuff();
        assert_eq!(w.woo_level, 0.0);
    }

    #[test]
    fn rebuff_fires_just_rejected() {
        let mut w = w();
        w.sway(5.0);
        w.rebuff();
        assert!(w.just_rejected);
    }

    #[test]
    fn rebuff_no_op_when_at_zero() {
        let mut w = w();
        w.rebuff();
        assert!(!w.just_rejected);
    }

    #[test]
    fn rebuff_no_op_when_disabled() {
        let mut w = w();
        w.sway(5.0);
        w.enabled = false;
        w.rebuff();
        assert!((w.woo_level - 5.0).abs() < 1e-5);
        assert!(!w.just_rejected);
    }

    // --- tick ---

    #[test]
    fn tick_decays_passively() {
        let mut w = w(); // decay=1/s
        w.sway(5.0);
        w.tick(2.0); // 5 - 2 = 3
        assert!((w.woo_level - 3.0).abs() < 1e-4);
    }

    #[test]
    fn tick_floors_at_zero() {
        let mut w = w();
        w.sway(2.0);
        w.tick(10.0); // 2 - 10 → 0
        assert_eq!(w.woo_level, 0.0);
    }

    #[test]
    fn tick_no_passive_event_when_draining() {
        let mut w = w();
        w.sway(5.0);
        w.tick(2.0);
        // No just_cooled equivalent — level fades silently
        assert!(!w.just_charmed);
        assert!(!w.just_smitten);
        assert!(!w.just_rejected);
    }

    #[test]
    fn tick_clears_just_charmed() {
        let mut w = w();
        w.sway(3.0); // just_charmed=true
        w.tick(0.016);
        assert!(!w.just_charmed);
    }

    #[test]
    fn tick_clears_just_smitten() {
        let mut w = w();
        w.sway(10.0); // just_smitten=true
        w.tick(0.016);
        assert!(!w.just_smitten);
    }

    #[test]
    fn tick_clears_just_rejected() {
        let mut w = w();
        w.sway(5.0);
        w.rebuff(); // just_rejected=true
        w.tick(0.016);
        assert!(!w.just_rejected);
    }

    #[test]
    fn tick_no_op_when_at_zero() {
        let mut w = w();
        w.tick(10.0); // nothing to decay
        assert_eq!(w.woo_level, 0.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = w();
        w.sway(5.0);
        w.enabled = false;
        w.tick(10.0);
        assert!((w.woo_level - 5.0).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_flags_even_when_disabled() {
        let mut w = w();
        w.just_charmed = true;
        w.just_smitten = true;
        w.just_rejected = true;
        w.enabled = false;
        w.tick(0.016);
        assert!(!w.just_charmed);
        assert!(!w.just_smitten);
        assert!(!w.just_rejected);
    }

    // --- is_charmed / is_smitten ---

    #[test]
    fn is_charmed_false_when_neutral() {
        let w = w();
        assert!(!w.is_charmed());
    }

    #[test]
    fn is_charmed_true_with_level() {
        let mut w = w();
        w.sway(3.0);
        assert!(w.is_charmed());
    }

    #[test]
    fn is_charmed_false_when_disabled() {
        let mut w = w();
        w.sway(5.0);
        w.enabled = false;
        assert!(!w.is_charmed());
    }

    #[test]
    fn is_smitten_false_below_max() {
        let mut w = w();
        w.sway(7.0);
        assert!(!w.is_smitten());
    }

    #[test]
    fn is_smitten_true_at_max() {
        let mut w = w();
        w.sway(10.0);
        assert!(w.is_smitten());
    }

    #[test]
    fn is_smitten_false_when_disabled() {
        let mut w = w();
        w.sway(10.0);
        w.enabled = false;
        assert!(!w.is_smitten());
    }

    // --- woo_fraction ---

    #[test]
    fn woo_fraction_zero_when_neutral() {
        let w = w();
        assert_eq!(w.woo_fraction(), 0.0);
    }

    #[test]
    fn woo_fraction_at_half() {
        let mut w = w(); // max=10
        w.sway(5.0); // 5/10=0.5
        assert!((w.woo_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn woo_fraction_one_at_max() {
        let mut w = w();
        w.sway(10.0);
        assert!((w.woo_fraction() - 1.0).abs() < 1e-4);
    }

    // --- effective_appeal ---

    #[test]
    fn effective_appeal_passthrough_when_neutral() {
        let w = w();
        assert!((w.effective_appeal(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_appeal_scaled_at_half() {
        let mut w = w();
        w.sway(5.0); // fraction=0.5 → 100*(1+0.5)=150
        assert!((w.effective_appeal(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_appeal_doubled_at_max() {
        let mut w = w();
        w.sway(10.0); // fraction=1.0 → 100*(1+1)=200
        assert!((w.effective_appeal(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn effective_appeal_passthrough_when_disabled() {
        let mut w = w();
        w.sway(10.0);
        w.enabled = false;
        assert!((w.effective_appeal(100.0) - 100.0).abs() < 1e-4);
    }

    // --- constructor clamping ---

    #[test]
    fn max_woo_clamped_to_one() {
        let w = Woo::new(0.0, 1.0);
        assert!((w.max_woo - 1.0).abs() < 1e-5);
    }

    #[test]
    fn decay_rate_clamped_to_zero() {
        let w = Woo::new(10.0, -1.0);
        assert_eq!(w.decay_rate, 0.0);
    }

    #[test]
    fn zero_decay_rate_keeps_level() {
        let mut w = Woo::new(10.0, 0.0);
        w.sway(7.0);
        w.tick(100.0);
        assert!((w.woo_level - 7.0).abs() < 1e-4);
    }

    // --- sway-decay-rebuff cycle ---

    #[test]
    fn sway_decay_cycle() {
        let mut w = w(); // decay=1/s
        w.sway(8.0); // 8.0
        w.tick(3.0); // 5.0
        w.sway(3.0); // 8.0 again
        assert!((w.woo_level - 8.0).abs() < 1e-4);
    }

    #[test]
    fn rebuff_then_sway_restarts() {
        let mut w = w();
        w.sway(8.0);
        w.rebuff(); // 0, just_rejected
        w.sway(3.0); // fresh start
        assert!((w.woo_level - 3.0).abs() < 1e-5);
        assert!(w.just_charmed);
    }

    #[test]
    fn decay_to_zero_then_resway() {
        let mut w = w(); // decay=1/s
        w.sway(2.0);
        w.tick(5.0); // fully decayed to 0
        assert_eq!(w.woo_level, 0.0);
        w.sway(4.0); // start fresh
        assert!((w.woo_level - 4.0).abs() < 1e-5);
    }
}
