use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Phase of a warp / blink teleport cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarpPhase {
    /// Ready to warp.
    Ready,
    /// Charging before teleport (optional wind-up).
    Charging,
    /// Teleport has fired this frame.
    Warping,
    /// Cooling down; cannot warp again until timer expires.
    Cooldown,
}

/// Entity-level blink / warp teleport component.
///
/// Separate from `Portal` (a world-space warp gate between two locations).
/// `Warp` is an ability attached to an entity that can instantly reposition
/// itself to a requested destination.
///
/// Call `set_destination(pos)` then `trigger()` to initiate. `tick(dt)`
/// drives the charge and cooldown timers. Check `just_warped` on the frame
/// the teleport completes to move the entity's `Transform`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Warp {
    pub phase: WarpPhase,
    /// Requested teleport target (world space).
    pub destination: Vec3,
    /// Maximum warp range (0 = unlimited).
    pub max_range: f32,
    /// Charge-up time in seconds before teleporting (0 = instant).
    pub charge_duration: f32,
    pub charge_timer: f32,
    /// Cooldown in seconds after teleporting.
    pub cooldown_duration: f32,
    pub cooldown_timer: f32,
    /// True on the exact frame the teleport fires (move Transform this frame).
    pub just_warped: bool,
    pub enabled: bool,
}

impl Warp {
    pub fn new(max_range: f32, cooldown_duration: f32) -> Self {
        Self {
            phase: WarpPhase::Ready,
            destination: Vec3::ZERO,
            max_range: max_range.max(0.0),
            charge_duration: 0.0,
            charge_timer: 0.0,
            cooldown_duration: cooldown_duration.max(0.0),
            cooldown_timer: 0.0,
            just_warped: false,
            enabled: true,
        }
    }

    /// Instant warp with no charge-up.
    pub fn instant(max_range: f32, cooldown_duration: f32) -> Self {
        Self::new(max_range, cooldown_duration)
    }

    pub fn with_charge(mut self, charge_duration: f32) -> Self {
        self.charge_duration = charge_duration.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Set the desired teleport destination.
    pub fn set_destination(&mut self, pos: Vec3) {
        self.destination = pos;
    }

    /// Attempt to begin warping. Returns false if disabled, on cooldown, or already charging.
    pub fn trigger(&mut self, origin: Vec3) -> bool {
        if !self.enabled || self.phase != WarpPhase::Ready {
            return false;
        }
        // Range check.
        if self.max_range > 0.0 {
            let dist = self.destination.distance(origin);
            if dist > self.max_range {
                return false;
            }
        }
        if self.charge_duration > 0.0 {
            self.phase = WarpPhase::Charging;
            self.charge_timer = self.charge_duration;
        } else {
            self.fire();
        }
        true
    }

    /// Cancel a charge in progress, returning to Ready without cooldown.
    pub fn cancel(&mut self) {
        if self.phase == WarpPhase::Charging {
            self.phase = WarpPhase::Ready;
            self.charge_timer = 0.0;
        }
    }

    /// Advance timers. Call once per frame.
    pub fn tick(&mut self, dt: f32) {
        self.just_warped = false;

        match self.phase {
            WarpPhase::Charging => {
                self.charge_timer = (self.charge_timer - dt).max(0.0);
                if self.charge_timer <= 0.0 {
                    self.fire();
                }
            }
            WarpPhase::Warping => {
                // Warping lasts exactly one frame.
                if self.cooldown_duration > 0.0 {
                    self.phase = WarpPhase::Cooldown;
                    self.cooldown_timer = self.cooldown_duration;
                } else {
                    self.phase = WarpPhase::Ready;
                }
            }
            WarpPhase::Cooldown => {
                self.cooldown_timer = (self.cooldown_timer - dt).max(0.0);
                if self.cooldown_timer <= 0.0 {
                    self.phase = WarpPhase::Ready;
                }
            }
            WarpPhase::Ready => {}
        }
    }

    pub fn is_ready(&self) -> bool {
        self.enabled && self.phase == WarpPhase::Ready
    }

    pub fn is_on_cooldown(&self) -> bool {
        self.phase == WarpPhase::Cooldown
    }

    pub fn cooldown_fraction(&self) -> f32 {
        if self.cooldown_duration > 0.0 {
            1.0 - self.cooldown_timer / self.cooldown_duration
        } else {
            1.0
        }
    }

    fn fire(&mut self) {
        self.phase = WarpPhase::Warping;
        self.just_warped = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn warp() -> Warp {
        Warp::new(20.0, 1.0)
    }

    #[test]
    fn instant_trigger_fires_immediately() {
        let mut w = warp();
        w.set_destination(Vec3::new(5.0, 0.0, 0.0));
        assert!(w.trigger(Vec3::ZERO));
        assert_eq!(w.phase, WarpPhase::Warping);
        assert!(w.just_warped);
    }

    #[test]
    fn out_of_range_trigger_fails() {
        let mut w = warp();
        w.set_destination(Vec3::new(100.0, 0.0, 0.0));
        assert!(!w.trigger(Vec3::ZERO));
        assert_eq!(w.phase, WarpPhase::Ready);
    }

    #[test]
    fn cooldown_after_warp() {
        let mut w = warp();
        w.set_destination(Vec3::new(5.0, 0.0, 0.0));
        w.trigger(Vec3::ZERO);
        w.tick(0.016); // warping → cooldown
        assert_eq!(w.phase, WarpPhase::Cooldown);
        assert!(!w.is_ready());
    }

    #[test]
    fn cooldown_expires() {
        let mut w = warp();
        w.set_destination(Vec3::X);
        w.trigger(Vec3::ZERO);
        w.tick(0.0); // warping → cooldown
        w.tick(1.0); // finish cooldown
        assert_eq!(w.phase, WarpPhase::Ready);
    }

    #[test]
    fn charge_duration_delays_warp() {
        let mut w = Warp::new(20.0, 0.5).with_charge(0.3);
        w.set_destination(Vec3::X);
        w.trigger(Vec3::ZERO);
        assert_eq!(w.phase, WarpPhase::Charging);
        w.tick(0.3);
        assert_eq!(w.phase, WarpPhase::Warping);
        assert!(w.just_warped);
    }

    #[test]
    fn disabled_blocks_trigger() {
        let mut w = Warp::new(20.0, 1.0).disabled();
        w.set_destination(Vec3::X);
        assert!(!w.trigger(Vec3::ZERO));
    }
}
