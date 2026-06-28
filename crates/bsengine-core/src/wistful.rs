use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wistful {
    pub longing: f32,
    pub max_longing: f32,
    pub pine_rate: f32,
    pub just_pining: bool,
    pub just_content: bool,
    pub enabled: bool,
}

impl Default for Wistful {
    fn default() -> Self {
        Self {
            longing: 0.0,
            max_longing: 100.0,
            pine_rate: 1.0,
            just_pining: false,
            just_content: false,
            enabled: true,
        }
    }
}

impl Wistful {
    pub fn pine(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_pining = false;
        self.just_content = false;
        let prev = self.longing;
        self.longing = (self.longing + amount).clamp(0.0, self.max_longing);
        if self.longing >= self.max_longing && prev < self.max_longing {
            self.just_pining = true;
        }
    }

    pub fn console(&mut self, amount: f32) {
        if !self.enabled || self.longing <= 0.0 {
            return;
        }
        self.just_pining = false;
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
        self.pine(self.pine_rate * dt);
    }

    pub fn is_pining(&self) -> bool {
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

    pub fn effective_yearning(&self, scale: f32) -> f32 {
        self.longing_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wistful() -> Wistful {
        Wistful {
            longing: 0.0,
            max_longing: 100.0,
            pine_rate: 10.0,
            just_pining: false,
            just_content: false,
            enabled: true,
        }
    }

    #[test]
    fn default_longing_zero() {
        let w = Wistful::default();
        assert_eq!(w.longing, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wistful::default().enabled);
    }

    #[test]
    fn pine_increases_longing() {
        let mut w = wistful();
        w.pine(30.0);
        assert_eq!(w.longing, 30.0);
    }

    #[test]
    fn pine_clamps_at_max() {
        let mut w = wistful();
        w.pine(200.0);
        assert_eq!(w.longing, 100.0);
    }

    #[test]
    fn pine_no_op_when_disabled() {
        let mut w = wistful();
        w.enabled = false;
        w.pine(50.0);
        assert_eq!(w.longing, 0.0);
    }

    #[test]
    fn pine_sets_just_pining_at_max() {
        let mut w = wistful();
        w.pine(100.0);
        assert!(w.just_pining);
    }

    #[test]
    fn pine_no_just_pining_if_already_max() {
        let mut w = wistful();
        w.longing = 100.0;
        w.pine(1.0);
        assert!(!w.just_pining);
    }

    #[test]
    fn console_decreases_longing() {
        let mut w = wistful();
        w.longing = 60.0;
        w.console(20.0);
        assert_eq!(w.longing, 40.0);
    }

    #[test]
    fn console_clamps_at_zero() {
        let mut w = wistful();
        w.longing = 30.0;
        w.console(200.0);
        assert_eq!(w.longing, 0.0);
    }

    #[test]
    fn console_no_op_when_disabled() {
        let mut w = wistful();
        w.longing = 50.0;
        w.enabled = false;
        w.console(10.0);
        assert_eq!(w.longing, 50.0);
    }

    #[test]
    fn console_no_op_when_already_content() {
        let mut w = wistful();
        w.console(10.0);
        assert_eq!(w.longing, 0.0);
    }

    #[test]
    fn console_sets_just_content_at_zero() {
        let mut w = wistful();
        w.longing = 10.0;
        w.console(10.0);
        assert!(w.just_content);
    }

    #[test]
    fn console_no_just_content_if_already_zero() {
        let mut w = wistful();
        w.console(1.0);
        assert!(!w.just_content);
    }

    #[test]
    fn tick_increases_longing() {
        let mut w = wistful();
        w.tick(1.0);
        assert_eq!(w.longing, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wistful();
        w.tick(2.0);
        assert_eq!(w.longing, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wistful();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.longing, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_pining() {
        let mut w = wistful();
        w.longing = 100.0;
        w.tick(1.0);
        assert_eq!(w.longing, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wistful();
        w.pine_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.longing, 0.0);
    }

    #[test]
    fn is_pining_true_at_max() {
        let mut w = wistful();
        w.longing = 100.0;
        assert!(w.is_pining());
    }

    #[test]
    fn is_pining_false_below_max() {
        let mut w = wistful();
        w.longing = 50.0;
        assert!(!w.is_pining());
    }

    #[test]
    fn is_pining_false_when_disabled() {
        let mut w = wistful();
        w.longing = 100.0;
        w.enabled = false;
        assert!(!w.is_pining());
    }

    #[test]
    fn is_content_true_at_zero() {
        let w = wistful();
        assert!(w.is_content());
    }

    #[test]
    fn is_content_false_above_zero() {
        let mut w = wistful();
        w.longing = 1.0;
        assert!(!w.is_content());
    }

    #[test]
    fn longing_fraction_zero_when_content() {
        let w = wistful();
        assert_eq!(w.longing_fraction(), 0.0);
    }

    #[test]
    fn longing_fraction_one_at_max() {
        let mut w = wistful();
        w.longing = 100.0;
        assert_eq!(w.longing_fraction(), 1.0);
    }

    #[test]
    fn longing_fraction_half_at_midpoint() {
        let mut w = wistful();
        w.longing = 50.0;
        assert_eq!(w.longing_fraction(), 0.5);
    }

    #[test]
    fn longing_fraction_zero_when_max_zero() {
        let mut w = wistful();
        w.max_longing = 0.0;
        assert_eq!(w.longing_fraction(), 0.0);
    }

    #[test]
    fn effective_yearning_scales() {
        let mut w = wistful();
        w.longing = 50.0;
        assert_eq!(w.effective_yearning(2.0), 1.0);
    }

    #[test]
    fn effective_yearning_zero_when_content() {
        let w = wistful();
        assert_eq!(w.effective_yearning(10.0), 0.0);
    }

    #[test]
    fn just_pining_cleared_on_next_pine() {
        let mut w = wistful();
        w.pine(100.0);
        assert!(w.just_pining);
        w.pine(1.0);
        assert!(!w.just_pining);
    }

    #[test]
    fn just_content_cleared_on_next_console() {
        let mut w = wistful();
        w.longing = 10.0;
        w.console(10.0);
        assert!(w.just_content);
        w.longing = 10.0;
        w.console(1.0);
        assert!(!w.just_content);
    }
}
