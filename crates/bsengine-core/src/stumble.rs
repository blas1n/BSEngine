use bevy_ecs::prelude::Component;

/// Brief loss-of-footing reaction that increases incoming damage and reduces
/// movement speed for a fixed duration. Multiple calls to `trigger()` refresh
/// the duration (reset to `stumble_duration`). Fires `just_stumbled` on the
/// first activation in a given bout, and `just_recovered` when the timer expires.
///
/// `trigger()` begins or refreshes the stumble. Increments `stumble_count` on
/// every call. Fires `just_stumbled` only when transitioning from non-stumbling
/// to stumbling (i.e., not if already mid-stumble). Resets `stumble_timer` to
/// `stumble_duration` regardless. No-op when disabled.
///
/// `tick(dt)` clears one-frame flags at start; counts down `stumble_timer`;
/// fires `just_recovered` and clears `stumbling` when the timer reaches zero.
/// No-op when disabled.
///
/// `is_stumbling()` returns `stumbling && enabled`.
///
/// `effective_incoming(base)` returns `base * (1.0 + vulnerability_factor)`
/// when stumbling and enabled — the entity is vulnerable during a stumble.
///
/// `effective_move_speed(base)` returns `base * (1.0 - move_penalty)` when
/// stumbling and enabled, floored at 0.0.
///
/// Distinct from `Stagger` (heavy-hit interrupt that cancels in-progress
/// actions), `Flinch` (minimal interrupt with no duration), `Daze` (prolonged
/// confusion debuff), and `Concuss` (stacking concussion debuff): Stumble is a
/// **brief footing-loss reaction** — a short fixed-duration window of
/// vulnerability and slowed movement, refreshable by repeated triggers.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Stumble {
    /// Remaining stumble duration in seconds. 0.0 when not stumbling.
    pub stumble_timer: f32,
    /// Duration of each stumble in seconds. Clamped >= 0.1.
    pub stumble_duration: f32,
    /// Additional incoming damage multiplier while stumbling. Clamped [0.0, 1.0].
    pub vulnerability_factor: f32,
    /// Movement speed reduction while stumbling. Clamped [0.0, 1.0].
    pub move_penalty: f32,
    /// Total number of times `trigger()` has been called successfully.
    pub stumble_count: u32,
    pub stumbling: bool,
    pub just_stumbled: bool,
    pub just_recovered: bool,
    pub enabled: bool,
}

impl Stumble {
    pub fn new(stumble_duration: f32, vulnerability_factor: f32, move_penalty: f32) -> Self {
        Self {
            stumble_timer: 0.0,
            stumble_duration: stumble_duration.max(0.1),
            vulnerability_factor: vulnerability_factor.clamp(0.0, 1.0),
            move_penalty: move_penalty.clamp(0.0, 1.0),
            stumble_count: 0,
            stumbling: false,
            just_stumbled: false,
            just_recovered: false,
            enabled: true,
        }
    }

    /// Trigger or refresh a stumble. Resets the timer to `stumble_duration`.
    /// Fires `just_stumbled` only on the transition from not-stumbling to
    /// stumbling. Always increments `stumble_count`. No-op when disabled.
    pub fn trigger(&mut self) {
        if !self.enabled {
            return;
        }
        let was_not_stumbling = !self.stumbling;
        self.stumbling = true;
        self.stumble_timer = self.stumble_duration;
        self.stumble_count += 1;
        if was_not_stumbling {
            self.just_stumbled = true;
        }
    }

    /// Advance the stumble timer. Clears one-frame flags at start; counts down
    /// `stumble_timer`; fires `just_recovered` and clears `stumbling` on expiry.
    /// No-op when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_stumbled = false;
        self.just_recovered = false;

        if !self.enabled || !self.stumbling {
            return;
        }

