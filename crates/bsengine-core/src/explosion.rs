use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Lifecycle state of an explosion entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplosionState {
    /// Pending detonation (fuse).
    Armed,
    /// Currently expanding — physics and damage systems read this each frame.
    Active,
    /// Blast has ended; entity should be despawned.
    Finished,
}

/// Radial explosion placed at an entity's position.
///
/// On detonation the physics system applies an outward impulse to all rigidbodies
/// within `radius` and the damage system deals `damage * falloff(distance)` to
/// Health components. Call `tick(dt)` each frame; the explosion finishes when
/// `elapsed >= duration`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Explosion {
    pub state: ExplosionState,
    /// Fuse countdown in seconds. 0 = detonate immediately when armed.
    pub fuse: f32,
    /// Remaining fuse time.
    pub fuse_timer: f32,
    /// Outer radius of the blast (units).
    pub radius: f32,
    /// Peak damage dealt at the epicentre.
    pub damage: f32,
    /// Peak outward impulse at the epicentre (kg·m/s equivalent).
    pub impulse: f32,
    /// Duration the explosion effect lasts (for VFX/audio queries).
    pub duration: f32,
    /// Time elapsed since detonation.
    pub elapsed: f32,
    /// Layer mask — which layers receive damage and impulse.
    pub layer_mask: u32,
    /// Origin override; `None` = use the entity's `Transform`.
    pub origin_override: Option<Vec3>,
    /// Whether the explosion has already applied its physics/damage pass.
    pub applied: bool,
    pub enabled: bool,
}

impl Explosion {
    pub fn new(radius: f32, damage: f32) -> Self {
        Self {
            state: ExplosionState::Armed,
            fuse: 0.0,
            fuse_timer: 0.0,
            radius: radius.max(0.01),
            damage: damage.max(0.0),
            impulse: damage * 2.0,
            duration: 0.2,
            elapsed: 0.0,
            layer_mask: u32::MAX,
            origin_override: None,
            applied: false,
            enabled: true,
        }
    }

    pub fn with_fuse(mut self, seconds: f32) -> Self {
        self.fuse = seconds.max(0.0);
        self.fuse_timer = self.fuse;
        self
    }

    pub fn with_impulse(mut self, impulse: f32) -> Self {
        self.impulse = impulse.max(0.0);
        self
    }

    pub fn with_duration(mut self, duration: f32) -> Self {
        self.duration = duration.max(0.0);
        self
    }

    pub fn with_layer_mask(mut self, mask: u32) -> Self {
        self.layer_mask = mask;
        self
    }

    pub fn with_origin(mut self, origin: Vec3) -> Self {
        self.origin_override = Some(origin);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Advance fuse / active timers. Returns `true` on the tick the blast becomes Active.
    pub fn tick(&mut self, dt: f32) -> bool {
        if !self.enabled {
            return false;
        }
        match self.state {
            ExplosionState::Armed => {
                if self.fuse_timer > 0.0 {
                    self.fuse_timer -= dt;
                    if self.fuse_timer > 0.0 {
                        return false;
                    }
                }
                self.state = ExplosionState::Active;
                return true;
            }
            ExplosionState::Active => {
                self.elapsed += dt;
                if self.elapsed >= self.duration {
                    self.state = ExplosionState::Finished;
                }
            }
            ExplosionState::Finished => {}
        }
        false
    }

    /// Damage falloff at `distance` from the epicentre — linear [0, 1].
    pub fn damage_at(&self, distance: f32) -> f32 {
        if self.radius <= 0.0 || distance >= self.radius {
            return 0.0;
        }
        (1.0 - distance / self.radius).clamp(0.0, 1.0) * self.damage
    }

    /// Impulse falloff at `distance` from the epicentre — linear [0, 1].
    pub fn impulse_at(&self, distance: f32) -> f32 {
        if self.radius <= 0.0 || distance >= self.radius {
            return 0.0;
        }
        (1.0 - distance / self.radius).clamp(0.0, 1.0) * self.impulse
    }

    pub fn is_active(&self) -> bool {
        self.state == ExplosionState::Active
    }

    pub fn is_finished(&self) -> bool {
        self.state == ExplosionState::Finished
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn immediate_detonation() {
        let mut e = Explosion::new(5.0, 100.0);
        let detonated = e.tick(0.016);
        assert!(detonated);
        assert!(e.is_active());
    }

    #[test]
    fn fuse_delays_detonation() {
        let mut e = Explosion::new(5.0, 100.0).with_fuse(1.0);
        assert!(!e.tick(0.5));
        assert_eq!(e.state, ExplosionState::Armed);
        assert!(e.tick(0.6));
        assert!(e.is_active());
    }

    #[test]
    fn finishes_after_duration() {
        let mut e = Explosion::new(5.0, 100.0).with_duration(0.3);
        e.tick(0.0);
        e.tick(0.31);
        assert!(e.is_finished());
    }

    #[test]
    fn damage_at_falloff() {
        let e = Explosion::new(10.0, 100.0);
        assert!((e.damage_at(0.0) - 100.0).abs() < 0.001);
        assert!((e.damage_at(5.0) - 50.0).abs() < 0.001);
        assert!((e.damage_at(10.0)).abs() < 0.001);
    }

    #[test]
    fn damage_at_outside_radius_is_zero() {
        let e = Explosion::new(5.0, 100.0);
        assert!((e.damage_at(6.0)).abs() < 0.001);
    }
}
