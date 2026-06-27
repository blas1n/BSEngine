use bevy_ecs::prelude::Component;

/// Phase of a ground-slam (ground-pound) attack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlamPhase {
    /// On the ground or not charged; ready to initiate if airborne.
    Idle,
    /// Rising / at peak, waiting for player input to trigger the slam.
    Charged,
    /// Plummeting downward at `slam_speed`.
    Descending,
    /// Hit the ground; impact area active for one frame.
    Impact,
    /// Post-impact recovery before the next slam.
    Cooldown,
}

/// Ground-slam (ground-pound) attack component.
///
/// The movement system sets `is_airborne` and `height_above_ground` each frame.
/// When `wants_slam` is true and the character is airborne above `min_height`,
/// `begin()` initiates the slam. The physics system applies `slam_speed`
/// downward until the character lands; call `land()` to trigger Impact.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Slam {
    pub phase: SlamPhase,
    /// Downward speed during the plummet (m/s, positive).
    pub slam_speed: f32,
    /// Horizontal radius of the impact area-of-effect (metres).
    pub impact_radius: f32,
    /// Outward impulse magnitude applied to entities within `impact_radius`.
    pub impact_force: f32,
    /// Minimum height above ground (metres) required to initiate a slam.
    pub min_height: f32,
    /// Height above ground when the slam was initiated (for damage scaling).
    pub launch_height: f32,
    /// Duration of the post-impact freeze / landing animation (seconds).
    pub recovery_time: f32,
    pub recovery_timer: f32,
    /// Cooldown before the next slam (seconds).
    pub cooldown: f32,
    pub cooldown_timer: f32,
    /// Input flag — set by the input system; read by `begin()`.
    pub wants_slam: bool,
    pub enabled: bool,
}

impl Slam {
    pub fn new(slam_speed: f32, impact_radius: f32, impact_force: f32) -> Self {
        Self {
            phase: SlamPhase::Idle,
            slam_speed: slam_speed.max(0.0),
            impact_radius: impact_radius.max(0.0),
            impact_force: impact_force.max(0.0),
            min_height: 1.0,
            launch_height: 0.0,
            recovery_time: 0.3,
            recovery_timer: 0.0,
            cooldown: 0.5,
            cooldown_timer: 0.0,
            wants_slam: false,
            enabled: true,
        }
    }

    pub fn with_min_height(mut self, h: f32) -> Self {
        self.min_height = h.max(0.0);
        self
    }

    pub fn with_recovery(mut self, t: f32) -> Self {
        self.recovery_time = t.max(0.0);
        self
    }

    pub fn with_cooldown(mut self, t: f32) -> Self {
        self.cooldown = t.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Initiate the slam. `height` is the current height above ground (metres).
    /// Returns true if accepted.
    pub fn begin(&mut self, height: f32) -> bool {
        if !self.enabled || self.phase != SlamPhase::Idle {
            return false;
        }
        if height < self.min_height {
            return false;
        }
        self.launch_height = height;
        self.phase = SlamPhase::Descending;
        self.wants_slam = false;
        true
    }

    /// Called by the physics system when the character hits the ground.
    pub fn land(&mut self) {
        if self.phase == SlamPhase::Descending {
            self.phase = SlamPhase::Impact;
            self.recovery_timer = self.recovery_time;
        }
    }

    /// Advance timers. Call once per frame after resolving `land()`.
    pub fn tick(&mut self, dt: f32) {
        if !self.enabled {
            return;
        }
        match self.phase {
            SlamPhase::Impact => {
                self.recovery_timer -= dt;
                if self.recovery_timer <= 0.0 {
                    self.phase = SlamPhase::Cooldown;
                    self.cooldown_timer = self.cooldown;
                }
            }
            SlamPhase::Cooldown => {
                self.cooldown_timer -= dt;
                if self.cooldown_timer <= 0.0 {
                    self.phase = SlamPhase::Idle;
                }
            }
            _ => {}
        }
    }

    pub fn is_descending(&self) -> bool {
        self.phase == SlamPhase::Descending
    }

    pub fn just_landed(&self) -> bool {
        self.phase == SlamPhase::Impact
    }

    pub fn is_available(&self) -> bool {
        self.phase == SlamPhase::Idle
    }

    /// Damage multiplier based on fall height (higher launch → more damage).
    pub fn height_multiplier(&self, base_height: f32) -> f32 {
        if base_height <= 0.0 {
            return 1.0;
        }
        (self.launch_height / base_height).clamp(1.0, 3.0)
    }

    pub fn cooldown_fraction(&self) -> f32 {
        if self.cooldown > 0.0 {
            (self.cooldown_timer / self.cooldown).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_sets_descending() {
        let mut s = Slam::new(20.0, 3.0, 500.0);
        let ok = s.begin(5.0);
        assert!(ok);
        assert_eq!(s.phase, SlamPhase::Descending);
    }

    #[test]
    fn begin_rejected_below_min_height() {
        let mut s = Slam::new(20.0, 3.0, 500.0).with_min_height(2.0);
        let ok = s.begin(1.5);
        assert!(!ok);
    }

    #[test]
    fn land_transitions_to_impact() {
        let mut s = Slam::new(20.0, 3.0, 500.0);
        s.begin(5.0);
        s.land();
        assert!(s.just_landed());
    }

    #[test]
    fn tick_advances_impact_to_cooldown() {
        let mut s = Slam::new(20.0, 3.0, 500.0).with_recovery(0.1);
        s.begin(5.0);
        s.land();
        s.tick(0.2);
        assert_eq!(s.phase, SlamPhase::Cooldown);
    }

    #[test]
    fn height_multiplier_clamped() {
        let mut s = Slam::new(20.0, 3.0, 500.0);
        s.launch_height = 100.0;
        assert_eq!(s.height_multiplier(1.0), 3.0); // clamped at max
        s.launch_height = 0.5;
        assert_eq!(s.height_multiplier(1.0), 1.0); // clamped at min
    }

    #[test]
    fn full_cycle_returns_to_idle() {
        let mut s = Slam::new(20.0, 3.0, 500.0)
            .with_recovery(0.1)
            .with_cooldown(0.1);
        s.begin(5.0);
        s.land();
        s.tick(0.15); // recovery → cooldown
        s.tick(0.15); // cooldown → idle
        assert_eq!(s.phase, SlamPhase::Idle);
    }
}
