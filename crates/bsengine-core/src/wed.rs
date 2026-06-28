use bevy_ecs::prelude::Component;

/// Bond-commitment accumulation tracker named after wed, the verb
/// meaning to marry; to unite closely or inextricably; to become
/// firmly committed to a belief, course of action, or attitude
/// — from the Old English weddian (to pledge, to betroth, to
/// covenant), from the Proto-Germanic wadjaną (to pledge), from
/// the Proto-Indo-European root wadh- (to pledge, to guarantee).
/// The root wadh- gave Gothic wadi (a pledge), Old High German
/// wetti (a pledge, a wager), Latin vas, vadis (a guarantor, a
/// bail), and the derived English gage and wage — all of them
/// words about making a promise, putting something forward as
/// security for the keeping of a contract. To wed, in the oldest
/// sense, is not simply to marry but to pledge — to bind oneself
/// by a solemn promise that costs something if broken. The
/// modern wedding retains this pledging sense in the exchange
/// of vows and rings: the ring is the token of the pledge, the
/// vow is the pledge itself. In metaphorical usage, to be wed
/// to an idea, a method, or a style is to be so committed to
/// it that departing from it feels like a kind of infidelity.
/// In game mechanics, a wed mechanic models the slow build of
/// commitment or bonding — the accumulation of attachment,
/// loyalty, or pledged alliance that eventually reaches the
/// threshold at which a relationship becomes binding, a faction
/// alliance is cemented, or a covenant is sealed. `bond` builds
/// via `pledge(amount)` and accumulates passively at `bind_rate`
/// per second in `tick(dt)` or dissolves via `sever(amount)`.
///
/// Models bond-commitment fill levels, alliance-saturation bars,
/// loyalty-accumulation trackers, covenant-build gauges, faction-
/// bond fill levels, marriage-saturation indicators, oath-
/// accumulation bars, devotion meters, treaty-completion fill
/// levels, or any mechanic where a character, faction, or entity
/// slowly accumulates the pledges, bonds, or commitments required
/// to cement a relationship, seal a pact, or reach the threshold
/// of true alliance.
///
/// `pledge(amount)` adds bond; fires `just_wedded` when first
/// reaching `max_bond`. No-op when disabled.
///
/// `sever(amount)` reduces bond immediately; fires `just_sundered`
/// when reaching 0. No-op when disabled or already sundered.
///
/// `tick(dt)` clears both flags, then increases bond by
/// `bind_rate * dt` (capped at `max_bond`). Fires `just_wedded`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_wedded()` returns `bond >= max_bond && enabled`.
///
/// `is_sundered()` returns `bond == 0.0` (not gated by `enabled`).
///
/// `bond_fraction()` returns `(bond / max_bond).clamp(0, 1)`.
///
/// `effective_devotion(scale)` returns `scale * bond_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — binds at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wed {
    pub bond: f32,
    pub max_bond: f32,
    pub bind_rate: f32,
    pub just_wedded: bool,
    pub just_sundered: bool,
    pub enabled: bool,
}

impl Wed {
    pub fn new(max_bond: f32, bind_rate: f32) -> Self {
        Self {
            bond: 0.0,
            max_bond: max_bond.max(0.1),
            bind_rate: bind_rate.max(0.0),
            just_wedded: false,
            just_sundered: false,
            enabled: true,
        }
    }

    /// Add bond; fires `just_wedded` when first reaching max.
    /// No-op when disabled.
    pub fn pledge(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.bond < self.max_bond;
        self.bond = (self.bond + amount).min(self.max_bond);
        if was_below && self.bond >= self.max_bond {
            self.just_wedded = true;
        }
    }

    /// Reduce bond; fires `just_sundered` when reaching 0.
    /// No-op when disabled or already sundered.
    pub fn sever(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.bond <= 0.0 {
            return;
        }
        self.bond = (self.bond - amount).max(0.0);
        if self.bond <= 0.0 {
            self.just_sundered = true;
        }
    }

    /// Clear flags, then increase bond by `bind_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_wedded = false;
        self.just_sundered = false;
        if self.enabled && self.bind_rate > 0.0 && self.bond < self.max_bond {
            let was_below = self.bond < self.max_bond;
            self.bond = (self.bond + self.bind_rate * dt).min(self.max_bond);
            if was_below && self.bond >= self.max_bond {
                self.just_wedded = true;
            }
        }
    }

    /// `true` when bond is at maximum and component is enabled.
    pub fn is_wedded(&self) -> bool {
        self.bond >= self.max_bond && self.enabled
    }

    /// `true` when bond is 0 (not gated by `enabled`).
    pub fn is_sundered(&self) -> bool {
        self.bond == 0.0
    }

    /// Fraction of maximum bond [0.0, 1.0].
    pub fn bond_fraction(&self) -> f32 {
        (self.bond / self.max_bond).clamp(0.0, 1.0)
    }

    /// Returns `scale * bond_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_devotion(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.bond_fraction()
    }
}

