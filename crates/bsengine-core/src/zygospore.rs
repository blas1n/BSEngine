use bevy_ecs::prelude::Component;

/// Fungal-resting-spore dormancy tracker named after zygospore,
/// the thick-walled resting structure formed when two compatible
/// hyphal branches fuse their contents in the sexual cycle of
/// zygomycetes — the mould division that includes bread moulds
/// (Rhizopus) and pin moulds (Mucor). When nutrients run low or
/// conditions turn hostile, the conjugating cells build a
/// multilayered spore wall that is impermeable to desiccation,
/// resistant to ultraviolet radiation, and capable of remaining
/// viable for years until moisture and warmth signal that
/// germination can safely begin. `dormancy` builds via
/// `sporulate(amount)` and increases passively at `harden_rate`
/// per second in `tick(dt)` or is reduced via `germinate(amount)`.
///
/// Models dormancy fill levels, resting-spore wall-thickness
/// saturation bars, stress-resistance accumulation trackers,
/// mould-colony survival-mode indicators, metabolic-arrest
/// depth gauges, winter-spore maturity progress meters,
/// drought-resistance saturation trackers, spore-viability
/// quality-control fill levels, mycological-culture bank
/// health indicators, or any mechanic where building up a
/// thick protective wall puts an organism into deep suspension
/// that lets it outlast catastrophes that would kill any
/// actively growing competitor — right up until a signal
/// chemical or a change in osmotic pressure cracks the wall
/// open and the dormant cell rushes back into life.
///
/// `sporulate(amount)` adds dormancy; fires `just_encapsulated`
/// when first reaching `max_dormancy`. No-op when disabled.
///
/// `germinate(amount)` reduces dormancy immediately; fires
/// `just_germinated` when reaching 0. No-op when disabled or
/// already germinated.
///
/// `tick(dt)` clears both flags, then increases dormancy by
/// `harden_rate * dt` (capped at `max_dormancy`). Fires
/// `just_encapsulated` when first reaching max. No-op when
/// disabled or rate is 0.
///
/// `is_encapsulated()` returns `dormancy >= max_dormancy && enabled`.
///
/// `is_germinated()` returns `dormancy == 0.0` (not gated by `enabled`).
///
/// `dormancy_fraction()` returns `(dormancy / max_dormancy).clamp(0, 1)`.
///
/// `effective_resilience(scale)` returns `scale * dormancy_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — hardens at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zygospore {
    pub dormancy: f32,
    pub max_dormancy: f32,
    pub harden_rate: f32,
    pub just_encapsulated: bool,
    pub just_germinated: bool,
    pub enabled: bool,
}

impl Zygospore {
    pub fn new(max_dormancy: f32, harden_rate: f32) -> Self {
        Self {
            dormancy: 0.0,
            max_dormancy: max_dormancy.max(0.1),
            harden_rate: harden_rate.max(0.0),
            just_encapsulated: false,
            just_germinated: false,
            enabled: true,
        }
    }

    /// Add dormancy; fires `just_encapsulated` when first reaching max.
    /// No-op when disabled.
    pub fn sporulate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.dormancy < self.max_dormancy;
        self.dormancy = (self.dormancy + amount).min(self.max_dormancy);
        if was_below && self.dormancy >= self.max_dormancy {
            self.just_encapsulated = true;
        }
    }

    /// Reduce dormancy; fires `just_germinated` when reaching 0.
    /// No-op when disabled or already germinated.
    pub fn germinate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.dormancy <= 0.0 {
            return;
        }
        self.dormancy = (self.dormancy - amount).max(0.0);
        if self.dormancy <= 0.0 {
            self.just_germinated = true;
        }
    }

    /// Clear flags, then increase dormancy by `harden_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_encapsulated = false;
        self.just_germinated = false;
        if self.enabled && self.harden_rate > 0.0 && self.dormancy < self.max_dormancy {
            let was_below = self.dormancy < self.max_dormancy;
            self.dormancy = (self.dormancy + self.harden_rate * dt).min(self.max_dormancy);
            if was_below && self.dormancy >= self.max_dormancy {
                self.just_encapsulated = true;
            }
        }
    }

    /// `true` when dormancy is at maximum and component is enabled.
    pub fn is_encapsulated(&self) -> bool {
        self.dormancy >= self.max_dormancy && self.enabled
    }

    /// `true` when dormancy is 0 (not gated by `enabled`).
    pub fn is_germinated(&self) -> bool {
        self.dormancy == 0.0
    }

    /// Fraction of maximum dormancy [0.0, 1.0].
    pub fn dormancy_fraction(&self) -> f32 {
        (self.dormancy / self.max_dormancy).clamp(0.0, 1.0)
    }

    /// Returns `scale * dormancy_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_resilience(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.dormancy_fraction()
    }
}

impl Default for Zygospore {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zygospore {
        Zygospore::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_germinated() {
        let z = z();
        assert_eq!(z.dormancy, 0.0);
        assert!(z.is_germinated());
        assert!(!z.is_encapsulated());
    }

