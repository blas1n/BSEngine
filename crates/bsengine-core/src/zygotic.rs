use bevy_ecs::prelude::Component;

/// Fertilized-cell developmental-potential tracker named after zygotic,
/// the adjective describing whatever belongs to, originates in, or
/// characterises a zygote — the single diploid cell produced the moment
/// a spermatozoon fuses with an oocyte and triggers the meiotic arrest
/// release, cortical reaction, and reprogramming cascade that converts
/// two haploid gametes into one totipotent organism. Zygotic gene
/// activation (ZGA) is the transcriptional switch-on that happens in the
/// first one to three cleavage cycles: before ZGA, all developmental
/// instructions come from maternal mRNA stockpiled in the egg; after ZGA,
/// the new diploid genome takes over and drives all subsequent patterning
/// decisions. Everything a future embryo will become — every cell type,
/// tissue, and axis — is encoded in the zygotic genome waiting for its
/// activation threshold to be crossed. `potential` builds via
/// `activate(amount)` and accumulates passively at `develop_rate` per
/// second in `tick(dt)` or is reduced via `revert(amount)`.
///
/// Models totipotent-cell developmental-readiness fill levels, zygotic-
/// gene-activation saturation bars, embryonic-potential accumulation
/// trackers, genome-reprogramming progress meters, fertilisation-cascade
/// completion fill levels, epigenetic-reset saturation indicators,
/// single-cell developmental-fate fill bars, cell-fate commitment
/// approach trackers, maternal-to-zygotic-transition saturation gauges,
/// or any mechanic where a single cell slowly charges up its genetic
/// programme until crossing the transcriptional threshold that commits
/// it irrevocably to becoming something — at which point every
/// subsequent division is determined by the blueprint laid down in
/// that very first moment of fusion.
///
/// `activate(amount)` adds potential; fires `just_totipotent` when
/// first reaching `max_potential`. No-op when disabled.
///
/// `revert(amount)` reduces potential immediately; fires `just_arrested`
/// when reaching 0. No-op when disabled or already arrested.
///
/// `tick(dt)` clears both flags, then increases potential by
/// `develop_rate * dt` (capped at `max_potential`). Fires
/// `just_totipotent` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_totipotent()` returns `potential >= max_potential && enabled`.
///
/// `is_arrested()` returns `potential == 0.0` (not gated by `enabled`).
///
/// `potential_fraction()` returns `(potential / max_potential).clamp(0, 1)`.
///
/// `effective_genome(scale)` returns `scale * potential_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — develops at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zygotic {
    pub potential: f32,
    pub max_potential: f32,
    pub develop_rate: f32,
    pub just_totipotent: bool,
    pub just_arrested: bool,
    pub enabled: bool,
}

impl Zygotic {
    pub fn new(max_potential: f32, develop_rate: f32) -> Self {
        Self {
            potential: 0.0,
            max_potential: max_potential.max(0.1),
            develop_rate: develop_rate.max(0.0),
            just_totipotent: false,
            just_arrested: false,
            enabled: true,
        }
    }

    /// Add potential; fires `just_totipotent` when first reaching max.
    /// No-op when disabled.
    pub fn activate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.potential < self.max_potential;
        self.potential = (self.potential + amount).min(self.max_potential);
        if was_below && self.potential >= self.max_potential {
            self.just_totipotent = true;
        }
    }

    /// Reduce potential; fires `just_arrested` when reaching 0.
    /// No-op when disabled or already arrested.
    pub fn revert(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.potential <= 0.0 {
            return;
        }
        self.potential = (self.potential - amount).max(0.0);
        if self.potential <= 0.0 {
            self.just_arrested = true;
        }
    }

    /// Clear flags, then increase potential by `develop_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_totipotent = false;
        self.just_arrested = false;
        if self.enabled && self.develop_rate > 0.0 && self.potential < self.max_potential {
            let was_below = self.potential < self.max_potential;
            self.potential = (self.potential + self.develop_rate * dt).min(self.max_potential);
            if was_below && self.potential >= self.max_potential {
                self.just_totipotent = true;
            }
        }
    }

    /// `true` when potential is at maximum and component is enabled.
    pub fn is_totipotent(&self) -> bool {
        self.potential >= self.max_potential && self.enabled
    }

    /// `true` when potential is 0 (not gated by `enabled`).
    pub fn is_arrested(&self) -> bool {
        self.potential == 0.0
    }

    /// Fraction of maximum potential [0.0, 1.0].
    pub fn potential_fraction(&self) -> f32 {
        (self.potential / self.max_potential).clamp(0.0, 1.0)
    }

    /// Returns `scale * potential_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_genome(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.potential_fraction()
    }
}

