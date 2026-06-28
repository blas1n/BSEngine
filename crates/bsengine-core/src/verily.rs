use bevy_ecs::prelude::Component;

/// Solemn-conviction accumulation tracker named after verily, the
/// archaic adverb meaning "in truth; certainly; indeed" — a word
/// whose trajectory through English charts the slow contraction of
/// the formal register from liturgy to irony. Verily appears
/// throughout the King James Bible as the English rendering of the
/// Greek amen at the head of Christ's emphatic declarations — "verily,
/// verily, I say unto you" — functioning not merely as an intensifier
/// but as a guarantee, a promissory note on the truth of what follows,
/// backed by the speaker's entire authority. By the seventeenth
/// century the word had already begun to acquire a slightly elevated
/// ring that separated it from the everyday surely or indeed; by the
/// nineteenth it was largely confined to formal prose and liturgical
/// contexts; by the twentieth it had become a signal of archaism,
/// deliberately invoked to confer either solemnity or ironic distance
/// on whatever followed it. Shakespeare used it without self-consciousness;
/// Dickens used it to mark characters who aspired to biblical gravity;
/// modern English uses it almost exclusively when it wants to mock
/// those same aspirations, or when it genuinely wants to reach back
/// through the secular centuries to the moment when an assertion could
/// be delivered with the force of an oath rather than merely a
/// statement of personal belief. In game narrative, characters who
/// speak in verily make promises that the rules of their world hold
/// binding — and the mechanic that backs them needs to track whether
/// the conviction has been built up sufficiently to sustain the weight
/// of what is being sworn. `conviction` builds via `affirm(amount)`
/// and accumulates passively at `oath_rate` per second in `tick(dt)`
/// or dissolves via `recant(amount)`.
///
/// Models solemn-conviction fill levels, oath-strength saturation
/// bars, pledge-commitment accumulators, vow-intensity gauges, liturgical-
/// certainty fill levels, oath-binding saturation indicators, sworn-
/// word accumulation bars, covenant-strength meters, promissory-
/// intensity fill levels, or any mechanic where a character, faction,
/// or deity slowly builds the weight of a binding assertion until the
/// declaration is backed by sufficient conviction to compel compliance
/// — or until a betrayal, contradiction, or revelation forces a
/// recantation that collapses the entire edifice back to silence.
///
/// `affirm(amount)` adds conviction; fires `just_sworn` when first
/// reaching `max_conviction`. No-op when disabled.
///
/// `recant(amount)` reduces conviction immediately; fires `just_recanted`
/// when reaching 0. No-op when disabled or already recanted.
///
/// `tick(dt)` clears both flags, then increases conviction by
/// `oath_rate * dt` (capped at `max_conviction`). Fires `just_sworn`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_sworn()` returns `conviction >= max_conviction && enabled`.
///
/// `is_recanted()` returns `conviction == 0.0` (not gated by `enabled`).
///
/// `conviction_fraction()` returns
/// `(conviction / max_conviction).clamp(0, 1)`.
///
/// `effective_pledge(scale)` returns `scale * conviction_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — builds conviction at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Verily {
    pub conviction: f32,
    pub max_conviction: f32,
    pub oath_rate: f32,
    pub just_sworn: bool,
    pub just_recanted: bool,
    pub enabled: bool,
}

impl Verily {
    pub fn new(max_conviction: f32, oath_rate: f32) -> Self {
        Self {
            conviction: 0.0,
            max_conviction: max_conviction.max(0.1),
            oath_rate: oath_rate.max(0.0),
            just_sworn: false,
            just_recanted: false,
            enabled: true,
        }
    }

    /// Add conviction; fires `just_sworn` when first reaching max.
    /// No-op when disabled.
    pub fn affirm(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.conviction < self.max_conviction;
        self.conviction = (self.conviction + amount).min(self.max_conviction);
        if was_below && self.conviction >= self.max_conviction {
            self.just_sworn = true;
        }
    }

    /// Reduce conviction; fires `just_recanted` when reaching 0.
    /// No-op when disabled or already recanted.
    pub fn recant(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.conviction <= 0.0 {
            return;
        }
        self.conviction = (self.conviction - amount).max(0.0);
        if self.conviction <= 0.0 {
            self.just_recanted = true;
        }
    }

    /// Clear flags, then increase conviction by `oath_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_sworn = false;
        self.just_recanted = false;
        if self.enabled && self.oath_rate > 0.0 && self.conviction < self.max_conviction {
            let was_below = self.conviction < self.max_conviction;
            self.conviction = (self.conviction + self.oath_rate * dt).min(self.max_conviction);
            if was_below && self.conviction >= self.max_conviction {
                self.just_sworn = true;
            }
        }
    }

    /// `true` when conviction is at maximum and component is enabled.
    pub fn is_sworn(&self) -> bool {
        self.conviction >= self.max_conviction && self.enabled
    }

    /// `true` when conviction is 0 (not gated by `enabled`).
    pub fn is_recanted(&self) -> bool {
        self.conviction == 0.0
    }

    /// Fraction of maximum conviction [0.0, 1.0].
    pub fn conviction_fraction(&self) -> f32 {
        (self.conviction / self.max_conviction).clamp(0.0, 1.0)
    }

    /// Returns `scale * conviction_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_pledge(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.conviction_fraction()
    }
}

