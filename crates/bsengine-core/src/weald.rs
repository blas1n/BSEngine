use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Weald {
    pub canopy: f32,
    pub max_canopy: f32,
    pub grow_rate: f32,
    pub just_forested: bool,
    pub just_cleared: bool,
    pub enabled: bool,
}

impl Default for Weald {
    fn default() -> Self {
        Self {
            canopy: 0.0,
            max_canopy: 100.0,
            grow_rate: 1.0,
            just_forested: false,
            just_cleared: false,
            enabled: true,
        }
    }
}

impl Weald {
    pub fn grow(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_forested = false;
        self.just_cleared = false;
        let prev = self.canopy;
        self.canopy = (self.canopy + amount).clamp(0.0, self.max_canopy);
        if self.canopy >= self.max_canopy && prev < self.max_canopy {
            self.just_forested = true;
        }
    }

    pub fn clear(&mut self, amount: f32) {
        if !self.enabled || self.canopy <= 0.0 {
            return;
        }
        self.just_forested = false;
        self.just_cleared = false;
        let prev = self.canopy;
        self.canopy = (self.canopy - amount).max(0.0);
        if self.canopy <= 0.0 && prev > 0.0 {
            self.just_cleared = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.canopy >= self.max_canopy {
            return;
        }
        self.grow(self.grow_rate * dt);
    }

    pub fn is_forested(&self) -> bool {
        self.enabled && self.canopy >= self.max_canopy
    }

    pub fn is_cleared(&self) -> bool {
        self.canopy <= 0.0
    }

    pub fn canopy_fraction(&self) -> f32 {
        if self.max_canopy <= 0.0 {
            return 0.0;
        }
        self.canopy / self.max_canopy
    }

    pub fn effective_shade(&self, scale: f32) -> f32 {
        self.canopy_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weald() -> Weald {
        Weald {
            canopy: 0.0,
            max_canopy: 100.0,
            grow_rate: 10.0,
            just_forested: false,
            just_cleared: false,
            enabled: true,
        }
    }

    #[test]
    fn default_canopy_zero() {
        let w = Weald::default();
        assert_eq!(w.canopy, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Weald::default().enabled);
    }

    #[test]
    fn grow_increases_canopy() {
        let mut w = weald();
        w.grow(30.0);
        assert_eq!(w.canopy, 30.0);
    }

    #[test]
    fn grow_clamps_at_max() {
        let mut w = weald();
        w.grow(200.0);
        assert_eq!(w.canopy, 100.0);
    }

    #[test]
    fn grow_no_op_when_disabled() {
        let mut w = weald();
        w.enabled = false;
        w.grow(50.0);
        assert_eq!(w.canopy, 0.0);
    }

    #[test]
    fn grow_sets_just_forested_at_max() {
        let mut w = weald();
        w.grow(100.0);
        assert!(w.just_forested);
    }

    #[test]
    fn grow_no_just_forested_if_already_max() {
        let mut w = weald();
        w.canopy = 100.0;
        w.grow(1.0);
        assert!(!w.just_forested);
    }

    #[test]
    fn clear_decreases_canopy() {
        let mut w = weald();
        w.canopy = 60.0;
        w.clear(20.0);
        assert_eq!(w.canopy, 40.0);
    }

    #[test]
    fn clear_clamps_at_zero() {
        let mut w = weald();
        w.canopy = 30.0;
        w.clear(200.0);
        assert_eq!(w.canopy, 0.0);
    }

    #[test]
    fn clear_no_op_when_disabled() {
        let mut w = weald();
        w.canopy = 50.0;
        w.enabled = false;
        w.clear(10.0);
        assert_eq!(w.canopy, 50.0);
    }

    #[test]
    fn clear_no_op_when_already_cleared() {
        let mut w = weald();
        w.clear(10.0);
        assert_eq!(w.canopy, 0.0);
    }

    #[test]
    fn clear_sets_just_cleared_at_zero() {
        let mut w = weald();
        w.canopy = 10.0;
        w.clear(10.0);
        assert!(w.just_cleared);
    }

    #[test]
    fn clear_no_just_cleared_if_already_zero() {
        let mut w = weald();
        w.clear(1.0);
        assert!(!w.just_cleared);
    }

    #[test]
    fn tick_increases_canopy() {
        let mut w = weald();
        w.tick(1.0);
        assert_eq!(w.canopy, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = weald();
        w.tick(2.0);
        assert_eq!(w.canopy, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = weald();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.canopy, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_forested() {
        let mut w = weald();
        w.canopy = 100.0;
        w.tick(1.0);
        assert_eq!(w.canopy, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = weald();
        w.grow_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.canopy, 0.0);
    }

    #[test]
    fn is_forested_true_at_max() {
        let mut w = weald();
        w.canopy = 100.0;
        assert!(w.is_forested());
    }

    #[test]
    fn is_forested_false_below_max() {
        let mut w = weald();
        w.canopy = 50.0;
        assert!(!w.is_forested());
    }

    #[test]
    fn is_forested_false_when_disabled() {
        let mut w = weald();
        w.canopy = 100.0;
        w.enabled = false;
        assert!(!w.is_forested());
    }

    #[test]
    fn is_cleared_true_at_zero() {
        let w = weald();
        assert!(w.is_cleared());
    }

    #[test]
    fn is_cleared_false_above_zero() {
        let mut w = weald();
        w.canopy = 1.0;
        assert!(!w.is_cleared());
    }

    #[test]
    fn canopy_fraction_zero_when_cleared() {
        let w = weald();
        assert_eq!(w.canopy_fraction(), 0.0);
    }

    #[test]
    fn canopy_fraction_one_at_max() {
        let mut w = weald();
        w.canopy = 100.0;
        assert_eq!(w.canopy_fraction(), 1.0);
    }

    #[test]
    fn canopy_fraction_half_at_midpoint() {
        let mut w = weald();
        w.canopy = 50.0;
        assert_eq!(w.canopy_fraction(), 0.5);
    }

    #[test]
    fn canopy_fraction_zero_when_max_zero() {
        let mut w = weald();
        w.max_canopy = 0.0;
        assert_eq!(w.canopy_fraction(), 0.0);
    }

    #[test]
    fn effective_shade_scales() {
        let mut w = weald();
        w.canopy = 50.0;
        assert_eq!(w.effective_shade(2.0), 1.0);
    }

    #[test]
    fn effective_shade_zero_when_cleared() {
        let w = weald();
        assert_eq!(w.effective_shade(10.0), 0.0);
    }

    #[test]
    fn just_forested_cleared_on_next_grow() {
        let mut w = weald();
        w.grow(100.0);
        assert!(w.just_forested);
        w.grow(1.0);
        assert!(!w.just_forested);
    }

    #[test]
    fn just_cleared_cleared_on_next_clear() {
        let mut w = weald();
        w.canopy = 10.0;
        w.clear(10.0);
        assert!(w.just_cleared);
        w.canopy = 10.0;
        w.clear(1.0);
        assert!(!w.just_cleared);
    }
}