    #[test]
    fn new_clamps_max_dormancy() {
        let z = Zygospore::new(-5.0, 1.5);
        assert!((z.max_dormancy - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_harden_rate() {
        let z = Zygospore::new(100.0, -1.5);
        assert_eq!(z.harden_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zygospore::default();
        assert!((z.max_dormancy - 100.0).abs() < 1e-5);
        assert!((z.harden_rate - 1.5).abs() < 1e-5);
    }

    // --- sporulate ---

    #[test]
    fn sporulate_adds_dormancy() {
        let mut z = z();
        z.sporulate(40.0);
        assert!((z.dormancy - 40.0).abs() < 1e-3);
    }

    #[test]
    fn sporulate_clamps_at_max() {
        let mut z = z();
        z.sporulate(200.0);
        assert!((z.dormancy - 100.0).abs() < 1e-3);
    }

    #[test]
    fn sporulate_fires_just_encapsulated_at_max() {
        let mut z = z();
        z.sporulate(100.0);
        assert!(z.just_encapsulated);
        assert!(z.is_encapsulated());
    }

    #[test]
    fn sporulate_no_just_encapsulated_when_already_at_max() {
        let mut z = z();
        z.dormancy = 100.0;
        z.sporulate(10.0);
        assert!(!z.just_encapsulated);
    }

    #[test]
    fn sporulate_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.sporulate(50.0);
        assert_eq!(z.dormancy, 0.0);
    }

    #[test]
    fn sporulate_no_op_when_amount_zero() {
        let mut z = z();
        z.sporulate(0.0);
        assert_eq!(z.dormancy, 0.0);
    }

    // --- germinate ---

    #[test]
    fn germinate_reduces_dormancy() {
        let mut z = z();
        z.dormancy = 60.0;
        z.germinate(20.0);
        assert!((z.dormancy - 40.0).abs() < 1e-3);
    }

    #[test]
    fn germinate_clamps_at_zero() {
        let mut z = z();
        z.dormancy = 30.0;
        z.germinate(200.0);
        assert_eq!(z.dormancy, 0.0);
    }

    #[test]
    fn germinate_fires_just_germinated_at_zero() {
        let mut z = z();
        z.dormancy = 30.0;
        z.germinate(30.0);
        assert!(z.just_germinated);
    }

    #[test]
    fn germinate_no_op_when_already_germinated() {
        let mut z = z();
        z.germinate(10.0);
        assert!(!z.just_germinated);
    }

    #[test]
    fn germinate_no_op_when_disabled() {
        let mut z = z();
        z.dormancy = 50.0;
        z.enabled = false;
        z.germinate(50.0);
        assert!((z.dormancy - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_hardens_dormancy() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.dormancy - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_encapsulated_on_harden_to_max() {
        let mut z = Zygospore::new(100.0, 200.0);
        z.dormancy = 95.0;
        z.tick(1.0);
        assert!(z.just_encapsulated);
        assert!(z.is_encapsulated());
    }

    #[test]
    fn tick_no_harden_when_already_encapsulated() {
        let mut z = z();
        z.dormancy = 100.0;
        z.tick(1.0);
        assert!(!z.just_encapsulated);
    }

    #[test]
    fn tick_no_harden_when_rate_zero() {
        let mut z = Zygospore::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.dormancy, 0.0);
    }

    #[test]
    fn tick_no_harden_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.dormancy, 0.0);
    }

    #[test]
    fn tick_clears_just_encapsulated() {
        let mut z = Zygospore::new(100.0, 200.0);
        z.dormancy = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_encapsulated);
    }

    #[test]
    fn tick_clears_just_germinated() {
        let mut z = z();
        z.dormancy = 10.0;
        z.germinate(10.0);
        z.tick(0.016);
        assert!(!z.just_germinated);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.dormancy - 9.0).abs() < 1e-3);
    }

    // --- is_encapsulated / is_germinated ---

    #[test]
    fn is_encapsulated_false_when_disabled() {
        let mut z = z();
        z.dormancy = 100.0;
        z.enabled = false;
        assert!(!z.is_encapsulated());
    }

    #[test]
    fn is_germinated_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_germinated());
    }

    // --- dormancy_fraction / effective_resilience ---

    #[test]
    fn dormancy_fraction_zero_when_germinated() {
        assert_eq!(z().dormancy_fraction(), 0.0);
    }

    #[test]
    fn dormancy_fraction_half_at_midpoint() {
        let mut z = z();
        z.dormancy = 50.0;
        assert!((z.dormancy_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_resilience_zero_when_germinated() {
        assert_eq!(z().effective_resilience(100.0), 0.0);
    }

    #[test]
    fn effective_resilience_scales_with_dormancy() {
        let mut z = z();
        z.dormancy = 75.0;
        assert!((z.effective_resilience(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_resilience_zero_when_disabled() {
        let mut z = z();
        z.dormancy = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_resilience(100.0), 0.0);
    }
}
