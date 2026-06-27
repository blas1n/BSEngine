use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Phase of a plunge / slam-dive cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlungePhase {
    /// Ready; no plunge in progress.
    Idle,
    /// Pre-dive stall/hang at apex (optional wind-up).
    Hang,
    /// Diving downward at high speed.
    Diving,
    /// Ground-impact recovery animation.
    Recovery,
}

/// Downward plunge-attack / dive-bomb component.
///
/// Models the "stomp / slam-down" mechanic common in platformers and action
/// games — character launches downward at speed, dealing impact damage on
/// landing, then enters a recovery window.
///
/// Distinct from `fall` (uncontrolled descent) and `dash` (horizontal burst).
/// `Plunge` is intentional, directional, and combat-capable.
///
/// Call `trigger()` to start the hang frame, then `tick(dt)` drives the
/// phase. Set `direction` before triggering for non-vertical slams.
/// `just_landed` fires for one frame when `Diving → Recovery`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Plunge {
    pub phase: PlungePhase,
    /// Normalized aim direction (defaults to straight down).
    pub direction: Vec3,
    /// Speed applied while diving (units/s).
    pub dive_speed: f32,
    /// Damage multiplier applied to the impact hit.
    pub damage_multiplier: f32,
    /// Radial impulse radius for ground-impact shockwave (0 = no AoE).
    pub impact_radius: f32,
    /// Optional hang-stall duration at apex (seconds, 0 = instant).
    pub hang_duration: f32,
    pub hang_timer: f32,
    /// Recovery duration after landing (seconds).
    pub recovery_duration: f32,
    pub recovery_timer: f32,
    /// True on the exact frame the entity hits the ground.
    pub just_landed: bool,
    pub enabled: bool,
}

impl Plunge {
    pub fn new(dive_speed: f32, recovery_duration: f32) -> Self {
        Self {
            phase: PlungePhase::Idle,
            direction: Vec3::NEG_Y,
            dive_speed: dive_speed.max(0.0),
            damage_multiplier: 1.0,
            impact_radius: 0.0,
            hang_duration: 0.0,
            hang_timer: 0.0,
            recovery_duration: recovery_duration.max(0.0),
            recovery_timer: 0.0,
            just_landed: false,
            enabled: true,
        }
    }

    pub fn with_hang(mut self, duration: f32) -> Self {
        self.hang_duration = duration.max(0.0);
        self
    }

    pub fn with_damage(mut self, multiplier: f32) -> Self {
        self.damage_multiplier = multiplier.max(0.0);
        self
    }

    pub fn with_impact_radius(mut self, radius: f32) -> Self {
        self.impact_radius = radius.max(0.0);
        self
    }

    pub fn with_direction(mut self, dir: Vec3) -> Self {
        self.direction = if dir.length_squared() > 0.0 {
            dir.normalize()
        } else {
            Vec3::NEG_Y
        };
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Start a plunge. Returns false if disabled or already in progress.
    pub fn trigger(&mut self) -> bool {
        if !self.enabled || self.phase != PlungePhase::Idle {
            return false;
        }
        if self.hang_duration > 0.0 {
            self.phase = PlungePhase::Hang;
            self.hang_timer = self.hang_duration;
        } else {
            self.phase = PlungePhase::Diving;
        }
        true
    }

    /// Notify the component that the entity touched the ground while diving.
    /// Triggers `Recovery` and sets `just_landed` for one frame.
    pub fn on_land(&mut self) {
        if self.phase == PlungePhase::Diving {
            self.phase = PlungePhase::Recovery;
            self.recovery_timer = self.recovery_duration;
            self.just_landed = true;
        }
    }

    /// Advance phase timers. Call once per frame.
    pub fn tick(&mut self, dt: f32) {
        self.just_landed = false;

        match self.phase {
            PlungePhase::Hang => {
                self.hang_timer = (self.hang_timer - dt).max(0.0);
                if self.hang_timer <= 0.0 {
                    self.phase = PlungePhase::Diving;
                }
            }
            PlungePhase::Recovery => {
                self.recovery_timer = (self.recovery_timer - dt).max(0.0);
                if self.recovery_timer <= 0.0 {
                    self.phase = PlungePhase::Idle;
                }
            }
            PlungePhase::Idle | PlungePhase::Diving => {}
        }
    }

    pub fn is_diving(&self) -> bool {
        self.phase == PlungePhase::Diving
    }

    pub fn is_recovering(&self) -> bool {
        self.phase == PlungePhase::Recovery
    }

    pub fn is_busy(&self) -> bool {
        !matches!(self.phase, PlungePhase::Idle)
    }

    /// [0, 1] recovery progress (1 = fully recovered).
    pub fn recovery_fraction(&self) -> f32 {
        if self.recovery_duration > 0.0 {
            1.0 - self.recovery_timer / self.recovery_duration
        } else {
            1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plunge() -> Plunge {
        Plunge::new(30.0, 0.5)
    }

    #[test]
    fn trigger_enters_diving() {
        let mut p = plunge();
        assert!(p.trigger());
        assert_eq!(p.phase, PlungePhase::Diving);
    }

    #[test]
    fn hang_delays_dive() {
        let mut p = Plunge::new(30.0, 0.5).with_hang(0.2);
        p.trigger();
        assert_eq!(p.phase, PlungePhase::Hang);
        p.tick(0.2);
        assert_eq!(p.phase, PlungePhase::Diving);
    }

    #[test]
    fn on_land_starts_recovery() {
        let mut p = plunge();
        p.trigger();
        p.on_land();
        assert_eq!(p.phase, PlungePhase::Recovery);
        assert!(p.just_landed);
    }

    #[test]
    fn just_landed_clears_next_tick() {
        let mut p = plunge();
        p.trigger();
        p.on_land();
        p.tick(0.0);
        assert!(!p.just_landed);
    }

    #[test]
    fn recovery_expires_to_idle() {
        let mut p = plunge();
        p.trigger();
        p.on_land();
        p.tick(0.5);
        assert_eq!(p.phase, PlungePhase::Idle);
    }

    #[test]
    fn disabled_blocks_trigger() {
        let mut p = plunge().disabled();
        assert!(!p.trigger());
    }

    #[test]
    fn double_trigger_blocked() {
        let mut p = plunge();
        assert!(p.trigger());
        assert!(!p.trigger());
    }
}
