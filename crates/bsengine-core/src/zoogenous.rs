use bevy_ecs::prelude::Component;

/// Animal-origin accumulation tracker named after zoogenous, the
/// adjective meaning "produced by, derived from, or having an animal
/// origin." The term is most commonly encountered in geology, ecology,
/// and soil science: a zoogenous reef is one constructed by carbonate-
/// secreting animals — corals, bryozoans, oysters, vermetid gastropods,
/// tube-building polychaetes — rather than by inorganic precipitation;
/// zoogenous rock is any sedimentary rock whose constituent particles
/// are organic in origin, such as chalk (composed of coccolithophore
/// plates), crinoidal limestone (packed crinoidal ossicles), or
/// graptolite shale (flattened colonial hemichordate skeletons). In
/// soil science, zoogenous soil is enriched by animal matter — dung,
/// carcasses, bones, shell middens — and typically shows elevated
/// phosphorus and calcium compared with surrounding soils; seabird
/// colonies build zoogenous soils on remote cliffs, and whale fall
/// creates temporary zoogenous hotspots on the abyssal plain that
/// support communities of specialist scavengers and chemosynthesisers
/// for decades. In ecology, zoogenous allochthonous inputs are
/// materials of animal origin transported into a receiving ecosystem
/// from outside: salmon carcasses carried up river banks by bears,
/// guano deposited by roosting bats, seal carcasses washed above the
/// tide line by storms. `genesis` builds via `emerge(amount)` and
/// accumulates passively at `origin_rate` per second in `tick(dt)` or
/// diminishes via `decay(amount)`.
///
/// Models animal-origin fill levels, organic-input saturation bars,
/// zoogenic-soil accumulation trackers, carcass-decomposition gauges,
/// reef-calcification fill levels, biogenic-carbonate saturation
/// indicators, animal-nutrient input accumulation bars, allochthonous-
/// matter arrival meters, bone-bed formation fill levels, or any
/// mechanic where the slow accumulation of animal-derived material —
/// bone, shell, dung, carcass, exoskeleton — enriches a substrate,
/// builds a structure, or drives a chemical process until the
/// zoogenous input reaches peak intensity.
///
/// `emerge(amount)` adds genesis; fires `just_emerged` when first
/// reaching `max_genesis`. No-op when disabled.
///
/// `decay(amount)` reduces genesis immediately; fires `just_inert`
/// when reaching 0. No-op when disabled or already inert.
///
/// `tick(dt)` clears both flags, then increases genesis by
/// `origin_rate * dt` (capped at `max_genesis`). Fires `just_emerged`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_emerged()` returns `genesis >= max_genesis && enabled`.
///
/// `is_inert()` returns `genesis == 0.0` (not gated by `enabled`).
///
/// `genesis_fraction()` returns
/// `(genesis / max_genesis).clamp(0, 1)`.
///
/// `effective_yield(scale)` returns `scale * genesis_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — originates at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoogenous {
    pub genesis: f32,
    pub max_genesis: f32,
    pub origin_rate: f32,
    pub just_emerged: bool,
    pub just_inert: bool,
    pub enabled: bool,
}

impl Zoogenous {
    pub fn new(max_genesis: f32, origin_rate: f32) -> Self {
        Self {
            genesis: 0.0,
            max_genesis: max_genesis.max(0.1),
            origin_rate: origin_rate.max(0.0),
            just_emerged: false,
            just_inert: false,
            enabled: true,
        }
    }

    /// Add genesis; fires `just_emerged` when first reaching max.
    /// No-op when disabled.
    pub fn emerge(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.genesis < self.max_genesis;
        self.genesis = (self.genesis + amount).min(self.max_genesis);
        if was_below && self.genesis >= self.max_genesis {
            self.just_emerged = true;
        }
    }

    /// Reduce genesis; fires `just_inert` when reaching 0.
    /// No-op when disabled or already inert.
    pub fn decay(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.genesis <= 0.0 {
            return;
        }
        self.genesis = (self.genesis - amount).max(0.0);
        if self.genesis <= 0.0 {
            self.just_inert = true;
        }
    }

    /// Clear flags, then increase genesis by `origin_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_emerged = false;
        self.just_inert = false;
        if self.enabled && self.origin_rate > 0.0 && self.genesis < self.max_genesis {
            let was_below = self.genesis < self.max_genesis;
            self.genesis = (self.genesis + self.origin_rate * dt).min(self.max_genesis);
            if was_below && self.genesis >= self.max_genesis {
                self.just_emerged = true;
            }
        }
    }

    /// `true` when genesis is at maximum and component is enabled.
    pub fn is_emerged(&self) -> bool {
        self.genesis >= self.max_genesis && self.enabled
    }

    /// `true` when genesis is 0 (not gated by `enabled`).
    pub fn is_inert(&self) -> bool {
        self.genesis == 0.0
    }

    /// Fraction of maximum genesis [0.0, 1.0].
    pub fn genesis_fraction(&self) -> f32 {
        (self.genesis / self.max_genesis).clamp(0.0, 1.0)
    }

    /// Returns `scale * genesis_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_yield(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.genesis_fraction()
    }
}

