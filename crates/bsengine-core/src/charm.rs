use bevy_ecs::prelude::{Component, Entity};

/// Charm CC — mind-control effect that forces the entity to fight for the
/// charmer's side for a limited duration.
///
/// A charmed entity treats its normal allies as enemies and vice versa.
/// The AI/faction system checks `is_active()` and reads `source` to
/// determine the temporary controlling faction. `just_charmed` and
/// `just_uncharmed` provide hooks for VFX and audio.
///
/// `apply(source, duration)` refreshes the charm if the new duration extends
/// beyond the remaining time. `tick(dt)` advances the timer. `clear()`
/// breaks the charm immediately (break-free ability, immunity).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Charm {
    pub duration: f32,
    pub timer: f32,
    /// The entity that cast the charm; `None` when charm is inactive.
    pub source: Option<Entity>,
    /// True on the first frame the charm is applied or refreshed.
    pub just_charmed: bool,
    /// True on the first frame the charm expires or is cleansed.
    pub just_uncharmed: bool,
    pub enabled: bool,
}

impl Charm {
    pub fn new() -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            source: None,
            just_charmed: false,
            just_uncharmed: false,
            enabled: true,
        }
    }

    /// Apply a charm from `source` for `duration` seconds.
    ///
    /// Resets the timer and adopts the new duration only if it exceeds the
    /// remaining time. The source is always updated to the latest charmer.
    pub fn apply(&mut self, source: Entity, duration: f32) {
        if !self.enabled {
            return;
        }
        let remaining = self.duration - self.timer;
        if duration > remaining {
            self.duration = duration.max(0.0);
            self.timer = 0.0;
        }
        self.source = Some(source);
        self.just_charmed = true;
    }

    /// Break the charm immediately (cleanse / immunity).
    pub fn clear(&mut self) {
        self.duration = 0.0;
        self.timer = 0.0;
        self.source = None;
    }

    /// Advance the charm timer by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        self.just_charmed = false;
        self.just_uncharmed = false;

        if !self.is_active() {
            return;
        }

        self.timer += dt;
        if self.timer >= self.duration {
            self.timer = self.duration;
            self.source = None;
            self.just_uncharmed = true;
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer < self.duration
    }

    /// Remaining charm duration in seconds.
    pub fn remaining(&self) -> f32 {
        (self.duration - self.timer).max(0.0)
    }

    /// Fraction of the charm duration elapsed [0.0, 1.0].
    pub fn elapsed_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 1.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Charm {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::world::World;

    fn dummy_entity() -> Entity {
        let mut world = World::new();
        world.spawn_empty().id()
    }

    #[test]
    fn apply_starts_charm() {
        let e = dummy_entity();
        let mut c = Charm::new();
        c.apply(e, 3.0);
        assert!(c.is_active());
        assert_eq!(c.source, Some(e));
        assert!(c.just_charmed);
    }

    #[test]
    fn longer_apply_resets_timer() {
        let e = dummy_entity();
        let mut c = Charm::new();
        c.apply(e, 2.0);
        c.tick(1.0);
        c.apply(e, 3.0); // remaining 1.0 < 3.0 → reset
        assert_eq!(c.timer, 0.0);
        assert_eq!(c.duration, 3.0);
    }

    #[test]
    fn shorter_apply_keeps_existing() {
        let e = dummy_entity();
        let mut c = Charm::new();
        c.apply(e, 5.0);
        c.apply(e, 1.0); // shorter → keep, but flag
        assert_eq!(c.duration, 5.0);
        assert!(c.just_charmed);
    }

    #[test]
    fn tick_expires_charm() {
        let e = dummy_entity();
        let mut c = Charm::new();
        c.apply(e, 2.0);
        c.tick(2.1);
        assert!(!c.is_active());
        assert!(c.just_uncharmed);
        assert_eq!(c.source, None);
    }

    #[test]
    fn clear_breaks_charm() {
        let e = dummy_entity();
        let mut c = Charm::new();
        c.apply(e, 5.0);
        c.clear();
        assert!(!c.is_active());
        assert_eq!(c.source, None);
    }

    #[test]
    fn disabled_ignores_apply() {
        let e = dummy_entity();
        let mut c = Charm::new();
        c.enabled = false;
        c.apply(e, 3.0);
        assert!(!c.is_active());
        assert!(!c.just_charmed);
    }

    #[test]
    fn remaining_decreases() {
        let e = dummy_entity();
        let mut c = Charm::new();
        c.apply(e, 4.0);
        c.tick(1.0);
        assert!((c.remaining() - 3.0).abs() < 1e-5);
    }

    #[test]
    fn elapsed_fraction_correct() {
        let e = dummy_entity();
        let mut c = Charm::new();
        c.apply(e, 4.0);
        c.tick(1.0);
        assert!((c.elapsed_fraction() - 0.25).abs() < 1e-5);
    }
}
