use bevy_ecs::prelude::Component;

/// Seize-force accumulator. Models the build-up of forceful intent needed to
/// wrench control away from an opponent — a weapon, position, or advantage.
///
/// `strain()` begins applying seize force; no-op if already straining or
/// disabled.
///
/// `ease()` stops straining; no-op if not currently straining.
///
/// `tick(dt)` clears both one-frame flags first, then:
/// - If `straining`: increases `wrest_level` by `strain_rate * dt` (capped
///   at `max_wrest`); fires `just_seized` the first time it reaches the cap.
/// - If `!straining` and `wrest_level > 0`: decays by `ease_rate * dt`
///   (floored at 0); fires `just_released` the first time it reaches 0.
/// - No-op when disabled (flags are still cleared).
///
/// `is_seized()` returns `wrest_level >= max_wrest && enabled`.
///
/// `seize_fraction()` returns `(wrest_level / max_wrest).clamp(0.0, 1.0)`.
///
/// `effective_force(base)` returns `base * (1.0 + seize_fraction())` when
/// enabled — force scales with accumulated seize level; returns `base`
/// unchanged when disabled.
///
/// Distinct from `Grab` (positional hold), `Grapple` (mutual contest), and
/// `Disarm` (instant item loss): Wrest models **escalating seize pressure** —
/// force accumulates over time as the entity strains and dissipates when it
/// eases off, creating a build-or-lose-it commitment mechanic.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wrest {
    /// Current seize-force level [0.0, max_wrest].
    pub wrest_level: f32,
    /// Maximum seize force. Clamped >= 1.0.
    pub max_wrest: f32,
    /// Force gain per second while straining. Clamped >= 0.0.
    pub strain_rate: f32,
    /// Force loss per second while easing. Clamped >= 0.0.
    pub ease_rate: f32,
    pub straining: bool,
    pub just_seized: bool,
    pub just_released: bool,
    pub enabled: bool,
}

impl Wrest {
    pub fn new(max_wrest: f32, strain_rate: f32, ease_rate: f32) -> Self {
        Self {
            wrest_level: 0.0,
            max_wrest: max_wrest.max(1.0),
            strain_rate: strain_rate.max(0.0),
            ease_rate: ease_rate.max(0.0),
            straining: false,
            just_seized: false,
            just_released: false,
            enabled: true,
        }
    }

    /// Begin straining. No-op if already straining or disabled.
    pub fn strain(&mut self) {
        if !self.enabled || self.straining {
            return;
        }
        self.straining = true;
    }

    /// Stop straining. No-op if not currently straining.
    pub fn ease(&mut self) {
        if !self.straining {
            return;
        }
        self.straining = false;
    }

    /// Advance one frame: clear flags, then build or decay force.
    /// No-op (beyond flag clear) when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_seized = false;
        self.just_released = false;

        if !self.enabled {
            return;
        }

        if self.straining {
            let was_below = self.wrest_level < self.max_wrest;
            self.wrest_level = (self.wrest_level + self.strain_rate * dt).min(self.max_wrest);
            if was_below && self.wrest_level >= self.max_wrest {
                self.just_seized = true;
            }
        } else if self.wrest_level > 0.0 {
            let was_above = self.wrest_level > 0.0;
            self.wrest_level = (self.wrest_level - self.ease_rate * dt).max(0.0);
            if was_above && self.wrest_level <= 0.0 {
                self.just_released = true;
            }
        }
    }

    /// `true` when seize force is at maximum and component is enabled.
    pub fn is_seized(&self) -> bool {
        self.wrest_level >= self.max_wrest && self.enabled
    }

    /// Seize force as a fraction of maximum [0.0, 1.0].
    pub fn seize_fraction(&self) -> f32 {
        (self.wrest_level / self.max_wrest).clamp(0.0, 1.0)
    }

    /// Scale `base` force by accumulated seize level. Returns
    /// `base * (1.0 + fraction)` when enabled; `base` otherwise.
    pub fn effective_force(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.seize_fraction())
    }
}

