use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Weak {
    pub frailty: f32,
    pub max_frailty: f32,
    pub sap_rate: f32,
    pub just_frail: bool,
    pub just_sturdy: bool,
    pub enabled: bool,
}

impl Default for Weak {
    fn default() -> Self {
        Self {
            frailty: 0.0,
            max_frailty: 100.0,
            sap_rate: 1.0,
            just_frail: false,
            just_sturdy: false,
            enabled: true,
        }
    }
}

impl Weak {
    pub fn sap(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_frail = false;
        self.just_sturdy = false;
        let prev = self.frailty;
        self.frailty = (self.frailty + amount).clamp(0.0, self.max_frailty);
        if self.frailty >= self.max_frailty && prev < self.max_frailty {
            self.just_frail = true;
        }
    }

    pub fn fortify(&mut self, amount: f32) {
        if !self.enabled || self.frailty <= 0.0 {
            return;
        }
        self.just_frail = false;
        self.just_sturdy = false;
        let prev = self.frailty;
        self.frailty = (self.frailty - amount).max(0.0);
        if self.frailty <= 0.0 && prev > 0.0 {
            self.just_sturdy = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.frailty >= self.max_frailty {
            return;
        }
        self.sap(self.sap_rate * dt);
    }

    pub fn is_frail(&self) -> bool {
        self.enabled && self.frailty >= self.max_frailty
    }

    pub fn is_sturdy(&self) -> bool {
        self.frailty <= 0.0
    }

    pub fn frailty_fraction(&self) -> f32 {
        if self.max_frailty <= 0.0 {
            return 0.0;
        }
        self.frailty / self.max_frailty
    }

    pub fn effective_debilitation(&self, scale: f32) -> f32 {
        self.frailty_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weak() -> Weak {
        Weak {
            frailty: 0.0,
            max_frailty: 100.0,
            sap_rate: 10.0,
            just_frail: false,
            just_sturdy: false,
            enabled: true,
        }
    }

    #[test]
    fn default_frailty_zero() {
        let w = Weak::default();
        assert_eq!(w.frailty, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Weak::default().enabled);
    }

    #[test]
    fn sap_increases_frailty() {
        let mut w = weak();
        w.sap(30.0);
        assert_eq!(w.frailty, 30.0);
    }

    #[test]
    fn sap_clamps_at_max() {
        let mut w = weak();
        w.sap(200.0);
        assert_eq!(w.frailty, 100.0);
    }

    #[test]
    fn sap_no_op_when_disabled() {
        let mut w = weak();
        w.enabled = false;
        w.sap(50.0);
        assert_eq!(w.frailty, 0.0);
    }

    #[test]
    fn sap_sets_just_frail_at_max() {
        let mut w = weak();
        w.sap(100.0);
        assert!(w.just_frail);
    }

    #[test]
    fn sap_no_just_frail_if_already_max() {
        let mut w = weak();
        w.frailty = 100.0;
        w.sap(1.0);
        assert!(!w.just_frail);
    }

    #[test]
    fn fortify_decreases_frailty() {
        let mut w = weak();
        w.frailty = 60.0;
        w.fortify(20.0);
        assert_eq!(w.frailty, 40.0);
    }

    #[test]
    fn fortify_clamps_at_zero() {
        let mut w = weak();
        w.frailty = 30.0;
        w.fortify(200.0);
        assert_eq!(w.frailty, 0.0);
    }

    #[test]
    fn fortify_no_op_when_disabled() {
        let mut w = weak();
        w.frailty = 50.0;
        w.enabled = false;
        w.fortify(10.0);
        assert_eq!(w.frailty, 50.0);
    }

    #[test]
    fn fortify_no_op_when_already_sturdy() {
        let mut w = weak();
        w.fortify(10.0);
        assert_eq!(w.frailty, 0.0);
    }

    #[test]
    fn fortify_sets_just_sturdy_at_zero() {
        let mut w = weak();
        w.frailty = 10.0;
        w.fortify(10.0);
        assert!(w.just_sturdy);
    }

    #[test]
    fn fortify_no_just_sturdy_if_already_zero() {
        let mut w = weak();
        w.fortify(1.0);
        assert!(!w.just_sturdy);
    }

    #[test]
    fn tick_increases_frailty() {
        let mut w = weak();
        w.tick(1.0);
        assert_eq!(w.frailty, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = weak();
        w.tick(2.0);
        assert_eq!(w.frailty, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = weak();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.frailty, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_frail() {
        let mut w = weak();
        w.frailty = 100.0;
        w.tick(1.0);
        assert_eq!(w.frailty, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = weak();
        w.sap_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.frailty, 0.0);
    }

    #[test]
    fn is_frail_true_at_max() {
        let mut w = weak();
        w.frailty = 100.0;
        assert!(w.is_frail());
    }

    #[test]
    fn is_frail_false_below_max() {
        let mut w = weak();
        w.frailty = 50.0;
        assert!(!w.is_frail());
    }

    #[test]
    fn is_frail_false_when_disabled() {
        let mut w = weak();
        w.frailty = 100.0;
        w.enabled = false;
        assert!(!w.is_frail());
    }

    #[test]
    fn is_sturdy_true_at_zero() {
        let w = weak();
        assert!(w.is_sturdy());
    }

    #[test]
    fn is_sturdy_false_above_zero() {
        let mut w = weak();
        w.frailty = 1.0;
        assert!(!w.is_sturdy());
    }

    #[test]
    fn frailty_fraction_zero_when_sturdy() {
        let w = weak();
        assert_eq!(w.frailty_fraction(), 0.0);
    }

    #[test]
    fn frailty_fraction_one_at_max() {
        let mut w = weak();
        w.frailty = 100.0;
        assert_eq!(w.frailty_fraction(), 1.0);
    }

    #[test]
    fn frailty_fraction_half_at_midpoint() {
        let mut w = weak();
        w.frailty = 50.0;
        assert_eq!(w.frailty_fraction(), 0.5);
    }

    #[test]
    fn frailty_fraction_zero_when_max_zero() {
        let mut w = weak();
        w.max_frailty = 0.0;
        assert_eq!(w.frailty_fraction(), 0.0);
    }

    #[test]
    fn effective_debilitation_scales() {
        let mut w = weak();
        w.frailty = 50.0;
        assert_eq!(w.effective_debilitation(2.0), 1.0);
    }

    #[test]
    fn effective_debilitation_zero_when_sturdy() {
        let w = weak();
        assert_eq!(w.effective_debilitation(10.0), 0.0);
    }

    #[test]
    fn just_frail_cleared_on_next_sap() {
        let mut w = weak();
        w.sap(100.0);
        assert!(w.just_frail);
        w.sap(1.0);
        assert!(!w.just_frail);
    }

    #[test]
    fn just_sturdy_cleared_on_next_fortify() {
        let mut w = weak();
        w.frailty = 10.0;
        w.fortify(10.0);
        assert!(w.just_sturdy);
        w.frailty = 10.0;
        w.fortify(1.0);
        assert!(!w.just_sturdy);
    }
}
