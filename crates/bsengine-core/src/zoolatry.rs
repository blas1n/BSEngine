use bevy_ecs::prelude::Component;

/// Animal-deity devotion tracker named after zoolatry, the worship of
/// animals — the practice of treating non-human creatures as divine
/// beings or as the earthly vessels of gods. Zoolatry is among the
/// oldest attested religious behaviours in the archaeological record:
/// the cave paintings of Lascaux and Altamira treat aurochs and bison
/// with something that looks indistinguishable from reverence; ancient
/// Egypt formalised it in the cults of Apis (sacred bull), Bastet
/// (cat goddess), Sobek (crocodile god), and Thoth (ibis); the Aztec
/// plumed serpent Quetzalcóatl and the Vedic Nandi bull belong to the
/// same broad pattern. Where the deity is believed to inhabit a living
/// animal, that animal must be fed, groomed, and guarded by temple
/// staff; when it dies its mummified body is buried in a sacred
/// necropolis with all the ceremony due to a monarch. `devotion`
/// builds via `worship(amount)` and accumulates passively at
/// `revere_rate` per second in `tick(dt)` or is reduced via
/// `desecrate(amount)`.
///
/// Models animal-deity temple fill levels, sacred-creature reverence
/// saturation bars, zoolatrous cult-devotion accumulation trackers,
/// totem-animal sacred-status fill levels, prehistoric-cave-art ritual-
/// intent gauges, Apis-bull ceremonial-honour saturation indicators,
/// crocodile-god sanctuary fill levels, spiritual-bond-with-beast
/// accumulation bars, primordial-hunt-deity propitiation meters, or any
/// mechanic where the player or a faction channels more and more ritual
/// energy into venerating a specific animal until the creature becomes
/// fully divine — shedding its mundane biology and becoming something
/// that must be protected at any cost lest the cosmic order collapse
/// along with it.
///
/// `worship(amount)` adds devotion; fires `just_revered` when first
/// reaching `max_devotion`. No-op when disabled.
///
/// `desecrate(amount)` reduces devotion immediately; fires
/// `just_profaned` when reaching 0. No-op when disabled or already
/// profaned.
///
/// `tick(dt)` clears both flags, then increases devotion by
/// `revere_rate * dt` (capped at `max_devotion`). Fires `just_revered`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_revered()` returns `devotion >= max_devotion && enabled`.
///
/// `is_profaned()` returns `devotion == 0.0` (not gated by `enabled`).
///
/// `devotion_fraction()` returns `(devotion / max_devotion).clamp(0, 1)`.
///
/// `effective_sanctity(scale)` returns `scale * devotion_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — reveres at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoolatry {
    pub devotion: f32,
    pub max_devotion: f32,
    pub revere_rate: f32,
    pub just_revered: bool,
    pub just_profaned: bool,
    pub enabled: bool,
}

impl Zoolatry {
    pub fn new(max_devotion: f32, revere_rate: f32) -> Self {
        Self {
            devotion: 0.0,
            max_devotion: max_devotion.max(0.1),
            revere_rate: revere_rate.max(0.0),
            just_revered: false,
            just_profaned: false,
            enabled: true,
        }
    }

    /// Add devotion; fires `just_revered` when first reaching max.
    /// No-op when disabled.
    pub fn worship(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.devotion < self.max_devotion;
        self.devotion = (self.devotion + amount).min(self.max_devotion);
        if was_below && self.devotion >= self.max_devotion {
            self.just_revered = true;
        }
    }

    /// Reduce devotion; fires `just_profaned` when reaching 0.
    /// No-op when disabled or already profaned.
    pub fn desecrate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.devotion <= 0.0 {
            return;
        }
        self.devotion = (self.devotion - amount).max(0.0);
        if self.devotion <= 0.0 {
            self.just_profaned = true;
        }
    }

    /// Clear flags, then increase devotion by `revere_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_revered = false;
        self.just_profaned = false;
        if self.enabled && self.revere_rate > 0.0 && self.devotion < self.max_devotion {
            let was_below = self.devotion < self.max_devotion;
            self.devotion = (self.devotion + self.revere_rate * dt).min(self.max_devotion);
            if was_below && self.devotion >= self.max_devotion {
                self.just_revered = true;
            }
        }
    }

    /// `true` when devotion is at maximum and component is enabled.
    pub fn is_revered(&self) -> bool {
        self.devotion >= self.max_devotion && self.enabled
    }

    /// `true` when devotion is 0 (not gated by `enabled`).
    pub fn is_profaned(&self) -> bool {
        self.devotion == 0.0
    }

    /// Fraction of maximum devotion [0.0, 1.0].
    pub fn devotion_fraction(&self) -> f32 {
        (self.devotion / self.max_devotion).clamp(0.0, 1.0)
    }

    /// Returns `scale * devotion_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_sanctity(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.devotion_fraction()
    }
}

