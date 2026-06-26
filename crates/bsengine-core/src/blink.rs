use bevy_ecs::prelude::Component;
use glam::Vec3;

/// State of a pending blink teleport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlinkState {
    /// Ready to use.
    Ready,
    /// Blink triggered; physics system should teleport the entity this frame.
    Pending,
    /// On cooldown — cannot blink yet.
    Cooldown,
}

/// Short-range teleport (blink) ability on a character entity.
/// When `state == Pending`, the movement system teleports the entity to
/// `target_position` and transitions to `Cooldown`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Blink {
    pub state: BlinkState,
    /// World-space destination set by the input/AI system before triggering.
    pub target_position: Vec3,
    /// Maximum teleport range in metres.
    pub max_range: f32,
    /// Cooldown duration in seconds.
    pub cooldown: f32,
    /// Time remaining on the current cooldown.
    pub cooldown_remaining: f32,
    /// Number of consecutive blinks allowed before a longer pause. `None` = unlimited.
    pub charges: Option<u32>,
    pub charges_remaining: u32,
    /// How long (seconds) to wait before restoring one charge.
    pub charge_regen_time: f32,
    pub charge_regen_accumulated: f32,
    pub enabled: bool,
}

impl Blink {
    pub fn new(max_range: f32, cooldown: f32) -> Self {
        Self {
            state: BlinkState::Ready,
            target_position: Vec3::ZERO,
            max_range: max_range.max(0.0),
            cooldown: cooldown.max(0.0),
            cooldown_remaining: 0.0,
            charges: None,
            charges_remaining: 1,
            charge_regen_time: cooldown,
            charge_regen_accumulated: 0.0,
            enabled: true,
        }
    }

    pub fn with_charges(mut self, charges: u32) -> Self {
        self.charges = Some(charges);
        self.charges_remaining = charges;
        self
    }

    pub fn with_charge_regen(mut self, regen_time: f32) -> Self {
        self.charge_regen_time = regen_time.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Request a blink to `target`. Clamps to max_range from `origin`.
    /// Returns `false` if not ready or disabled.
    pub fn trigger(&mut self, origin: Vec3, target: Vec3) -> bool {
        if !self.enabled || self.state != BlinkState::Ready {
            return false;
        }
        if let Some(charges) = self.charges {
            if self.charges_remaining == 0 {
                return false;
            }
            let _ = charges;
        }
        let dir = (target - origin).normalize_or_zero();
        let dist = (target - origin).length().min(self.max_range);
        self.target_position = origin + dir * dist;
        self.state = BlinkState::Pending;
        true
    }

    /// Called by the movement system after the teleport is applied.
    pub fn consume(&mut self) {
        if let Some(_) = self.charges {
            if self.charges_remaining > 0 {
                self.charges_remaining -= 1;
            }
        }
        self.cooldown_remaining = self.cooldown;
        self.state = BlinkState::Cooldown;
    }

    /// Advance timers each frame.
    pub fn tick(&mut self, dt: f32) {
        if self.state == BlinkState::Cooldown {
            self.cooldown_remaining -= dt;
            if self.cooldown_remaining <= 0.0 {
                self.cooldown_remaining = 0.0;
                if self.charges.is_none() {
                    self.state = BlinkState::Ready;
                }
            }
        }
        if let Some(_) = self.charges {
            if self.charges_remaining < self.charges.unwrap_or(0) {
                self.charge_regen_accumulated += dt;
                if self.charge_regen_accumulated >= self.charge_regen_time {
                    self.charge_regen_accumulated -= self.charge_regen_time;
                    self.charges_remaining += 1;
                }
            }
            if self.state == BlinkState::Cooldown && self.charges_remaining > 0 {
                self.state = BlinkState::Ready;
            }
        }
    }

    pub fn is_ready(&self) -> bool {
        self.enabled && self.state == BlinkState::Ready
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blink_trigger_sets_pending() {
        let mut b = Blink::new(10.0, 2.0);
        assert!(b.trigger(Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0)));
        assert_eq!(b.state, BlinkState::Pending);
    }

    #[test]
    fn blink_clamps_to_max_range() {
        let mut b = Blink::new(3.0, 1.0);
        b.trigger(Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0));
        assert!((b.target_position.x - 3.0).abs() < 0.001);
    }

    #[test]
    fn blink_cooldown_blocks_retrigger() {
        let mut b = Blink::new(10.0, 2.0);
        b.trigger(Vec3::ZERO, Vec3::X);
        b.consume();
        assert!(!b.trigger(Vec3::ZERO, Vec3::X));
    }

    #[test]
    fn blink_ready_after_cooldown() {
        let mut b = Blink::new(10.0, 1.0);
        b.trigger(Vec3::ZERO, Vec3::X);
        b.consume();
        b.tick(1.1);
        assert!(b.is_ready());
    }

    #[test]
    fn blink_disabled_rejects_trigger() {
        let mut b = Blink::new(10.0, 1.0).disabled();
        assert!(!b.trigger(Vec3::ZERO, Vec3::X));
    }
}
