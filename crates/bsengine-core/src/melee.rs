use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Phase of a melee strike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeleePhase {
    /// Weapon at rest; accepting input.
    Idle,
    /// Pre-strike build-up (animation, tracking target).
    Windup,
    /// Hitbox active; dealing damage.
    Active,
    /// Post-strike cooldown before next attack.
    Recovery,
}

/// Melee attack component — swing arc, timing, and combo tracking.
///
/// The combat system calls `begin()` when the player attacks. Each frame it calls
/// `tick(dt)` to advance phases. During `Active`, the system checks entities within
/// `reach` and within `arc_angle` of `attack_direction` for hits.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Melee {
    pub phase: MeleePhase,
    /// World-space direction the attack is aimed (normalised by the combat system).
    pub attack_direction: Vec3,
    /// Maximum reach of the attack (metres from entity origin).
    pub reach: f32,
    /// Half-angle of the attack arc (radians). π = 360° sweep.
    pub arc_angle: f32,
    /// Duration of windup before the hitbox activates (seconds).
    pub windup_time: f32,
    /// Duration the hitbox is active (seconds).
    pub active_time: f32,
    /// Duration of recovery before the next attack can begin (seconds).
    pub recovery_time: f32,
    /// Time remaining in the current phase.
    pub timer: f32,
    /// Number of hits registered this swing (reset on each new attack).
    pub hit_count: u32,
    /// Maximum hits per swing (0 = unlimited, e.g. piercing).
    pub max_hits: u32,
    /// Combo step (0 = first strike). Combat system increments before calling `begin()`.
    pub combo_step: u32,
    /// True when the player presses attack during Active/Recovery to queue the next swing.
    pub combo_buffered: bool,
    /// Whether a new attack can be triggered while already in recovery.
    pub can_cancel_recovery: bool,
    pub enabled: bool,
}

impl Melee {
    pub fn new(reach: f32, windup: f32, active: f32, recovery: f32) -> Self {
        Self {
            phase: MeleePhase::Idle,
            attack_direction: Vec3::Z,
            reach: reach.max(0.0),
            arc_angle: std::f32::consts::FRAC_PI_4, // 45° half-angle
            windup_time: windup.max(0.0),
            active_time: active.max(0.0),
            recovery_time: recovery.max(0.0),
            timer: 0.0,
            hit_count: 0,
            max_hits: 0,
            combo_step: 0,
            combo_buffered: false,
            can_cancel_recovery: false,
            enabled: true,
        }
    }

    pub fn with_arc(mut self, half_angle_radians: f32) -> Self {
        self.arc_angle = half_angle_radians.abs();
        self
    }

    pub fn with_max_hits(mut self, n: u32) -> Self {
        self.max_hits = n;
        self
    }

    pub fn with_cancel_recovery(mut self) -> Self {
        self.can_cancel_recovery = true;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Begin an attack. Returns true if accepted.
    pub fn begin(&mut self, direction: Vec3) -> bool {
        if !self.enabled {
            return false;
        }
        match self.phase {
            MeleePhase::Idle => {}
            MeleePhase::Recovery if self.can_cancel_recovery => {}
            _ => return false,
        }
        self.attack_direction = direction.normalize_or_zero();
        self.phase = MeleePhase::Windup;
        self.timer = self.windup_time;
        self.hit_count = 0;
        self.combo_buffered = false;
        true
    }

    /// Advance the attack state machine. Call every frame.
    pub fn tick(&mut self, dt: f32) {
        if !self.enabled {
            return;
        }

        match self.phase {
            MeleePhase::Idle => {}
            MeleePhase::Windup => {
                self.timer -= dt;
                if self.timer <= 0.0 {
                    self.phase = MeleePhase::Active;
                    self.timer = self.active_time;
                }
            }
            MeleePhase::Active => {
                self.timer -= dt;
                if self.timer <= 0.0 {
                    self.phase = MeleePhase::Recovery;
                    self.timer = self.recovery_time;
                }
            }
            MeleePhase::Recovery => {
                self.timer -= dt;
                if self.timer <= 0.0 {
                    self.phase = MeleePhase::Idle;
                    self.timer = 0.0;
                    self.combo_step = 0;
                }
            }
        }
    }

    /// Register a hit this swing. Returns false if max_hits has been reached.
    pub fn register_hit(&mut self) -> bool {
        if self.max_hits > 0 && self.hit_count >= self.max_hits {
            return false;
        }
        self.hit_count += 1;
        true
    }

    /// True when the hitbox is active and can deal damage.
    pub fn is_active(&self) -> bool {
        self.phase == MeleePhase::Active && (self.max_hits == 0 || self.hit_count < self.max_hits)
    }

    /// True if `target_dir` is within the attack arc.
    pub fn in_arc(&self, target_dir: Vec3) -> bool {
        let cos = self.attack_direction.dot(target_dir.normalize_or_zero());
        cos >= self.arc_angle.cos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_transitions_to_windup() {
        let mut m = Melee::new(2.0, 0.2, 0.15, 0.3);
        let ok = m.begin(Vec3::Z);
        assert!(ok);
        assert_eq!(m.phase, MeleePhase::Windup);
    }

    #[test]
    fn tick_advances_through_all_phases() {
        let mut m = Melee::new(2.0, 0.1, 0.1, 0.1);
        m.begin(Vec3::Z);
        m.tick(0.15); // past windup
        assert_eq!(m.phase, MeleePhase::Active);
        m.tick(0.15); // past active
        assert_eq!(m.phase, MeleePhase::Recovery);
        m.tick(0.15); // past recovery
        assert_eq!(m.phase, MeleePhase::Idle);
    }

    #[test]
    fn max_hits_blocks_is_active_after_limit() {
        let mut m = Melee::new(2.0, 0.0, 0.5, 0.2).with_max_hits(1);
        m.begin(Vec3::Z);
        m.tick(0.05); // enter active
        m.register_hit();
        assert!(!m.is_active());
    }

    #[test]
    fn in_arc_returns_true_for_frontal_target() {
        let m = Melee::new(2.0, 0.1, 0.1, 0.1).with_arc(std::f32::consts::FRAC_PI_4);
        assert!(m.in_arc(Vec3::Z)); // straight ahead
    }

    #[test]
    fn in_arc_returns_false_for_rear_target() {
        let m = Melee::new(2.0, 0.1, 0.1, 0.1).with_arc(std::f32::consts::FRAC_PI_4);
        assert!(!m.in_arc(-Vec3::Z)); // directly behind
    }

    #[test]
    fn begin_rejected_during_active() {
        let mut m = Melee::new(2.0, 0.0, 0.5, 0.2);
        m.begin(Vec3::Z);
        m.tick(0.05); // enter active
        let ok = m.begin(Vec3::Z);
        assert!(!ok);
    }
}
