use bevy_ecs::prelude::Component;

/// Taunt / provoke mechanic — forces nearby enemies to redirect aggro.
///
/// The aggro system checks `is_active()` and, when true, applies
/// `threat_boost` as an additional flat multiplier to the taunting entity's
/// threat value in every `Aggro` component within `radius`.
///
/// A taunt can be permanent (duration=0) or temporary (duration>0). Temporary
/// taunts count down through `tick(dt)` and fire `just_expired` on the first
/// frame they end.
///
/// Works alongside the existing `Aggro` component — `Taunt` marks the
/// *source* of the provocation; `Aggro` stores the threat table on enemies.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Taunt {
    pub is_active: bool,
    /// Radius within which enemy `Aggro` components are overridden (world units).
    pub radius: f32,
    /// Flat bonus added to this entity's threat entry in affected `Aggro` tables.
    pub threat_boost: f32,
    /// Total taunt duration (0.0 = permanent while active).
    pub duration: f32,
    /// Remaining time in seconds.
    pub timer: f32,
    /// True on the first frame the taunt activates.
    pub just_activated: bool,
    /// True on the first frame the taunt expires naturally.
    pub just_expired: bool,
    pub enabled: bool,
}

impl Taunt {
    /// Temporary taunt that automatically expires after `duration` seconds.
    pub fn new(radius: f32, threat_boost: f32, duration: f32) -> Self {
        Self {
            is_active: false,
            radius: radius.max(0.0),
            threat_boost: threat_boost.max(0.0),
            duration: duration.max(0.0),
            timer: 0.0,
            just_activated: false,
            just_expired: false,
            enabled: true,
        }
    }

    /// Permanent taunt — stays active until `deactivate()` is called.
    pub fn permanent(radius: f32, threat_boost: f32) -> Self {
        Self::new(radius, threat_boost, 0.0)
    }

    pub fn with_boost(mut self, boost: f32) -> Self {
        self.threat_boost = boost.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Begin taunting. If already active this is a no-op.
    pub fn activate(&mut self) {
        if !self.enabled || self.is_active {
            return;
        }
        self.is_active = true;
        self.timer = self.duration;
        self.just_activated = true;
    }

    /// End the taunt immediately.
    pub fn deactivate(&mut self) {
        self.is_active = false;
        self.timer = 0.0;
    }

    /// Advance the taunt timer. Fires `just_expired` when a timed taunt ends.
    pub fn tick(&mut self, dt: f32) {
        self.just_activated = false;
        self.just_expired = false;

        if !self.enabled || !self.is_active {
            return;
        }

        if self.duration > 0.0 {
            self.timer = (self.timer - dt).max(0.0);
            if self.timer <= 0.0 {
                self.is_active = false;
                self.just_expired = true;
            }
        }
    }

    /// True only while the taunt is enabled and actively running.
    pub fn is_taunting(&self) -> bool {
        self.enabled && self.is_active
    }

    /// Fraction of the taunt duration remaining (0.0 when permanent).
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        self.timer / self.duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activate_fires_just_activated() {
        let mut t = Taunt::new(10.0, 2.0, 3.0);
        t.activate();
        assert!(t.is_taunting());
        assert!(t.just_activated);
    }

    #[test]
    fn tick_clears_just_activated() {
        let mut t = Taunt::new(10.0, 2.0, 3.0);
        t.activate();
        t.tick(0.0);
        assert!(!t.just_activated);
    }

    #[test]
    fn timed_taunt_expires() {
        let mut t = Taunt::new(10.0, 2.0, 1.0);
        t.activate();
        t.tick(0.0);
        t.tick(1.0);
        assert!(!t.is_taunting());
        assert!(t.just_expired);
    }

    #[test]
    fn permanent_taunt_does_not_expire() {
        let mut t = Taunt::permanent(10.0, 2.0);
        t.activate();
        t.tick(100.0);
        assert!(t.is_taunting());
        assert!(!t.just_expired);
    }

    #[test]
    fn deactivate_ends_taunt() {
        let mut t = Taunt::permanent(10.0, 2.0);
        t.activate();
        t.deactivate();
        assert!(!t.is_taunting());
    }

    #[test]
    fn disabled_blocks_activate() {
        let mut t = Taunt::new(10.0, 2.0, 3.0).disabled();
        t.activate();
        assert!(!t.is_taunting());
    }

    #[test]
    fn remaining_fraction_decreases() {
        let mut t = Taunt::new(10.0, 2.0, 4.0);
        t.activate();
        t.tick(0.0);
        t.tick(2.0); // 2s of 4s consumed
        assert!((t.remaining_fraction() - 0.5).abs() < 1e-5);
    }
}
