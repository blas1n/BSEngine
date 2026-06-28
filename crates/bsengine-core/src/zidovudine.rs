use bevy_ecs::prelude::Component;

/// Antiviral-dosage accumulation tracker named after zidovudine, the
/// nucleoside-analogue antiretroviral drug — generic name for the
/// compound first synthesised in 1964 by Jerome Horwitz as a potential
/// anti-cancer agent, then shelved for two decades before researchers
/// at Burroughs Wellcome discovered in 1985 that it suppressed HIV
/// replication in cell culture. Approved by the FDA in 1987 under the
/// brand name Retrovir, zidovudine was the first antiretroviral drug
/// licensed for clinical use and the first treatment to alter the
/// course of AIDS — an achievement made under extraordinary regulatory
/// pressure as the epidemic was claiming tens of thousands of lives
/// per year. The drug works by mimicking thymidine: it is taken up by
/// HIV's reverse transcriptase enzyme and incorporated into the growing
/// viral DNA strand, but because it lacks the 3'-hydroxyl group that
/// allows the next nucleotide to be added, it acts as a chain
/// terminator, halting viral genome replication mid-strand. The virus
/// cannot complete its life cycle; replication slows; the patient's
/// CD4+ T-cell count stabilises or rises. Zidovudine monotherapy is
/// now rarely used because the virus mutates rapidly and resistance
/// emerges within weeks, but the drug remains a cornerstone of
/// combination antiretroviral therapy (cART) — combined with other
/// reverse-transcriptase inhibitors and protease inhibitors, it
/// continues to suppress viral load to undetectable levels in millions
/// of patients. `dose` builds via `administer(amount)` and accumulates
/// passively at `infusion_rate` per second in `tick(dt)` or is cleared
/// via `metabolise(amount)`.
///
/// Models antiviral-dosage fill levels, drug-concentration saturation
/// bars, retrovirion-suppression accumulators, chain-terminator-dose
/// gauges, antiretroviral-coverage fill levels, viral-load-reduction
/// saturation indicators, nucleoside-analogue accumulation bars,
/// CD4-stabilisation dose meters, prophylaxis-saturation fill levels,
/// or any mechanic where a patient, character, or biological system
/// slowly accumulates a therapeutic compound until a protective
/// threshold is reached — after which the viral replication machinery
/// grinds to a halt and every new virion produced is a dead-end copy
/// with a broken strand where its genome should be.
///
/// `administer(amount)` adds dose; fires `just_suppressed` when first
/// reaching `max_dose`. No-op when disabled.
///
/// `metabolise(amount)` reduces dose immediately; fires `just_cleared`
/// when reaching 0. No-op when disabled or already cleared.
///
/// `tick(dt)` clears both flags, then increases dose by
/// `infusion_rate * dt` (capped at `max_dose`). Fires `just_suppressed`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_suppressed()` returns `dose >= max_dose && enabled`.
///
/// `is_cleared()` returns `dose == 0.0` (not gated by `enabled`).
///
/// `dose_fraction()` returns
/// `(dose / max_dose).clamp(0, 1)`.
///
/// `effective_inhibition(scale)` returns `scale * dose_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — infuses at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zidovudine {
    pub dose: f32,
    pub max_dose: f32,
    pub infusion_rate: f32,
    pub just_suppressed: bool,
    pub just_cleared: bool,
    pub enabled: bool,
}

impl Zidovudine {
    pub fn new(max_dose: f32, infusion_rate: f32) -> Self {
        Self {
            dose: 0.0,
            max_dose: max_dose.max(0.1),
            infusion_rate: infusion_rate.max(0.0),
            just_suppressed: false,
            just_cleared: false,
            enabled: true,
        }
    }

    /// Add dose; fires `just_suppressed` when first reaching max.
    /// No-op when disabled.
    pub fn administer(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.dose < self.max_dose;
        self.dose = (self.dose + amount).min(self.max_dose);
        if was_below && self.dose >= self.max_dose {
            self.just_suppressed = true;
        }
    }

    /// Reduce dose; fires `just_cleared` when reaching 0.
    /// No-op when disabled or already cleared.
    pub fn metabolise(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.dose <= 0.0 {
            return;
        }
        self.dose = (self.dose - amount).max(0.0);
        if self.dose <= 0.0 {
            self.just_cleared = true;
        }
    }

    /// Clear flags, then increase dose by `infusion_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_suppressed = false;
        self.just_cleared = false;
        if self.enabled && self.infusion_rate > 0.0 && self.dose < self.max_dose {
            let was_below = self.dose < self.max_dose;
            self.dose = (self.dose + self.infusion_rate * dt).min(self.max_dose);
            if was_below && self.dose >= self.max_dose {
                self.just_suppressed = true;
            }
        }
    }

    /// `true` when dose is at maximum and component is enabled.
    pub fn is_suppressed(&self) -> bool {
        self.dose >= self.max_dose && self.enabled
    }

    /// `true` when dose is 0 (not gated by `enabled`).
    pub fn is_cleared(&self) -> bool {
        self.dose == 0.0
    }

    /// Fraction of maximum dose [0.0, 1.0].
    pub fn dose_fraction(&self) -> f32 {
        (self.dose / self.max_dose).clamp(0.0, 1.0)
    }

    /// Returns `scale * dose_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_inhibition(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.dose_fraction()
    }
}

