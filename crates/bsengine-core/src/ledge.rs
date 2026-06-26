use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Phase of a ledge-grab interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgePhase {
    /// No ledge interaction active.
    None,
    /// Character is hanging from the ledge edge.
    Hanging,
    /// Character is pulling up onto the ledge.
    ClimbingUp,
    /// Character is dropping down from the ledge.
    Dropping,
}

/// Ledge-grab state for a character entity.
///
/// The ledge-detection system writes `ledge_normal`, `hang_position`, and
/// calls `grab()`/`release()` when a suitable ledge is found or lost.
/// `tick(dt)` advances the climb animation timer.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Ledge {
    pub phase: LedgePhase,
    /// World-space position the character's hands grip.
    pub hang_position: Vec3,
    /// Outward normal of the ledge surface (points away from the wall).
    pub ledge_normal: Vec3,
    /// How long the climb-up animation takes in seconds.
    pub climb_duration: f32,
    /// Elapsed time into the current climb animation.
    pub climb_timer: f32,
    /// Maximum downward distance from the character's feet to detect a ledge.
    pub detection_range: f32,
    /// Whether the character can grab ledges.
    pub can_grab: bool,
    pub enabled: bool,
}

impl Ledge {
    pub fn new() -> Self {
        Self {
            phase: LedgePhase::None,
            hang_position: Vec3::ZERO,
            ledge_normal: Vec3::Z,
            climb_duration: 0.4,
            climb_timer: 0.0,
            detection_range: 0.6,
            can_grab: true,
            enabled: true,
        }
    }

    pub fn with_climb_duration(mut self, seconds: f32) -> Self {
        self.climb_duration = seconds.max(0.01);
        self
    }

    pub fn with_detection_range(mut self, range: f32) -> Self {
        self.detection_range = range.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Attach to a ledge at `hang_pos` with outward normal `normal`.
    /// No-op if already hanging or grabbing is disabled.
    pub fn grab(&mut self, hang_pos: Vec3, normal: Vec3) {
        if !self.enabled || !self.can_grab || self.phase != LedgePhase::None {
            return;
        }
        self.hang_position = hang_pos;
        self.ledge_normal = normal.normalize_or_zero();
        self.phase = LedgePhase::Hanging;
    }

    /// Begin climbing up from Hanging.
    pub fn climb_up(&mut self) {
        if self.phase == LedgePhase::Hanging {
            self.phase = LedgePhase::ClimbingUp;
            self.climb_timer = 0.0;
        }
    }

    /// Begin dropping from Hanging.
    pub fn drop(&mut self) {
        if self.phase == LedgePhase::Hanging {
            self.phase = LedgePhase::Dropping;
        }
    }

    /// Release from the ledge (any phase → None).
    pub fn release(&mut self) {
        self.phase = LedgePhase::None;
        self.climb_timer = 0.0;
    }

    /// Advance climb timer. Returns `true` when ClimbingUp finishes.
    pub fn tick(&mut self, dt: f32) -> bool {
        if self.phase == LedgePhase::ClimbingUp {
            self.climb_timer += dt;
            if self.climb_timer >= self.climb_duration {
                self.release();
                return true;
            }
        }
        false
    }

    pub fn is_active(&self) -> bool {
        self.phase != LedgePhase::None
    }

    /// Fraction of climb completed [0, 1]. 0 outside ClimbingUp.
    pub fn climb_fraction(&self) -> f32 {
        if self.phase != LedgePhase::ClimbingUp || self.climb_duration <= 0.0 {
            return 0.0;
        }
        (self.climb_timer / self.climb_duration).clamp(0.0, 1.0)
    }
}

impl Default for Ledge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grab_sets_hanging_phase() {
        let mut l = Ledge::new();
        l.grab(Vec3::new(0.0, 2.0, 0.0), Vec3::Z);
        assert_eq!(l.phase, LedgePhase::Hanging);
        assert!(l.is_active());
    }

    #[test]
    fn climb_up_finishes_after_duration() {
        let mut l = Ledge::new().with_climb_duration(1.0);
        l.grab(Vec3::ZERO, Vec3::Z);
        l.climb_up();
        assert!(!l.tick(0.5));
        assert!(l.tick(0.6));
        assert_eq!(l.phase, LedgePhase::None);
    }

    #[test]
    fn drop_from_hanging() {
        let mut l = Ledge::new();
        l.grab(Vec3::ZERO, Vec3::Z);
        l.drop();
        assert_eq!(l.phase, LedgePhase::Dropping);
    }

    #[test]
    fn release_clears_phase() {
        let mut l = Ledge::new();
        l.grab(Vec3::ZERO, Vec3::Z);
        l.release();
        assert!(!l.is_active());
    }

    #[test]
    fn disabled_cannot_grab() {
        let mut l = Ledge::new().disabled();
        l.grab(Vec3::ZERO, Vec3::Z);
        assert_eq!(l.phase, LedgePhase::None);
    }
}
