use bevy_ecs::prelude::Component;

/// Tracks XP and level progression for an entity.
/// The progression system calls `add_xp()` and reads `pending_level_ups()` each frame.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Experience {
    pub level: u32,
    pub current_xp: f32,
    /// XP required to advance from `level` to `level + 1`.
    /// The system multiplies this by `xp_scale^level` for the actual threshold.
    pub base_xp_per_level: f32,
    /// Exponential growth factor applied per level. 1.0 = flat, 1.5 = 50% more each level.
    pub xp_scale: f32,
    /// Maximum level the entity can reach. `None` = unlimited.
    pub max_level: Option<u32>,
    pub enabled: bool,
}

impl Experience {
    pub fn new(base_xp_per_level: f32) -> Self {
        Self {
            level: 1,
            current_xp: 0.0,
            base_xp_per_level: base_xp_per_level.max(1.0),
            xp_scale: 1.0,
            max_level: None,
            enabled: true,
        }
    }

    pub fn with_xp_scale(mut self, scale: f32) -> Self {
        self.xp_scale = scale.max(1.0);
        self
    }

    pub fn with_max_level(mut self, max: u32) -> Self {
        self.max_level = Some(max.max(1));
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// XP needed to advance from the current level to the next.
    pub fn xp_for_next_level(&self) -> f32 {
        self.base_xp_per_level * self.xp_scale.powi((self.level - 1) as i32)
    }

    /// Returns `true` if the entity is at `max_level`.
    pub fn is_max_level(&self) -> bool {
        self.max_level.map_or(false, |max| self.level >= max)
    }

    /// Add `amount` XP and process level-ups. Returns the number of levels gained.
    pub fn add_xp(&mut self, amount: f32) -> u32 {
        if !self.enabled || amount <= 0.0 {
            return 0;
        }
        self.current_xp += amount;
        let mut gained = 0;
        loop {
            if self.is_max_level() {
                self.current_xp = 0.0;
                break;
            }
            let threshold = self.xp_for_next_level();
            if self.current_xp >= threshold {
                self.current_xp -= threshold;
                self.level += 1;
                gained += 1;
            } else {
                break;
            }
        }
        gained
    }

    /// XP progress toward the next level as a fraction in [0, 1].
    pub fn progress(&self) -> f32 {
        if self.is_max_level() {
            return 1.0;
        }
        (self.current_xp / self.xp_for_next_level()).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn experience_level_up() {
        let mut xp = Experience::new(100.0);
        let gained = xp.add_xp(100.0);
        assert_eq!(gained, 1);
        assert_eq!(xp.level, 2);
        assert_eq!(xp.current_xp, 0.0);
    }

    #[test]
    fn experience_partial_xp() {
        let mut xp = Experience::new(100.0);
        xp.add_xp(50.0);
        assert_eq!(xp.level, 1);
        assert!((xp.progress() - 0.5).abs() < 0.001);
    }

    #[test]
    fn experience_max_level_capped() {
        let mut xp = Experience::new(10.0).with_max_level(2);
        xp.add_xp(1000.0);
        assert_eq!(xp.level, 2);
        assert!(xp.is_max_level());
    }

    #[test]
    fn experience_xp_scale() {
        let xp = Experience::new(100.0).with_xp_scale(2.0);
        assert!((xp.xp_for_next_level() - 100.0).abs() < 0.001); // level 1: scale^0 = 1
        let mut xp2 = xp.clone();
        xp2.level = 2;
        assert!((xp2.xp_for_next_level() - 200.0).abs() < 0.001); // level 2: scale^1 = 2
    }

    #[test]
    fn disabled_experience_no_gain() {
        let mut xp = Experience::new(100.0).disabled();
        let gained = xp.add_xp(200.0);
        assert_eq!(gained, 0);
        assert_eq!(xp.level, 1);
    }
}
