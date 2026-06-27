use bevy_ecs::prelude::Component;

/// On-demand single-shot emitter with per-yap rate limiting. Call `yap()` to
/// fire an alert; it succeeds only when the cooldown has expired. Models a
/// bark, ping, or chatty notification source that must respect a minimum
/// interval between emissions.
///
/// `yap()` fires `just_yapped`, increments `yap_count`, and resets
/// `cooldown_remaining` to `yap_interval`. No-op when disabled or cooling
/// down.
///
/// `tick(dt)` clears `just_yapped` first, then if enabled drains
/// `cooldown_remaining` toward 0.
///
/// `is_ready()` returns `cooldown_remaining == 0.0 && enabled`.
///
/// `cooldown_fraction()` returns `(cooldown_remaining / yap_interval).clamp(0,1)`;
/// returns `0.0` when `yap_interval == 0`.
///
/// `effective_urgency(base)` returns `base` when `just_yapped && enabled`;
/// `0.0` otherwise.
///
/// Default: `new(1.0)` — one-second cooldown between yaps.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yap {
    /// Minimum seconds between yaps. Clamped >= 0.0.
    pub yap_interval: f32,
    /// Seconds until next yap is allowed [0, yap_interval].
    pub cooldown_remaining: f32,
    /// Total yaps fired since creation.
    pub yap_count: u32,
    pub just_yapped: bool,
    pub enabled: bool,
}

impl Yap {
    pub fn new(yap_interval: f32) -> Self {
        Self {
            yap_interval: yap_interval.max(0.0),
            cooldown_remaining: 0.0,
            yap_count: 0,
            just_yapped: false,
            enabled: true,
        }
    }

    /// Emit one yap if enabled and not cooling down.
    pub fn yap(&mut self) {
        if !self.enabled || self.cooldown_remaining > 0.0 {
            return;
        }
        self.just_yapped = true;
        self.yap_count += 1;
        self.cooldown_remaining = self.yap_interval;
    }

    /// Advance one frame: clear flags, then drain cooldown.
    pub fn tick(&mut self, dt: f32) {
        self.just_yapped = false;
        if !self.enabled {
            return;
        }
        self.cooldown_remaining = (self.cooldown_remaining - dt).max(0.0);
    }

    /// `true` when no cooldown is active and component is enabled.
    pub fn is_ready(&self) -> bool {
        self.cooldown_remaining == 0.0 && self.enabled
    }

    /// Cooldown progress as a fraction [0.0, 1.0]. 0.0 when `yap_interval` is 0.
    pub fn cooldown_fraction(&self) -> f32 {
        if self.yap_interval <= 0.0 {
            return 0.0;
        }
        (self.cooldown_remaining / self.yap_interval).clamp(0.0, 1.0)
    }

    /// Returns `base` when `just_yapped` and enabled; `0.0` otherwise.
    pub fn effective_urgency(&self, base: f32) -> f32 {
        if self.enabled && self.just_yapped {
            base
        } else {
            0.0
        }
    }
}

