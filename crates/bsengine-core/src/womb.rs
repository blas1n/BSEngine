use bevy_ecs::prelude::Component;

/// Gestation-incubation accumulation tracker named after womb, the
/// noun meaning the organ in the lower body of a woman or female
/// mammal where offspring are conceived and in which they gestate
/// before birth; any place in which something is generated or
/// developed — from the Old English wamb, womb (belly, womb,
/// bowels, hollow interior), from the Proto-Germanic wambō
/// (belly, paunch, womb), from the Proto-Indo-European root
/// wombho- (belly, womb). The same root gave the Old Norse vömb
/// and the Gothic wamba (belly), both surviving in modern Germanic
/// languages. The extension from anatomical sense to metaphorical
/// sense — the womb of time, the womb of the earth, born from
/// the womb of the sea — appears early in the language and
/// reflects a deep human tendency to figure processes of generation
/// and emergence in terms of biological birth. In mythology,
/// the womb appears as the cosmic source: Gaia's womb brought
/// forth the world; the cave is the womb of the mountain; the
/// dark is the womb of light. Alchemists spoke of the alembic
/// as a womb in which base metals gestated toward gold. Medieval
/// theologians wrote of the womb of the church from which
/// baptism brought the soul to second birth. In game mechanics,
/// a womb mechanic models the slow accumulation of gestation
/// — the filling of an incubation bar, the maturation of a
/// seed into a creature, the development of an egg into a hatchling,
/// the growth of a plan into an event. `gestation` builds via
/// `nurture(amount)` and accumulates passively at `grow_rate`
/// per second in `tick(dt)` or regresses via `wither(amount)`.
///
/// Models gestation-incubation fill levels, egg-maturation
/// bars, seed-germination accumulators, creature-development
/// gauges, plan-maturation fill levels, larvae-pupation
/// saturation indicators, cocoon-completion accumulation bars,
/// fermentation meters, metamorphosis-completion fill levels,
/// or any mechanic where something slowly develops, matures,
/// or becomes ready inside a container, host, or environment —
/// each moment of nurture adding a fraction of development until
/// the threshold is crossed and birth, hatching, blooming, or
/// emergence occurs.
///
/// `nurture(amount)` adds gestation; fires `just_born` when
/// first reaching `max_gestation`. No-op when disabled.
///
/// `wither(amount)` reduces gestation immediately; fires
/// `just_lost` when reaching 0. No-op when disabled or
/// already empty.
///
/// `tick(dt)` clears both flags, then increases gestation by
/// `grow_rate * dt` (capped at `max_gestation`). Fires
/// `just_born` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_born()` returns `gestation >= max_gestation && enabled`.
///
/// `is_empty()` returns `gestation == 0.0` (not gated by
/// `enabled`).
///
/// `gestation_fraction()` returns
/// `(gestation / max_gestation).clamp(0, 1)`.
///
/// `effective_growth(scale)` returns `scale * gestation_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — gestates at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Womb {
    pub gestation: f32,
    pub max_gestation: f32,
    pub grow_rate: f32,
    pub just_born: bool,
    pub just_lost: bool,
    pub enabled: bool,
}

impl Womb {
    pub fn new(max_gestation: f32, grow_rate: f32) -> Self {
        Self {
            gestation: 0.0,
            max_gestation: max_gestation.max(0.1),
            grow_rate: grow_rate.max(0.0),
            just_born: false,
            just_lost: false,
            enabled: true,
        }
    }

    /// Add gestation; fires `just_born` when first reaching max.
    /// No-op when disabled.
    pub fn nurture(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.gestation < self.max_gestation;
        self.gestation = (self.gestation + amount).min(self.max_gestation);
        if was_below && self.gestation >= self.max_gestation {
            self.just_born = true;
        }
    }

    /// Reduce gestation; fires `just_lost` when reaching 0.
    /// No-op when disabled or already empty.
    pub fn wither(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.gestation <= 0.0 {
            return;
        }
        self.gestation = (self.gestation - amount).max(0.0);
        if self.gestation <= 0.0 {
            self.just_lost = true;
        }
    }

    /// Clear flags, then increase gestation by `grow_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_born = false;
        self.just_lost = false;
        if self.enabled && self.grow_rate > 0.0 && self.gestation < self.max_gestation {
            let was_below = self.gestation < self.max_gestation;
            self.gestation = (self.gestation + self.grow_rate * dt).min(self.max_gestation);
            if was_below && self.gestation >= self.max_gestation {
                self.just_born = true;
            }
        }
    }

    /// `true` when gestation is at maximum and component is enabled.
    pub fn is_born(&self) -> bool {
        self.gestation >= self.max_gestation && self.enabled
    }

    /// `true` when gestation is 0 (not gated by `enabled`).
    pub fn is_empty(&self) -> bool {
        self.gestation == 0.0
    }

    /// Fraction of maximum gestation [0.0, 1.0].
    pub fn gestation_fraction(&self) -> f32 {
        (self.gestation / self.max_gestation).clamp(0.0, 1.0)
    }

    /// Returns `scale * gestation_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_growth(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.gestation_fraction()
    }
}

