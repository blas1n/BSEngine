use bevy_ecs::prelude::Component;

/// Animal-affection accumulation tracker named after the zoophile,
/// a person who loves and is devoted to animals — in its original and
/// most common sense an ardent admirer of the animal kingdom, someone
/// whose concern for non-human creatures shapes their vocation,
/// leisure, and moral commitments. The zoophile in this classical sense
/// is the naturalist who spends childhood turning over stones to observe
/// beetles, the veterinarian who absorbs every detail of comparative
/// anatomy, the wildlife rehabilitator who wakes at midnight to tube-
/// feed an orphaned hedgehog, the ethologist whose years of patient
/// field observation have earned the trust of a wolf pack. This
/// affective bond with the animal world is distinct from the merely
/// scientific curiosity of the zoologist: it involves emotional
/// investment in the wellbeing of specific creatures as well as species,
/// a disposition to anthropomorphise carefully while still respecting
/// the otherness of non-human minds, and a willingness to subordinate
/// human convenience to animal welfare. The word entered English in
/// the late nineteenth century alongside zoophilia — the more abstract
/// noun denoting the condition — as animal welfare movements were
/// formalising and acquiring philosophical frameworks derived from
/// Benthamite utilitarianism and Darwinian continuism. In game design,
/// a zoophile character arc might track growing identification with the
/// animal world: beginning as an outsider observing wildlife and
/// accumulating empathy, rapport, and interspecies communication skill
/// until the character has crossed some threshold of allegiance where
/// the welfare of animals matters more than the welfare of the faction
/// they originally served. `affection` builds via `bond(amount)` and
/// accumulates passively at `empathy_rate` per second in `tick(dt)` or
/// is eroded via `estrange(amount)`.
///
/// Models animal-affection fill levels, interspecies-rapport saturation
/// bars, wildlife-empathy accumulators, companion-bond gauges, animal-
/// welfare-commitment fill levels, zoophilic-devotion saturation
/// indicators, fauna-kinship accumulation bars, creature-companion
/// attachment meters, beastmaster-affinity fill levels, or any mechanic
/// where a character or faction slowly deepens its emotional investment
/// in the animal world — learning to read body language, earning trust,
/// and finally crossing the threshold at which an animal partner would
/// give its life for the bond they share.
///
/// `bond(amount)` adds affection; fires `just_bonded` when first
/// reaching `max_affection`. No-op when disabled.
///
/// `estrange(amount)` reduces affection immediately; fires `just_estranged`
/// when reaching 0. No-op when disabled or already estranged.
///
/// `tick(dt)` clears both flags, then increases affection by
/// `empathy_rate * dt` (capped at `max_affection`). Fires `just_bonded`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_bonded()` returns `affection >= max_affection && enabled`.
///
/// `is_estranged()` returns `affection == 0.0` (not gated by `enabled`).
///
/// `affection_fraction()` returns
/// `(affection / max_affection).clamp(0, 1)`.
///
/// `effective_kinship(scale)` returns `scale * affection_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — bonds at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoophile {
    pub affection: f32,
    pub max_affection: f32,
    pub empathy_rate: f32,
    pub just_bonded: bool,
    pub just_estranged: bool,
    pub enabled: bool,
}

impl Zoophile {
    pub fn new(max_affection: f32, empathy_rate: f32) -> Self {
        Self {
            affection: 0.0,
            max_affection: max_affection.max(0.1),
            empathy_rate: empathy_rate.max(0.0),
            just_bonded: false,
            just_estranged: false,
            enabled: true,
        }
    }

    /// Add affection; fires `just_bonded` when first reaching max.
    /// No-op when disabled.
    pub fn bond(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.affection < self.max_affection;
        self.affection = (self.affection + amount).min(self.max_affection);
        if was_below && self.affection >= self.max_affection {
            self.just_bonded = true;
        }
    }

    /// Reduce affection; fires `just_estranged` when reaching 0.
    /// No-op when disabled or already estranged.
    pub fn estrange(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.affection <= 0.0 {
            return;
        }
        self.affection = (self.affection - amount).max(0.0);
        if self.affection <= 0.0 {
            self.just_estranged = true;
        }
    }

    /// Clear flags, then increase affection by `empathy_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_bonded = false;
        self.just_estranged = false;
        if self.enabled && self.empathy_rate > 0.0 && self.affection < self.max_affection {
            let was_below = self.affection < self.max_affection;
            self.affection = (self.affection + self.empathy_rate * dt).min(self.max_affection);
            if was_below && self.affection >= self.max_affection {
                self.just_bonded = true;
            }
        }
    }

    /// `true` when affection is at maximum and component is enabled.
    pub fn is_bonded(&self) -> bool {
        self.affection >= self.max_affection && self.enabled
    }

    /// `true` when affection is 0 (not gated by `enabled`).
    pub fn is_estranged(&self) -> bool {
        self.affection == 0.0
    }

    /// Fraction of maximum affection [0.0, 1.0].
    pub fn affection_fraction(&self) -> f32 {
        (self.affection / self.max_affection).clamp(0.0, 1.0)
    }

    /// Returns `scale * affection_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_kinship(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.affection_fraction()
    }
}

