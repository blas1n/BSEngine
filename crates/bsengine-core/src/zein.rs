use bevy_ecs::prelude::Component;

/// Prolamin-protein accumulation tracker named after zein, the major
/// storage protein of the maize endosperm and the most extensively
/// studied member of the prolamin superfamily — a group of plant seed
/// storage proteins characterised by their solubility in alcohol-water
/// mixtures and their high content of the amino acids proline and
/// glutamine. In the maize kernel, zein accounts for roughly half the
/// total protein mass, deposited in discrete subcellular compartments
/// called protein bodies within the endosperm cells; the four zein
/// fractions — alpha, beta, gamma, and delta — differ in molecular
/// weight, solubility, and localisation within the protein body,
/// with alpha-zein forming the electron-lucent interior and the other
/// fractions forming a shell around it. This hierarchical self-
/// assembly makes zein a natural analogue of synthetic polymer
/// nanoparticles: zein can be dissolved, reprecipitated, and formed
/// into films, fibres, capsules, and microbeads with remarkable ease,
/// which has made it commercially valuable as a coating for
/// pharmaceutical tablets (it is tasteless, odourless, moisture-
/// resistant, and edible), a carrier for nutraceuticals and flavour
/// compounds, a biodegradable plastic precursor, and an ink
/// binder in historical printing. The protein's nutritional
/// limitation — it is deficient in the essential amino acids
/// lysine and tryptophan — drove twentieth-century breeding programs
/// that produced opaque-2 maize, a lysine-rich mutant that disrupts
/// normal zein accumulation. `protein` builds via `deposit(amount)` and
/// accumulates passively at `store_rate` per second in `tick(dt)` or
/// is consumed via `hydrolyse(amount)`.
///
/// Models zein-protein fill levels, prolamin-storage saturation bars,
/// seed-protein accumulation trackers, endosperm-reserve gauges, zein-
/// coating fill levels, biopolymer-precursor saturation indicators,
/// protein-body formation accumulation bars, edible-film readiness
/// meters, maize-endosperm fill levels, or any mechanic where a plant,
/// character, or system slowly deposits a structural protein reserve
/// — monomer by monomer, protein body by protein body — until the
/// storage tissue is completely loaded and every cell glistens with
/// translucent proteinaceous inclusion bodies.
///
/// `deposit(amount)` adds protein; fires `just_loaded` when first
/// reaching `max_protein`. No-op when disabled.
///
/// `hydrolyse(amount)` reduces protein immediately; fires `just_depleted`
/// when reaching 0. No-op when disabled or already depleted.
///
/// `tick(dt)` clears both flags, then increases protein by
/// `store_rate * dt` (capped at `max_protein`). Fires `just_loaded`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_loaded()` returns `protein >= max_protein && enabled`.
///
/// `is_depleted()` returns `protein == 0.0` (not gated by `enabled`).
///
/// `protein_fraction()` returns
/// `(protein / max_protein).clamp(0, 1)`.
///
/// `effective_coating(scale)` returns `scale * protein_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — stores at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zein {
    pub protein: f32,
    pub max_protein: f32,
    pub store_rate: f32,
    pub just_loaded: bool,
    pub just_depleted: bool,
    pub enabled: bool,
}

impl Zein {
    pub fn new(max_protein: f32, store_rate: f32) -> Self {
        Self {
            protein: 0.0,
            max_protein: max_protein.max(0.1),
            store_rate: store_rate.max(0.0),
            just_loaded: false,
            just_depleted: false,
            enabled: true,
        }
    }

    /// Add protein; fires `just_loaded` when first reaching max.
    /// No-op when disabled.
    pub fn deposit(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.protein < self.max_protein;
        self.protein = (self.protein + amount).min(self.max_protein);
        if was_below && self.protein >= self.max_protein {
            self.just_loaded = true;
        }
    }

    /// Reduce protein; fires `just_depleted` when reaching 0.
    /// No-op when disabled or already depleted.
    pub fn hydrolyse(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.protein <= 0.0 {
            return;
        }
        self.protein = (self.protein - amount).max(0.0);
        if self.protein <= 0.0 {
            self.just_depleted = true;
        }
    }

    /// Clear flags, then increase protein by `store_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_loaded = false;
        self.just_depleted = false;
        if self.enabled && self.store_rate > 0.0 && self.protein < self.max_protein {
            let was_below = self.protein < self.max_protein;
            self.protein = (self.protein + self.store_rate * dt).min(self.max_protein);
            if was_below && self.protein >= self.max_protein {
                self.just_loaded = true;
            }
        }
    }

    /// `true` when protein is at maximum and component is enabled.
    pub fn is_loaded(&self) -> bool {
        self.protein >= self.max_protein && self.enabled
    }

    /// `true` when protein is 0 (not gated by `enabled`).
    pub fn is_depleted(&self) -> bool {
        self.protein == 0.0
    }

    /// Fraction of maximum protein [0.0, 1.0].
    pub fn protein_fraction(&self) -> f32 {
        (self.protein / self.max_protein).clamp(0.0, 1.0)
    }

    /// Returns `scale * protein_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_coating(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.protein_fraction()
    }
}

