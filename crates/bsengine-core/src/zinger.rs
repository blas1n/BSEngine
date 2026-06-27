use bevy_ecs::prelude::Component;

/// Barb-sharpness tracker. `sting` builds via `barb(amount)` and
/// sharpens passively at `sharpen_rate` per second in `tick(dt)` or
/// is soothed immediately via `soothe(amount)`.
///
/// Models wit-meter gauges, banter-charge bars, insult-potency
/// accumulators, poison-barb stacking trackers, critical-taunt
/// effectiveness, venom-stack build-up meters, comeback-ability
/// indicators, or any mechanic where accumulated sharpness
/// amplifies the next strike and can be defused by friendly action.
///
/// `barb(amount)` adds sting; fires `just_stinging` when first
/// reaching `max_sting`. No-op when disabled.
///
/// `soothe(amount)` reduces sting immediately; fires `just_soothed`
/// when reaching 0. No-op when disabled or already soothed.
///
/// `tick(dt)` clears both flags, then increases sting by
/// `sharpen_rate * dt` (capped at `max_sting`). Fires `just_stinging`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_stinging()` returns `sting >= max_sting && enabled`.
///
/// `is_soothed()` returns `sting == 0.0` (not gated by `enabled`).
///
/// `sting_fraction()` returns `(sting / max_sting).clamp(0, 1)`.
///
/// `effective_barb(scale)` returns `scale * sting_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 9.0)` — sharpens at 9 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zinger {
    pub sting: f32,
    pub max_sting: f32,
    pub sharpen_rate: f32,
    pub just_stinging: bool,
    pub just_soothed: bool,
    pub enabled: bool,
}

impl Zinger {
    pub fn new(max_sting: f32, sharpen_rate: f32) -> Self {
        Self {
            sting: 0.0,
            max_sting: max_sting.max(0.1),
            sharpen_rate: sharpen_rate.max(0.0),
            just_stinging: false,
            just_soothed: false,
            enabled: true,
        }
    }

    /// Add sting; fires `just_stinging` when first reaching max.
    /// No-op when disabled.
    pub fn barb(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.sting < self.max_sting;
        self.sting = (self.sting + amount).min(self.max_sting);
        if was_below && self.sting >= self.max_sting {
            self.just_stinging = true;
        }
    }

    /// Reduce sting; fires `just_soothed` when reaching 0.
    /// No-op when disabled or already soothed.
    pub fn soothe(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.sting <= 0.0 {
            return;
        }
        self.sting = (self.sting - amount).max(0.0);
        if self.sting <= 0.0 {
            self.just_soothed = true;
        }
    }

    /// Clear flags, then increase sting by `sharpen_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_stinging = false;
        self.just_soothed = false;
        if self.enabled && self.sharpen_rate > 0.0 && self.sting < self.max_sting {
            let was_below = self.sting < self.max_sting;
            self.sting = (self.sting + self.sharpen_rate * dt).min(self.max_sting);
            if was_below && self.sting >= self.max_sting {
                self.just_stinging = true;
            }
        }
    }

    /// `true` when sting is at maximum and component is enabled.
    pub fn is_stinging(&self) -> bool {
        self.sting >= self.max_sting && self.enabled
    }

    /// `true` when sting is 0 (not gated by `enabled`).
    pub fn is_soothed(&self) -> bool {
        self.sting == 0.0
    }

    /// Fraction of maximum sting [0.0, 1.0].
    pub fn sting_fraction(&self) -> f32 {
        (self.sting / self.max_sting).clamp(0.0, 1.0)
    }

    /// Returns `scale * sting_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_barb(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.sting_fraction()
    }
}

