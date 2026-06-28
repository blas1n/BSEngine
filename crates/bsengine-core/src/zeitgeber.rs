use bevy_ecs::prelude::Component;

/// Circadian-rhythm entrainment tracker named after the zeitgeber
/// (from German Zeit "time" + Geber "giver"), the environmental cue
/// that synchronises an organism's internal circadian clock to the
/// external twenty-four-hour day. The concept was introduced by Jürgen
/// Aschoff and Rütger Wever in the 1950s during isolation experiments
/// at the Max Planck Institute: volunteers living in underground bunkers
/// cut off from all time cues began to free-run on their own internal
/// period — typically 24.2 hours for humans — drifting slowly out of
/// phase with the external world until a periodic light signal was
/// reintroduced, whereupon their rhythms snapped back into lock-step
/// alignment within one or two cycles. The primary zeitgeber for most
/// mammals is the light-dark transition at dawn and dusk, detected by
/// melanopsin-expressing intrinsically photosensitive retinal ganglion
/// cells (ipRGCs) that project directly to the suprachiasmatic nucleus
/// (SCN) of the hypothalamus — the master pacemaker that co-ordinates
/// peripheral clocks in every cell of the body. Secondary zeitgebers
/// include ambient temperature (powerful in ectotherms), scheduled
/// feeding, timed exercise, and social activity. Shift workers, long-
/// haul travellers crossing many time zones, and blind individuals
/// without functional ipRGCs experience the consequences of zeitgeber
/// disruption: their clocks run on internal time while their social and
/// metabolic worlds demand external time, producing the fatigue,
/// impaired cognition, and long-term cardiometabolic risk now called
/// circadian misalignment. `entrainment` builds via `signal(amount)`
/// and accumulates passively at `cue_rate` per second in `tick(dt)` or
/// dissipates via `drift(amount)`.
///
/// Models circadian-clock synchronisation fill levels, environmental-
/// time-cue saturation bars, jet-lag-recovery accumulation trackers,
/// zeitgeber-strength gauges, SCN-entrainment saturation indicators,
/// light-dark-cycle alignment bars, chronobiological-reset fill levels,
/// social-cue synchrony accumulators, free-run-vs-entrained state
/// meters, or any mechanic where a periodic environmental signal slowly
/// pulls an internal rhythm into phase alignment — and where removing
/// the signal causes the internal period to drift free of all external
/// constraints until chaos reigns and every organ is running on its
/// own private calendar.
///
/// `signal(amount)` adds entrainment; fires `just_entrained` when first
/// reaching `max_entrainment`. No-op when disabled.
///
/// `drift(amount)` reduces entrainment immediately; fires `just_drifted`
/// when reaching 0. No-op when disabled or already drifted.
///
/// `tick(dt)` clears both flags, then increases entrainment by
/// `cue_rate * dt` (capped at `max_entrainment`). Fires `just_entrained`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_entrained()` returns `entrainment >= max_entrainment && enabled`.
///
/// `is_drifted()` returns `entrainment == 0.0` (not gated by `enabled`).
///
/// `entrainment_fraction()` returns
/// `(entrainment / max_entrainment).clamp(0, 1)`.
///
/// `effective_synchrony(scale)` returns `scale * entrainment_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — cues at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zeitgeber {
    pub entrainment: f32,
    pub max_entrainment: f32,
    pub cue_rate: f32,
    pub just_entrained: bool,
    pub just_drifted: bool,
    pub enabled: bool,
}

impl Zeitgeber {
    pub fn new(max_entrainment: f32, cue_rate: f32) -> Self {
        Self {
            entrainment: 0.0,
            max_entrainment: max_entrainment.max(0.1),
            cue_rate: cue_rate.max(0.0),
            just_entrained: false,
            just_drifted: false,
            enabled: true,
        }
    }

    /// Add entrainment; fires `just_entrained` when first reaching max.
    /// No-op when disabled.
    pub fn signal(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.entrainment < self.max_entrainment;
        self.entrainment = (self.entrainment + amount).min(self.max_entrainment);
        if was_below && self.entrainment >= self.max_entrainment {
            self.just_entrained = true;
        }
    }

    /// Reduce entrainment; fires `just_drifted` when reaching 0.
    /// No-op when disabled or already drifted.
    pub fn drift(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.entrainment <= 0.0 {
            return;
        }
        self.entrainment = (self.entrainment - amount).max(0.0);
        if self.entrainment <= 0.0 {
            self.just_drifted = true;
        }
    }

    /// Clear flags, then increase entrainment by `cue_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_entrained = false;
        self.just_drifted = false;
        if self.enabled && self.cue_rate > 0.0 && self.entrainment < self.max_entrainment {
            let was_below = self.entrainment < self.max_entrainment;
            self.entrainment = (self.entrainment + self.cue_rate * dt).min(self.max_entrainment);
            if was_below && self.entrainment >= self.max_entrainment {
                self.just_entrained = true;
            }
        }
    }

    /// `true` when entrainment is at maximum and component is enabled.
    pub fn is_entrained(&self) -> bool {
        self.entrainment >= self.max_entrainment && self.enabled
    }

    /// `true` when entrainment is 0 (not gated by `enabled`).
    pub fn is_drifted(&self) -> bool {
        self.entrainment == 0.0
    }

    /// Fraction of maximum entrainment [0.0, 1.0].
    pub fn entrainment_fraction(&self) -> f32 {
        (self.entrainment / self.max_entrainment).clamp(0.0, 1.0)
    }

    /// Returns `scale * entrainment_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_synchrony(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.entrainment_fraction()
    }
}

