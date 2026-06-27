use bevy_ecs::prelude::Component;

/// Clamp-onto-target mechanic: entity grips a larger target (a boss, vehicle,
/// or obstacle) and deals damage-over-time while attached.
///
/// `latch(duration)` enters or extends the attachment (high-watermark); sets
/// `just_latched` on the inactive → active transition. `release()` detaches
/// early and sets `just_released`. `tick(dt)` counts down and sets
/// `just_released` on natural expiry.
///
/// `damage_this_frame(dt)` returns `damage_per_second * dt` while latched
/// and enabled; returns `0.0` otherwise. The combat system should call this
/// once per tick and apply the result to the host entity's health.
///
/// Distinct from `Grab` (entity holds a smaller target — the grabber
/// immobilizes the grabbed entity), `Hound` (pursuit with escalating
/// tracking intensity), and `Leech` (drains HP from target into self):
/// Latch is a **rider mechanic** — entity clings to a larger host,
/// moving with it and steadily dealing damage from the outside in.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Latch {
    pub active: bool,
    pub timer: f32,
    /// Damage dealt to the host per second while latched. Clamped ≥ 0.0.
    pub damage_per_second: f32,
    pub just_latched: bool,
    pub just_released: bool,
    pub enabled: bool,
}

impl Latch {
    pub fn new(damage_per_second: f32) -> Self {
        Self {
            active: false,
            timer: 0.0,
            damage_per_second: damage_per_second.max(0.0),
            just_latched: false,
            just_released: false,
            enabled: true,
        }
    }

    /// Enter or extend the latch for `duration` seconds. High-watermark:
    /// only replaces the current timer when `duration > timer`. Sets
    /// `just_latched` on the inactive → active transition. No-op when
    /// disabled or `duration ≤ 0`.
    pub fn latch(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        if duration > self.timer {
            let was_active = self.active;
            self.active = true;
            self.timer = duration;
            if !was_active {
                self.just_latched = true;
            }
        }
    }

    /// Detach early. Sets `just_released`. No-op when not latched.
    pub fn release(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        self.timer = 0.0;
        self.just_released = true;
    }

    /// Advance the latch timer by `dt`. Sets `just_released` on natural
    /// expiry. Clears one-frame flags at the start of each tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_latched = false;
        self.just_released = false;

        if self.active {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.active = false;
                self.just_released = true;
            }
        }
    }

    /// `true` when the entity is latched and the component is enabled.
    pub fn is_latched(&self) -> bool {
        self.active && self.enabled
    }

    /// Damage to apply to the host this frame.
    /// Returns `damage_per_second * dt` when latched and enabled; 0.0 otherwise.
    pub fn damage_this_frame(&self, dt: f32) -> f32 {
        if self.is_latched() {
            self.damage_per_second * dt
        } else {
            0.0
        }
    }

    /// Fraction of latch duration remaining [1.0 = just latched, 0.0 = expired].
    /// Returns 0.0 when not active or timer was never set.
    pub fn remaining_fraction(&self, original_duration: f32) -> f32 {
        if !self.active || original_duration <= 0.0 {
            return 0.0;
        }
        (self.timer / original_duration).clamp(0.0, 1.0)
    }
}

impl Default for Latch {
    fn default() -> Self {
        Self::new(10.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_inactive() {
        let l = Latch::new(10.0);
        assert!(!l.active);
        assert!(!l.is_latched());
    }

    #[test]
    fn latch_activates() {
        let mut l = Latch::new(10.0);
        l.latch(3.0);
        assert!(l.active);
        assert!(l.just_latched);
        assert!(l.is_latched());
    }

    #[test]
    fn latch_extends_on_longer_duration() {
        let mut l = Latch::new(10.0);
        l.latch(2.0);
        l.tick(0.016);
        l.latch(5.0);
        assert!((l.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn latch_no_extend_on_shorter_duration() {
        let mut l = Latch::new(10.0);
        l.latch(5.0);
        l.latch(2.0);
        assert!((l.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn just_latched_not_set_on_extend() {
        let mut l = Latch::new(10.0);
        l.latch(2.0);
        l.tick(0.016);
        l.latch(5.0);
        assert!(!l.just_latched);
    }

    #[test]
    fn latch_no_op_when_disabled() {
        let mut l = Latch::new(10.0);
        l.enabled = false;
        l.latch(2.0);
        assert!(!l.active);
    }

    #[test]
    fn latch_no_op_at_zero_duration() {
        let mut l = Latch::new(10.0);
        l.latch(0.0);
        assert!(!l.active);
    }

    #[test]
    fn latch_no_op_at_negative_duration() {
        let mut l = Latch::new(10.0);
        l.latch(-1.0);
        assert!(!l.active);
    }

    #[test]
    fn release_detaches() {
        let mut l = Latch::new(10.0);
        l.latch(3.0);
        l.release();
        assert!(!l.active);
        assert!(l.just_released);
    }

    #[test]
    fn release_no_op_when_not_active() {
        let mut l = Latch::new(10.0);
        l.release();
        assert!(!l.just_released);
    }

    #[test]
    fn tick_expires_latch() {
        let mut l = Latch::new(10.0);
        l.latch(1.0);
        l.tick(1.1);
        assert!(!l.active);
        assert!(l.just_released);
    }

    #[test]
    fn tick_clears_just_latched() {
        let mut l = Latch::new(10.0);
        l.latch(2.0);
        l.tick(0.016);
        assert!(!l.just_latched);
    }

    #[test]
    fn tick_clears_just_released() {
        let mut l = Latch::new(10.0);
        l.latch(0.5);
        l.tick(1.0);
        l.tick(0.016);
        assert!(!l.just_released);
    }

    #[test]
    fn tick_decrements_timer() {
        let mut l = Latch::new(10.0);
        l.latch(3.0);
        l.tick(1.0);
        assert!((l.timer - 2.0).abs() < 1e-4);
        assert!(l.active);
    }

    #[test]
    fn is_latched_false_when_disabled() {
        let mut l = Latch::new(10.0);
        l.latch(2.0);
        l.enabled = false;
        assert!(!l.is_latched());
    }

    #[test]
    fn damage_this_frame_while_latched() {
        let mut l = Latch::new(20.0);
        l.latch(3.0);
        // 20 dps * 0.5 dt = 10
        assert!((l.damage_this_frame(0.5) - 10.0).abs() < 1e-3);
    }

    #[test]
    fn damage_this_frame_zero_when_not_latched() {
        let l = Latch::new(20.0);
        assert!(l.damage_this_frame(0.016).abs() < 1e-5);
    }

    #[test]
    fn damage_this_frame_zero_when_disabled() {
        let mut l = Latch::new(20.0);
        l.latch(3.0);
        l.enabled = false;
        assert!(l.damage_this_frame(0.5).abs() < 1e-5);
    }

    #[test]
    fn damage_per_second_clamped_non_negative() {
        let l = Latch::new(-5.0);
        assert_eq!(l.damage_per_second, 0.0);
    }

    #[test]
    fn reattach_after_release_fires_just_latched() {
        let mut l = Latch::new(10.0);
        l.latch(2.0);
        l.release();
        l.tick(0.016);
        l.latch(2.0);
        assert!(l.just_latched);
    }
}
