use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Waif {
    pub frailty: f32,
    pub max_frailty: f32,
    pub wither_rate: f32,
    pub just_withered: bool,
    pub just_hale: bool,
    pub enabled: bool,
}

impl Default for Waif {
    fn default() -> Self {
        Self {
            frailty: 0.0,
            max_frailty: 100.0,
            wither_rate: 1.0,
            just_withered: false,
            just_hale: false,
            enabled: true,
        }
    }
}

impl Waif {
    pub fn wither(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_withered = false;
        self.just_hale = false;
        let prev = self.frailty;
        self.frailty = (self.frailty + amount).clamp(0.0, self.max_frailty);
        if self.frailty >= self.max_frailty && prev < self.max_frailty {
            self.just_withered = true;
        }
    }

    pub fn nurture(&mut self, amount: f32) {
        if !self.enabled || self.frailty <= 0.0 {
            return;
        }
        self.just_withered = false;
        self.just_hale = false;
        let prev = self.frailty;
        self.frailty = (self.frailty - amount).max(0.0);
        if self.frailty <= 0.0 && prev > 0.0 {
            self.just_hale = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.frailty >= self.max_frailty {
            return;
        }
        self.wither(self.wither_rate * dt);
    }

    pub fn is_withered(&self) -> bool {
        self.enabled && self.frailty >= self.max_frailty
    }

    pub fn is_hale(&self) -> bool {
        self.frailty <= 0.0
    }

    pub fn frailty_fraction(&self) -> f32 {
        if self.max_frailty <= 0.0 {
            return 0.0;
        }
        self.frailty / self.max_frailty
    }

    pub fn effective_weakness(&self, scale: f32) -> f32 {
        self.frailty_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn waif() -> Waif {
        Waif {
            frailty: 0.0,
            max_frailty: 100.0,
            wither_rate: 10.0,
            just_withered: false,
            just_hale: false,
            enabled: true,
        }
    }

    #[test]
    fn default_frailty_zero() {
        let w = Waif::default();
        assert_eq!(w.frailty, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Waif::default().enabled);
    }

    #[test]
    fn wither_increases_frailty() {
        let mut w = waif();
        w.wither(30.0);
        assert_eq!(w.frailty, 30.0);
    }

    #[test]
    fn wither_clamps_at_max() {
        let mut w = waif();
        w.wither(200.0);
        assert_eq!(w.frailty, 100.0);
    }

    #[test]
    fn wither_no_op_when_disabled() {
        let mut w = waif();
        w.enabled = false;
        w.wither(50.0);
        assert_eq!(w.frailty, 0.0);
    }

    #[test]
    fn wither_sets_just_withered_at_max() {
        let mut w = waif();
        w.wither(100.0);
        assert!(w.just_withered);
    }

    #[test]
    fn wither_no_just_withered_if_already_max() {
        let mut w = waif();
        w.frailty = 100.0;
        w.wither(1.0);
        assert!(!w.just_withered);
    }

    #[test]
    fn nurture_decreases_frailty() {
        let mut w = waif();
        w.frailty = 60.0;
        w.nurture(20.0);
        assert_eq!(w.frailty, 40.0);
    }

    #[test]
    fn nurture_clamps_at_zero() {
        let mut w = waif();
        w.frailty = 30.0;
        w.nurture(200.0);
        assert_eq!(w.frailty, 0.0);
    }

    #[test]
    fn nurture_no_op_when_disabled() {
        let mut w = waif();
        w.frailty = 50.0;
        w.enabled = false;
        w.nurture(10.0);
        assert_eq!(w.frailty, 50.0);
    }

    #[test]
    fn nurture_no_op_when_already_hale() {
        let mut w = waif();
        w.nurture(10.0);
        assert_eq!(w.frailty, 0.0);
    }

    #[test]
    fn nurture_sets_just_hale_at_zero() {
        let mut w = waif();
        w.frailty = 10.0;
        w.nurture(10.0);
        assert!(w.just_hale);
    }

    #[test]
    fn nurture_no_just_hale_if_already_zero() {
        let mut w = waif();
        w.nurture(1.0);
        assert!(!w.just_hale);
    }

    #[test]
    fn tick_increases_frailty() {
        let mut w = waif();
        w.tick(1.0);
        assert_eq!(w.frailty, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = waif();
        w.tick(2.0);
        assert_eq!(w.frailty, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = waif();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.frailty, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_withered() {
        let mut w = waif();
        w.frailty = 100.0;
        w.tick(1.0);
        assert_eq!(w.frailty, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = waif();
        w.wither_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.frailty, 0.0);
    }

    #[test]
    fn is_withered_true_at_max() {
        let mut w = waif();
        w.frailty = 100.0;
        assert!(w.is_withered());
    }

    #[test]
    fn is_withered_false_below_max() {
        let mut w = waif();
        w.frailty = 50.0;
        assert!(!w.is_withered());
    }

    #[test]
    fn is_withered_false_when_disabled() {
        let mut w = waif();
        w.frailty = 100.0;
        w.enabled = false;
        assert!(!w.is_withered());
    }

    #[test]
    fn is_hale_true_at_zero() {
        let w = waif();
        assert!(w.is_hale());
    }

    #[test]
    fn is_hale_false_above_zero() {
        let mut w = waif();
        w.frailty = 1.0;
        assert!(!w.is_hale());
    }

    #[test]
    fn frailty_fraction_zero_when_hale() {
        let w = waif();
        assert_eq!(w.frailty_fraction(), 0.0);
    }

    #[test]
    fn frailty_fraction_one_at_max() {
        let mut w = waif();
        w.frailty = 100.0;
        assert_eq!(w.frailty_fraction(), 1.0);
    }

    #[test]
    fn frailty_fraction_half_at_midpoint() {
        let mut w = waif();
        w.frailty = 50.0;
        assert_eq!(w.frailty_fraction(), 0.5);
    }

    #[test]
    fn frailty_fraction_zero_when_max_zero() {
        let mut w = waif();
        w.max_frailty = 0.0;
        assert_eq!(w.frailty_fraction(), 0.0);
    }

    #[test]
    fn effective_weakness_scales() {
        let mut w = waif();
        w.frailty = 50.0;
        assert_eq!(w.effective_weakness(2.0), 1.0);
    }

    #[test]
    fn effective_weakness_zero_when_hale() {
        let w = waif();
        assert_eq!(w.effective_weakness(10.0), 0.0);
    }

    #[test]
    fn just_withered_cleared_on_next_wither() {
        let mut w = waif();
        w.wither(100.0);
        assert!(w.just_withered);
        w.wither(1.0);
        assert!(!w.just_withered);
    }

    #[test]
    fn just_hale_cleared_on_next_nurture() {
        let mut w = waif();
        w.frailty = 10.0;
        w.nurture(10.0);
        assert!(w.just_hale);
        w.frailty = 10.0;
        w.nurture(1.0);
        assert!(!w.just_hale);
    }
}
