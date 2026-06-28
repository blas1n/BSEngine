use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wore {
    pub wear: f32,
    pub max_wear: f32,
    pub abrade_rate: f32,
    pub just_worn: bool,
    pub just_fresh: bool,
    pub enabled: bool,
}

impl Default for Wore {
    fn default() -> Self {
        Self {
            wear: 0.0,
            max_wear: 100.0,
            abrade_rate: 1.0,
            just_worn: false,
            just_fresh: false,
            enabled: true,
        }
    }
}

impl Wore {
    pub fn abrade(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_worn = false;
        self.just_fresh = false;
        let prev = self.wear;
        self.wear = (self.wear + amount).clamp(0.0, self.max_wear);
        if self.wear >= self.max_wear && prev < self.max_wear {
            self.just_worn = true;
        }
    }

    pub fn restore(&mut self, amount: f32) {
        if !self.enabled || self.wear <= 0.0 {
            return;
        }
        self.just_worn = false;
        self.just_fresh = false;
        let prev = self.wear;
        self.wear = (self.wear - amount).max(0.0);
        if self.wear <= 0.0 && prev > 0.0 {
            self.just_fresh = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.wear >= self.max_wear {
            return;
        }
        self.abrade(self.abrade_rate * dt);
    }

    pub fn is_worn(&self) -> bool {
        self.enabled && self.wear >= self.max_wear
    }

    pub fn is_fresh(&self) -> bool {
        self.wear <= 0.0
    }

    pub fn wear_fraction(&self) -> f32 {
        if self.max_wear <= 0.0 {
            return 0.0;
        }
        self.wear / self.max_wear
    }

    pub fn effective_deterioration(&self, scale: f32) -> f32 {
        self.wear_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wore() -> Wore {
        Wore {
            wear: 0.0,
            max_wear: 100.0,
            abrade_rate: 10.0,
            just_worn: false,
            just_fresh: false,
            enabled: true,
        }
    }

    #[test]
    fn default_wear_zero() {
        let w = Wore::default();
        assert_eq!(w.wear, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wore::default().enabled);
    }

    #[test]
    fn abrade_increases_wear() {
        let mut w = wore();
        w.abrade(30.0);
        assert_eq!(w.wear, 30.0);
    }

    #[test]
    fn abrade_clamps_at_max() {
        let mut w = wore();
        w.abrade(200.0);
        assert_eq!(w.wear, 100.0);
    }

    #[test]
    fn abrade_no_op_when_disabled() {
        let mut w = wore();
        w.enabled = false;
        w.abrade(50.0);
        assert_eq!(w.wear, 0.0);
    }

    #[test]
    fn abrade_sets_just_worn_at_max() {
        let mut w = wore();
        w.abrade(100.0);
        assert!(w.just_worn);
    }

    #[test]
    fn abrade_no_just_worn_if_already_max() {
        let mut w = wore();
        w.wear = 100.0;
        w.abrade(1.0);
        assert!(!w.just_worn);
    }

    #[test]
    fn restore_decreases_wear() {
        let mut w = wore();
        w.wear = 60.0;
        w.restore(20.0);
        assert_eq!(w.wear, 40.0);
    }

    #[test]
    fn restore_clamps_at_zero() {
        let mut w = wore();
        w.wear = 30.0;
        w.restore(200.0);
        assert_eq!(w.wear, 0.0);
    }

    #[test]
    fn restore_no_op_when_disabled() {
        let mut w = wore();
        w.wear = 50.0;
        w.enabled = false;
        w.restore(10.0);
        assert_eq!(w.wear, 50.0);
    }

    #[test]
    fn restore_no_op_when_already_fresh() {
        let mut w = wore();
        w.restore(10.0);
        assert_eq!(w.wear, 0.0);
    }

    #[test]
    fn restore_sets_just_fresh_at_zero() {
        let mut w = wore();
        w.wear = 10.0;
        w.restore(10.0);
        assert!(w.just_fresh);
    }

    #[test]
    fn restore_no_just_fresh_if_already_zero() {
        let mut w = wore();
        w.restore(1.0);
        assert!(!w.just_fresh);
    }

    #[test]
    fn tick_increases_wear() {
        let mut w = wore();
        w.tick(1.0);
        assert_eq!(w.wear, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wore();
        w.tick(2.0);
        assert_eq!(w.wear, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wore();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.wear, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_worn() {
        let mut w = wore();
        w.wear = 100.0;
        w.tick(1.0);
        assert_eq!(w.wear, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wore();
        w.abrade_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.wear, 0.0);
    }

    #[test]
    fn is_worn_true_at_max() {
        let mut w = wore();
        w.wear = 100.0;
        assert!(w.is_worn());
    }

    #[test]
    fn is_worn_false_below_max() {
        let mut w = wore();
        w.wear = 50.0;
        assert!(!w.is_worn());
    }

    #[test]
    fn is_worn_false_when_disabled() {
        let mut w = wore();
        w.wear = 100.0;
        w.enabled = false;
        assert!(!w.is_worn());
    }

    #[test]
    fn is_fresh_true_at_zero() {
        let w = wore();
        assert!(w.is_fresh());
    }

    #[test]
    fn is_fresh_false_above_zero() {
        let mut w = wore();
        w.wear = 1.0;
        assert!(!w.is_fresh());
    }

    #[test]
    fn wear_fraction_zero_when_fresh() {
        let w = wore();
        assert_eq!(w.wear_fraction(), 0.0);
    }

    #[test]
    fn wear_fraction_one_at_max() {
        let mut w = wore();
        w.wear = 100.0;
        assert_eq!(w.wear_fraction(), 1.0);
    }

    #[test]
    fn wear_fraction_half_at_midpoint() {
        let mut w = wore();
        w.wear = 50.0;
        assert_eq!(w.wear_fraction(), 0.5);
    }

    #[test]
    fn wear_fraction_zero_when_max_zero() {
        let mut w = wore();
        w.max_wear = 0.0;
        assert_eq!(w.wear_fraction(), 0.0);
    }

    #[test]
    fn effective_deterioration_scales() {
        let mut w = wore();
        w.wear = 50.0;
        assert_eq!(w.effective_deterioration(2.0), 1.0);
    }

    #[test]
    fn effective_deterioration_zero_when_fresh() {
        let w = wore();
        assert_eq!(w.effective_deterioration(10.0), 0.0);
    }

    #[test]
    fn just_worn_cleared_on_next_abrade() {
        let mut w = wore();
        w.abrade(100.0);
        assert!(w.just_worn);
        w.abrade(1.0);
        assert!(!w.just_worn);
    }

    #[test]
    fn just_fresh_cleared_on_next_restore() {
        let mut w = wore();
        w.wear = 10.0;
        w.restore(10.0);
        assert!(w.just_fresh);
        w.wear = 10.0;
        w.restore(1.0);
        assert!(!w.just_fresh);
    }
}
