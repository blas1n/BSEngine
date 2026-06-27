use bevy_ecs::prelude::{Component, Entity};

/// Tactical cover state — entity is sheltering behind an obstacle.
///
/// The combat and AI systems check `in_cover` to grant damage reduction or
/// change targeting priority. `cover_entity` points to the obstacle being used
/// as cover so physics/destruction systems can break it.
///
/// `take_cover(obstacle)` enters cover. `break_cover()` exits it immediately
/// (e.g. the obstacle is destroyed). `peek` is a soft flag for animations — it
/// doesn't remove cover protection but signals the entity is briefly exposing
/// itself to fire.
///
/// `tick(dt)` advances an optional exposure timer: if `exposure_time > 0`
/// and the entity is peeking, the exposure timer counts up; break cover
/// automatically after `exposure_time` seconds of peeking.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Cover {
    /// True while the entity is sheltering behind cover.
    pub in_cover: bool,
    /// The obstacle entity providing cover; `None` if not in cover.
    pub cover_entity: Option<Entity>,
    /// True when the entity is peeking out from cover (to fire or look).
    pub peek: bool,
    /// If > 0, automatically breaks cover after this many seconds of peeking.
    pub exposure_time: f32,
    /// Counts up while peeking; resets when peek is cleared.
    pub exposure_timer: f32,
    /// True on the first frame the entity takes cover.
    pub just_took_cover: bool,
    /// True on the first frame the entity leaves cover.
    pub just_broke_cover: bool,
    pub enabled: bool,
}

impl Cover {
    pub fn new() -> Self {
        Self {
            in_cover: false,
            cover_entity: None,
            peek: false,
            exposure_time: 0.0,
            exposure_timer: 0.0,
            just_took_cover: false,
            just_broke_cover: false,
            enabled: true,
        }
    }

    pub fn with_exposure_time(mut self, seconds: f32) -> Self {
        self.exposure_time = seconds.max(0.0);
        self
    }

    /// Enter cover behind `obstacle`.
    pub fn take_cover(&mut self, obstacle: Entity) {
        if !self.enabled || self.in_cover {
            return;
        }
        self.in_cover = true;
        self.cover_entity = Some(obstacle);
        self.peek = false;
        self.exposure_timer = 0.0;
        self.just_took_cover = true;
    }

    /// Leave cover immediately (obstacle destroyed, voluntary exit, CC break).
    pub fn break_cover(&mut self) {
        if !self.in_cover {
            return;
        }
        self.in_cover = false;
        self.cover_entity = None;
        self.peek = false;
        self.exposure_timer = 0.0;
        self.just_broke_cover = true;
    }

    /// Toggle or set the peek state while in cover.
    pub fn set_peek(&mut self, peeking: bool) {
        if self.in_cover {
            if !peeking {
                self.exposure_timer = 0.0;
            }
            self.peek = peeking;
        }
    }

    /// Advance the exposure timer; breaks cover automatically when exceeded.
    pub fn tick(&mut self, dt: f32) {
        self.just_took_cover = false;
        self.just_broke_cover = false;

        if !self.in_cover || !self.peek || self.exposure_time <= 0.0 {
            return;
        }

        self.exposure_timer += dt;
        if self.exposure_timer >= self.exposure_time {
            self.break_cover();
            self.just_broke_cover = true;
        }
    }

    /// Fraction of the exposure timer used [0.0, 1.0]; 0.0 when not peeking.
    pub fn exposure_fraction(&self) -> f32 {
        if !self.peek || self.exposure_time <= 0.0 {
            return 0.0;
        }
        (self.exposure_timer / self.exposure_time).clamp(0.0, 1.0)
    }
}

impl Default for Cover {
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
    fn take_cover_sets_state() {
        let e = dummy_entity();
        let mut c = Cover::new();
        c.take_cover(e);
        assert!(c.in_cover);
        assert_eq!(c.cover_entity, Some(e));
        assert!(c.just_took_cover);
    }

    #[test]
    fn break_cover_clears_state() {
        let e = dummy_entity();
        let mut c = Cover::new();
        c.take_cover(e);
        c.break_cover();
        assert!(!c.in_cover);
        assert_eq!(c.cover_entity, None);
        assert!(c.just_broke_cover);
    }

    #[test]
    fn peek_sets_flag() {
        let e = dummy_entity();
        let mut c = Cover::new();
        c.take_cover(e);
        c.set_peek(true);
        assert!(c.peek);
    }

    #[test]
    fn peek_clears_exposure_timer_on_false() {
        let e = dummy_entity();
        let mut c = Cover::new().with_exposure_time(3.0);
        c.take_cover(e);
        c.set_peek(true);
        c.tick(1.0);
        c.set_peek(false);
        assert_eq!(c.exposure_timer, 0.0);
    }

    #[test]
    fn exposure_breaks_cover() {
        let e = dummy_entity();
        let mut c = Cover::new().with_exposure_time(2.0);
        c.take_cover(e);
        c.set_peek(true);
        c.tick(2.1);
        assert!(!c.in_cover);
        assert!(c.just_broke_cover);
    }

    #[test]
    fn no_exposure_when_not_peeking() {
        let e = dummy_entity();
        let mut c = Cover::new().with_exposure_time(2.0);
        c.take_cover(e);
        c.tick(5.0); // not peeking → no break
        assert!(c.in_cover);
    }

    #[test]
    fn disabled_ignores_take_cover() {
        let e = dummy_entity();
        let mut c = Cover::new();
        c.enabled = false;
        c.take_cover(e);
        assert!(!c.in_cover);
    }

    #[test]
    fn exposure_fraction_while_peeking() {
        let e = dummy_entity();
        let mut c = Cover::new().with_exposure_time(4.0);
        c.take_cover(e);
        c.set_peek(true);
        c.tick(1.0);
        assert!((c.exposure_fraction() - 0.25).abs() < 1e-5);
    }
}
