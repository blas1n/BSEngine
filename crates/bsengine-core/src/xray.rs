use bevy_ecs::prelude::Component;

/// Persistent through-wall detector with an optional burst mode. Models
/// vision or sensing that works passively at `xray_range` at all times;
/// `pulse()` temporarily extends it by `pulse_bonus` for `pulse_duration`
/// seconds.
///
/// Unlike `Radar` (periodic sweep) and `Wow` (binary spectacle gate), Xray
/// has a **non-zero base output when idle** — the entity always detects up to
/// `xray_range`, and the pulse is an enhancement, not the primary mode.
///
/// `pulse()` fires `just_pulsed` and resets `pulse_timer` to
/// `pulse_duration`. No-op when already pulsing or disabled.
///
/// `tick(dt)` clears one-frame flags first, then decrements `pulse_timer` by
/// `dt` (floors at 0) when enabled. No-op (beyond flag clear) when disabled.
///
/// `is_pulsing()` returns `pulse_timer > 0.0 && enabled`.
///
/// `pulse_fraction()` returns `(pulse_timer / pulse_duration).clamp(0.0, 1.0)`
/// — 1.0 when freshly pulsed, 0.0 when idle.
///
/// `effective_range()` returns `xray_range + pulse_bonus * pulse_fraction()`
/// when enabled; 0.0 when disabled.
///
/// `has_range()` returns `effective_range() > 0.0 && enabled`.
///
/// Default: `new(5.0, 10.0, 3.0)` — 5 m base, 10 m bonus, 3 s pulse window.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Xray {
    /// Passive detection radius (always active). Clamped >= 0.0.
    pub xray_range: f32,
    /// Extra range added during a pulse. Clamped >= 0.0.
    pub pulse_bonus: f32,
    /// Remaining pulse duration [0, pulse_duration].
    pub pulse_timer: f32,
    /// Duration of each pulse in seconds. Clamped >= 0.1.
    pub pulse_duration: f32,
    pub just_pulsed: bool,
    pub enabled: bool,
}

impl Xray {
    pub fn new(xray_range: f32, pulse_bonus: f32, pulse_duration: f32) -> Self {
        Self {
            xray_range: xray_range.max(0.0),
            pulse_bonus: pulse_bonus.max(0.0),
            pulse_timer: 0.0,
            pulse_duration: pulse_duration.max(0.1),
            just_pulsed: false,
            enabled: true,
        }
    }

    /// Start a pulse burst. Fires `just_pulsed` and resets timer to full
    /// pulse duration. No-op when already pulsing or disabled.
    pub fn pulse(&mut self) {
        if !self.enabled || self.pulse_timer > 0.0 {
            return;
        }
        self.pulse_timer = self.pulse_duration;
        self.just_pulsed = true;
    }

    /// Advance one frame: clear flags, then tick pulse timer down. No-op
    /// (beyond flag clear) when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_pulsed = false;

        if !self.enabled || self.pulse_timer == 0.0 {
            return;
        }

        self.pulse_timer = (self.pulse_timer - dt).max(0.0);
    }

    /// `true` while a pulse is active and component is enabled.
    pub fn is_pulsing(&self) -> bool {
        self.pulse_timer > 0.0 && self.enabled
    }

    /// Pulse freshness [0.0, 1.0]: 1.0 at start of pulse, 0.0 when idle.
    pub fn pulse_fraction(&self) -> f32 {
        (self.pulse_timer / self.pulse_duration).clamp(0.0, 1.0)
    }

    /// Total effective detection range. Returns `xray_range + pulse_bonus *
    /// pulse_fraction()` when enabled; 0.0 when disabled.
    pub fn effective_range(&self) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        self.xray_range + self.pulse_bonus * self.pulse_fraction()
    }

    /// `true` when `effective_range() > 0.0` and component is enabled.
    pub fn has_range(&self) -> bool {
        self.effective_range() > 0.0 && self.enabled
    }
}

