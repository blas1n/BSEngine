use bevy_ecs::prelude::Component;

/// Parry-window for sending incoming projectiles back at their source.
///
/// Distinct from `Guard` (melee damage block) and `Ricochet` (projectile
/// bouncing off surfaces). `Reflect` is placed on the *target* entity and
/// models the window in which it can deflect projectile attacks back toward
/// the shooter (think Genji's deflect, Paladin's reflect shield).
///
/// The physics/combat system checks `is_reflecting()` when a projectile
/// contacts this entity, then reads `damage_multiplier` to scale the returned
/// projectile's damage. The system should call `notify_reflected()` so
/// `just_reflected` fires for VFX / sound hooks.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Reflect {
    pub is_active: bool,
    /// Reflected projectile deals `original_damage * damage_multiplier`.
    pub damage_multiplier: f32,
    /// Total duration of one reflect window in seconds (0.0 = instantaneous parry).
    pub window_duration: f32,
    /// Remaining time in the current window.
    pub window_timer: f32,
    /// True on the first frame a reflect window opens.
    pub just_activated: bool,
    /// True on the first frame a projectile is successfully reflected.
    pub just_reflected: bool,
    /// True on the first frame the reflect window closes.
    pub just_closed: bool,
    pub enabled: bool,
}

impl Reflect {
    pub fn new(window_duration: f32, damage_multiplier: f32) -> Self {
        Self {
            is_active: false,
            damage_multiplier: damage_multiplier.max(0.0),
            window_duration: window_duration.max(0.0),
            window_timer: 0.0,
            just_activated: false,
            just_reflected: false,
            just_closed: false,
            enabled: true,
        }
    }

    /// Parry that lasts exactly one frame (e.g. a perfect-parry timing window).
    pub fn instant(damage_multiplier: f32) -> Self {
        Self::new(0.0, damage_multiplier)
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Open the reflect window. No-op if already active.
    pub fn activate(&mut self) {
        if !self.enabled || self.is_active {
            return;
        }
        self.is_active = true;
        self.window_timer = self.window_duration;
        self.just_activated = true;
    }

    /// Close the reflect window early.
    pub fn deactivate(&mut self) {
        if self.is_active {
            self.is_active = false;
            self.window_timer = 0.0;
            self.just_closed = true;
        }
    }

    /// Called by the combat system when a projectile is successfully deflected.
    pub fn notify_reflected(&mut self) {
        self.just_reflected = true;
    }

    /// Advance the window timer. Call once per frame.
    pub fn tick(&mut self, dt: f32) {
        self.just_activated = false;
        self.just_reflected = false;
        self.just_closed = false;

        if !self.enabled || !self.is_active {
            return;
        }

        if self.window_duration > 0.0 {
            self.window_timer = (self.window_timer - dt).max(0.0);
            if self.window_timer <= 0.0 {
                self.is_active = false;
                self.just_closed = true;
            }
        }
    }

    pub fn is_reflecting(&self) -> bool {
        self.enabled && self.is_active
    }

    /// Remaining window fraction (1.0 = just opened, 0.0 = closed or instant).
    pub fn window_fraction(&self) -> f32 {
        if self.window_duration <= 0.0 {
            return 0.0;
        }
        self.window_timer / self.window_duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activate_opens_window() {
        let mut r = Reflect::new(1.0, 2.0);
        r.activate();
        assert!(r.is_reflecting());
        assert!(r.just_activated);
    }

    #[test]
    fn window_expires_on_tick() {
        let mut r = Reflect::new(1.0, 2.0);
        r.activate();
        r.tick(0.0);
        r.tick(1.0);
        assert!(!r.is_reflecting());
        assert!(r.just_closed);
    }

    #[test]
    fn deactivate_closes_early() {
        let mut r = Reflect::new(5.0, 2.0);
        r.activate();
        r.tick(0.0);
        r.deactivate();
        assert!(!r.is_reflecting());
        assert!(r.just_closed);
    }

    #[test]
    fn notify_reflected_fires_flag() {
        let mut r = Reflect::new(1.0, 2.0);
        r.activate();
        r.notify_reflected();
        assert!(r.just_reflected);
    }

    #[test]
    fn tick_clears_flags() {
        let mut r = Reflect::new(5.0, 2.0);
        r.activate();
        r.notify_reflected();
        r.tick(0.0);
        assert!(!r.just_activated);
        assert!(!r.just_reflected);
    }

    #[test]
    fn window_fraction_decreases() {
        let mut r = Reflect::new(4.0, 1.0);
        r.activate();
        r.tick(0.0);
        r.tick(2.0);
        assert!((r.window_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn disabled_blocks_activate() {
        let mut r = Reflect::new(1.0, 2.0).disabled();
        r.activate();
        assert!(!r.is_reflecting());
    }
}
