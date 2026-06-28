use bevy_ecs::prelude::Component;

/// Inter-species affinity tracker named after zoophilia (from Greek
/// zoion "animal" + philia "love, fondness"), the deep bond or
/// affective attachment that forms between individuals of different
/// species. In ecology the term appears as "zoophilous" to describe
/// organisms that depend on animals — a zoophilous plant is one that
/// relies on animal visitors for pollination or seed dispersal; a
/// zoophilous parasite is one that preferentially infects non-human
/// hosts. In ethology the broader sense encompasses the suite of
/// attachment behaviours that cross species lines: the domestic dog
/// reading human facial expressions, the elephant returning year after
/// year to the bones of a keeper it outlived, the kea that seeks
/// social interaction with any sufficiently curious creature it
/// encounters. Human zoophily — in the ecologically broad sense —
/// is so ancient that it predates written language: cave painters
/// depicting the inner life of bison, goat-herders tracing bloodlines,
/// falconers training individual raptors to read human signals. The
/// bonding is mutual as well as one-directional; the domestication of
/// dogs likely began as convergent social tolerance, where the least-
/// fearful wolves began camping closer to human middens and both sides
/// benefited incrementally until the relationship became obligate.
/// `affinity` builds via `attune(amount)` and accumulates passively at
/// `bond_rate` per second in `tick(dt)` or dissipates via
/// `alienate(amount)`.
///
/// Models inter-species bond-strength fill levels, creature-affinity
/// saturation bars, animal-companion loyalty accumulators, wildlife-
/// devotion fill levels, domestication-progress gauges, zoophilous-
/// pollinator attraction bars, cross-species trust saturation meters,
/// keeper-bond fill levels, raptor-imprinting progress trackers, or
/// any mechanic where patient cross-species interaction slowly deepens
/// an affective bridge until the creature reads the player's intent
/// without cue, follows without command, and defends without reward —
/// until some betrayal of trust collapses the accumulated goodwill
/// back to the wariness where everything began.
///
/// `attune(amount)` adds affinity; fires `just_bonded` when first
/// reaching `max_affinity`. No-op when disabled.
///
/// `alienate(amount)` reduces affinity immediately; fires
/// `just_estranged` when reaching 0. No-op when disabled or already
/// estranged.
///
/// `tick(dt)` clears both flags, then increases affinity by
/// `bond_rate * dt` (capped at `max_affinity`). Fires `just_bonded`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_bonded()` returns `affinity >= max_affinity && enabled`.
///
/// `is_estranged()` returns `affinity == 0.0` (not gated by `enabled`).
///
/// `affinity_fraction()` returns `(affinity / max_affinity).clamp(0, 1)`.
///
/// `effective_rapport(scale)` returns `scale * affinity_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — bonds at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoophilia {
    pub affinity: f32,
    pub max_affinity: f32,
    pub bond_rate: f32,
    pub just_bonded: bool,
    pub just_estranged: bool,
    pub enabled: bool,
}

impl Zoophilia {
    pub fn new(max_affinity: f32, bond_rate: f32) -> Self {
        Self {
            affinity: 0.0,
            max_affinity: max_affinity.max(0.1),
            bond_rate: bond_rate.max(0.0),
            just_bonded: false,
            just_estranged: false,
            enabled: true,
        }
    }

    /// Add affinity; fires `just_bonded` when first reaching max.
    /// No-op when disabled.
    pub fn attune(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.affinity < self.max_affinity;
        self.affinity = (self.affinity + amount).min(self.max_affinity);
        if was_below && self.affinity >= self.max_affinity {
            self.just_bonded = true;
        }
    }

    /// Reduce affinity; fires `just_estranged` when reaching 0.
    /// No-op when disabled or already estranged.
    pub fn alienate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.affinity <= 0.0 {
            return;
        }
        self.affinity = (self.affinity - amount).max(0.0);
        if self.affinity <= 0.0 {
            self.just_estranged = true;
        }
    }

    /// Clear flags, then increase affinity by `bond_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_bonded = false;
        self.just_estranged = false;
        if self.enabled && self.bond_rate > 0.0 && self.affinity < self.max_affinity {
            let was_below = self.affinity < self.max_affinity;
            self.affinity = (self.affinity + self.bond_rate * dt).min(self.max_affinity);
            if was_below && self.affinity >= self.max_affinity {
                self.just_bonded = true;
            }
        }
    }

    /// `true` when affinity is at maximum and component is enabled.
    pub fn is_bonded(&self) -> bool {
        self.affinity >= self.max_affinity && self.enabled
    }

    /// `true` when affinity is 0 (not gated by `enabled`).
    pub fn is_estranged(&self) -> bool {
        self.affinity == 0.0
    }

    /// Fraction of maximum affinity [0.0, 1.0].
    pub fn affinity_fraction(&self) -> f32 {
        (self.affinity / self.max_affinity).clamp(0.0, 1.0)
    }

    /// Returns `scale * affinity_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_rapport(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.affinity_fraction()
    }
}

