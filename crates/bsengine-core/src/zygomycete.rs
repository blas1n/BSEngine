use bevy_ecs::prelude::Component;

/// Fungal-mycelium accumulation tracker named after the zygomycete,
/// a member of the class Zygomycetes — a division of the kingdom Fungi
/// characterised by the formation of thick-walled sexual spores called
/// zygospores when compatible hyphae fuse across a conjugation bridge.
/// The most familiar zygomycete is Rhizopus stolonifer, the common black
/// bread mould, whose white cotton-like mycelium spreads across damp
/// surfaces within hours before erecting vertical aerial hyphae tipped
/// with globular sporangia packed with thousands of asexual spores; when
/// two compatible mating types encounter each other they send out
/// gametangia that press together, fuse, and harden into an ornate black
/// zygospore that can remain dormant for months before germinating into a
/// new sporangium. Closely related Mucor and Rhizomucor species are used
/// commercially in Asian fermented foods — tempeh starter, some rice
/// wines — exploiting the same aggressive saprotrophic metabolism that
/// makes the group such effective decomposers of organic matter in soil.
/// At the other end of the ecological spectrum, Entomophthora muscae
/// manipulates the behaviour of infected houseflies, compelling them to
/// climb to an elevated position before dying, whereupon the fungus
/// ruptures the abdominal wall to launch conidia toward any fly that
/// alights nearby. `mycelium` builds via `sporulate(amount)` and
/// accumulates passively at `spore_rate` per second in `tick(dt)` or
/// is cleared via `bleach(amount)`.
///
/// Models fungal-mycelium fill levels, spore-density saturation bars,
/// decomposition-network growth trackers, mycoparasite spread gauges,
/// zygospore-formation fill levels, saprotrophic-colony saturation
/// indicators, soil-fungus expansion accumulation bars, bread-mould
/// spread meters, entomopathogen-infection fill levels, or any mechanic
/// where a fungal or fungus-like colony slowly ramifies through a
/// substrate — digesting nutrients, releasing enzymes, and filling every
/// available crevice with pale filaments — until the colony is dense
/// enough to produce its final reproductive stage.
///
/// `sporulate(amount)` adds mycelium; fires `just_sporulated` when first
/// reaching `max_mycelium`. No-op when disabled.
///
/// `bleach(amount)` reduces mycelium immediately; fires `just_sterile`
/// when reaching 0. No-op when disabled or already sterile.
///
/// `tick(dt)` clears both flags, then increases mycelium by
/// `spore_rate * dt` (capped at `max_mycelium`). Fires `just_sporulated`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_sporulated()` returns `mycelium >= max_mycelium && enabled`.
///
/// `is_sterile()` returns `mycelium == 0.0` (not gated by `enabled`).
///
/// `mycelium_fraction()` returns
/// `(mycelium / max_mycelium).clamp(0, 1)`.
///
/// `effective_decomposition(scale)` returns `scale * mycelium_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — sporulates at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zygomycete {
    pub mycelium: f32,
    pub max_mycelium: f32,
    pub spore_rate: f32,
    pub just_sporulated: bool,
    pub just_sterile: bool,
    pub enabled: bool,
}

impl Zygomycete {
    pub fn new(max_mycelium: f32, spore_rate: f32) -> Self {
        Self {
            mycelium: 0.0,
            max_mycelium: max_mycelium.max(0.1),
            spore_rate: spore_rate.max(0.0),
            just_sporulated: false,
            just_sterile: false,
            enabled: true,
        }
    }

    /// Add mycelium; fires `just_sporulated` when first reaching max.
    /// No-op when disabled.
    pub fn sporulate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.mycelium < self.max_mycelium;
        self.mycelium = (self.mycelium + amount).min(self.max_mycelium);
        if was_below && self.mycelium >= self.max_mycelium {
            self.just_sporulated = true;
        }
    }

    /// Reduce mycelium; fires `just_sterile` when reaching 0.
    /// No-op when disabled or already sterile.
    pub fn bleach(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.mycelium <= 0.0 {
            return;
        }
        self.mycelium = (self.mycelium - amount).max(0.0);
        if self.mycelium <= 0.0 {
            self.just_sterile = true;
        }
    }

    /// Clear flags, then increase mycelium by `spore_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_sporulated = false;
        self.just_sterile = false;
        if self.enabled && self.spore_rate > 0.0 && self.mycelium < self.max_mycelium {
            let was_below = self.mycelium < self.max_mycelium;
            self.mycelium = (self.mycelium + self.spore_rate * dt).min(self.max_mycelium);
            if was_below && self.mycelium >= self.max_mycelium {
                self.just_sporulated = true;
            }
        }
    }

    /// `true` when mycelium is at maximum and component is enabled.
    pub fn is_sporulated(&self) -> bool {
        self.mycelium >= self.max_mycelium && self.enabled
    }

    /// `true` when mycelium is 0 (not gated by `enabled`).
    pub fn is_sterile(&self) -> bool {
        self.mycelium == 0.0
    }

    /// Fraction of maximum mycelium [0.0, 1.0].
    pub fn mycelium_fraction(&self) -> f32 {
        (self.mycelium / self.max_mycelium).clamp(0.0, 1.0)
    }

    /// Returns `scale * mycelium_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_decomposition(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.mycelium_fraction()
    }
}