impl Default for Zeitgeber {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zeitgeber {
        Zeitgeber::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_drifted() {
        let z = z();
        assert_eq!(z.entrainment, 0.0);
        assert!(z.is_drifted());
        assert!(!z.is_entrained());
    }

    #[test]
    fn new_clamps_max_entrainment() {
        let z = Zeitgeber::new(-5.0, 1.5);
        assert!((z.max_entrainment - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_cue_rate() {
        let z = Zeitgeber::new(100.0, -1.5);
        assert_eq!(z.cue_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zeitgeber::default();
        assert!((z.max_entrainment - 100.0).abs() < 1e-5);
        assert!((z.cue_rate - 1.5).abs() < 1e-5);
    }

    // --- signal ---

    #[test]
    fn signal_adds_entrainment() {
        let mut z = z();
        z.signal(40.0);
        assert!((z.entrainment - 40.0).abs() < 1e-3);
    }

    #[test]
    fn signal_clamps_at_max() {
        let mut z = z();
        z.signal(200.0);
        assert!((z.entrainment - 100.0).abs() < 1e-3);
    }

    #[test]
    fn signal_fires_just_entrained_at_max() {
        let mut z = z();
        z.signal(100.0);
        assert!(z.just_entrained);
        assert!(z.is_entrained());
    }

    #[test]
    fn signal_no_just_entrained_when_already_at_max() {
        let mut z = z();
        z.entrainment = 100.0;
        z.signal(10.0);
        assert!(!z.just_entrained);
    }

    #[test]
    fn signal_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.signal(50.0);
        assert_eq!(z.entrainment, 0.0);
    }

    #[test]
    fn signal_no_op_when_amount_zero() {
        let mut z = z();
        z.signal(0.0);
        assert_eq!(z.entrainment, 0.0);
    }

    // --- drift ---

    #[test]
    fn drift_reduces_entrainment() {
        let mut z = z();
        z.entrainment = 60.0;
        z.drift(20.0);
        assert!((z.entrainment - 40.0).abs() < 1e-3);
    }

    #[test]
    fn drift_clamps_at_zero() {
        let mut z = z();
        z.entrainment = 30.0;
        z.drift(200.0);
        assert_eq!(z.entrainment, 0.0);
    }

    #[test]
    fn drift_fires_just_drifted_at_zero() {
        let mut z = z();
        z.entrainment = 30.0;
        z.drift(30.0);
        assert!(z.just_drifted);
    }

    #[test]
    fn drift_no_op_when_already_drifted() {
        let mut z = z();
        z.drift(10.0);
        assert!(!z.just_drifted);
    }

    #[test]
    fn drift_no_op_when_disabled() {
        let mut z = z();
        z.entrainment = 50.0;
        z.enabled = false;
        z.drift(50.0);
        assert!((z.entrainment - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_cues_entrainment() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.entrainment - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_entrained_on_cue_to_max() {
        let mut z = Zeitgeber::new(100.0, 200.0);
        z.entrainment = 95.0;
        z.tick(1.0);
        assert!(z.just_entrained);
        assert!(z.is_entrained());
    }

    #[test]
    fn tick_no_cue_when_already_entrained() {
        let mut z = z();
        z.entrainment = 100.0;
        z.tick(1.0);
        assert!(!z.just_entrained);
    }

    #[test]
    fn tick_no_cue_when_rate_zero() {
        let mut z = Zeitgeber::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.entrainment, 0.0);
    }

    #[test]
    fn tick_no_cue_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.entrainment, 0.0);
    }

    #[test]
    fn tick_clears_just_entrained() {
        let mut z = Zeitgeber::new(100.0, 200.0);
        z.entrainment = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_entrained);
    }

    #[test]
    fn tick_clears_just_drifted() {
        let mut z = z();
        z.entrainment = 10.0;
        z.drift(10.0);
        z.tick(0.016);
        assert!(!z.just_drifted);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.entrainment - 9.0).abs() < 1e-3);
    }

    // --- is_entrained / is_drifted ---

    #[test]
    fn is_entrained_false_when_disabled() {
        let mut z = z();
        z.entrainment = 100.0;
        z.enabled = false;
        assert!(!z.is_entrained());
    }

    #[test]
    fn is_drifted_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_drifted());
    }

    // --- entrainment_fraction / effective_synchrony ---

    #[test]
    fn entrainment_fraction_zero_when_drifted() {
        assert_eq!(z().entrainment_fraction(), 0.0);
    }

    #[test]
    fn entrainment_fraction_half_at_midpoint() {
        let mut z = z();
        z.entrainment = 50.0;
        assert!((z.entrainment_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_synchrony_zero_when_drifted() {
        assert_eq!(z().effective_synchrony(100.0), 0.0);
    }

    #[test]
    fn effective_synchrony_scales_with_entrainment() {
        let mut z = z();
        z.entrainment = 75.0;
        assert!((z.effective_synchrony(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_synchrony_zero_when_disabled() {
        let mut z = z();
        z.entrainment = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_synchrony(100.0), 0.0);
    }
}