impl Default for Womb {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Womb {
        Womb::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_empty() {
        let w = w();
        assert_eq!(w.gestation, 0.0);
        assert!(w.is_empty());
        assert!(!w.is_born());
    }

    #[test]
    fn new_clamps_max_gestation() {
        let w = Womb::new(-5.0, 1.5);
        assert!((w.max_gestation - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_grow_rate() {
        let w = Womb::new(100.0, -1.5);
        assert_eq!(w.grow_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Womb::default();
        assert!((w.max_gestation - 100.0).abs() < 1e-5);
        assert!((w.grow_rate - 1.5).abs() < 1e-5);
    }

    // --- nurture ---

    #[test]
    fn nurture_adds_gestation() {
        let mut w = w();
        w.nurture(40.0);
        assert!((w.gestation - 40.0).abs() < 1e-3);
    }

    #[test]
    fn nurture_clamps_at_max() {
        let mut w = w();
        w.nurture(200.0);
        assert!((w.gestation - 100.0).abs() < 1e-3);
    }

    #[test]
    fn nurture_fires_just_born_at_max() {
        let mut w = w();
        w.nurture(100.0);
        assert!(w.just_born);
        assert!(w.is_born());
    }

    #[test]
    fn nurture_no_just_born_when_already_at_max() {
        let mut w = w();
        w.gestation = 100.0;
        w.nurture(10.0);
        assert!(!w.just_born);
    }

    #[test]
    fn nurture_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.nurture(50.0);
        assert_eq!(w.gestation, 0.0);
    }

    #[test]
    fn nurture_no_op_when_amount_zero() {
        let mut w = w();
        w.nurture(0.0);
        assert_eq!(w.gestation, 0.0);
    }

    // --- wither ---

    #[test]
    fn wither_reduces_gestation() {
        let mut w = w();
        w.gestation = 60.0;
        w.wither(20.0);
        assert!((w.gestation - 40.0).abs() < 1e-3);
    }

    #[test]
    fn wither_clamps_at_zero() {
        let mut w = w();
        w.gestation = 30.0;
        w.wither(200.0);
        assert_eq!(w.gestation, 0.0);
    }

    #[test]
    fn wither_fires_just_lost_at_zero() {
        let mut w = w();
        w.gestation = 30.0;
        w.wither(30.0);
        assert!(w.just_lost);
    }

    #[test]
    fn wither_no_op_when_already_empty() {
        let mut w = w();
        w.wither(10.0);
        assert!(!w.just_lost);
    }

    #[test]
    fn wither_no_op_when_disabled() {
        let mut w = w();
        w.gestation = 50.0;
        w.enabled = false;
        w.wither(50.0);
        assert!((w.gestation - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_gestation() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.gestation - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_born_on_gestation_to_max() {
        let mut w = Womb::new(100.0, 200.0);
        w.gestation = 95.0;
        w.tick(1.0);
        assert!(w.just_born);
        assert!(w.is_born());
    }

    #[test]
    fn tick_no_build_when_already_born() {
        let mut w = w();
        w.gestation = 100.0;
        w.tick(1.0);
        assert!(!w.just_born);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Womb::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.gestation, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.gestation, 0.0);
    }

    #[test]
    fn tick_clears_just_born() {
        let mut w = Womb::new(100.0, 200.0);
        w.gestation = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_born);
    }

    #[test]
    fn tick_clears_just_lost() {
        let mut w = w();
        w.gestation = 10.0;
        w.wither(10.0);
        w.tick(0.016);
        assert!(!w.just_lost);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.gestation - 9.0).abs() < 1e-3);
    }

    // --- is_born / is_empty ---

    #[test]
    fn is_born_false_when_disabled() {
        let mut w = w();
        w.gestation = 100.0;
        w.enabled = false;
        assert!(!w.is_born());
    }

    #[test]
    fn is_empty_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_empty());
    }

    // --- gestation_fraction / effective_growth ---

    #[test]
    fn gestation_fraction_zero_when_empty() {
        assert_eq!(w().gestation_fraction(), 0.0);
    }

    #[test]
    fn gestation_fraction_half_at_midpoint() {
        let mut w = w();
        w.gestation = 50.0;
        assert!((w.gestation_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_growth_zero_when_empty() {
        assert_eq!(w().effective_growth(100.0), 0.0);
    }

    #[test]
    fn effective_growth_scales_with_gestation() {
        let mut w = w();
        w.gestation = 75.0;
        assert!((w.effective_growth(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_growth_zero_when_disabled() {
        let mut w = w();
        w.gestation = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_growth(100.0), 0.0);
    }
}