impl Default for Zidovudine {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zidovudine {
        Zidovudine::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_cleared() {
        let z = z();
        assert_eq!(z.dose, 0.0);
        assert!(z.is_cleared());
        assert!(!z.is_suppressed());
    }

    #[test]
    fn new_clamps_max_dose() {
        let z = Zidovudine::new(-5.0, 1.5);
        assert!((z.max_dose - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_infusion_rate() {
        let z = Zidovudine::new(100.0, -1.5);
        assert_eq!(z.infusion_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zidovudine::default();
        assert!((z.max_dose - 100.0).abs() < 1e-5);
        assert!((z.infusion_rate - 1.5).abs() < 1e-5);
    }

    // --- administer ---

    #[test]
    fn administer_adds_dose() {
        let mut z = z();
        z.administer(40.0);
        assert!((z.dose - 40.0).abs() < 1e-3);
    }

    #[test]
    fn administer_clamps_at_max() {
        let mut z = z();
        z.administer(200.0);
        assert!((z.dose - 100.0).abs() < 1e-3);
    }

    #[test]
    fn administer_fires_just_suppressed_at_max() {
        let mut z = z();
        z.administer(100.0);
        assert!(z.just_suppressed);
        assert!(z.is_suppressed());
    }

    #[test]
    fn administer_no_just_suppressed_when_already_at_max() {
        let mut z = z();
        z.dose = 100.0;
        z.administer(10.0);
        assert!(!z.just_suppressed);
    }

    #[test]
    fn administer_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.administer(50.0);
        assert_eq!(z.dose, 0.0);
    }

    #[test]
    fn administer_no_op_when_amount_zero() {
        let mut z = z();
        z.administer(0.0);
        assert_eq!(z.dose, 0.0);
    }

    // --- metabolise ---

    #[test]
    fn metabolise_reduces_dose() {
        let mut z = z();
        z.dose = 60.0;
        z.metabolise(20.0);
        assert!((z.dose - 40.0).abs() < 1e-3);
    }

    #[test]
    fn metabolise_clamps_at_zero() {
        let mut z = z();
        z.dose = 30.0;
        z.metabolise(200.0);
        assert_eq!(z.dose, 0.0);
    }

    #[test]
    fn metabolise_fires_just_cleared_at_zero() {
        let mut z = z();
        z.dose = 30.0;
        z.metabolise(30.0);
        assert!(z.just_cleared);
    }

    #[test]
    fn metabolise_no_op_when_already_cleared() {
        let mut z = z();
        z.metabolise(10.0);
        assert!(!z.just_cleared);
    }

    #[test]
    fn metabolise_no_op_when_disabled() {
        let mut z = z();
        z.dose = 50.0;
        z.enabled = false;
        z.metabolise(50.0);
        assert!((z.dose - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_infuses_dose() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.dose - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_suppressed_on_infuse_to_max() {
        let mut z = Zidovudine::new(100.0, 200.0);
        z.dose = 95.0;
        z.tick(1.0);
        assert!(z.just_suppressed);
        assert!(z.is_suppressed());
    }

    #[test]
    fn tick_no_infuse_when_already_suppressed() {
        let mut z = z();
        z.dose = 100.0;
        z.tick(1.0);
        assert!(!z.just_suppressed);
    }

    #[test]
    fn tick_no_infuse_when_rate_zero() {
        let mut z = Zidovudine::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.dose, 0.0);
    }

    #[test]
    fn tick_no_infuse_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.dose, 0.0);
    }

    #[test]
    fn tick_clears_just_suppressed() {
        let mut z = Zidovudine::new(100.0, 200.0);
        z.dose = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_suppressed);
    }

    #[test]
    fn tick_clears_just_cleared() {
        let mut z = z();
        z.dose = 10.0;
        z.metabolise(10.0);
        z.tick(0.016);
        assert!(!z.just_cleared);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.dose - 9.0).abs() < 1e-3);
    }

    // --- is_suppressed / is_cleared ---

    #[test]
    fn is_suppressed_false_when_disabled() {
        let mut z = z();
        z.dose = 100.0;
        z.enabled = false;
        assert!(!z.is_suppressed());
    }

    #[test]
    fn is_cleared_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_cleared());
    }

    // --- dose_fraction / effective_inhibition ---

    #[test]
    fn dose_fraction_zero_when_cleared() {
        assert_eq!(z().dose_fraction(), 0.0);
    }

    #[test]
    fn dose_fraction_half_at_midpoint() {
        let mut z = z();
        z.dose = 50.0;
        assert!((z.dose_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_inhibition_zero_when_cleared() {
        assert_eq!(z().effective_inhibition(100.0), 0.0);
    }

    #[test]
    fn effective_inhibition_scales_with_dose() {
        let mut z = z();
        z.dose = 75.0;
        assert!((z.effective_inhibition(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_inhibition_zero_when_disabled() {
        let mut z = z();
        z.dose = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_inhibition(100.0), 0.0);
    }
}
