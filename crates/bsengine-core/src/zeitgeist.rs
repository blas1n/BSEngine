use bevy_ecs::prelude::Component;

/// Cultural-momentum accumulator. `momentum` builds via `influence(amount)`
/// and fades passively at `fade_rate` per second in `tick(dt)` or
/// immediately via `dissipate(amount)`.
///
/// Models collective-consciousness intensity, cultural-trend meters, viral
/// spread pressure, era-dominance gauges, faction-popularity trackers, or
/// any mechanic where an idea or movement gathers momentum before eventually
/// fading when no longer actively reinforced.
///
/// `influence(amount)` adds momentum; fires `just_surged` when first
/// reaching `max_momentum`. No-op when disabled.
///
/// `dissipate(amount)` reduces momentum immediately; fires `just_faded`
/// when reaching 0. No-op when disabled or already faded.
///
/// `tick(dt)` clears both flags, then fades momentum by
/// `fade_rate * dt` (floored at 0). Fires `just_faded` when reaching 0
/// via passive fade. No-op when disabled or rate is 0.
///
/// `is_surging()` returns `momentum >= max_momentum && enabled`.
///
/// `is_faded()` returns `momentum == 0.0` (not gated by `enabled`).
///
/// `momentum_fraction()` returns `(momentum / max_momentum).clamp(0, 1)`.
///
/// `effective_influence(scale)` returns `scale * momentum_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 6.0)` — fades at 6 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zeitgeist {
    pub momentum: f32,
    pub max_momentum: f32,
    pub fade_rate: f32,
    pub just_surged: bool,
    pub just_faded: bool,
    pub enabled: bool,
}

impl Zeitgeist {
    pub fn new(max_momentum: f32, fade_rate: f32) -> Self {
        Self {
            momentum: 0.0,
            max_momentum: max_momentum.max(0.1),
            fade_rate: fade_rate.max(0.0),
            just_surged: false,
            just_faded: false,
            enabled: true,
        }
    }

    /// Add momentum; fires `just_surged` when first reaching max.
    /// No-op when disabled.
    pub fn influence(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.momentum < self.max_momentum;
        self.momentum = (self.momentum + amount).min(self.max_momentum);
        if was_below && self.momentum >= self.max_momentum {
            self.just_surged = true;
        }
    }

    /// Reduce momentum; fires `just_faded` when reaching 0.
    /// No-op when disabled or already faded.
    pub fn dissipate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.momentum <= 0.0 {
            return;
        }
        self.momentum = (self.momentum - amount).max(0.0);
        if self.momentum <= 0.0 {
            self.just_faded = true;
        }
    }

    /// Clear flags, then fade momentum by `fade_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_surged = false;
        self.just_faded = false;
        if self.enabled && self.fade_rate > 0.0 && self.momentum > 0.0 {
            self.momentum = (self.momentum - self.fade_rate * dt).max(0.0);
            if self.momentum <= 0.0 {
                self.just_faded = true;
            }
        }
    }

    /// `true` when momentum is at maximum and component is enabled.
    pub fn is_surging(&self) -> bool {
        self.momentum >= self.max_momentum && self.enabled
    }

    /// `true` when momentum is 0 (not gated by `enabled`).
    pub fn is_faded(&self) -> bool {
        self.momentum == 0.0
    }

    /// Fraction of maximum momentum [0.0, 1.0].
    pub fn momentum_fraction(&self) -> f32 {
        (self.momentum / self.max_momentum).clamp(0.0, 1.0)
    }

    /// Returns `scale * momentum_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_influence(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.momentum_fraction()
    }
}