impl Default for Yap {
    fn default() -> Self {
        Self::new(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yap {
        Yap::new(1.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_ready_with_no_count() {
        let y = y();
        assert_eq!(y.yap_count, 0);
        assert_eq!(y.cooldown_remaining, 0.0);
        assert!(!y.just_yapped);
        assert!(y.is_ready());
    }

    #[test]
    fn interval_clamped_to_zero() {
        let y = Yap::new(-5.0);
        assert_eq!(y.yap_interval, 0.0);
    }

    // --- yap ---

    #[test]
    fn yap_fires_just_yapped() {
        let mut y = y();
        y.yap();
        assert!(y.just_yapped);
    }

    #[test]
    fn yap_increments_count() {
        let mut y = y();
        y.yap();
        assert_eq!(y.yap_count, 1);
    }

    #[test]
    fn yap_starts_cooldown() {
        let mut y = y();
        y.yap();
        assert!((y.cooldown_remaining - 1.0).abs() < 1e-5);
    }

    #[test]
    fn yap_no_op_when_cooling_down() {
        let mut y = y();
        y.yap();
        y.tick(0.016); // clears just_yapped, cooldown still ~0.984
        y.yap();
        assert!(!y.just_yapped);
        assert_eq!(y.yap_count, 1);
    }

    #[test]
    fn yap_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.yap();
        assert!(!y.just_yapped);
        assert_eq!(y.yap_count, 0);
    }

    #[test]
    fn yap_fires_again_after_cooldown_expires() {
        let mut y = y();
        y.yap();
        y.tick(1.0); // cooldown drains to 0
        y.yap();
        assert!(y.just_yapped);
        assert_eq!(y.yap_count, 2);
    }

    #[test]
    fn yap_fires_immediately_with_zero_interval() {
        let mut y = Yap::new(0.0);
        y.yap();
        assert!(y.just_yapped);
        assert_eq!(y.cooldown_remaining, 0.0);
        y.tick(0.0);
        y.yap();
        assert!(y.just_yapped);
        assert_eq!(y.yap_count, 2);
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_yapped() {
        let mut y = y();
        y.yap();
        y.tick(0.016);
        assert!(!y.just_yapped);
    }

    #[test]
    fn tick_drains_cooldown() {
        let mut y = y();
        y.yap();
        y.tick(0.5);
        assert!((y.cooldown_remaining - 0.5).abs() < 1e-4);
    }

    #[test]
    fn tick_floors_cooldown_at_zero() {
        let mut y = y();
        y.yap();
        y.tick(2.0);
        assert_eq!(y.cooldown_remaining, 0.0);
    }

    #[test]
    fn tick_clears_flag_even_when_disabled() {
        let mut y = y();
        y.just_yapped = true;
        y.enabled = false;
        y.tick(1.0);
        assert!(!y.just_yapped);
    }

    #[test]
    fn tick_no_op_on_cooldown_when_disabled() {
        let mut y = y();
        y.cooldown_remaining = 1.0;
        y.enabled = false;
        y.tick(0.5);
        assert!((y.cooldown_remaining - 1.0).abs() < 1e-5);
    }

    // --- is_ready ---

    #[test]
    fn is_ready_true_at_start() {
        assert!(y().is_ready());
    }

    #[test]
    fn is_ready_false_while_cooling() {
        let mut y = y();
        y.yap();
        assert!(!y.is_ready());
    }

    #[test]
    fn is_ready_true_after_cooldown_expires() {
        let mut y = y();
        y.yap();
        y.tick(1.0);
        assert!(y.is_ready());
    }

    #[test]
    fn is_ready_false_when_disabled() {
        let y_disabled = {
            let mut y = y();
            y.enabled = false;
            y
        };
        assert!(!y_disabled.is_ready());
    }

    // --- cooldown_fraction ---

    #[test]
    fn cooldown_fraction_zero_when_ready() {
        assert_eq!(y().cooldown_fraction(), 0.0);
    }

    #[test]
    fn cooldown_fraction_one_immediately_after_yap() {
        let mut y = y();
        y.yap();
        assert!((y.cooldown_fraction() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn cooldown_fraction_half_at_midpoint() {
        let mut y = y();
        y.yap();
        y.tick(0.5);
        assert!((y.cooldown_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn cooldown_fraction_zero_for_zero_interval() {
        let mut y = Yap::new(0.0);
        y.yap();
        assert_eq!(y.cooldown_fraction(), 0.0);
    }

    // --- effective_urgency ---

    #[test]
    fn effective_urgency_base_when_just_yapped() {
        let mut y = y();
        y.yap();
        assert!((y.effective_urgency(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_urgency_zero_when_not_just_yapped() {
        let y = y();
        assert_eq!(y.effective_urgency(100.0), 0.0);
    }

    #[test]
    fn effective_urgency_zero_after_tick() {
        let mut y = y();
        y.yap();
        y.tick(0.016);
        assert_eq!(y.effective_urgency(100.0), 0.0);
    }

    #[test]
    fn effective_urgency_zero_when_disabled() {
        let mut y = y();
        y.just_yapped = true;
        y.enabled = false;
        assert_eq!(y.effective_urgency(100.0), 0.0);
    }

    // --- multi-yap cycle ---

    #[test]
    fn multiple_yap_cycle() {
        let mut y = y();
        y.yap();
        assert_eq!(y.yap_count, 1);
        y.tick(1.0);
        y.yap();
        assert_eq!(y.yap_count, 2);
        y.tick(1.0);
        y.yap();
        assert_eq!(y.yap_count, 3);
    }
}
