use bevy_ecs::prelude::{Component, Entity};
use glam::Vec3;

/// Current state of the respawn sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RespawnState {
    /// Entity is alive; no respawn pending.
    Alive,
    /// Entity has died and is counting down before respawning.
    Pending,
    /// Respawn timer expired; ready for the game system to reposition and revive.
    Ready,
    /// Respawn is forbidden (lives exhausted or permanent death).
    Forbidden,
}

/// Respawn data attached to player and mob entities.
///
/// When the entity dies the game system sets `state = Pending`.
/// `tick(dt)` counts down `delay_timer`; when it reaches 0 it transitions to `Ready`.
/// The game system then reads `last_spawn_point` / `forced_position` and revives the entity.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Respawn {
    pub state: RespawnState,
    /// Seconds to wait before respawning. 0 = immediate.
    pub delay: f32,
    /// Remaining wait time.
    pub delay_timer: f32,
    /// Last spawn-point entity used. None = use world default.
    pub last_spawn_point: Option<Entity>,
    /// Override position; takes precedence over `last_spawn_point` when `Some`.
    pub forced_position: Option<Vec3>,
    /// Remaining lives. `None` = unlimited.
    pub lives: Option<u32>,
    /// Total number of times this entity has respawned.
    pub respawn_count: u32,
    pub enabled: bool,
}

impl Respawn {
    pub fn new(delay: f32) -> Self {
        Self {
            state: RespawnState::Alive,
            delay: delay.max(0.0),
            delay_timer: 0.0,
            last_spawn_point: None,
            forced_position: None,
            lives: None,
            respawn_count: 0,
            enabled: true,
        }
    }

    pub fn immediate() -> Self {
        Self::new(0.0)
    }

    pub fn with_lives(mut self, lives: u32) -> Self {
        self.lives = Some(lives);
        self
    }

    pub fn with_spawn_point(mut self, entity: Entity) -> Self {
        self.last_spawn_point = Some(entity);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Call when the entity dies. Starts the respawn countdown or marks Forbidden if out of lives.
    pub fn on_death(&mut self) {
        if !self.enabled || self.state != RespawnState::Alive {
            return;
        }
        if let Some(lives) = self.lives {
            if lives == 0 {
                self.state = RespawnState::Forbidden;
                return;
            }
            self.lives = Some(lives - 1);
        }
        self.delay_timer = self.delay;
        self.state = if self.delay <= 0.0 {
            RespawnState::Ready
        } else {
            RespawnState::Pending
        };
    }

    /// Call each frame while state is Pending. Returns `true` when Ready.
    pub fn tick(&mut self, dt: f32) -> bool {
        if !self.enabled || self.state != RespawnState::Pending {
            return false;
        }
        self.delay_timer -= dt;
        if self.delay_timer <= 0.0 {
            self.delay_timer = 0.0;
            self.state = RespawnState::Ready;
            return true;
        }
        false
    }

    /// Call after the game system has repositioned the entity.
    pub fn on_respawn(&mut self) {
        if self.state == RespawnState::Ready {
            self.respawn_count += 1;
            self.state = RespawnState::Alive;
            self.forced_position = None;
        }
    }

    pub fn is_alive(&self) -> bool {
        self.state == RespawnState::Alive
    }

    pub fn fraction_remaining(&self) -> f32 {
        if self.delay <= 0.0 {
            return 0.0;
        }
        (self.delay_timer / self.delay).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_death_starts_countdown() {
        let mut r = Respawn::new(3.0);
        r.on_death();
        assert_eq!(r.state, RespawnState::Pending);
        assert!((r.delay_timer - 3.0).abs() < 0.001);
    }

    #[test]
    fn tick_transitions_to_ready() {
        let mut r = Respawn::new(1.0);
        r.on_death();
        assert!(!r.tick(0.6));
        assert!(r.tick(0.5));
        assert_eq!(r.state, RespawnState::Ready);
    }

    #[test]
    fn on_respawn_increments_count() {
        let mut r = Respawn::new(0.0);
        r.on_death();
        assert_eq!(r.state, RespawnState::Ready);
        r.on_respawn();
        assert_eq!(r.respawn_count, 1);
        assert!(r.is_alive());
    }

    #[test]
    fn lives_exhausted_forbids_respawn() {
        let mut r = Respawn::new(1.0).with_lives(1);
        r.on_death();
        assert_eq!(r.state, RespawnState::Pending);
        r.tick(10.0);
        r.on_respawn();
        r.on_death();
        assert_eq!(r.state, RespawnState::Forbidden);
    }

    #[test]
    fn fraction_remaining_correct() {
        let mut r = Respawn::new(4.0);
        r.on_death();
        r.tick(1.0);
        assert!((r.fraction_remaining() - 0.75).abs() < 0.001);
    }
}