impl Default for Zoogenous {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoogenous {
        Zoogenous::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_inert() {
        let z = z();
        assert_eq!(z.genesis, 0.0);
        assert!(z.is_inert());
        assert!(!z.is_emerged());
    }

    #[test]
    fn new_clamps_max_genesis() {
        let z = Zoogenous::new(-5.0, 1.5);
        assert!((z.max_genesis - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_origin_rate() {
        let z = Zoogenous::new(100.0, -1.5);
        assert_eq!(z.origin_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoogenous::default();
        assert!((z.max_genesis - 100.0).abs() < 1e-5);
        assert!((z.origin_rate - 1.5).abs() < 1e-5);
    }

    // --- emerge ---

    #[test]
    fn emerge_adds_genesis() {
        let mut z = z();
        z.emerge(40.0);
        assert!((z.genesis - 40.0).abs() < 1e-3);
    }

    #[test]
    fn emerge_clamps_at_max() {
        let mut z = z();
        z.emerge(200.0);
        assert!((z.genesis - 100.0).abs() < 1e-3);
    }

    #[test]
    fn emerge_fires_just_emerged_at_max() {
        let mut z = z();
        z.emerge(100.0);
        assert!(z.just_emerged);
        assert!(z.is_emerged());
    }

    #[test]
    fn emerge_no_just_emerged_when_already_at_max() {
        let mut z = z();
        z.genesis = 100.0;
        z.emerge(10.0);
        assert!(!z.just_emerged);
    }

    #[test]
    fn emerge_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.emerge(50.0);
        assert_eq!(z.genesis, 0.0);
    }

    #[test]
    fn emerge_no_op_when_amount_zero() {
        let mut z = z();
        z.emerge(0.0);
        assert_eq!(z.genesis, 0.0);
    }

    // --- decay ---

    #[test]
    fn decay_reduces_genesis() {
        let mut z = z();
        z.genesis = 60.0;
        z.decay(20.0);
        assert!((z.genesis - 40.0).abs() < 1e-3);
    }

    #[test]
    fn decay_clamps_at_zero() {
        let mut z = z();
        z.genesis = 30.0;
        z.decay(200.0);
        assert_eq!(z.genesis, 0.0);
    }

    #[test]
    fn decay_fires_just_inert_at_zero() {
        let mut z = z();
        z.genesis = 30.0;
        z.decay(30.0);
        assert!(z.just_inert);
    }

    #[test]
    fn decay_no_op_when_already_inert() {
        let mut z = z();
        z.decay(10.0);
        assert!(!z.just_inert);
    }

    #[test]
    fn decay_no_op_when_disabled() {
        let mut z = z();
        z.genesis = 50.0;
        z.enabled = false;
        z.decay(50.0);
        assert!((z.genesis - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_originates_genesis() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.genesis - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_emerged_on_origin_to_max() {
        let mut z = Zoogenous::new(100.0, 200.0);
        z.genesis = 95.0;
        z.tick(1.0);
        assert!(z.just_emerged);
        assert!(z.is_emerged());
    }

    #[test]
    fn tick_no_origin_when_already_emerged() {
        let mut z = z();
        z.genesis = 100.0;
        z.tick(1.0);
        assert!(!z.just_emerged);
    }

    #[test]
    fn tick_no_origin_when_rate_zero() {
        let mut z = Zoogenous::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.genesis, 0.0);
    }

    #[test]
    fn tick_no_origin_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.genesis, 0.0);
    }

    #[test]
    fn tick_clears_just_emerged() {
        let mut z = Zoogenous::new(100.0, 200.0);
        z.genesis = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_emerged);
    }

    #[test]
    fn tick_clears_just_inert() {
        let mut z = z();
        z.genesis = 10.0;
        z.decay(10.0);
        z.tick(0.016);
        assert!(!z.just_inert);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.genesis - 9.0).abs() < 1e-3);
    }

    // --- is_emerged / is_inert ---

    #[test]
    fn is_emerged_false_when_disabled() {
        let mut z = z();
        z.genesis = 100.0;
        z.enabled = false;
        assert!(!z.is_emerged());
    }

    #[test]
    fn is_inert_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_inert());
    }

    // --- genesis_fraction / effective_yield ---

    #[test]
    fn genesis_fraction_zero_when_inert() {
        assert_eq!(z().genesis_fraction(), 0.0);
    }

    #[test]
    fn genesis_fraction_half_at_midpoint() {
        let mut z = z();
        z.genesis = 50.0;
        z.decay(0.0);
        assert!((z.genesis_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_yield_zero_when_inert() {
        assert_eq!(z().effective_yield(100.0), 0.0);
    }

    #[test]
    fn effective_yield_scales_with_genesis() {
        let mut z = z();
        z.genesis = 75.0;
        assert!((z.effective_yield(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_yield_zero_when_disabled() {
        let mut z = z();
        z.genesis = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_yield(100.0), 0.0);
    }
}
