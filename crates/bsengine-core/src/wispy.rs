use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wispy {
    pub ethereal: f32,
    pub max_ethereal: f32,
    pub drift_rate: f32,
    pub just_vaporous: bool,
    pub just_solid: bool,
    pub enabled: bool,
}

impl Default for Wispy {
    fn default() -> Self {
        Self {
            ethereal: 0.0,
            max_ethereal: 100.0,
            drift_rate: 1.0,
            just_vaporous: false,
            just_solid: false,
            enabled: true,
        }
    }
}

impl Wispy {
    pub fn drift(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_vaporous = false;
        self.just_solid = false;
        let prev = self.ethereal;
        self.ethereal = (self.ethereal + amount).clamp(0.0, self.max_ethereal);
        if self.ethereal >= self.max_ethereal && prev < self.max_ethereal {
            self.just_vaporous = true;
        }
    }

    pub fn condense(&mut self, amount: f32) {
        if !self.enabled || self.ethereal <= 0.0 {
            return;
        }
        self.just_vaporous = false;
        self.just_solid = false;
        let prev = self.ethereal;
        self.ethereal = (self.ethereal - amount).max(0.0);
        if self.ethereal <= 0.0 && prev > 0.0 {
            self.just_solid = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.ethereal >= self.max_ethereal {
            return;
        }
        self.drift(self.drift_rate * dt);
    }

    pub fn is_vaporous(&self) -> bool {
        self.enabled && self.ethereal >= self.max_ethereal
    }

    pub fn is_solid(&self) -> bool {
        self.ethereal <= 0.0
    }

    pub fn ethereal_fraction(&self) -> f32 {
        if self.max_ethereal <= 0.0 {
            return 0.0;
        }
        self.ethereal / self.max_ethereal
    }

    pub fn effective_mist(&self, scale: f32) -> f32 {
        self.ethereal_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wispy() -> Wispy {
        Wispy {
            ethereal: 0.0,
            max_ethereal: 100.0,
            drift_rate: 10.0,
            just_vaporous: false,
            just_solid: false,
            enabled: true,
        }
    }

    #[test]
    fn default_ethereal_zero() {
        let w = Wispy::default();
        assert_eq!(w.ethereal, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wispy::default().enabled);
    }

    #[test]
    fn drift_increases_ethereal() {
        let mut w = wispy();
        w.drift(30.0);
        assert_eq!(w.ethereal, 30.0);
    }

    #[test]
    fn drift_clamps_at_max() {
        let mut w = wispy();
        w.drift(200.0);
        assert_eq!(w.ethereal, 100.0);
    }

    #[test]
    fn drift_no_op_when_disabled() {
        let mut w = wispy();
        w.enabled = false;
        w.drift(50.0);
        assert_eq!(w.ethereal, 0.0);
    }

    #[test]
    fn drift_sets_just_vaporous_at_max() {
        let mut w = wispy();
        w.drift(100.0);
        assert!(w.just_vaporous);
    }

    #[test]
    fn drift_no_just_vaporous_if_already_max() {
        let mut w = wispy();
        w.ethereal = 100.0;
        w.drift(1.0);
        assert!(!w.just_vaporous);
    }

    #[test]
    fn condense_decreases_ethereal() {
        let mut w = wispy();
        w.ethereal = 60.0;
        w.condense(20.0);
        assert_eq!(w.ethereal, 40.0);
    }

    #[test]
    fn condense_clamps_at_zero() {
        let mut w = wispy();
        w.ethereal = 30.0;
        w.condense(200.0);
        assert_eq!(w.ethereal, 0.0);
    }

    #[test]
    fn condense_no_op_when_disabled() {
        let mut w = wispy();
        w.ethereal = 50.0;
        w.enabled = false;
        w.condense(10.0);
        assert_eq!(w.ethereal, 50.0);
    }

    #[test]
    fn condense_no_op_when_already_solid() {
        let mut w = wispy();
        w.condense(10.0);
        assert_eq!(w.ethereal, 0.0);
    }

    #[test]
    fn condense_sets_just_solid_at_zero() {
        let mut w = wispy();
        w.ethereal = 10.0;
        w.condense(10.0);
        assert!(w.just_solid);
    }

    #[test]
    fn condense_no_just_solid_if_already_zero() {
        let mut w = wispy();
        w.condense(1.0);
        assert!(!w.just_solid);
    }

    #[test]
    fn tick_increases_ethereal() {
        let mut w = wispy();
        w.tick(1.0);
        assert_eq!(w.ethereal, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wispy();
        w.tick(2.0);
        assert_eq!(w.ethereal, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wispy();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.ethereal, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_vaporous() {
        let mut w = wispy();
        w.ethereal = 100.0;
        w.tick(1.0);
        assert_eq!(w.ethereal, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wispy();
        w.drift_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.ethereal, 0.0);
    }

    #[test]
    fn is_vaporous_true_at_max() {
        let mut w = wispy();
        w.ethereal = 100.0;
        assert!(w.is_vaporous());
    }

    #[test]
    fn is_vaporous_false_below_max() {
        let mut w = wispy();
        w.ethereal = 50.0;
        assert!(!w.is_vaporous());
    }

    #[test]
    fn is_vaporous_false_when_disabled() {
        let mut w = wispy();
        w.ethereal = 100.0;
        w.enabled = false;
        assert!(!w.is_vaporous());
    }

    #[test]
    fn is_solid_true_at_zero() {
        let w = wispy();
        assert!(w.is_solid());
    }

    #[test]
    fn is_solid_false_above_zero() {
        let mut w = wispy();
        w.ethereal = 1.0;
        assert!(!w.is_solid());
    }

    #[test]
    fn ethereal_fraction_zero_when_solid() {
        let w = wispy();
        assert_eq!(w.ethereal_fraction(), 0.0);
    }

    #[test]
    fn ethereal_fraction_one_at_max() {
        let mut w = wispy();
        w.ethereal = 100.0;
        assert_eq!(w.ethereal_fraction(), 1.0);
    }

    #[test]
    fn ethereal_fraction_half_at_midpoint() {
        let mut w = wispy();
        w.ethereal = 50.0;
        assert_eq!(w.ethereal_fraction(), 0.5);
    }

    #[test]
    fn ethereal_fraction_zero_when_max_zero() {
        let mut w = wispy();
        w.max_ethereal = 0.0;
        assert_eq!(w.ethereal_fraction(), 0.0);
    }

    #[test]
    fn effective_mist_scales() {
        let mut w = wispy();
        w.ethereal = 50.0;
        assert_eq!(w.effective_mist(2.0), 1.0);
    }

    #[test]
    fn effective_mist_zero_when_solid() {
        let w = wispy();
        assert_eq!(w.effective_mist(10.0), 0.0);
    }

    #[test]
    fn just_vaporous_cleared_on_next_drift() {
        let mut w = wispy();
        w.drift(100.0);
        assert!(w.just_vaporous);
        w.drift(1.0);
        assert!(!w.just_vaporous);
    }

    #[test]
    fn just_solid_cleared_on_next_condense() {
        let mut w = wispy();
        w.ethereal = 10.0;
        w.condense(10.0);
        assert!(w.just_solid);
        w.ethereal = 10.0;
        w.condense(1.0);
        assert!(!w.just_solid);
    }
}
