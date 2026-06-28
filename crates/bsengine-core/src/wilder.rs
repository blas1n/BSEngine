use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wilder {
    pub frenzy: f32,
    pub max_frenzy: f32,
    pub ramp_rate: f32,
    pub just_frenzied: bool,
    pub just_calmed: bool,
    pub enabled: bool,
}

impl Default for Wilder {
    fn default() -> Self {
        Self {
            frenzy: 0.0,
            max_frenzy: 100.0,
            ramp_rate: 1.0,
            just_frenzied: false,
            just_calmed: false,
            enabled: true,
        }
    }
}

impl Wilder {
    pub fn ramp(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_frenzied = false;
        self.just_calmed = false;
        let prev = self.frenzy;
        self.frenzy = (self.frenzy + amount).clamp(0.0, self.max_frenzy);
        if self.frenzy >= self.max_frenzy && prev < self.max_frenzy {
            self.just_frenzied = true;
        }
    }

    pub fn calm(&mut self, amount: f32) {
        if !self.enabled || self.frenzy <= 0.0 {
            return;
        }
        self.just_frenzied = false;
        self.just_calmed = false;
        let prev = self.frenzy;
        self.frenzy = (self.frenzy - amount).max(0.0);
        if self.frenzy <= 0.0 && prev > 0.0 {
            self.just_calmed = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.frenzy >= self.max_frenzy {
            return;
        }
        self.ramp(self.ramp_rate * dt);
    }

    pub fn is_frenzied(&self) -> bool {
        self.enabled && self.frenzy >= self.max_frenzy
    }

    pub fn is_calmed(&self) -> bool {
        self.frenzy <= 0.0
    }

    pub fn frenzy_fraction(&self) -> f32 {
        if self.max_frenzy <= 0.0 {
            return 0.0;
        }
        self.frenzy / self.max_frenzy
    }

    pub fn effective_chaos(&self, scale: f32) -> f32 {
        self.frenzy_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wilder() -> Wilder {
        Wilder {
            frenzy: 0.0,
            max_frenzy: 100.0,
            ramp_rate: 10.0,
            just_frenzied: false,
            just_calmed: false,
            enabled: true,
        }
    }

    #[test]
    fn default_frenzy_zero() {
        let w = Wilder::default();
        assert_eq!(w.frenzy, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wilder::default().enabled);
    }

    #[test]
    fn ramp_increases_frenzy() {
        let mut w = wilder();
        w.ramp(30.0);
        assert_eq!(w.frenzy, 30.0);
    }

    #[test]
    fn ramp_clamps_at_max() {
        let mut w = wilder();
        w.ramp(200.0);
        assert_eq!(w.frenzy, 100.0);
    }

    #[test]
    fn ramp_no_op_when_disabled() {
        let mut w = wilder();
        w.enabled = false;
        w.ramp(50.0);
        assert_eq!(w.frenzy, 0.0);
    }

    #[test]
    fn ramp_sets_just_frenzied_at_max() {
        let mut w = wilder();
        w.ramp(100.0);
        assert!(w.just_frenzied);
    }

    #[test]
    fn ramp_no_just_frenzied_if_already_max() {
        let mut w = wilder();
        w.frenzy = 100.0;
        w.ramp(1.0);
        assert!(!w.just_frenzied);
    }

    #[test]
    fn calm_decreases_frenzy() {
        let mut w = wilder();
        w.frenzy = 60.0;
        w.calm(20.0);
        assert_eq!(w.frenzy, 40.0);
    }

    #[test]
    fn calm_clamps_at_zero() {
        let mut w = wilder();
        w.frenzy = 30.0;
        w.calm(200.0);
        assert_eq!(w.frenzy, 0.0);
    }

    #[test]
    fn calm_no_op_when_disabled() {
        let mut w = wilder();
        w.frenzy = 50.0;
        w.enabled = false;
        w.calm(10.0);
        assert_eq!(w.frenzy, 50.0);
    }

    #[test]
    fn calm_no_op_when_already_calmed() {
        let mut w = wilder();
        w.calm(10.0);
        assert_eq!(w.frenzy, 0.0);
    }

    #[test]
    fn calm_sets_just_calmed_at_zero() {
        let mut w = wilder();
        w.frenzy = 10.0;
        w.calm(10.0);
        assert!(w.just_calmed);
    }

    #[test]
    fn calm_no_just_calmed_if_already_zero() {
        let mut w = wilder();
        w.calm(1.0);
        assert!(!w.just_calmed);
    }

    #[test]
    fn tick_increases_frenzy() {
        let mut w = wilder();
        w.tick(1.0);
        assert_eq!(w.frenzy, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wilder();
        w.tick(2.0);
        assert_eq!(w.frenzy, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wilder();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.frenzy, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_frenzied() {
        let mut w = wilder();
        w.frenzy = 100.0;
        w.tick(1.0);
        assert_eq!(w.frenzy, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wilder();
        w.ramp_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.frenzy, 0.0);
    }

    #[test]
    fn is_frenzied_true_at_max() {
        let mut w = wilder();
        w.frenzy = 100.0;
        assert!(w.is_frenzied());
    }

    #[test]
    fn is_frenzied_false_below_max() {
        let mut w = wilder();
        w.frenzy = 50.0;
        assert!(!w.is_frenzied());
    }

    #[test]
    fn is_frenzied_false_when_disabled() {
        let mut w = wilder();
        w.frenzy = 100.0;
        w.enabled = false;
        assert!(!w.is_frenzied());
    }

    #[test]
    fn is_calmed_true_at_zero() {
        let w = wilder();
        assert!(w.is_calmed());
    }

    #[test]
    fn is_calmed_false_above_zero() {
        let mut w = wilder();
        w.frenzy = 1.0;
        assert!(!w.is_calmed());
    }

    #[test]
    fn frenzy_fraction_zero_when_calmed() {
        let w = wilder();
        assert_eq!(w.frenzy_fraction(), 0.0);
    }

    #[test]
    fn frenzy_fraction_one_at_max() {
        let mut w = wilder();
        w.frenzy = 100.0;
        assert_eq!(w.frenzy_fraction(), 1.0);
    }

    #[test]
    fn frenzy_fraction_half_at_midpoint() {
        let mut w = wilder();
        w.frenzy = 50.0;
        assert_eq!(w.frenzy_fraction(), 0.5);
    }

    #[test]
    fn frenzy_fraction_zero_when_max_zero() {
        let mut w = wilder();
        w.max_frenzy = 0.0;
        assert_eq!(w.frenzy_fraction(), 0.0);
    }

    #[test]
    fn effective_chaos_scales() {
        let mut w = wilder();
        w.frenzy = 50.0;
        assert_eq!(w.effective_chaos(2.0), 1.0);
    }

    #[test]
    fn effective_chaos_zero_when_calmed() {
        let w = wilder();
        assert_eq!(w.effective_chaos(10.0), 0.0);
    }

    #[test]
    fn just_frenzied_cleared_on_next_ramp() {
        let mut w = wilder();
        w.ramp(100.0);
        assert!(w.just_frenzied);
        w.ramp(1.0);
        assert!(!w.just_frenzied);
    }

    #[test]
    fn just_calmed_cleared_on_next_calm() {
        let mut w = wilder();
        w.frenzy = 10.0;
        w.calm(10.0);
        assert!(w.just_calmed);
        w.frenzy = 10.0;
        w.calm(1.0);
        assert!(!w.just_calmed);
    }
}
