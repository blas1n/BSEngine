use bevy_ecs::prelude::Component;

/// Oath-commitment accumulation tracker named after vow, the noun
/// and verb meaning a solemn promise or pledge, especially one made
/// to a deity or in a religious context — from the Old French vou,
/// from the Latin votum, meaning a vow, wish, or dedication, past
/// participle of vovere (to vow). The Latin root also gave the
/// language vote and devout: a vote was originally a vow of intention,
/// and the devout person was one who had formally dedicated themselves
/// through a vow. In religious practice, a vow is a commitment made
/// before a witness — divine, communal, or both — that transforms the
/// vower's future behaviour from a matter of choice into a matter of
/// obligation. The monastic vow of poverty, chastity, and obedience;
/// the marriage vow; the vow of silence; the Nazirite vow — all share
/// the structure of a voluntary but subsequently binding constraint:
/// after the vow is made, the vower is no longer free to act as though
/// the vow did not exist. This binding quality distinguishes the vow
/// from the ordinary promise, which can be renegotiated; the vow is
/// supposed to be irrevocable, or at least very costly to revoke. In
/// secular usage the word weakened — "I vow to finish this project"
/// carries less gravity than "I take this person as my lawfully wedded
/// spouse" — but the connotation of binding commitment remains. In
/// game mechanics, a vow mechanic models the accumulation of sworn
/// obligation — the build of spiritual or social debt that reaches a
/// point of full commitment, after which the character's options
/// narrow to the path of fulfilling the oath. `pledge` builds via
/// `swear(amount)` and accumulates passively at `devotion_rate` per
/// second in `tick(dt)` or is broken via `renounce(amount)`.
///
/// Models oath-commitment fill levels, devotion-saturation bars,
/// pledge-accumulation trackers, covenant-depth gauges, vow-binding
/// fill levels, spiritual-obligation saturation indicators, oath-
/// charge accumulation bars, commitment-depth meters, sacred-debt
/// fill levels, or any mechanic where a character, faction, or deity
/// slowly accumulates the weight of a sworn promise — word by word,
/// act by act — until the vow reaches full binding force and every
/// subsequent action is measured against the obligation incurred.
///
/// `swear(amount)` adds pledge; fires `just_bound` when first
/// reaching `max_pledge`. No-op when disabled.
///
/// `renounce(amount)` reduces pledge immediately; fires `just_broken`
/// when reaching 0. No-op when disabled or already broken.
///
/// `tick(dt)` clears both flags, then increases pledge by
/// `devotion_rate * dt` (capped at `max_pledge`). Fires `just_bound`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_bound()` returns `pledge >= max_pledge && enabled`.
///
/// `is_broken()` returns `pledge == 0.0` (not gated by `enabled`).
///
/// `pledge_fraction()` returns `(pledge / max_pledge).clamp(0, 1)`.
///
/// `effective_devotion(scale)` returns `scale * pledge_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — builds devotion at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Vow {
    pub pledge: f32,
    pub max_pledge: f32,
    pub devotion_rate: f32,
    pub just_bound: bool,
    pub just_broken: bool,
    pub enabled: bool,
}

impl Vow {
    pub fn new(max_pledge: f32, devotion_rate: f32) -> Self {
        Self {
            pledge: 0.0,
            max_pledge: max_pledge.max(0.1),
            devotion_rate: devotion_rate.max(0.0),
            just_bound: false,
            just_broken: false,
            enabled: true,
        }
    }

    /// Add pledge; fires `just_bound` when first reaching max.
    /// No-op when disabled.
    pub fn swear(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.pledge < self.max_pledge;
        self.pledge = (self.pledge + amount).min(self.max_pledge);
        if was_below && self.pledge >= self.max_pledge {
            self.just_bound = true;
        }
    }

    /// Reduce pledge; fires `just_broken` when reaching 0.
    /// No-op when disabled or already broken.
    pub fn renounce(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.pledge <= 0.0 {
            return;
        }
        self.pledge = (self.pledge - amount).max(0.0);
        if self.pledge <= 0.0 {
            self.just_broken = true;
        }
    }

    /// Clear flags, then increase pledge by `devotion_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_bound = false;
        self.just_broken = false;
        if self.enabled && self.devotion_rate > 0.0 && self.pledge < self.max_pledge {
            let was_below = self.pledge < self.max_pledge;
            self.pledge = (self.pledge + self.devotion_rate * dt).min(self.max_pledge);
            if was_below && self.pledge >= self.max_pledge {
                self.just_bound = true;
            }
        }
    }

    /// `true` when pledge is at maximum and component is enabled.
    pub fn is_bound(&self) -> bool {
        self.pledge >= self.max_pledge && self.enabled
    }

    /// `true` when pledge is 0 (not gated by `enabled`).
    pub fn is_broken(&self) -> bool {
        self.pledge == 0.0
    }

    /// Fraction of maximum pledge [0.0, 1.0].
    pub fn pledge_fraction(&self) -> f32 {
        (self.pledge / self.max_pledge).clamp(0.0, 1.0)
    }

    /// Returns `scale * pledge_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_devotion(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.pledge_fraction()
    }
}

