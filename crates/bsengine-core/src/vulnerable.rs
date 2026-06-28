use bevy_ecs::prelude::Component;

/// Susceptibility-exposure accumulation tracker named after vulnerable,
/// the adjective meaning capable of being physically or emotionally
/// wounded; open to attack, harm, or damage — from the Latin
/// vulnerabilis, meaning wounding, from vulnerare (to wound), from
/// vulnus (a wound). The medical Latin gave English both vulnerable
/// and its antonym invulnerable, as well as wound through a Germanic
/// cognate, since the Proto-Indo-European root welh- or wele- (to
/// strike) left traces across the Indo-European language family. The
/// word in English carries two registers: the tactical and the
/// emotional. In tactical usage, a position is vulnerable when it
/// can be attacked; a flank is vulnerable when it is unprotected;
/// a structure is vulnerable to fire, to flood, to vibration. In
/// emotional usage, a person is vulnerable when they have opened
/// themselves to the possibility of being hurt — when they have
/// removed a psychological defence and shown something that matters
/// to them in a context where it could be damaged. The two senses
/// share the same structure: vulnerability is the absence of a
/// protective barrier, whether physical or psychological, combined
/// with the presence of a force or agent capable of exploiting that
/// absence. In ecology, a species is vulnerable (IUCN status) when
/// population decline places it at risk of becoming threatened: the
/// vulnerability is systemic rather than individual, a property of
/// the population's trajectory. In game mechanics, a vulnerability
/// mechanic tracks the accumulation of exposed weakness — the slow
/// build of armour deterioration, guard-break pressure, emotional
/// openness, or structural damage that eventually reaches a threshold
/// at which the target is fully vulnerable. `exposure` builds via
/// `weaken(amount)` and accumulates passively at `breach_rate` per
/// second in `tick(dt)` or is reduced via `fortify(amount)`.
///
/// Models susceptibility-exposure fill levels, guard-break saturation
/// bars, armour-deterioration accumulators, weakness-accumulation
/// gauges, emotional-openness fill levels, defence-erosion saturation
/// indicators, damage-amplification accumulation bars, stagger-
/// threshold meters, shield-decay fill levels, or any mechanic where
/// a character, structure, or system slowly accumulates the damage,
/// pressure, or exposure that strips away its defences until a
/// threshold is reached and the entity becomes fully vulnerable —
/// all its protections gone, all its weaknesses exposed, ready to
/// receive the full weight of whatever force has been building
/// against it.
///
/// `weaken(amount)` adds exposure; fires `just_exposed` when first
/// reaching `max_exposure`. No-op when disabled.
///
/// `fortify(amount)` reduces exposure immediately; fires
/// `just_hardened` when reaching 0. No-op when disabled or already
/// hardened.
///
/// `tick(dt)` clears both flags, then increases exposure by
/// `breach_rate * dt` (capped at `max_exposure`). Fires `just_exposed`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_exposed()` returns `exposure >= max_exposure && enabled`.
///
/// `is_hardened()` returns `exposure == 0.0` (not gated by `enabled`).
///
/// `exposure_fraction()` returns
/// `(exposure / max_exposure).clamp(0, 1)`.
///
/// `effective_weakness(scale)` returns `scale * exposure_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — breaches at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Vulnerable {
    pub exposure: f32,
    pub max_exposure: f32,
    pub breach_rate: f32,
    pub just_exposed: bool,
    pub just_hardened: bool,
    pub enabled: bool,
}

impl Vulnerable {
    pub fn new(max_exposure: f32, breach_rate: f32) -> Self {
        Self {
            exposure: 0.0,
            max_exposure: max_exposure.max(0.1),
            breach_rate: breach_rate.max(0.0),
            just_exposed: false,
            just_hardened: false,
            enabled: true,
        }
    }

    /// Add exposure; fires `just_exposed` when first reaching max.
    /// No-op when disabled.
    pub fn weaken(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.exposure < self.max_exposure;
        self.exposure = (self.exposure + amount).min(self.max_exposure);
        if was_below && self.exposure >= self.max_exposure {
            self.just_exposed = true;
        }
    }

    /// Reduce exposure; fires `just_hardened` when reaching 0.
    /// No-op when disabled or already hardened.
    pub fn fortify(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.exposure <= 0.0 {
            return;
        }
        self.exposure = (self.exposure - amount).max(0.0);
        if self.exposure <= 0.0 {
            self.just_hardened = true;
        }
    }

    /// Clear flags, then increase exposure by `breach_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_exposed = false;
        self.just_hardened = false;
        if self.enabled && self.breach_rate > 0.0 && self.exposure < self.max_exposure {
            let was_below = self.exposure < self.max_exposure;
            self.exposure = (self.exposure + self.breach_rate * dt).min(self.max_exposure);
            if was_below && self.exposure >= self.max_exposure {
                self.just_exposed = true;
            }
        }
    }

    /// `true` when exposure is at maximum and component is enabled.
    pub fn is_exposed(&self) -> bool {
        self.exposure >= self.max_exposure && self.enabled
    }

    /// `true` when exposure is 0 (not gated by `enabled`).
    pub fn is_hardened(&self) -> bool {
        self.exposure == 0.0
    }

    /// Fraction of maximum exposure [0.0, 1.0].
    pub fn exposure_fraction(&self) -> f32 {
        (self.exposure / self.max_exposure).clamp(0.0, 1.0)
    }

    /// Returns `scale * exposure_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_weakness(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.exposure_fraction()
    }
}