impl Default for Zoophile {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoophile {
        Zoophile::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_estranged() {
        let z = z();
        assert_eq!(z.affection, 0.0);
        assert!(z.is_estranged());
        assert!(!z.is_bonded());
    }

    #[test]
    fn new_clamps_max_affection() {
        let z = Zoophile::new(-5.0, 1.5);
        assert!((z.max_affection - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_empathy_rate() {
        let z = Zoophile::new(100.0, -1.5);
        assert_eq!(z.empathy_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoophile::default();
        assert!((z.max_affection - 100.0).abs() < 1e-5);
        assert!((z.empathy_rate - 1.5).abs() < 1e-5);
    }

    // --- bond ---

    #[test]
    fn bond_adds_affection() {
        let mut z = z();
        z.bond(40.0);
        assert!((z.affection - 40.0).abs() < 1e-3);
    }

    #[test]
    fn bond_clamps_at_max() {
        let mut z = z();
        z.bond(200.0);
        assert!((z.affection - 100.0).abs() < 1e-3);
    }

    #[test]
    fn bond_fires_just_bonded_at_max() {
        let mut z = z();
        z.bond(100.0);
        assert!(z.just_bonded);
        assert!(z.is_bonded());
    }

    #[test]
    fn bond_no_just_bonded_when_already_at_max() {
        let mut z = z();
        z.affection = 100.0;
        z.bond(10.0);
        assert!(!z.just_bonded);
    }

    #[test]
    fn bond_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.bond(50.0);
        assert_eq!(z.affection, 0.0);
    }

    #[test]
    fn bond_no_op_when_amount_zero() {
        let mut z = z();
        z.bond(0.0);
        assert_eq!(z.affection, 0.0);
    }

    // --- estrange ---

    #[test]
    fn estrange_reduces_affection() {
        let mut z = z();
        z.affection = 60.0;
        z.estrange(20.0);
        assert!((z.affection - 40.0).abs() < 1e-3);
    }

    #[test]
    fn estrange_clamps_at_zero() {
        let mut z = z();
        z.affection = 30.0;
        z.estrange(200.0);
        assert_eq!(z.affection, 0.0);
    }

    #[test]
    fn estrange_fires_just_estranged_at_zero() {
        let mut z = z();
        z.affection = 30.0;
        z.estrange(30.0);
        assert!(z.just_estranged);
    }

    #[test]
    fn estrange_no_op_when_already_estranged() {
        let mut z = z();
        z.estrange(10.0);
        assert!(!z.just_estranged);
    }

    #[test]
    fn estrange_no_op_when_disabled() {
        let mut z = z();
        z.affection = 50.0;
        z.enabled = false;
        z.estrange(50.0);
        assert!((z.affection - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_bonds_affection() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.affection - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_bonded_on_bond_to_max() {
        let mut z = Zoophile::new(100.0, 200.0);
        z.affection = 95.0;
        z.tick(1.0);
        assert!(z.just_bonded);
        assert!(z.is_bonded());
    }

    #[test]
    fn tick_no_bond_when_already_bonded() {
        let mut z = z();
        z.affection = 100.0;
        z.tick(1.0);
        assert!(!z.just_bonded);
    }

    #[test]
    fn tick_no_bond_when_rate_zero() {
        let mut z = Zoophile::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.affection, 0.0);
    }

    #[test]
    fn tick_no_bond_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.affection, 0.0);
    }

    #[test]
    fn tick_clears_just_bonded() {
        let mut z = Zoophile::new(100.0, 200.0);
        z.affection = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_bonded);
    }

    #[test]
    fn tick_clears_just_estranged() {
        let mut z = z();
        z.affection = 10.0;
        z.estrange(10.0);
        z.tick(0.016);
        assert!(!z.just_estranged);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.affection - 9.0).abs() < 1e-3);
    }

    // --- is_bonded / is_estranged ---

    #[test]
    fn is_bonded_false_when_disabled() {
        let mut z = z();
        z.affection = 100.0;
        z.enabled = false;
        assert!(!z.is_bonded());
    }

    #[test]
    fn is_estranged_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_estranged());
    }

    // --- affection_fraction / effective_kinship ---

    #[test]
    fn affection_fraction_zero_when_estranged() {
        assert_eq!(z().affection_fraction(), 0.0);
    }

    #[test]
    fn affection_fraction_half_at_midpoint() {
        let mut z = z();
        z.affection = 50.0;
        assert!((z.affection_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_kinship_zero_when_estranged() {
        assert_eq!(z().effective_kinship(100.0), 0.0);
    }

    #[test]
    fn effective_kinship_scales_with_affection() {
        let mut z = z();
        z.affection = 75.0;
        assert!((z.effective_kinship(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_kinship_zero_when_disabled() {
        let mut z = z();
        z.affection = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_kinship(100.0), 0.0);
    }
}