impl Default for Wrest {
    fn default() -> Self {
        Self::new(10.0, 5.0, 3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wrest {
        Wrest::new(10.0, 5.0, 3.0)
    }

    #[test]
    fn new_starts_idle() {
        let w = w();
        assert_eq!(w.wrest_level, 0.0);
        assert!(!w.straining);
        assert!(!w.just_seized);
        assert!(!w.just_released);
        assert!(!w.is_seized());
    }

    #[test]
    fn strain_sets_straining() {
        let mut w = w();
        w.strain();
        assert!(w.straining);
    }

    #[test]
    fn strain_no_op_when_already_straining() {
        let mut w = w();
        w.strain();
        w.tick(0.5); // 2.5
        w.strain();
        assert!(w.straining);
        assert!((w.wrest_level - 2.5).abs() < 1e-4);
    }

    #[test]
    fn strain_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.strain();
        assert!(!w.straining);
    }

    #[test]
    fn ease_clears_straining() {
        let mut w = w();
        w.strain();
        w.ease();
        assert!(!w.straining);
    }

    #[test]
    fn ease_no_op_when_not_straining() {
        let mut w = w();
        w.ease();
        assert!(!w.straining);
    }

    #[test]
    fn tick_builds_force_while_straining() {
        let mut w = w(); // strain_rate=5.0
        w.strain();
        w.tick(1.0); // 5.0
        assert!((w.wrest_level - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_caps_at_max() {
        let mut w = w();
        w.strain();
        w.tick(100.0); // capped at 10
        assert!((w.wrest_level - 10.0).abs() < 1e-4);
    }

    #[test]
    fn tick_no_build_without_strain() {
        let mut w = w();
        w.tick(5.0);
        assert_eq!(w.wrest_level, 0.0);
    }

    #[test]
    fn tick_decays_force_after_ease() {
        let mut w = w(); // ease_rate=3.0
        w.strain();
        w.tick(2.0); // 10.0
        w.ease();
        w.tick(1.0); // 10.0 - 3.0 = 7.0
        assert!((w.wrest_level - 7.0).abs() < 1e-4);
    }

    #[test]
    fn tick_floors_decay_at_zero() {
        let mut w = w();
        w.strain();
        w.tick(1.0); // 5.0
        w.ease();
        w.tick(100.0); // floors at 0
        assert_eq!(w.wrest_level, 0.0);
    }

    #[test]
    fn tick_no_op_when_disabled_no_build() {
        let mut w = w();
        w.strain();
        w.enabled = false;
        w.tick(5.0);
        assert_eq!(w.wrest_level, 0.0);
    }

    #[test]
    fn tick_no_op_when_disabled_no_decay() {
        let mut w = w();
        w.strain();
        w.tick(1.0); // 5.0
        w.enabled = false;
        w.tick(5.0);
        assert!((w.wrest_level - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_clears_flags_even_when_disabled() {
        let mut w = w();
        w.just_seized = true;
        w.just_released = true;
        w.enabled = false;
        w.tick(0.016);
        assert!(!w.just_seized);
        assert!(!w.just_released);
    }

    #[test]
    fn just_seized_fires_at_cap() {
        let mut w = w(); // strain_rate=5.0 → max in 2s
        w.strain();
        w.tick(2.0);
        assert!(w.just_seized);
    }

    #[test]
    fn just_seized_clears_next_tick() {
        let mut w = w();
        w.strain();
        w.tick(2.0);
        w.tick(0.016);
        assert!(!w.just_seized);
    }

    #[test]
    fn just_seized_fires_only_once_at_cap() {
        let mut w = w();
        w.strain();
        w.tick(2.0); // seized
        w.tick(0.016); // already at max, clears
        assert!(!w.just_seized);
        w.tick(1.0); // still at max, no re-fire
        assert!(!w.just_seized);
    }

    #[test]
    fn just_released_fires_at_zero() {
        let mut w = w(); // ease_rate=3.0
        w.strain();
        w.tick(1.0); // 5.0
        w.ease();
        w.tick(2.0); // 5.0 - 6.0 = 0.0 → released
        assert!(w.just_released);
    }

    #[test]
    fn just_released_clears_next_tick() {
        let mut w = w();
        w.strain();
        w.tick(1.0); // 5.0
        w.ease();
        w.tick(2.0); // 0.0 — released
        w.tick(0.016); // clears flag
        assert!(!w.just_released);
    }

    #[test]
    fn just_released_fires_only_once_at_zero() {
        let mut w = w();
        w.strain();
        w.tick(1.0); // 5.0
        w.ease();
        w.tick(2.0); // 0.0 — fires
        w.tick(0.016); // cleared
        w.tick(1.0); // stays 0, no re-fire
        assert!(!w.just_released);
    }

    #[test]
    fn is_seized_true_at_max() {
        let mut w = w();
        w.strain();
        w.tick(100.0);
        assert!(w.is_seized());
    }

    #[test]
    fn is_seized_false_below_max() {
        let mut w = w();
        w.strain();
        w.tick(0.5); // 2.5 < 10.0
        assert!(!w.is_seized());
    }

    #[test]
    fn is_seized_false_when_disabled() {
        let mut w = w();
        w.strain();
        w.tick(100.0);
        w.enabled = false;
        assert!(!w.is_seized());
    }

    #[test]
    fn seize_fraction_zero_when_empty() {
        let w = w();
        assert_eq!(w.seize_fraction(), 0.0);
    }

    #[test]
    fn seize_fraction_half_at_midpoint() {
        let mut w = w();
        w.wrest_level = 5.0;
        assert!((w.seize_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn seize_fraction_one_at_max() {
        let mut w = w();
        w.strain();
        w.tick(100.0);
        assert!((w.seize_fraction() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn effective_force_base_when_empty() {
        let w = w();
        assert!((w.effective_force(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_force_scaled_at_half_force() {
        let mut w = w();
        w.wrest_level = 5.0; // fraction = 0.5
                             // 100 * (1 + 0.5) = 150
        assert!((w.effective_force(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_force_doubled_at_full_force() {
        let mut w = w();
        w.strain();
        w.tick(100.0); // full seize
                       // 100 * (1 + 1.0) = 200
        assert!((w.effective_force(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn effective_force_passthrough_when_disabled() {
        let mut w = w();
        w.strain();
        w.tick(100.0);
        w.enabled = false;
        assert!((w.effective_force(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn max_wrest_clamped_to_one() {
        let w = Wrest::new(0.0, 5.0, 3.0);
        assert!((w.max_wrest - 1.0).abs() < 1e-5);
    }

    #[test]
    fn strain_rate_clamped_to_zero() {
        let w = Wrest::new(10.0, -5.0, 3.0);
        assert_eq!(w.strain_rate, 0.0);
    }

    #[test]
    fn ease_rate_clamped_to_zero() {
        let w = Wrest::new(10.0, 5.0, -3.0);
        assert_eq!(w.ease_rate, 0.0);
    }

    #[test]
    fn strain_ease_strain_cycle() {
        let mut w = w(); // strain_rate=5.0, ease_rate=3.0
        w.strain();
        w.tick(1.0); // 5.0
        w.ease();
        w.tick(1.0); // 5.0 - 3.0 = 2.0
        w.strain();
        w.tick(1.0); // 2.0 + 5.0 = 7.0
        assert!((w.wrest_level - 7.0).abs() < 1e-4);
    }
}
