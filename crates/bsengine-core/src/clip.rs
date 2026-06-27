use bevy_ecs::prelude::Component;

/// State of a firearm clip/magazine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipState {
    /// Ammo available; can fire.
    Ready,
    /// Reload animation in progress; cannot fire.
    Reloading,
    /// Clip empty and no reserve ammo; cannot fire or reload.
    Empty,
}

/// Ammo/magazine component for ranged weapons.
///
/// Models a single clip/magazine with reserve ammo pool. Call `shoot()`
/// each time the weapon fires. When `needs_reload()` (or explicitly),
/// call `reload()` — `tick(dt)` drives the reload timer and sets
/// `just_reloaded` on the frame reloading finishes.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Clip {
    pub state: ClipState,
    pub current_ammo: u32,
    pub max_ammo: u32,
    pub reserve_ammo: u32,
    pub max_reserve: u32,
    /// Duration of the reload animation in seconds.
    pub reload_duration: f32,
    pub reload_timer: f32,
    /// True on the exact frame a reload completes.
    pub just_reloaded: bool,
    pub enabled: bool,
}

impl Clip {
    pub fn new(max_ammo: u32, max_reserve: u32, reload_duration: f32) -> Self {
        Self {
            state: ClipState::Ready,
            current_ammo: max_ammo,
            max_ammo,
            reserve_ammo: max_reserve,
            max_reserve,
            reload_duration: reload_duration.max(0.0),
            reload_timer: 0.0,
            just_reloaded: false,
            enabled: true,
        }
    }

    /// Create a clip that starts empty (e.g., just picked up, no ammo).
    pub fn depleted(max_ammo: u32, max_reserve: u32, reload_duration: f32) -> Self {
        let mut c = Self::new(max_ammo, max_reserve, reload_duration);
        c.current_ammo = 0;
        c.reserve_ammo = 0;
        c.state = ClipState::Empty;
        c
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Consume one round. Returns true if a shot was fired.
    pub fn shoot(&mut self) -> bool {
        if !self.enabled || self.state != ClipState::Ready || self.current_ammo == 0 {
            return false;
        }
        self.current_ammo -= 1;
        if self.current_ammo == 0 {
            self.state = if self.reserve_ammo > 0 {
                ClipState::Ready // still Ready; caller must trigger reload if desired
            } else {
                ClipState::Empty
            };
        }
        true
    }

    /// Begin reloading. Returns false if already reloading, no reserve ammo, or clip is full.
    pub fn reload(&mut self) -> bool {
        if !self.enabled
            || self.state == ClipState::Reloading
            || self.reserve_ammo == 0
            || self.current_ammo == self.max_ammo
        {
            return false;
        }
        self.state = ClipState::Reloading;
        self.reload_timer = self.reload_duration;
        true
    }

    /// Add rounds directly to reserve (e.g., ammo pickup).
    pub fn add_reserve(&mut self, amount: u32) {
        self.reserve_ammo = (self.reserve_ammo + amount).min(self.max_reserve);
        if self.state == ClipState::Empty && self.reserve_ammo > 0 {
            self.state = ClipState::Ready;
        }
    }

    /// Advance the reload timer. Call once per frame.
    pub fn tick(&mut self, dt: f32) {
        self.just_reloaded = false;

        if self.state != ClipState::Reloading {
            return;
        }

        self.reload_timer = (self.reload_timer - dt).max(0.0);
        if self.reload_timer <= 0.0 {
            let needed = self.max_ammo - self.current_ammo;
            let taken = needed.min(self.reserve_ammo);
            self.current_ammo += taken;
            self.reserve_ammo -= taken;

            self.state = if self.current_ammo > 0 {
                ClipState::Ready
            } else {
                ClipState::Empty
            };
            self.just_reloaded = true;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.current_ammo == 0 && self.state != ClipState::Reloading
    }

    /// True when the clip has fewer rounds than max and there is reserve ammo.
    pub fn needs_reload(&self) -> bool {
        self.current_ammo < self.max_ammo && self.reserve_ammo > 0
    }

    /// Fraction of current clip filled [0, 1].
    pub fn ammo_fraction(&self) -> f32 {
        if self.max_ammo == 0 {
            0.0
        } else {
            self.current_ammo as f32 / self.max_ammo as f32
        }
    }

    /// Fraction of reserve ammo remaining [0, 1].
    pub fn reserve_fraction(&self) -> f32 {
        if self.max_reserve == 0 {
            0.0
        } else {
            self.reserve_ammo as f32 / self.max_reserve as f32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pistol() -> Clip {
        Clip::new(12, 48, 1.5)
    }

    #[test]
    fn shoot_decrements_ammo() {
        let mut c = pistol();
        assert!(c.shoot());
        assert_eq!(c.current_ammo, 11);
    }

    #[test]
    fn shoot_fails_while_reloading() {
        let mut c = pistol();
        c.current_ammo = 0;
        c.reload();
        assert!(!c.shoot());
    }

    #[test]
    fn reload_restores_ammo() {
        let mut c = pistol();
        c.current_ammo = 0;
        assert!(c.reload());
        assert_eq!(c.state, ClipState::Reloading);
        c.tick(1.5);
        assert_eq!(c.state, ClipState::Ready);
        assert_eq!(c.current_ammo, 12);
        assert!(c.just_reloaded);
        assert_eq!(c.reserve_ammo, 48 - 12);
    }

    #[test]
    fn reload_partial_reserve() {
        let mut c = Clip::new(12, 5, 1.0);
        c.current_ammo = 0;
        c.reload();
        c.tick(1.0);
        assert_eq!(c.current_ammo, 5);
        assert_eq!(c.reserve_ammo, 0);
        assert_eq!(c.state, ClipState::Ready); // still has bullets, just no reserve
    }

    #[test]
    fn reload_blocked_when_full() {
        let mut c = pistol();
        assert!(!c.reload());
    }

    #[test]
    fn add_reserve_updates_state() {
        let mut c = Clip::depleted(10, 30, 1.0);
        assert_eq!(c.state, ClipState::Empty);
        c.add_reserve(10);
        assert_eq!(c.state, ClipState::Ready);
    }

    #[test]
    fn disabled_blocks_shoot() {
        let mut c = pistol().disabled();
        assert!(!c.shoot());
    }
}
