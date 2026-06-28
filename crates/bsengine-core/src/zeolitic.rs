use bevy_ecs::prelude::Component;

/// Zeolite-character accumulation tracker named after zeolitic, the
/// adjective describing materials that possess the structural and
/// chemical properties of zeolites — the family of crystalline
/// aluminosilicate minerals whose precisely ordered three-dimensional
/// frameworks of SiO₄ and AlO₄ tetrahedra enclose networks of
/// interconnected microporous channels and cavities on the scale of
/// individual molecules. The name zeolite, coined by Axel Fredrik
/// Cronstedt in 1756, comes from Greek zein "to boil" and lithos
/// "stone," because he observed water vapour escaping from heated
/// stilbite as though the mineral were boiling — the water having
/// been trapped in the framework cavities under ambient conditions.
/// A zeolitic material's value lies entirely in its microporous
/// architecture: the uniform pore geometry acts as a molecular sieve,
/// admitting molecules smaller than the pore diameter while excluding
/// larger ones; the aluminium substitutions within the tetrahedral
/// framework create localised negative charges that are balanced by
/// exchangeable cations, making zeolites powerful ion-exchange agents;
/// and the combination of confinement and acidity makes them among the
/// most widely used industrial catalysts, cracking long hydrocarbon
/// chains in petroleum refining, removing ammonia from wastewater,
/// and drying refrigerant streams to parts-per-million water levels.
/// `microporosity` builds via `crystallize(amount)` and accumulates
/// passively at `sieve_rate` per second in `tick(dt)` or collapses
/// via `collapse(amount)`.
///
/// Models zeolitic-framework fill levels, microporous-material
/// saturation bars, molecular-sieve selectivity gauges, ion-exchange-
/// capacity accumulation trackers, catalytic-site density indicators,
/// crystalline-order progress bars, adsorption-site fill levels,
/// pore-geometry completion meters, mineral-framework integrity
/// trackers, or any mechanic where a material slowly develops an
/// ordered internal architecture until every channel is precisely
/// dimensioned and every active site is in place — and where thermal
/// shock or chemical attack collapses that architecture back to
/// amorphous rubble in a single event.
///
/// `crystallize(amount)` adds microporosity; fires `just_ordered`
/// when first reaching `max_microporosity`. No-op when disabled.
///
/// `collapse(amount)` reduces microporosity immediately; fires
/// `just_amorphous` when reaching 0. No-op when disabled or already
/// amorphous.
///
/// `tick(dt)` clears both flags, then increases microporosity by
/// `sieve_rate * dt` (capped at `max_microporosity`). Fires
/// `just_ordered` when first reaching max. No-op when disabled or
/// rate is 0.
///
/// `is_ordered()` returns `microporosity >= max_microporosity && enabled`.
///
/// `is_amorphous()` returns `microporosity == 0.0` (not gated by `enabled`).
///
/// `microporosity_fraction()` returns
/// `(microporosity / max_microporosity).clamp(0, 1)`.
///
/// `effective_adsorption(scale)` returns `scale * microporosity_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — sieves at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zeolitic {
    pub microporosity: f32,
    pub max_microporosity: f32,
    pub sieve_rate: f32,
    pub just_ordered: bool,
    pub just_amorphous: bool,
    pub enabled: bool,
}

impl Zeolitic {
    pub fn new(max_microporosity: f32, sieve_rate: f32) -> Self {
        Self {
            microporosity: 0.0,
            max_microporosity: max_microporosity.max(0.1),
            sieve_rate: sieve_rate.max(0.0),
            just_ordered: false,
            just_amorphous: false,
            enabled: true,
        }
    }

    /// Add microporosity; fires `just_ordered` when first reaching max.
    /// No-op when disabled.
    pub fn crystallize(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.microporosity < self.max_microporosity;
        self.microporosity = (self.microporosity + amount).min(self.max_microporosity);
        if was_below && self.microporosity >= self.max_microporosity {
            self.just_ordered = true;
        }
    }

    /// Reduce microporosity; fires `just_amorphous` when reaching 0.
    /// No-op when disabled or already amorphous.
    pub fn collapse(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.microporosity <= 0.0 {
            return;
        }
        self.microporosity = (self.microporosity - amount).max(0.0);
        if self.microporosity <= 0.0 {
            self.just_amorphous = true;
        }
    }

    /// Clear flags, then increase microporosity by `sieve_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_ordered = false;
        self.just_amorphous = false;
        if self.enabled && self.sieve_rate > 0.0 && self.microporosity < self.max_microporosity {
            let was_below = self.microporosity < self.max_microporosity;
            self.microporosity =
                (self.microporosity + self.sieve_rate * dt).min(self.max_microporosity);
            if was_below && self.microporosity >= self.max_microporosity {
                self.just_ordered = true;
            }
        }
    }

    /// `true` when microporosity is at maximum and component is enabled.
    pub fn is_ordered(&self) -> bool {
        self.microporosity >= self.max_microporosity && self.enabled
    }

    /// `true` when microporosity is 0 (not gated by `enabled`).
    pub fn is_amorphous(&self) -> bool {
        self.microporosity == 0.0
    }

    /// Fraction of maximum microporosity [0.0, 1.0].
    pub fn microporosity_fraction(&self) -> f32 {
        (self.microporosity / self.max_microporosity).clamp(0.0, 1.0)
    }

    /// Returns `scale * microporosity_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_adsorption(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.microporosity_fraction()
    }
}

