use bevy_ecs::prelude::{Component, Entity};
use glam::Vec3;

/// Fire mode of the turret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FireMode {
    /// Fires continuously while a target is in range.
    Auto,
    /// Fires one burst then waits for a full cooldown.
    Burst,
    /// Fires one shot per target acquisition.
    SingleShot,
}

/// Auto-aiming turret that tracks and fires at enemy entities.
/// The combat system sets `current_target` and calls `tick()` each frame.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Turret {
    /// Entity currently being tracked. `None` = no target.
    pub current_target: Option<Entity>,
    /// Facing direction in world space.
    pub facing: Vec3,
    /// How fast the turret rotates toward its target (rad/s).
    pub rotation_speed: f32,
    /// Maximum detection range in metres.
    pub range: f32,
    /// Cone half-angle for target detection (radians). `π` = 360 °.
    pub detection_angle: f32,
    pub fire_mode: FireMode,
    /// Delay between shots (seconds).
    pub fire_rate: f32,
    /// Time until next shot is allowed.
    pub fire_cooldown: f32,
    /// Layer mask for which entities this turret targets.
    pub target_layer: u32,
    /// Whether the turret can fire this frame (set to true by `tick` when ready).
    pub wants_to_fire: bool,
    pub enabled: bool,
}

impl Turret {
    pub fn new(range: f32, fire_rate: f32) -> Self {
        Self {
            current_target: None,
            facing: Vec3::Z,
            rotation_speed: std::f32::consts::PI,
            range: range.max(0.0),
            detection_angle: std::f32::consts::PI,
            fire_mode: FireMode::Auto,
            fire_rate: fire_rate.max(0.0001),
            fire_cooldown: 0.0,
            target_layer: u32::MAX,
            wants_to_fire: false,
            enabled: true,
        }
    }

    pub fn with_rotation_speed(mut self, rad_per_sec: f32) -> Self {
        self.rotation_speed = rad_per_sec.max(0.0);
        self
    }

    pub fn with_detection_angle(mut self, radians: f32) -> Self {
        self.detection_angle = radians.clamp(0.0, std::f32::consts::TAU);
        self
    }

    pub fn with_fire_mode(mut self, mode: FireMode) -> Self {
        self.fire_mode = mode;
        self
    }

    pub fn with_target_layer(mut self, layer: u32) -> Self {
        self.target_layer = layer;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Set a new target. Clears `wants_to_fire`.
    pub fn acquire(&mut self, target: Entity) {
        self.current_target = Some(target);
        self.wants_to_fire = false;
    }

    pub fn lose_target(&mut self) {
        self.current_target = None;
        self.wants_to_fire = false;
    }

    /// Rotate facing toward `target_dir` by at most `rotation_speed * dt` radians.
    /// Returns `true` when facing is aligned (within 0.01 rad).
    pub fn rotate_toward(&mut self, target_dir: Vec3, dt: f32) -> bool {
        let target = target_dir.normalize_or_zero();
        let dot = self.facing.dot(target).clamp(-1.0, 1.0);
        let angle = dot.acos();
        if angle < 0.01 {
            self.facing = target;
            return true;
        }
        let step = (self.rotation_speed * dt).min(angle);
        let t = step / angle;
        self.facing = self.facing.lerp(target, t).normalize_or_zero();
        false
    }

    /// Advance fire cooldown. Sets `wants_to_fire = true` when ready.
    pub fn tick(&mut self, dt: f32) {
        if !self.enabled {
            self.wants_to_fire = false;
            return;
        }
        if self.fire_cooldown > 0.0 {
            self.fire_cooldown -= dt;
        }
        if self.fire_cooldown <= 0.0 && self.current_target.is_some() {
            self.wants_to_fire = true;
        } else {
            self.wants_to_fire = false;
        }
    }

    /// Called by the weapon system after a shot is fired.
    pub fn on_fired(&mut self) {
        self.wants_to_fire = false;
        self.fire_cooldown = self.fire_rate;
        if self.fire_mode == FireMode::SingleShot {
            self.lose_target();
        }
    }

    pub fn is_ready(&self) -> bool {
        self.enabled && self.fire_cooldown <= 0.0 && self.current_target.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::world::World;

    fn entity() -> Entity {
        World::new().spawn_empty().id()
    }

    #[test]
    fn turret_tick_sets_wants_to_fire() {
        let mut t = Turret::new(20.0, 0.5);
        t.acquire(entity());
        t.tick(0.6);
        assert!(t.wants_to_fire);
    }

    #[test]
    fn turret_on_fired_starts_cooldown() {
        let mut t = Turret::new(20.0, 1.0);
        t.acquire(entity());
        t.tick(0.1);
        t.on_fired();
        assert!(!t.wants_to_fire);
        assert!(t.fire_cooldown > 0.0);
    }

    #[test]
    fn turret_cooldown_expires() {
        let mut t = Turret::new(20.0, 0.5);
        t.acquire(entity());
        t.on_fired();
        t.tick(0.6);
        assert!(t.wants_to_fire);
    }

    #[test]
    fn turret_single_shot_loses_target() {
        let mut t = Turret::new(10.0, 0.5).with_fire_mode(FireMode::SingleShot);
        t.acquire(entity());
        t.on_fired();
        assert!(t.current_target.is_none());
    }

    #[test]
    fn turret_disabled_no_fire() {
        let mut t = Turret::new(20.0, 0.1).disabled();
        t.acquire(entity());
        t.tick(1.0);
        assert!(!t.wants_to_fire);
    }
}
