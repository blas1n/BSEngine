use bevy_ecs::prelude::Component;

/// Fatigue-exhaustion accumulation tracker named after weary, the
/// adjective and verb meaning exhausted in strength, endurance,
/// or vigour; having one's patience, tolerance, or pleasure
/// exhausted (used with of); to make or become weary — from the
/// Old English wērig (weary, tired, exhausted), from the Proto-
/// Germanic werugaz, perhaps from the Proto-Indo-European root
/// wer- (to cover) or from a root related to the idea of
/// wandering until spent, though the exact origin is uncertain.
/// The word's semantic core is not acute pain but chronic
/// depletion — the fatigue that comes from effort sustained too
/// long, from resistance that has absorbed too many blows, from
/// patience extended past its natural limit until the reservoir is
/// simply empty. A person who is tired may recover after a night's
/// sleep; a person who is weary has been drained at a deeper level,
/// and recovery requires not just rest but a fundamental
/// replenishment of something that cannot be named precisely but
/// is felt as essential. The weariness of soldiers in a long
/// campaign, the weariness of caregivers in a prolonged illness,
/// the weariness of activists in a cause that seems never to end —
/// all share the structure of accumulated depletion: each effort
/// costs slightly more than it recovers, until the cumulative
/// deficit becomes insupportable. In game mechanics, a weary
/// mechanic models the slow accumulation of fatigue — the draining
/// of endurance, the erosion of will, the fill of exhaustion that
/// eventually reaches a threshold at which performance degrades,
/// collapse becomes imminent, or a rest state becomes mandatory.
/// `fatigue` builds via `tire(amount)` and accumulates passively
/// at `drain_rate` per second in `tick(dt)` or recovers via
/// `rest(amount)`.
///
/// Models fatigue-exhaustion fill levels, stamina-drain saturation
/// bars, endurance-depletion accumulators, weariness-build gauges,
/// morale-erosion fill levels, will-drain saturation indicators,
/// patience-depletion accumulation bars, sustained-effort meters,
/// attrition-fill levels, or any mechanic where a character,
/// unit, or system slowly accumulates the fatigue of sustained
/// effort until a threshold is crossed and the entity must stop,
/// rest, or suffer the consequences of having nothing left to give.
///
/// `tire(amount)` adds fatigue; fires `just_exhausted` when first
/// reaching `max_fatigue`. No-op when disabled.
///
/// `rest(amount)` reduces fatigue immediately; fires `just_refreshed`
/// when reaching 0. No-op when disabled or already fresh.
///
/// `tick(dt)` clears both flags, then increases fatigue by
/// `drain_rate * dt` (capped at `max_fatigue`). Fires
/// `just_exhausted` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_exhausted()` returns `fatigue >= max_fatigue && enabled`.
///
/// `is_fresh()` returns `fatigue == 0.0` (not gated by `enabled`).
///
/// `fatigue_fraction()` returns
/// `(fatigue / max_fatigue).clamp(0, 1)`.
///
/// `effective_fatigue(scale)` returns `scale * fatigue_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — drains at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Weary {
    pub fatigue: f32,
    pub max_fatigue: f32,
    pub drain_rate: f32,
    pub just_exhausted: bool,
    pub just_refreshed: bool,
    pub enabled: bool,
}

impl Weary {
    pub fn new(max_fatigue: f32, drain_rate: f32) -> Self {
        Self {
            fatigue: 0.0,
            max_fatigue: max_fatigue.max(0.1),
            drain_rate: drain_rate.max(0.0),
            just_exhausted: false,
            just_refreshed: false,
            enabled: true,
        }
    }

    /// Add fatigue; fires `just_exhausted` when first reaching max.
    /// No-op when disabled.
    pub fn tire(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.fatigue < self.max_fatigue;
        self.fatigue = (self.fatigue + amount).min(self.max_fatigue);
        if was_below && self.fatigue >= self.max_fatigue {
            self.just_exhausted = true;
        }
    }

    /// Reduce fatigue; fires `just_refreshed` when reaching 0.
    /// No-op when disabled or already fresh.
    pub fn rest(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.fatigue <= 0.0 {
            return;
        }
        self.fatigue = (self.fatigue - amount).max(0.0);
        if self.fatigue <= 0.0 {
            self.just_refreshed = true;
        }
    }

    /// Clear flags, then increase fatigue by `drain_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_exhausted = false;
        self.just_refreshed = false;
        if self.enabled && self.drain_rate > 0.0 && self.fatigue < self.max_fatigue {
            let was_below = self.fatigue < self.max_fatigue;
            self.fatigue = (self.fatigue + self.drain_rate * dt).min(self.max_fatigue);
            if was_below && self.fatigue >= self.max_fatigue {
                self.just_exhausted = true;
            }
        }
    }

    /// `true` when fatigue is at maximum and component is enabled.
    pub fn is_exhausted(&self) -> bool {
        self.fatigue >= self.max_fatigue && self.enabled
    }

    /// `true` when fatigue is 0 (not gated by `enabled`).
    pub fn is_fresh(&self) -> bool {
        self.fatigue == 0.0
    }

    /// Fraction of maximum fatigue [0.0, 1.0].
    pub fn fatigue_fraction(&self) -> f32 {
        (self.fatigue / self.max_fatigue).clamp(0.0, 1.0)
    }

    /// Returns `scale * fatigue_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_fatigue(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.fatigue_fraction()
    }
}