impl Default for Verily {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v() -> Verily {
        Verily::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_recanted() {
        let v = v();
        assert_eq!(v.conviction, 0.0);
        assert!(v.is_recanted());
        assert!(!v.is_sworn());
    }

    #[test]
    fn new_clamps_max_conviction() {
        let v = Verily::new(-5.0, 1.5);
        assert!((v.max_conviction - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_oath_rate() {
        let v = Verily::new(100.0, -1.5);
        assert_eq!(v.oath_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let v = Verily::default();
        assert!((v.max_conviction - 100.0).abs() < 1e-5);
        assert!((v.oath_rate - 1.5).abs() < 1e-5);
    }

    // --- affirm ---

    #[test]
    fn affirm_adds_conviction() {
        let mut v = v();
        v.affirm(40.0);
        assert!((v.conviction - 40.0).abs() < 1e-3);
    }

    #[test]
    fn affirm_clamps_at_max() {
        let mut v = v();
        v.affirm(200.0);
        assert!((v.conviction - 100.0).abs() < 1e-3);
    }

    #[test]
    fn affirm_fires_just_sworn_at_max() {
        let mut v = v();
        v.affirm(100.0);
        assert!(v.just_sworn);
        assert!(v.is_sworn());
    }

    #[test]
    fn affirm_no_just_sworn_when_already_at_max() {
        let mut v = v();
        v.conviction = 100.0;
        v.affirm(10.0);
        assert!(!v.just_sworn);
    }

    #[test]
    fn affirm_no_op_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.affirm(50.0);
        assert_eq!(v.conviction, 0.0);
    }

    #[test]
    fn affirm_no_op_when_amount_zero() {
        let mut v = v();
        v.affirm(0.0);
        assert_eq!(v.conviction, 0.0);
    }

    // --- recant ---

    #[test]
    fn recant_reduces_conviction() {
        let mut v = v();
        v.conviction = 60.0;
        v.recant(20.0);
        assert!((v.conviction - 40.0).abs() < 1e-3);
    }

    #[test]
    fn recant_clamps_at_zero() {
        let mut v = v();
        v.conviction = 30.0;
        v.recant(200.0);
        assert_eq!(v.conviction, 0.0);
    }

    #[test]
    fn recant_fires_just_recanted_at_zero() {
        let mut v = v();
        v.conviction = 30.0;
        v.recant(30.0);
        assert!(v.just_recanted);
    }

    #[test]
    fn recant_no_op_when_already_recanted() {
        let mut v = v();
        v.recant(10.0);
        assert!(!v.just_recanted);
    }

    #[test]
    fn recant_no_op_when_disabled() {
        let mut v = v();
        v.conviction = 50.0;
        v.enabled = false;
        v.recant(50.0);
        assert!((v.conviction - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_conviction() {
        let mut v = v(); // rate=1.5
        v.tick(4.0); // 0 + 1.5*4 = 6
        assert!((v.conviction - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_sworn_on_conviction_to_max() {
        let mut v = Verily::new(100.0, 200.0);
        v.conviction = 95.0;
        v.tick(1.0);
        assert!(v.just_sworn);
        assert!(v.is_sworn());
    }

    #[test]
    fn tick_no_build_when_already_sworn() {
        let mut v = v();
        v.conviction = 100.0;
        v.tick(1.0);
        assert!(!v.just_sworn);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut v = Verily::new(100.0, 0.0);
        v.tick(100.0);
        assert_eq!(v.conviction, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.tick(1.0);
        assert_eq!(v.conviction, 0.0);
    }

    #[test]
    fn tick_clears_just_sworn() {
        let mut v = Verily::new(100.0, 200.0);
        v.conviction = 95.0;
        v.tick(1.0);
        v.tick(0.016);
        assert!(!v.just_sworn);
    }

    #[test]
    fn tick_clears_just_recanted() {
        let mut v = v();
        v.conviction = 10.0;
        v.recant(10.0);
        v.tick(0.016);
        assert!(!v.just_recanted);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut v = v(); // rate=1.5
        v.tick(6.0); // 1.5*6 = 9
        assert!((v.conviction - 9.0).abs() < 1e-3);
    }

    // --- is_sworn / is_recanted ---

    #[test]
    fn is_sworn_false_when_disabled() {
        let mut v = v();
        v.conviction = 100.0;
        v.enabled = false;
        assert!(!v.is_sworn());
    }

    #[test]
    fn is_recanted_not_gated_by_enabled() {
        let mut v = v();
        v.enabled = false;
        assert!(v.is_recanted());
    }

    // --- conviction_fraction / effective_pledge ---

    #[test]
    fn conviction_fraction_zero_when_recanted() {
        assert_eq!(v().conviction_fraction(), 0.0);
    }

    #[test]
    fn conviction_fraction_half_at_midpoint() {
        let mut v = v();
        v.conviction = 50.0;
        assert!((v.conviction_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_pledge_zero_when_recanted() {
        assert_eq!(v().effective_pledge(100.0), 0.0);
    }

    #[test]
    fn effective_pledge_scales_with_conviction() {
        let mut v = v();
        v.conviction = 75.0;
        assert!((v.effective_pledge(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_pledge_zero_when_disabled() {
        let mut v = v();
        v.conviction = 50.0;
        v.enabled = false;
        assert_eq!(v.effective_pledge(100.0), 0.0);
    }
}