impl Default for Zygomycete {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zygomycete {
        Zygomycete::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_sterile() {
        let z = z();
        assert_eq!(z.mycelium, 0.0);
        assert!(z.is_sterile());
        assert!(!z.is_sporulated());
    }

    #[test]
    fn new_clamps_max_mycelium() {
        let z = Zygomycete::new(-5.0, 1.5);
        assert!((z.max_mycelium - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_spore_rate() {
        let z = Zygomycete::new(100.0, -1.5);
        assert_eq!(z.spore_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zygomycete::default();
        assert!((z.max_mycelium - 100.0).abs() < 1e-5);
        assert!((z.spore_rate - 1.5).abs() < 1e-5);
    }

    // --- sporulate ---

    #[test]
    fn sporulate_adds_mycelium() {
        let mut z = z();
        z.sporulate(40.0);
        assert!((z.mycelium - 40.0).abs() < 1e-3);
    }

    #[test]
    fn sporulate_clamps_at_max() {
        let mut z = z();
        z.sporulate(200.0);
        assert!((z.mycelium - 100.0).abs() < 1e-3);
    }

    #[test]
    fn sporulate_fires_just_sporulated_at_max() {
        let mut z = z();
        z.sporulate(100.0);
        assert!(z.just_sporulated);
        assert!(z.is_sporulated());
    }

    #[test]
    fn sporulate_no_just_sporulated_when_already_at_max() {
        let mut z = z();
        z.mycelium = 100.0;
        z.sporulate(10.0);
        assert!(!z.just_sporulated);
    }

    #[test]
    fn sporulate_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.sporulate(50.0);
        assert_eq!(z.mycelium, 0.0);
    }

    #[test]
    fn sporulate_no_op_when_amount_zero() {
        let mut z = z();
        z.sporulate(0.0);
        assert_eq!(z.mycelium, 0.0);
    }

    // --- bleach ---

    #[test]
    fn bleach_reduces_mycelium() {
        let mut z = z();
        z.mycelium = 60.0;
        z.bleach(20.0);
        assert!((z.mycelium - 40.0).abs() < 1e-3);
    }

    #[test]
    fn bleach_clamps_at_zero() {
        let mut z = z();
        z.mycelium = 30.0;
        z.bleach(200.0);
        assert_eq!(z.mycelium, 0.0);
    }

    #[test]
    fn bleach_fires_just_sterile_at_zero() {
        let mut z = z();
        z.mycelium = 30.0;
        z.bleach(30.0);
        assert!(z.just_sterile);
    }

    #[test]
    fn bleach_no_op_when_already_sterile() {
        let mut z = z();
        z.bleach(10.0);
        assert!(!z.just_sterile);
    }

    #[test]
    fn bleach_no_op_when_disabled() {
        let mut z = z();
        z.mycelium = 50.0;
        z.enabled = false;
        z.bleach(50.0);
        assert!((z.mycelium - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_sporulates_mycelium() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.mycelium - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_sporulated_on_spore_to_max() {
        let mut z = Zygomycete::new(100.0, 200.0);
        z.mycelium = 95.0;
        z.tick(1.0);
        assert!(z.just_sporulated);
        assert!(z.is_sporulated());
    }

    #[test]
    fn tick_no_spore_when_already_sporulated() {
        let mut z = z();
        z.mycelium = 100.0;
        z.tick(1.0);
        assert!(!z.just_sporulated);
    }

    #[test]
    fn tick_no_spore_when_rate_zero() {
        let mut z = Zygomycete::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.mycelium, 0.0);
    }

    #[test]
    fn tick_no_spore_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.mycelium, 0.0);
    }

    #[test]
    fn tick_clears_just_sporulated() {
        let mut z = Zygomycete::new(100.0, 200.0);
        z.mycelium = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_sporulated);
    }

    #[test]
    fn tick_clears_just_sterile() {
        let mut z = z();
        z.mycelium = 10.0;
        z.bleach(10.0);
        z.tick(0.016);
        assert!(!z.just_sterile);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.mycelium - 9.0).abs() < 1e-3);
    }

    // --- is_sporulated / is_sterile ---

    #[test]
    fn is_sporulated_false_when_disabled() {
        let mut z = z();
        z.mycelium = 100.0;
        z.enabled = false;
        assert!(!z.is_sporulated());
    }

    #[test]
    fn is_sterile_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_sterile());
    }

    // --- mycelium_fraction / effective_decomposition ---

    #[test]
    fn mycelium_fraction_zero_when_sterile() {
        assert_eq!(z().mycelium_fraction(), 0.0);
    }

    #[test]
    fn mycelium_fraction_half_at_midpoint() {
        let mut z = z();
        z.mycelium = 50.0;
        assert!((z.mycelium_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_decomposition_zero_when_sterile() {
        assert_eq!(z().effective_decomposition(100.0), 0.0);
    }

    #[test]
    fn effective_decomposition_scales_with_mycelium() {
        let mut z = z();
        z.mycelium = 75.0;
        assert!((z.effective_decomposition(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_decomposition_zero_when_disabled() {
        let mut z = z();
        z.mycelium = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_decomposition(100.0), 0.0);
    }
}
