use bevy_ecs::prelude::Component;

/// Composure-based crowd-control resistance that degrades under sustained
/// pressure. The entity starts at full `composure` and is largely immune to
/// CC effects. Each crowd-control hit drains composure via `on_cc(weight)`;
/// when composure is exhausted the entity takes CC at full duration.
/// Composure slowly recovers via `tick(dt)`.
///
/// `on_cc(weight)` drains `composure` by `weight` (floored at 0.0). Fires
/// `just_rattled` on the first transition to 0. No-op when disabled or
/// `weight ≤ 0`.
///
/// `tick(dt)` clears one-frame flags first; restores `composure` by
/// `recovery_rate * dt` (capped at `max_composure`); fires `just_unshaken`
/// on the first tick that reaches full.
///
/// `composure_fraction()` returns `(composure / max_composure).clamp(0, 1)`.
///
/// `effective_cc_duration(base)` returns
/// `base * (1.0 - composure_fraction())` when enabled:
/// full composure → CC duration ≈ 0 (near-immune); zero composure →
/// full base duration. Returns `base` when disabled.
///
/// `is_rattled()` returns `composure == 0.0 && enabled`.
///
/// Distinct from `Immune` (flat, permanent CC immunity toggle), `Nullify`
/// (single-use negation), and `Invincible` (damage invulnerability): Faze
/// is a **degrading composure-resistance meter** — the entity shrugs off
/// the first CC attempts and only succumbs after sustained pressure; a
/// brief lull lets composure recover before the next assault.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Faze {
    /// Current composure level [0.0, max_composure].
    pub composure: f32,
    /// Maximum composure. Clamped ≥ 1.0.
    pub max_composure: f32,
    /// Composure recovered per second. Clamped ≥ 0.0.
    pub recovery_rate: f32,
    pub just_rattled: bool,
    pub just_unshaken: bool,
    pub enabled: bool,
}

impl Faze {
    pub fn new(max_composure: f32, recovery_rate: f32) -> Self {
        let max_composure = max_composure.max(1.0);
        Self {
            composure: max_composure,
            max_composure,
            recovery_rate: recovery_rate.max(0.0),
            just_rattled: false,
            just_unshaken: false,
            enabled: true,
        }
    }

    /// Apply crowd-control pressure. Drains `composure` by `weight` (floored
    /// at 0.0). Fires `just_rattled` on the first transition to 0. No-op
    /// when disabled or `weight ≤ 0`.
    pub fn on_cc(&mut self, weight: f32) {
        if !self.enabled || weight <= 0.0 {
            return;
        }
        let was_above_zero = self.composure > 0.0;
        self.composure = (self.composure - weight).max(0.0);
        if was_above_zero && self.composure == 0.0 {
            self.just_rattled = true;
        }
    }

    /// Recover composure over time. Clears `just_rattled` and
    /// `just_unshaken` first; restores `composure` by `recovery_rate * dt`
    /// (capped at `max_composure`); fires `just_unshaken` on the first tick
    /// that reaches full.
    pub fn tick(&mut self, dt: f32) {
        self.just_rattled = false;
        self.just_unshaken = false;

        if self.recovery_rate > 0.0 && self.composure < self.max_composure {
            let was_below_max = self.composure < self.max_composure;
            self.composure = (self.composure + self.recovery_rate * dt).min(self.max_composure);
            if was_below_max && self.composure >= self.max_composure {
                self.just_unshaken = true;
            }
        }
    }

    /// `true` when composure is fully exhausted and the component is enabled.
    pub fn is_rattled(&self) -> bool {
        self.composure == 0.0 && self.enabled
    }

    /// Composure fill fraction [0.0 = exhausted, 1.0 = full]. Always in [0, 1].
    pub fn composure_fraction(&self) -> f32 {
        (self.composure / self.max_composure).clamp(0.0, 1.0)
    }

    /// Effective CC duration shortened by current composure. Returns
    /// `base * (1.0 - composure_fraction())` when enabled — at full
    /// composure the entity is near-immune; at zero it takes the full
    /// duration. Returns `base` when disabled.
    pub fn effective_cc_duration(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 - self.composure_fraction())
    }
}