impl Default for Zoolatry {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoolatry {
        Zoolatry::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_profaned() {
        let z = z();
        assert_eq!(z.devotion, 0.0);
        assert!(z.is_profaned());
        assert!(!z.is_revered());
    }

    #[test]
    fn new_clamps_max_devotion() {
        let z = Zoolatry::new(-5.0, 1.5);
        assert!((z.max_devotion - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_revere_rate() {
        let z = Zoolatry::new(100.0, -1.5);
        assert_eq!(z.revere_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoolatry::default();
        assert!((z.max_devotion - 100.0).abs() < 1e-5);
        assert!((z.revere_rate - 1.5).abs() < 1e-5);
    }

    // --- worship ---

    #[test]
    fn worship_adds_devotion() {
        let mut z = z();
        z.worship(40.0);
        assert!((z.devotion - 40.0).abs() < 1e-3);
    }

    #[test]
    fn worship_clamps_at_max() {
        let mut z = z();
        z.worship(200.0);
        assert!((z.devotion - 100.0).abs() < 1e-3);
    }

    #[test]
    fn worship_fires_just_revered_at_max() {
        let mut z = z();
        z.worship(100.0);
        assert!(z.just_revered);
        assert!(z.is_revered());
    }

    #[test]
    fn worship_no_just_revered_when_already_at_max() {
        let mut z = z();
        z.devotion = 100.0;
        z.worship(10.0);
        assert!(!z.just_revered);
    }

    #[test]
    fn worship_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.worship(50.0);
        assert_eq!(z.devotion, 0.0);
    }

    #[test]
    fn worship_no_op_when_amount_zero() {
        let mut z = z();
        z.worship(0.0);
        assert_eq!(z.devotion, 0.0);
    }

    // --- desecrate ---

    #[test]
    fn desecrate_reduces_devotion() {
        let mut z = z();
        z.devotion = 60.0;
        z.desecrate(20.0);
        assert!((z.devotion - 40.0).abs() < 1e-3);
    }

    #[test]
    fn desecrate_clamps_at_zero() {
        let mut z = z();
        z.devotion = 30.0;
        z.desecrate(200.0);
        assert_eq!(z.devotion, 0.0);
    }

    #[test]
    fn desecrate_fires_just_profaned_at_zero() {
        let mut z = z();
        z.devotion = 30.0;
        z.desecrate(30.0);
        assert!(z.just_profaned);
    }

    #[test]
    fn desecrate_no_op_when_already_profaned() {
        let mut z = z();
        z.desecrate(10.0);
        assert!(!z.just_profaned);
    }

    #[test]
    fn desecrate_no_op_when_disabled() {
        let mut z = z();
        z.devotion = 50.0;
        z.enabled = false;
        z.desecrate(50.0);
        assert!((z.devotion - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_reveres_devotion() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.devotion - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_revered_on_revere_to_max() {
        let mut z = Zoolatry::new(100.0, 200.0);
        z.devotion = 95.0;
        z.tick(1.0);
        assert!(z.just_revered);
        assert!(z.is_revered());
    }

    #[test]
    fn tick_no_revere_when_already_revered() {
        let mut z = z();
        z.devotion = 100.0;
        z.tick(1.0);
        assert!(!z.just_revered);
    }

    #[test]
    fn tick_no_revere_when_rate_zero() {
        let mut z = Zoolatry::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.devotion, 0.0);
    }

    #[test]
    fn tick_no_revere_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.devotion, 0.0);
    }

    #[test]
    fn tick_clears_just_revered() {
        let mut z = Zoolatry::new(100.0, 200.0);
        z.devotion = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_revered);
    }

    #[test]
    fn tick_clears_just_profaned() {
        let mut z = z();
        z.devotion = 10.0;
        z.desecrate(10.0);
        z.tick(0.016);
        assert!(!z.just_profaned);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.devotion - 9.0).abs() < 1e-3);
    }

    // --- is_revered / is_profaned ---

    #[test]
    fn is_revered_false_when_disabled() {
        let mut z = z();
        z.devotion = 100.0;
        z.enabled = false;
        assert!(!z.is_revered());
    }

    #[test]
    fn is_profaned_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_profaned());
    }

    // --- devotion_fraction / effective_sanctity ---

    #[test]
    fn devotion_fraction_zero_when_profaned() {
        assert_eq!(z().devotion_fraction(), 0.0);
    }

    #[test]
    fn devotion_fraction_half_at_midpoint() {
        let mut z = z();
        z.devotion = 50.0;
        assert!((z.devotion_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_sanctity_zero_when_profaned() {
        assert_eq!(z().effective_sanctity(100.0), 0.0);
    }

    #[test]
    fn effective_sanctity_scales_with_devotion() {
        let mut z = z();
        z.devotion = 75.0;
        assert!((z.effective_sanctity(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_sanctity_zero_when_disabled() {
        let mut z = z();
        z.devotion = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_sanctity(100.0), 0.0);
    }
}