impl Default for Wed {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wed {
        Wed::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_sundered() {
        let w = w();
        assert_eq!(w.bond, 0.0);
        assert!(w.is_sundered());
        assert!(!w.is_wedded());
    }

    #[test]
    fn new_clamps_max_bond() {
        let w = Wed::new(-5.0, 1.5);
        assert!((w.max_bond - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_bind_rate() {
        let w = Wed::new(100.0, -1.5);
        assert_eq!(w.bind_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Wed::default();
        assert!((w.max_bond - 100.0).abs() < 1e-5);
        assert!((w.bind_rate - 1.5).abs() < 1e-5);
    }

    // --- pledge ---

    #[test]
    fn pledge_adds_bond() {
        let mut w = w();
        w.pledge(40.0);
        assert!((w.bond - 40.0).abs() < 1e-3);
    }

    #[test]
    fn pledge_clamps_at_max() {
        let mut w = w();
        w.pledge(200.0);
        assert!((w.bond - 100.0).abs() < 1e-3);
    }

    #[test]
    fn pledge_fires_just_wedded_at_max() {
        let mut w = w();
        w.pledge(100.0);
        assert!(w.just_wedded);
        assert!(w.is_wedded());
    }

    #[test]
    fn pledge_no_just_wedded_when_already_at_max() {
        let mut w = w();
        w.bond = 100.0;
        w.pledge(10.0);
        assert!(!w.just_wedded);
    }

    #[test]
    fn pledge_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.pledge(50.0);
        assert_eq!(w.bond, 0.0);
    }

    #[test]
    fn pledge_no_op_when_amount_zero() {
        let mut w = w();
        w.pledge(0.0);
        assert_eq!(w.bond, 0.0);
    }

    // --- sever ---

    #[test]
    fn sever_reduces_bond() {
        let mut w = w();
        w.bond = 60.0;
        w.sever(20.0);
        assert!((w.bond - 40.0).abs() < 1e-3);
    }

    #[test]
    fn sever_clamps_at_zero() {
        let mut w = w();
        w.bond = 30.0;
        w.sever(200.0);
        assert_eq!(w.bond, 0.0);
    }

    #[test]
    fn sever_fires_just_sundered_at_zero() {
        let mut w = w();
        w.bond = 30.0;
        w.sever(30.0);
        assert!(w.just_sundered);
    }

    #[test]
    fn sever_no_op_when_already_sundered() {
        let mut w = w();
        w.sever(10.0);
        assert!(!w.just_sundered);
    }

    #[test]
    fn sever_no_op_when_disabled() {
        let mut w = w();
        w.bond = 50.0;
        w.enabled = false;
        w.sever(50.0);
        assert!((w.bond - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_bond() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.bond - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_wedded_on_bond_to_max() {
        let mut w = Wed::new(100.0, 200.0);
        w.bond = 95.0;
        w.tick(1.0);
        assert!(w.just_wedded);
        assert!(w.is_wedded());
    }

    #[test]
    fn tick_no_build_when_already_wedded() {
        let mut w = w();
        w.bond = 100.0;
        w.tick(1.0);
        assert!(!w.just_wedded);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Wed::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.bond, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.bond, 0.0);
    }

    #[test]
    fn tick_clears_just_wedded() {
        let mut w = Wed::new(100.0, 200.0);
        w.bond = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_wedded);
    }

    #[test]
    fn tick_clears_just_sundered() {
        let mut w = w();
        w.bond = 10.0;
        w.sever(10.0);
        w.tick(0.016);
        assert!(!w.just_sundered);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.bond - 9.0).abs() < 1e-3);
    }

    // --- is_wedded / is_sundered ---

    #[test]
    fn is_wedded_false_when_disabled() {
        let mut w = w();
        w.bond = 100.0;
        w.enabled = false;
        assert!(!w.is_wedded());
    }

    #[test]
    fn is_sundered_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_sundered());
    }

    // --- bond_fraction / effective_devotion ---

    #[test]
    fn bond_fraction_zero_when_sundered() {
        assert_eq!(w().bond_fraction(), 0.0);
    }

    #[test]
    fn bond_fraction_half_at_midpoint() {
        let mut w = w();
        w.bond = 50.0;
        assert!((w.bond_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_devotion_zero_when_sundered() {
        assert_eq!(w().effective_devotion(100.0), 0.0);
    }

    #[test]
    fn effective_devotion_scales_with_bond() {
        let mut w = w();
        w.bond = 75.0;
        assert!((w.effective_devotion(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_devotion_zero_when_disabled() {
        let mut w = w();
        w.bond = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_devotion(100.0), 0.0);
    }
}
