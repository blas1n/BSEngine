use bevy_ecs::prelude::Component;

/// Temporary damage-immunity component.
///
/// Multiple systems can grant invincibility by calling `grant(duration)`, which
/// increments `stacks` and takes the longest of the current timer vs the new
/// duration. Invincibility ends when all stacks expire via `tick(dt)`.
///
/// Use `just_became_invincible` / `just_lost_invincibility` for one-frame VFX /
/// sound triggers. Use `flash_visible` (toggled every `flash_interval`) for the
/// classic hurt-flash effect.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Invincible {
    /// Active stacks of invincibility (different sources can each grant one).
    pub stacks: u32,
    /// Time remaining on the current invincibility grant.
    pub timer: f32,
    /// Interval between flash toggles (seconds). 0.0 disables flashing.
    pub flash_interval: f32,
    /// Accumulator for the flash timer.
    pub flash_timer: f32,
    /// Current flash visibility state (toggles every `flash_interval`).
    pub flash_visible: bool,
    /// True on the frame invincibility is first activated.
    pub just_became_invincible: bool,
    /// True on the frame the last invincibility stack expires.
    pub just_lost_invincibility: bool,
    pub enabled: bool,
}

impl Invincible {
    pub fn new() -> Self {
        Self {
            stacks: 0,
            timer: 0.0,
            flash_interval: 0.1,
            flash_timer: 0.0,
            flash_visible: true,
            just_became_invincible: false,
            just_lost_invincibility: false,
            enabled: true,
        }
    }

    pub fn with_flash_interval(mut self, interval: f32) -> Self {
        self.flash_interval = interval.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Grant invincibility for `duration` seconds (stacks additively on top of any existing grant).
    pub fn grant(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }
        let was_active = self.is_invincible();
        self.stacks += 1;
        // Extend the timer to the max of current remaining or new duration.
        if duration > self.timer {
            self.timer = duration;
        }
        if !was_active {
            self.just_became_invincible = true;
            self.flash_visible = true;
            self.flash_timer = 0.0;
        }
    }

    /// Remove one stack of invincibility immediately.
    pub fn revoke(&mut self) {
        if self.stacks > 0 {
            self.stacks -= 1;
            if self.stacks == 0 {
                self.timer = 0.0;
                self.just_lost_invincibility = true;
                self.flash_visible = true;
            }
        }
    }

    /// Advance state. Call once per frame.
    pub fn tick(&mut self, dt: f32) {
        self.just_became_invincible = false;
        self.just_lost_invincibility = false;

        if !self.enabled || self.stacks == 0 {
            return;
        }

        self.timer -= dt;
        if self.timer <= 0.0 {
            self.timer = 0.0;
            self.stacks = 0;
            self.just_lost_invincibility = true;
            self.flash_visible = true;
            return;
        }

        if self.flash_interval > 0.0 {
            self.flash_timer += dt;
            while self.flash_timer >= self.flash_interval {
                self.flash_timer -= self.flash_interval;
                self.flash_visible = !self.flash_visible;
            }
        }
    }

    pub fn is_invincible(&self) -> bool {
        self.enabled && self.stacks > 0
    }
}

impl Default for Invincible {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grant_activates_invincibility() {
        let mut inv = Invincible::new();
        inv.grant(1.0);
        assert!(inv.is_invincible());
        assert!(inv.just_became_invincible);
    }

    #[test]
    fn tick_expires_invincibility() {
        let mut inv = Invincible::new();
        inv.grant(0.5);
        inv.tick(0.6);
        assert!(!inv.is_invincible());
        assert!(inv.just_lost_invincibility);
    }

    #[test]
    fn second_grant_extends_timer() {
        let mut inv = Invincible::new();
        inv.grant(0.5);
        inv.grant(2.0);
        assert_eq!(inv.stacks, 2);
        assert!((inv.timer - 2.0).abs() < 1e-5);
    }

    #[test]
    fn revoke_removes_one_stack() {
        let mut inv = Invincible::new();
        inv.grant(1.0);
        inv.grant(1.0);
        inv.tick(0.0); // clear just_became flags
        inv.revoke();
        assert_eq!(inv.stacks, 1);
        assert!(inv.is_invincible());
    }

    #[test]
    fn flash_toggles_at_interval() {
        let mut inv = Invincible::new().with_flash_interval(0.1);
        inv.grant(1.0);
        inv.tick(0.0); // clear just_became
        assert!(inv.flash_visible);
        inv.tick(0.15);
        assert!(!inv.flash_visible);
        inv.tick(0.1);
        assert!(inv.flash_visible);
    }

    #[test]
    fn disabled_ignores_grant() {
        let mut inv = Invincible::new().disabled();
        inv.grant(1.0);
        assert!(!inv.is_invincible());
    }
}
