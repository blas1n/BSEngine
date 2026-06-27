use bevy_ecs::prelude::Component;

/// Tracks an entity's current and maximum RPG level.
///
/// `level_up()` increments `current` (clamped to `max`) and sets
/// `just_leveled_up` for one frame. Designed to be paired with `Experience`
/// — the XP system calls `level_up()` when a threshold is reached.
///
/// `prestige_level` tracks the number of resets past `max` for games that
/// allow prestiging. `reset()` drops `current` back to 1 and increments
/// `prestige_level`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Level {
    pub current: u32,
    pub max: u32,
    pub prestige_level: u32,
    pub just_leveled_up: bool,
    pub just_prestiged: bool,
    pub enabled: bool,
}

impl Level {
    pub fn new(max: u32) -> Self {
        Self {
            current: 1,
            max: max.max(1),
            prestige_level: 0,
            just_leveled_up: false,
            just_prestiged: false,
            enabled: true,
        }
    }

    pub fn with_current(mut self, current: u32) -> Self {
        self.current = current.clamp(1, self.max);
        self
    }

    /// Increment `current` by one. Returns true if the level actually increased.
    pub fn level_up(&mut self) -> bool {
        if !self.enabled {
            return false;
        }

        self.just_leveled_up = false;

        if self.current < self.max {
            self.current += 1;
            self.just_leveled_up = true;
            return true;
        }

        false
    }

    /// Reset to level 1 and increment `prestige_level`. Only valid when at max.
    pub fn prestige(&mut self) -> bool {
        if !self.enabled || self.current < self.max {
            return false;
        }

        self.current = 1;
        self.prestige_level += 1;
        self.just_prestiged = true;
        true
    }

    /// Clear single-frame flags; call once per frame.
    pub fn tick(&mut self, _dt: f32) {
        self.just_leveled_up = false;
        self.just_prestiged = false;
    }

    pub fn is_max_level(&self) -> bool {
        self.current >= self.max
    }

    /// Fraction of the way to max level [0.0, 1.0].
    pub fn progress_fraction(&self) -> f32 {
        if self.max <= 1 {
            return 1.0;
        }
        (self.current.saturating_sub(1)) as f32 / (self.max - 1) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_level_1() {
        let l = Level::new(10);
        assert_eq!(l.current, 1);
        assert_eq!(l.max, 10);
    }

    #[test]
    fn level_up_increments() {
        let mut l = Level::new(10);
        let up = l.level_up();
        assert!(up);
        assert_eq!(l.current, 2);
        assert!(l.just_leveled_up);
    }

    #[test]
    fn level_up_at_max_returns_false() {
        let mut l = Level::new(3).with_current(3);
        let up = l.level_up();
        assert!(!up);
        assert_eq!(l.current, 3);
        assert!(!l.just_leveled_up);
    }

    #[test]
    fn tick_clears_just_leveled_up() {
        let mut l = Level::new(10);
        l.level_up();
        l.tick(0.016);
        assert!(!l.just_leveled_up);
    }

    #[test]
    fn prestige_resets_to_1_at_max() {
        let mut l = Level::new(3).with_current(3);
        let ok = l.prestige();
        assert!(ok);
        assert_eq!(l.current, 1);
        assert_eq!(l.prestige_level, 1);
        assert!(l.just_prestiged);
    }

    #[test]
    fn prestige_fails_before_max() {
        let mut l = Level::new(10).with_current(5);
        let ok = l.prestige();
        assert!(!ok);
        assert_eq!(l.prestige_level, 0);
    }

    #[test]
    fn is_max_level() {
        let l = Level::new(5).with_current(5);
        assert!(l.is_max_level());
    }

    #[test]
    fn progress_fraction_midpoint() {
        let l = Level::new(5).with_current(3); // (3-1)/(5-1) = 0.5
        assert!((l.progress_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn disabled_level_up_no_op() {
        let mut l = Level::new(10);
        l.enabled = false;
        let up = l.level_up();
        assert!(!up);
        assert_eq!(l.current, 1);
    }
}
