use bevy_ecs::prelude::{Component, Entity};

/// The current state of a frightened entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FearState {
    /// No fear active; entity behaves normally.
    Calm,
    /// Entity is frightened but not yet fleeing (brief reactive window).
    Frightened,
    /// Entity is actively fleeing from `source`.
    Fleeing,
}

/// Fear status effect — entity flees from a source when frightened.
///
/// The AI system reads `state` to override normal behaviour: while `Fleeing`,
/// the entity moves away from `source` at `flee_speed_multiplier × base_speed`.
///
/// Call `apply(source, duration)` to trigger fear. Multiple calls replace
/// the existing effect if the new duration is longer. Call `tick(dt)` every
/// frame to advance the timer. `clear()` removes the effect immediately (e.g.
/// when the source dies or the entity becomes invincible).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Fear {
    pub state: FearState,
    /// Entity the fearful entity is fleeing from, if any.
    pub source: Option<Entity>,
    pub duration: f32,
    pub timer: f32,
    /// Speed multiplier applied to movement while fleeing (e.g. 1.3 = 30% faster).
    pub flee_speed_multiplier: f32,
    /// True on the first frame fear begins.
    pub just_feared: bool,
    /// True on the first frame fear ends naturally.
    pub just_calmed: bool,
    pub enabled: bool,
}

impl Fear {
    pub fn new(flee_speed_multiplier: f32) -> Self {
        Self {
            state: FearState::Calm,
            source: None,
            duration: 0.0,
            timer: 0.0,
            flee_speed_multiplier: flee_speed_multiplier.max(1.0),
            just_feared: false,
            just_calmed: false,
            enabled: true,
        }
    }

    /// Apply fear from `source` for `duration` seconds.
    ///
    /// Does nothing if `enabled` is false. If already feared for longer,
    /// the existing effect is kept.
    pub fn apply(&mut self, source: Entity, duration: f32) {
        if !self.enabled {
            return;
        }
        let remaining = self.duration - self.timer;
        if duration > remaining {
            let was_calm = self.state == FearState::Calm;
            self.source = Some(source);
            self.duration = duration;
            self.timer = 0.0;
            self.state = FearState::Fleeing;
            self.just_feared = was_calm;
        }
    }

    /// Remove the fear effect immediately.
    pub fn clear(&mut self) {
        self.state = FearState::Calm;
        self.source = None;
        self.duration = 0.0;
        self.timer = 0.0;
    }

    /// Advance the fear timer by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        self.just_feared = false;
        self.just_calmed = false;

        if !self.is_active() {
            return;
        }

        self.timer += dt;
        if self.timer >= self.duration {
            self.timer = self.duration;
            self.state = FearState::Calm;
            self.source = None;
            self.just_calmed = true;
        }
    }

    pub fn is_active(&self) -> bool {
        self.state == FearState::Frightened || self.state == FearState::Fleeing
    }

    /// Fraction of the fear duration that has elapsed [0.0, 1.0].
    pub fn elapsed_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 1.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }

    /// Remaining fear duration in seconds.
    pub fn remaining(&self) -> f32 {
        (self.duration - self.timer).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::entity::Entity;

    fn entity(id: u32) -> Entity {
        Entity::from_raw(id)
    }

    #[test]
    fn apply_starts_fleeing() {
        let mut f = Fear::new(1.3);
        f.apply(entity(1), 3.0);
        assert_eq!(f.state, FearState::Fleeing);
        assert!(f.just_feared);
        assert_eq!(f.source, Some(entity(1)));
    }

    #[test]
    fn shorter_reapply_does_not_replace() {
        let mut f = Fear::new(1.3);
        f.apply(entity(1), 5.0);
        f.apply(entity(2), 1.0);
        assert_eq!(f.source, Some(entity(1)));
        assert_eq!(f.duration, 5.0);
    }

    #[test]
    fn longer_reapply_replaces() {
        let mut f = Fear::new(1.3);
        f.apply(entity(1), 2.0);
        f.apply(entity(2), 4.0);
        assert_eq!(f.source, Some(entity(2)));
        assert_eq!(f.duration, 4.0);
    }

    #[test]
    fn tick_expires_and_calms() {
        let mut f = Fear::new(1.3);
        f.apply(entity(1), 1.0);
        f.tick(0.5);
        assert!(f.is_active());
        f.tick(0.6);
        assert_eq!(f.state, FearState::Calm);
        assert!(f.just_calmed);
    }

    #[test]
    fn clear_removes_immediately() {
        let mut f = Fear::new(1.3);
        f.apply(entity(1), 5.0);
        f.clear();
        assert_eq!(f.state, FearState::Calm);
        assert!(f.source.is_none());
    }

    #[test]
    fn disabled_ignores_apply() {
        let mut f = Fear::new(1.3);
        f.enabled = false;
        f.apply(entity(1), 3.0);
        assert_eq!(f.state, FearState::Calm);
    }

    #[test]
    fn elapsed_fraction_in_range() {
        let mut f = Fear::new(1.3);
        f.apply(entity(1), 4.0);
        f.tick(1.0);
        let frac = f.elapsed_fraction();
        assert!((frac - 0.25).abs() < 1e-5);
    }

    #[test]
    fn remaining_decreases() {
        let mut f = Fear::new(1.3);
        f.apply(entity(1), 4.0);
        f.tick(1.0);
        assert!((f.remaining() - 3.0).abs() < 1e-5);
    }
}
