use bevy_ecs::prelude::Component;

/// Truth-confirmation accumulation tracker named after verify, the
/// verb meaning to ascertain the truth, accuracy, or reality of
/// something — from the Latin verus (true) and facere (to make),
/// so literally "to make true" in the sense of demonstrating rather
/// than manufacturing truth. The verification impulse runs through
/// every domain of human knowledge: a scientist replicates a result
/// not because they distrust the original experimenter but because
/// replication is the process by which a finding is promoted from
/// observation to fact; a detective assembles corroborating evidence
/// not because any single witness is necessarily lying but because
/// testimony is fallible and the chain of inference must be strong
/// enough to carry the weight of the conclusion. The word entered
/// Middle English through Old French vérifier and carries both its
/// practical and its epistemological sense simultaneously: to verify
/// a document is to authenticate its signature, to verify a claim is
/// to confirm its correspondence with an observable state of affairs.
/// In computing, verification occupies a distinct niche from
/// validation: to verify is to check that the system was built
/// correctly (does the code do what the specification says?); to
/// validate is to check that the right system was built (does the
/// specification say the right thing?). In formal methods, a program
/// is verified when a proof has been constructed that its behaviour
/// under all inputs satisfies its postconditions — a standard so
/// demanding that most real-world software is merely tested rather
/// than verified, the proof replaced by a sample large enough to
/// instil reasonable confidence. `confidence` builds via
/// `confirm(amount)` and accumulates passively at `probe_rate` per
/// second in `tick(dt)` or collapses via `refute(amount)`.
///
/// Models truth-confirmation fill levels, verification-confidence
/// saturation bars, corroboration-accumulation trackers, evidence-
/// chain gauges, proof-completion fill levels, forensic-certainty
/// saturation indicators, attestation-accumulation bars, replication-
/// confidence meters, formal-verification fill levels, or any mechanic
/// where a clue, experiment, or observation slowly builds toward
/// certainty — each new piece of evidence adding a layer of
/// corroboration until the conclusion stands unassailable, or until
/// a single contradictory fact collapses the entire chain and the
/// investigator must start again.
///
/// `confirm(amount)` adds confidence; fires `just_verified` when first
/// reaching `max_confidence`. No-op when disabled.
///
/// `refute(amount)` reduces confidence immediately; fires `just_refuted`
/// when reaching 0. No-op when disabled or already refuted.
///
/// `tick(dt)` clears both flags, then increases confidence by
/// `probe_rate * dt` (capped at `max_confidence`). Fires `just_verified`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_verified()` returns `confidence >= max_confidence && enabled`.
///
/// `is_refuted()` returns `confidence == 0.0` (not gated by `enabled`).
///
/// `confidence_fraction()` returns
/// `(confidence / max_confidence).clamp(0, 1)`.
///
/// `effective_assurance(scale)` returns `scale * confidence_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — probes at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Verify {
    pub confidence: f32,
    pub max_confidence: f32,
    pub probe_rate: f32,
    pub just_verified: bool,
    pub just_refuted: bool,
    pub enabled: bool,
}

impl Verify {
    pub fn new(max_confidence: f32, probe_rate: f32) -> Self {
        Self {
            confidence: 0.0,
            max_confidence: max_confidence.max(0.1),
            probe_rate: probe_rate.max(0.0),
            just_verified: false,
            just_refuted: false,
            enabled: true,
        }
    }

    /// Add confidence; fires `just_verified` when first reaching max.
    /// No-op when disabled.
    pub fn confirm(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.confidence < self.max_confidence;
        self.confidence = (self.confidence + amount).min(self.max_confidence);
        if was_below && self.confidence >= self.max_confidence {
            self.just_verified = true;
        }
    }

    /// Reduce confidence; fires `just_refuted` when reaching 0.
    /// No-op when disabled or already refuted.
    pub fn refute(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.confidence <= 0.0 {
            return;
        }
        self.confidence = (self.confidence - amount).max(0.0);
        if self.confidence <= 0.0 {
            self.just_refuted = true;
        }
    }

    /// Clear flags, then increase confidence by `probe_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_verified = false;
        self.just_refuted = false;
        if self.enabled && self.probe_rate > 0.0 && self.confidence < self.max_confidence {
            let was_below = self.confidence < self.max_confidence;
            self.confidence = (self.confidence + self.probe_rate * dt).min(self.max_confidence);
            if was_below && self.confidence >= self.max_confidence {
                self.just_verified = true;
            }
        }
    }

    /// `true` when confidence is at maximum and component is enabled.
    pub fn is_verified(&self) -> bool {
        self.confidence >= self.max_confidence && self.enabled
    }

    /// `true` when confidence is 0 (not gated by `enabled`).
    pub fn is_refuted(&self) -> bool {
        self.confidence == 0.0
    }

    /// Fraction of maximum confidence [0.0, 1.0].
    pub fn confidence_fraction(&self) -> f32 {
        (self.confidence / self.max_confidence).clamp(0.0, 1.0)
    }

    /// Returns `scale * confidence_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_assurance(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.confidence_fraction()
    }
}

impl Default for Verify {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v() -> Verify {
        Verify::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_refuted() {
        let v = v();
        assert_eq!(v.confidence, 0.0);
        assert!(v.is_refuted());
        assert!(!v.is_verified());
    }

