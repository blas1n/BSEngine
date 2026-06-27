use bevy_ecs::prelude::{Component, Entity};

/// State of an entity's grab/clinch involvement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrabRole {
    None,
    Grabber,
    Grabbed,
}

/// Melee grab/clinch mechanic — distinct from `Grapple` (rope/hook) and
/// `Carry` (transport). `Grab` models close-quarters wrestling: one entity
/// seizes another for a brief window during which both are locked together,
/// the grabbed entity cannot act freely, and the grabber can follow up with a
/// throw or a grounded strike.
///
/// The combat system sets `grabbing` / `grabbed_by` on both participants.
/// `tick(dt)` counts down the clinch duration; `release()` cleanly ends it.
/// One-frame flags `just_grabbed`, `just_released` support animation triggers.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Grab {
    /// Which role this entity currently plays in the clinch.
    pub role: GrabRole,
    /// Entity being held (set on the grabber).
    pub grabbing: Option<Entity>,
    /// Entity holding this one (set on the grabbed entity).
    pub grabbed_by: Option<Entity>,
    /// How long a grab lasts (seconds).
    pub grab_duration: f32,
    /// Remaining time in the current grab.
    pub timer: f32,
    /// Range within which a grab can be initiated.
    pub grab_range: f32,
    pub just_grabbed: bool,
    pub just_released: bool,
    pub enabled: bool,
}

impl Grab {
    pub fn new(grab_duration: f32, grab_range: f32) -> Self {
        Self {
            role: GrabRole::None,
            grabbing: None,
            grabbed_by: None,
            grab_duration: grab_duration.max(0.0),
            timer: 0.0,
            grab_range: grab_range.max(0.0),
            just_grabbed: false,
            just_released: false,
            enabled: true,
        }
    }

    /// Initiate a grab on `target`. Call on the grabber entity.
    pub fn begin_grab(&mut self, target: Entity) -> bool {
        if !self.enabled || self.role != GrabRole::None {
            return false;
        }
        self.role = GrabRole::Grabber;
        self.grabbing = Some(target);
        self.timer = self.grab_duration;
        self.just_grabbed = true;
        true
    }

    /// Mark this entity as being grabbed by `source`. Call on the grabbed entity.
    pub fn begin_grabbed(&mut self, source: Entity) -> bool {
        if !self.enabled || self.role != GrabRole::None {
            return false;
        }
        self.role = GrabRole::Grabbed;
        self.grabbed_by = Some(source);
        self.timer = self.grab_duration;
        self.just_grabbed = true;
        true
    }

    /// End the grab/clinch on this entity.
    pub fn release(&mut self) {
        if self.role != GrabRole::None {
            self.role = GrabRole::None;
            self.grabbing = None;
            self.grabbed_by = None;
            self.timer = 0.0;
            self.just_released = true;
        }
    }

    /// Advance the grab timer; auto-releases when the duration expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_grabbed = false;
        self.just_released = false;

        if self.role == GrabRole::None {
            return;
        }

        self.timer -= dt;
        if self.timer <= 0.0 {
            self.release();
        }
    }

    pub fn is_grabbing(&self) -> bool {
        self.role == GrabRole::Grabber
    }

    pub fn is_grabbed(&self) -> bool {
        self.role == GrabRole::Grabbed
    }

    pub fn is_free(&self) -> bool {
        self.role == GrabRole::None
    }

    /// Fraction of the grab duration remaining [1.0 = just started, 0.0 = expired].
    pub fn timer_fraction(&self) -> f32 {
        if self.grab_duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.grab_duration).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::entity::Entity;

    fn e(raw: u32) -> Entity {
        Entity::from_raw(raw)
    }

    #[test]
    fn begin_grab_sets_state() {
        let mut g = Grab::new(2.0, 1.5);
        let ok = g.begin_grab(e(1));
        assert!(ok);
        assert!(g.is_grabbing());
        assert_eq!(g.grabbing, Some(e(1)));
        assert!(g.just_grabbed);
    }

    #[test]
    fn begin_grab_fails_when_already_in_grab() {
        let mut g = Grab::new(2.0, 1.5);
        g.begin_grab(e(1));
        let ok = g.begin_grab(e(2));
        assert!(!ok);
    }

    #[test]
    fn begin_grabbed_sets_state() {
        let mut g = Grab::new(2.0, 1.5);
        g.begin_grabbed(e(2));
        assert!(g.is_grabbed());
        assert_eq!(g.grabbed_by, Some(e(2)));
    }

    #[test]
    fn tick_expires_grab() {
        let mut g = Grab::new(1.0, 1.5);
        g.begin_grab(e(1));
        g.tick(1.1);
        assert!(g.is_free());
        assert!(g.just_released);
    }

    #[test]
    fn tick_clears_just_grabbed() {
        let mut g = Grab::new(2.0, 1.5);
        g.begin_grab(e(1));
        g.tick(0.5);
        assert!(!g.just_grabbed);
    }

    #[test]
    fn release_ends_grab() {
        let mut g = Grab::new(5.0, 1.5);
        g.begin_grab(e(1));
        g.release();
        assert!(g.is_free());
        assert!(g.just_released);
        assert!(g.grabbing.is_none());
    }

    #[test]
    fn timer_fraction_mid_grab() {
        let mut g = Grab::new(2.0, 1.5);
        g.begin_grab(e(1));
        g.tick(1.0); // half elapsed
        let frac = g.timer_fraction();
        assert!((frac - 0.5).abs() < 1e-4);
    }

    #[test]
    fn disabled_begin_grab_no_op() {
        let mut g = Grab::new(2.0, 1.5);
        g.enabled = false;
        let ok = g.begin_grab(e(1));
        assert!(!ok);
        assert!(g.is_free());
    }
}
