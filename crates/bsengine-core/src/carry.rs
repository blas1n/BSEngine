use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Phase of a carry interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarryPhase {
    /// No object is being held.
    None,
    /// Lifting an object toward the carry position (timed transition).
    Lifting,
    /// Object is held at the carry position.
    Carrying,
    /// Object is being released (timed transition).
    Dropping,
}

/// Physical-carry / hold component — for characters that can pick up and
/// transport objects (crates, NPCs, ragdolls, projectiles).
///
/// Attach to the **carrier**. The held object's position is driven by the
/// physics / transform system using `carry_offset` and `carry_height`.
///
/// Call `pickup(weight)` to begin carrying, `drop()` to release.
/// `tick(dt)` advances the Lifting / Dropping transitions.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Carry {
    pub phase: CarryPhase,
    /// Local-space offset from carrier origin to the hold point.
    pub carry_offset: Vec3,
    /// Maximum combined weight this carrier can hold.
    pub max_carry_weight: f32,
    /// Weight of the currently held object (0 when not carrying).
    pub current_carry_weight: f32,
    /// How long the Lifting transition takes (seconds).
    pub lift_duration: f32,
    /// How long the Dropping transition takes (seconds).
    pub drop_duration: f32,
    pub lift_timer: f32,
    pub drop_timer: f32,
    /// True on the frame Carrying begins.
    pub just_picked_up: bool,
    /// True on the frame Dropping completes (object fully released).
    pub just_dropped: bool,
    pub enabled: bool,
}

impl Carry {
    pub fn new(carry_offset: Vec3, max_carry_weight: f32) -> Self {
        Self {
            phase: CarryPhase::None,
            carry_offset,
            max_carry_weight: max_carry_weight.max(0.0),
            current_carry_weight: 0.0,
            lift_duration: 0.2,
            drop_duration: 0.1,
            lift_timer: 0.0,
            drop_timer: 0.0,
            just_picked_up: false,
            just_dropped: false,
            enabled: true,
        }
    }

    pub fn with_lift_duration(mut self, secs: f32) -> Self {
        self.lift_duration = secs.max(0.0);
        self
    }

    pub fn with_drop_duration(mut self, secs: f32) -> Self {
        self.drop_duration = secs.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Attempt to pick up an object of `weight`. Returns true if accepted.
    pub fn pickup(&mut self, weight: f32) -> bool {
        if !self.enabled || self.phase != CarryPhase::None {
            return false;
        }
        if weight > self.max_carry_weight {
            return false;
        }
        self.current_carry_weight = weight.max(0.0);
        self.phase = CarryPhase::Lifting;
        self.lift_timer = self.lift_duration;
        false // caller checks is_carrying() after tick
    }

    /// Begin releasing the held object.
    pub fn drop(&mut self) {
        if self.phase == CarryPhase::Carrying || self.phase == CarryPhase::Lifting {
            self.phase = CarryPhase::Dropping;
            self.drop_timer = self.drop_duration;
        }
    }

    /// Advance timers. Call once per frame.
    pub fn tick(&mut self, dt: f32) {
        self.just_picked_up = false;
        self.just_dropped = false;

        match self.phase {
            CarryPhase::Lifting => {
                self.lift_timer = (self.lift_timer - dt).max(0.0);
                if self.lift_timer <= 0.0 {
                    self.phase = CarryPhase::Carrying;
                    self.just_picked_up = true;
                }
            }
            CarryPhase::Dropping => {
                self.drop_timer = (self.drop_timer - dt).max(0.0);
                if self.drop_timer <= 0.0 {
                    self.phase = CarryPhase::None;
                    self.current_carry_weight = 0.0;
                    self.just_dropped = true;
                }
            }
            _ => {}
        }
    }

    pub fn is_carrying(&self) -> bool {
        self.phase == CarryPhase::Carrying
    }

    pub fn is_busy(&self) -> bool {
        self.phase != CarryPhase::None
    }

    /// Whether an object of `weight` can be picked up right now.
    pub fn can_carry(&self, weight: f32) -> bool {
        self.enabled && self.phase == CarryPhase::None && weight <= self.max_carry_weight
    }

    /// Fraction [0.0, 1.0] of the lift or drop transition (1.0 = at rest).
    pub fn transition_fraction(&self) -> f32 {
        match self.phase {
            CarryPhase::Lifting => {
                if self.lift_duration > 0.0 {
                    1.0 - self.lift_timer / self.lift_duration
                } else {
                    1.0
                }
            }
            CarryPhase::Dropping => {
                if self.drop_duration > 0.0 {
                    self.drop_timer / self.drop_duration
                } else {
                    0.0
                }
            }
            CarryPhase::Carrying => 1.0,
            CarryPhase::None => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn carry() -> Carry {
        Carry::new(Vec3::new(0.0, 1.5, 0.5), 50.0)
            .with_lift_duration(0.3)
            .with_drop_duration(0.2)
    }

    #[test]
    fn pickup_starts_lifting() {
        let mut c = carry();
        c.pickup(10.0);
        assert_eq!(c.phase, CarryPhase::Lifting);
        assert!((c.lift_timer - 0.3).abs() < 1e-5);
    }

    #[test]
    fn too_heavy_rejected() {
        let mut c = carry();
        assert!(!c.pickup(100.0));
        assert_eq!(c.phase, CarryPhase::None);
    }

    #[test]
    fn lift_completes_after_duration() {
        let mut c = carry();
        c.pickup(10.0);
        c.tick(0.3);
        assert_eq!(c.phase, CarryPhase::Carrying);
        assert!(c.just_picked_up);
        assert!(c.is_carrying());
    }

    #[test]
    fn drop_completes_after_duration() {
        let mut c = carry();
        c.pickup(10.0);
        c.tick(0.3); // finish lifting
        c.drop();
        c.tick(0.2); // finish dropping
        assert_eq!(c.phase, CarryPhase::None);
        assert!(c.just_dropped);
        assert_eq!(c.current_carry_weight, 0.0);
    }

    #[test]
    fn can_carry_checks_weight_and_phase() {
        let mut c = carry();
        assert!(c.can_carry(50.0));
        assert!(!c.can_carry(51.0));
        c.pickup(10.0);
        assert!(!c.can_carry(5.0)); // busy
    }

    #[test]
    fn disabled_ignores_pickup() {
        let mut c = carry().disabled();
        c.pickup(10.0);
        assert_eq!(c.phase, CarryPhase::None);
    }
}