impl Default for Zoophilia {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoophilia {
        Zoophilia::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_estranged() {
        let z = z();
        assert_eq!(z.affinity, 0.0);
        assert!(z.is_estranged());
        assert!(!z.is_bonded());
    }

    #[test]
    fn new_clamps_max_affinity() {
        let z = Zoophilia::new(-5.0, 1.5);
        assert!((z.max_affinity - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_bond_rate() {
        let z = Zoophilia::new(100.0, -1.5);
        assert_eq!(z.bond_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoophilia::default();
        assert!((z.max_affinity - 100.0).abs() < 1e-5);
        assert!((z.bond_rate - 1.5).abs() < 1e-5);
    }

    // --- attune ---

    #[test]
    fn attune_adds_affinity() {
        let mut z = z();
        z.attune(40.0);
        assert!((z.affinity - 40.0).abs() < 1e-3);
    }

    #[test]
    fn attune_clamps_at_max() {
        let mut z = z();
        z.attune(200.0);
        assert!((z.affinity - 100.0).abs() < 1e-3);
    }

    #[test]
    fn attune_fires_just_bonded_at_max() {
        let mut z = z();
        z.attune(100.0);
        assert!(z.just_bonded);
        assert!(z.is_bonded());
    }

    #[test]
    fn attune_no_just_bonded_when_already_at_max() {
        let mut z = z();
        z.affinity = 100.0;
        z.attune(10.0);
        assert!(!z.just_bonded);
    }

    #[test]
    fn attune_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.attune(50.0);
        assert_eq!(z.affinity, 0.0);
    }

    #[test]
    fn attune_no_op_when_amount_zero() {
        let mut z = z();
        z.attune(0.0);
        assert_eq!(z.affinity, 0.0);
    }

    // --- alienate ---

    #[test]
    fn alienate_reduces_affinity() {
        let mut z = z();
        z.affinity = 60.0;
        z.alienate(20.0);
        assert!((z.affinity - 40.0).abs() < 1e-3);
    }

    #[test]
    fn alienate_clamps_at_zero() {
        let mut z = z();
        z.affinity = 30.0;
        z.alienate(200.0);
        assert_eq!(z.affinity, 0.0);
    }

    #[test]
    fn alienate_fires_just_estranged_at_zero() {
        let mut z = z();
        z.affinity = 30.0;
        z.alienate(30.0);
        assert!(z.just_estranged);
    }

    #[test]
    fn alienate_no_op_when_already_estranged() {
        let mut z = z();
        z.alienate(10.0);
        assert!(!z.just_estranged);
    }

    #[test]
    fn alienate_no_op_when_disabled() {
        let mut z = z();
        z.affinity = 50.0;
        z.enabled = false;
        z.alienate(50.0);
        assert!((z.affinity - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_bonds_affinity() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.affinity - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_bonded_on_bond_to_max() {
        let mut z = Zoophilia::new(100.0, 200.0);
        z.affinity = 95.0;
        z.tick(1.0);
        assert!(z.just_bonded);
        assert!(z.is_bonded());
    }

    #[test]
    fn tick_no_bond_when_already_bonded() {
        let mut z = z();
        z.affinity = 100.0;
        z.tick(1.0);
        assert!(!z.just_bonded);
    }

    #[test]
    fn tick_no_bond_when_rate_zero() {
        let mut z = Zoophilia::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.affinity, 0.0);
    }

    #[test]
    fn tick_no_bond_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.affinity, 0.0);
    }

    #[test]
    fn tick_clears_just_bonded() {
        let mut z = Zoophilia::new(100.0, 200.0);
        z.affinity = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_bonded);
    }

    #[test]
    fn tick_clears_just_estranged() {
        let mut z = z();
        z.affinity = 10.0;
        z.alienate(10.0);
        z.tick(0.016);
        assert!(!z.just_estranged);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.affinity - 9.0).abs() < 1e-3);
    }

    // --- is_bonded / is_estranged ---

    #[test]
    fn is_bonded_false_when_disabled() {
        let mut z = z();
        z.affinity = 100.0;
        z.enabled = false;
        assert!(!z.is_bonded());
    }

    #[test]
    fn is_estranged_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_estranged());
    }

    // --- affinity_fraction / effective_rapport ---

    #[test]
    fn affinity_fraction_zero_when_estranged() {
        assert_eq!(z().affinity_fraction(), 0.0);
    }

    #[test]
    fn affinity_fraction_half_at_midpoint() {
        let mut z = z();
        z.affinity = 50.0;
        assert!((z.affinity_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_rapport_zero_when_estranged() {
        assert_eq!(z().effective_rapport(100.0), 0.0);
    }

    #[test]
    fn effective_rapport_scales_with_affinity() {
        let mut z = z();
        z.affinity = 75.0;
        assert!((z.effective_rapport(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_rapport_zero_when_disabled() {
        let mut z = z();
        z.affinity = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_rapport(100.0), 0.0);
    }
}
