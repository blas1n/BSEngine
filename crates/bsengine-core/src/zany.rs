use bevy_ecs::prelude::Component;

/// Whimsy/chaos tracker. `whimsy` builds via `spark(amount)` and fades
/// passively at `fade_rate` per second in `tick(dt)` or immediately via
/// `calm(amount)`.
///
/// Models unpredictability meters, trickster charge, chaotic-energy
/// gauges, clown-mode escalation, NPC erratic-behaviour bars, or any
/// mechanic where zaniness accumulates and deflates without active
/// reinforcement.
///
/// `spark(amount)` adds whimsy; fires `just_unhinged` when first reaching
/// `max_whimsy`. No-op when disabled.
///
/// `calm(amount)` reduces whimsy immediately; fires `just_sane` when
/// reaching 0. No-op when disabled or already sane.
///
/// `tick(dt)` clears both flags, then fades whimsy by `fade_rate * dt`
/// (floored at 0). Fires `just_sane` when reaching 0 via fade. No-op
/// when disabled or rate is 0.
///
/// `is_unhinged()` returns `whimsy >= max_whimsy && enabled`.
///
/// `is_sane()` returns `whimsy == 0.0` (not gated by `enabled`).
///
/// `whimsy_fraction()` returns `(whimsy / max_whimsy).clamp(0, 1)`.
///
/// `effective_chaos(scale)` returns `scale * whimsy_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 10.0)` — fades at 10 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zany {
    pub whimsy: f32,
    pub max_whimsy: f32,
    pub fade_rate: f32,
    pub just_unhinged: bool,
    pub just_sane: bool,
    pub enabled: bool,
}

impl Zany {
    pub fn new(max_whimsy: f32, fade_rate: f32) -> Self {
        Self {
            whimsy: 0.0,
            max_whimsy: max_whimsy.max(0.1),
            fade_rate: fade_rate.max(0.0),
            just_unhinged: false,
            just_sane: false,
            enabled: true,
        }
    }

    /// Add whimsy; fires `just_unhinged` when first reaching max.
    /// No-op when disabled.
    pub fn spark(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.whimsy < self.max_whimsy;
        self.whimsy = (self.whimsy + amount).min(self.max_whimsy);
        if was_below && self.whimsy >= self.max_whimsy {
            self.just_unhinged = true;
        }
    }

    /// Reduce whimsy; fires `just_sane` when reaching 0.
    /// No-op when disabled or already sane.
    pub fn calm(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.whimsy <= 0.0 {
            return;
        }
        self.whimsy = (self.whimsy - amount).max(0.0);
        if self.whimsy <= 0.0 {
            self.just_sane = true;
        }
    }

    /// Clear flags, then fade whimsy by `fade_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_unhinged = false;
        self.just_sane = false;
        if self.enabled && self.fade_rate > 0.0 && self.whimsy > 0.0 {
            self.whimsy = (self.whimsy - self.fade_rate * dt).max(0.0);
            if self.whimsy <= 0.0 {
                self.just_sane = true;
            }
        }
    }

    /// `true` when whimsy is at maximum and component is enabled.
    pub fn is_unhinged(&self) -> bool {
        self.whimsy >= self.max_whimsy && self.enabled
    }

    /// `true` when whimsy is 0 (not gated by `enabled`).
    pub fn is_sane(&self) -> bool {
        self.whimsy == 0.0
    }

    /// Fraction of maximum whimsy [0.0, 1.0].
    pub fn whimsy_fraction(&self) -> f32 {
        (self.whimsy / self.max_whimsy).clamp(0.0, 1.0)
    }

    /// Returns `scale * whimsy_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_chaos(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.whimsy_fraction()
    }
}