    #[test]
    fn new_clamps_max_confidence() {
        let v = Verify::new(-5.0, 1.5);
        assert!((v.max_confidence - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_probe_rate() {
        let v = Verify::new(100.0, -1.5);
        assert_eq!(v.probe_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let v = Verify::default();
        assert!((v.max_confidence - 100.0).abs() < 1e-5);
        assert!((v.probe_rate - 1.5).abs() < 1e-5);
    }

    // --- confirm ---

    #[test]
    fn confirm_adds_confidence() {
        let mut v = v();
        v.confirm(40.0);
        assert!((v.confidence - 40.0).abs() < 1e-3);
    }

    #[test]
    fn confirm_clamps_at_max() {
        let mut v = v();
        v.confirm(200.0);
        assert!((v.confidence - 100.0).abs() < 1e-3);
    }

    #[test]
    fn confirm_fires_just_verified_at_max() {
        let mut v = v();
        v.confirm(100.0);
        assert!(v.just_verified);
        assert!(v.is_verified());
    }

    #[test]
    fn confirm_no_just_verified_when_already_at_max() {
        let mut v = v();
        v.confidence = 100.0;
        v.confirm(10.0);
        assert!(!v.just_verified);
    }

    #[test]
    fn confirm_no_op_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.confirm(50.0);
        assert_eq!(v.confidence, 0.0);
    }

    #[test]
    fn confirm_no_op_when_amount_zero() {
        let mut v = v();
        v.confirm(0.0);
        assert_eq!(v.confidence, 0.0);
    }

    // --- refute ---

    #[test]
    fn refute_reduces_confidence() {
        let mut v = v();
        v.confidence = 60.0;
        v.refute(20.0);
        assert!((v.confidence - 40.0).abs() < 1e-3);
    }

    #[test]
    fn refute_clamps_at_zero() {
        let mut v = v();
        v.confidence = 30.0;
        v.refute(200.0);
        assert_eq!(v.confidence, 0.0);
    }

    #[test]
    fn refute_fires_just_refuted_at_zero() {
        let mut v = v();
        v.confidence = 30.0;
        v.refute(30.0);
        assert!(v.just_refuted);
    }

    #[test]
    fn refute_no_op_when_already_refuted() {
        let mut v = v();
        v.refute(10.0);
        assert!(!v.just_refuted);
    }

    #[test]
    fn refute_no_op_when_disabled() {
        let mut v = v();
        v.confidence = 50.0;
        v.enabled = false;
        v.refute(50.0);
        assert!((v.confidence - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_confidence() {
        let mut v = v(); // rate=1.5
        v.tick(4.0); // 0 + 1.5*4 = 6
        assert!((v.confidence - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_verified_on_confidence_to_max() {
        let mut v = Verify::new(100.0, 200.0);
        v.confidence = 95.0;
        v.tick(1.0);
        assert!(v.just_verified);
        assert!(v.is_verified());
    }

    #[test]
    fn tick_no_build_when_already_verified() {
        let mut v = v();
        v.confidence = 100.0;
        v.tick(1.0);
        assert!(!v.just_verified);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut v = Verify::new(100.0, 0.0);
        v.tick(100.0);
        assert_eq!(v.confidence, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.tick(1.0);
        assert_eq!(v.confidence, 0.0);
    }

    #[test]
    fn tick_clears_just_verified() {
        let mut v = Verify::new(100.0, 200.0);
        v.confidence = 95.0;
        v.tick(1.0);
        v.tick(0.016);
        assert!(!v.just_verified);
    }

    #[test]
    fn tick_clears_just_refuted() {
        let mut v = v();
        v.confidence = 10.0;
        v.refute(10.0);
        v.tick(0.016);
        assert!(!v.just_refuted);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut v = v(); // rate=1.5
        v.tick(6.0); // 1.5*6 = 9
        assert!((v.confidence - 9.0).abs() < 1e-3);
    }

    // --- is_verified / is_refuted ---

    #[test]
    fn is_verified_false_when_disabled() {
        let mut v = v();
        v.confidence = 100.0;
        v.enabled = false;
        assert!(!v.is_verified());
    }

    #[test]
    fn is_refuted_not_gated_by_enabled() {
        let mut v = v();
        v.enabled = false;
        assert!(v.is_refuted());
    }

    // --- confidence_fraction / effective_assurance ---

    #[test]
    fn confidence_fraction_zero_when_refuted() {
        assert_eq!(v().confidence_fraction(), 0.0);
    }

    #[test]
    fn confidence_fraction_half_at_midpoint() {
        let mut v = v();
        v.confidence = 50.0;
        assert!((v.confidence_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_assurance_zero_when_refuted() {
        assert_eq!(v().effective_assurance(100.0), 0.0);
    }

    #[test]
    fn effective_assurance_scales_with_confidence() {
        let mut v = v();
        v.confidence = 75.0;
        assert!((v.effective_assurance(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_assurance_zero_when_disabled() {
        let mut v = v();
        v.confidence = 50.0;
        v.enabled = false;
        assert_eq!(v.effective_assurance(100.0), 0.0);
    }
}
