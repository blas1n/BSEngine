use bevy_ecs::prelude::Component;

/// Active drain channel that extracts a resource from the target each frame and
/// optionally restores a fraction of the amount to the caster.
///
/// `tick(dt)` returns `(drained, restored)` — the caller subtracts `drained`
/// from the target and adds `restored` to the caster. `return_fraction = 0.0`
/// means pure drain with no restoration; `1.0` fully heals the caster for
/// every point drained.
///
/// `start(duration)` begins a new channel (no-op if already active or
/// disabled). `stop()` ends it early.
///
/// Distinct from `Leech` (passive on-hit restoration) and `Drain` (resource
/// depletion without a heal-back component): Siphon is an active, timed
/// drain-and-restore channel intended for abilities like life-tap or mana sap.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Siphon {
    pub duration: f32,
    pub timer: f32,
    /// Resource drained from the target per second.
    pub drain_per_second: f32,
    /// Fraction [0.0, 1.0] of the drained amount restored to the caster.
    pub return_fraction: f32,
    pub just_started: bool,
    pub just_ended: bool,
    pub enabled: bool,
}

impl Siphon {
    pub fn new(drain_per_second: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            drain_per_second: drain_per_second.max(0.0),
            return_fraction: 0.0,
            just_started: false,
            just_ended: false,
            enabled: true,
        }
    }

    pub fn with_return_fraction(mut self, fraction: f32) -> Self {
        self.return_fraction = fraction.clamp(0.0, 1.0);
        self
    }

    /// Begin a siphon channel. No-op if already active or disabled.
    pub fn start(&mut self, duration: f32) {
        if !self.enabled || self.is_active() {
            return;
        }
        self.duration = duration.max(0.0);
        self.timer = self.duration;
        self.just_started = true;
    }

    /// End the channel early.
    pub fn stop(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_ended = true;
        }
    }

    /// Advance the timer and return `(drained, restored)` for this frame.
    /// Sets `just_ended` when the channel expires. Returns `(0.0, 0.0)` when
    /// inactive or disabled.
    pub fn tick(&mut self, dt: f32) -> (f32, f32) {
        self.just_started = false;
        self.just_ended = false;

        if !self.enabled || !self.is_active() {
            return (0.0, 0.0);
        }

        self.timer -= dt;
        let expired = self.timer <= 0.0;
        if expired {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_ended = true;
        }

        let drained = self.drain_per_second * dt;
        let restored = drained * self.return_fraction;
        (drained, restored)
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Fraction of the channel duration remaining [1.0 = just started, 0.0 = ended].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Siphon {
    fn default() -> Self {
        Self::new(10.0).with_return_fraction(0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_activates_siphon() {
        let mut s = Siphon::new(10.0);
        s.start(3.0);
        assert!(s.is_active());
        assert!(s.just_started);
    }

    #[test]
    fn start_no_op_when_already_active() {
        let mut s = Siphon::new(10.0);
        s.start(3.0);
        s.tick(0.016);
        let timer_before = s.timer;
        s.start(5.0);
        assert!((s.timer - timer_before).abs() < 1e-5);
    }

    #[test]
    fn stop_ends_channel_early() {
        let mut s = Siphon::new(10.0);
        s.start(5.0);
        s.stop();
        assert!(!s.is_active());
        assert!(s.just_ended);
    }

    #[test]
    fn tick_drains_and_restores() {
        let mut s = Siphon::new(20.0).with_return_fraction(0.5);
        s.start(3.0);
        let (drained, restored) = s.tick(0.1);
        assert!((drained - 2.0).abs() < 1e-4); // 20 * 0.1
        assert!((restored - 1.0).abs() < 1e-4); // 2.0 * 0.5
    }

    #[test]
    fn tick_zero_when_inactive() {
        let mut s = Siphon::new(10.0);
        let (drained, restored) = s.tick(0.1);
        assert_eq!(drained, 0.0);
        assert_eq!(restored, 0.0);
    }

    #[test]
    fn tick_expires_channel() {
        let mut s = Siphon::new(10.0);
        s.start(1.0);
        s.tick(1.1);
        assert!(!s.is_active());
        assert!(s.just_ended);
    }

    #[test]
    fn return_fraction_zero_no_restore() {
        let mut s = Siphon::new(10.0); // return_fraction defaults to 0.0
        s.start(3.0);
        let (drained, restored) = s.tick(0.1);
        assert!(drained > 0.0);
        assert_eq!(restored, 0.0);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut s = Siphon::new(10.0);
        s.start(2.0);
        s.tick(1.0);
        assert!((s.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_start_no_op() {
        let mut s = Siphon::new(10.0);
        s.enabled = false;
        s.start(3.0);
        assert!(!s.is_active());
    }

    #[test]
    fn disabled_tick_returns_zero() {
        let mut s = Siphon::new(10.0);
        s.start(3.0);
        s.enabled = false;
        let (drained, restored) = s.tick(0.1);
        assert_eq!(drained, 0.0);
        assert_eq!(restored, 0.0);
    }

    #[test]
    fn tick_clears_just_started() {
        let mut s = Siphon::new(10.0);
        s.start(3.0);
        s.tick(0.016);
        assert!(!s.just_started);
    }
}