impl Default for Vow {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v() -> Vow {
        Vow::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_broken() {
        let v = v();
        assert_eq!(v.pledge, 0.0);
        assert!(v.is_broken());
        assert!(!v.is_bound());
    }

    #[test]
    fn new_clamps_max_pledge() {
        let v = Vow::new(-5.0, 1.5);
        assert!((v.max_pledge - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_devotion_rate() {
        let v = Vow::new(100.0, -1.5);
        assert_eq!(v.devotion_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let v = Vow::default();
        assert!((v.max_pledge - 100.0).abs() < 1e-5);
        assert!((v.devotion_rate - 1.5).abs() < 1e-5);
    }

    // --- swear ---

    #[test]
    fn swear_adds_pledge() {
        let mut v = v();
        v.swear(40.0);
        assert!((v.pledge - 40.0).abs() < 1e-3);
    }

    #[test]
    fn swear_clamps_at_max() {
        let mut v = v();
        v.swear(200.0);
        assert!((v.pledge - 100.0).abs() < 1e-3);
    }

    #[test]
    fn swear_fires_just_bound_at_max() {
        let mut v = v();
        v.swear(100.0);
        assert!(v.just_bound);
        assert!(v.is_bound());
    }

    #[test]
    fn swear_no_just_bound_when_already_at_max() {
        let mut v = v();
        v.pledge = 100.0;
        v.swear(10.0);
        assert!(!v.just_bound);
    }

    #[test]
    fn swear_no_op_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.swear(50.0);
        assert_eq!(v.pledge, 0.0);
    }

    #[test]
    fn swear_no_op_when_amount_zero() {
        let mut v = v();
        v.swear(0.0);
        assert_eq!(v.pledge, 0.0);
    }

    // --- renounce ---

    #[test]
    fn renounce_reduces_pledge() {
        let mut v = v();
        v.pledge = 60.0;
        v.renounce(20.0);
        assert!((v.pledge - 40.0).abs() < 1e-3);
    }

    #[test]
    fn renounce_clamps_at_zero() {
        let mut v = v();
        v.pledge = 30.0;
        v.renounce(200.0);
        assert_eq!(v.pledge, 0.0);
    }

    #[test]
    fn renounce_fires_just_broken_at_zero() {
        let mut v = v();
        v.pledge = 30.0;
        v.renounce(30.0);
        assert!(v.just_broken);
    }

    #[test]
    fn renounce_no_op_when_already_broken() {
        let mut v = v();
        v.renounce(10.0);
        assert!(!v.just_broken);
    }

    #[test]
    fn renounce_no_op_when_disabled() {
        let mut v = v();
        v.pledge = 50.0;
        v.enabled = false;
        v.renounce(50.0);
        assert!((v.pledge - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_pledge() {
        let mut v = v(); // rate=1.5
        v.tick(4.0); // 0 + 1.5*4 = 6
        assert!((v.pledge - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_bound_on_pledge_to_max() {
        let mut v = Vow::new(100.0, 200.0);
        v.pledge = 95.0;
        v.tick(1.0);
        assert!(v.just_bound);
        assert!(v.is_bound());
    }

    #[test]
    fn tick_no_build_when_already_bound() {
        let mut v = v();
        v.pledge = 100.0;
        v.tick(1.0);
        assert!(!v.just_bound);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut v = Vow::new(100.0, 0.0);
        v.tick(100.0);
        assert_eq!(v.pledge, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.tick(1.0);
        assert_eq!(v.pledge, 0.0);
    }

    #[test]
    fn tick_clears_just_bound() {
        let mut v = Vow::new(100.0, 200.0);
        v.pledge = 95.0;
        v.tick(1.0);
        v.tick(0.016);
        assert!(!v.just_bound);
    }

    #[test]
    fn tick_clears_just_broken() {
        let mut v = v();
        v.pledge = 10.0;
        v.renounce(10.0);
        v.tick(0.016);
        assert!(!v.just_broken);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut v = v(); // rate=1.5
        v.tick(6.0); // 1.5*6 = 9
        assert!((v.pledge - 9.0).abs() < 1e-3);
    }

    // --- is_bound / is_broken ---

    #[test]
    fn is_bound_false_when_disabled() {
        let mut v = v();
        v.pledge = 100.0;
        v.enabled = false;
        assert!(!v.is_bound());
    }

    #[test]
    fn is_broken_not_gated_by_enabled() {
        let mut v = v();
        v.enabled = false;
        assert!(v.is_broken());
    }

    // --- pledge_fraction / effective_devotion ---

    #[test]
    fn pledge_fraction_zero_when_broken() {
        assert_eq!(v().pledge_fraction(), 0.0);
    }

    #[test]
    fn pledge_fraction_half_at_midpoint() {
        let mut v = v();
        v.pledge = 50.0;
        assert!((v.pledge_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_devotion_zero_when_broken() {
        assert_eq!(v().effective_devotion(100.0), 0.0);
    }

    #[test]
    fn effective_devotion_scales_with_pledge() {
        let mut v = v();
        v.pledge = 75.0;
        assert!((v.effective_devotion(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_devotion_zero_when_disabled() {
        let mut v = v();
        v.pledge = 50.0;
        v.enabled = false;
        assert_eq!(v.effective_devotion(100.0), 0.0);
    }
}
