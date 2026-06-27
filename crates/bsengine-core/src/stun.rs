use bevy_ecs::prelude::Component;

/// Severity tier of a stun.
///
/// The combat system maps severity to which actions remain available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StunSeverity {
    /// Brief flinch — movement partially impaired.
    Light,
    /// Full interrupt — movement stops, actions cancelled.
    Heavy,
    /// Total incapacitation — entity is fully helpless.
    Knockdown,
}

/// Full-incapacitation stun component.
///
/// Distinct from `Stagger` (a brief hitlag flinch that only interrupts
/// attacks). `Stun` models longer, more debilitating incapacitations such
/// as electrocution, concussion, or knockdown.
///
/// A new stun from `apply(duration, severity)` REPLACES the current stun
/// only if it is longer or more severe than the active one. This prevents
/// minor stuns from cutting short a strong knockdown.
///
/// `just_stunned` fires on the exact frame a stun begins;
/// `just_recovered` fires on the exact frame it ends.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Stun {
    pub severity: StunSeverity,
    /// Remaining stun duration in seconds.
    pub timer: f32,
    /// True on the first frame a stun becomes active.
    pub just_stunned: bool,
    /// True on the first frame the stun expires.
    pub just_recovered: bool,
    pub enabled: bool,
}

impl Stun {
    pub fn new() -> Self {
        Self {
            severity: StunSeverity::Light,
            timer: 0.0,
            just_stunned: false,
            just_recovered: false,
            enabled: true,
        }
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Apply a stun. Replaces the current one only when the new stun is
    /// more severe OR would last longer than what remains.
    pub fn apply(&mut self, duration: f32, severity: StunSeverity) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        let was_stunned = self.is_active();
        if !was_stunned || severity > self.severity || duration > self.timer {
            self.timer = duration;
            self.severity = severity;
            if !was_stunned {
                self.just_stunned = true;
            }
        }
    }

    /// Force the stun to end immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.just_recovered = true;
        }
    }

    /// Advance the stun timer. Call once per frame.
    pub fn tick(&mut self, dt: f32) {
        self.just_stunned = false;
        self.just_recovered = false;

        if !self.enabled || self.timer <= 0.0 {
            return;
        }

        self.timer = (self.timer - dt).max(0.0);
        if self.timer <= 0.0 {
            self.just_recovered = true;
        }
    }

    pub fn is_active(&self) -> bool {
        self.enabled && self.timer > 0.0
    }
}

impl Default for Stun {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_starts_stun() {
        let mut s = Stun::new();
        s.apply(2.0, StunSeverity::Heavy);
        assert!(s.is_active());
        assert!(s.just_stunned);
    }

    #[test]
    fn tick_expires_stun() {
        let mut s = Stun::new();
        s.apply(1.0, StunSeverity::Light);
        s.tick(0.0); // clear just_stunned
        s.tick(1.0);
        assert!(!s.is_active());
        assert!(s.just_recovered);
    }

    #[test]
    fn stronger_severity_overrides() {
        let mut s = Stun::new();
        s.apply(1.0, StunSeverity::Light);
        s.tick(0.0);
        s.apply(0.5, StunSeverity::Knockdown);
        assert_eq!(s.severity, StunSeverity::Knockdown);
        assert!((s.timer - 0.5).abs() < 1e-5);
    }

    #[test]
    fn weaker_severity_does_not_override() {
        let mut s = Stun::new();
        s.apply(2.0, StunSeverity::Heavy);
        s.tick(0.0);
        s.apply(0.3, StunSeverity::Light);
        assert_eq!(s.severity, StunSeverity::Heavy);
        assert!((s.timer - 2.0).abs() < 1e-5);
    }

    #[test]
    fn longer_duration_overrides_same_severity() {
        let mut s = Stun::new();
        s.apply(1.0, StunSeverity::Light);
        s.tick(0.0);
        s.apply(3.0, StunSeverity::Light);
        assert!((s.timer - 3.0).abs() < 1e-5);
    }

    #[test]
    fn clear_fires_recovered() {
        let mut s = Stun::new();
        s.apply(5.0, StunSeverity::Heavy);
        s.tick(0.0);
        s.clear();
        assert!(!s.is_active());
        assert!(s.just_recovered);
    }

    #[test]
    fn disabled_blocks_apply() {
        let mut s = Stun::new().disabled();
        s.apply(1.0, StunSeverity::Heavy);
        assert!(!s.is_active());
    }
}