impl Default for Zeolitic {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zeolitic {
        Zeolitic::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_amorphous() {
        let z = z();
        assert_eq!(z.microporosity, 0.0);
        assert!(z.is_amorphous());
        assert!(!z.is_ordered());
    }

    #[test]
    fn new_clamps_max_microporosity() {
        let z = Zeolitic::new(-5.0, 1.5);
        assert!((z.max_microporosity - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_sieve_rate() {
        let z = Zeolitic::new(100.0, -1.5);
        assert_eq!(z.sieve_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zeolitic::default();
        assert!((z.max_microporosity - 100.0).abs() < 1e-5);
        assert!((z.sieve_rate - 1.5).abs() < 1e-5);
    }

    // --- crystallize ---

    #[test]
    fn crystallize_adds_microporosity() {
        let mut z = z();
        z.crystallize(40.0);
        assert!((z.microporosity - 40.0).abs() < 1e-3);
    }

    #[test]
    fn crystallize_clamps_at_max() {
        let mut z = z();
        z.crystallize(200.0);
        assert!((z.microporosity - 100.0).abs() < 1e-3);
    }

    #[test]
    fn crystallize_fires_just_ordered_at_max() {
        let mut z = z();
        z.crystallize(100.0);
        assert!(z.just_ordered);
        assert!(z.is_ordered());
    }

    #[test]
    fn crystallize_no_just_ordered_when_already_at_max() {
        let mut z = z();
        z.microporosity = 100.0;
        z.crystallize(10.0);
        assert!(!z.just_ordered);
    }

    #[test]
    fn crystallize_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.crystallize(50.0);
        assert_eq!(z.microporosity, 0.0);
    }

    #[test]
    fn crystallize_no_op_when_amount_zero() {
        let mut z = z();
        z.crystallize(0.0);
        assert_eq!(z.microporosity, 0.0);
    }

    // --- collapse ---

    #[test]
    fn collapse_reduces_microporosity() {
        let mut z = z();
        z.microporosity = 60.0;
        z.collapse(20.0);
        assert!((z.microporosity - 40.0).abs() < 1e-3);
    }

    #[test]
    fn collapse_clamps_at_zero() {
        let mut z = z();
        z.microporosity = 30.0;
        z.collapse(200.0);
        assert_eq!(z.microporosity, 0.0);
    }

    #[test]
    fn collapse_fires_just_amorphous_at_zero() {
        let mut z = z();
        z.microporosity = 30.0;
        z.collapse(30.0);
        assert!(z.just_amorphous);
    }

    #[test]
    fn collapse_no_op_when_already_amorphous() {
        let mut z = z();
        z.collapse(10.0);
        assert!(!z.just_amorphous);
    }

    #[test]
    fn collapse_no_op_when_disabled() {
        let mut z = z();
        z.microporosity = 50.0;
        z.enabled = false;
        z.collapse(50.0);
        assert!((z.microporosity - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_sieves_microporosity() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.microporosity - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_ordered_on_sieve_to_max() {
        let mut z = Zeolitic::new(100.0, 200.0);
        z.microporosity = 95.0;
        z.tick(1.0);
        assert!(z.just_ordered);
        assert!(z.is_ordered());
    }

    #[test]
    fn tick_no_sieve_when_already_ordered() {
        let mut z = z();
        z.microporosity = 100.0;
        z.tick(1.0);
        assert!(!z.just_ordered);
    }

    #[test]
    fn tick_no_sieve_when_rate_zero() {
        let mut z = Zeolitic::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.microporosity, 0.0);
    }

    #[test]
    fn tick_no_sieve_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.microporosity, 0.0);
    }

    #[test]
    fn tick_clears_just_ordered() {
        let mut z = Zeolitic::new(100.0, 200.0);
        z.microporosity = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_ordered);
    }

    #[test]
    fn tick_clears_just_amorphous() {
        let mut z = z();
        z.microporosity = 10.0;
        z.collapse(10.0);
        z.tick(0.016);
        assert!(!z.just_amorphous);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.microporosity - 9.0).abs() < 1e-3);
    }

    // --- is_ordered / is_amorphous ---

    #[test]
    fn is_ordered_false_when_disabled() {
        let mut z = z();
        z.microporosity = 100.0;
        z.enabled = false;
        assert!(!z.is_ordered());
    }

    #[test]
    fn is_amorphous_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_amorphous());
    }

    // --- microporosity_fraction / effective_adsorption ---

    #[test]
    fn microporosity_fraction_zero_when_amorphous() {
        assert_eq!(z().microporosity_fraction(), 0.0);
    }

    #[test]
    fn microporosity_fraction_half_at_midpoint() {
        let mut z = z();
        z.microporosity = 50.0;
        assert!((z.microporosity_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_adsorption_zero_when_amorphous() {
        assert_eq!(z().effective_adsorption(100.0), 0.0);
    }

    #[test]
    fn effective_adsorption_scales_with_microporosity() {
        let mut z = z();
        z.microporosity = 75.0;
        assert!((z.effective_adsorption(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_adsorption_zero_when_disabled() {
        let mut z = z();
        z.microporosity = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_adsorption(100.0), 0.0);
    }
}
