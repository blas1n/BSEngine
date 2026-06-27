use bevy_ecs::prelude::Component;

/// Tracks the knocked-down / prone state of an entity.
///
/// While prone the entity cannot move normally (`movement_penalty` multiplies
/// speed) and attacks are impaired (`attack_penalty` multiplies attack rate or
/// damage). A stand-up animation takes `stand_up_duration` seconds; call
/// `begin_stand_up()` when the input or AI decides to recover.
///
/// State machine: Standing → Prone (via `fall()`) → StandingUp (via
/// `begin_stand_up()`) → Standing (when timer reaches 0).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Prone {
    pub is_prone: bool,
    /// Total time to stand up in seconds.
    pub stand_up_duration: f32,
    /// Remaining stand-up time. 0 when not standing up.
    pub stand_up_timer: f32,
    /// Speed multiplier while prone [0.0, 1.0]. 0.0 = cannot move at all.
    pub movement_penalty: f32,
    /// Attack rate / damage multiplier while prone [0.0, 1.0].
    pub attack_penalty: f32,
    pub just_fell_prone: bool,
    pub just_stood_up: bool,
    pub enabled: bool,
}

impl Prone {
    pub fn new(stand_up_duration: f32) -> Self {
        Self {
            is_prone: false,
            stand_up_duration: stand_up_duration.max(0.0),
            stand_up_timer: 0.0,
            movement_penalty: 0.0,
            attack_penalty: 0.5,
            just_fell_prone: false,
            just_stood_up: false,
            enabled: true,
        }
    }

    pub fn with_movement_penalty(mut self, penalty: f32) -> Self {
        self.movement_penalty = penalty.clamp(0.0, 1.0);
        self
    }

    pub fn with_attack_penalty(mut self, penalty: f32) -> Self {
        self.attack_penalty = penalty.clamp(0.0, 1.0);
        self
    }

    /// Immediately knock the entity prone. Cancels any stand-up in progress.
    pub fn fall(&mut self) {
        if !self.enabled || self.is_prone {
            return;
        }
        self.is_prone = true;
        self.stand_up_timer = 0.0;
        self.just_fell_prone = true;
    }

    /// Begin standing up. Only valid while prone and not already standing up.
    /// Returns `true` if the stand-up was started.
    pub fn begin_stand_up(&mut self) -> bool {
        if !self.enabled || !self.is_prone || self.is_standing_up() {
            return false;
        }
        self.stand_up_timer = self.stand_up_duration;
        true
    }

    /// Instantly stand up (e.g. for low-latency recovery items).
    pub fn force_stand_up(&mut self) {
        if self.is_prone {
            self.is_prone = false;
            self.stand_up_timer = 0.0;
            self.just_stood_up = true;
        }
    }

    /// Advance timers; sets `just_stood_up` when the stand-up animation ends.
    pub fn tick(&mut self, dt: f32) {
        self.just_fell_prone = false;
        self.just_stood_up = false;

        if self.stand_up_timer > 0.0 {
            self.stand_up_timer -= dt;
            if self.stand_up_timer <= 0.0 {
                self.stand_up_timer = 0.0;
                self.is_prone = false;
                self.just_stood_up = true;
            }
        }
    }

    pub fn is_standing_up(&self) -> bool {
        self.stand_up_timer > 0.0
    }

    pub fn is_standing(&self) -> bool {
        !self.is_prone
    }

    /// Stand-up progress [0.0 = just started, 1.0 = complete].
    pub fn stand_up_fraction(&self) -> f32 {
        if self.stand_up_duration <= 0.0 || !self.is_standing_up() {
            return 0.0;
        }
        1.0 - (self.stand_up_timer / self.stand_up_duration).clamp(0.0, 1.0)
    }
}

impl Default for Prone {
    fn default() -> Self {
        Self::new(0.8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fall_sets_prone() {
        let mut p = Prone::new(0.5);
        p.fall();
        assert!(p.is_prone);
        assert!(p.just_fell_prone);
        assert!(!p.is_standing());
    }

    #[test]
    fn begin_stand_up_starts_timer() {
        let mut p = Prone::new(0.5);
        p.fall();
        let ok = p.begin_stand_up();
        assert!(ok);
        assert!(p.is_standing_up());
    }

    #[test]
    fn begin_stand_up_rejects_when_not_prone() {
        let mut p = Prone::new(0.5);
        let ok = p.begin_stand_up();
        assert!(!ok);
    }

    #[test]
    fn tick_completes_stand_up() {
        let mut p = Prone::new(0.5);
        p.fall();
        p.begin_stand_up();
        p.tick(0.6);
        assert!(!p.is_prone);
        assert!(p.just_stood_up);
        assert!(p.is_standing());
    }

    #[test]
    fn force_stand_up_immediate() {
        let mut p = Prone::new(1.0);
        p.fall();
        p.force_stand_up();
        assert!(!p.is_prone);
        assert!(p.just_stood_up);
    }

    #[test]
    fn fall_no_op_when_already_prone() {
        let mut p = Prone::new(0.5);
        p.fall();
        p.tick(0.016); // clear flag
        p.fall();
        assert!(!p.just_fell_prone); // no second event
    }

    #[test]
    fn stand_up_fraction_midpoint() {
        let mut p = Prone::new(1.0);
        p.fall();
        p.begin_stand_up();
        p.tick(0.5);
        let frac = p.stand_up_fraction();
        assert!((frac - 0.5).abs() < 0.01);
    }

    #[test]
    fn disabled_fall_no_op() {
        let mut p = Prone::new(0.5);
        p.enabled = false;
        p.fall();
        assert!(!p.is_prone);
    }
}
