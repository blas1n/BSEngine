use bevy_ecs::prelude::Component;

/// Venom-accumulation tracker named after viper, the common noun for
/// a member of the family Viperidae — the largest family of venomous
/// snakes, distributed across every continent except Australia and
/// Antarctica, and including the true vipers of Eurasia and Africa
/// as well as the pit vipers of the Americas and Asia. The word
/// entered English in the fifteenth century through Old French vipere
/// from the Latin vipera, which is itself a contraction of vivipara
/// (live-bearer), because ancient natural history observed, correctly
/// if incompletely, that vipers give birth to live young rather than
/// laying eggs — in fact many species retain fertilised eggs inside
/// the body and the young are born already equipped to survive
/// independently. The common European adder, Vipera berus, is the
/// only venomous snake native to Britain and gave the English language
/// most of its cultural inheritance around vipers: the creature that
/// lurks in the heather, that strikes without apparent warning, that
/// administers a venom causing tissue damage, haemorrhage, and
/// occasionally death. Viperine imagery saturates European rhetoric:
/// the viper at the breast is the metaphor for ingratitude and
/// treachery whose victim nurtured the danger they suffered; to
/// nurse a viper is to show misplaced charity. Viper venom is
/// predominantly haemotoxic, meaning that it attacks blood and
/// tissue rather than the nervous system: the damaged vessels allow
/// blood to leak into surrounding tissue, producing the characteristic
/// swelling and discolouration that make viper bites dramatically
/// visible even when not immediately lethal. In game mechanics,
/// venom is the cleanest model for a damage-over-time system that
/// accumulates with each application and depletes through either
/// time or counter-measures, with the severity of damage at any
/// moment proportional to the current venom level rather than a
/// fixed value. `venom` builds via `envenomate(amount)` and
/// accumulates passively at `toxin_rate` per second in `tick(dt)`
/// or is reduced via `drain(amount)`.
///
/// Models venom-fill levels, poison-saturation bars, toxin-
/// accumulation trackers, haemotoxin gauges, viper-bite fill
/// levels, envenomation-saturation indicators, venom-stack
/// accumulation bars, serpentine-curse meters, tissue-damage fill
/// levels, or any mechanic where successive applications of a
/// venom, toxin, or corrupting substance build toward a threshold
/// beyond which the target is fully envenomed — helpless, visibly
/// suffering, at the mercy of whoever controls the antidote and
/// whoever chose to bite.
///
/// `envenomate(amount)` adds venom; fires `just_envenomed` when
/// first reaching `max_venom`. No-op when disabled.
///
/// `drain(amount)` reduces venom immediately; fires `just_drained`
/// when reaching 0. No-op when disabled or already drained.
///
/// `tick(dt)` clears both flags, then increases venom by
/// `toxin_rate * dt` (capped at `max_venom`). Fires `just_envenomed`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_envenomed()` returns `venom >= max_venom && enabled`.
///
/// `is_drained()` returns `venom == 0.0` (not gated by `enabled`).
///
/// `venom_fraction()` returns `(venom / max_venom).clamp(0, 1)`.
///
/// `effective_toxin(scale)` returns `scale * venom_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — accumulates toxin at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Viper {
    pub venom: f32,
    pub max_venom: f32,
    pub toxin_rate: f32,
    pub just_envenomed: bool,
    pub just_drained: bool,
    pub enabled: bool,
}

impl Viper {
    pub fn new(max_venom: f32, toxin_rate: f32) -> Self {
        Self {
            venom: 0.0,
            max_venom: max_venom.max(0.1),
            toxin_rate: toxin_rate.max(0.0),
            just_envenomed: false,
            just_drained: false,
            enabled: true,
        }
    }

    /// Add venom; fires `just_envenomed` when first reaching max.
    /// No-op when disabled.
    pub fn envenomate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.venom < self.max_venom;
        self.venom = (self.venom + amount).min(self.max_venom);
        if was_below && self.venom >= self.max_venom {
            self.just_envenomed = true;
        }
    }

    /// Reduce venom; fires `just_drained` when reaching 0.
    /// No-op when disabled or already drained.
    pub fn drain(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.venom <= 0.0 {
            return;
        }
        self.venom = (self.venom - amount).max(0.0);
        if self.venom <= 0.0 {
            self.just_drained = true;
        }
    }

    /// Clear flags, then increase venom by `toxin_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_envenomed = false;
        self.just_drained = false;
        if self.enabled && self.toxin_rate > 0.0 && self.venom < self.max_venom {
            let was_below = self.venom < self.max_venom;
            self.venom = (self.venom + self.toxin_rate * dt).min(self.max_venom);
            if was_below && self.venom >= self.max_venom {
                self.just_envenomed = true;
            }
        }
    }

    /// `true` when venom is at maximum and component is enabled.
    pub fn is_envenomed(&self) -> bool {
        self.venom >= self.max_venom && self.enabled
    }

    /// `true` when venom is 0 (not gated by `enabled`).
    pub fn is_drained(&self) -> bool {
        self.venom == 0.0
    }

    /// Fraction of maximum venom [0.0, 1.0].
    pub fn venom_fraction(&self) -> f32 {
        (self.venom / self.max_venom).clamp(0.0, 1.0)
    }

    /// Returns `scale * venom_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_toxin(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.venom_fraction()
    }
}

