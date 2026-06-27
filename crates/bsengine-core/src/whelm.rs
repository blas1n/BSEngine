use bevy_ecs::prelude::Component;

/// Overwhelming-pressure accumulator. Models the build-up of crushing force
/// that erodes an entity's defenses as it accumulates.
///
/// `surge()` marks the entity as under active overwhelming force; no-op if
/// already surging or disabled.
///
/// `subside()` ends the active surge; no-op if not surging.
///
/// `tick(dt)` clears both one-frame flags first, then:
/// - If `whelming`: increases `whelm_level` by `surge_rate * dt` (capped at
///   `max_whelm`); fires `just_overwhelmed` the first time it reaches the cap.
/// - If `!whelming` and `whelm_level > 0`: decays by `decay_rate * dt`
///   (floored at 0); fires `just_cleared` the first time it reaches 0.
/// - No-op when disabled (flags are still cleared).
///
/// `is_overwhelmed()` returns `whelm_level >= max_whelm && enabled`.
///
/// `whelm_fraction()` returns `(whelm_level / max_whelm).clamp(0.0, 1.0)`.
///
/// `effective_resistance(base)` returns
/// `base * (1.0 - whelm_fraction())` when enabled (full whelm nullifies
/// resistance entirely); returns `base` unchanged otherwise.
///
/// Distinct from `Crush` (instantaneous force application), `Suppress`
/// (ability silencing), and `Stun` (brief incapacitation): Whelm models
/// **sustained overwhelming pressure** — defense erodes gradually as pressure
/// builds and recovers gradually when the pressure relents.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Whelm {
    /// Current accumulated pressure [0.0, max_whelm].
    pub whelm_level: f32,
    /// Maximum pressure before overwhelmed. Clamped >= 1.0.
    pub max_whelm: f32,
    /// Pressure increase per second while surging. Clamped >= 0.0.
    pub surge_rate: f32,
    /// Pressure decrease per second while not surging. Clamped >= 0.0.
    pub decay_rate: f32,
    pub whelming: bool,
    pub just_overwhelmed: bool,
    pub just_cleared: bool,
    pub enabled: bool,
}

impl Whelm {
    pub fn new(max_whelm: f32, surge_rate: f32, decay_rate: f32) -> Self {
        Self {
            whelm_level: 0.0,
            max_whelm: max_whelm.max(1.0),
            surge_rate: surge_rate.max(0.0),
            decay_rate: decay_rate.max(0.0),
            whelming: false,
            just_overwhelmed: false,
            just_cleared: false,
            enabled: true,
        }
    }

    /// Begin surging. No-op if already whelming or disabled.
    pub fn surge(&mut self) {
        if !self.enabled || self.whelming {
            return;
        }
        self.whelming = true;
    }

    /// End surging. No-op if not currently whelming.
    pub fn subside(&mut self) {
        if !self.whelming {
            return;
        }
        self.whelming = false;
    }

    /// Advance one frame: clear flags, then build or decay pressure.
    /// No-op (beyond flag clear) when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_overwhelmed = false;
        self.just_cleared = false;

        if !self.enabled {
            return;
        }

        if self.whelming {
            let was_below = self.whelm_level < self.max_whelm;
            self.whelm_level = (self.whelm_level + self.surge_rate * dt).min(self.max_whelm);
            if was_below && self.whelm_level >= self.max_whelm {
                self.just_overwhelmed = true;
            }
        } else if self.whelm_level > 0.0 {
            let was_above = self.whelm_level > 0.0;
            self.whelm_level = (self.whelm_level - self.decay_rate * dt).max(0.0);
            if was_above && self.whelm_level <= 0.0 {
                self.just_cleared = true;
            }
        }
    }

    /// `true` when pressure is at maximum and component is enabled.
    pub fn is_overwhelmed(&self) -> bool {
        self.whelm_level >= self.max_whelm && self.enabled
    }

    /// Pressure as a fraction of maximum [0.0, 1.0].
    pub fn whelm_fraction(&self) -> f32 {
        (self.whelm_level / self.max_whelm).clamp(0.0, 1.0)
    }

    /// Scale `base` resistance down by pressure fraction. Returns
    /// `base * (1.0 - fraction)` when enabled; `base` otherwise.
    pub fn effective_resistance(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 - self.whelm_fraction())
    }
}