impl Default for Zeitgeist {
    fn default() -> Self {
        Self::new(100.0, 6.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zeitgeist {
        Zeitgeist::new(100.0, 6.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_faded() {
        let z = z();
        assert_eq!(z.momentum, 0.0);
        assert!(z.is_faded());
        assert!(!z.is_surging());
    }

    #[test]
    fn new_clamps_max_momentum() {
        let z = Zeitgeist::new(-5.0, 6.0);
        assert!((z.max_momentum - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_fade_rate() {
        let z = Zeitgeist::new(100.0, -3.0);
        assert_eq!(z.fade_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zeitgeist::default();
        assert!((z.max_momentum - 100.0).abs() < 1e-5);
        assert!((z.fade_rate - 6.0).abs() < 1e-5);
    }

    // --- influence ---

    #[test]
    fn influence_adds_momentum() {
        let mut z = z();
        z.influence(40.0);
        assert!((z.momentum - 40.0).abs() < 1e-3);
    }

    #[test]
    fn influence_clamps_at_max() {
        let mut z = z();
        z.influence(200.0);
        assert!((z.momentum - 100.0).abs() < 1e-3);
    }

    #[test]
    fn influence_fires_just_surged_at_max() {
        let mut z = z();
        z.influence(100.0);
        assert!(z.just_surged);
        assert!(z.is_surging());
    }

    #[test]
    fn influence_no_just_surged_when_already_at_max() {
        let mut z = z();
        z.momentum = 100.0;
        z.influence(10.0);
        assert!(!z.just_surged);
    }

    #[test]
    fn influence_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.influence(50.0);
        assert_eq!(z.momentum, 0.0);
    }

    #[test]
    fn influence_no_op_when_amount_zero() {
        let mut z = z();
        z.influence(0.0);
        assert_eq!(z.momentum, 0.0);
    }

    // --- dissipate ---

    #[test]
    fn dissipate_reduces_momentum() {
        let mut z = z();
        z.momentum = 60.0;
        z.dissipate(20.0);
        assert!((z.momentum - 40.0).abs() < 1e-3);
    }

    #[test]
    fn dissipate_clamps_at_zero() {
        let mut z = z();
        z.momentum = 30.0;
        z.dissipate(200.0);
        assert_eq!(z.momentum, 0.0);
    }

    #[test]
    fn dissipate_fires_just_faded_at_zero() {
        let mut z = z();
        z.momentum = 30.0;
        z.dissipate(30.0);
        assert!(z.just_faded);
    }

    #[test]
    fn dissipate_no_op_when_already_faded() {
        let mut z = z();
        z.dissipate(10.0);
        assert!(!z.just_faded);
    }

    #[test]
    fn dissipate_no_op_when_disabled() {
        let mut z = z();
        z.momentum = 50.0;
        z.enabled = false;
        z.dissipate(50.0);
        assert!((z.momentum - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_fades_momentum() {
        let mut z = z(); // fade=6
        z.momentum = 60.0;
        z.tick(1.0); // 60 - 6 = 54
        assert!((z.momentum - 54.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_faded_on_fade_to_zero() {
        let mut z = Zeitgeist::new(100.0, 200.0);
        z.momentum = 5.0;
        z.tick(1.0);
        assert!(z.just_faded);
        assert!(z.is_faded());
    }

    #[test]
    fn tick_no_fade_when_already_faded() {
        let mut z = z();
        z.tick(10.0);
        assert!(!z.just_faded);
    }

    #[test]
    fn tick_no_fade_when_rate_zero() {
        let mut z = Zeitgeist::new(100.0, 0.0);
        z.momentum = 50.0;
        z.tick(100.0);
        assert!((z.momentum - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_fade_when_disabled() {
        let mut z = z();
        z.momentum = 50.0;
        z.enabled = false;
        z.tick(1.0);
        assert!((z.momentum - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_surged() {
        let mut z = z();
        z.influence(100.0);
        z.tick(0.016);
        assert!(!z.just_surged);
    }

    #[test]
    fn tick_clears_just_faded() {
        let mut z = Zeitgeist::new(100.0, 200.0);
        z.momentum = 5.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_faded);
    }

    #[test]
    fn tick_scales_fade_with_dt() {
        let mut z = z(); // fade=6
        z.momentum = 100.0;
        z.tick(4.0); // 100 - 6*4 = 76
        assert!((z.momentum - 76.0).abs() < 1e-3);
    }

    // --- is_surging / is_faded ---

    #[test]
    fn is_surging_false_when_disabled() {
        let mut z = z();
        z.momentum = 100.0;
        z.enabled = false;
        assert!(!z.is_surging());
    }

    #[test]
    fn is_faded_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_faded());
    }

    // --- momentum_fraction / effective_influence ---

    #[test]
    fn momentum_fraction_zero_when_faded() {
        assert_eq!(z().momentum_fraction(), 0.0);
    }

    #[test]
    fn momentum_fraction_half_at_midpoint() {
        let mut z = z();
        z.momentum = 50.0;
        assert!((z.momentum_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_influence_zero_when_faded() {
        assert_eq!(z().effective_influence(100.0), 0.0);
    }

    #[test]
    fn effective_influence_scales_with_momentum() {
        let mut z = z();
        z.momentum = 65.0;
        assert!((z.effective_influence(100.0) - 65.0).abs() < 1e-3);
    }

    #[test]
    fn effective_influence_zero_when_disabled() {
        let mut z = z();
        z.momentum = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_influence(100.0), 0.0);
    }
}
