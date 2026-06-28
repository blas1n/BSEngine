use bevy_ecs::prelude::Component;

/// Infestation-density accumulation tracker named after vermin, the
/// collective noun (plural in construction) for small common harmful
/// or objectionable animals that are difficult to control — rats,
/// mice, fleas, cockroaches, bedbugs, weevils, and the entire
/// teeming guild of creatures that follow human settlement into every
/// corner of the built environment, thriving in the heat and food
/// waste and structural complexity that civilisation provides. The
/// word entered English in the fourteenth century through Old French
/// vermin from Latin vermis (worm), and it has always carried a
/// strong affective charge: vermin are not merely small and harmful
/// but threatening, contaminating, prolific beyond control, and
/// intimately associated with disease, rotting food, and the failure
/// of the social order to maintain the boundary between the domestic
/// interior and the wild outside. Plague, historically, was synonymous
/// with vermin: rats carried the fleas that carried the bacterium;
/// rat populations exploded in cities where grain was stored and
/// rubbish accumulated; efforts to control plague were inseparable
/// from efforts to control rats. The association between vermin and
/// moral disorder runs deeper still: the language of vermin has
/// historically been deployed in the rhetoric of dehumanisation,
/// making it a word that must be handled with care even in its
/// purely ecological sense. In game mechanics, vermin provide a
/// clean model for a population that expands in the absence of active
/// suppression, reaches a threshold at which it causes catastrophic
/// damage, and can be reduced but never permanently eliminated because
/// the ecological conditions that sustain it persist. `infestation`
/// builds via `infest(amount)` and accumulates passively at
/// `swarm_rate` per second in `tick(dt)` or is reduced via
/// `exterminate(amount)`.
///
/// Models infestation-density fill levels, pest-population saturation
/// bars, rodent-encroachment accumulators, insect-colony growth
/// gauges, plague-vector fill levels, parasite-load saturation
/// indicators, infestation-spread accumulation bars, blight-density
/// meters, contamination-source fill levels, or any mechanic where
/// a pest population slowly fills a space — crevice by crevice,
/// grain-sack by grain-sack — until the threshold is crossed and
/// the infestation becomes undeniable, triggering quarantines,
/// evacuations, or emergency extermination campaigns whose partial
/// success only buys time before the population rebounds.
///
/// `infest(amount)` adds infestation; fires `just_swarmed` when first
/// reaching `max_infestation`. No-op when disabled.
///
/// `exterminate(amount)` reduces infestation immediately; fires
/// `just_exterminated` when reaching 0. No-op when disabled or
/// already exterminated.
///
/// `tick(dt)` clears both flags, then increases infestation by
/// `swarm_rate * dt` (capped at `max_infestation`). Fires
/// `just_swarmed` when first reaching max. No-op when disabled or
/// rate is 0.
///
/// `is_swarmed()` returns `infestation >= max_infestation && enabled`.
///
/// `is_exterminated()` returns `infestation == 0.0` (not gated by
/// `enabled`).
///
/// `infestation_fraction()` returns
/// `(infestation / max_infestation).clamp(0, 1)`.
///
/// `effective_blight(scale)` returns `scale * infestation_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — swarms at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Vermin {
    pub infestation: f32,
    pub max_infestation: f32,
    pub swarm_rate: f32,
    pub just_swarmed: bool,
    pub just_exterminated: bool,
    pub enabled: bool,
}

impl Vermin {
    pub fn new(max_infestation: f32, swarm_rate: f32) -> Self {
        Self {
            infestation: 0.0,
            max_infestation: max_infestation.max(0.1),
            swarm_rate: swarm_rate.max(0.0),
            just_swarmed: false,
            just_exterminated: false,
            enabled: true,
        }
    }

    /// Add infestation; fires `just_swarmed` when first reaching max.
    /// No-op when disabled.
    pub fn infest(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.infestation < self.max_infestation;
        self.infestation = (self.infestation + amount).min(self.max_infestation);
        if was_below && self.infestation >= self.max_infestation {
            self.just_swarmed = true;
        }
    }

    /// Reduce infestation; fires `just_exterminated` when reaching 0.
    /// No-op when disabled or already exterminated.
    pub fn exterminate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.infestation <= 0.0 {
            return;
        }
        self.infestation = (self.infestation - amount).max(0.0);
        if self.infestation <= 0.0 {
            self.just_exterminated = true;
        }
    }

    /// Clear flags, then increase infestation by `swarm_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_swarmed = false;
        self.just_exterminated = false;
        if self.enabled && self.swarm_rate > 0.0 && self.infestation < self.max_infestation {
            let was_below = self.infestation < self.max_infestation;
            self.infestation = (self.infestation + self.swarm_rate * dt).min(self.max_infestation);
            if was_below && self.infestation >= self.max_infestation {
                self.just_swarmed = true;
            }
        }
    }

    /// `true` when infestation is at maximum and component is enabled.
    pub fn is_swarmed(&self) -> bool {
        self.infestation >= self.max_infestation && self.enabled
    }

    /// `true` when infestation is 0 (not gated by `enabled`).
    pub fn is_exterminated(&self) -> bool {
        self.infestation == 0.0
    }

    /// Fraction of maximum infestation [0.0, 1.0].
    pub fn infestation_fraction(&self) -> f32 {
        (self.infestation / self.max_infestation).clamp(0.0, 1.0)
    }

    /// Returns `scale * infestation_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_blight(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.infestation_fraction()
    }
}