impl Default for Zein {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zein {
        Zein::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_depleted() {
        let z = z();
        assert_eq!(z.protein, 0.0);
        assert!(z.is_depleted());
        assert!(!z.is_loaded());
    }

    #[test]
    fn new_clamps_max_protein() {
        let z = Zein::new(-5.0, 1.5);
        assert!((z.max_protein - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_store_rate() {
        let z = Zein::new(100.0, -1.5);
        assert_eq!(z.store_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zein::default();
        assert!((z.max_protein - 100.0).abs() < 1e-5);
        assert!((z.store_rate - 1.5).abs() < 1e-5);
    }

    // --- deposit ---

    #[test]
    fn deposit_adds_protein() {
        let mut z = z();
        z.deposit(40.0);
        assert!((z.protein - 40.0).abs() < 1e-3);
    }

    #[test]
    fn deposit_clamps_at_max() {
        let mut z = z();
        z.deposit(200.0);
        assert!((z.protein - 100.0).abs() < 1e-3);
    }

    #[test]
    fn deposit_fires_just_loaded_at_max() {
        let mut z = z();
        z.deposit(100.0);
        assert!(z.just_loaded);
        assert!(z.is_loaded());
    }

    #[test]
    fn deposit_no_just_loaded_when_already_at_max() {
        let mut z = z();
        z.protein = 100.0;
        z.deposit(10.0);
        assert!(!z.just_loaded);
    }

    #[test]
    fn deposit_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.deposit(50.0);
        assert_eq!(z.protein, 0.0);
    }

    #[test]
    fn deposit_no_op_when_amount_zero() {
        let mut z = z();
        z.deposit(0.0);
        assert_eq!(z.protein, 0.0);
    }

    // --- hydrolyse ---

    #[test]
    fn hydrolyse_reduces_protein() {
        let mut z = z();
        z.protein = 60.0;
        z.hydrolyse(20.0);
        assert!((z.protein - 40.0).abs() < 1e-3);
    }

    #[test]
    fn hydrolyse_clamps_at_zero() {
        let mut z = z();
        z.protein = 30.0;
        z.hydrolyse(200.0);
        assert_eq!(z.protein, 0.0);
    }

    #[test]
    fn hydrolyse_fires_just_depleted_at_zero() {
        let mut z = z();
        z.protein = 30.0;
        z.hydrolyse(30.0);
        assert!(z.just_depleted);
    }

    #[test]
    fn hydrolyse_no_op_when_already_depleted() {
        let mut z = z();
        z.hydrolyse(10.0);
        assert!(!z.just_depleted);
    }

    #[test]
    fn hydrolyse_no_op_when_disabled() {
        let mut z = z();
        z.protein = 50.0;
        z.enabled = false;
        z.hydrolyse(50.0);
        assert!((z.protein - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_stores_protein() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.protein - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_loaded_on_store_to_max() {
        let mut z = Zein::new(100.0, 200.0);
        z.protein = 95.0;
        z.tick(1.0);
        assert!(z.just_loaded);
        assert!(z.is_loaded());
    }

    #[test]
    fn tick_no_store_when_already_loaded() {
        let mut z = z();
        z.protein = 100.0;
        z.tick(1.0);
        assert!(!z.just_loaded);
    }

    #[test]
    fn tick_no_store_when_rate_zero() {
        let mut z = Zein::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.protein, 0.0);
    }

    #[test]
    fn tick_no_store_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.protein, 0.0);
    }

    #[test]
    fn tick_clears_just_loaded() {
        let mut z = Zein::new(100.0, 200.0);
        z.protein = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_loaded);
    }

    #[test]
    fn tick_clears_just_depleted() {
        let mut z = z();
        z.protein = 10.0;
        z.hydrolyse(10.0);
        z.tick(0.016);
        assert!(!z.just_depleted);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.protein - 9.0).abs() < 1e-3);
    }

    // --- is_loaded / is_depleted ---

    #[test]
    fn is_loaded_false_when_disabled() {
        let mut z = z();
        z.protein = 100.0;
        z.enabled = false;
        assert!(!z.is_loaded());
    }

    #[test]
    fn is_depleted_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_depleted());
    }

    // --- protein_fraction / effective_coating ---

    #[test]
    fn protein_fraction_zero_when_depleted() {
        assert_eq!(z().protein_fraction(), 0.0);
    }

    #[test]
    fn protein_fraction_half_at_midpoint() {
        let mut z = z();
        z.protein = 50.0;
        assert!((z.protein_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_coating_zero_when_depleted() {
        assert_eq!(z().effective_coating(100.0), 0.0);
    }

    #[test]
    fn effective_coating_scales_with_protein() {
        let mut z = z();
        z.protein = 75.0;
        assert!((z.effective_coating(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_coating_zero_when_disabled() {
        let mut z = z();
        z.protein = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_coating(100.0), 0.0);
    }
}