impl Default for Zygotic {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zygotic {
        Zygotic::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_arrested() {
        let z = z();
        assert_eq!(z.potential, 0.0);
        assert!(z.is_arrested());
        assert!(!z.is_totipotent());
    }

    #[test]
    fn new_clamps_max_potential() {
        let z = Zygotic::new(-5.0, 1.5);
        assert!((z.max_potential - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_develop_rate() {
        let z = Zygotic::new(100.0, -1.5);
        assert_eq!(z.develop_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zygotic::default();
        assert!((z.max_potential - 100.0).abs() < 1e-5);
        assert!((z.develop_rate - 1.5).abs() < 1e-5);
    }

    // --- activate ---

    #[test]
    fn activate_adds_potential() {
        let mut z = z();
        z.activate(40.0);
        assert!((z.potential - 40.0).abs() < 1e-3);
    }

    #[test]
    fn activate_clamps_at_max() {
        let mut z = z();
        z.activate(200.0);
        assert!((z.potential - 100.0).abs() < 1e-3);
    }

    #[test]
    fn activate_fires_just_totipotent_at_max() {
        let mut z = z();
        z.activate(100.0);
        assert!(z.just_totipotent);
        assert!(z.is_totipotent());
    }

    #[test]
    fn activate_no_just_totipotent_when_already_at_max() {
        let mut z = z();
        z.potential = 100.0;
        z.activate(10.0);
        assert!(!z.just_totipotent);
    }

    #[test]
    fn activate_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.activate(50.0);
        assert_eq!(z.potential, 0.0);
    }

    #[test]
    fn activate_no_op_when_amount_zero() {
        let mut z = z();
        z.activate(0.0);
        assert_eq!(z.potential, 0.0);
    }

    // --- revert ---

    #[test]
    fn revert_reduces_potential() {
        let mut z = z();
        z.potential = 60.0;
        z.revert(20.0);
        assert!((z.potential - 40.0).abs() < 1e-3);
    }

    #[test]
    fn revert_clamps_at_zero() {
        let mut z = z();
        z.potential = 30.0;
        z.revert(200.0);
        assert_eq!(z.potential, 0.0);
    }

    #[test]
    fn revert_fires_just_arrested_at_zero() {
        let mut z = z();
        z.potential = 30.0;
        z.revert(30.0);
        assert!(z.just_arrested);
    }

    #[test]
    fn revert_no_op_when_already_arrested() {
        let mut z = z();
        z.revert(10.0);
        assert!(!z.just_arrested);
    }

    #[test]
    fn revert_no_op_when_disabled() {
        let mut z = z();
        z.potential = 50.0;
        z.enabled = false;
        z.revert(50.0);
        assert!((z.potential - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_develops_potential() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.potential - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_totipotent_on_develop_to_max() {
        let mut z = Zygotic::new(100.0, 200.0);
        z.potential = 95.0;
        z.tick(1.0);
        assert!(z.just_totipotent);
        assert!(z.is_totipotent());
    }

    #[test]
    fn tick_no_develop_when_already_totipotent() {
        let mut z = z();
        z.potential = 100.0;
        z.tick(1.0);
        assert!(!z.just_totipotent);
    }

    #[test]
    fn tick_no_develop_when_rate_zero() {
        let mut z = Zygotic::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.potential, 0.0);
    }

    #[test]
    fn tick_no_develop_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.potential, 0.0);
    }

    #[test]
    fn tick_clears_just_totipotent() {
        let mut z = Zygotic::new(100.0, 200.0);
        z.potential = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_totipotent);
    }

    #[test]
    fn tick_clears_just_arrested() {
        let mut z = z();
        z.potential = 10.0;
        z.revert(10.0);
        z.tick(0.016);
        assert!(!z.just_arrested);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.potential - 9.0).abs() < 1e-3);
    }

    // --- is_totipotent / is_arrested ---

    #[test]
    fn is_totipotent_false_when_disabled() {
        let mut z = z();
        z.potential = 100.0;
        z.enabled = false;
        assert!(!z.is_totipotent());
    }

    #[test]
    fn is_arrested_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_arrested());
    }

    // --- potential_fraction / effective_genome ---

    #[test]
    fn potential_fraction_zero_when_arrested() {
        assert_eq!(z().potential_fraction(), 0.0);
    }

    #[test]
    fn potential_fraction_half_at_midpoint() {
        let mut z = z();
        z.potential = 50.0;
        assert!((z.potential_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_genome_zero_when_arrested() {
        assert_eq!(z().effective_genome(100.0), 0.0);
    }

    #[test]
    fn effective_genome_scales_with_potential() {
        let mut z = z();
        z.potential = 75.0;
        assert!((z.effective_genome(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_genome_zero_when_disabled() {
        let mut z = z();
        z.potential = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_genome(100.0), 0.0);
    }
}