impl Default for Vermin {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v() -> Vermin {
        Vermin::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_exterminated() {
        let v = v();
        assert_eq!(v.infestation, 0.0);
        assert!(v.is_exterminated());
        assert!(!v.is_swarmed());
    }

    #[test]
    fn new_clamps_max_infestation() {
        let v = Vermin::new(-5.0, 1.5);
        assert!((v.max_infestation - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_swarm_rate() {
        let v = Vermin::new(100.0, -1.5);
        assert_eq!(v.swarm_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let v = Vermin::default();
        assert!((v.max_infestation - 100.0).abs() < 1e-5);
        assert!((v.swarm_rate - 1.5).abs() < 1e-5);
    }

    // --- infest ---

    #[test]
    fn infest_adds_infestation() {
        let mut v = v();
        v.infest(40.0);
        assert!((v.infestation - 40.0).abs() < 1e-3);
    }

    #[test]
    fn infest_clamps_at_max() {
        let mut v = v();
        v.infest(200.0);
        assert!((v.infestation - 100.0).abs() < 1e-3);
    }

    #[test]
    fn infest_fires_just_swarmed_at_max() {
        let mut v = v();
        v.infest(100.0);
        assert!(v.just_swarmed);
        assert!(v.is_swarmed());
    }

    #[test]
    fn infest_no_just_swarmed_when_already_at_max() {
        let mut v = v();
        v.infestation = 100.0;
        v.infest(10.0);
        assert!(!v.just_swarmed);
    }

    #[test]
    fn infest_no_op_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.infest(50.0);
        assert_eq!(v.infestation, 0.0);
    }

    #[test]
    fn infest_no_op_when_amount_zero() {
        let mut v = v();
        v.infest(0.0);
        assert_eq!(v.infestation, 0.0);
    }

    // --- exterminate ---

    #[test]
    fn exterminate_reduces_infestation() {
        let mut v = v();
        v.infestation = 60.0;
        v.exterminate(20.0);
        assert!((v.infestation - 40.0).abs() < 1e-3);
    }

    #[test]
    fn exterminate_clamps_at_zero() {
        let mut v = v();
        v.infestation = 30.0;
        v.exterminate(200.0);
        assert_eq!(v.infestation, 0.0);
    }

    #[test]
    fn exterminate_fires_just_exterminated_at_zero() {
        let mut v = v();
        v.infestation = 30.0;
        v.exterminate(30.0);
        assert!(v.just_exterminated);
    }

    #[test]
    fn exterminate_no_op_when_already_exterminated() {
        let mut v = v();
        v.exterminate(10.0);
        assert!(!v.just_exterminated);
    }

    #[test]
    fn exterminate_no_op_when_disabled() {
        let mut v = v();
        v.infestation = 50.0;
        v.enabled = false;
        v.exterminate(50.0);
        assert!((v.infestation - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_spreads_infestation() {
        let mut v = v(); // rate=1.5
        v.tick(4.0); // 0 + 1.5*4 = 6
        assert!((v.infestation - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_swarmed_on_infestation_to_max() {
        let mut v = Vermin::new(100.0, 200.0);
        v.infestation = 95.0;
        v.tick(1.0);
        assert!(v.just_swarmed);
        assert!(v.is_swarmed());
    }

    #[test]
    fn tick_no_spread_when_already_swarmed() {
        let mut v = v();
        v.infestation = 100.0;
        v.tick(1.0);
        assert!(!v.just_swarmed);
    }

    #[test]
    fn tick_no_spread_when_rate_zero() {
        let mut v = Vermin::new(100.0, 0.0);
        v.tick(100.0);
        assert_eq!(v.infestation, 0.0);
    }

    #[test]
    fn tick_no_spread_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.tick(1.0);
        assert_eq!(v.infestation, 0.0);
    }

    #[test]
    fn tick_clears_just_swarmed() {
        let mut v = Vermin::new(100.0, 200.0);
        v.infestation = 95.0;
        v.tick(1.0);
        v.tick(0.016);
        assert!(!v.just_swarmed);
    }

    #[test]
    fn tick_clears_just_exterminated() {
        let mut v = v();
        v.infestation = 10.0;
        v.exterminate(10.0);
        v.tick(0.016);
        assert!(!v.just_exterminated);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut v = v(); // rate=1.5
        v.tick(6.0); // 1.5*6 = 9
        assert!((v.infestation - 9.0).abs() < 1e-3);
    }

    // --- is_swarmed / is_exterminated ---

    #[test]
    fn is_swarmed_false_when_disabled() {
        let mut v = v();
        v.infestation = 100.0;
        v.enabled = false;
        assert!(!v.is_swarmed());
    }

    #[test]
    fn is_exterminated_not_gated_by_enabled() {
        let mut v = v();
        v.enabled = false;
        assert!(v.is_exterminated());
    }

    // --- infestation_fraction / effective_blight ---

    #[test]
    fn infestation_fraction_zero_when_exterminated() {
        assert_eq!(v().infestation_fraction(), 0.0);
    }

    #[test]
    fn infestation_fraction_half_at_midpoint() {
        let mut v = v();
        v.infestation = 50.0;
        assert!((v.infestation_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_blight_zero_when_exterminated() {
        assert_eq!(v().effective_blight(100.0), 0.0);
    }

    #[test]
    fn effective_blight_scales_with_infestation() {
        let mut v = v();
        v.infestation = 75.0;
        assert!((v.effective_blight(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_blight_zero_when_disabled() {
        let mut v = v();
        v.infestation = 50.0;
        v.enabled = false;
        assert_eq!(v.effective_blight(100.0), 0.0);
    }
}