impl Default for Xray {
    fn default() -> Self {
        Self::new(5.0, 10.0, 3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn x() -> Xray {
        Xray::new(5.0, 10.0, 4.0) // 5m base, 10m bonus, 4s pulse
    }

    // --- construction ---

    #[test]
    fn new_starts_idle() {
        let x = x();
        assert_eq!(x.pulse_timer, 0.0);
        assert!(!x.just_pulsed);
        assert!(!x.is_pulsing());
        assert!((x.xray_range - 5.0).abs() < 1e-5);
    }

    #[test]
    fn xray_range_clamped_to_zero() {
        let x = Xray::new(-1.0, 5.0, 2.0);
        assert_eq!(x.xray_range, 0.0);
    }

    #[test]
    fn pulse_bonus_clamped_to_zero() {
        let x = Xray::new(5.0, -3.0, 2.0);
        assert_eq!(x.pulse_bonus, 0.0);
    }

    #[test]
    fn pulse_duration_clamped_to_point_one() {
        let x = Xray::new(5.0, 10.0, 0.0);
        assert!((x.pulse_duration - 0.1).abs() < 1e-5);
    }

    // --- pulse ---

    #[test]
    fn pulse_sets_timer_to_duration() {
        let mut x = x();
        x.pulse();
        assert!((x.pulse_timer - 4.0).abs() < 1e-5);
    }

    #[test]
    fn pulse_fires_just_pulsed() {
        let mut x = x();
        x.pulse();
        assert!(x.just_pulsed);
    }

    #[test]
    fn pulse_activates_is_pulsing() {
        let mut x = x();
        x.pulse();
        assert!(x.is_pulsing());
    }

    #[test]
    fn pulse_no_op_while_already_pulsing() {
        let mut x = x();
        x.pulse();
        x.tick(1.0); // timer=3, flags cleared
        x.pulse(); // no-op
        assert!(!x.just_pulsed);
        assert!((x.pulse_timer - 3.0).abs() < 1e-4);
    }

    #[test]
    fn pulse_no_op_when_disabled() {
        let mut x = x();
        x.enabled = false;
        x.pulse();
        assert_eq!(x.pulse_timer, 0.0);
        assert!(!x.just_pulsed);
    }

    #[test]
    fn pulse_allowed_after_expiry() {
        let mut x = x();
        x.pulse();
        x.tick(4.0); // expires
        x.pulse(); // new pulse
        assert!(x.just_pulsed);
        assert!((x.pulse_timer - 4.0).abs() < 1e-5);
    }

    // --- tick ---

    #[test]
    fn tick_counts_down_pulse_timer() {
        let mut x = x();
        x.pulse(); // 4.0
        x.tick(1.5); // 2.5
        assert!((x.pulse_timer - 2.5).abs() < 1e-4);
    }

    #[test]
    fn tick_floors_timer_at_zero() {
        let mut x = x();
        x.pulse();
        x.tick(10.0); // over by 6
        assert_eq!(x.pulse_timer, 0.0);
    }

    #[test]
    fn tick_clears_just_pulsed_next_frame() {
        let mut x = x();
        x.pulse();
        x.tick(0.016);
        assert!(!x.just_pulsed);
    }

    #[test]
    fn tick_no_op_when_idle() {
        let mut x = x();
        x.tick(1.0);
        assert_eq!(x.pulse_timer, 0.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut x = x();
        x.pulse();
        x.enabled = false;
        let timer_before = x.pulse_timer;
        x.tick(2.0);
        assert!((x.pulse_timer - timer_before).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_flags_even_when_disabled() {
        let mut x = x();
        x.just_pulsed = true;
        x.enabled = false;
        x.tick(0.016);
        assert!(!x.just_pulsed);
    }

    // --- is_pulsing ---

    #[test]
    fn is_pulsing_false_when_idle() {
        let x = x();
        assert!(!x.is_pulsing());
    }

    #[test]
    fn is_pulsing_true_while_active() {
        let mut x = x();
        x.pulse();
        x.tick(2.0); // 2.0 remaining
        assert!(x.is_pulsing());
    }

    #[test]
    fn is_pulsing_false_when_expired() {
        let mut x = x();
        x.pulse();
        x.tick(4.0);
        assert!(!x.is_pulsing());
    }

    #[test]
    fn is_pulsing_false_when_disabled() {
        let mut x = x();
        x.pulse();
        x.enabled = false;
        assert!(!x.is_pulsing());
    }

    // --- pulse_fraction ---

    #[test]
    fn pulse_fraction_zero_when_idle() {
        let x = x();
        assert_eq!(x.pulse_fraction(), 0.0);
    }

    #[test]
    fn pulse_fraction_one_immediately_after_pulse() {
        let mut x = x();
        x.pulse(); // timer=4, duration=4 → 1.0
        assert!((x.pulse_fraction() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn pulse_fraction_half_at_midpoint() {
        let mut x = x(); // duration=4
        x.pulse();
        x.tick(2.0); // timer=2 → 2/4=0.5
        assert!((x.pulse_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn pulse_fraction_zero_when_expired() {
        let mut x = x();
        x.pulse();
        x.tick(4.0);
        assert_eq!(x.pulse_fraction(), 0.0);
    }

    // --- effective_range ---

    #[test]
    fn effective_range_equals_base_when_idle() {
        let x = x(); // base=5, bonus=10, fraction=0 → 5+0=5
        assert!((x.effective_range() - 5.0).abs() < 1e-4);
    }

    #[test]
    fn effective_range_at_full_pulse() {
        let mut x = x();
        x.pulse(); // fraction=1.0 → 5+10*1=15
        assert!((x.effective_range() - 15.0).abs() < 1e-3);
    }

    #[test]
    fn effective_range_at_half_pulse() {
        let mut x = x();
        x.pulse();
        x.tick(2.0); // fraction=0.5 → 5+10*0.5=10
        assert!((x.effective_range() - 10.0).abs() < 1e-3);
    }

    #[test]
    fn effective_range_returns_base_after_expiry() {
        let mut x = x();
        x.pulse();
        x.tick(4.0); // expired → 5+0=5
        assert!((x.effective_range() - 5.0).abs() < 1e-4);
    }

    #[test]
    fn effective_range_zero_when_disabled() {
        let x = {
            let mut x = x();
            x.enabled = false;
            x
        };
        assert_eq!(x.effective_range(), 0.0);
    }

    #[test]
    fn effective_range_zero_base_no_bonus() {
        let x = Xray::new(0.0, 0.0, 2.0);
        assert_eq!(x.effective_range(), 0.0);
    }

    // --- has_range ---

    #[test]
    fn has_range_true_with_nonzero_base() {
        let x = x(); // base=5
        assert!(x.has_range());
    }

    #[test]
    fn has_range_false_when_disabled() {
        let x = {
            let mut x = x();
            x.enabled = false;
            x
        };
        assert!(!x.has_range());
    }

    #[test]
    fn has_range_false_when_no_base_and_idle() {
        let x = Xray::new(0.0, 5.0, 2.0); // no base; bonus only during pulse
        assert!(!x.has_range());
    }

    #[test]
    fn has_range_true_when_no_base_but_pulsing() {
        let mut x = Xray::new(0.0, 5.0, 2.0);
        x.pulse(); // bonus=5 active
        assert!(x.has_range());
    }

    // --- re-pulse cycle ---

    #[test]
    fn can_pulse_again_after_expiry() {
        let mut x = x();
        x.pulse();
        x.tick(4.0); // expire
        x.tick(0.016); // clear flags
        x.pulse(); // new pulse
        assert!(x.just_pulsed);
        assert!((x.pulse_timer - 4.0).abs() < 1e-5);
    }
}