impl Default for Zinger {
    fn default() -> Self {
        Self::new(100.0, 9.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zinger {
        Zinger::new(100.0, 9.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_soothed() {
        let z = z();
        assert_eq!(z.sting, 0.0);
        assert!(z.is_soothed());
        assert!(!z.is_stinging());
    }

    #[test]
    fn new_clamps_max_sting() {
        let z = Zinger::new(-5.0, 9.0);
        assert!((z.max_sting - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_sharpen_rate() {
        let z = Zinger::new(100.0, -3.0);
        assert_eq!(z.sharpen_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zinger::default();
        assert!((z.max_sting - 100.0).abs() < 1e-5);
        assert!((z.sharpen_rate - 9.0).abs() < 1e-5);
    }

    // --- barb ---

    #[test]
    fn barb_adds_sting() {
        let mut z = z();
        z.barb(40.0);
        assert!((z.sting - 40.0).abs() < 1e-3);
    }

    #[test]
    fn barb_clamps_at_max() {
        let mut z = z();
        z.barb(200.0);
        assert!((z.sting - 100.0).abs() < 1e-3);
    }

    #[test]
    fn barb_fires_just_stinging_at_max() {
        let mut z = z();
        z.barb(100.0);
        assert!(z.just_stinging);
        assert!(z.is_stinging());
    }

    #[test]
    fn barb_no_just_stinging_when_already_at_max() {
        let mut z = z();
        z.sting = 100.0;
        z.barb(10.0);
        assert!(!z.just_stinging);
    }

    #[test]
    fn barb_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.barb(50.0);
        assert_eq!(z.sting, 0.0);
    }

    #[test]
    fn barb_no_op_when_amount_zero() {
        let mut z = z();
        z.barb(0.0);
        assert_eq!(z.sting, 0.0);
    }

    // --- soothe ---

    #[test]
    fn soothe_reduces_sting() {
        let mut z = z();
        z.sting = 60.0;
        z.soothe(20.0);
        assert!((z.sting - 40.0).abs() < 1e-3);
    }

    #[test]
    fn soothe_clamps_at_zero() {
        let mut z = z();
        z.sting = 30.0;
        z.soothe(200.0);
        assert_eq!(z.sting, 0.0);
    }

    #[test]
    fn soothe_fires_just_soothed_at_zero() {
        let mut z = z();
        z.sting = 30.0;
        z.soothe(30.0);
        assert!(z.just_soothed);
    }

    #[test]
    fn soothe_no_op_when_already_soothed() {
        let mut z = z();
        z.soothe(10.0);
        assert!(!z.just_soothed);
    }

    #[test]
    fn soothe_no_op_when_disabled() {
        let mut z = z();
        z.sting = 50.0;
        z.enabled = false;
        z.soothe(50.0);
        assert!((z.sting - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_sharpens_sting() {
        let mut z = z(); // rate=9
        z.tick(1.0); // 0 + 9 = 9
        assert!((z.sting - 9.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_stinging_on_sharpen_to_max() {
        let mut z = Zinger::new(100.0, 200.0);
        z.sting = 95.0;
        z.tick(1.0);
        assert!(z.just_stinging);
        assert!(z.is_stinging());
    }

    #[test]
    fn tick_no_sharpen_when_already_stinging() {
        let mut z = z();
        z.sting = 100.0;
        z.tick(1.0);
        assert!(!z.just_stinging);
    }

    #[test]
    fn tick_no_sharpen_when_rate_zero() {
        let mut z = Zinger::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.sting, 0.0);
    }

    #[test]
    fn tick_no_sharpen_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.sting, 0.0);
    }

    #[test]
    fn tick_clears_just_stinging() {
        let mut z = Zinger::new(100.0, 200.0);
        z.sting = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_stinging);
    }

    #[test]
    fn tick_clears_just_soothed() {
        let mut z = z();
        z.sting = 10.0;
        z.soothe(10.0);
        z.tick(0.016);
        assert!(!z.just_soothed);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=9
        z.tick(3.0); // 9*3 = 27
        assert!((z.sting - 27.0).abs() < 1e-3);
    }

    // --- is_stinging / is_soothed ---

    #[test]
    fn is_stinging_false_when_disabled() {
        let mut z = z();
        z.sting = 100.0;
        z.enabled = false;
        assert!(!z.is_stinging());
    }

    #[test]
    fn is_soothed_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_soothed());
    }

    // --- sting_fraction / effective_barb ---

    #[test]
    fn sting_fraction_zero_when_soothed() {
        assert_eq!(z().sting_fraction(), 0.0);
    }

    #[test]
    fn sting_fraction_half_at_midpoint() {
        let mut z = z();
        z.sting = 50.0;
        assert!((z.sting_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_barb_zero_when_soothed() {
        assert_eq!(z().effective_barb(100.0), 0.0);
    }

    #[test]
    fn effective_barb_scales_with_sting() {
        let mut z = z();
        z.sting = 55.0;
        assert!((z.effective_barb(100.0) - 55.0).abs() < 1e-3);
    }

    #[test]
    fn effective_barb_zero_when_disabled() {
        let mut z = z();
        z.sting = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_barb(100.0), 0.0);
    }
}