impl Default for Whelm {
    fn default() -> Self {
        Self::new(10.0, 4.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Whelm {
        Whelm::new(10.0, 4.0, 2.0)
    }

    #[test]
    fn new_starts_idle() {
        let w = w();
        assert_eq!(w.whelm_level, 0.0);
        assert!(!w.whelming);
        assert!(!w.just_overwhelmed);
        assert!(!w.just_cleared);
        assert!(!w.is_overwhelmed());
    }

    #[test]
    fn surge_sets_whelming() {
        let mut w = w();
        w.surge();
        assert!(w.whelming);
    }

    #[test]
    fn surge_no_op_when_already_whelming() {
        let mut w = w();
        w.surge();
        w.tick(0.5); // 2.0
        w.surge(); // should not re-trigger
        assert!(w.whelming);
        assert!((w.whelm_level - 2.0).abs() < 1e-4);
    }

    #[test]
    fn surge_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.surge();
        assert!(!w.whelming);
    }

    #[test]
    fn subside_clears_whelming() {
        let mut w = w();
        w.surge();
        w.subside();
        assert!(!w.whelming);
    }

    #[test]
    fn subside_no_op_when_not_whelming() {
        let mut w = w();
        w.subside(); // no panic, no state change
        assert!(!w.whelming);
    }

    #[test]
    fn tick_builds_pressure_while_surging() {
        let mut w = w(); // surge_rate=4.0
        w.surge();
        w.tick(1.0); // 4.0
        assert!((w.whelm_level - 4.0).abs() < 1e-4);
    }

    #[test]
    fn tick_caps_at_max() {
        let mut w = w();
        w.surge();
        w.tick(100.0); // capped at 10
        assert!((w.whelm_level - 10.0).abs() < 1e-4);
    }

    #[test]
    fn tick_no_build_without_surge() {
        let mut w = w();
        w.tick(5.0);
        assert_eq!(w.whelm_level, 0.0);
    }

    #[test]
    fn tick_decays_pressure_after_subside() {
        let mut w = w(); // decay_rate=2.0
        w.surge();
        w.tick(2.0); // 8.0
        w.subside();
        w.tick(1.0); // 8.0 - 2.0 = 6.0
        assert!((w.whelm_level - 6.0).abs() < 1e-4);
    }

    #[test]
    fn tick_floors_decay_at_zero() {
        let mut w = w();
        w.surge();
        w.tick(1.0); // 4.0
        w.subside();
        w.tick(100.0); // floors at 0
        assert_eq!(w.whelm_level, 0.0);
    }

    #[test]
    fn tick_no_op_when_disabled_no_build() {
        let mut w = w();
        w.surge();
        w.enabled = false;
        w.tick(5.0);
        assert_eq!(w.whelm_level, 0.0);
    }

    #[test]
    fn tick_no_op_when_disabled_no_decay() {
        let mut w = w();
        w.surge();
        w.tick(2.0); // 8.0
        w.enabled = false;
        w.tick(5.0); // no decay
        assert!((w.whelm_level - 8.0).abs() < 1e-4);
    }

    #[test]
    fn tick_clears_flags_even_when_disabled() {
        let mut w = w();
        w.just_overwhelmed = true;
        w.just_cleared = true;
        w.enabled = false;
        w.tick(0.016);
        assert!(!w.just_overwhelmed);
        assert!(!w.just_cleared);
    }

    #[test]
    fn just_overwhelmed_fires_at_cap() {
        let mut w = w();
        w.surge();
        w.tick(2.5); // 4.0 * 2.5 = 10.0 → cap
        assert!(w.just_overwhelmed);
    }

    #[test]
    fn just_overwhelmed_clears_next_tick() {
        let mut w = w();
        w.surge();
        w.tick(3.0); // overwhelmed
        w.tick(0.016); // clears
        assert!(!w.just_overwhelmed);
    }

    #[test]
    fn just_overwhelmed_fires_only_once_at_cap() {
        let mut w = w();
        w.surge();
        w.tick(3.0); // overwhelmed
        w.tick(0.016); // already at max, clears
        assert!(!w.just_overwhelmed);
        w.tick(1.0); // still at max, no re-fire
        assert!(!w.just_overwhelmed);
    }

    #[test]
    fn just_cleared_fires_at_zero() {
        let mut w = w(); // decay_rate=2.0
        w.surge();
        w.tick(1.0); // 4.0
        w.subside();
        w.tick(2.0); // 4.0 - 4.0 = 0.0 → cleared
        assert!(w.just_cleared);
    }

    #[test]
    fn just_cleared_clears_next_tick() {
        let mut w = w();
        w.surge();
        w.tick(1.0); // 4.0
        w.subside();
        w.tick(2.0); // cleared
        w.tick(0.016); // clears flag
        assert!(!w.just_cleared);
    }

    #[test]
    fn just_cleared_fires_only_once_at_zero() {
        let mut w = w();
        w.surge();
        w.tick(1.0); // 4.0
        w.subside();
        w.tick(2.0); // 0.0 — fires
        w.tick(0.016); // cleared
        w.tick(1.0); // stays 0, no re-fire
        assert!(!w.just_cleared);
    }

    #[test]
    fn is_overwhelmed_true_at_max() {
        let mut w = w();
        w.surge();
        w.tick(100.0);
        assert!(w.is_overwhelmed());
    }

    #[test]
    fn is_overwhelmed_false_below_max() {
        let mut w = w();
        w.surge();
        w.tick(0.5); // 2.0 < 10.0
        assert!(!w.is_overwhelmed());
    }

    #[test]
    fn is_overwhelmed_false_when_disabled() {
        let mut w = w();
        w.surge();
        w.tick(100.0);
        w.enabled = false;
        assert!(!w.is_overwhelmed());
    }

    #[test]
    fn whelm_fraction_zero_when_empty() {
        let w = w();
        assert_eq!(w.whelm_fraction(), 0.0);
    }

    #[test]
    fn whelm_fraction_half_at_midpoint() {
        let mut w = w();
        w.whelm_level = 5.0;
        assert!((w.whelm_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn whelm_fraction_one_at_max() {
        let mut w = w();
        w.surge();
        w.tick(100.0);
        assert!((w.whelm_fraction() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn effective_resistance_full_at_empty() {
        let w = w(); // no pressure → full resistance
        assert!((w.effective_resistance(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_resistance_halved_at_half_pressure() {
        let mut w = w();
        w.whelm_level = 5.0; // fraction = 0.5
                             // 100 * (1 - 0.5) = 50
        assert!((w.effective_resistance(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_resistance_zero_at_full_pressure() {
        let mut w = w();
        w.surge();
        w.tick(100.0); // full whelm
                       // 100 * (1 - 1.0) = 0
        assert!((w.effective_resistance(100.0) - 0.0).abs() < 1e-3);
    }

    #[test]
    fn effective_resistance_passthrough_when_disabled() {
        let mut w = w();
        w.surge();
        w.tick(100.0);
        w.enabled = false;
        assert!((w.effective_resistance(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn max_whelm_clamped_to_one() {
        let w = Whelm::new(0.0, 4.0, 2.0);
        assert!((w.max_whelm - 1.0).abs() < 1e-5);
    }

    #[test]
    fn surge_rate_clamped_to_zero() {
        let w = Whelm::new(10.0, -5.0, 2.0);
        assert_eq!(w.surge_rate, 0.0);
    }

    #[test]
    fn decay_rate_clamped_to_zero() {
        let w = Whelm::new(10.0, 4.0, -1.0);
        assert_eq!(w.decay_rate, 0.0);
    }

    #[test]
    fn surge_subside_surge_cycle() {
        let mut w = w(); // surge_rate=4.0, decay_rate=2.0
        w.surge();
        w.tick(1.0); // 4.0
        w.subside();
        w.tick(1.0); // 4.0 - 2.0 = 2.0
        w.surge();
        w.tick(1.0); // 2.0 + 4.0 = 6.0
        assert!((w.whelm_level - 6.0).abs() < 1e-4);
    }
}