impl Default for Vulnerable {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v() -> Vulnerable {
        Vulnerable::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_hardened() {
        let v = v();
        assert_eq!(v.exposure, 0.0);
        assert!(v.is_hardened());
        assert!(!v.is_exposed());
    }

    #[test]
    fn new_clamps_max_exposure() {
        let v = Vulnerable::new(-5.0, 1.5);
        assert!((v.max_exposure - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_breach_rate() {
        let v = Vulnerable::new(100.0, -1.5);
        assert_eq!(v.breach_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let v = Vulnerable::default();
        assert!((v.max_exposure - 100.0).abs() < 1e-5);
        assert!((v.breach_rate - 1.5).abs() < 1e-5);
    }

    // --- weaken ---

    #[test]
    fn weaken_adds_exposure() {
        let mut v = v();
        v.weaken(40.0);
        assert!((v.exposure - 40.0).abs() < 1e-3);
    }

    #[test]
    fn weaken_clamps_at_max() {
        let mut v = v();
        v.weaken(200.0);
        assert!((v.exposure - 100.0).abs() < 1e-3);
    }

    #[test]
    fn weaken_fires_just_exposed_at_max() {
        let mut v = v();
        v.weaken(100.0);
        assert!(v.just_exposed);
        assert!(v.is_exposed());
    }

    #[test]
    fn weaken_no_just_exposed_when_already_at_max() {
        let mut v = v();
        v.exposure = 100.0;
        v.weaken(10.0);
        assert!(!v.just_exposed);
    }

    #[test]
    fn weaken_no_op_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.weaken(50.0);
        assert_eq!(v.exposure, 0.0);
    }

    #[test]
    fn weaken_no_op_when_amount_zero() {
        let mut v = v();
        v.weaken(0.0);
        assert_eq!(v.exposure, 0.0);
    }

    // --- fortify ---

    #[test]
    fn fortify_reduces_exposure() {
        let mut v = v();
        v.exposure = 60.0;
        v.fortify(20.0);
        assert!((v.exposure - 40.0).abs() < 1e-3);
    }

    #[test]
    fn fortify_clamps_at_zero() {
        let mut v = v();
        v.exposure = 30.0;
        v.fortify(200.0);
        assert_eq!(v.exposure, 0.0);
    }

    #[test]
    fn fortify_fires_just_hardened_at_zero() {
        let mut v = v();
        v.exposure = 30.0;
        v.fortify(30.0);
        assert!(v.just_hardened);
    }

    #[test]
    fn fortify_no_op_when_already_hardened() {
        let mut v = v();
        v.fortify(10.0);
        assert!(!v.just_hardened);
    }

    #[test]
    fn fortify_no_op_when_disabled() {
        let mut v = v();
        v.exposure = 50.0;
        v.enabled = false;
        v.fortify(50.0);
        assert!((v.exposure - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_exposure() {
        let mut v = v(); // rate=1.5
        v.tick(4.0); // 0 + 1.5*4 = 6
        assert!((v.exposure - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_exposed_on_exposure_to_max() {
        let mut v = Vulnerable::new(100.0, 200.0);
        v.exposure = 95.0;
        v.tick(1.0);
        assert!(v.just_exposed);
        assert!(v.is_exposed());
    }

    #[test]
    fn tick_no_build_when_already_exposed() {
        let mut v = v();
        v.exposure = 100.0;
        v.tick(1.0);
        assert!(!v.just_exposed);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut v = Vulnerable::new(100.0, 0.0);
        v.tick(100.0);
        assert_eq!(v.exposure, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.tick(1.0);
        assert_eq!(v.exposure, 0.0);
    }

    #[test]
    fn tick_clears_just_exposed() {
        let mut v = Vulnerable::new(100.0, 200.0);
        v.exposure = 95.0;
        v.tick(1.0);
        v.tick(0.016);
        assert!(!v.just_exposed);
    }

    #[test]
    fn tick_clears_just_hardened() {
        let mut v = v();
        v.exposure = 10.0;
        v.fortify(10.0);
        v.tick(0.016);
        assert!(!v.just_hardened);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut v = v(); // rate=1.5
        v.tick(6.0); // 1.5*6 = 9
        assert!((v.exposure - 9.0).abs() < 1e-3);
    }

    // --- is_exposed / is_hardened ---

    #[test]
    fn is_exposed_false_when_disabled() {
        let mut v = v();
        v.exposure = 100.0;
        v.enabled = false;
        assert!(!v.is_exposed());
    }

    #[test]
    fn is_hardened_not_gated_by_enabled() {
        let mut v = v();
        v.enabled = false;
        assert!(v.is_hardened());
    }

    // --- exposure_fraction / effective_weakness ---

    #[test]
    fn exposure_fraction_zero_when_hardened() {
        assert_eq!(v().exposure_fraction(), 0.0);
    }

    #[test]
    fn exposure_fraction_half_at_midpoint() {
        let mut v = v();
        v.exposure = 50.0;
        assert!((v.exposure_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_weakness_zero_when_hardened() {
        assert_eq!(v().effective_weakness(100.0), 0.0);
    }

    #[test]
    fn effective_weakness_scales_with_exposure() {
        let mut v = v();
        v.exposure = 75.0;
        assert!((v.effective_weakness(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_weakness_zero_when_disabled() {
        let mut v = v();
        v.exposure = 50.0;
        v.enabled = false;
        assert_eq!(v.effective_weakness(100.0), 0.0);
    }
}
