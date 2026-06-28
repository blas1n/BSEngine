use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wish {
    pub longing: f32,
    pub max_longing: f32,
    pub yearn_rate: f32,
    pub just_yearning: bool,
    pub just_content: bool,
    pub enabled: bool,
}

impl Default for Wish {
    fn default() -> Self {
        Self {
            longing: 0.0,
            max_longing: 100.0,
            yearn_rate: 1.0,
            just_yearning: false,
            just_content: false,
            enabled: true,
        }
    }
}

impl Wish {
    pub fn yearn(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_yearning = false;
        self.just_content = false;
        let prev = self.longing;
        self.longing = (self.longing + amount).clamp(0.0, self.max_longing);
        if self.longing >= self.max_longing && prev < self.max_longing {
            self.just_yearning = true;
        }
    }

    pub fn content(&mut self, amount: f32) {
        if !self.enabled || self.longing <= 0.0 {
            return;
        }
        self.just_yearning = false;
        self.just_content = false;
        let prev = self.longing;
        self.longing = (self.longing - amount).max(0.0);
        if self.longing <= 0.0 && prev > 0.0 {
            self.just_content = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.longing >= self.max_longing {
            return;
        }
        self.yearn(self.yearn_rate * dt);
    }

    pub fn is_yearning(&self) -> bool {
        self.enabled && self.longing >= self.max_longing
    }

    pub fn is_content(&self) -> bool {
        self.longing <= 0.0
    }

    pub fn longing_fraction(&self) -> f32 {
        if self.max_longing <= 0.0 {
            return 0.0;
        }
        self.longing / self.max_longing
    }

    pub fn effective_desire(&self, scale: f32) -> f32 {
        self.longing_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wish() -> Wish {
        Wish {
            longing: 0.0,
            max_longing: 100.0,
            yearn_rate: 10.0,
            just_yearning: false,
            just_content: false,
            enabled: true,
        }
    }

    #[test]
    fn default_longing_zero() {
        let w = Wish::default();
        assert_eq!(w.longing, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wish::default().enabled);
    }

    #[test]
    fn yearn_increases_longing() {
        let mut w = wish();
        w.yearn(30.0);
        assert_eq!(w.longing, 30.0);
    }

    #[test]
    fn yearn_clamps_at_max() {
        let mut w = wish();
        w.yearn(200.0);
        assert_eq!(w.longing, 100.0);
    }

    #[test]
    fn yearn_no_op_when_disabled() {
        let mut w = wish();
        w.enabled = false;
        w.yearn(50.0);
        assert_eq!(w.longing, 0.0);
    }

    #[test]
    fn yearn_sets_just_yearning_at_max() {
        let mut w = wish();
        w.yearn(100.0);
        assert!(w.just_yearning);
    }

    #[test]
    fn yearn_no_just_yearning_if_already_max() {
        let mut w = wish();
        w.longing = 100.0;
        w.yearn(1.0);
        assert!(!w.just_yearning);
    }

    #[test]
    fn content_decreases_longing() {
        let mut w = wish();
        w.longing = 60.0;
        w.content(20.0);
        assert_eq!(w.longing, 40.0);
    }

    #[test]
    fn content_clamps_at_zero() {
        let mut w = wish();
        w.longing = 30.0;
        w.content(200.0);
        assert_eq!(w.longing, 0.0);
    }

    #[test]
    fn content_no_op_when_disabled() {
        let mut w = wish();
        w.longing = 50.0;
        w.enabled = false;
        w.content(10.0);
        assert_eq!(w.longing, 50.0);
    }

    #[test]
    fn content_no_op_when_already_content() {
        let mut w = wish();
        w.content(10.0);
        assert_eq!(w.longing, 0.0);
    }

    #[test]
    fn content_sets_just_content_at_zero() {
        let mut w = wish();
        w.longing = 10.0;
        w.content(10.0);
        assert!(w.just_content);
    }

    #[test]
    fn content_no_just_content_if_already_zero() {
        let mut w = wish();
        w.content(1.0);
        assert!(!w.just_content);
    }

    #[test]
    fn tick_increases_longing() {
        let mut w = wish();
        w.tick(1.0);
        assert_eq!(w.longing, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wish();
        w.tick(2.0);
        assert_eq!(w.longing, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wish();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.longing, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_yearning() {
        let mut w = wish();
        w.longing = 100.0;
        w.tick(1.0);
        assert_eq!(w.longing, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wish();
        w.yearn_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.longing, 0.0);
    }

    #[test]
    fn is_yearning_true_at_max() {
        let mut w = wish();
        w.longing = 100.0;
        assert!(w.is_yearning());
    }

    #[test]
    fn is_yearning_false_below_max() {
        let mut w = wish();
        w.longing = 50.0;
        assert!(!w.is_yearning());
    }

    #[test]
    fn is_yearning_false_when_disabled() {
        let mut w = wish();
        w.longing = 100.0;
        w.enabled = false;
        assert!(!w.is_yearning());
    }

    #[test]
    fn is_content_true_at_zero() {
        let w = wish();
        assert!(w.is_content());
    }

    #[test]
    fn is_content_false_above_zero() {
        let mut w = wish();
        w.longing = 1.0;
        assert!(!w.is_content());
    }

    #[test]
    fn longing_fraction_zero_when_content() {
        let w = wish();
        assert_eq!(w.longing_fraction(), 0.0);
    }

    #[test]
    fn longing_fraction_one_at_max() {
        let mut w = wish();
        w.longing = 100.0;
        assert_eq!(w.longing_fraction(), 1.0);
    }

    #[test]
    fn longing_fraction_half_at_midpoint() {
        let mut w = wish();
        w.longing = 50.0;
        assert_eq!(w.longing_fraction(), 0.5);
    }

    #[test]
    fn longing_fraction_zero_when_max_zero() {
        let mut w = wish();
        w.max_longing = 0.0;
        assert_eq!(w.longing_fraction(), 0.0);
    }

    #[test]
    fn effective_desire_scales() {
        let mut w = wish();
        w.longing = 50.0;
        assert_eq!(w.effective_desire(2.0), 1.0);
    }

    #[test]
    fn effective_desire_zero_when_content() {
        let w = wish();
        assert_eq!(w.effective_desire(10.0), 0.0);
    }

    #[test]
    fn just_yearning_cleared_on_next_yearn() {
        let mut w = wish();
        w.yearn(100.0);
        assert!(w.just_yearning);
        w.yearn(1.0);
        assert!(!w.just_yearning);
    }

    #[test]
    fn just_content_cleared_on_next_content() {
        let mut w = wish();
        w.longing = 10.0;
        w.content(10.0);
        assert!(w.just_content);
        w.longing = 10.0;
        w.content(1.0);
        assert!(!w.just_content);
    }
}