impl Default for Faze {
    fn default() -> Self {
        Self::new(10.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_full_composure() {
        let f = Faze::new(10.0, 2.0);
        assert!((f.composure - 10.0).abs() < 1e-5);
        assert!((f.composure_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn on_cc_drains_composure() {
        let mut f = Faze::new(10.0, 2.0);
        f.on_cc(3.0);
        assert!((f.composure - 7.0).abs() < 1e-5);
    }

    #[test]
    fn on_cc_floors_at_zero() {
        let mut f = Faze::new(5.0, 1.0);
        f.on_cc(100.0);
        assert_eq!(f.composure, 0.0);
    }

    #[test]
    fn on_cc_fires_just_rattled_on_first_depletion() {
        let mut f = Faze::new(5.0, 1.0);
        f.on_cc(5.0);
        assert!(f.just_rattled);
        assert!(f.is_rattled());
    }

    #[test]
    fn on_cc_no_just_rattled_when_already_rattled() {
        let mut f = Faze::new(5.0, 1.0);
        f.on_cc(5.0); // rattled
        f.tick(0.0); // clears flag
        f.on_cc(1.0); // still at 0
        assert!(!f.just_rattled);
    }

    #[test]
    fn on_cc_no_op_when_disabled() {
        let mut f = Faze::new(10.0, 2.0);
        f.enabled = false;
        f.on_cc(5.0);
        assert!((f.composure - 10.0).abs() < 1e-5);
    }

    #[test]
    fn on_cc_no_op_when_weight_zero() {
        let mut f = Faze::new(10.0, 2.0);
        f.on_cc(0.0);
        assert!((f.composure - 10.0).abs() < 1e-5);
    }

    #[test]
    fn on_cc_no_op_when_weight_negative() {
        let mut f = Faze::new(10.0, 2.0);
        f.on_cc(-5.0);
        assert!((f.composure - 10.0).abs() < 1e-5);
    }

    #[test]
    fn tick_restores_composure() {
        let mut f = Faze::new(10.0, 2.0);
        f.on_cc(4.0); // composure = 6
        f.tick(1.0); // 6 + 2 = 8
        assert!((f.composure - 8.0).abs() < 1e-5);
    }

    #[test]
    fn tick_caps_at_max_composure() {
        let mut f = Faze::new(10.0, 5.0);
        f.on_cc(2.0); // composure = 8
        f.tick(10.0); // would overshoot; caps at 10
        assert!((f.composure - 10.0).abs() < 1e-5);
    }

    #[test]
    fn tick_fires_just_unshaken_on_full_recovery() {
        let mut f = Faze::new(5.0, 5.0);
        f.on_cc(5.0); // drained
        f.tick(0.0); // clears flags
        f.tick(1.0); // recovers fully
        assert!(f.just_unshaken);
    }

    #[test]
    fn tick_no_just_unshaken_when_already_full() {
        let mut f = Faze::new(10.0, 2.0);
        f.tick(5.0); // already at max
        assert!(!f.just_unshaken);
    }

    #[test]
    fn tick_clears_just_rattled() {
        let mut f = Faze::new(5.0, 1.0);
        f.on_cc(5.0); // just_rattled = true
        f.tick(0.5);
        assert!(!f.just_rattled);
    }

    #[test]
    fn tick_clears_just_unshaken() {
        let mut f = Faze::new(5.0, 5.0);
        f.on_cc(5.0);
        f.tick(0.0);
        f.tick(1.0); // just_unshaken = true
        f.tick(0.016); // cleared
        assert!(!f.just_unshaken);
    }

    #[test]
    fn is_rattled_false_when_composure_above_zero() {
        let mut f = Faze::new(10.0, 2.0);
        f.on_cc(5.0);
        assert!(!f.is_rattled());
    }

    #[test]
    fn is_rattled_false_when_disabled() {
        let mut f = Faze::new(5.0, 1.0);
        f.composure = 0.0;
        f.enabled = false;
        assert!(!f.is_rattled());
    }

    #[test]
    fn composure_fraction_at_full() {
        let f = Faze::new(10.0, 2.0);
        assert!((f.composure_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn composure_fraction_at_half() {
        let mut f = Faze::new(10.0, 2.0);
        f.on_cc(5.0);
        assert!((f.composure_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn composure_fraction_at_zero() {
        let mut f = Faze::new(5.0, 1.0);
        f.on_cc(5.0);
        assert_eq!(f.composure_fraction(), 0.0);
    }

    #[test]
    fn effective_cc_duration_zero_at_full_composure() {
        let f = Faze::new(10.0, 2.0);
        // fraction = 1.0 → 1.0 - 1.0 = 0
        assert!((f.effective_cc_duration(5.0)).abs() < 1e-5);
    }

    #[test]
    fn effective_cc_duration_base_at_zero_composure() {
        let mut f = Faze::new(5.0, 1.0);
        f.on_cc(5.0);
        // fraction = 0.0 → 1.0 - 0.0 = 1.0 → base unchanged
        assert!((f.effective_cc_duration(3.0) - 3.0).abs() < 1e-5);
    }

    #[test]
    fn effective_cc_duration_half_at_half_composure() {
        let mut f = Faze::new(10.0, 2.0);
        f.on_cc(5.0); // fraction = 0.5 → 1 - 0.5 = 0.5
                      // 4.0 * 0.5 = 2.0
        assert!((f.effective_cc_duration(4.0) - 2.0).abs() < 1e-3);
    }

    #[test]
    fn effective_cc_duration_base_when_disabled() {
        let f = Faze::new(10.0, 2.0);
        let mut f = Faze {
            enabled: false,
            ..f
        };
        assert!((f.effective_cc_duration(5.0) - 5.0).abs() < 1e-5);
    }

    #[test]
    fn drain_and_recover_cycle() {
        let mut f = Faze::new(10.0, 2.0);
        f.on_cc(10.0); // fully drained
        assert!(f.is_rattled());
        f.tick(0.0); // clear flags
        f.tick(5.0); // restore 10 (2.0 * 5.0)
        assert!(!f.is_rattled());
        assert!((f.composure - 10.0).abs() < 1e-5);
    }

    #[test]
    fn just_rattled_fires_only_once_per_drain() {
        let mut f = Faze::new(5.0, 0.0);
        f.on_cc(3.0);
        f.on_cc(2.0); // hits zero — just_rattled
        f.tick(0.0); // cleared
        f.on_cc(1.0); // stays at 0, no new flag
        assert!(!f.just_rattled);
    }

    #[test]
    fn max_composure_clamped_to_one() {
        let f = Faze::new(0.0, 2.0);
        assert!((f.max_composure - 1.0).abs() < 1e-5);
        assert!((f.composure - 1.0).abs() < 1e-5);
    }

    #[test]
    fn recovery_rate_clamped_non_negative() {
        let f = Faze::new(10.0, -3.0);
        assert_eq!(f.recovery_rate, 0.0);
    }

    #[test]
    fn zero_recovery_rate_never_restores() {
        let mut f = Faze::new(10.0, 0.0);
        f.on_cc(5.0);
        f.tick(1000.0);
        assert!((f.composure - 5.0).abs() < 1e-5);
    }
}
