use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Weep {
    pub grief: f32,
    pub max_grief: f32,
    pub sorrow_rate: f32,
    pub just_weeping: bool,
    pub just_consoled: bool,
    pub enabled: bool,
}

impl Default for Weep {
    fn default() -> Self {
        Self {
            grief: 0.0,
            max_grief: 100.0,
            sorrow_rate: 1.0,
            just_weeping: false,
            just_consoled: false,
            enabled: true,
        }
    }
}

impl Weep {
    pub fn mourn(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_weeping = false;
        self.just_consoled = false;
        let prev = self.grief;
        self.grief = (self.grief + amount).clamp(0.0, self.max_grief);
        if self.grief >= self.max_grief && prev < self.max_grief {
            self.just_weeping = true;
        }
    }

    pub fn console(&mut self, amount: f32) {
        if !self.enabled || self.grief <= 0.0 {
            return;
        }
        self.just_weeping = false;
        self.just_consoled = false;
        let prev = self.grief;
        self.grief = (self.grief - amount).max(0.0);
        if self.grief <= 0.0 && prev > 0.0 {
            self.just_consoled = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.grief >= self.max_grief {
            return;
        }
        self.mourn(self.sorrow_rate * dt);
    }

    pub fn is_weeping(&self) -> bool {
        self.enabled && self.grief >= self.max_grief
    }

    pub fn is_consoled(&self) -> bool {
        self.grief <= 0.0
    }

    pub fn grief_fraction(&self) -> f32 {
        if self.max_grief <= 0.0 {
            return 0.0;
        }
        self.grief / self.max_grief
    }

    pub fn effective_sorrow(&self, scale: f32) -> f32 {
        self.grief_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weep() -> Weep {
        Weep {
            grief: 0.0,
            max_grief: 100.0,
            sorrow_rate: 10.0,
            just_weeping: false,
            just_consoled: false,
            enabled: true,
        }
    }

    #[test]
    fn default_grief_zero() {
        let w = Weep::default();
        assert_eq!(w.grief, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Weep::default().enabled);
    }

    #[test]
    fn mourn_increases_grief() {
        let mut w = weep();
        w.mourn(30.0);
        assert_eq!(w.grief, 30.0);
    }

    #[test]
    fn mourn_clamps_at_max() {
        let mut w = weep();
        w.mourn(200.0);
        assert_eq!(w.grief, 100.0);
    }

    #[test]
    fn mourn_no_op_when_disabled() {
        let mut w = weep();
        w.enabled = false;
        w.mourn(50.0);
        assert_eq!(w.grief, 0.0);
    }

    #[test]
    fn mourn_sets_just_weeping_at_max() {
        let mut w = weep();
        w.mourn(100.0);
        assert!(w.just_weeping);
    }

    #[test]
    fn mourn_no_just_weeping_if_already_max() {
        let mut w = weep();
        w.grief = 100.0;
        w.mourn(1.0);
        assert!(!w.just_weeping);
    }

    #[test]
    fn console_decreases_grief() {
        let mut w = weep();
        w.grief = 60.0;
        w.console(20.0);
        assert_eq!(w.grief, 40.0);
    }

    #[test]
    fn console_clamps_at_zero() {
        let mut w = weep();
        w.grief = 30.0;
        w.console(200.0);
        assert_eq!(w.grief, 0.0);
    }

    #[test]
    fn console_no_op_when_disabled() {
        let mut w = weep();
        w.grief = 50.0;
        w.enabled = false;
        w.console(10.0);
        assert_eq!(w.grief, 50.0);
    }

    #[test]
    fn console_no_op_when_already_consoled() {
        let mut w = weep();
        w.console(10.0);
        assert_eq!(w.grief, 0.0);
    }

    #[test]
    fn console_sets_just_consoled_at_zero() {
        let mut w = weep();
        w.grief = 10.0;
        w.console(10.0);
        assert!(w.just_consoled);
    }

    #[test]
    fn console_no_just_consoled_if_already_zero() {
        let mut w = weep();
        w.console(1.0);
        assert!(!w.just_consoled);
    }

    #[test]
    fn tick_increases_grief() {
        let mut w = weep();
        w.tick(1.0);
        assert_eq!(w.grief, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = weep();
        w.tick(2.0);
        assert_eq!(w.grief, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = weep();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.grief, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_weeping() {
        let mut w = weep();
        w.grief = 100.0;
        w.tick(1.0);
        assert_eq!(w.grief, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = weep();
        w.sorrow_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.grief, 0.0);
    }

    #[test]
    fn is_weeping_true_at_max() {
        let mut w = weep();
        w.grief = 100.0;
        assert!(w.is_weeping());
    }

    #[test]
    fn is_weeping_false_below_max() {
        let mut w = weep();
        w.grief = 50.0;
        assert!(!w.is_weeping());
    }

    #[test]
    fn is_weeping_false_when_disabled() {
        let mut w = weep();
        w.grief = 100.0;
        w.enabled = false;
        assert!(!w.is_weeping());
    }

    #[test]
    fn is_consoled_true_at_zero() {
        let w = weep();
        assert!(w.is_consoled());
    }

    #[test]
    fn is_consoled_false_above_zero() {
        let mut w = weep();
        w.grief = 1.0;
        assert!(!w.is_consoled());
    }

    #[test]
    fn grief_fraction_zero_when_consoled() {
        let w = weep();
        assert_eq!(w.grief_fraction(), 0.0);
    }

    #[test]
    fn grief_fraction_one_at_max() {
        let mut w = weep();
        w.grief = 100.0;
        assert_eq!(w.grief_fraction(), 1.0);
    }

    #[test]
    fn grief_fraction_half_at_midpoint() {
        let mut w = weep();
        w.grief = 50.0;
        assert_eq!(w.grief_fraction(), 0.5);
    }

    #[test]
    fn grief_fraction_zero_when_max_zero() {
        let mut w = weep();
        w.max_grief = 0.0;
        assert_eq!(w.grief_fraction(), 0.0);
    }

    #[test]
    fn effective_sorrow_scales() {
        let mut w = weep();
        w.grief = 50.0;
        assert_eq!(w.effective_sorrow(2.0), 1.0);
    }

    #[test]
    fn effective_sorrow_zero_when_consoled() {
        let w = weep();
        assert_eq!(w.effective_sorrow(10.0), 0.0);
    }

    #[test]
    fn just_weeping_cleared_on_next_mourn() {
        let mut w = weep();
        w.mourn(100.0);
        assert!(w.just_weeping);
        w.mourn(1.0);
        assert!(!w.just_weeping);
    }

    #[test]
    fn just_consoled_cleared_on_next_console() {
        let mut w = weep();
        w.grief = 10.0;
        w.console(10.0);
        assert!(w.just_consoled);
        w.grief = 10.0;
        w.console(1.0);
        assert!(!w.just_consoled);
    }
}