impl Default for Zany {
    fn default() -> Self {
        Self::new(100.0, 10.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zany {
        Zany::new(100.0, 10.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_sane() {
        let z = z();
        assert_eq!(z.whimsy, 0.0);
        assert!(z.is_sane());
        assert!(!z.is_unhinged());
    }

    #[test]
    fn new_clamps_max_whimsy() {
        let z = Zany::new(-5.0, 10.0);
        assert!((z.max_whimsy - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_fade_rate() {
        let z = Zany::new(100.0, -3.0);
        assert_eq!(z.fade_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zany::default();
        assert!((z.max_whimsy - 100.0).abs() < 1e-5);
        assert!((z.fade_rate - 10.0).abs() < 1e-5);
    }

    // --- spark ---

    #[test]
    fn spark_adds_whimsy() {
        let mut z = z();
        z.spark(40.0);
        assert!((z.whimsy - 40.0).abs() < 1e-3);
    }

    #[test]
    fn spark_clamps_at_max() {
        let mut z = z();
        z.spark(200.0);
        assert!((z.whimsy - 100.0).abs() < 1e-3);
    }

    #[test]
    fn spark_fires_just_unhinged_at_max() {
        let mut z = z();
        z.spark(100.0);
        assert!(z.just_unhinged);
        assert!(z.is_unhinged());
    }

    #[test]
    fn spark_no_just_unhinged_when_already_at_max() {
        let mut z = z();
        z.whimsy = 100.0;
        z.spark(10.0);
        assert!(!z.just_unhinged);
    }

    #[test]
    fn spark_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.spark(50.0);
        assert_eq!(z.whimsy, 0.0);
    }

    #[test]
    fn spark_no_op_when_amount_zero() {
        let mut z = z();
        z.spark(0.0);
        assert_eq!(z.whimsy, 0.0);
    }

    // --- calm ---

    #[test]
    fn calm_reduces_whimsy() {
        let mut z = z();
        z.whimsy = 60.0;
        z.calm(20.0);
        assert!((z.whimsy - 40.0).abs() < 1e-3);
    }

    #[test]
    fn calm_clamps_at_zero() {
        let mut z = z();
        z.whimsy = 30.0;
        z.calm(200.0);
        assert_eq!(z.whimsy, 0.0);
    }

    #[test]
    fn calm_fires_just_sane_at_zero() {
        let mut z = z();
        z.whimsy = 30.0;
        z.calm(30.0);
        assert!(z.just_sane);
    }

    #[test]
    fn calm_no_op_when_already_sane() {
        let mut z = z();
        z.calm(10.0);
        assert!(!z.just_sane);
    }

    #[test]
    fn calm_no_op_when_disabled() {
        let mut z = z();
        z.whimsy = 50.0;
        z.enabled = false;
        z.calm(50.0);
        assert!((z.whimsy - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_fades_whimsy() {
        let mut z = z(); // fade=10
        z.whimsy = 60.0;
        z.tick(1.0); // 60 - 10 = 50
        assert!((z.whimsy - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_sane_on_fade_to_zero() {
        let mut z = Zany::new(100.0, 200.0);
        z.whimsy = 5.0;
        z.tick(1.0);
        assert!(z.just_sane);
        assert!(z.is_sane());
    }

    #[test]
    fn tick_no_fade_when_already_sane() {
        let mut z = z();
        z.tick(10.0);
        assert!(!z.just_sane);
    }

    #[test]
    fn tick_no_fade_when_rate_zero() {
        let mut z = Zany::new(100.0, 0.0);
        z.whimsy = 50.0;
        z.tick(100.0);
        assert!((z.whimsy - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_fade_when_disabled() {
        let mut z = z();
        z.whimsy = 50.0;
        z.enabled = false;
        z.tick(1.0);
        assert!((z.whimsy - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_unhinged() {
        let mut z = z();
        z.spark(100.0);
        z.tick(0.016);
        assert!(!z.just_unhinged);
    }

    #[test]
    fn tick_clears_just_sane() {
        let mut z = Zany::new(100.0, 200.0);
        z.whimsy = 5.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_sane);
    }

    #[test]
    fn tick_scales_fade_with_dt() {
        let mut z = z(); // fade=10
        z.whimsy = 100.0;
        z.tick(3.0); // 100 - 10*3 = 70
        assert!((z.whimsy - 70.0).abs() < 1e-3);
    }

    // --- is_unhinged / is_sane ---

    #[test]
    fn is_unhinged_false_when_disabled() {
        let mut z = z();
        z.whimsy = 100.0;
        z.enabled = false;
        assert!(!z.is_unhinged());
    }

    #[test]
    fn is_sane_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_sane());
    }

    // --- whimsy_fraction / effective_chaos ---

    #[test]
    fn whimsy_fraction_zero_when_sane() {
        assert_eq!(z().whimsy_fraction(), 0.0);
    }

    #[test]
    fn whimsy_fraction_half_at_midpoint() {
        let mut z = z();
        z.whimsy = 50.0;
        assert!((z.whimsy_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_chaos_zero_when_sane() {
        assert_eq!(z().effective_chaos(100.0), 0.0);
    }

    #[test]
    fn effective_chaos_scales_with_whimsy() {
        let mut z = z();
        z.whimsy = 80.0;
        assert!((z.effective_chaos(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_chaos_zero_when_disabled() {
        let mut z = z();
        z.whimsy = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_chaos(100.0), 0.0);
    }
}