impl Default for Weary {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Weary {
        Weary::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_fresh() {
        let w = w();
        assert_eq!(w.fatigue, 0.0);
        assert!(w.is_fresh());
        assert!(!w.is_exhausted());
    }

    #[test]
    fn new_clamps_max_fatigue() {
        let w = Weary::new(-5.0, 1.5);
        assert!((w.max_fatigue - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_drain_rate() {
        let w = Weary::new(100.0, -1.5);
        assert_eq!(w.drain_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Weary::default();
        assert!((w.max_fatigue - 100.0).abs() < 1e-5);
        assert!((w.drain_rate - 1.5).abs() < 1e-5);
    }

    // --- tire ---

    #[test]
    fn tire_adds_fatigue() {
        let mut w = w();
        w.tire(40.0);
        assert!((w.fatigue - 40.0).abs() < 1e-3);
    }

    #[test]
    fn tire_clamps_at_max() {
        let mut w = w();
        w.tire(200.0);
        assert!((w.fatigue - 100.0).abs() < 1e-3);
    }

    #[test]
    fn tire_fires_just_exhausted_at_max() {
        let mut w = w();
        w.tire(100.0);
        assert!(w.just_exhausted);
        assert!(w.is_exhausted());
    }

    #[test]
    fn tire_no_just_exhausted_when_already_at_max() {
        let mut w = w();
        w.fatigue = 100.0;
        w.tire(10.0);
        assert!(!w.just_exhausted);
    }

    #[test]
    fn tire_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tire(50.0);
        assert_eq!(w.fatigue, 0.0);
    }

    #[test]
    fn tire_no_op_when_amount_zero() {
        let mut w = w();
        w.tire(0.0);
        assert_eq!(w.fatigue, 0.0);
    }

    // --- rest ---

    #[test]
    fn rest_reduces_fatigue() {
        let mut w = w();
        w.fatigue = 60.0;
        w.rest(20.0);
        assert!((w.fatigue - 40.0).abs() < 1e-3);
    }

    #[test]
    fn rest_clamps_at_zero() {
        let mut w = w();
        w.fatigue = 30.0;
        w.rest(200.0);
        assert_eq!(w.fatigue, 0.0);
    }

    #[test]
    fn rest_fires_just_refreshed_at_zero() {
        let mut w = w();
        w.fatigue = 30.0;
        w.rest(30.0);
        assert!(w.just_refreshed);
    }

    #[test]
    fn rest_no_op_when_already_fresh() {
        let mut w = w();
        w.rest(10.0);
        assert!(!w.just_refreshed);
    }

    #[test]
    fn rest_no_op_when_disabled() {
        let mut w = w();
        w.fatigue = 50.0;
        w.enabled = false;
        w.rest(50.0);
        assert!((w.fatigue - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_fatigue() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.fatigue - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_exhausted_on_fatigue_to_max() {
        let mut w = Weary::new(100.0, 200.0);
        w.fatigue = 95.0;
        w.tick(1.0);
        assert!(w.just_exhausted);
        assert!(w.is_exhausted());
    }

    #[test]
    fn tick_no_build_when_already_exhausted() {
        let mut w = w();
        w.fatigue = 100.0;
        w.tick(1.0);
        assert!(!w.just_exhausted);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Weary::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.fatigue, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.fatigue, 0.0);
    }

    #[test]
    fn tick_clears_just_exhausted() {
        let mut w = Weary::new(100.0, 200.0);
        w.fatigue = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_exhausted);
    }

    #[test]
    fn tick_clears_just_refreshed() {
        let mut w = w();
        w.fatigue = 10.0;
        w.rest(10.0);
        w.tick(0.016);
        assert!(!w.just_refreshed);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.fatigue - 9.0).abs() < 1e-3);
    }

    // --- is_exhausted / is_fresh ---

    #[test]
    fn is_exhausted_false_when_disabled() {
        let mut w = w();
        w.fatigue = 100.0;
        w.enabled = false;
        assert!(!w.is_exhausted());
    }

    #[test]
    fn is_fresh_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_fresh());
    }

    // --- fatigue_fraction / effective_fatigue ---

    #[test]
    fn fatigue_fraction_zero_when_fresh() {
        assert_eq!(w().fatigue_fraction(), 0.0);
    }

    #[test]
    fn fatigue_fraction_half_at_midpoint() {
        let mut w = w();
        w.fatigue = 50.0;
        assert!((w.fatigue_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_fatigue_zero_when_fresh() {
        assert_eq!(w().effective_fatigue(100.0), 0.0);
    }

    #[test]
    fn effective_fatigue_scales_with_fatigue() {
        let mut w = w();
        w.fatigue = 75.0;
        assert!((w.effective_fatigue(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_fatigue_zero_when_disabled() {
        let mut w = w();
        w.fatigue = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_fatigue(100.0), 0.0);
    }
}
