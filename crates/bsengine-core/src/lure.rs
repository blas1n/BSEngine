use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Lifecycle state of a lure/decoy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LureState {
    /// Lure has not been deployed.
    Inactive,
    /// Lure is active and attracting AI agents.
    Active,
    /// Lure duration expired and is no longer attracting.
    Expired,
}

/// Decoy / bait component that attracts AI agents to a world position.
///
/// Attach to a thrown decoy entity (grenade, stone, meat) or to a fixed
/// environmental object (bait station, noisy generator). The AI perception
/// system reads `position`, `radius`, and `strength` to decide whether to
/// investigate.
///
/// Call `deploy(pos)` to activate. `tick(dt)` counts down the duration.
/// `just_activated` fires on the first frame of activation;
/// `just_expired` fires when the timer runs out.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Lure {
    pub state: LureState,
    /// World-space position that AI agents are attracted to.
    pub position: Vec3,
    /// Perception radius — how far away agents can detect the lure.
    pub radius: f32,
    /// Relative attractiveness [0, 1]; lets systems prioritize competing lures.
    pub strength: f32,
    /// Duration in seconds (0 = permanent until explicitly deactivated).
    pub duration: f32,
    pub timer: f32,
    /// True on the exact frame the lure becomes active.
    pub just_activated: bool,
    /// True on the exact frame the lure expires.
    pub just_expired: bool,
    pub enabled: bool,
}

impl Lure {
    pub fn new(radius: f32, strength: f32, duration: f32) -> Self {
        Self {
            state: LureState::Inactive,
            position: Vec3::ZERO,
            radius: radius.max(0.0),
            strength: strength.clamp(0.0, 1.0),
            duration: duration.max(0.0),
            timer: 0.0,
            just_activated: false,
            just_expired: false,
            enabled: true,
        }
    }

    /// Permanent lure (never expires on its own).
    pub fn permanent(radius: f32, strength: f32) -> Self {
        Self::new(radius, strength, 0.0)
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Deploy the lure at the given world position. No-op if already active.
    pub fn deploy(&mut self, position: Vec3) {
        if !self.enabled || self.state == LureState::Active {
            return;
        }
        self.position = position;
        self.timer = self.duration;
        self.state = LureState::Active;
        self.just_activated = true;
    }

    /// Deactivate before the natural timeout.
    pub fn deactivate(&mut self) {
        if self.state == LureState::Active {
            self.state = LureState::Inactive;
            self.timer = 0.0;
        }
    }

    /// Re-arm after expiry for reuse.
    pub fn reset(&mut self) {
        self.state = LureState::Inactive;
        self.timer = 0.0;
        self.just_activated = false;
        self.just_expired = false;
    }

    /// Advance the duration timer. Call once per frame.
    pub fn tick(&mut self, dt: f32) {
        self.just_activated = false;
        self.just_expired = false;

        if self.state != LureState::Active || self.duration <= 0.0 {
            return;
        }

        self.timer = (self.timer - dt).max(0.0);
        if self.timer <= 0.0 {
            self.state = LureState::Expired;
            self.just_expired = true;
        }
    }

    pub fn is_active(&self) -> bool {
        self.enabled && self.state == LureState::Active
    }

    /// Whether a given world position is within detection range.
    pub fn in_range(&self, pos: Vec3) -> bool {
        self.is_active() && self.position.distance_squared(pos) <= self.radius * self.radius
    }

    /// Remaining duration fraction [0, 1]. 1.0 for permanent or inactive.
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 || self.state != LureState::Active {
            1.0
        } else {
            self.timer / self.duration
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lure() -> Lure {
        Lure::new(10.0, 1.0, 5.0)
    }

    #[test]
    fn deploy_activates() {
        let mut l = lure();
        l.deploy(Vec3::ZERO);
        assert_eq!(l.state, LureState::Active);
        assert!(l.just_activated);
        assert!(l.is_active());
    }

    #[test]
    fn just_activated_clears_next_tick() {
        let mut l = lure();
        l.deploy(Vec3::ZERO);
        l.tick(0.0);
        assert!(!l.just_activated);
    }

    #[test]
    fn expires_after_duration() {
        let mut l = lure();
        l.deploy(Vec3::ZERO);
        l.tick(5.0);
        assert_eq!(l.state, LureState::Expired);
        assert!(l.just_expired);
    }

    #[test]
    fn permanent_does_not_expire() {
        let mut l = Lure::permanent(10.0, 0.8);
        l.deploy(Vec3::ZERO);
        l.tick(999.0);
        assert_eq!(l.state, LureState::Active);
        assert!(!l.just_expired);
    }

    #[test]
    fn in_range_check() {
        let mut l = lure();
        l.deploy(Vec3::ZERO);
        assert!(l.in_range(Vec3::new(5.0, 0.0, 0.0)));
        assert!(!l.in_range(Vec3::new(15.0, 0.0, 0.0)));
    }

    #[test]
    fn deactivate_before_expiry() {
        let mut l = lure();
        l.deploy(Vec3::ZERO);
        l.deactivate();
        assert_eq!(l.state, LureState::Inactive);
        assert!(!l.is_active());
    }

    #[test]
    fn disabled_blocks_deploy() {
        let mut l = lure().disabled();
        l.deploy(Vec3::ZERO);
        assert!(!l.is_active());
    }
}