impl Default for Viper {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v() -> Viper {
        Viper::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_drained() {
        let v = v();
        assert_eq!(v.venom, 0.0);
        assert!(v.is_drained());
        assert!(!v.is_envenomed());
    }

    #[test]
    fn new_clamps_max_venom() {
        let v = Viper::new(-5.0, 1.5);
        assert!((v.max_venom - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_toxin_rate() {
        let v = Viper::new(100.0, -1.5);
        assert_eq!(v.toxin_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let v = Viper::default();
        assert!((v.max_venom - 100.0).abs() < 1e-5);
        assert!((v.toxin_rate - 1.5).abs() < 1e-5);
    }

    // --- envenomate ---

    #[test]
    fn envenomate_adds_venom() {
        let mut v = v();
        v.envenomate(40.0);
        assert!((v.venom - 40.0).abs() < 1e-3);
    }

    #[test]
    fn envenomate_clamps_at_max() {
        let mut v = v();
        v.envenomate(200.0);
        assert!((v.venom - 100.0).abs() < 1e-3);
    }

    #[test]
    fn envenomate_fires_just_envenomed_at_max() {
        let mut v = v();
        v.envenomate(100.0);
        assert!(v.just_envenomed);
        assert!(v.is_envenomed());
    }

    #[test]
    fn envenomate_no_just_envenomed_when_already_at_max() {
        let mut v = v();
        v.venom = 100.0;
        v.envenomate(10.0);
        assert!(!v.just_envenomed);
    }

    #[test]
    fn envenomate_no_op_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.envenomate(50.0);
        assert_eq!(v.venom, 0.0);
    }

    #[test]
    fn envenomate_no_op_when_amount_zero() {
        let mut v = v();
        v.envenomate(0.0);
        assert_eq!(v.venom, 0.0);
    }

    // --- drain ---

    #[test]
    fn drain_reduces_venom() {
        let mut v = v();
        v.venom = 60.0;
        v.drain(20.0);
        assert!((v.venom - 40.0).abs() < 1e-3);
    }

    #[test]
    fn drain_clamps_at_zero() {
        let mut v = v();
        v.venom = 30.0;
        v.drain(200.0);
        assert_eq!(v.venom, 0.0);
    }

    #[test]
    fn drain_fires_just_drained_at_zero() {
        let mut v = v();
        v.venom = 30.0;
        v.drain(30.0);
        assert!(v.just_drained);
    }

    #[test]
    fn drain_no_op_when_already_drained() {
        let mut v = v();
        v.drain(10.0);
        assert!(!v.just_drained);
    }

    #[test]
    fn drain_no_op_when_disabled() {
        let mut v = v();
        v.venom = 50.0;
        v.enabled = false;
        v.drain(50.0);
        assert!((v.venom - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_venom() {
        let mut v = v(); // rate=1.5
        v.tick(4.0); // 0 + 1.5*4 = 6
        assert!((v.venom - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_envenomed_on_venom_to_max() {
        let mut v = Viper::new(100.0, 200.0);
        v.venom = 95.0;
        v.tick(1.0);
        assert!(v.just_envenomed);
        assert!(v.is_envenomed());
    }

    #[test]
    fn tick_no_build_when_already_envenomed() {
        let mut v = v();
        v.venom = 100.0;
        v.tick(1.0);
        assert!(!v.just_envenomed);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut v = Viper::new(100.0, 0.0);
        v.tick(100.0);
        assert_eq!(v.venom, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.tick(1.0);
        assert_eq!(v.venom, 0.0);
    }

    #[test]
    fn tick_clears_just_envenomed() {
        let mut v = Viper::new(100.0, 200.0);
        v.venom = 95.0;
        v.tick(1.0);
        v.tick(0.016);
        assert!(!v.just_envenomed);
    }

    #[test]
    fn tick_clears_just_drained() {
        let mut v = v();
        v.venom = 10.0;
        v.drain(10.0);
        v.tick(0.016);
        assert!(!v.just_drained);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut v = v(); // rate=1.5
        v.tick(6.0); // 1.5*6 = 9
        assert!((v.venom - 9.0).abs() < 1e-3);
    }

    // --- is_envenomed / is_drained ---

    #[test]
    fn is_envenomed_false_when_disabled() {
        let mut v = v();
        v.venom = 100.0;
        v.enabled = false;
        assert!(!v.is_envenomed());
    }

    #[test]
    fn is_drained_not_gated_by_enabled() {
        let mut v = v();
        v.enabled = false;
        assert!(v.is_drained());
    }

    // --- venom_fraction / effective_toxin ---

    #[test]
    fn venom_fraction_zero_when_drained() {
        assert_eq!(v().venom_fraction(), 0.0);
    }

    #[test]
    fn venom_fraction_half_at_midpoint() {
        let mut v = v();
        v.venom = 50.0;
        assert!((v.venom_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_toxin_zero_when_drained() {
        assert_eq!(v().effective_toxin(100.0), 0.0);
    }

    #[test]
    fn effective_toxin_scales_with_venom() {
        let mut v = v();
        v.venom = 75.0;
        assert!((v.effective_toxin(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_toxin_zero_when_disabled() {
        let mut v = v();
        v.venom = 50.0;
        v.enabled = false;
        assert_eq!(v.effective_toxin(100.0), 0.0);
    }
}