        self.stumble_timer = (self.stumble_timer - dt).max(0.0);
        if self.stumble_timer == 0.0 {
            self.stumbling = false;
            self.just_recovered = true;
        }
    }

    /// `true` when currently stumbling and the component is enabled.
    pub fn is_stumbling(&self) -> bool {
        self.stumbling && self.enabled
    }

    /// Incoming damage amplified by vulnerability. Returns
    /// `base * (1.0 + vulnerability_factor)` when stumbling and enabled;
    /// returns `base` otherwise.
    pub fn effective_incoming(&self, base: f32) -> f32 {
        if self.is_stumbling() {
            base * (1.0 + self.vulnerability_factor)
        } else {
            base
        }
    }

    /// Movement speed reduced by stumble penalty. Returns
    /// `base * (1.0 - move_penalty)` when stumbling and enabled, floored at
    /// 0.0; returns `base` otherwise.
    pub fn effective_move_speed(&self, base: f32) -> f32 {
        if self.is_stumbling() {
            (base * (1.0 - self.move_penalty)).max(0.0)
        } else {
            base
        }
    }
}

impl Default for Stumble {
    fn default() -> Self {
        Self::new(0.5, 0.3, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_stable() {
        let s = Stumble::new(0.5, 0.3, 0.5);
        assert!(!s.stumbling);
        assert_eq!(s.stumble_timer, 0.0);
        assert_eq!(s.stumble_count, 0);
        assert!(!s.is_stumbling());
    }

    #[test]
    fn trigger_sets_stumbling() {
        let mut s = Stumble::new(0.5, 0.3, 0.5);
        s.trigger();
        assert!(s.stumbling);
        assert!(s.is_stumbling());
    }

    #[test]
    fn trigger_fires_just_stumbled_on_first_activation() {
        let mut s = Stumble::new(0.5, 0.3, 0.5);
        s.trigger();
        assert!(s.just_stumbled);
    }

    #[test]
    fn trigger_no_just_stumbled_when_already_stumbling() {
        let mut s = Stumble::new(1.0, 0.3, 0.5);
        s.trigger();
        s.tick(0.0); // clear flags
        s.trigger(); // refresh while still stumbling
        assert!(!s.just_stumbled);
    }

    #[test]
    fn trigger_increments_count_every_call() {
        let mut s = Stumble::new(1.0, 0.3, 0.5);
        s.trigger();
        s.trigger();
        assert_eq!(s.stumble_count, 2);
    }

    #[test]
    fn trigger_resets_timer_on_refresh() {
        let mut s = Stumble::new(2.0, 0.3, 0.5);
        s.trigger();
        s.tick(1.0); // timer = 1.0
        s.trigger(); // resets to 2.0
        assert!((s.stumble_timer - 2.0).abs() < 1e-5);
    }

    #[test]
    fn trigger_no_op_when_disabled() {
        let mut s = Stumble::new(0.5, 0.3, 0.5);
        s.enabled = false;
        s.trigger();
        assert!(!s.stumbling);
        assert_eq!(s.stumble_count, 0);
    }

    #[test]
    fn tick_counts_down_timer() {
        let mut s = Stumble::new(2.0, 0.3, 0.5);
        s.trigger();
        s.tick(1.0);
        assert!((s.stumble_timer - 1.0).abs() < 1e-5);
        assert!(s.stumbling);
    }

    #[test]
    fn tick_fires_just_recovered_on_expiry() {
        let mut s = Stumble::new(1.0, 0.3, 0.5);
        s.trigger();
        s.tick(1.0);
        assert!(s.just_recovered);
        assert!(!s.stumbling);
    }

    #[test]
    fn tick_clears_stumbling_on_expiry() {
        let mut s = Stumble::new(0.5, 0.3, 0.5);
        s.trigger();
        s.tick(0.5);
        assert!(!s.stumbling);
        assert!(!s.is_stumbling());
    }

    #[test]
    fn tick_clears_just_stumbled() {
        let mut s = Stumble::new(0.5, 0.3, 0.5);
        s.trigger();
        s.tick(0.016);
        assert!(!s.just_stumbled);
    }

    #[test]
    fn tick_clears_just_recovered_next_frame() {
        let mut s = Stumble::new(0.5, 0.3, 0.5);
        s.trigger();
        s.tick(0.5); // just_recovered = true
        s.tick(0.016); // cleared
        assert!(!s.just_recovered);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut s = Stumble::new(1.0, 0.3, 0.5);
        s.trigger();
        s.enabled = false;
        s.tick(1.0);
        // timer doesn't advance, stumbling stays
        assert!((s.stumble_timer - 1.0).abs() < 1e-5);
        assert!(s.stumbling);
    }

    #[test]
    fn tick_no_op_when_not_stumbling() {
        let mut s = Stumble::new(0.5, 0.3, 0.5);
        s.tick(5.0); // no panic
        assert!(!s.stumbling);
    }

    #[test]
    fn is_stumbling_false_when_disabled() {
        let mut s = Stumble::new(0.5, 0.3, 0.5);
        s.stumbling = true;
        s.enabled = false;
        assert!(!s.is_stumbling());
    }

    #[test]
    fn effective_incoming_amplified_while_stumbling() {
        let mut s = Stumble::new(1.0, 0.3, 0.5);
        s.trigger();
        // 100 * (1 + 0.3) = 130
        assert!((s.effective_incoming(100.0) - 130.0).abs() < 1e-3);
    }

    #[test]
    fn effective_incoming_base_when_not_stumbling() {
        let s = Stumble::new(0.5, 0.3, 0.5);
        assert!((s.effective_incoming(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_incoming_base_when_disabled() {
        let mut s = Stumble::new(0.5, 0.3, 0.5);
        s.stumbling = true;
        s.enabled = false;
        assert!((s.effective_incoming(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_incoming_base_after_recovery() {
        let mut s = Stumble::new(0.5, 0.3, 0.5);
        s.trigger();
        s.tick(0.5);
        assert!((s.effective_incoming(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_move_speed_reduced_while_stumbling() {
        let mut s = Stumble::new(1.0, 0.3, 0.5);
        s.trigger();
        // 100 * (1 - 0.5) = 50
        assert!((s.effective_move_speed(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_move_speed_base_when_not_stumbling() {
        let s = Stumble::new(0.5, 0.3, 0.5);
        assert!((s.effective_move_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_move_speed_base_when_disabled() {
        let mut s = Stumble::new(0.5, 0.3, 0.5);
        s.stumbling = true;
        s.enabled = false;
        assert!((s.effective_move_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_move_speed_floored_at_zero() {
        let mut s = Stumble::new(1.0, 0.3, 1.0);
        s.trigger();
        assert_eq!(s.effective_move_speed(100.0), 0.0);
    }

    #[test]
    fn stumble_duration_clamped_to_minimum() {
        let s = Stumble::new(0.0, 0.3, 0.5);
        assert!((s.stumble_duration - 0.1).abs() < 1e-5);
    }

    #[test]
    fn vulnerability_factor_clamped_to_one() {
        let s = Stumble::new(0.5, 2.0, 0.5);
        assert!((s.vulnerability_factor - 1.0).abs() < 1e-5);
    }

    #[test]
    fn vulnerability_factor_clamped_to_zero() {
        let s = Stumble::new(0.5, -0.5, 0.5);
        assert_eq!(s.vulnerability_factor, 0.0);
    }

    #[test]
    fn move_penalty_clamped_to_one() {
        let s = Stumble::new(0.5, 0.3, 2.0);
        assert!((s.move_penalty - 1.0).abs() < 1e-5);
    }

    #[test]
    fn move_penalty_clamped_to_zero() {
        let s = Stumble::new(0.5, 0.3, -0.5);
        assert_eq!(s.move_penalty, 0.0);
    }

    #[test]
    fn retrigger_after_recovery_fires_just_stumbled_again() {
        let mut s = Stumble::new(0.5, 0.3, 0.5);
        s.trigger();
        s.tick(0.5); // just_recovered=true
        s.trigger(); // re-trigger after full recovery
        assert!(s.just_stumbled);
    }

    #[test]
    fn count_accumulates_across_refreshes_and_recoveries() {
        let mut s = Stumble::new(0.5, 0.3, 0.5);
        s.trigger(); // 1
        s.trigger(); // 2 (refresh)
        s.tick(0.5); // recover
        s.trigger(); // 3
        assert_eq!(s.stumble_count, 3);
    }
}
